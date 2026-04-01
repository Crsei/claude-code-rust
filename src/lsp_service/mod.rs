//! LSP service — manages Language Server Protocol server connections.
//!
//! Corresponds to TypeScript: internal LSP server manager used by LSPTool.
//!
//! This module provides the interface for communicating with LSP servers,
//! using `lsp-types` for proper protocol types.
//!
//! LSP servers are managed per-language (determined by file extension).
//! Each server is a subprocess communicating via stdin/stdout using JSON-RPC 2.0.

#![allow(unused)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::tools::lsp::{HoverInfo, SourceLocation, SymbolInfo};

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
            extensions: vec!["c".into(), "h".into(), "cpp".into(), "hpp".into(), "cc".into()],
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
pub enum ServerState {
    NotStarted,
    Starting,
    Running,
    Stopped,
    Error(String),
}

/// Tracked LSP server instance.
#[derive(Debug)]
struct ServerInstance {
    config: LspServerConfig,
    state: ServerState,
    request_id: u64,
}

/// Global server manager.
static SERVERS: LazyLock<Mutex<HashMap<String, ServerInstance>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// Public API (feature-gated implementations)
// ---------------------------------------------------------------------------

/// Go to definition at position.

pub async fn go_to_definition(
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Vec<SourceLocation>> {
    // TODO: Full LSP client implementation
    // This would:
    // 1. Find/start the appropriate language server
    // 2. Send textDocument/didOpen if needed
    // 3. Send textDocument/definition request
    // 4. Parse Location[] or LocationLink[] response
    // 5. Convert to SourceLocation
    bail!("LSP server connection not yet implemented — compile with full LSP client support")
}

/// Go to implementation at position.

pub async fn go_to_implementation(
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Vec<SourceLocation>> {
    bail!("LSP server connection not yet implemented")
}

/// Find references at position.

pub async fn find_references(
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Vec<SourceLocation>> {
    bail!("LSP server connection not yet implemented")
}

/// Hover at position.

pub async fn hover(uri: &str, line: u32, character: u32) -> Result<HoverInfo> {
    bail!("LSP server connection not yet implemented")
}

/// List document symbols.

pub async fn document_symbols(uri: &str) -> Result<Vec<SymbolInfo>> {
    bail!("LSP server connection not yet implemented")
}

/// Search workspace symbols.

pub async fn workspace_symbols(query: &str) -> Result<Vec<SymbolInfo>> {
    bail!("LSP server connection not yet implemented")
}

/// Prepare call hierarchy.

pub async fn prepare_call_hierarchy(
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Vec<SymbolInfo>> {
    bail!("LSP server connection not yet implemented")
}

/// Get incoming calls.

pub async fn incoming_calls(item: &SymbolInfo) -> Result<Vec<SymbolInfo>> {
    bail!("LSP server connection not yet implemented")
}

/// Get outgoing calls.

pub async fn outgoing_calls(item: &SymbolInfo) -> Result<Vec<SymbolInfo>> {
    bail!("LSP server connection not yet implemented")
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
        let states = vec![
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
