//! LSP service — manages Language Server Protocol server connections.
//!
//! Corresponds to TypeScript: internal LSP server manager used by LSPTool.
//!
//! This module provides the interface for communicating with LSP servers,
//! using `lsp-types` for proper protocol types.
//!
//! LSP servers are managed per-language (determined by file extension).
//! Each server is a subprocess communicating via stdin/stdout using JSON-RPC 2.0.

pub mod client;
pub mod conversions;
pub mod types;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub use types::{HoverInfo, SourceLocation, SymbolInfo};

pub mod transport;

// ---------------------------------------------------------------------------
// Server configuration
// ---------------------------------------------------------------------------

/// Configuration for a language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// Language ID (e.g. "rust", "typescript", "python").
    pub language_id: String,
    /// File extensions this server handles.
    pub extensions: Vec<String>,
    /// Command to launch the server.
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Additional initialization options.
    #[serde(default)]
    pub init_options: Option<serde_json::Value>,
}

/// Known default LSP server configurations.
pub fn default_server_configs() -> Vec<LspServerConfig> {
    vec![
        LspServerConfig {
            language_id: "rust".into(),
            extensions: vec!["rs".into()],
            command: "rust-analyzer".into(),
            args: vec![],
            init_options: None,
        },
        LspServerConfig {
            language_id: "typescript".into(),
            extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
            command: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            init_options: None,
        },
        LspServerConfig {
            language_id: "python".into(),
            extensions: vec!["py".into()],
            command: "pylsp".into(),
            args: vec![],
            init_options: None,
        },
        LspServerConfig {
            language_id: "go".into(),
            extensions: vec!["go".into()],
            command: "gopls".into(),
            args: vec![],
            init_options: None,
        },
        LspServerConfig {
            language_id: "c".into(),
            extensions: vec![
                "c".into(),
                "h".into(),
                "cpp".into(),
                "hpp".into(),
                "cc".into(),
            ],
            command: "clangd".into(),
            args: vec![],
            init_options: None,
        },
        LspServerConfig {
            language_id: "java".into(),
            extensions: vec!["java".into()],
            command: "jdtls".into(),
            args: vec![],
            init_options: None,
        },
    ]
}

/// Get the language ID for a file extension.
#[allow(dead_code)] // Public API — used in tests, will be used by future callers.
pub fn language_for_extension(ext: &str) -> Option<String> {
    for config in default_server_configs() {
        if config.extensions.contains(&ext.to_string()) {
            return Some(config.language_id);
        }
    }
    None
}

/// Get the server config for a file path.
pub fn config_for_file(path: &Path) -> Option<LspServerConfig> {
    let ext = path.extension()?.to_str()?;
    default_server_configs()
        .into_iter()
        .find(|c| c.extensions.contains(&ext.to_string()))
}

// ---------------------------------------------------------------------------
// Server state tracking
// ---------------------------------------------------------------------------

/// Connection state for an LSP server.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Tested + will be used for server lifecycle tracking.
pub enum ServerState {
    NotStarted,
    Starting,
    Running,
    Stopped,
    Error(String),
}

/// Global LSP client instances, keyed by language_id.
/// Uses tokio::sync::Mutex because LspClient methods are async.
static LSP_CLIENTS: LazyLock<tokio::sync::Mutex<HashMap<String, client::LspClient>>> =
    LazyLock::new(|| tokio::sync::Mutex::new(HashMap::new()));

/// Event sender for subsystem events (injected by headless event loop).
static EVENT_TX: LazyLock<
    parking_lot::Mutex<
        Option<tokio::sync::broadcast::Sender<crate::ipc::subsystem_events::SubsystemEvent>>,
    >,
> = LazyLock::new(|| parking_lot::Mutex::new(None));

/// Inject the event sender from the headless event loop.
#[allow(dead_code)] // Called by headless event loop wiring (Task 12).
pub fn set_event_sender(
    tx: tokio::sync::broadcast::Sender<crate::ipc::subsystem_events::SubsystemEvent>,
) {
    *EVENT_TX.lock() = Some(tx);
}

/// Emit a subsystem event (no-op if no sender is set).
pub(crate) fn emit_event(event: crate::ipc::subsystem_events::SubsystemEvent) {
    if let Some(tx) = EVENT_TX.lock().as_ref() {
        let _ = tx.send(event);
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Get or start an LSP client for the file at the given URI.
/// Returns the language_id key to look up the client.
async fn get_or_start_client(
    uri: &str,
    clients: &mut HashMap<String, client::LspClient>,
) -> Result<String> {
    // Extract file path from URI
    let path = uri
        .strip_prefix("file:///")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri);
    let file_path = Path::new(path);
    let config = config_for_file(file_path)
        .with_context(|| format!("No LSP server configured for: {}", path))?;

    let lang = config.language_id.clone();

    // Check if existing client is alive
    if let Some(existing) = clients.get_mut(&lang) {
        if existing.is_alive() {
            return Ok(lang);
        }
        tracing::warn!(language = %lang, "LSP server died, will restart");
        clients.remove(&lang);
        emit_event(crate::ipc::subsystem_events::SubsystemEvent::Lsp(
            crate::ipc::subsystem_events::LspEvent::ServerStateChanged {
                language_id: lang.clone(),
                state: "stopped".to_string(),
                error: Some("server process died".to_string()),
            },
        ));
    }

    // Start new client
    let root_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let new_client = client::LspClient::start(&config, &root_path).await?;
    clients.insert(lang.clone(), new_client);
    emit_event(crate::ipc::subsystem_events::SubsystemEvent::Lsp(
        crate::ipc::subsystem_events::LspEvent::ServerStateChanged {
            language_id: lang.clone(),
            state: "running".to_string(),
            error: None,
        },
    ));
    Ok(lang)
}

/// Reconstruct a `CallHierarchyItem` JSON from a [`SymbolInfo`] for call
/// hierarchy requests (incoming/outgoing calls).
///
/// The SymbolInfo's 1-based positions are converted back to 0-based LSP
/// positions.  `SymbolKind::FUNCTION` is used as the default kind because
/// we don't store the original numeric kind value.
fn symbol_info_to_call_hierarchy_json(item: &SymbolInfo) -> Result<serde_json::Value> {
    let uri = conversions::file_path_to_uri(&item.location.file_path)?;

    // Convert 1-based back to 0-based
    let start_line = item.location.line.saturating_sub(1);
    let start_char = item.location.character.saturating_sub(1);
    let end_line = item
        .location
        .end_line
        .unwrap_or(item.location.line)
        .saturating_sub(1);
    let end_char = item
        .location
        .end_character
        .unwrap_or(item.location.character)
        .saturating_sub(1);

    // SymbolKind::FUNCTION = 12 in LSP specification
    let kind_value = serde_json::to_value(lsp_types::SymbolKind::FUNCTION)
        .unwrap_or(serde_json::Value::Number(12.into()));

    Ok(serde_json::json!({
        "name": item.name,
        "kind": kind_value,
        "uri": uri.as_str(),
        "range": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": end_line, "character": end_char }
        },
        "selectionRange": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": end_line, "character": end_char }
        }
    }))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Go to definition at position.
pub async fn go_to_definition(uri: &str, line: u32, character: u32) -> Result<Vec<SourceLocation>> {
    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(uri, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();
    client.ensure_file_open(uri).await?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    });
    let response = client.request("textDocument/definition", params).await?;
    conversions::parse_location_response(response)
}

/// Go to implementation at position.
pub async fn go_to_implementation(
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Vec<SourceLocation>> {
    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(uri, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();
    client.ensure_file_open(uri).await?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    });
    let response = client
        .request("textDocument/implementation", params)
        .await?;
    conversions::parse_location_response(response)
}

/// Find references at position.
pub async fn find_references(uri: &str, line: u32, character: u32) -> Result<Vec<SourceLocation>> {
    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(uri, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();
    client.ensure_file_open(uri).await?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character },
        "context": { "includeDeclaration": true }
    });
    let response = client.request("textDocument/references", params).await?;
    conversions::parse_location_response(response)
}

/// Hover at position.
pub async fn hover(uri: &str, line: u32, character: u32) -> Result<HoverInfo> {
    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(uri, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();
    client.ensure_file_open(uri).await?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    });
    let response = client.request("textDocument/hover", params).await?;
    conversions::parse_hover_response(response)
}

/// List document symbols.
pub async fn document_symbols(uri: &str) -> Result<Vec<SymbolInfo>> {
    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(uri, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();
    client.ensure_file_open(uri).await?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri }
    });
    let response = client
        .request("textDocument/documentSymbol", params)
        .await?;
    conversions::parse_document_symbols_response(response)
}

/// Search workspace symbols.
pub async fn workspace_symbols(query: &str) -> Result<Vec<SymbolInfo>> {
    let mut clients = LSP_CLIENTS.lock().await;

    // Use any running client — workspace/symbol is not file-specific.
    let lang = {
        let mut found = None;
        for (lang, c) in clients.iter_mut() {
            if c.is_alive() {
                found = Some(lang.clone());
                break;
            }
        }
        found
    };

    let lang = lang.ok_or_else(|| anyhow::anyhow!("No LSP server running"))?;
    let client = clients.get_mut(&lang).unwrap();

    let params = serde_json::json!({
        "query": query
    });
    let response = client.request("workspace/symbol", params).await?;
    conversions::parse_workspace_symbols_response(response)
}

/// Prepare call hierarchy.
pub async fn prepare_call_hierarchy(
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Vec<SymbolInfo>> {
    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(uri, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();
    client.ensure_file_open(uri).await?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    });
    let response = client
        .request("textDocument/prepareCallHierarchy", params)
        .await?;
    conversions::parse_call_hierarchy_items(response)
}

/// Get incoming calls.
pub async fn incoming_calls(item: &SymbolInfo) -> Result<Vec<SymbolInfo>> {
    let uri_str = conversions::file_path_to_uri(&item.location.file_path)?
        .as_str()
        .to_string();

    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(&uri_str, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();

    let call_item = symbol_info_to_call_hierarchy_json(item)?;
    let params = serde_json::json!({ "item": call_item });
    let response = client
        .request("callHierarchy/incomingCalls", params)
        .await?;
    conversions::parse_incoming_calls(response)
}

/// Get outgoing calls.
pub async fn outgoing_calls(item: &SymbolInfo) -> Result<Vec<SymbolInfo>> {
    let uri_str = conversions::file_path_to_uri(&item.location.file_path)?
        .as_str()
        .to_string();

    let mut clients = LSP_CLIENTS.lock().await;
    let lang = get_or_start_client(&uri_str, &mut clients).await?;
    let client = clients.get_mut(&lang).unwrap();

    let call_item = symbol_info_to_call_hierarchy_json(item)?;
    let params = serde_json::json!({ "item": call_item });
    let response = client
        .request("callHierarchy/outgoingCalls", params)
        .await?;
    conversions::parse_outgoing_calls(response)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_for_extension() {
        assert_eq!(language_for_extension("rs"), Some("rust".to_string()));
        assert_eq!(language_for_extension("ts"), Some("typescript".to_string()));
        assert_eq!(language_for_extension("py"), Some("python".to_string()));
        assert_eq!(language_for_extension("go"), Some("go".to_string()));
        assert_eq!(language_for_extension("cpp"), Some("c".to_string()));
        assert_eq!(language_for_extension("xyz"), None);
    }

    #[test]
    fn test_config_for_file() {
        let config = config_for_file(Path::new("main.rs"));
        assert!(config.is_some());
        let c = config.unwrap();
        assert_eq!(c.language_id, "rust");
        assert_eq!(c.command, "rust-analyzer");

        let config = config_for_file(Path::new("app.tsx"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().language_id, "typescript");
    }

    #[test]
    fn test_config_for_unknown_file() {
        assert!(config_for_file(Path::new("data.csv")).is_none());
        assert!(config_for_file(Path::new("Makefile")).is_none());
    }

    #[test]
    fn test_default_configs_not_empty() {
        let configs = default_server_configs();
        assert!(configs.len() >= 6);
        for c in &configs {
            assert!(!c.language_id.is_empty());
            assert!(!c.command.is_empty());
            assert!(!c.extensions.is_empty());
        }
    }

    #[test]
    fn test_server_config_serde() {
        let config = LspServerConfig {
            language_id: "rust".into(),
            extensions: vec!["rs".into()],
            command: "rust-analyzer".into(),
            args: vec!["--log-file".into(), "/tmp/ra.log".into()],
            init_options: Some(serde_json::json!({"checkOnSave": true})),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: LspServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.language_id, "rust");
        assert_eq!(back.args.len(), 2);
        assert!(back.init_options.is_some());
    }

    #[test]
    fn test_server_state_variants() {
        let states = [
            ServerState::NotStarted,
            ServerState::Starting,
            ServerState::Running,
            ServerState::Stopped,
            ServerState::Error("timeout".into()),
        ];
        assert_eq!(states.len(), 5);
        assert_ne!(ServerState::NotStarted, ServerState::Running);
    }
}
