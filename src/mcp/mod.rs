#![allow(unused)]
//! Phase 11: Model Context Protocol (network required) — Low Priority
//!
//! MCP allows Claude Code to communicate with external tool servers.

pub mod client;
pub mod discovery;
pub mod tools;

/// MCP server connection state
#[derive(Debug, Clone)]
pub enum McpConnectionState {
    Pending,
    Connected,
    Disconnected,
    Error(String),
}

/// MCP server configuration (from settings)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    /// "stdio" or "sse"
    pub transport: String,
    /// Command to launch (for stdio transport)
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    /// URL (for SSE transport)
    pub url: Option<String>,
    /// Environment variables
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// MCP tool definition (received from server)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub server_name: String,
}

/// MCP server resource
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}
