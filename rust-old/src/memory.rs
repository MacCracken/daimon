//! Agent persistent memory store — per-agent key-value storage.
//!
//! Each agent gets an isolated namespace under `{data_dir}/agent-memory/`.
//! Values are JSON-serializable and stored as individual files for simplicity
//! and crash-safety (atomic write via rename).

use std::path::PathBuf;

use agnostik::AgentId;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::{DaimonError, Result};

/// Maximum key length (bytes).
const MAX_KEY_LENGTH: usize = 256;

/// Maximum value size (bytes) — 1 MiB.
const MAX_VALUE_SIZE: usize = 1_048_576;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single memory entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MemoryEntry {
    /// The key under which this entry is stored.
    pub key: String,
    /// The stored value (arbitrary JSON).
    pub value: serde_json::Value,
    /// RFC 3339 timestamp of initial creation.
    pub created_at: String,
    /// RFC 3339 timestamp of most recent update.
    pub updated_at: String,
    /// Optional tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// AgentMemoryStore
// ---------------------------------------------------------------------------

/// Per-agent key-value store backed by the filesystem.
///
/// Each agent's entries are isolated in `{base_dir}/{agent_id}/`.
/// Writes are atomic (write-to-tmp then rename).
pub struct AgentMemoryStore {
    base_dir: PathBuf,
}

impl AgentMemoryStore {
    /// Create a store rooted at the given directory.
    #[must_use]
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Directory for a specific agent's entries.
    fn agent_dir(&self, agent_id: AgentId) -> PathBuf {
        self.base_dir.join(agent_id.to_string())
    }

    /// File path for a specific key (sanitized).
    fn key_path(&self, agent_id: AgentId, key: &str) -> PathBuf {
        let safe_key = sanitize_key(key);
        self.agent_dir(agent_id).join(format!("{safe_key}.json"))
    }

    // ------------------------------------------------------------------
    // CRUD
    // ------------------------------------------------------------------

    /// Store a value for an agent.
    pub async fn set(
        &self,
        agent_id: AgentId,
        key: &str,
        value: serde_json::Value,
        tags: Vec<String>,
    ) -> Result<()> {
        validate_key(key)?;

        let serialized = serde_json::to_vec_pretty(&value)
            .map_err(|e| DaimonError::StorageError(format!("serialize: {e}")))?;
        if serialized.len() > MAX_VALUE_SIZE {
            return Err(DaimonError::InvalidParameter(format!(
                "value exceeds maximum size of {MAX_VALUE_SIZE} bytes"
            )));
        }

        let dir = self.agent_dir(agent_id);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| DaimonError::StorageError(format!("create dir: {e}")))?;

        let now = chrono::Utc::now().to_rfc3339();
        let path = self.key_path(agent_id, key);

        // Preserve created_at from existing entry if present.
        let created_at = match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str::<MemoryEntry>(&content)
                .map(|e| e.created_at)
                .unwrap_or_else(|_| now.clone()),
            Err(_) => now.clone(),
        };

        let entry = MemoryEntry {
            key: key.to_string(),
            value,
            created_at,
            updated_at: now,
            tags,
        };

        let content = serde_json::to_string_pretty(&entry)
            .map_err(|e| DaimonError::StorageError(format!("serialize entry: {e}")))?;

        // Atomic write: tmp → rename.
        let tmp_path = path.with_extension("tmp");
        tokio::fs::write(&tmp_path, &content)
            .await
            .map_err(|e| DaimonError::StorageError(format!("write tmp: {e}")))?;
        tokio::fs::rename(&tmp_path, &path)
            .await
            .map_err(|e| DaimonError::StorageError(format!("rename: {e}")))?;

        debug!("agent {} stored key '{}'", agent_id, key);
        Ok(())
    }

    /// Retrieve an entry for an agent. Returns `None` if the key does not exist.
    pub async fn get(&self, agent_id: AgentId, key: &str) -> Result<Option<MemoryEntry>> {
        let path = self.key_path(agent_id, key);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let entry: MemoryEntry = serde_json::from_str(&content)
                    .map_err(|e| DaimonError::StorageError(format!("parse entry: {e}")))?;
                Ok(Some(entry))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(DaimonError::StorageError(format!("read entry: {e}"))),
        }
    }

    /// Delete a key for an agent. Returns `true` if the key existed.
    pub async fn delete(&self, agent_id: AgentId, key: &str) -> Result<bool> {
        let path = self.key_path(agent_id, key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                debug!("agent {} deleted key '{}'", agent_id, key);
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// List all keys for an agent (sorted).
    pub async fn list_keys(&self, agent_id: AgentId) -> Result<Vec<String>> {
        let dir = self.agent_dir(agent_id);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut keys = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                keys.push(stem.to_string());
            }
        }
        keys.sort();
        Ok(keys)
    }

    /// List keys that have a specific tag (sorted).
    pub async fn list_by_tag(&self, agent_id: AgentId, tag: &str) -> Result<Vec<String>> {
        let dir = self.agent_dir(agent_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut matching = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && let Ok(content) = tokio::fs::read_to_string(&path).await
                && let Ok(mem_entry) = serde_json::from_str::<MemoryEntry>(&content)
                && mem_entry.tags.iter().any(|t| t == tag)
            {
                matching.push(mem_entry.key);
            }
        }
        matching.sort();
        Ok(matching)
    }

    /// Remove all entries for an agent. Returns the number deleted.
    pub async fn clear(&self, agent_id: AgentId) -> Result<u64> {
        let dir = self.agent_dir(agent_id);
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0u64;
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                tokio::fs::remove_file(entry.path()).await?;
                count += 1;
            }
        }
        debug!("agent {} cleared {} memory entries", agent_id, count);
        Ok(count)
    }

    /// Total disk usage in bytes for an agent's memory.
    pub async fn usage_bytes(&self, agent_id: AgentId) -> Result<u64> {
        let dir = self.agent_dir(agent_id);
        if !dir.exists() {
            return Ok(0);
        }

        let mut total = 0u64;
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(meta) = entry.metadata().await {
                total += meta.len();
            }
        }
        Ok(total)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Sanitize a key for use as a filename.
#[must_use]
fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Validate a key before use.
fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(DaimonError::InvalidParameter("key cannot be empty".into()));
    }
    if key.len() > MAX_KEY_LENGTH {
        return Err(DaimonError::InvalidParameter(format!(
            "key exceeds maximum length of {MAX_KEY_LENGTH} bytes"
        )));
    }
    if key.contains("..") || key.contains('/') || key.contains('\\') {
        return Err(DaimonError::InvalidParameter(
            "key contains invalid characters".into(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn agent(n: u8) -> AgentId {
        AgentId(Uuid::from_bytes([n; 16]))
    }

    #[tokio::test]
    async fn set_get_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        store
            .set(id, "greeting", serde_json::json!("hello"), vec![])
            .await
            .unwrap();

        let entry = store.get(id, "greeting").await.unwrap().unwrap();
        assert_eq!(entry.key, "greeting");
        assert_eq!(entry.value, serde_json::json!("hello"));
    }

    #[tokio::test]
    async fn get_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let result = store.get(agent(1), "nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_existing() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        store
            .set(id, "tmp", serde_json::json!(42), vec![])
            .await
            .unwrap();
        assert!(store.delete(id, "tmp").await.unwrap());
        assert!(store.get(id, "tmp").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        assert!(!store.delete(agent(1), "ghost").await.unwrap());
    }

    #[tokio::test]
    async fn list_keys_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        assert!(store.list_keys(agent(1)).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_keys_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        for k in ["gamma", "alpha", "beta"] {
            store
                .set(id, k, serde_json::json!(1), vec![])
                .await
                .unwrap();
        }

        assert_eq!(
            store.list_keys(id).await.unwrap(),
            vec!["alpha", "beta", "gamma"]
        );
    }

    #[tokio::test]
    async fn clear_entries() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        store
            .set(id, "a", serde_json::json!(1), vec![])
            .await
            .unwrap();
        store
            .set(id, "b", serde_json::json!(2), vec![])
            .await
            .unwrap();

        assert_eq!(store.clear(id).await.unwrap(), 2);
        assert!(store.list_keys(id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn clear_empty_agent() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        assert_eq!(store.clear(agent(99)).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn usage_bytes_tracking() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        assert_eq!(store.usage_bytes(id).await.unwrap(), 0);

        store
            .set(id, "data", serde_json::json!("some content"), vec![])
            .await
            .unwrap();

        assert!(store.usage_bytes(id).await.unwrap() > 0);
    }

    #[tokio::test]
    async fn tags_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        store
            .set(
                id,
                "config",
                serde_json::json!({"level": "debug"}),
                vec!["settings".into(), "debug".into()],
            )
            .await
            .unwrap();

        let entry = store.get(id, "config").await.unwrap().unwrap();
        assert_eq!(entry.tags, vec!["settings", "debug"]);
    }

    #[tokio::test]
    async fn list_by_tag_filtering() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        store
            .set(id, "a", serde_json::json!(1), vec!["x".into()])
            .await
            .unwrap();
        store
            .set(id, "b", serde_json::json!(2), vec!["y".into()])
            .await
            .unwrap();
        store
            .set(id, "c", serde_json::json!(3), vec!["x".into(), "y".into()])
            .await
            .unwrap();

        assert_eq!(store.list_by_tag(id, "x").await.unwrap(), vec!["a", "c"]);
        assert_eq!(store.list_by_tag(id, "y").await.unwrap(), vec!["b", "c"]);
        assert!(store.list_by_tag(id, "z").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_by_tag_no_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        assert!(store.list_by_tag(agent(1), "any").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn value_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());

        let big = "x".repeat(MAX_VALUE_SIZE + 100);
        let err = store
            .set(agent(1), "big", serde_json::json!(big), vec![])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("maximum size"));
    }

    #[tokio::test]
    async fn agent_isolation() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id1 = agent(1);
        let id2 = agent(2);

        store
            .set(id1, "shared", serde_json::json!("val1"), vec![])
            .await
            .unwrap();
        store
            .set(id2, "shared", serde_json::json!("val2"), vec![])
            .await
            .unwrap();

        assert_eq!(
            store.get(id1, "shared").await.unwrap().unwrap().value,
            serde_json::json!("val1")
        );
        assert_eq!(
            store.get(id2, "shared").await.unwrap().unwrap().value,
            serde_json::json!("val2")
        );
    }

    #[tokio::test]
    async fn overwrite_preserves_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentMemoryStore::new(dir.path().to_path_buf());
        let id = agent(1);

        store
            .set(id, "evolving", serde_json::json!("v1"), vec![])
            .await
            .unwrap();
        let first = store.get(id, "evolving").await.unwrap().unwrap();
        let original_created = first.created_at.clone();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        store
            .set(id, "evolving", serde_json::json!("v2"), vec![])
            .await
            .unwrap();
        let second = store.get(id, "evolving").await.unwrap().unwrap();

        assert_eq!(second.created_at, original_created);
        assert_eq!(second.value, serde_json::json!("v2"));
    }

    #[test]
    fn validate_key_valid() {
        assert!(validate_key("my-key").is_ok());
        assert!(validate_key("key_123").is_ok());
        assert!(validate_key("a").is_ok());
    }

    #[test]
    fn validate_key_empty() {
        assert!(validate_key("").unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn validate_key_too_long() {
        let long = "x".repeat(MAX_KEY_LENGTH + 1);
        assert!(
            validate_key(&long)
                .unwrap_err()
                .to_string()
                .contains("maximum length")
        );
    }

    #[test]
    fn validate_key_path_traversal() {
        assert!(validate_key("../etc/passwd").is_err());
        assert!(validate_key("foo/bar").is_err());
        assert!(validate_key("foo\\bar").is_err());
        assert!(validate_key("..").is_err());
    }

    #[test]
    fn sanitize_key_special_chars() {
        assert_eq!(sanitize_key("hello-world"), "hello-world");
        assert_eq!(sanitize_key("key_123"), "key_123");
        assert_eq!(sanitize_key("has spaces"), "has_spaces");
        assert_eq!(sanitize_key("path/traversal"), "path_traversal");
        assert_eq!(sanitize_key("special!@#chars"), "special___chars");
    }

    #[test]
    fn memory_entry_serde_roundtrip() {
        let entry = MemoryEntry {
            key: "test".into(),
            value: serde_json::json!({"nested": true}),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
            tags: vec!["tag1".into()],
        };

        let json = serde_json::to_string(&entry).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key, "test");
        assert_eq!(back.tags, vec!["tag1"]);
    }

    #[test]
    fn memory_entry_missing_tags_defaults() {
        let json = r#"{"key":"k","value":1,"created_at":"t","updated_at":"t"}"#;
        let entry: MemoryEntry = serde_json::from_str(json).unwrap();
        assert!(entry.tags.is_empty());
    }
}
