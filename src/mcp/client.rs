//! MCP client — communicates with MCP servers over stdio (subprocess) or SSE.
//!
//! The stdio transport spawns a subprocess and exchanges line-delimited
//! JSON-RPC 2.0 messages over stdin/stdout. A background reader task
//! dispatches incoming responses to waiting request futures.
//!
//! Lifecycle:
//!   1. `McpClient::connect()` — spawn process, start reader task
//!   2. `McpClient::initialize()` — JSON-RPC `initialize` + `notifications/initialized`
//!   3. `McpClient::list_tools()` / `call_tool()` / `list_resources()` / `read_resource()`
//!   4. `McpClient::disconnect()` — graceful shutdown

#![allow(unused)]

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
    CallToolResult, InitializeResult, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    ListResourcesResult, ListToolsResult, McpConnectionState, McpResource, McpResourceContent,
    McpServerConfig, McpToolDef, ReadResourceResult, ServerCapabilities, ServerInfo,
    ToolCallContent, CONNECT_TIMEOUT_SECS, PROTOCOL_VERSION, TOOL_CALL_TIMEOUT_SECS,
    CLIENT_NAME, CLIENT_VERSION,
};

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

    /// Stdin writer for the subprocess (wrapped in Arc<Mutex> for shared access).
    stdin_writer: Option<Arc<Mutex<tokio::process::ChildStdin>>>,
    /// Handle to the background reader task.
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    /// Handle to the child process.
    child: Option<tokio::process::Child>,
    /// Monotonically increasing request ID counter.
    next_id: Arc<AtomicU64>,
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
    ///
    /// For stdio transport: spawns the subprocess, sets up stdin/stdout pipes,
    /// starts the background reader task.
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

    /// Connect via stdio transport — spawn subprocess.
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

        // Build environment
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply custom environment variables
        if let Some(ref env_map) = self.config.env {
            for (k, v) in env_map {
                cmd.env(k, v);
            }
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn MCP server: {}", command))?;

        // Take ownership of stdin and stdout
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture MCP server stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture MCP server stdout"))?;

        // Capture stderr for logging (best-effort)
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

    /// Initialize the MCP connection — exchange capabilities with the server.
    ///
    /// Must be called after `connect()`. Sends the `initialize` request and
    /// the `notifications/initialized` notification.
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

        let init_result: InitializeResult = serde_json::from_value(response)
            .context("failed to parse initialize response")?;

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

        // Send the initialized notification (no response expected)
        self.send_notification("notifications/initialized", None)
            .await?;

        Ok(())
    }

    /// List available tools from the MCP server.
    ///
    /// Updates `self.tools` and returns the list.
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolDef>> {
        if self.state != McpConnectionState::Connected {
            bail!("cannot list tools: not connected");
        }

        let response = self
            .send_request("tools/list", None)
            .await
            .context("tools/list request failed")?;

        let result: ListToolsResult = serde_json::from_value(response)
            .context("failed to parse tools/list response")?;

        // Tag each tool with the server name
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
    ///
    /// Returns the result content blocks.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<CallToolResult> {
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

    /// List resources from the MCP server.
    ///
    /// Updates `self.resources` and returns the list.
    pub async fn list_resources(&mut self) -> Result<Vec<McpResource>> {
        if self.state != McpConnectionState::Connected {
            bail!("cannot list resources: not connected");
        }

        let response = self
            .send_request("resources/list", None)
            .await
            .context("resources/list request failed")?;

        let result: ListResourcesResult = serde_json::from_value(response)
            .context("failed to parse resources/list response")?;

        info!(
            server = %self.config.name,
            count = result.resources.len(),
            "MCP: discovered resources"
        );

        self.resources = result.resources.clone();
        Ok(result.resources)
    }

    /// Read a resource from the MCP server.
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

    /// Disconnect from the MCP server.
    ///
    /// Sends a best-effort shutdown, then kills the subprocess.
    pub async fn disconnect(&mut self) {
        info!(server = %self.config.name, "MCP: disconnecting");

        // Drop the stdin writer to close the pipe
        self.stdin_writer.take();

        // Abort the reader task
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

        // Kill the child process
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }

        // Clear pending requests
        {
            let mut pending = self.pending.lock().await;
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(anyhow::anyhow!("MCP client disconnected")));
            }
        }

        self.state = McpConnectionState::Disconnected;
    }

    // -----------------------------------------------------------------------
    // JSON-RPC messaging
    // -----------------------------------------------------------------------

    /// Send a JSON-RPC request and wait for the response.
    ///
    /// Uses the default connect timeout.
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
        let request_json = serde_json::to_string(&request)
            .context("failed to serialize JSON-RPC request")?;

        debug!(
            server = %self.config.name,
            id = id,
            method = method,
            "MCP: sending request"
        );

        // Register a oneshot channel for the response
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        // Write to stdin
        self.write_line(&request_json).await?;

        // Wait for response with timeout
        let timeout = std::time::Duration::from_secs(timeout_secs);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // Oneshot channel was dropped (reader task died)
                bail!(
                    "MCP server '{}' closed connection while waiting for response to '{}'",
                    self.config.name,
                    method
                );
            }
            Err(_) => {
                // Timeout — remove from pending
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
        // Best-effort cleanup: abort the reader and kill the child.
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
        // We can't do async in Drop, so just try to start_kill().
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}

// ---------------------------------------------------------------------------
// Background reader task
// ---------------------------------------------------------------------------

/// Background task that reads JSON-RPC responses from the MCP server's stdout.
///
/// Each line is parsed as a JSON-RPC response and dispatched to the
/// corresponding pending request via its oneshot channel.
async fn reader_loop(
    stdout: tokio::process::ChildStdout,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    server_name: String,
) {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                // Try to parse as a JSON-RPC response
                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(response) => {
                        dispatch_response(&pending, &server_name, response).await;
                    }
                    Err(_) => {
                        // Could be a notification from the server — try parsing
                        match serde_json::from_str::<Value>(&line) {
                            Ok(val) => {
                                // If it has an "id" field, it's a malformed response
                                if val.get("id").is_some() {
                                    warn!(
                                        server = %server_name,
                                        line = %line,
                                        "MCP: received malformed response"
                                    );
                                } else if let Some(method) = val.get("method").and_then(|m| m.as_str()) {
                                    // Server-initiated notification
                                    debug!(
                                        server = %server_name,
                                        method = method,
                                        "MCP: received server notification"
                                    );
                                } else {
                                    debug!(
                                        server = %server_name,
                                        "MCP: received unknown JSON message"
                                    );
                                }
                            }
                            Err(e) => {
                                debug!(
                                    server = %server_name,
                                    error = %e,
                                    line = %line,
                                    "MCP: non-JSON line from server stdout"
                                );
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                // EOF — server closed stdout
                info!(server = %server_name, "MCP: server stdout closed (EOF)");
                break;
            }
            Err(e) => {
                warn!(
                    server = %server_name,
                    error = %e,
                    "MCP: error reading server stdout"
                );
                break;
            }
        }
    }

    // On exit, fail all pending requests
    let mut pending = pending.lock().await;
    for (id, sender) in pending.drain() {
        debug!(server = %server_name, id = id, "MCP: failing pending request (reader exited)");
        let _ = sender.send(Err(anyhow::anyhow!(
            "MCP server '{}' closed connection",
            server_name
        )));
    }
}

/// Dispatch a parsed JSON-RPC response to the corresponding pending request.
async fn dispatch_response(
    pending: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    server_name: &str,
    response: JsonRpcResponse,
) {
    let id = match response.id.as_u64() {
        Some(id) => id,
        None => {
            debug!(
                server = %server_name,
                id = ?response.id,
                "MCP: response has non-integer id, ignoring"
            );
            return;
        }
    };

    let mut pending = pending.lock().await;
    if let Some(sender) = pending.remove(&id) {
        let result = if let Some(error) = response.error {
            Err(anyhow::anyhow!(
                "MCP server '{}' returned error (code {}): {}",
                server_name,
                error.code,
                error.message
            ))
        } else {
            Ok(response.result.unwrap_or(Value::Null))
        };

        let _ = sender.send(result);
    } else {
        debug!(
            server = %server_name,
            id = id,
            "MCP: received response for unknown request id"
        );
    }
}

// ---------------------------------------------------------------------------
// McpManager — manages multiple MCP server connections
// ---------------------------------------------------------------------------

/// Manages multiple MCP server connections.
///
/// Provides a high-level interface for discovering and connecting to MCP
/// servers, and aggregating their tools and resources.
pub struct McpManager {
    /// Active clients, keyed by server name.
    pub clients: HashMap<String, McpClient>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Connect to all configured MCP servers.
    ///
    /// Discovers servers from settings, connects to each one, and
    /// initializes them. Failures for individual servers are logged
    /// but do not prevent other servers from connecting.
    pub async fn connect_all(&mut self, configs: Vec<McpServerConfig>) -> Result<()> {
        for config in configs {
            let name = config.name.clone();
            let mut client = McpClient::new(config);

            match client.connect().await {
                Ok(()) => {
                    match client.initialize().await {
                        Ok(()) => {
                            // Discover tools if supported
                            if client.supports_tools() {
                                if let Err(e) = client.list_tools().await {
                                    warn!(
                                        server = %name,
                                        error = %e,
                                        "MCP: failed to list tools"
                                    );
                                }
                            }

                            // Discover resources if supported
                            if client.supports_resources() {
                                if let Err(e) = client.list_resources().await {
                                    warn!(
                                        server = %name,
                                        error = %e,
                                        "MCP: failed to list resources"
                                    );
                                }
                            }

                            info!(
                                server = %name,
                                tools = client.tools.len(),
                                resources = client.resources.len(),
                                "MCP: server ready"
                            );

                            self.clients.insert(name, client);
                        }
                        Err(e) => {
                            warn!(
                                server = %name,
                                error = %e,
                                "MCP: failed to initialize server"
                            );
                            client.disconnect().await;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        server = %name,
                        error = %e,
                        "MCP: failed to connect to server"
                    );
                }
            }
        }

        Ok(())
    }

    /// Get all tools from all connected servers.
    pub fn all_tools(&self) -> Vec<McpToolDef> {
        self.clients
            .values()
            .flat_map(|c| c.tools.iter().cloned())
            .collect()
    }

    /// Get all resources from all connected servers.
    pub fn all_resources(&self) -> Vec<McpResource> {
        self.clients
            .values()
            .flat_map(|c| c.resources.iter().cloned())
            .collect()
    }

    /// Find the client that owns a tool by name.
    pub fn find_client_for_tool(&self, tool_name: &str) -> Option<&McpClient> {
        self.clients
            .values()
            .find(|c| c.tools.iter().any(|t| t.name == tool_name))
    }

    /// Disconnect from all servers.
    pub async fn disconnect_all(&mut self) {
        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in names {
            if let Some(mut client) = self.clients.remove(&name) {
                client.disconnect().await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_client_new() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: Some(vec!["hello".to_string()]),
            url: None,
            headers: None,
            env: None,
        };

        let client = McpClient::new(config);
        assert_eq!(client.state, McpConnectionState::Pending);
        assert!(client.tools.is_empty());
        assert!(client.resources.is_empty());
    }

    #[test]
    fn test_mcp_manager_new() {
        let manager = McpManager::new();
        assert!(manager.clients.is_empty());
        assert!(manager.all_tools().is_empty());
        assert!(manager.all_resources().is_empty());
    }

    #[test]
    fn test_jsonrpc_request_ids_increment() {
        let config = McpServerConfig {
            name: "test".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
        };
        let client = McpClient::new(config);

        let id1 = client.next_id.fetch_add(1, Ordering::SeqCst);
        let id2 = client.next_id.fetch_add(1, Ordering::SeqCst);
        assert_eq!(id1 + 1, id2);
    }

    #[tokio::test]
    async fn test_dispatch_response_success() {
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let (tx, rx) = oneshot::channel();
        {
            let mut p = pending.lock().await;
            p.insert(42, tx);
        }

        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(42),
            result: Some(json!({"tools": []})),
            error: None,
        };

        dispatch_response(&pending, "test", response).await;

        let result = rx.await.unwrap().unwrap();
        assert_eq!(result, json!({"tools": []}));
    }

    #[tokio::test]
    async fn test_dispatch_response_error() {
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let (tx, rx) = oneshot::channel();
        {
            let mut p = pending.lock().await;
            p.insert(7, tx);
        }

        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(7),
            result: None,
            error: Some(super::super::JsonRpcError {
                code: -32600,
                message: "Invalid Request".to_string(),
                data: None,
            }),
        };

        dispatch_response(&pending, "test", response).await;

        let result = rx.await.unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid Request"));
    }

    #[tokio::test]
    async fn test_dispatch_response_unknown_id() {
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // No pending request with id=99
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(99),
            result: Some(json!("ignored")),
            error: None,
        };

        // Should not panic
        dispatch_response(&pending, "test", response).await;
    }

    #[tokio::test]
    async fn test_connect_stdio_missing_command() {
        let config = McpServerConfig {
            name: "bad-server".to_string(),
            transport: "stdio".to_string(),
            command: None, // Missing command
            args: None,
            url: None,
            headers: None,
            env: None,
        };

        let mut client = McpClient::new(config);
        let result = client.connect().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("command"));
    }

    #[tokio::test]
    async fn test_connect_sse_not_implemented() {
        let config = McpServerConfig {
            name: "sse-server".to_string(),
            transport: "sse".to_string(),
            command: None,
            args: None,
            url: Some("http://localhost:8080".to_string()),
            headers: None,
            env: None,
        };

        let mut client = McpClient::new(config);
        let result = client.connect().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("SSE"));
    }

    #[tokio::test]
    async fn test_disconnect_idempotent() {
        let config = McpServerConfig {
            name: "test".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
        };

        let mut client = McpClient::new(config);
        // Disconnect without connecting should not panic
        client.disconnect().await;
        assert_eq!(client.state, McpConnectionState::Disconnected);

        // Double disconnect
        client.disconnect().await;
        assert_eq!(client.state, McpConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_list_tools_not_connected() {
        let config = McpServerConfig {
            name: "test".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
        };

        let mut client = McpClient::new(config);
        let result = client.list_tools().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not connected"));
    }

    #[tokio::test]
    async fn test_call_tool_not_connected() {
        let config = McpServerConfig {
            name: "test".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
        };

        let client = McpClient::new(config);
        let result = client.call_tool("test", json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mcp_manager_connect_all_invalid_server() {
        let mut manager = McpManager::new();
        let configs = vec![McpServerConfig {
            name: "nonexistent".to_string(),
            transport: "stdio".to_string(),
            command: Some("this-command-does-not-exist-at-all-12345".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
        }];

        // Should not fail — individual server failures are logged
        let result = manager.connect_all(configs).await;
        assert!(result.is_ok());
        // But no clients should be connected
        assert!(manager.clients.is_empty());
    }
}
