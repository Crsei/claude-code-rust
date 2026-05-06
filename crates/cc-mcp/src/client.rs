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
use std::fmt;
use std::io;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use futures::TryStreamExt;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, CACHE_CONTROL, CONTENT_TYPE, USER_AGENT,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio_util::io::StreamReader;
use tracing::{debug, info, warn};
use url::Url;

use super::{
    CallToolResult, InitializeResult, JsonRpcNotification, JsonRpcRequest, ListResourcesResult,
    ListToolsResult, McpConnectionState, McpResource, McpServerConfig, McpToolDef,
    ReadResourceResult, ServerCapabilities, ServerInfo, ToolCallContent, CLIENT_NAME,
    CLIENT_VERSION, CONNECT_TIMEOUT_SECS, PROTOCOL_VERSION, TOOL_CALL_TIMEOUT_SECS,
};

use super::transport::{reader_loop, sse_reader_loop};

#[derive(Debug, Clone)]
struct McpAuthNeededError {
    server_name: String,
    status: StatusCode,
}

impl fmt::Display for McpAuthNeededError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MCP server '{}' requires authentication (HTTP {}); OAuth/interactive auth is not implemented yet",
            self.server_name,
            self.status.as_u16()
        )
    }
}

impl std::error::Error for McpAuthNeededError {}

pub fn is_auth_needed_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<McpAuthNeededError>().is_some()
}

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
    /// HTTP POST sender for SSE transport.
    sse_sender: Option<SseHttpSender>,
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
            sse_sender: None,
            next_id: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // -----------------------------------------------------------------------
    // Connection lifecycle
    // -----------------------------------------------------------------------

    /// Connect to the MCP server.
    pub async fn connect(&mut self) -> Result<()> {
        let result = match self.config.transport.as_str() {
            "stdio" => self.connect_stdio().await,
            "sse" => self.connect_sse().await,
            other => bail!("unknown MCP transport type: '{}'", other),
        };

        if let Err(ref e) = result {
            let state = if is_auth_needed_error(e) {
                "auth-needed"
            } else {
                "error"
            };
            super::emit_event(super::McpSubsystemEvent::ServerStateChanged {
                server_name: self.config.name.clone(),
                state: state.to_string(),
                error: Some(e.to_string()),
            });
        }

        result
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

        super::emit_event(super::McpSubsystemEvent::ServerStateChanged {
            server_name: self.config.name.clone(),
            state: "connected".to_string(),
            error: None,
        });

        debug!(server = %self.config.name, "MCP: stdio server connected");
        Ok(())
    }

    /// Connect via HTTP SSE transport.
    ///
    /// Loopback `http://` URLs keep the minimal raw TCP implementation. Remote
    /// endpoints must use `https://` and go through reqwest with redirects
    /// disabled so credentials are never forwarded to an unvalidated location.
    async fn connect_sse(&mut self) -> Result<()> {
        validate_sse_config(&self.config)?;
        let url = self
            .config
            .url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("sse transport requires 'url' field"))?;
        let base_target = SseConnectTarget::parse(url)?;
        let headers = normalized_sse_headers(&self.config);

        super::emit_event(super::McpSubsystemEvent::ServerStateChanged {
            server_name: self.config.name.clone(),
            state: "connecting".to_string(),
            error: None,
        });

        info!(
            server = %self.config.name,
            url = %redact_url_for_log(url),
            "MCP: connecting to SSE server"
        );

        let (reader_handle, endpoint_rx, remote_http_client) = match &base_target {
            SseConnectTarget::Loopback(target) => {
                let reader =
                    connect_loopback_sse_stream(target, &headers, &self.config.name).await?;
                let (handle, rx) =
                    spawn_sse_reader(reader, self.pending.clone(), self.config.name.clone());
                (handle, rx, None)
            }
            SseConnectTarget::RemoteHttps(target) => {
                let http_client = remote_sse_http_client()?;
                let reader = connect_remote_https_sse_stream(
                    &http_client,
                    target,
                    &headers,
                    &self.config.name,
                )
                .await?;
                let (handle, rx) =
                    spawn_sse_reader(reader, self.pending.clone(), self.config.name.clone());
                (handle, rx, Some(http_client))
            }
        };

        let endpoint_data =
            receive_sse_endpoint(&self.config.name, &reader_handle, endpoint_rx).await?;
        let post_target = base_target.resolve_endpoint(&endpoint_data)?;
        self.sse_sender = Some(SseHttpSender {
            target: post_target,
            headers,
            server_name: self.config.name.clone(),
            http_client: remote_http_client,
        });
        self.reader_handle = Some(reader_handle);
        self.state = McpConnectionState::Connected;

        super::emit_event(super::McpSubsystemEvent::ServerStateChanged {
            server_name: self.config.name.clone(),
            state: "connected".to_string(),
            error: None,
        });

        debug!(server = %self.config.name, "MCP: SSE server connected");
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
        self.sse_sender.take();

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

        super::emit_event(super::McpSubsystemEvent::ServerStateChanged {
            server_name: self.config.name.clone(),
            state: "disconnected".to_string(),
            error: None,
        });
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

        super::emit_event(super::McpSubsystemEvent::ToolsDiscovered {
            server_name: self.config.name.clone(),
            tools: self
                .tools
                .iter()
                .map(|t| super::McpToolInfo {
                    server_name: self.config.name.clone(),
                    tool_name: t.name.clone(),
                    description: t.description.clone(),
                })
                .collect(),
        });

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

        super::emit_event(super::McpSubsystemEvent::ResourcesDiscovered {
            server_name: self.config.name.clone(),
            resources: self
                .resources
                .iter()
                .map(|r| super::McpResourceInfo {
                    server_name: self.config.name.clone(),
                    uri: r.uri.clone(),
                    name: r.name.clone(),
                    description: None,
                    mime_type: r.mime_type.clone(),
                })
                .collect(),
        });

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

        if let Err(error) = self.write_line(&request_json).await {
            let mut pending = self.pending.lock().await;
            pending.remove(&id);
            return Err(error);
        }

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

    /// Write a JSON-RPC line to the active transport.
    async fn write_line(&self, line: &str) -> Result<()> {
        if let Some(sender) = &self.sse_sender {
            return sender.post_json(line).await;
        }

        let writer = self.stdin_writer.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "MCP transport writer not available for server '{}'",
                self.config.name
            )
        })?;

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

fn validate_sse_config(config: &McpServerConfig) -> Result<()> {
    let url = config
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("sse transport requires 'url' field"))?;
    validate_sse_url(url)?;

    if let Some(headers) = &config.headers {
        for (name, value) in headers {
            validate_header_name(name)?;
            validate_header_value(name, value)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum SseConnectTarget {
    Loopback(SseHttpTarget),
    RemoteHttps(RemoteSseHttpTarget),
}

impl SseConnectTarget {
    fn parse(url: &str) -> Result<Self> {
        validate_sse_url(url)?;
        if url.starts_with("https://") {
            return Ok(Self::RemoteHttps(RemoteSseHttpTarget::parse(url)?));
        }
        Ok(Self::Loopback(SseHttpTarget::parse_loopback(url)?))
    }

    fn resolve_endpoint(&self, endpoint: &str) -> Result<SsePostTarget> {
        match self {
            Self::Loopback(target) => target
                .resolve_endpoint(endpoint)
                .map(SsePostTarget::Loopback),
            Self::RemoteHttps(target) => target
                .resolve_endpoint(endpoint)
                .map(SsePostTarget::RemoteHttps),
        }
    }
}

#[derive(Debug, Clone)]
enum SsePostTarget {
    Loopback(SseHttpTarget),
    RemoteHttps(RemoteSseHttpTarget),
}

#[derive(Debug, Clone)]
struct SseHttpTarget {
    host: String,
    port: u16,
    authority: String,
    path_and_query: String,
}

impl SseHttpTarget {
    fn parse_loopback(url: &str) -> Result<Self> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| anyhow::anyhow!("loopback SSE transport requires an http:// URL"))?;
        let rest = rest.split('#').next().unwrap_or(rest);
        let split_at = rest.find(['/', '?']).unwrap_or(rest.len());
        let authority = &rest[..split_at];
        if authority.is_empty() {
            bail!("sse url must include a host");
        }
        let suffix = &rest[split_at..];
        let path_and_query = if suffix.is_empty() {
            "/".to_string()
        } else if suffix.starts_with('/') {
            suffix.to_string()
        } else {
            format!("/{}", suffix)
        };

        let (host, port) = parse_authority(authority)?;
        if !is_loopback_host(&host) {
            bail!("sse transport requires https URLs unless the host is loopback");
        }

        Ok(Self {
            host,
            port,
            authority: authority.to_string(),
            path_and_query,
        })
    }

    fn socket_addr(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    fn resolve_endpoint(&self, endpoint: &str) -> Result<Self> {
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            bail!("MCP SSE endpoint event was empty");
        }
        if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            return Self::parse_loopback(endpoint);
        }
        if endpoint.starts_with('/') {
            let mut target = self.clone();
            target.path_and_query = strip_fragment(endpoint).to_string();
            return Ok(target);
        }
        bail!(
            "MCP SSE endpoint event must be an absolute loopback URL or absolute path, got '{}'",
            endpoint
        );
    }
}

#[derive(Debug, Clone)]
struct RemoteSseHttpTarget {
    url: Url,
}

impl RemoteSseHttpTarget {
    fn parse(url: &str) -> Result<Self> {
        let mut url = Url::parse(url).context("invalid remote SSE URL")?;
        validate_remote_https_url(&url)?;
        url.set_fragment(None);
        Ok(Self { url })
    }

    fn resolve_endpoint(&self, endpoint: &str) -> Result<Self> {
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            bail!("MCP SSE endpoint event was empty");
        }

        let mut resolved = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            Url::parse(endpoint).context("invalid MCP SSE endpoint event URL")?
        } else {
            self.url
                .join(endpoint)
                .context("invalid MCP SSE endpoint event path")?
        };
        resolved.set_fragment(None);
        validate_remote_https_url(&resolved)?;
        if !same_origin(&self.url, &resolved) {
            bail!("MCP SSE endpoint event must stay on the same https origin");
        }
        Ok(Self { url: resolved })
    }
}

#[derive(Debug, Clone)]
struct SseHttpSender {
    target: SsePostTarget,
    headers: Vec<(String, String)>,
    server_name: String,
    http_client: Option<reqwest::Client>,
}

impl SseHttpSender {
    async fn post_json(&self, body: &str) -> Result<()> {
        match &self.target {
            SsePostTarget::Loopback(target) => {
                post_loopback_sse_json(target, &self.headers, &self.server_name, body).await
            }
            SsePostTarget::RemoteHttps(target) => {
                let http_client = self.http_client.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "remote SSE HTTP client missing for server '{}'",
                        self.server_name
                    )
                })?;
                post_remote_https_sse_json(
                    http_client,
                    target,
                    &self.headers,
                    &self.server_name,
                    body,
                )
                .await
            }
        }
    }
}

async fn post_loopback_sse_json(
    target: &SseHttpTarget,
    headers: &[(String, String)],
    server_name: &str,
    body: &str,
) -> Result<()> {
    let mut stream = TcpStream::connect(target.socket_addr())
        .await
        .with_context(|| {
            format!(
                "failed to connect to MCP SSE endpoint for server '{}'",
                server_name
            )
        })?;
    let request = build_sse_post_request(target, headers, body);
    stream
        .write_all(request.as_bytes())
        .await
        .context("failed to send MCP SSE POST request")?;
    stream
        .flush()
        .await
        .context("failed to flush MCP SSE POST request")?;

    let mut reader = BufReader::new(stream);
    let status = read_http_response_head(&mut reader)
        .await
        .context("failed to read MCP SSE POST response headers")?;
    handle_sse_post_status(StatusCode::from_u16(status)?, server_name)?;

    Ok(())
}

async fn post_remote_https_sse_json(
    http_client: &reqwest::Client,
    target: &RemoteSseHttpTarget,
    headers: &[(String, String)],
    server_name: &str,
    body: &str,
) -> Result<()> {
    let request = http_client
        .post(target.url.clone())
        .header(CONTENT_TYPE, "application/json")
        .headers(reqwest_header_map(headers)?)
        .body(body.to_string());
    let response = request.send().await.with_context(|| {
        format!(
            "failed to POST MCP SSE JSON-RPC for server '{}'",
            server_name
        )
    })?;
    handle_sse_post_status(response.status(), server_name)?;

    Ok(())
}

async fn connect_loopback_sse_stream(
    target: &SseHttpTarget,
    headers: &[(String, String)],
    server_name: &str,
) -> Result<BufReader<TcpStream>> {
    let mut stream = TcpStream::connect(target.socket_addr())
        .await
        .with_context(|| format!("failed to connect to MCP SSE server '{}'", server_name))?;
    let request = build_sse_get_request(target, headers);
    stream
        .write_all(request.as_bytes())
        .await
        .context("failed to send MCP SSE GET request")?;
    stream
        .flush()
        .await
        .context("failed to flush MCP SSE GET request")?;

    let mut reader = BufReader::new(stream);
    let status = read_http_response_head(&mut reader)
        .await
        .context("failed to read MCP SSE response headers")?;
    handle_sse_event_stream_status(StatusCode::from_u16(status)?, server_name)?;

    Ok(reader)
}

async fn connect_remote_https_sse_stream(
    http_client: &reqwest::Client,
    target: &RemoteSseHttpTarget,
    headers: &[(String, String)],
    server_name: &str,
) -> Result<
    BufReader<
        StreamReader<
            impl futures::Stream<Item = io::Result<bytes::Bytes>> + Send + 'static,
            bytes::Bytes,
        >,
    >,
> {
    let response = http_client
        .get(target.url.clone())
        .header(ACCEPT, "text/event-stream")
        .header(CACHE_CONTROL, "no-cache")
        .header(USER_AGENT, format!("{CLIENT_NAME}/{CLIENT_VERSION}"))
        .headers(reqwest_header_map(headers)?)
        .send()
        .await
        .with_context(|| format!("failed to connect to MCP SSE server '{}'", server_name))?;
    handle_sse_event_stream_status(response.status(), server_name)?;
    if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
        let content_type = content_type.to_str().unwrap_or_default();
        if !content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case("text/event-stream")
        {
            bail!(
                "MCP SSE server '{}' returned non-SSE content type '{}'",
                server_name,
                content_type
            );
        }
    }

    let body_stream = response.bytes_stream().map_err(reqwest_error_to_io);
    Ok(BufReader::new(StreamReader::new(body_stream)))
}

fn remote_sse_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .build()
        .context("failed to build remote MCP SSE HTTP client")
}

fn spawn_sse_reader<R>(
    reader: R,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    server_name: String,
) -> (
    tokio::task::JoinHandle<()>,
    oneshot::Receiver<Result<String>>,
)
where
    R: tokio::io::AsyncBufRead + Unpin + Send + 'static,
{
    let (endpoint_tx, endpoint_rx) = oneshot::channel();
    let reader_handle = tokio::spawn(async move {
        sse_reader_loop(reader, pending, server_name, Some(endpoint_tx)).await;
    });
    (reader_handle, endpoint_rx)
}

async fn receive_sse_endpoint(
    server_name: &str,
    reader_handle: &tokio::task::JoinHandle<()>,
    endpoint_rx: oneshot::Receiver<Result<String>>,
) -> Result<String> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS),
        endpoint_rx,
    )
    .await
    {
        Ok(Ok(Ok(endpoint))) => Ok(endpoint),
        Ok(Ok(Err(e))) => {
            reader_handle.abort();
            Err(e)
        }
        Ok(Err(_)) => {
            reader_handle.abort();
            bail!(
                "MCP SSE server '{}' closed before sending endpoint event",
                server_name
            );
        }
        Err(_) => {
            reader_handle.abort();
            bail!(
                "MCP SSE server '{}' did not send endpoint event within {}s",
                server_name,
                CONNECT_TIMEOUT_SECS
            );
        }
    }
}

fn handle_sse_event_stream_status(status: StatusCode, server_name: &str) -> Result<()> {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(McpAuthNeededError {
            server_name: server_name.to_string(),
            status,
        }
        .into());
    }
    if status.is_redirection() {
        bail!(
            "MCP SSE server '{}' returned HTTP {}; redirects are disabled for SSE transport",
            server_name,
            status.as_u16()
        );
    }
    if !status.is_success() {
        bail!(
            "MCP SSE server '{}' returned HTTP {} for event stream",
            server_name,
            status.as_u16()
        );
    }
    Ok(())
}

fn handle_sse_post_status(status: StatusCode, server_name: &str) -> Result<()> {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        super::emit_event(super::McpSubsystemEvent::ServerStateChanged {
            server_name: server_name.to_string(),
            state: "auth-needed".to_string(),
            error: Some(
                McpAuthNeededError {
                    server_name: server_name.to_string(),
                    status,
                }
                .to_string(),
            ),
        });
        return Err(McpAuthNeededError {
            server_name: server_name.to_string(),
            status,
        }
        .into());
    }
    if status.is_redirection() {
        bail!(
            "MCP SSE server '{}' returned HTTP {}; redirects are disabled for JSON-RPC POST",
            server_name,
            status.as_u16()
        );
    }
    if !status.is_success() {
        bail!(
            "MCP SSE server '{}' returned HTTP {} for JSON-RPC POST",
            server_name,
            status.as_u16()
        );
    }
    Ok(())
}

fn reqwest_header_map(headers: &[(String, String)]) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .with_context(|| format!("invalid SSE header name '{}'", name))?;
        let header_value = HeaderValue::from_str(value)
            .with_context(|| format!("invalid SSE header value for '{}'", name))?;
        map.insert(header_name, header_value);
    }
    Ok(map)
}

fn reqwest_error_to_io(error: reqwest::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

fn parse_authority(authority: &str) -> Result<(String, u16)> {
    if let Some(after_bracket) = authority.strip_prefix('[') {
        let (host, rest) = after_bracket
            .split_once(']')
            .ok_or_else(|| anyhow::anyhow!("invalid IPv6 host in SSE URL"))?;
        let port = if let Some(port) = rest.strip_prefix(':') {
            parse_port(port)?
        } else if rest.is_empty() {
            80
        } else {
            bail!("invalid IPv6 authority in SSE URL");
        };
        return Ok((host.to_string(), port));
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            (host, parse_port(port)?)
        }
        _ => (authority, 80),
    };

    Ok((host.to_string(), port))
}

fn parse_port(port: &str) -> Result<u16> {
    port.parse::<u16>()
        .with_context(|| format!("invalid port '{}' in SSE URL", port))
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn strip_fragment(value: &str) -> &str {
    value.split('#').next().unwrap_or(value)
}

fn normalized_sse_headers(config: &McpServerConfig) -> Vec<(String, String)> {
    let mut headers = config
        .headers
        .as_ref()
        .map(|headers| {
            headers
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    headers.sort_by(|(left, _), (right, _)| left.cmp(right));
    headers
}

fn build_sse_get_request(target: &SseHttpTarget, headers: &[(String, String)]) -> String {
    let mut request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n",
        target.path_and_query, target.authority
    );
    append_user_headers(&mut request, headers);
    request.push_str("\r\n");
    request
}

fn build_sse_post_request(
    target: &SseHttpTarget,
    headers: &[(String, String)],
    body: &str,
) -> String {
    let mut request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        target.path_and_query,
        target.authority,
        body.as_bytes().len()
    );
    append_user_headers(&mut request, headers);
    request.push_str("\r\n");
    request.push_str(body);
    request
}

fn append_user_headers(request: &mut String, headers: &[(String, String)]) {
    for (name, value) in headers {
        request.push_str(name);
        request.push_str(": ");
        request.push_str(value);
        request.push_str("\r\n");
    }
}

async fn read_http_response_head(reader: &mut BufReader<TcpStream>) -> Result<u16> {
    let mut status_line = String::new();
    if reader
        .read_line(&mut status_line)
        .await
        .context("failed to read HTTP status line")?
        == 0
    {
        bail!("HTTP server closed connection before status line");
    }

    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("invalid HTTP status line: {}", status_line.trim()))?
        .parse::<u16>()
        .with_context(|| format!("invalid HTTP status line: {}", status_line.trim()))?;

    loop {
        let mut line = String::new();
        if reader
            .read_line(&mut line)
            .await
            .context("failed to read HTTP response header")?
            == 0
        {
            bail!("HTTP server closed connection before response headers completed");
        }
        if line.trim_end_matches(['\r', '\n']).is_empty() {
            break;
        }
    }

    Ok(status)
}

fn validate_sse_url(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed != url {
        bail!("sse url must be a non-empty URL without surrounding whitespace");
    }
    if trimmed.starts_with("https://") {
        let parsed = Url::parse(trimmed).context("invalid remote SSE URL")?;
        validate_remote_https_url(&parsed)?;
        return Ok(());
    }
    if let Some(rest) = trimmed.strip_prefix("http://") {
        let host_port = rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
            .trim();
        let host = host_port
            .strip_prefix('[')
            .and_then(|value| value.split(']').next())
            .or_else(|| host_port.split(':').next())
            .unwrap_or_default();
        if matches!(host, "localhost" | "127.0.0.1" | "::1") {
            return Ok(());
        }
        bail!("sse transport requires https URLs unless the host is loopback");
    }
    bail!("sse transport requires an http:// or https:// URL");
}

fn validate_header_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| matches!(b, b'!' | b'#'..=b'\'' | b'*' | b'+' | b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'^' | b'_' | b'`' | b'a'..=b'z' | b'|' | b'~'))
    {
        bail!("invalid SSE header name '{}'", name);
    }
    if is_reserved_sse_header(name) {
        bail!(
            "unsafe SSE header '{}' is managed by the MCP transport",
            name
        );
    }
    Ok(())
}

fn validate_header_value(name: &str, value: &str) -> Result<()> {
    if value.contains('\r') || value.contains('\n') || value.contains('\0') {
        bail!("invalid SSE header value for '{}'", name);
    }
    Ok(())
}

fn validate_remote_https_url(url: &Url) -> Result<()> {
    if url.scheme() != "https" {
        bail!("remote SSE transport requires https URLs");
    }
    if url.host_str().is_none() {
        bail!("remote SSE URL must include a host");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("remote SSE URL must not include embedded credentials");
    }
    Ok(())
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn is_reserved_sse_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host" | "connection" | "content-length" | "transfer-encoding"
    )
}

fn redact_url_for_log(url: &str) -> String {
    match Url::parse(url) {
        Ok(mut parsed) => {
            if parsed.query().is_some() {
                parsed.set_query(Some("redacted"));
            }
            parsed.set_fragment(None);
            parsed.to_string()
        }
        Err(_) => strip_fragment(url)
            .split('?')
            .next()
            .unwrap_or(url)
            .to_string(),
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
