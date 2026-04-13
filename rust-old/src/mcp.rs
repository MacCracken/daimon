//! MCP tool dispatch — re-exports bote's hosting types and adds
//! daimon-specific tool registration.
//!
//! All MCP hosting types live in [`bote::host`]. Daimon registers its
//! built-in tools into the [`McpHostRegistry`] at startup.

#[cfg(feature = "mcp")]
pub use bote::host::{
    ExternalMcpTool, McpContentBlock, McpHostRegistry, McpToolCall, McpToolDescription,
    McpToolManifest, McpToolResult, RegisterMcpToolRequest, validate_callback_url,
};

/// Daimon-local re-exports for when the `mcp` feature is disabled.
/// Provides the same types as stubs so the rest of the crate can compile.
#[cfg(not(feature = "mcp"))]
#[allow(missing_docs)]
mod fallback {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct McpToolDescription {
        pub name: String,
        pub description: String,
        pub input_schema: serde_json::Value,
    }

    impl McpToolDescription {
        #[must_use]
        pub fn new(
            name: impl Into<String>,
            description: impl Into<String>,
            input_schema: serde_json::Value,
        ) -> Self {
            Self {
                name: name.into(),
                description: description.into(),
                input_schema,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct McpToolManifest {
        pub tools: Vec<McpToolDescription>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct McpToolCall {
        pub name: String,
        #[serde(default)]
        pub arguments: serde_json::Value,
    }

    impl McpToolCall {
        #[must_use]
        pub fn new(name: impl Into<String>, arguments: serde_json::Value) -> Self {
            Self {
                name: name.into(),
                arguments,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct McpContentBlock {
        #[serde(rename = "type")]
        pub content_type: String,
        pub text: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct McpToolResult {
        pub content: Vec<McpContentBlock>,
        #[serde(rename = "isError")]
        pub is_error: bool,
    }

    impl McpToolResult {
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

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct ExternalMcpTool {
        pub tool: McpToolDescription,
        pub callback_url: String,
        pub source: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct RegisterMcpToolRequest {
        pub name: String,
        pub description: String,
        pub input_schema: serde_json::Value,
        pub callback_url: String,
        pub source: Option<String>,
    }

    impl RegisterMcpToolRequest {
        #[must_use]
        pub fn new(
            name: impl Into<String>,
            description: impl Into<String>,
            input_schema: serde_json::Value,
            callback_url: impl Into<String>,
        ) -> Self {
            Self {
                name: name.into(),
                description: description.into(),
                input_schema,
                callback_url: callback_url.into(),
                source: None,
            }
        }

        #[must_use]
        pub fn with_source(mut self, source: impl Into<String>) -> Self {
            self.source = Some(source.into());
            self
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct McpHostRegistry {
        builtin: HashMap<String, McpToolDescription>,
        external: HashMap<String, ExternalMcpTool>,
    }

    impl McpHostRegistry {
        #[must_use]
        pub fn new() -> Self {
            Self {
                builtin: HashMap::new(),
                external: HashMap::new(),
            }
        }

        pub fn register_builtin(&mut self, tool: McpToolDescription) {
            self.builtin.insert(tool.name.clone(), tool);
        }

        pub fn register_external(
            &mut self,
            req: RegisterMcpToolRequest,
            _validate_ssrf: bool,
        ) -> Result<(), String> {
            if req.name.is_empty() {
                return Err("tool name cannot be empty".into());
            }
            let tool = McpToolDescription::new(req.name.clone(), req.description, req.input_schema);
            let external = ExternalMcpTool {
                tool,
                callback_url: req.callback_url,
                source: req.source.unwrap_or_else(|| "unknown".into()),
            };
            self.external.insert(req.name, external);
            Ok(())
        }

        pub fn deregister(&mut self, name: &str) -> Result<(), String> {
            if self.external.remove(name).is_none() {
                return Err(format!("external tool not found: {name}"));
            }
            Ok(())
        }

        #[must_use]
        pub fn manifest(&self) -> McpToolManifest {
            let mut tools: Vec<McpToolDescription> = self.builtin.values().cloned().collect();
            tools.extend(self.external.values().map(|e| e.tool.clone()));
            tools.sort_by(|a, b| a.name.cmp(&b.name));
            McpToolManifest { tools }
        }

        #[must_use]
        pub fn find_tool(&self, name: &str) -> Option<&McpToolDescription> {
            self.builtin
                .get(name)
                .or_else(|| self.external.get(name).map(|e| &e.tool))
        }

        #[must_use]
        pub fn external_callback(&self, name: &str) -> Option<&str> {
            self.external.get(name).map(|e| e.callback_url.as_str())
        }

        #[must_use]
        pub fn tool_count(&self) -> usize {
            self.builtin.len() + self.external.len()
        }
    }

    impl Default for McpHostRegistry {
        fn default() -> Self {
            Self::new()
        }
    }

    pub fn validate_callback_url(_url: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(feature = "mcp"))]
pub use fallback::*;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_tool(name: &str) -> McpToolDescription {
        McpToolDescription::new(
            name,
            format!("Test tool: {name}"),
            json!({"type": "object"}),
        )
    }

    fn test_register_req(name: &str) -> RegisterMcpToolRequest {
        RegisterMcpToolRequest::new(
            name,
            format!("External: {name}"),
            json!({"type": "object"}),
            "http://localhost:9000/callback",
        )
        .with_source("test")
    }

    #[test]
    fn register_builtin() {
        let mut reg = McpHostRegistry::new();
        reg.register_builtin(test_tool("scan"));
        assert_eq!(reg.tool_count(), 1);
        assert!(reg.find_tool("scan").is_some());
    }

    #[test]
    fn register_external() {
        let mut reg = McpHostRegistry::new();
        reg.register_external(test_register_req("custom"), false)
            .unwrap();
        assert_eq!(reg.tool_count(), 1);
        assert!(reg.find_tool("custom").is_some());
        assert!(reg.external_callback("custom").is_some());
    }

    #[test]
    fn deregister_external() {
        let mut reg = McpHostRegistry::new();
        reg.register_external(test_register_req("temp"), false)
            .unwrap();
        assert!(reg.deregister("temp").is_ok());
        assert_eq!(reg.tool_count(), 0);
    }

    #[test]
    fn manifest_sorted() {
        let mut reg = McpHostRegistry::new();
        reg.register_builtin(test_tool("zebra"));
        reg.register_builtin(test_tool("alpha"));
        reg.register_external(test_register_req("middle"), false)
            .unwrap();

        let manifest = reg.manifest();
        let names: Vec<&str> = manifest.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn tool_result_text() {
        let r = McpToolResult::text("hello");
        assert!(!r.is_error);
        let text = &r.content[0].text;
        assert!(format!("{text:?}").contains("hello"));
    }

    #[test]
    fn tool_result_error() {
        let r = McpToolResult::error("boom");
        assert!(r.is_error);
        let text = &r.content[0].text;
        assert!(format!("{text:?}").contains("boom"));
    }

    #[test]
    fn tool_call_serde_roundtrip() {
        let call = McpToolCall::new("scan", json!({"target": "localhost"}));
        let json_str = serde_json::to_string(&call).unwrap();
        let back: McpToolCall = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.name, "scan");
    }

    #[test]
    fn tool_result_serde_roundtrip() {
        let result = McpToolResult::text("ok");
        let json_str = serde_json::to_string(&result).unwrap();
        let back: McpToolResult = serde_json::from_str(&json_str).unwrap();
        assert!(!back.is_error);
    }

    #[cfg(not(feature = "mcp"))]
    #[test]
    fn mcp_host_registry_serde_roundtrip() {
        let mut reg = McpHostRegistry::new();
        reg.register_builtin(test_tool("scan"));
        reg.register_external(test_register_req("ext"), false)
            .unwrap();
        let json = serde_json::to_string(&reg).unwrap();
        let back: McpHostRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tool_count(), 2);
    }
}
