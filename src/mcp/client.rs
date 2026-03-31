#![allow(unused)]
//! MCP client — communicates with MCP servers over stdio or SSE
use anyhow::Result;
use super::{McpConnectionState, McpServerConfig, McpToolDef, McpResource};

/// MCP client for a single server connection
pub struct McpClient {
    pub config: McpServerConfig,
    pub state: McpConnectionState,
    pub tools: Vec<McpToolDef>,
    pub resources: Vec<McpResource>,
}

impl McpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            state: McpConnectionState::Pending,
            tools: Vec::new(),
            resources: Vec::new(),
        }
    }

    /// Connect to the MCP server (stub — requires network feature)
    pub async fn connect(&mut self) -> Result<()> {
        // In a full implementation:
        // - stdio: spawn the command, communicate via stdin/stdout JSON-RPC
        // - sse: connect to the SSE endpoint
        self.state = McpConnectionState::Disconnected;
        anyhow::bail!("MCP client connect not yet implemented")
    }

    /// Initialize the connection (exchange capabilities)
    pub async fn initialize(&mut self) -> Result<()> {
        anyhow::bail!("MCP client initialize not yet implemented")
    }

    /// List available tools from the server
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolDef>> {
        anyhow::bail!("MCP client list_tools not yet implemented")
    }

    /// Call a tool on the MCP server
    pub async fn call_tool(&self, tool_name: &str, input: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("MCP client call_tool not yet implemented")
    }

    /// List resources from the server
    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        anyhow::bail!("MCP client list_resources not yet implemented")
    }

    /// Disconnect from the server
    pub async fn disconnect(&mut self) {
        self.state = McpConnectionState::Disconnected;
    }
}
