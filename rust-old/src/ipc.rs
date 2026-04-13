//! Inter-process communication — Unix domain sockets, message bus, and RPC.
//!
//! Three layers:
//! - [`AgentIpc`] — per-agent Unix socket endpoint with length-prefixed framing
//! - [`MessageBus`] — in-memory pub/sub message routing between agents
//! - [`RpcRegistry`] / [`RpcRouter`] — typed request-response RPC over the bus

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use agnostik::{AgentId, MessageType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum message payload size (64 KiB).
const MAX_MESSAGE_SIZE: u32 = 64 * 1024;

/// Maximum monitoring subscribers.
const MAX_GLOBAL_SUBSCRIBERS: usize = 16;

/// Maximum concurrent connections per socket.
const MAX_CONCURRENT_CONNECTIONS: usize = 64;

/// Global file descriptor limit across all sockets (H15).
const MAX_GLOBAL_FD_LIMIT: usize = 1024;

// Wire protocol response codes.
const ACK: u8 = 0x01;
const NACK_QUEUE_FULL: u8 = 0x02;
const NACK_INVALID: u8 = 0x03;

/// Global connection counter.
static GLOBAL_ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

/// Current number of active IPC connections.
#[must_use]
pub fn active_connection_count() -> usize {
    GLOBAL_ACTIVE_CONNECTIONS.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// IpcMessage — wire-format message
// ---------------------------------------------------------------------------

/// An IPC message exchanged between agents over Unix sockets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IpcMessage {
    /// Unique message identifier.
    pub id: String,
    /// Source agent name or ID.
    pub source: String,
    /// Target agent name, ID, `"*"`, or `"broadcast"`.
    pub target: String,
    /// Message classification.
    pub message_type: MessageType,
    /// Arbitrary JSON payload.
    pub payload: serde_json::Value,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
}

impl IpcMessage {
    /// Create a new message.
    #[must_use]
    pub fn new(
        source: impl Into<String>,
        target: impl Into<String>,
        message_type: MessageType,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source: source.into(),
            target: target.into(),
            message_type,
            payload,
            timestamp: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// AgentIpc — per-agent Unix socket endpoint
// ---------------------------------------------------------------------------

/// Per-agent Unix socket IPC endpoint.
///
/// Creates a socket at `{socket_dir}/{agent_id}.sock` and listens for
/// length-prefixed JSON messages.
pub struct AgentIpc {
    agent_id: AgentId,
    socket_path: PathBuf,
    message_tx: mpsc::Sender<IpcMessage>,
}

impl AgentIpc {
    /// Create a new IPC endpoint. Returns the endpoint and a receiver for incoming messages.
    pub fn new(agent_id: AgentId, socket_dir: &Path) -> Result<(Self, mpsc::Receiver<IpcMessage>)> {
        let socket_path = socket_dir.join(format!("{agent_id}.sock"));
        let (tx, rx) = mpsc::channel(100);

        let ipc = Self {
            agent_id,
            socket_path,
            message_tx: tx,
        };

        Ok((ipc, rx))
    }

    /// Start listening for incoming connections.
    ///
    /// Creates the socket directory, cleans up stale sockets, binds, and
    /// spawns a listener task.
    pub async fn start_listening(&self) -> Result<()> {
        if let Some(parent) = self.socket_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| DaimonError::IpcError(format!("create socket dir: {e}")))?;
        }

        cleanup_stale_socket(&self.socket_path).await;

        let listener = UnixListener::bind(&self.socket_path).map_err(|e| {
            DaimonError::IpcError(format!("bind {}: {e}", self.socket_path.display()))
        })?;

        // Set restrictive permissions (owner-only).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            if let Err(e) = std::fs::set_permissions(&self.socket_path, perms) {
                warn!(path = %self.socket_path.display(), err = %e, "failed to set socket permissions");
            }
        }

        info!(agent_id = %self.agent_id, path = %self.socket_path.display(), "IPC listening");

        let tx = self.message_tx.clone();
        let agent_id = self.agent_id;
        let semaphore =
            std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

        tokio::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        warn!("IPC accept error: {e}");
                        continue;
                    }
                };

                let current = GLOBAL_ACTIVE_CONNECTIONS.load(Ordering::Relaxed);
                if current >= MAX_GLOBAL_FD_LIMIT {
                    warn!("global FD limit reached ({current}), rejecting connection");
                    drop(stream);
                    continue;
                }

                let permit = match semaphore.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        warn!("per-socket connection limit reached for agent {agent_id}");
                        drop(stream);
                        continue;
                    }
                };

                GLOBAL_ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
                let tx = tx.clone();

                tokio::spawn(async move {
                    handle_connection(stream, tx, agent_id).await;
                    GLOBAL_ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
                    drop(permit);
                });
            }
        });

        Ok(())
    }

    /// Send a message to this agent's queue.
    pub async fn send(&self, message: IpcMessage) -> Result<()> {
        self.message_tx
            .send(message)
            .await
            .map_err(|_| DaimonError::IpcError("channel closed".into()))?;
        Ok(())
    }

    /// Socket path for this agent.
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl Drop for AgentIpc {
    fn drop(&mut self) {
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).ok();
        }
    }
}

// ---------------------------------------------------------------------------
// Connection handling (wire protocol)
// ---------------------------------------------------------------------------

/// Handle a single client connection using length-prefixed JSON framing.
async fn handle_connection(
    mut stream: UnixStream,
    tx: mpsc::Sender<IpcMessage>,
    owner_agent_id: AgentId,
) {
    loop {
        // Read 4-byte big-endian length prefix.
        let len = match stream.read_u32().await {
            Ok(0) => continue, // zero-length: keep-alive / skip
            Ok(n) => n,
            Err(_) => break, // connection closed or error
        };

        if len > MAX_MESSAGE_SIZE {
            error!(
                agent_id = %owner_agent_id,
                len,
                max = MAX_MESSAGE_SIZE,
                "message exceeds size limit"
            );
            break;
        }

        let mut buf = vec![0u8; len as usize];
        if stream.read_exact(&mut buf).await.is_err() {
            break;
        }

        match serde_json::from_slice::<IpcMessage>(&buf) {
            Ok(msg) => {
                let ack = match tx.try_send(msg) {
                    Ok(()) => ACK,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        warn!(agent_id = %owner_agent_id, "IPC queue full");
                        NACK_QUEUE_FULL
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        error!(agent_id = %owner_agent_id, "IPC channel closed");
                        break;
                    }
                };
                if stream.write_u8(ack).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                debug!(agent_id = %owner_agent_id, err = %e, "invalid IPC message");
                if stream.write_u8(NACK_INVALID).await.is_err() {
                    break;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MessageBus — in-memory pub/sub
// ---------------------------------------------------------------------------

/// In-memory message router between agents.
///
/// Supports named routing, broadcast, and global monitoring subscribers.
pub struct MessageBus {
    subscribers: RwLock<HashMap<AgentId, mpsc::Sender<IpcMessage>>>,
    agent_names: RwLock<HashMap<String, AgentId>>,
    global_subscribers: RwLock<Vec<mpsc::Sender<IpcMessage>>>,
}

impl MessageBus {
    /// Create a new message bus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
            agent_names: RwLock::new(HashMap::new()),
            global_subscribers: RwLock::new(Vec::new()),
        }
    }

    /// Register an agent to receive messages.
    pub async fn subscribe(&self, agent_id: AgentId, sender: mpsc::Sender<IpcMessage>) {
        self.subscribers.write().await.insert(agent_id, sender);
    }

    /// Unregister an agent.
    pub async fn unsubscribe(&self, agent_id: AgentId) {
        self.subscribers.write().await.remove(&agent_id);
    }

    /// Map an agent name to its ID for named routing.
    pub async fn register_agent_name(&self, agent_id: AgentId, name: &str) {
        self.agent_names
            .write()
            .await
            .insert(name.to_string(), agent_id);
    }

    /// Remove a named mapping.
    pub async fn unregister_agent_name(&self, name: &str) {
        self.agent_names.write().await.remove(name);
    }

    /// Look up an agent ID by name.
    pub async fn get_agent_id(&self, name: &str) -> Option<AgentId> {
        self.agent_names.read().await.get(name).copied()
    }

    /// Subscribe to all messages (monitoring). Limited to 16 global subscribers.
    pub async fn subscribe_global(&self, sender: mpsc::Sender<IpcMessage>) -> Result<()> {
        let mut globals = self.global_subscribers.write().await;
        if globals.len() >= MAX_GLOBAL_SUBSCRIBERS {
            return Err(DaimonError::IpcError(format!(
                "global subscriber limit ({MAX_GLOBAL_SUBSCRIBERS}) reached"
            )));
        }
        globals.push(sender);
        Ok(())
    }

    /// Publish a message. Routes by target name or broadcasts.
    pub async fn publish(&self, message: IpcMessage) -> Result<()> {
        let is_broadcast =
            message.target == "*" || message.target.eq_ignore_ascii_case("broadcast");

        let subs = self.subscribers.read().await;

        if is_broadcast {
            for tx in subs.values() {
                if tx.try_send(message.clone()).is_err() {
                    warn!("dropped broadcast message (queue full or closed)");
                }
            }
        } else {
            let names = self.agent_names.read().await;
            if let Some(id) = names.get(&message.target) {
                if let Some(tx) = subs.get(id)
                    && tx.try_send(message.clone()).is_err()
                {
                    warn!(target_agent = %message.target, "dropped directed message (queue full or closed)");
                }
            } else {
                debug!(target = %message.target, "target not found, broadcasting");
                for tx in subs.values() {
                    if tx.try_send(message.clone()).is_err() {
                        warn!("dropped fallback broadcast message (queue full or closed)");
                    }
                }
            }
        }

        // Always send to global subscribers.
        let globals = self.global_subscribers.read().await;
        for tx in globals.iter() {
            if tx.try_send(message.clone()).is_err() {
                warn!("dropped global subscriber message (queue full or closed)");
            }
        }

        Ok(())
    }

    /// Send directly to an agent by ID.
    pub async fn send_to(&self, agent_id: AgentId, message: IpcMessage) -> Result<()> {
        let subs = self.subscribers.read().await;
        if let Some(tx) = subs.get(&agent_id) {
            tx.try_send(message)
                .map_err(|_| DaimonError::IpcError("agent queue full or closed".into()))?;
            Ok(())
        } else {
            Err(DaimonError::AgentNotFound(agent_id.to_string()))
        }
    }

    /// Send directly to an agent by name.
    pub async fn send_to_name(&self, name: &str, message: IpcMessage) -> Result<()> {
        let id = self
            .get_agent_id(name)
            .await
            .ok_or_else(|| DaimonError::AgentNotFound(name.into()))?;
        self.send_to(id, message).await
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RPC types
// ---------------------------------------------------------------------------

/// RPC error with numeric code and message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RpcError {
    /// Error code (negative = system, positive = application).
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
}

impl RpcError {
    /// Create a new RPC error.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Method not found error (code -1).
    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self::new(-1, format!("method not found: {method}"))
    }

    /// Timeout error (code -2).
    #[must_use]
    pub fn timeout(method: &str, timeout_ms: u64) -> Self {
        Self::new(-2, format!("{method} timed out after {timeout_ms}ms"))
    }

    /// Internal error (code -3).
    #[must_use]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(-3, msg)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// An RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RpcRequest {
    /// Request identifier (for matching responses).
    pub id: Uuid,
    /// Method name to invoke.
    pub method: String,
    /// Method parameters.
    pub params: serde_json::Value,
    /// Timeout in milliseconds (0 = no timeout).
    pub timeout_ms: u64,
    /// ID of the calling agent.
    pub sender_id: AgentId,
}

impl RpcRequest {
    /// Create a new RPC request with a 5-second default timeout.
    pub fn new(sender_id: AgentId, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            method: method.into(),
            params,
            timeout_ms: 5000,
            sender_id,
        }
    }

    /// Set a custom timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// An RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RpcResponse {
    /// The request this response corresponds to.
    pub request_id: Uuid,
    /// Result: Ok(value) or Err(RpcError).
    pub result: std::result::Result<serde_json::Value, RpcError>,
    /// Time taken to produce the response in milliseconds.
    pub duration_ms: u64,
}

impl RpcResponse {
    /// Create a success response.
    #[must_use]
    pub fn success(request_id: Uuid, value: serde_json::Value, duration_ms: u64) -> Self {
        Self {
            request_id,
            result: Ok(value),
            duration_ms,
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn error(request_id: Uuid, err: RpcError, duration_ms: u64) -> Self {
        Self {
            request_id,
            result: Err(err),
            duration_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// RpcRegistry
// ---------------------------------------------------------------------------

/// Registry mapping RPC method names to handler agents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RpcRegistry {
    methods: HashMap<String, AgentId>,
    agent_methods: HashMap<AgentId, Vec<String>>,
}

impl RpcRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a method as handled by the given agent.
    pub fn register_method(&mut self, agent_id: AgentId, method: &str) {
        self.methods.insert(method.to_string(), agent_id);
        self.agent_methods
            .entry(agent_id)
            .or_default()
            .push(method.to_string());
    }

    /// Find which agent handles a method.
    #[must_use]
    pub fn find_handler(&self, method: &str) -> Option<AgentId> {
        self.methods.get(method).copied()
    }

    /// List all methods provided by an agent.
    #[must_use]
    pub fn list_methods(&self, agent_id: &AgentId) -> Vec<String> {
        self.agent_methods
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all registered (method, handler) pairs.
    #[must_use]
    pub fn all_methods(&self) -> Vec<(String, AgentId)> {
        self.methods
            .iter()
            .map(|(m, id)| (m.clone(), *id))
            .collect()
    }

    /// Unregister all methods for an agent.
    pub fn unregister_agent(&mut self, agent_id: &AgentId) {
        if let Some(methods) = self.agent_methods.remove(agent_id) {
            for m in methods {
                self.methods.remove(&m);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// RpcRouter
// ---------------------------------------------------------------------------

struct PendingCall {
    sender: tokio::sync::oneshot::Sender<RpcResponse>,
    sent_at: std::time::Instant,
}

/// Routes RPC calls to registered handlers and manages pending responses.
pub struct RpcRouter {
    registry: RpcRegistry,
    pending: std::sync::Mutex<HashMap<Uuid, PendingCall>>,
}

impl RpcRouter {
    /// Create a new router with the given registry.
    #[must_use]
    pub fn new(registry: RpcRegistry) -> Self {
        Self {
            registry,
            pending: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Issue an RPC call. Waits for a response (with optional timeout).
    pub async fn call(&self, request: RpcRequest) -> Result<RpcResponse> {
        if self.registry.find_handler(&request.method).is_none() {
            return Ok(RpcResponse::error(
                request.id,
                RpcError::method_not_found(&request.method),
                0,
            ));
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| DaimonError::IpcError("pending RPC store lock poisoned".into()))?;
            pending.insert(
                request.id,
                PendingCall {
                    sender: tx,
                    sent_at: std::time::Instant::now(),
                },
            );
        }

        if request.timeout_ms > 0 {
            match tokio::time::timeout(std::time::Duration::from_millis(request.timeout_ms), rx)
                .await
            {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(_)) => {
                    self.cleanup_pending(&request.id);
                    Ok(RpcResponse::error(
                        request.id,
                        RpcError::internal("handler dropped"),
                        0,
                    ))
                }
                Err(_) => {
                    self.cleanup_pending(&request.id);
                    Ok(RpcResponse::error(
                        request.id,
                        RpcError::timeout(&request.method, request.timeout_ms),
                        request.timeout_ms,
                    ))
                }
            }
        } else {
            match rx.await {
                Ok(resp) => Ok(resp),
                Err(_) => Ok(RpcResponse::error(
                    request.id,
                    RpcError::internal("handler dropped"),
                    0,
                )),
            }
        }
    }

    /// Deliver a response for a pending call.
    pub fn handle_response(&self, response: RpcResponse) {
        let mut pending = match self.pending.lock() {
            Ok(p) => p,
            Err(e) => {
                error!("pending RPC store lock poisoned: {e}");
                return;
            }
        };
        if let Some(call) = pending.remove(&response.request_id) {
            debug!(
                request_id = %response.request_id,
                elapsed_ms = call.sent_at.elapsed().as_millis() as u64,
                "RPC response delivered"
            );
            let _ = call.sender.send(response);
        }
    }

    /// Number of calls awaiting a response.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }

    fn cleanup_pending(&self, request_id: &Uuid) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(request_id);
        }
    }
}

// ---------------------------------------------------------------------------
// Stale socket cleanup
// ---------------------------------------------------------------------------

/// Remove a stale socket file (one that no process is listening on).
pub async fn cleanup_stale_socket(path: &Path) {
    if !path.exists() {
        return;
    }

    match tokio::time::timeout(
        std::time::Duration::from_millis(500),
        UnixStream::connect(path),
    )
    .await
    {
        Ok(Ok(_)) => {
            // Another process is listening — leave it alone.
        }
        _ => {
            // Stale or unreachable — remove.
            if std::fs::remove_file(path).is_ok() {
                debug!(path = %path.display(), "removed stale socket");
            }
        }
    }
}

/// Clean up stale `.sock` files in a directory.
pub async fn cleanup_stale_sockets_in_dir(dir: &Path) {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sock") {
            cleanup_stale_socket(&path).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_agent_id(n: u8) -> AgentId {
        AgentId(uuid::Uuid::from_bytes([n; 16]))
    }

    // -- IpcMessage --

    #[test]
    fn ipc_message_serde_roundtrip() {
        let msg = IpcMessage::new("src", "dst", MessageType::Command, json!({"op": "ping"}));
        let json = serde_json::to_string(&msg).unwrap();
        let back: IpcMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, "src");
        assert_eq!(back.target, "dst");
        assert_eq!(back.message_type, MessageType::Command);
    }

    #[test]
    fn ipc_message_unique_ids() {
        let m1 = IpcMessage::new("a", "b", MessageType::Event, json!(null));
        let m2 = IpcMessage::new("a", "b", MessageType::Event, json!(null));
        assert_ne!(m1.id, m2.id);
    }

    // -- AgentIpc --

    #[test]
    fn agent_ipc_socket_path() {
        let id = test_agent_id(1);
        let dir = tempfile::tempdir().unwrap();
        let (ipc, _rx) = AgentIpc::new(id, dir.path()).unwrap();
        assert!(ipc.socket_path().to_str().unwrap().ends_with(".sock"));
        assert!(
            ipc.socket_path()
                .to_str()
                .unwrap()
                .contains(&id.to_string())
        );
    }

    #[tokio::test]
    async fn agent_ipc_send() {
        let id = test_agent_id(2);
        let dir = tempfile::tempdir().unwrap();
        let (ipc, mut rx) = AgentIpc::new(id, dir.path()).unwrap();

        let msg = IpcMessage::new("test", "agent", MessageType::Command, json!("hello"));
        ipc.send(msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.payload, json!("hello"));
    }

    // -- MessageBus --

    #[tokio::test]
    async fn bus_subscribe_and_send() {
        let bus = MessageBus::new();
        let id = test_agent_id(1);
        let (tx, mut rx) = mpsc::channel(10);
        bus.subscribe(id, tx).await;

        let msg = IpcMessage::new("src", "*", MessageType::Event, json!("data"));
        bus.publish(msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.payload, json!("data"));
    }

    #[tokio::test]
    async fn bus_named_routing() {
        let bus = MessageBus::new();
        let id1 = test_agent_id(1);
        let id2 = test_agent_id(2);
        let (tx1, mut rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);

        bus.subscribe(id1, tx1).await;
        bus.subscribe(id2, tx2).await;
        bus.register_agent_name(id1, "scanner").await;

        let msg = IpcMessage::new("src", "scanner", MessageType::Command, json!("scan"));
        bus.publish(msg).await.unwrap();

        // Only scanner should receive it.
        let received = rx1.recv().await.unwrap();
        assert_eq!(received.payload, json!("scan"));
        assert!(rx2.try_recv().is_err());
    }

    #[tokio::test]
    async fn bus_broadcast() {
        let bus = MessageBus::new();
        let (tx1, mut rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);
        bus.subscribe(test_agent_id(1), tx1).await;
        bus.subscribe(test_agent_id(2), tx2).await;

        let msg = IpcMessage::new("src", "*", MessageType::Event, json!("all"));
        bus.publish(msg).await.unwrap();

        assert!(rx1.recv().await.is_some());
        assert!(rx2.recv().await.is_some());
    }

    #[tokio::test]
    async fn bus_unsubscribe() {
        let bus = MessageBus::new();
        let id = test_agent_id(1);
        let (tx, _rx) = mpsc::channel(10);
        bus.subscribe(id, tx).await;
        bus.unsubscribe(id).await;

        let msg = IpcMessage::new("src", "*", MessageType::Event, json!(null));
        bus.publish(msg).await.unwrap();
        // No panic — message is silently dropped.
    }

    #[tokio::test]
    async fn bus_send_to_by_id() {
        let bus = MessageBus::new();
        let id = test_agent_id(1);
        let (tx, mut rx) = mpsc::channel(10);
        bus.subscribe(id, tx).await;

        let msg = IpcMessage::new("src", "direct", MessageType::Command, json!("hi"));
        bus.send_to(id, msg).await.unwrap();
        assert!(rx.recv().await.is_some());
    }

    #[tokio::test]
    async fn bus_send_to_unknown_agent() {
        let bus = MessageBus::new();
        let msg = IpcMessage::new("src", "x", MessageType::Event, json!(null));
        assert!(bus.send_to(test_agent_id(99), msg).await.is_err());
    }

    #[tokio::test]
    async fn bus_send_to_name() {
        let bus = MessageBus::new();
        let id = test_agent_id(1);
        let (tx, mut rx) = mpsc::channel(10);
        bus.subscribe(id, tx).await;
        bus.register_agent_name(id, "worker").await;

        let msg = IpcMessage::new("src", "worker", MessageType::Command, json!("work"));
        bus.send_to_name("worker", msg).await.unwrap();
        assert_eq!(rx.recv().await.unwrap().payload, json!("work"));
    }

    #[tokio::test]
    async fn bus_global_subscriber() {
        let bus = MessageBus::new();
        let (global_tx, mut global_rx) = mpsc::channel(10);
        bus.subscribe_global(global_tx).await.unwrap();

        let id = test_agent_id(1);
        let (tx, _rx) = mpsc::channel(10);
        bus.subscribe(id, tx).await;

        let msg = IpcMessage::new("src", "*", MessageType::Event, json!("monitored"));
        bus.publish(msg).await.unwrap();

        assert_eq!(global_rx.recv().await.unwrap().payload, json!("monitored"));
    }

    #[tokio::test]
    async fn bus_global_subscriber_limit() {
        let bus = MessageBus::new();
        for _ in 0..MAX_GLOBAL_SUBSCRIBERS {
            let (tx, _rx) = mpsc::channel(1);
            bus.subscribe_global(tx).await.unwrap();
        }
        let (tx, _rx) = mpsc::channel(1);
        assert!(bus.subscribe_global(tx).await.is_err());
    }

    // -- RPC types --

    #[test]
    fn rpc_error_serde_roundtrip() {
        let err = RpcError::method_not_found("scan");
        let json = serde_json::to_string(&err).unwrap();
        let back: RpcError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, -1);
        assert!(back.message.contains("scan"));
    }

    #[test]
    fn rpc_request_default_timeout() {
        let req = RpcRequest::new(test_agent_id(1), "ping", json!(null));
        assert_eq!(req.timeout_ms, 5000);
    }

    #[test]
    fn rpc_request_custom_timeout() {
        let req = RpcRequest::new(test_agent_id(1), "slow", json!(null)).with_timeout(30000);
        assert_eq!(req.timeout_ms, 30000);
    }

    #[test]
    fn rpc_response_success() {
        let resp = RpcResponse::success(Uuid::new_v4(), json!(42), 10);
        assert!(resp.result.is_ok());
        assert_eq!(resp.duration_ms, 10);
    }

    #[test]
    fn rpc_response_error() {
        let resp = RpcResponse::error(Uuid::new_v4(), RpcError::internal("boom"), 5);
        assert!(resp.result.is_err());
    }

    #[test]
    fn rpc_response_serde_roundtrip() {
        let resp = RpcResponse::success(Uuid::new_v4(), json!({"ok": true}), 42);
        let json = serde_json::to_string(&resp).unwrap();
        let back: RpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.duration_ms, 42);
        assert!(back.result.is_ok());
    }

    // -- RpcRegistry --

    #[test]
    fn registry_register_and_find() {
        let mut reg = RpcRegistry::new();
        let id = test_agent_id(1);
        reg.register_method(id, "scan_ports");
        assert_eq!(reg.find_handler("scan_ports"), Some(id));
        assert_eq!(reg.find_handler("unknown"), None);
    }

    #[test]
    fn registry_list_methods() {
        let mut reg = RpcRegistry::new();
        let id = test_agent_id(1);
        reg.register_method(id, "a");
        reg.register_method(id, "b");
        let methods = reg.list_methods(&id);
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn registry_unregister_agent() {
        let mut reg = RpcRegistry::new();
        let id = test_agent_id(1);
        reg.register_method(id, "m1");
        reg.register_method(id, "m2");
        reg.unregister_agent(&id);
        assert_eq!(reg.find_handler("m1"), None);
        assert_eq!(reg.find_handler("m2"), None);
        assert!(reg.list_methods(&id).is_empty());
    }

    #[test]
    fn registry_all_methods() {
        let mut reg = RpcRegistry::new();
        reg.register_method(test_agent_id(1), "a");
        reg.register_method(test_agent_id(2), "b");
        assert_eq!(reg.all_methods().len(), 2);
    }

    // -- RpcRouter --

    #[tokio::test]
    async fn router_unknown_method() {
        let router = RpcRouter::new(RpcRegistry::new());
        let req = RpcRequest::new(test_agent_id(1), "nope", json!(null));
        let resp = router.call(req).await.unwrap();
        assert!(resp.result.is_err());
        assert_eq!(resp.result.unwrap_err().code, -1);
    }

    #[tokio::test]
    async fn router_call_with_response() {
        let mut reg = RpcRegistry::new();
        reg.register_method(test_agent_id(2), "echo");
        let router = std::sync::Arc::new(RpcRouter::new(reg));

        let req = RpcRequest::new(test_agent_id(1), "echo", json!("hello")).with_timeout(1000);
        let req_id = req.id;

        let router2 = router.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            router2.handle_response(RpcResponse::success(req_id, json!("hello back"), 10));
        });

        let resp = router.call(req).await.unwrap();
        assert!(resp.result.is_ok());
        assert_eq!(resp.result.unwrap(), json!("hello back"));
    }

    #[tokio::test]
    async fn router_timeout() {
        let mut reg = RpcRegistry::new();
        reg.register_method(test_agent_id(2), "slow");
        let router = RpcRouter::new(reg);

        let req = RpcRequest::new(test_agent_id(1), "slow", json!(null)).with_timeout(50);
        let resp = router.call(req).await.unwrap();
        assert!(resp.result.is_err());
        assert_eq!(resp.result.unwrap_err().code, -2);
    }

    #[tokio::test]
    async fn router_pending_count() {
        let mut reg = RpcRegistry::new();
        reg.register_method(test_agent_id(2), "m");
        let router = std::sync::Arc::new(RpcRouter::new(reg));
        assert_eq!(router.pending_count(), 0);

        let req = RpcRequest::new(test_agent_id(1), "m", json!(null)).with_timeout(500);
        let req_id = req.id;
        let router2 = router.clone();

        let handle = tokio::spawn(async move { router2.call(req).await });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(router.pending_count(), 1);

        router.handle_response(RpcResponse::success(req_id, json!(null), 0));
        handle.await.unwrap().unwrap();
        assert_eq!(router.pending_count(), 0);
    }

    // -- Stale socket cleanup --

    #[tokio::test]
    async fn cleanup_nonexistent_socket() {
        cleanup_stale_socket(Path::new("/tmp/daimon_test_nonexistent.sock")).await;
        // Should not panic.
    }

    #[tokio::test]
    async fn cleanup_stale_socket_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stale.sock");
        tokio::fs::write(&path, "").await.unwrap();
        assert!(path.exists());
        cleanup_stale_socket(&path).await;
        assert!(!path.exists());
    }

    // -- Constants --

    #[test]
    fn constants_valid() {
        const {
            assert!(MAX_MESSAGE_SIZE > 0);
            assert!(MAX_CONCURRENT_CONNECTIONS > 0);
            assert!(MAX_GLOBAL_FD_LIMIT > MAX_CONCURRENT_CONNECTIONS);
            assert!(ACK != NACK_QUEUE_FULL);
            assert!(ACK != NACK_INVALID);
        }
    }

    #[test]
    fn rpc_registry_serde_roundtrip() {
        let mut reg = RpcRegistry::new();
        let id = AgentId(uuid::Uuid::from_bytes([1; 16]));
        reg.register_method(id, "test.method");
        let json = serde_json::to_string(&reg).unwrap();
        let back: RpcRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.find_handler("test.method"), Some(id));
    }
}
