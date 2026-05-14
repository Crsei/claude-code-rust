//! LSP service contracts owned outside the root binary.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// Stable server key. Defaults to `language_id`; plugin/settings configs
    /// may set this to avoid collisions between servers that share a language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Language ID (e.g. "rust", "typescript", "python").
    #[serde(default, rename = "languageId", alias = "language_id")]
    pub language_id: String,
    /// File extensions this server handles.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Optional extension-to-language map used by plugin LSP configs.
    #[serde(
        default,
        rename = "extensionToLanguage",
        alias = "extension_to_language"
    )]
    pub extension_to_language: HashMap<String, String>,
    /// Command to launch the server.
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the server subprocess.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Optional workspace folder override.
    #[serde(default, rename = "workspaceFolder", alias = "workspace_folder")]
    pub workspace_folder: Option<String>,
    /// Additional initialization options.
    #[serde(
        default,
        rename = "initializationOptions",
        alias = "init_options",
        alias = "initialization_options"
    )]
    pub init_options: Option<serde_json::Value>,
    /// Human-readable source marker: default/settings/plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Known default LSP server configurations.
pub fn default_server_configs() -> Vec<LspServerConfig> {
    vec![
        builtin_config("rust", &["rs"], "rust-analyzer", &[]),
        builtin_config(
            "typescript",
            &["ts", "tsx", "js", "jsx"],
            "typescript-language-server",
            &["--stdio"],
        ),
        builtin_config("python", &["py"], "pylsp", &[]),
        builtin_config("go", &["go"], "gopls", &[]),
        builtin_config("c", &["c", "h", "cpp", "hpp", "cc"], "clangd", &[]),
        builtin_config("java", &["java"], "jdtls", &[]),
    ]
}

pub fn builtin_config(
    language_id: &str,
    extensions: &[&str],
    command: &str,
    args: &[&str],
) -> LspServerConfig {
    LspServerConfig {
        name: None,
        language_id: language_id.to_string(),
        extensions: extensions.iter().map(|s| (*s).to_string()).collect(),
        extension_to_language: HashMap::new(),
        command: command.to_string(),
        args: args.iter().map(|s| (*s).to_string()).collect(),
        env: HashMap::new(),
        workspace_folder: None,
        init_options: None,
        source: Some("default".to_string()),
    }
}
