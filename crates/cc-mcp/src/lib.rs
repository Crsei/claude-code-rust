//! MCP (Model Context Protocol) — JSON-RPC 2.0 based protocol for external
//! tool servers.
//!
//! MCP allows Claude Code to communicate with external tool servers via:
//! - **stdio**: spawn a subprocess, communicate via stdin/stdout (primary)
//! - **sse**: HTTP Server-Sent Events (requires network feature)
//!
//! Protocol specification: https://modelcontextprotocol.io/specification/2025-03-26/

pub mod channel;
pub mod client;
pub mod discovery;
pub mod manager;
pub mod transport;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Protocol constants
// ---------------------------------------------------------------------------

/// MCP protocol version we advertise during initialization.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Client name sent during initialization.
pub const CLIENT_NAME: &str = "claude-code-rs";

/// Client version sent during initialization.
pub const CLIENT_VERSION: &str = "0.1.0";

/// Default connection/initialize timeout (seconds).
pub const CONNECT_TIMEOUT_SECS: u64 = 30;

/// Default tool call timeout (seconds). Very generous — MCP tools can be slow.
pub const TOOL_CALL_TIMEOUT_SECS: u64 = 300;

// ---------------------------------------------------------------------------
// Subsystem event emission
// ---------------------------------------------------------------------------
//
// cc-mcp used to hold a
// `broadcast::Sender<crate::ipc::subsystem_events::SubsystemEvent>` directly.
// Once mcp moved into its own crate (issue #72), reaching back into the root
// crate's `ipc` module would have been a cycle. The host now registers a
// simple callback that receives cc-mcp's own minimal event enum and is
// responsible for adapting it into the broader `SubsystemEvent` wrapper.
// Same pattern as `cc-skills::set_event_callback`.

use parking_lot::Mutex as SyncMutex;
use std::sync::LazyLock;

/// Tool information surfaced to the host when `ToolsDiscovered` fires.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
}

/// Resource information surfaced to the host when `ResourcesDiscovered` fires.
#[derive(Debug, Clone)]
pub struct McpResourceInfo {
    pub server_name: String,
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Minimal event set emitted by the MCP subsystem. The host adapts these
/// into its own subsystem-event wrapper.
#[derive(Debug, Clone)]
pub enum McpSubsystemEvent {
    ServerStateChanged {
        server_name: String,
        state: String,
        error: Option<String>,
    },
    ToolsDiscovered {
        server_name: String,
        tools: Vec<McpToolInfo>,
    },
    ResourcesDiscovered {
        server_name: String,
        resources: Vec<McpResourceInfo>,
    },
}

type EventCallback = Box<dyn Fn(McpSubsystemEvent) + Send + Sync>;

static EVENT_CALLBACK: LazyLock<SyncMutex<Option<EventCallback>>> =
    LazyLock::new(|| SyncMutex::new(None));

/// Register the host's event adapter. Replaces any previous callback.
pub fn set_event_callback<F>(cb: F)
where
    F: Fn(McpSubsystemEvent) + Send + Sync + 'static,
{
    *EVENT_CALLBACK.lock() = Some(Box::new(cb));
}

/// Emit an event through the registered callback (no-op if unset).
pub(crate) fn emit_event(event: McpSubsystemEvent) {
    if let Some(cb) = EVENT_CALLBACK.lock().as_ref() {
        cb(event);
    }
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

/// MCP server connection state.
#[derive(Debug, Clone, PartialEq)]
pub enum McpConnectionState {
    /// Not yet connected.
    Pending,
    /// Connection established and initialized.
    Connected,
    /// Disconnected (graceful or after error).
    Disconnected,
    /// Connection failed with an error.
    #[allow(dead_code)]
    Error(String),
}

// ---------------------------------------------------------------------------
// Server configuration (from settings.json)
// ---------------------------------------------------------------------------

/// MCP server configuration (from settings.json `mcpServers` key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name (the key in the mcpServers map).
    #[serde(default)]
    pub name: String,
    /// Transport type: "stdio" (default) or "sse".
    #[serde(rename = "type", default = "default_transport")]
    pub transport: String,
    /// Command to launch (for stdio transport).
    pub command: Option<String>,
    /// Command arguments (for stdio transport).
    #[serde(default)]
    pub args: Option<Vec<String>>,
    /// URL (for SSE transport).
    pub url: Option<String>,
    /// Additional HTTP headers (for SSE transport).
    pub headers: Option<HashMap<String, String>>,
    /// Environment variables to set for the subprocess.
    pub env: Option<HashMap<String, String>>,
    /// Opt-in flag: treat every tool from this server as a browser MCP tool
    /// (enables the `# Browser Automation` system-prompt section, category-aware
    /// permission prompts, and browser result rendering). When absent, the
    /// engine falls back to a tool-name heuristic. See `src/browser/detection.rs`.
    #[serde(default, rename = "browserMcp")]
    pub browser_mcp: Option<bool>,
    /// Soft-disable flag: when `Some(true)`, the discovery and manager layers
    /// skip this server at connection time. Settings-edit UX flips this via
    /// the `ToggleEnabled` IPC command without removing the entry entirely,
    /// matching the upstream "Disable / Enable" menu option for every scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

fn default_transport() -> String {
    "stdio".to_string()
}

// ---------------------------------------------------------------------------
// MCP tool / resource definitions (received from server)
// ---------------------------------------------------------------------------

/// Tool definition received from an MCP server via `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    /// Tool name.
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    #[serde(default = "default_schema", rename = "inputSchema")]
    pub input_schema: Value,
    /// Name of the server that provides this tool (set client-side).
    #[serde(default)]
    pub server_name: String,
}

fn default_schema() -> Value {
    serde_json::json!({"type": "object", "properties": {}})
}

/// Resource definition received from an MCP server via `resources/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// Resource URI.
    pub uri: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// MIME type.
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// Content returned from `resources/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    /// Text content (mutually exclusive with blob).
    pub text: Option<String>,
    /// Base64-encoded binary content.
    pub blob: Option<String>,
}

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 protocol types
// ---------------------------------------------------------------------------

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response (success or error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request.
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Value::Number(id.into()),
            method: method.to_string(),
            params,
        }
    }
}

impl JsonRpcNotification {
    /// Create a new JSON-RPC notification.
    pub fn new(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }
}

// ---------------------------------------------------------------------------
// MCP-specific request/response payloads
// ---------------------------------------------------------------------------

/// Server capabilities received during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Whether the server supports tools.
    pub tools: Option<Value>,
    /// Whether the server supports resources.
    pub resources: Option<Value>,
    /// Whether the server supports prompts.
    pub prompts: Option<Value>,
}

/// Server info received during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    #[serde(default)]
    pub version: String,
}

/// Result of the `initialize` handshake.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: ServerCapabilities,
    #[serde(default)]
    pub server_info: ServerInfo,
    pub instructions: Option<String>,
}

/// Result of `tools/list`.
#[derive(Debug, Clone, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<McpToolDef>,
}

/// Result of `tools/call`.
#[derive(Debug, Clone, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<ToolCallContent>,
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

/// Content block in a tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolCallContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "resource")]
    Resource { resource: McpResourceContent },
}

/// Result of `resources/list`.
#[derive(Debug, Clone, Deserialize)]
pub struct ListResourcesResult {
    pub resources: Vec<McpResource>,
}

/// Result of `resources/read`.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ReadResourceResult {
    pub contents: Vec<McpResourceContent>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_jsonrpc_request_serialization() {
        let req = JsonRpcRequest::new(1, "tools/list", None);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["method"], "tools/list");
        assert!(json.get("params").is_none());
    }

    #[test]
    fn test_jsonrpc_request_with_params() {
        let req = JsonRpcRequest::new(
            2,
            "tools/call",
            Some(json!({"name": "search", "arguments": {"query": "test"}})),
        );
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tools/call");
        assert_eq!(json["params"]["name"], "search");
    }

    #[test]
    fn test_jsonrpc_notification_serialization() {
        let notif = JsonRpcNotification::new("notifications/initialized", None);
        let json = serde_json::to_value(&notif).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "notifications/initialized");
        assert!(json.get("id").is_none());
    }

    #[test]
    fn test_jsonrpc_response_deserialization_success() {
        let json = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"tools": []}
        });
        let resp: JsonRpcResponse = serde_json::from_value(json).unwrap();
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_deserialization_error() {
        let json = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32600, "message": "Invalid Request"}
        });
        let resp: JsonRpcResponse = serde_json::from_value(json).unwrap();
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn test_server_config_deserialization() {
        let json = json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"],
            "env": {"HOME": "/tmp"}
        });
        let config: McpServerConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.transport, "stdio");
        assert_eq!(config.command.unwrap(), "npx");
        assert_eq!(
            config.args.unwrap(),
            vec!["-y", "@modelcontextprotocol/server-filesystem"]
        );
    }

    #[test]
    fn test_tool_def_deserialization() {
        let json = json!({
            "name": "read_file",
            "description": "Read a file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }
        });
        let tool: McpToolDef = serde_json::from_value(json).unwrap();
        assert_eq!(tool.name, "read_file");
        assert_eq!(tool.description, "Read a file");
    }

    #[test]
    fn test_call_tool_result_deserialization() {
        let json = json!({
            "content": [
                {"type": "text", "text": "file contents here"}
            ],
            "isError": false
        });
        let result: CallToolResult = serde_json::from_value(json).unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        match &result.content[0] {
            ToolCallContent::Text { text } => assert_eq!(text, "file contents here"),
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn test_initialize_result_deserialization() {
        let json = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}, "resources": {}},
            "serverInfo": {"name": "test-server", "version": "1.0"}
        });
        let result: InitializeResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.protocol_version, "2024-11-05");
        assert_eq!(result.server_info.name, "test-server");
    }

    #[test]
    fn test_resource_content_deserialization() {
        let json = json!({
            "uri": "file:///tmp/test.txt",
            "mimeType": "text/plain",
            "text": "hello world"
        });
        let content: McpResourceContent = serde_json::from_value(json).unwrap();
        assert_eq!(content.uri, "file:///tmp/test.txt");
        assert_eq!(content.text.unwrap(), "hello world");
    }
}
