//! Firewall MCP tool handlers — wires nein tools into daimon's MCP dispatch.
//!
//! Registers `nein_status`, `nein_allow`, `nein_deny`, `nein_list` as built-in
//! MCP tools that agents can invoke to manage host firewall rules.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::api::McpToolHandler;
use crate::mcp::{McpHostRegistry, McpToolDescription, McpToolResult};

/// Register nein firewall tools into the MCP registry and return handlers.
///
/// Call this during `AppState` construction to wire up the firewall tools.
pub fn register(registry: &mut McpHostRegistry) -> HashMap<String, McpToolHandler> {
    let mut handlers: HashMap<String, McpToolHandler> = HashMap::new();

    // Register tool descriptors from nein
    for tool_def in nein::mcp::tool_descriptors() {
        let desc = McpToolDescription::new(
            &tool_def.name,
            &tool_def.description,
            serde_json::to_value(&tool_def.input_schema).unwrap_or_default(),
        );
        registry.register_builtin(desc);
    }

    // nein_status handler
    handlers.insert(
        "nein_status".into(),
        Arc::new(
            |_args: serde_json::Value| -> Pin<Box<dyn Future<Output = McpToolResult> + Send>> {
                Box::pin(async move {
                    match nein::inspect::status().await {
                        Ok(status) => {
                            let resp = nein::mcp::StatusResponse {
                                tables: status.tables,
                                total_rules: status.total_rules,
                                raw_ruleset: status.raw_ruleset,
                            };
                            match serde_json::to_string_pretty(&resp) {
                                Ok(json) => McpToolResult::text(json),
                                Err(e) => McpToolResult::error(format!("serialization error: {e}")),
                            }
                        }
                        Err(e) => McpToolResult::error(format!("nein_status failed: {e}")),
                    }
                })
            },
        ),
    );

    // nein_allow handler
    handlers.insert(
        "nein_allow".into(),
        Arc::new(
            |args: serde_json::Value| -> Pin<Box<dyn Future<Output = McpToolResult> + Send>> {
                Box::pin(async move {
                    let req: nein::mcp::AllowRequest = match serde_json::from_value(args) {
                        Ok(r) => r,
                        Err(e) => return McpToolResult::error(format!("invalid request: {e}")),
                    };
                    let table = req.table.clone();
                    let chain = req.chain.clone();
                    match nein::mcp::build_allow_rule(&req) {
                        Ok(rule) => {
                            match nein::apply::add_rule("inet", &table, &chain, &rule).await {
                                Ok(()) => McpToolResult::text(format!("rule added: {rule}")),
                                Err(e) => McpToolResult::error(format!("apply failed: {e}")),
                            }
                        }
                        Err(e) => McpToolResult::error(e),
                    }
                })
            },
        ),
    );

    // nein_deny handler
    handlers.insert(
        "nein_deny".into(),
        Arc::new(
            |args: serde_json::Value| -> Pin<Box<dyn Future<Output = McpToolResult> + Send>> {
                Box::pin(async move {
                    let req: nein::mcp::DenyRequest = match serde_json::from_value(args) {
                        Ok(r) => r,
                        Err(e) => return McpToolResult::error(format!("invalid request: {e}")),
                    };
                    let table = req.table.clone();
                    let chain = req.chain.clone();
                    match nein::mcp::build_deny_rule(&req) {
                        Ok(rule) => {
                            match nein::apply::add_rule("inet", &table, &chain, &rule).await {
                                Ok(()) => McpToolResult::text(format!("rule added: {rule}")),
                                Err(e) => McpToolResult::error(format!("apply failed: {e}")),
                            }
                        }
                        Err(e) => McpToolResult::error(e),
                    }
                })
            },
        ),
    );

    // nein_list handler
    handlers.insert(
        "nein_list".into(),
        Arc::new(
            |args: serde_json::Value| -> Pin<Box<dyn Future<Output = McpToolResult> + Send>> {
                Box::pin(async move {
                    let req: nein::mcp::ListRequest = match serde_json::from_value(args) {
                        Ok(r) => r,
                        Err(e) => return McpToolResult::error(format!("invalid request: {e}")),
                    };
                    match nein::apply::list_ruleset().await {
                        Ok(raw) => {
                            let lines: Vec<&str> = raw.lines().collect();
                            let mut rules = vec![];
                            let mut current_table = String::new();
                            let mut current_chain = String::new();

                            for line in &lines {
                                let trimmed = line.trim();
                                if let Some(rest) = trimmed.strip_prefix("table ") {
                                    current_table = rest.trim_end_matches(" {").to_string();
                                } else if let Some(rest) = trimmed.strip_prefix("chain ") {
                                    current_chain = rest.trim_end_matches(" {").to_string();
                                } else if !trimmed.is_empty()
                                    && !trimmed.starts_with("type ")
                                    && trimmed != "}"
                                {
                                    // Apply optional table/chain filters
                                    if let Some(ref tf) = req.table
                                        && current_table != *tf
                                    {
                                        continue;
                                    }
                                    if let Some(ref cf) = req.chain
                                        && current_chain != *cf
                                    {
                                        continue;
                                    }
                                    rules.push(nein::mcp::ListEntry {
                                        table: current_table.clone(),
                                        chain: current_chain.clone(),
                                        rule: trimmed.to_string(),
                                        handle: None,
                                    });
                                }
                            }

                            let resp = nein::mcp::ListResponse {
                                count: rules.len(),
                                rules,
                            };
                            match serde_json::to_string_pretty(&resp) {
                                Ok(json) => McpToolResult::text(json),
                                Err(e) => McpToolResult::error(format!("serialization error: {e}")),
                            }
                        }
                        Err(e) => McpToolResult::error(format!("nein_list failed: {e}")),
                    }
                })
            },
        ),
    );

    tracing::info!(tools = 4, "registered nein firewall MCP tools");
    handlers
}
