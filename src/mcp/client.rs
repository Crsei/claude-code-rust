//! MCP client -- communicates with MCP servers over stdio (subprocess) or SSE.
//!
//! The stdio transport spawns a subprocess and exchanges line-delimited
//! JSON-RPC 2.0 messages over stdin/stdout. A background reader task
//! dispatches incoming responses to waiting request futures.
//!
//! Lifecycle:
//!   1. `McpClient::connect()` -- spawn process, start reader task
//!   2. `McpClient::initialize()` -- JSON-RPC `initialize` + `notifications/initialized`
//!   3. `McpClient::list_tools()` / `call_tool()` / `list_resources()` / `read_resource()`
//!   4. `McpClient::disconnect()` -- graceful shutdown

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, info, warn};

use super::{
    CallToolResult, InitializeResult, JsonRpcNotification, JsonRpcRequest,
    ListResourcesResult, ListToolsResult, McpConnectionState, McpResource,
    McpServerConfig, McpToolDef, ReadResourceResult, ServerCapabilities, ServerInfo,
    ToolCallContent, CLIENT_NAME, CLIENT_VERSION, CONNECT_TIMEOUT_SECS, PROTOCOL_VERSION,
    TOOL_CALL_TIMEOUT_SECS,
};

use super::transport::reader_loop;

// ---------------------------------------------------------------------------
// McpClient
// ---------------------------------------------------------------------------

/// MCP client for a single server connection.
///
/// Manages the subprocess lifecycle and JSON-RPC communication.
pub struct McpClient {
    /// Server configuration.
    pub config: McpServerConfig,
    /// Current connection state.
    pub state: McpConnectionState,
    /// Tools discovered from this server.
    pub tools: Vec<McpToolDef>,
    /// Resources discovered from this server.
    pub resources: Vec<McpResource>,
    /// Server capabilities (set after initialize).
    pub server_capabilities: ServerCapabilities,
    /// Server info (set after initialize).
    pub server_info: ServerInfo,
    /// Server instructions (set after initialize).
    pub instructions: Option<String>,

    // -- Internal state (stdio transport) ------------------------------------
    /// Stdin writer for the subprocess.
    stdin_writer: Option<Arc<Mutex<tokio::process::ChildStdin>>>,
    /// Handle to the background reader task.
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    /// Handle to the child process.
    child: Option<tokio::process::Child>,
    /// Monotonically increasing request ID counter.
    pub(crate) next_id: Arc<AtomicU64>,
    /// Pending requests: id -> oneshot sender for the response.
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
}

impl McpClient {
    /// Create a new MCP client for the given server configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            state: McpConnectionState::Pending,
            tools: Vec::new(),
            resources: Vec::new(),
            server_capabilities: ServerCapabilities::default(),
            server_info: ServerInfo::default(),
            instructions: None,
            stdin_writer: None,
            reader_handle: None,
            child: None,
            next_id: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // -----------------------------------------------------------------------
    // Connection lifecycle
    // -----------------------------------------------------------------------

    /// Connect to the MCP server.
    pub async fn connect(&mut self) -> Result<()> {
        match self.config.transport.as_str() {
            "stdio" => self.connect_stdio().await,
            "sse" => {
                bail!(
                    "SSE transport is not yet implemented. \
                     Use stdio transport instead."
                )
            }
            other => bail!("unknown MCP transport type: '{}'", other),
        }
    }

    /// Connect via stdio transport -- spawn subprocess.
    async fn connect_stdio(&mut self) -> Result<()> {
        let command = self
            .config
            .command
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("stdio transport requires 'command' field"))?;

        let args = self.config.args.clone().unwrap_or_default();

        info!(
            server = %self.config.name,
            command = command,
            args = ?args,
            "MCP: spawning stdio server"
        );

        let mut cmd = tokio::process::Command::new(command);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref env_map) = self.config.env {
            for (k, v) in env_map {
                cmd.env(k, v);
            }
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn MCP server: {}", command))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture MCP server stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture MCP server stdout"))?;

        // Capture stderr for logging
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            let server_name = self.config.name.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!(server = %server_name, stderr = %line, "MCP server stderr");
                }
            });
        }

        let stdin_writer = Arc::new(Mutex::new(stdin));
        self.stdin_writer = Some(stdin_writer.clone());

        // Start background reader task
        let pending = self.pending.clone();
        let server_name = self.config.name.clone();
        let reader_handle = tokio::spawn(async move {
            reader_loop(stdout, pending, server_name).await;
        });
        self.reader_handle = Some(reader_handle);

        self.child = Some(child);
        self.state = McpConnectionState::Connected;

        debug!(server = %self.config.name, "MCP: stdio server connected");
        Ok(())
    }

    /// Initialize the MCP connection -- exchange capabilities with the server.
    pub async fn initialize(&mut self) -> Result<()> {
        if self.state != McpConnectionState::Connected {
            bail!("cannot initialize: not connected (state: {:?})", self.state);
        }

        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "roots": {}
            },
            "clientInfo": {
                "name": CLIENT_NAME,
                "version": CLIENT_VERSION
            }
        });

        let response = self
            .send_request("initialize", Some(params))
            .await
            .context("MCP initialize handshake failed")?;

        let init_result: InitializeResult =
            serde_json::from_value(response).context("failed to parse initialize response")?;

        self.server_capabilities = init_result.capabilities;
        self.server_info = init_result.server_info.clone();
        self.instructions = init_result.instructions;

        info!(
            server = %self.config.name,
            protocol_version = %init_result.protocol_version,
            server_name = %init_result.server_info.name,
            server_version = %init_result.server_info.version,
            "MCP: initialized"
        );

        self.send_notification("notifications/initialized", None)
            .await?;

        Ok(())
    }

    /// Disconnect from the MCP server.
    pub async fn disconnect(&mut self) {
        info!(server = %self.config.name, "MCP: disconnecting");

        self.stdin_writer.take();

        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }

        {
            let mut pending = self.pending.lock().await;
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(anyhow::anyhow!("MCP client disconnected")));
            }
        }

        self.state = McpConnectionState::Disconnected;
    }

    // -----------------------------------------------------------------------
    // Tool operations
    // -----------------------------------------------------------------------

    /// List available tools from the MCP server.
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolDef>> {
        if self.state != McpConnectionState::Connected {
            bail!("cannot list tools: not connected");
        }

        let response = self
            .send_request("tools/list", None)
            .await
            .context("tools/list request failed")?;

        let result: ListToolsResult =
            serde_json::from_value(response).context("failed to parse tools/list response")?;

        let mut tools = result.tools;
        for tool in &mut tools {
            tool.server_name = self.config.name.clone();
        }

        info!(
            server = %self.config.name,
            count = tools.len(),
            "MCP: discovered tools"
        );

        self.tools = tools.clone();
        Ok(tools)
    }

    /// Call a tool on the MCP server.
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<CallToolResult> {
        if self.state != McpConnectionState::Connected {
            bail!("cannot call tool: not connected");
        }

        let params = json!({
            "name": tool_name,
            "arguments": arguments,
        });

        let response = self
            .send_request_with_timeout("tools/call", Some(params), TOOL_CALL_TIMEOUT_SECS)
            .await
            .with_context(|| format!("tools/call '{}' failed", tool_name))?;

        let result: CallToolResult = serde_json::from_value(response)
            .with_context(|| format!("failed to parse tools/call '{}' response", tool_name))?;

        if result.is_error {
            let error_text = result
                .content
                .iter()
                .filter_map(|c| match c {
                    ToolCallContent::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            warn!(
                server = %self.config.name,
                tool = tool_name,
                error = %error_text,
                "MCP tool returned error"
            );
        }

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Resource operations
    // -----------------------------------------------------------------------

    /// List resources from the MCP server.
    pub async fn list_resources(&mut self) -> Result<Vec<McpResource>> {
        if self.state != McpConnectionState::Connected {
            bail!("cannot list resources: not connected");
        }

        let response = self
            .send_request("resources/list", None)
            .await
            .context("resources/list request failed")?;

        let result: ListResourcesResult =
            serde_json::from_value(response).context("failed to parse resources/list response")?;

        info!(
            server = %self.config.name,
            count = result.resources.len(),
            "MCP: discovered resources"
        );

        self.resources = result.resources.clone();
        Ok(result.resources)
    }

    /// Read a resource from the MCP server.
    #[allow(dead_code)]
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult> {
        if self.state != McpConnectionState::Connected {
            bail!("cannot read resource: not connected");
        }

        let params = json!({ "uri": uri });

        let response = self
            .send_request("resources/read", Some(params))
            .await
            .with_context(|| format!("resources/read '{}' failed", uri))?;

        let result: ReadResourceResult = serde_json::from_value(response)
            .with_context(|| format!("failed to parse resources/read '{}' response", uri))?;

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // JSON-RPC messaging
    // -----------------------------------------------------------------------

    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        self.send_request_with_timeout(method, params, CONNECT_TIMEOUT_SECS)
            .await
    }

    /// Send a JSON-RPC request with a custom timeout.
    async fn send_request_with_timeout(
        &self,
        method: &str,
        params: Option<Value>,
        timeout_secs: u64,
    ) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let request = JsonRpcRequest::new(id, method, params);
        let request_json =
            serde_json::to_string(&request).context("failed to serialize JSON-RPC request")?;

        debug!(
            server = %self.config.name,
            id = id,
            method = method,
            "MCP: sending request"
        );

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        self.write_line(&request_json).await?;

        let timeout = std::time::Duration::from_secs(timeout_secs);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                bail!(
                    "MCP server '{}' closed connection while waiting for response to '{}'",
                    self.config.name,
                    method
                );
            }
            Err(_) => {
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                bail!(
                    "MCP request '{}' to server '{}' timed out after {}s",
                    method,
                    self.config.name,
                    timeout_secs
                );
            }
        }
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let notification = JsonRpcNotification::new(method, params);
        let json = serde_json::to_string(&notification)
            .context("failed to serialize JSON-RPC notification")?;

        debug!(
            server = %self.config.name,
            method = method,
            "MCP: sending notification"
        );

        self.write_line(&json).await
    }

    /// Write a line to the subprocess stdin.
    async fn write_line(&self, line: &str) -> Result<()> {
        let writer = self
            .stdin_writer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("MCP server stdin not available"))?;

        let mut writer = writer.lock().await;
        writer
            .write_all(line.as_bytes())
            .await
            .context("failed to write to MCP server stdin")?;
        writer
            .write_all(b"\n")
            .await
            .context("failed to write newline to MCP server stdin")?;
        writer
            .flush()
            .await
            .context("failed to flush MCP server stdin")?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Capability checks
    // -----------------------------------------------------------------------

    /// Check whether the server advertises tool support.
    pub fn supports_tools(&self) -> bool {
        self.server_capabilities.tools.is_some()
    }

    /// Check whether the server advertises resource support.
    pub fn supports_resources(&self) -> bool {
        self.server_capabilities.resources.is_some()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod client_tests;
