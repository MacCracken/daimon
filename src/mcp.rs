//! MCP tool dispatch — tool registry, manifest, external tool registration,
//! and tool call execution.
//!
//! Implements the Model Context Protocol (MCP) tool layer. Built-in tools
//! are registered at startup; external tools can be added dynamically via
//! [`McpToolRegistry::register_external`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Description of a single MCP tool (schema for discovery).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct McpToolDescription {
    /// Tool name (unique identifier).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// Complete tool manifest returned by the discovery endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct McpToolManifest {
    /// All available tools.
    pub tools: Vec<McpToolDescription>,
}

/// A request to call a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct McpToolCall {
    /// Tool name to invoke.
    pub name: String,
    /// Arguments to pass.
    pub arguments: serde_json::Value,
}

/// A content block in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct McpContentBlock {
    /// MIME type (e.g. "text/plain").
    pub content_type: String,
    /// Text content.
    pub text: String,
}

/// Result of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct McpToolResult {
    /// Content blocks.
    pub content: Vec<McpContentBlock>,
    /// Whether this result represents an error.
    pub is_error: bool,
}

impl McpToolResult {
    /// Create a success result with a single text block.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContentBlock {
                content_type: "text/plain".into(),
                text: text.into(),
            }],
            is_error: false,
        }
    }

    /// Create an error result.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![McpContentBlock {
                content_type: "text/plain".into(),
                text: message.into(),
            }],
            is_error: true,
        }
    }
}

/// An externally registered MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExternalMcpTool {
    /// Tool description.
    pub tool: McpToolDescription,
    /// Callback URL for execution.
    pub callback_url: String,
    /// Source identifier (who registered it).
    pub source: String,
}

/// Request to register an external tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RegisterMcpToolRequest {
    /// Tool name.
    pub name: String,
    /// Description.
    pub description: String,
    /// JSON Schema for input.
    pub input_schema: serde_json::Value,
    /// Callback URL for tool execution.
    pub callback_url: String,
    /// Optional source identifier.
    pub source: Option<String>,
}

// ---------------------------------------------------------------------------
// McpToolRegistry
// ---------------------------------------------------------------------------

/// Registry for built-in and external MCP tools.
pub struct McpToolRegistry {
    builtin: HashMap<String, McpToolDescription>,
    external: HashMap<String, ExternalMcpTool>,
}

impl McpToolRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            builtin: HashMap::new(),
            external: HashMap::new(),
        }
    }

    /// Register a built-in tool.
    pub fn register_builtin(&mut self, tool: McpToolDescription) {
        debug!(name = %tool.name, "registered built-in MCP tool");
        self.builtin.insert(tool.name.clone(), tool);
    }

    /// Register an external tool from a request.
    pub fn register_external(&mut self, req: RegisterMcpToolRequest) -> Result<()> {
        if req.name.is_empty() {
            return Err(DaimonError::InvalidParameter(
                "tool name cannot be empty".into(),
            ));
        }
        if req.callback_url.is_empty() {
            return Err(DaimonError::InvalidParameter(
                "callback URL cannot be empty".into(),
            ));
        }

        let tool = McpToolDescription {
            name: req.name.clone(),
            description: req.description,
            input_schema: req.input_schema,
        };

        let external = ExternalMcpTool {
            tool,
            callback_url: req.callback_url,
            source: req.source.unwrap_or_else(|| "unknown".into()),
        };

        info!(name = %req.name, "registered external MCP tool");
        self.external.insert(req.name, external);
        Ok(())
    }

    /// Deregister a tool by name (external only).
    pub fn deregister(&mut self, name: &str) -> Result<()> {
        if self.external.remove(name).is_none() {
            return Err(DaimonError::InvalidParameter(format!(
                "external tool not found: {name}"
            )));
        }
        info!(name = %name, "deregistered external MCP tool");
        Ok(())
    }

    /// Build the complete tool manifest (built-in + external).
    #[must_use]
    pub fn manifest(&self) -> McpToolManifest {
        let mut tools: Vec<McpToolDescription> = self.builtin.values().cloned().collect();
        tools.extend(self.external.values().map(|e| e.tool.clone()));
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        McpToolManifest { tools }
    }

    /// Look up a tool by name (built-in first, then external).
    #[must_use]
    pub fn find_tool(&self, name: &str) -> Option<&McpToolDescription> {
        self.builtin
            .get(name)
            .or_else(|| self.external.get(name).map(|e| &e.tool))
    }

    /// Get callback URL for an external tool.
    #[must_use]
    pub fn external_callback(&self, name: &str) -> Option<&str> {
        self.external.get(name).map(|e| e.callback_url.as_str())
    }

    /// Number of registered tools.
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.builtin.len() + self.external.len()
    }
}

impl Default for McpToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_tool(name: &str) -> McpToolDescription {
        McpToolDescription {
            name: name.into(),
            description: format!("Test tool: {name}"),
            input_schema: json!({"type": "object"}),
        }
    }

    fn test_register_req(name: &str) -> RegisterMcpToolRequest {
        RegisterMcpToolRequest {
            name: name.into(),
            description: format!("External: {name}"),
            input_schema: json!({"type": "object"}),
            callback_url: "http://localhost:9000/callback".into(),
            source: Some("test".into()),
        }
    }

    #[test]
    fn register_builtin() {
        let mut reg = McpToolRegistry::new();
        reg.register_builtin(test_tool("scan"));
        assert_eq!(reg.tool_count(), 1);
        assert!(reg.find_tool("scan").is_some());
    }

    #[test]
    fn register_external() {
        let mut reg = McpToolRegistry::new();
        reg.register_external(test_register_req("custom")).unwrap();
        assert_eq!(reg.tool_count(), 1);
        assert!(reg.find_tool("custom").is_some());
        assert!(reg.external_callback("custom").is_some());
    }

    #[test]
    fn register_external_empty_name_rejected() {
        let mut reg = McpToolRegistry::new();
        let mut req = test_register_req("x");
        req.name = String::new();
        assert!(reg.register_external(req).is_err());
    }

    #[test]
    fn register_external_empty_url_rejected() {
        let mut reg = McpToolRegistry::new();
        let mut req = test_register_req("x");
        req.callback_url = String::new();
        assert!(reg.register_external(req).is_err());
    }

    #[test]
    fn deregister_external() {
        let mut reg = McpToolRegistry::new();
        reg.register_external(test_register_req("temp")).unwrap();
        assert!(reg.deregister("temp").is_ok());
        assert_eq!(reg.tool_count(), 0);
    }

    #[test]
    fn deregister_nonexistent() {
        let mut reg = McpToolRegistry::new();
        assert!(reg.deregister("nope").is_err());
    }

    #[test]
    fn manifest_sorted() {
        let mut reg = McpToolRegistry::new();
        reg.register_builtin(test_tool("zebra"));
        reg.register_builtin(test_tool("alpha"));
        reg.register_external(test_register_req("middle")).unwrap();

        let manifest = reg.manifest();
        let names: Vec<&str> = manifest.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn find_prefers_builtin() {
        let mut reg = McpToolRegistry::new();
        reg.register_builtin(test_tool("overlap"));
        reg.register_external(test_register_req("overlap")).unwrap();
        // find_tool returns builtin first.
        let tool = reg.find_tool("overlap").unwrap();
        assert!(tool.description.starts_with("Test tool"));
    }

    #[test]
    fn tool_result_text() {
        let r = McpToolResult::text("hello");
        assert!(!r.is_error);
        assert_eq!(r.content[0].text, "hello");
    }

    #[test]
    fn tool_result_error() {
        let r = McpToolResult::error("boom");
        assert!(r.is_error);
        assert_eq!(r.content[0].text, "boom");
    }

    // -- serde roundtrips --

    #[test]
    fn tool_call_serde_roundtrip() {
        let call = McpToolCall {
            name: "scan".into(),
            arguments: json!({"target": "localhost"}),
        };
        let json = serde_json::to_string(&call).unwrap();
        let back: McpToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "scan");
    }

    #[test]
    fn tool_result_serde_roundtrip() {
        let result = McpToolResult::text("ok");
        let json = serde_json::to_string(&result).unwrap();
        let back: McpToolResult = serde_json::from_str(&json).unwrap();
        assert!(!back.is_error);
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let mut reg = McpToolRegistry::new();
        reg.register_builtin(test_tool("t1"));
        let manifest = reg.manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let back: McpToolManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tools.len(), 1);
    }

    #[test]
    fn external_tool_serde_roundtrip() {
        let ext = ExternalMcpTool {
            tool: test_tool("ext"),
            callback_url: "http://example.com".into(),
            source: "test".into(),
        };
        let json = serde_json::to_string(&ext).unwrap();
        let back: ExternalMcpTool = serde_json::from_str(&json).unwrap();
        assert_eq!(back.callback_url, "http://example.com");
    }
}
