//! # Daimon
//!
//! **Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator.
//!
//! Provides the core agent runtime: HTTP API (port 8090), agent lifecycle management,
//! process supervision, IPC, task scheduling, federation, edge fleet management,
//! memory/vector/RAG stores, MCP tool dispatch, and screen capture.

#![warn(missing_docs)]

pub mod agent;
pub mod api;
pub mod config;
pub mod edge;
pub mod error;
pub mod federation;
pub mod ipc;
pub mod mcp;
pub mod memory;
pub mod rag;
pub mod scheduler;
pub mod screen;
pub mod supervisor;
pub mod vector_store;

#[cfg(feature = "logging")]
pub mod logging;

pub use config::Config;
pub use error::{DaimonError, Result};
