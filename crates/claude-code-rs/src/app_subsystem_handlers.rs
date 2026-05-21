//! Subsystem command handlers and status snapshot builders.
//!
//! **Command handlers** respond to `FrontendMessage` commands for each subsystem
//! (LSP, MCP, Plugin, Skill).  Each handler returns a `Vec<BackendMessage>`
//! that the caller sends via the [`FrontendSink`].  Handlers never write to
//! stdout directly.
//!
//! **Status snapshot builders** assemble point-in-time status objects from
//! each subsystem's in-memory state.  These are used by `QueryStatus` commands
//! and the `SystemStatus` tool.

use std::future::Future;
use std::path::{Path, PathBuf};

use cc_ipc_protocol::subsystem_events::{IdeEvent, LspEvent, McpEvent, PluginEvent, SkillEvent};
use cc_ipc_protocol::subsystem_types::*;
use cc_ipc_protocol::BackendMessage;
use cc_mcp::discovery::DiscoveryScope;

// ===========================================================================
// Command handlers (return value pattern — no direct I/O)
// ===========================================================================

fn lsp_diagnostic_to_ipc(diagnostic: cc_lsp_service::LspDiagnostic) -> LspDiagnostic {
    LspDiagnostic {
        range: DiagnosticRange {
            start_line: diagnostic.range.start_line,
            start_character: diagnostic.range.start_character,
            end_line: diagnostic.range.end_line,
            end_character: diagnostic.range.end_character,
        },
        severity: diagnostic.severity,
        message: diagnostic.message,
        source: diagnostic.source,
        code: diagnostic.code,
    }
}

fn lsp_document_change_from_ipc(
    change: cc_ipc_protocol::DocumentChange,
) -> cc_lsp_service::DocumentChange {
    cc_lsp_service::DocumentChange {
        range: cc_lsp_service::SourceRange {
            start_line: change.range.start_line,
            start_character: change.range.start_character,
            end_line: change.range.end_line,
            end_character: change.range.end_character,
        },
        range_length: change.range_length,
        text: change.text,
    }
}

fn lsp_server_info_to_ipc(info: cc_lsp_service::LspServerInfo) -> LspServerInfo {
    LspServerInfo {
        language_id: info.language_id,
        state: info.state,
        extensions: info.extensions,
        open_files_count: info.open_files_count,
        error: info.error,
    }
}

/// Handle an LSP subsystem command from the frontend.
///
/// Read-only snapshots are returned immediately. Live editor operations are
/// scheduled onto the active Tokio runtime and report results through the LSP
/// subsystem event bus, so the headless loop can keep processing stdin while
/// language servers start, sync documents, or compute completions.
pub fn handle_lsp_command(
    cmd: cc_ipc_protocol::subsystem_events::LspCommand,
) -> Vec<BackendMessage> {
    use cc_ipc_protocol::subsystem_events::LspCommand;

    match cmd {
        LspCommand::StartServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP start requested via IPC");
            spawn_lsp_task("start_server", None, async move {
                cc_lsp_service::start_server(&language_id).await
            })
        }
        LspCommand::StopServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP stop requested via IPC");
            spawn_lsp_task("stop_server", None, async move {
                cc_lsp_service::stop_server(&language_id).await
            })
        }
        LspCommand::RestartServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP restart requested via IPC");
            spawn_lsp_task("restart_server", None, async move {
                cc_lsp_service::restart_server(&language_id).await
            })
        }
        LspCommand::QueryStatus => {
            let servers = build_lsp_server_info_list();
            vec![BackendMessage::LspEvent {
                event: LspEvent::ServerList { servers },
            }]
        }
        LspCommand::QueryDiagnostics { uri } => {
            let entries = cc_lsp_service::diagnostics_snapshot(uri.as_deref())
                .into_iter()
                .map(|(uri, diagnostics)| LspDiagnosticSnapshotEntry {
                    uri,
                    diagnostics: diagnostics.into_iter().map(lsp_diagnostic_to_ipc).collect(),
                })
                .collect();
            vec![BackendMessage::LspEvent {
                event: LspEvent::DiagnosticsSnapshot { entries },
            }]
        }
        LspCommand::OpenDocument {
            uri,
            language_id,
            text,
        } => spawn_lsp_task("open_document", None, async move {
            cc_lsp_service::open_document(&uri, language_id, text)
                .await
                .map(|_| ())
        }),
        LspCommand::ChangeDocument {
            uri,
            version,
            text,
            changes,
        } => spawn_lsp_task("change_document", None, async move {
            let changes = changes
                .into_iter()
                .map(lsp_document_change_from_ipc)
                .collect();
            cc_lsp_service::change_document(&uri, text, changes, version)
                .await
                .map(|_| ())
        }),
        LspCommand::SaveDocument { uri, text } => {
            spawn_lsp_task("save_document", None, async move {
                cc_lsp_service::save_document(&uri, text).await.map(|_| ())
            })
        }
        LspCommand::CloseDocument { uri } => spawn_lsp_task("close_document", None, async move {
            cc_lsp_service::close_document(&uri).await.map(|_| ())
        }),
        LspCommand::Completion {
            request_id,
            uri,
            line,
            character,
            trigger_character,
        } => {
            let request_id_for_error = request_id.clone();
            spawn_lsp_task("completion", Some(request_id_for_error), async move {
                let items = cc_lsp_service::completion(
                    &uri,
                    line.saturating_sub(1),
                    character.saturating_sub(1),
                    trigger_character,
                )
                .await?;
                cc_lsp_service::emit_event(cc_lsp_service::LspEvent::CompletionResults {
                    request_id,
                    uri,
                    items,
                });
                Ok(())
            })
        }
        LspCommand::QuerySettings => {
            let settings = load_lsp_recommendation_settings();
            vec![BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            }]
        }
        LspCommand::RecommendationResponse {
            request_id,
            plugin_name,
            decision,
        } => {
            tracing::info!(
                request_id = %request_id,
                plugin_name = %plugin_name,
                decision = %decision,
                "LSP recommendation response"
            );
            let (settings, info_text) = apply_recommendation_decision(&plugin_name, &decision);
            let mut msgs = Vec::with_capacity(3);
            msgs.push(BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            });
            // For "yes", also perform the actual plugin installation
            if decision == "yes" {
                let install_info = install_recommended_plugin(&plugin_name);
                msgs.push(BackendMessage::SystemInfo {
                    text: install_info,
                    level: "info".to_string(),
                });
            }
            if let Some(text) = info_text {
                msgs.push(BackendMessage::SystemInfo {
                    text,
                    level: "info".to_string(),
                });
            }
            msgs
        }
        LspCommand::UnmutePlugin { plugin_name } => {
            let settings = unmute_lsp_plugin(&plugin_name);
            vec![BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            }]
        }
        LspCommand::SetRecommendationsDisabled { disabled } => {
            let settings = set_lsp_recommendations_disabled(disabled);
            vec![BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            }]
        }
    }
}

fn spawn_lsp_task<F>(
    operation: &'static str,
    request_id: Option<String>,
    future: F,
) -> Vec<BackendMessage>
where
    F: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(err) => {
            return vec![BackendMessage::SystemInfo {
                text: format!("LSP {operation} could not run without an async runtime: {err}"),
                level: "error".to_string(),
            }];
        }
    };

    handle.spawn(async move {
        if let Err(err) = future.await {
            tracing::warn!(operation, error = %err, "LSP IPC command failed");
            cc_lsp_service::emit_event(cc_lsp_service::LspEvent::CommandError {
                request_id,
                message: format!("{operation}: {err}"),
            });
        }
    });

    vec![BackendMessage::SystemInfo {
        text: format!("Queued LSP {operation}."),
        level: "info".to_string(),
    }]
}

// ---------------------------------------------------------------------------
// LSP recommendation settings persistence
// ---------------------------------------------------------------------------

/// Key inside the user-level `settings.json` object that holds the
/// `LspRecommendationSettings` payload.
const LSP_RECOMMENDATIONS_KEY: &str = "lspRecommendations";

/// Read the `lspRecommendations` block from the user settings file.
///
/// Returns a `Default` value when the file is missing, the key is
/// absent, or the stored value fails to deserialize — a corrupt entry
/// should never brick the prompt pipeline.
pub fn load_lsp_recommendation_settings() -> LspRecommendationSettings {
    let path = cc_config::settings::user_settings_path();
    let Ok(value) = read_settings_value(&path) else {
        return LspRecommendationSettings::default();
    };
    value
        .get(LSP_RECOMMENDATIONS_KEY)
        .and_then(|v| serde_json::from_value::<LspRecommendationSettings>(v.clone()).ok())
        .unwrap_or_default()
}

/// Persist `settings` under the `lspRecommendations` key, preserving every
/// other field in the user settings file.
fn save_lsp_recommendation_settings(settings: &LspRecommendationSettings) {
    let path = cc_config::settings::user_settings_path();
    let mut value = read_settings_value(&path).unwrap_or_else(|err| {
        tracing::warn!(error = %err, "LSP recommendations: read user settings failed; overwriting with fresh object");
        serde_json::Value::Object(serde_json::Map::new())
    });
    if !value.is_object() {
        value = serde_json::Value::Object(serde_json::Map::new());
    }
    let encoded = match serde_json::to_value(settings) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(error = %err, "LSP recommendations: serialize failed");
            return;
        }
    };
    value
        .as_object_mut()
        .expect("value is object")
        .insert(LSP_RECOMMENDATIONS_KEY.to_string(), encoded);
    if let Err(err) = write_settings_value(&path, &value) {
        tracing::warn!(error = %err, "LSP recommendations: write user settings failed");
    }
}

/// Apply a user decision from an [`LspEvent::RecommendationRequest`] prompt.
///
/// Returns the updated settings snapshot plus an optional info-level
/// message the frontend can surface in its system log. `yes` / `no` do
/// not mutate persistent state — the install itself is carried out
/// by [`install_recommended_plugin`]; only `never` and `disable` are sticky.
fn apply_recommendation_decision(
    plugin_name: &str,
    decision: &str,
) -> (LspRecommendationSettings, Option<String>) {
    let mut settings = load_lsp_recommendation_settings();
    let info = match decision {
        "yes" => {
            // Installation is handled by install_recommended_plugin()
            None
        }
        "no" => None,
        "never" => {
            if !settings.muted_plugins.iter().any(|p| p == plugin_name) {
                settings.muted_plugins.push(plugin_name.to_string());
                save_lsp_recommendation_settings(&settings);
            }
            Some(format!(
                "Muted LSP recommendation for '{}'. Run /lsp to undo.",
                plugin_name
            ))
        }
        "disable" => {
            if !settings.disabled {
                settings.disabled = true;
                save_lsp_recommendation_settings(&settings);
            }
            Some("Disabled all LSP plugin recommendations. Run /lsp to re-enable.".to_string())
        }
        other => {
            tracing::warn!(decision = %other, "LSP recommendations: unknown decision value");
            None
        }
    };
    (settings, info)
}

/// Install a plugin in response to a LSP recommendation "yes" decision.
///
/// Uses a one-shot tokio runtime to call the async installation API.
/// Returns an info message suitable for the frontend system log.
fn install_recommended_plugin(plugin_name: &str) -> String {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            return format!(
                "Failed to create runtime for installing LSP plugin '{}': {}",
                plugin_name, e
            );
        }
    };

    let engine_version = Some(env!("CARGO_PKG_VERSION"));
    let available_plugins = std::collections::HashMap::<String, String>::new();
    let all_manifests =
        std::collections::HashMap::<String, cc_plugins::manifest::PluginManifest>::new();

    match rt.block_on(cc_plugins::installation::install_plugin(
        plugin_name,
        None,
        engine_version,
        None,
        &available_plugins,
        &all_manifests,
    )) {
        Ok(result) => format!(
            "Installed LSP plugin '{}' v{}",
            result.plugin.name, result.plugin.version
        ),
        Err(e) => format!("Failed to install LSP plugin '{}': {}", plugin_name, e),
    }
}

/// Remove `plugin_name` from the muted list and persist the result.
fn unmute_lsp_plugin(plugin_name: &str) -> LspRecommendationSettings {
    let mut settings = load_lsp_recommendation_settings();
    let before = settings.muted_plugins.len();
    settings.muted_plugins.retain(|p| p != plugin_name);
    if settings.muted_plugins.len() != before {
        save_lsp_recommendation_settings(&settings);
    }
    settings
}

/// Flip the global "disable all recommendations" switch.
fn set_lsp_recommendations_disabled(disabled: bool) -> LspRecommendationSettings {
    let mut settings = load_lsp_recommendation_settings();
    if settings.disabled != disabled {
        settings.disabled = disabled;
        save_lsp_recommendation_settings(&settings);
    }
    settings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpRuntimeOperation {
    Connect,
    Disconnect,
    Reconnect,
}

impl McpRuntimeOperation {
    fn verb(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Disconnect => "disconnect",
            Self::Reconnect => "reconnect",
        }
    }

    fn past_tense(self) -> &'static str {
        match self {
            Self::Connect => "Connected",
            Self::Disconnect => "Disconnected",
            Self::Reconnect => "Reconnected",
        }
    }
}

#[derive(Debug, Clone)]
pub struct McpRuntimeReport {
    pub server_name: String,
    pub state: String,
    pub error: Option<String>,
    pub text: String,
    pub level: String,
}

/// Handle an MCP subsystem command from the frontend.
///
/// Lifecycle operations (`ConnectServer` / `DisconnectServer` /
/// `ReconnectServer`) surface an info-level system message pointing at the
/// `/mcp` slash command; the actual live-state mutation lives in the
/// manager owned by the query engine.
///
/// `QueryStatus` builds a runtime-state list; `QueryConfig`/`UpsertConfig`/
/// `RemoveConfig` implement the scope-aware config editor (issue #44).
pub fn handle_mcp_command(
    cmd: cc_ipc_protocol::subsystem_events::McpCommand,
) -> Vec<BackendMessage> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    handle_mcp_command_at_cwd(cmd, &cwd)
}

pub async fn handle_mcp_command_with_runtime(
    cmd: cc_ipc_protocol::subsystem_events::McpCommand,
    cwd: &Path,
) -> Vec<BackendMessage> {
    use cc_ipc_protocol::subsystem_events::McpCommand;

    match cmd {
        McpCommand::ConnectServer { server_name } => {
            let report =
                run_mcp_runtime_operation(cwd, McpRuntimeOperation::Connect, &server_name).await;
            mcp_runtime_report_messages(report)
        }
        McpCommand::DisconnectServer { server_name } => {
            let report =
                run_mcp_runtime_operation(cwd, McpRuntimeOperation::Disconnect, &server_name).await;
            mcp_runtime_report_messages(report)
        }
        McpCommand::ReconnectServer { server_name } => {
            let report =
                run_mcp_runtime_operation(cwd, McpRuntimeOperation::Reconnect, &server_name).await;
            mcp_runtime_report_messages(report)
        }
        McpCommand::QueryStatus => {
            let servers = build_mcp_server_info_list_for_cwd_async(cwd).await;
            vec![BackendMessage::McpEvent {
                event: McpEvent::ServerList { servers },
            }]
        }
        McpCommand::StartAuth { server_name } => start_mcp_auth(cwd, &server_name).await,
        McpCommand::CompleteAuth {
            server_name,
            code,
            state,
        } => complete_mcp_auth(cwd, &server_name, &code, state.as_deref()).await,
        other => handle_mcp_command_at_cwd(other, cwd),
    }
}

pub async fn run_mcp_runtime_operation(
    cwd: &Path,
    operation: McpRuntimeOperation,
    server_name: &str,
) -> McpRuntimeReport {
    tracing::info!(
        server_name = %server_name,
        operation = operation.verb(),
        "MCP runtime command requested"
    );

    let Some(manager) = cc_mcp::runtime::current_manager() else {
        return mcp_runtime_report(
            server_name,
            "error",
            Some("MCP runtime manager is not available".to_string()),
            format!(
                "Cannot {} MCP server `{}` because the runtime manager is not available.",
                operation.verb(),
                server_name
            ),
            "error",
        );
    };

    if operation == McpRuntimeOperation::Disconnect {
        let had_client = {
            let mut manager = manager.lock().await;
            manager.disconnect_server(server_name).await
        };
        let text = if had_client {
            format!("Disconnected MCP server `{}`.", server_name)
        } else {
            format!(
                "MCP server `{}` had no active connection; marked disconnected.",
                server_name
            )
        };
        return mcp_runtime_report(server_name, "disconnected", None, text, "info");
    }

    let config = match find_mcp_runtime_config(cwd, server_name) {
        Ok(config) => config,
        Err(message) => {
            return mcp_runtime_report(
                server_name,
                "error",
                Some(message.clone()),
                message,
                "error",
            );
        }
    };
    let disabled = config.disabled.unwrap_or(false);

    let result = {
        let mut manager = manager.lock().await;
        match operation {
            McpRuntimeOperation::Connect => manager.connect_server(config).await,
            McpRuntimeOperation::Reconnect => manager.reconnect_server(config).await,
            McpRuntimeOperation::Disconnect => unreachable!("disconnect handled above"),
        }
    };

    match result {
        Ok(()) if disabled => mcp_runtime_report(
            server_name,
            "disabled",
            None,
            format!(
                "MCP server `{}` is disabled in settings; no live client was kept.",
                server_name
            ),
            "info",
        ),
        Ok(()) => mcp_runtime_report(
            server_name,
            "connected",
            None,
            format!("{} MCP server `{}`.", operation.past_tense(), server_name),
            "info",
        ),
        Err(err) => {
            let state = if cc_mcp::client::is_auth_needed_error(&err) {
                "auth-needed"
            } else {
                "error"
            };
            let message = format!(
                "Failed to {} MCP server `{}`: {}",
                operation.verb(),
                server_name,
                err
            );
            mcp_runtime_report(server_name, state, Some(err.to_string()), message, "error")
        }
    }
}

fn handle_mcp_command_at_cwd(
    cmd: cc_ipc_protocol::subsystem_events::McpCommand,
    cwd: &Path,
) -> Vec<BackendMessage> {
    use cc_ipc_protocol::subsystem_events::McpCommand;

    match cmd {
        McpCommand::ConnectServer { server_name } => mcp_runtime_report_messages(
            mcp_runtime_unavailable_report(McpRuntimeOperation::Connect, &server_name),
        ),
        McpCommand::DisconnectServer { server_name } => mcp_runtime_report_messages(
            mcp_runtime_unavailable_report(McpRuntimeOperation::Disconnect, &server_name),
        ),
        McpCommand::ReconnectServer { server_name } => {
            tracing::info!(server_name = %server_name, "MCP reconnect requested via IPC");
            // Emit a live `connecting` state plus a system note. The runtime
            // still drives reconnection on the next connection pass — the UI
            // reflects intent immediately so the user sees feedback instead of
            // a stale status row.
            let mut messages = Vec::with_capacity(2);
            messages.push(BackendMessage::McpEvent {
                event: McpEvent::ServerStateChanged {
                    server_name: server_name.clone(),
                    state: "pending".to_string(),
                    error: None,
                },
            });
            messages.push(BackendMessage::SystemInfo {
                text: format!(
                    "Queued reconnect for MCP server `{}`. The active session will cycle its connection.",
                    server_name
                ),
                level: "info".to_string(),
            });
            messages
        }
        McpCommand::QueryStatus => {
            let servers = build_mcp_server_info_list_for_cwd(cwd);
            vec![BackendMessage::McpEvent {
                event: McpEvent::ServerList { servers },
            }]
        }
        McpCommand::QueryConfig => {
            let entries = build_mcp_server_config_entries(cwd);
            vec![BackendMessage::McpEvent {
                event: McpEvent::ConfigList { entries },
            }]
        }
        McpCommand::UpsertConfig { entry } => match upsert_mcp_entry(cwd, *entry) {
            Ok(updated) => vec![BackendMessage::McpEvent {
                event: McpEvent::ConfigChanged {
                    server_name: updated.name.clone(),
                    entry: Some(Box::new(updated)),
                },
            }],
            Err((server_name, message)) => {
                tracing::warn!(
                    server = %server_name,
                    error = %message,
                    "MCP: upsert_config rejected"
                );
                vec![BackendMessage::McpEvent {
                    event: McpEvent::ConfigError {
                        server_name,
                        error: message,
                    },
                }]
            }
        },
        McpCommand::RemoveConfig { server_name, scope } => {
            match remove_mcp_entry(cwd, &server_name, &scope) {
                Ok(()) => vec![BackendMessage::McpEvent {
                    event: McpEvent::ConfigChanged {
                        server_name,
                        entry: None,
                    },
                }],
                Err(message) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %message,
                        "MCP: remove_config rejected"
                    );
                    vec![BackendMessage::McpEvent {
                        event: McpEvent::ConfigError {
                            server_name,
                            error: message,
                        },
                    }]
                }
            }
        }
        McpCommand::ToggleEnabled { server_name, scope } => {
            match toggle_mcp_entry_enabled(cwd, &server_name, scope.as_ref()) {
                Ok(updated) => {
                    // Sync runtime state — flipping `disabled` immediately
                    // surfaces in the MCP status list so the UI reflects the
                    // new state without waiting for a full re-discovery.
                    let now_disabled = updated.disabled.unwrap_or(false);
                    let next_state = if now_disabled { "disabled" } else { "pending" };
                    vec![
                        BackendMessage::McpEvent {
                            event: McpEvent::ConfigChanged {
                                server_name: updated.name.clone(),
                                entry: Some(Box::new(updated.clone())),
                            },
                        },
                        BackendMessage::McpEvent {
                            event: McpEvent::ServerStateChanged {
                                server_name: updated.name.clone(),
                                state: next_state.to_string(),
                                error: None,
                            },
                        },
                    ]
                }
                Err(message) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %message,
                        "MCP: toggle_enabled rejected"
                    );
                    vec![BackendMessage::McpEvent {
                        event: McpEvent::ConfigError {
                            server_name,
                            error: message,
                        },
                    }]
                }
            }
        }
        McpCommand::ClearAuth { server_name } => clear_mcp_auth(cwd, &server_name),
        McpCommand::QueryAuth { server_name } => query_mcp_auth(cwd, &server_name),
        McpCommand::StartAuth { server_name } | McpCommand::CompleteAuth { server_name, .. } => {
            vec![BackendMessage::McpEvent {
                event: McpEvent::ConfigError {
                    server_name,
                    error: "MCP OAuth command requires the async runtime handler".to_string(),
                },
            }]
        }
    }
}

async fn start_mcp_auth(cwd: &Path, server_name: &str) -> Vec<BackendMessage> {
    let config = match find_mcp_runtime_config(cwd, server_name) {
        Ok(config) => config,
        Err(message) => return vec![mcp_config_error_message(server_name, message)],
    };
    match cc_mcp::auth::start_authorization(&config).await {
        Ok(start) => vec![
            BackendMessage::McpEvent {
                event: McpEvent::AuthStarted {
                    server_name: server_name.to_string(),
                    authorization_url: start.authorization_url,
                    state: start.state,
                    redirect_uri: start.redirect_uri,
                    token_store_path: start.token_store_path.display().to_string(),
                },
            },
            BackendMessage::SystemInfo {
                text: format!(
                    "OAuth authorization started for MCP server `{}`. Complete it with the returned code.",
                    server_name
                ),
                level: "info".to_string(),
            },
        ],
        Err(err) => vec![mcp_config_error_message(server_name, err.to_string())],
    }
}

async fn complete_mcp_auth(
    cwd: &Path,
    server_name: &str,
    code: &str,
    state: Option<&str>,
) -> Vec<BackendMessage> {
    let config = match find_mcp_runtime_config(cwd, server_name) {
        Ok(config) => config,
        Err(message) => return vec![mcp_config_error_message(server_name, message)],
    };
    match cc_mcp::auth::complete_authorization(&config, code, state).await {
        Ok(_) => {
            let mut messages = query_mcp_auth(cwd, server_name);
            messages.push(BackendMessage::SystemInfo {
                text: format!(
                    "Stored OAuth credentials for MCP server `{}` in {}.",
                    server_name,
                    cc_mcp::auth::token_store_path().display()
                ),
                level: "info".to_string(),
            });
            messages
        }
        Err(err) => vec![mcp_config_error_message(server_name, err.to_string())],
    }
}

fn clear_mcp_auth(cwd: &Path, server_name: &str) -> Vec<BackendMessage> {
    let config = match find_mcp_runtime_config(cwd, server_name) {
        Ok(config) => config,
        Err(message) => return vec![mcp_config_error_message(server_name, message)],
    };
    match cc_mcp::auth::clear_stored_token(&config) {
        Ok(_) => query_mcp_auth(cwd, server_name),
        Err(err) => vec![mcp_config_error_message(server_name, err.to_string())],
    }
}

fn query_mcp_auth(cwd: &Path, server_name: &str) -> Vec<BackendMessage> {
    let config = match find_mcp_runtime_config(cwd, server_name) {
        Ok(config) => config,
        Err(message) => return vec![mcp_config_error_message(server_name, message)],
    };
    match cc_mcp::auth::credential_status(&config) {
        Ok(status) => vec![BackendMessage::McpEvent {
            event: McpEvent::AuthStatus {
                server_name: server_name.to_string(),
                configured: status.configured,
                authorized: status.authorized,
                expired: status.expired,
                can_refresh: status.can_refresh,
                token_store_path: status.token_store_path.display().to_string(),
            },
        }],
        Err(err) => vec![mcp_config_error_message(server_name, err.to_string())],
    }
}

fn mcp_config_error_message(server_name: &str, error: String) -> BackendMessage {
    BackendMessage::McpEvent {
        event: McpEvent::ConfigError {
            server_name: server_name.to_string(),
            error,
        },
    }
}

fn mcp_runtime_unavailable_report(
    operation: McpRuntimeOperation,
    server_name: &str,
) -> McpRuntimeReport {
    mcp_runtime_report(
        server_name,
        "error",
        Some("MCP runtime manager is not available".to_string()),
        format!(
            "Cannot {} MCP server `{}` because the live runtime manager is not available.",
            operation.verb(),
            server_name
        ),
        "error",
    )
}

fn mcp_runtime_report(
    server_name: &str,
    state: &str,
    error: Option<String>,
    text: String,
    level: &str,
) -> McpRuntimeReport {
    cc_mcp::runtime::record_server_state(server_name, state, error.clone());
    McpRuntimeReport {
        server_name: server_name.to_string(),
        state: state.to_string(),
        error,
        text,
        level: level.to_string(),
    }
}

fn mcp_runtime_report_messages(report: McpRuntimeReport) -> Vec<BackendMessage> {
    vec![
        BackendMessage::McpEvent {
            event: McpEvent::ServerStateChanged {
                server_name: report.server_name,
                state: report.state,
                error: report.error,
            },
        },
        BackendMessage::SystemInfo {
            text: report.text,
            level: report.level,
        },
    ]
}

fn find_mcp_runtime_config(
    cwd: &Path,
    server_name: &str,
) -> Result<cc_mcp::McpServerConfig, String> {
    let configs = cc_mcp::discovery::discover_mcp_servers(cwd)
        .map_err(|err| format!("Failed to discover MCP servers: {err}"))?;
    configs
        .into_iter()
        .find(|cfg| cfg.name == server_name)
        .ok_or_else(|| {
            format!(
                "No MCP server named `{}` was found in the current discovery scope.",
                server_name
            )
        })
}

/// Handle a plugin subsystem command from the frontend.
///
/// Enable/disable are deferred to the `/plugin` slash command.
/// `QueryStatus` returns the full plugin list.
pub fn handle_plugin_command(
    cmd: cc_ipc_protocol::subsystem_events::PluginCommand,
) -> Vec<BackendMessage> {
    use cc_ipc_protocol::subsystem_events::PluginCommand;

    match cmd {
        PluginCommand::Enable { plugin_id } => {
            tracing::info!(plugin_id = %plugin_id, "Plugin enable requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /plugin to manage plugins. To enable {}, run: /plugin enable {}",
                    plugin_id, plugin_id
                ),
                level: "info".to_string(),
            }]
        }
        PluginCommand::Disable { plugin_id } => {
            tracing::info!(plugin_id = %plugin_id, "Plugin disable requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /plugin to manage plugins. To disable {}, run: /plugin disable {}",
                    plugin_id, plugin_id
                ),
                level: "info".to_string(),
            }]
        }
        PluginCommand::QueryStatus => {
            let plugins = build_plugin_info_list();
            vec![BackendMessage::PluginEvent {
                event: PluginEvent::PluginList { plugins },
            }]
        }
        PluginCommand::Reload => {
            tracing::info!("Plugin reload requested via IPC");
            let report = cc_plugins::reload_plugins();
            vec![BackendMessage::PluginEvent {
                event: PluginEvent::Reloaded {
                    count: report.count,
                    had_error: report.had_error(),
                },
            }]
        }
        PluginCommand::Uninstall {
            plugin_id,
            purge_cache,
        } => {
            tracing::info!(
                plugin_id = %plugin_id,
                purge_cache,
                "Plugin uninstall requested via IPC"
            );
            match cc_plugins::uninstall_plugin(&plugin_id, purge_cache) {
                Ok(Some(entry)) => vec![BackendMessage::PluginEvent {
                    event: PluginEvent::StatusChanged {
                        plugin_id: entry.id.clone(),
                        name: entry.name.clone(),
                        status: "not_installed".to_string(),
                        error: None,
                    },
                }],
                Ok(None) => vec![BackendMessage::SystemInfo {
                    text: format!("Plugin '{}' is not installed.", plugin_id),
                    level: "warn".to_string(),
                }],
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("Failed to uninstall '{}': {}", plugin_id, e),
                    level: "error".to_string(),
                }],
            }
        }
    }
}

/// Handle an IDE subsystem command from the frontend (issue #41).
///
/// - `Detect` / `QueryStatus` re-run detection and return the current list.
/// - `Select` / `Clear` persist the user's selection through `crate::ide`.
/// - `Reconnect` re-triggers a `ConnectionStateChanged` event so the MCP
///   manager notices the selection on its next discovery pass.
pub fn handle_ide_command(
    cmd: cc_ipc_protocol::subsystem_events::IdeCommand,
) -> Vec<BackendMessage> {
    use cc_ipc_protocol::subsystem_events::IdeCommand;

    match cmd {
        IdeCommand::Detect | IdeCommand::QueryStatus => {
            let ides = build_ide_info_list();
            vec![BackendMessage::IdeEvent {
                event: IdeEvent::IdeList { ides },
            }]
        }
        IdeCommand::Select { ide_id } => {
            tracing::info!(ide_id = %ide_id, "IDE select requested via IPC");
            match cc_lsp_service::ide::select_ide(&ide_id) {
                Ok(()) => {
                    let ides = build_ide_info_list();
                    vec![BackendMessage::IdeEvent {
                        event: IdeEvent::IdeList { ides },
                    }]
                }
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("IDE select failed: {}", e),
                    level: "error".to_string(),
                }],
            }
        }
        IdeCommand::Clear => {
            tracing::info!("IDE selection clear requested via IPC");
            match cc_lsp_service::ide::clear_selection() {
                Ok(()) => {
                    let ides = build_ide_info_list();
                    vec![BackendMessage::IdeEvent {
                        event: IdeEvent::IdeList { ides },
                    }]
                }
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("IDE clear failed: {}", e),
                    level: "error".to_string(),
                }],
            }
        }
        IdeCommand::Reconnect => {
            tracing::info!("IDE reconnect requested via IPC");
            match cc_lsp_service::ide::reconnect_selected() {
                Ok(()) => vec![BackendMessage::SystemInfo {
                    text: "IDE reconnect scheduled".to_string(),
                    level: "info".to_string(),
                }],
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("IDE reconnect failed: {}", e),
                    level: "error".to_string(),
                }],
            }
        }
    }
}

/// Handle a skill subsystem command from the frontend.
///
/// `Reload` clears and re-initialises the skill registry.
/// `QueryStatus` returns the full skill list.
pub fn handle_skill_command(
    cmd: cc_ipc_protocol::subsystem_events::SkillCommand,
) -> Vec<BackendMessage> {
    use cc_ipc_protocol::subsystem_events::SkillCommand;

    match cmd {
        SkillCommand::Reload => {
            let cwd = std::env::current_dir().ok();
            let plugin_skills = discover_plugin_skills_for_handlers();
            let report = cc_skills::reload_skills_with_extra(
                &cc_config::paths::skills_dir_global(),
                cwd.as_deref(),
                plugin_skills,
                cc_skills::SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
            );
            tracing::info!(
                count = report.loaded,
                skipped = report.skipped,
                revision = report.revision,
                errors = report.error_count(),
                warnings = report.warning_count(),
                "Skills reloaded via IPC"
            );
            let mut messages = vec![BackendMessage::SkillEvent {
                event: SkillEvent::SkillsLoaded {
                    count: report.loaded,
                },
            }];
            if report.error_count() > 0 || report.warning_count() > 0 {
                messages.push(BackendMessage::SystemInfo {
                    text: format!(
                        "Skill reload completed with {} warning(s), {} error(s).",
                        report.warning_count(),
                        report.error_count()
                    ),
                    level: if report.error_count() > 0 {
                        "warn".to_string()
                    } else {
                        "info".to_string()
                    },
                });
            }
            messages
        }
        SkillCommand::QueryStatus => {
            let skills = build_skill_info_list();
            vec![BackendMessage::SkillEvent {
                event: SkillEvent::SkillList { skills },
            }]
        }
    }
}

// ===========================================================================
// Status snapshot builders
// ===========================================================================

/// Build a list of LSP server info from the default server configurations.
pub fn build_lsp_server_info_list() -> Vec<LspServerInfo> {
    cc_lsp_service::server_info_snapshot()
        .into_iter()
        .map(lsp_server_info_to_ipc)
        .collect()
}

/// Build a list of MCP server status info from discovered configurations.
pub fn build_mcp_server_info_list() -> Vec<McpServerStatusInfo> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    build_mcp_server_info_list_for_cwd(&cwd)
}

pub fn build_mcp_server_info_list_for_cwd(cwd: &Path) -> Vec<McpServerStatusInfo> {
    let (configs, mut diagnostics) = match discover_mcp_runtime_configs_with_diagnostics(cwd) {
        Ok(discovery) => discovery,
        Err(err) => return vec![mcp_discovery_error_status(err)],
    };
    if let Some(manager) = cc_mcp::runtime::current_manager() {
        if let Ok(manager) = manager.try_lock() {
            let mut rows = build_mcp_server_info_list_from_configs(configs, Some(&manager));
            rows.append(&mut diagnostics);
            return rows;
        }
    }

    let mut rows = build_mcp_server_info_list_from_configs(configs, None);
    rows.append(&mut diagnostics);
    rows
}

pub async fn build_mcp_server_info_list_for_cwd_async(cwd: &Path) -> Vec<McpServerStatusInfo> {
    let (configs, mut diagnostics) = match discover_mcp_runtime_configs_with_diagnostics(cwd) {
        Ok(discovery) => discovery,
        Err(err) => return vec![mcp_discovery_error_status(err)],
    };
    if let Some(manager) = cc_mcp::runtime::current_manager() {
        let manager = manager.lock().await;
        let mut rows = build_mcp_server_info_list_from_configs(configs, Some(&manager));
        rows.append(&mut diagnostics);
        return rows;
    }

    let mut rows = build_mcp_server_info_list_from_configs(configs, None);
    rows.append(&mut diagnostics);
    rows
}

fn mcp_discovery_error_status(err: anyhow::Error) -> McpServerStatusInfo {
    McpServerStatusInfo {
        name: "discovery".to_string(),
        state: "error".to_string(),
        transport: "settings".to_string(),
        tools_count: 0,
        resources_count: 0,
        server_info: None,
        instructions: None,
        error: Some(format!("Failed to discover MCP servers: {err:#}")),
    }
}

fn discover_mcp_runtime_configs_with_diagnostics(
    cwd: &Path,
) -> anyhow::Result<(Vec<cc_mcp::McpServerConfig>, Vec<McpServerStatusInfo>)> {
    let scoped = cc_mcp::discovery::discover_mcp_servers_scoped(cwd)?;
    let mut configs: Vec<cc_mcp::McpServerConfig> = Vec::new();
    let mut diagnostics = Vec::new();

    for entry in scoped {
        if let Some(error) = entry.error {
            diagnostics.push(McpServerStatusInfo {
                name: entry.config.name,
                state: "error".to_string(),
                transport: entry.config.transport,
                tools_count: 0,
                resources_count: 0,
                server_info: None,
                instructions: None,
                error: Some(format!(
                    "{} scope: {}",
                    scope_from_discovery(&entry.scope).label(),
                    error
                )),
            });
            continue;
        }

        if let Some(existing) = configs
            .iter_mut()
            .find(|config| config.name == entry.config.name)
        {
            *existing = entry.config;
        } else {
            configs.push(entry.config);
        }
    }

    Ok((configs, diagnostics))
}

fn build_mcp_server_info_list_from_configs(
    configs: Vec<cc_mcp::McpServerConfig>,
    manager: Option<&cc_mcp::manager::McpManager>,
) -> Vec<McpServerStatusInfo> {
    configs
        .into_iter()
        .map(|cfg| build_mcp_server_info(cfg, manager))
        .collect()
}

fn build_mcp_server_info(
    cfg: cc_mcp::McpServerConfig,
    manager: Option<&cc_mcp::manager::McpManager>,
) -> McpServerStatusInfo {
    if cfg.disabled.unwrap_or(false) {
        return McpServerStatusInfo {
            name: cfg.name,
            state: "disabled".to_string(),
            transport: cfg.transport,
            tools_count: 0,
            resources_count: 0,
            server_info: None,
            instructions: None,
            error: None,
        };
    }

    if let Some(client) = manager.and_then(|manager| manager.clients.get(&cfg.name)) {
        let (state, error) = match &client.state {
            cc_mcp::McpConnectionState::Pending => ("pending".to_string(), None),
            cc_mcp::McpConnectionState::Connected => ("connected".to_string(), None),
            cc_mcp::McpConnectionState::Disconnected => ("disconnected".to_string(), None),
            cc_mcp::McpConnectionState::Error(error) => ("error".to_string(), Some(error.clone())),
        };
        let server_info = (!client.server_info.name.is_empty()).then(|| McpServerInfoBrief {
            name: client.server_info.name.clone(),
            version: client.server_info.version.clone(),
        });
        return McpServerStatusInfo {
            name: cfg.name,
            state,
            transport: cfg.transport,
            tools_count: client.tools.len(),
            resources_count: client.resources.len(),
            server_info,
            instructions: client.instructions.clone(),
            error,
        };
    }

    let remembered = cc_mcp::runtime::server_state(&cfg.name);
    McpServerStatusInfo {
        name: cfg.name,
        state: remembered
            .as_ref()
            .map(|state| state.state.clone())
            .unwrap_or_else(|| "pending".to_string()),
        transport: cfg.transport,
        tools_count: 0,
        resources_count: 0,
        server_info: None,
        instructions: None,
        error: remembered.and_then(|state| state.error),
    }
}

/// Build a list of editable config entries (issue #44) from scope-aware
/// discovery. Unlike [`build_mcp_server_info_list`] this preserves one row
/// per scope so the same logical server can appear in multiple scopes (e.g.
/// "same name in user + project").
pub fn build_mcp_server_config_entries(cwd: &std::path::Path) -> Vec<McpServerConfigEntry> {
    let scoped = match cc_mcp::discovery::discover_mcp_servers_scoped(cwd) {
        Ok(scoped) => scoped,
        Err(err) => {
            tracing::warn!(error = %err, "Failed to discover scoped MCP server configs");
            return vec![McpServerConfigEntry {
                name: "discovery".to_string(),
                transport: "settings".to_string(),
                command: None,
                args: None,
                url: None,
                headers: None,
                oauth: None,
                env: None,
                browser_mcp: None,
                disabled: Some(true),
                scope: ConfigScope::User,
            }];
        }
    };
    scoped
        .into_iter()
        .map(|s| McpServerConfigEntry {
            name: s.config.name,
            scope: scope_from_discovery(&s.scope),
            transport: s.config.transport,
            command: s.config.command,
            args: s.config.args,
            url: s.config.url,
            headers: s.config.headers,
            oauth: s.config.oauth,
            env: s.config.env,
            browser_mcp: s.config.browser_mcp,
            disabled: s.config.disabled,
        })
        .collect()
}

/// Map the discovery-layer `DiscoveryScope` onto the IPC `ConfigScope`.
fn scope_from_discovery(scope: &DiscoveryScope) -> ConfigScope {
    match scope {
        DiscoveryScope::User => ConfigScope::User,
        DiscoveryScope::Project => ConfigScope::Project,
        DiscoveryScope::Plugin(id) => ConfigScope::Plugin { id: id.clone() },
        DiscoveryScope::Ide(id) => ConfigScope::Ide { id: id.clone() },
    }
}

// ---------------------------------------------------------------------------
// MCP config persistence (issue #44)
// ---------------------------------------------------------------------------

/// Resolve the `settings.json` path for an editable scope.
///
/// Returns `Err` when the scope is read-only (plugin / IDE).
///
/// We intentionally **don't** walk ancestors for `Project`: the scoped
/// discovery layer reads exactly `{cwd}/.cc-rust/settings.json`, so any
/// write must land in the same place or the round-trip breaks. Callers
/// that really want the ancestor-walking behaviour should stabilize their
/// project root before invoking this.
fn settings_path_for_scope(cwd: &std::path::Path, scope: &ConfigScope) -> Result<PathBuf, String> {
    match scope {
        ConfigScope::User => Ok(cc_config::settings::user_settings_path()),
        ConfigScope::Project => Ok(cwd.join(".cc-rust").join("settings.json")),
        ConfigScope::Plugin { id } => Err(format!(
            "scope `plugin:{}` is read-only — edit the plugin manifest instead",
            id
        )),
        ConfigScope::Ide { id } => Err(format!(
            "scope `ide:{}` is read-only — edit the IDE bridge config instead",
            id
        )),
    }
}

/// Read the raw settings file (returning defaults if missing).
fn read_settings_value(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    if content.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(&content).map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

/// Write a raw settings value with parent-dir creation + atomic rename.
fn write_settings_value(path: &std::path::Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {}", parent.display(), e))?;
    }
    let pretty = serde_json::to_string_pretty(value)
        .map_err(|e| format!("failed to serialize settings: {}", e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, pretty)
        .map_err(|e| format!("failed to write {}: {}", tmp.display(), e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        format!(
            "failed to rename {} -> {}: {}",
            tmp.display(),
            path.display(),
            e
        )
    })?;
    Ok(())
}

/// Upsert a server config into the settings file backing `entry.scope`.
///
/// Returns the entry that was persisted on success, or `(server_name, message)`
/// on failure (so the caller can emit `McpEvent::ConfigError`).
fn upsert_mcp_entry(
    cwd: &std::path::Path,
    entry: McpServerConfigEntry,
) -> Result<McpServerConfigEntry, (String, String)> {
    if !entry.scope.is_editable() {
        return Err((
            entry.name.clone(),
            format!(
                "scope `{}` is read-only — cannot upsert MCP server config",
                entry.scope.label()
            ),
        ));
    }

    let path = settings_path_for_scope(cwd, &entry.scope).map_err(|e| (entry.name.clone(), e))?;

    let mut settings = read_settings_value(&path).map_err(|e| (entry.name.clone(), e))?;
    if !settings.is_object() {
        return Err((
            entry.name.clone(),
            format!("{} is not a JSON object", path.display()),
        ));
    }

    let obj = settings.as_object_mut().unwrap();
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !servers.is_object() {
        return Err((
            entry.name.clone(),
            format!("{} has a non-object `mcpServers` field", path.display()),
        ));
    }
    let servers_obj = servers.as_object_mut().unwrap();
    servers_obj.insert(entry.name.clone(), entry_to_settings_value(&entry));

    write_settings_value(&path, &settings).map_err(|e| (entry.name.clone(), e))?;
    Ok(entry)
}

/// Remove a server config entry from the settings file backing `scope`.
fn remove_mcp_entry(
    cwd: &std::path::Path,
    server_name: &str,
    scope: &ConfigScope,
) -> Result<(), String> {
    if !scope.is_editable() {
        return Err(format!(
            "scope `{}` is read-only — cannot remove MCP server config",
            scope.label()
        ));
    }
    let path = settings_path_for_scope(cwd, scope)?;
    if !path.exists() {
        return Err(format!(
            "no settings file at {} — nothing to remove",
            path.display()
        ));
    }
    let mut settings = read_settings_value(&path)?;
    let Some(obj) = settings.as_object_mut() else {
        return Err(format!("{} is not a JSON object", path.display()));
    };
    let Some(servers) = obj.get_mut("mcpServers") else {
        return Err(format!("{} has no `mcpServers` section", path.display()));
    };
    let Some(servers_obj) = servers.as_object_mut() else {
        return Err(format!(
            "{} has a non-object `mcpServers` field",
            path.display()
        ));
    };
    if servers_obj.remove(server_name).is_none() {
        return Err(format!(
            "{} has no MCP server named `{}`",
            path.display(),
            server_name
        ));
    }
    write_settings_value(&path, &settings)?;
    Ok(())
}

/// Flip the `disabled` flag on an existing entry.
///
/// Locate a matching editable entry (`scope`-aware when provided, otherwise
/// Project > User > plugin/ide rejected), toggle its `disabled` bit, persist,
/// and return the updated entry. The returned value always has an explicit
/// `Some(bool)` for `disabled` so the caller can emit a meaningful state.
fn toggle_mcp_entry_enabled(
    cwd: &std::path::Path,
    server_name: &str,
    scope: Option<&ConfigScope>,
) -> Result<McpServerConfigEntry, String> {
    let existing = build_mcp_server_config_entries(cwd);
    let matches: Vec<&McpServerConfigEntry> = existing
        .iter()
        .filter(|e| {
            e.name == server_name
                && match scope {
                    Some(wanted) => e.scope == *wanted,
                    None => true,
                }
        })
        .collect();
    if matches.is_empty() {
        return Err(format!(
            "no MCP server named `{}`{} found",
            server_name,
            scope
                .map(|s| format!(" in scope `{}`", s.label()))
                .unwrap_or_default()
        ));
    }
    if matches.len() > 1 && scope.is_none() {
        let labels: Vec<String> = matches.iter().map(|e| e.scope.label()).collect();
        return Err(format!(
            "`{}` exists in multiple scopes ({}). Retry with an explicit scope.",
            server_name,
            labels.join(", ")
        ));
    }

    // Prefer the most specific editable match: project > user > plugin/ide.
    let target = matches
        .iter()
        .find(|e| e.scope == ConfigScope::Project)
        .or_else(|| matches.iter().find(|e| e.scope == ConfigScope::User))
        .or_else(|| matches.first())
        .cloned()
        .cloned();
    let Some(mut target) = target else {
        return Err(format!("no editable match for `{}`", server_name));
    };
    if !target.scope.is_editable() {
        return Err(format!(
            "scope `{}` is read-only — cannot toggle MCP server `{}`",
            target.scope.label(),
            server_name
        ));
    }

    let was_disabled = target.disabled.unwrap_or(false);
    target.disabled = Some(!was_disabled);
    upsert_mcp_entry(cwd, target).map_err(|(_, msg)| msg)
}

/// Serialize an entry for the on-disk `mcpServers[name]` value.
///
/// The settings file uses the legacy `McpServerConfig` shape (transport under
/// `type`, `command`/`args`/`url`/-. Consumers using different shapes can
/// still round-trip thanks to `McpServerConfig`'s permissive deserializer.
fn entry_to_settings_value(entry: &McpServerConfigEntry) -> serde_json::Value {
    let cfg = cc_mcp::McpServerConfig {
        name: entry.name.clone(),
        transport: entry.transport.clone(),
        command: entry.command.clone(),
        args: entry.args.clone(),
        url: entry.url.clone(),
        headers: entry.headers.clone(),
        oauth: entry.oauth.clone(),
        env: entry.env.clone(),
        browser_mcp: entry.browser_mcp,
        disabled: entry.disabled,
    };
    // `McpServerConfig` serializes `name` as a field; the settings file uses
    // the map key for naming, so drop it from the inner object.
    let mut value = serde_json::to_value(&cfg).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = value.as_object_mut() {
        obj.remove("name");
    }
    value
}

/// Build a list of plugin info from the in-memory plugin registry.
pub fn build_plugin_info_list() -> Vec<PluginInfo> {
    use cc_plugins::PluginStatus;

    cc_plugins::get_all_plugins()
        .into_iter()
        .map(|p| {
            let (status_str, error) = match &p.status {
                PluginStatus::NotInstalled => ("not_installed".to_string(), None),
                PluginStatus::Installed => ("installed".to_string(), None),
                PluginStatus::Disabled => ("disabled".to_string(), None),
                PluginStatus::Error(e) => ("error".to_string(), Some(e.clone())),
            };
            PluginInfo {
                id: p.id,
                name: p.name,
                version: p.version,
                status: status_str,
                contributed_tools: p.tools,
                contributed_skills: p.skills,
                contributed_mcp_servers: p.mcp_servers,
                error,
            }
        })
        .collect()
}

fn discover_plugin_skills_for_handlers() -> Vec<cc_skills::SkillDefinition> {
    let mut out = Vec::new();

    for contributed in cc_plugins::discover_plugin_skill_definitions() {
        let source = cc_skills::SkillSource::Plugin(contributed.plugin_id.clone());
        let mut skill =
            match cc_skills::loader::load_skill_from_file_path(&contributed.path, source) {
                Some(skill) => skill,
                None => {
                    tracing::warn!(
                        plugin = %contributed.plugin_id,
                        path = %contributed.path.display(),
                        "Plugin: failed to load contributed skill file"
                    );
                    continue;
                }
            };

        skill.name = contributed.name;
        if let Some(desc) = contributed.description {
            if !desc.trim().is_empty() {
                skill.frontmatter.description = desc;
            }
        }
        out.push(skill);
    }

    out
}

/// Build a list of skill info from the global skill registry.
pub fn build_skill_info_list() -> Vec<SkillInfo> {
    use cc_skills::SkillSource;

    cc_skills::get_all_skills()
        .into_iter()
        .map(|s| {
            let source_str = match &s.source {
                SkillSource::Bundled => "bundled".to_string(),
                SkillSource::User => "user".to_string(),
                SkillSource::Project => "project".to_string(),
                SkillSource::Plugin(_) => "plugin".to_string(),
                SkillSource::Mcp(_) => "mcp".to_string(),
            };
            SkillInfo {
                name: s.display_name().to_string(),
                source: source_str,
                description: s.frontmatter.description.clone(),
                user_invocable: s.is_user_invocable(),
                model_invocable: s.is_model_invocable(),
            }
        })
        .collect()
}

/// Build the list of detected IDE integrations (issue #41).
///
/// Thin wrapper around [`cc_lsp_service::ide::detect_ides`] that exists primarily
/// so the IPC layer has a stable entry point we can hook from other
/// places (e.g. the future `/ide` TUI view) without reaching into the
/// `ide` module.
pub fn build_ide_info_list() -> Vec<IdeInfo> {
    cc_lsp_service::ide::detect_ides()
}

/// Build a complete subsystem status snapshot combining all subsystems.
pub fn build_subsystem_status_snapshot() -> SubsystemStatusSnapshot {
    SubsystemStatusSnapshot {
        lsp: build_lsp_server_info_list(),
        mcp: build_mcp_server_info_list(),
        plugins: build_plugin_info_list(),
        skills: build_skill_info_list(),
        ides: build_ide_info_list(),
        timestamp: chrono::Utc::now().timestamp(),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_lsp_server_info_list_returns_configured_servers() {
        let infos = build_lsp_server_info_list();
        assert!(infos.len() >= 6);
        let rust = infos.iter().find(|i| i.language_id == "rust");
        assert!(rust.is_some());
        assert_eq!(rust.unwrap().state, "not_started");
    }

    #[test]
    fn build_lsp_server_info_list_has_dotted_extensions() {
        let infos = build_lsp_server_info_list();
        let rust = infos.iter().find(|i| i.language_id == "rust").unwrap();
        assert!(
            rust.extensions.contains(&".rs".to_string()),
            "extensions should be dot-prefixed"
        );
    }

    #[test]
    #[serial_test::serial]
    fn build_mcp_server_info_list_defaults_to_pending() {
        cc_mcp::runtime::clear_for_tests();
        let infos = build_mcp_server_info_list();
        for info in &infos {
            assert_eq!(info.state, "pending");
        }
    }

    #[test]
    fn build_plugin_info_list_maps_status() {
        cc_plugins::clear_plugins();
        cc_plugins::register_plugin(cc_plugins::PluginEntry {
            id: "test-plugin-handlers".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "For testing".to_string(),
            source: cc_plugins::PluginSource::Local {
                path: "/tmp/test".to_string(),
            },
            status: cc_plugins::PluginStatus::Installed,
            marketplace: None,
            cache_path: None,
            tools: vec!["tool_a".to_string()],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        });
        cc_plugins::register_plugin(cc_plugins::PluginEntry {
            id: "err-plugin-handlers".to_string(),
            name: "Error Plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "Broken".to_string(),
            source: cc_plugins::PluginSource::Local {
                path: "/tmp/err".to_string(),
            },
            status: cc_plugins::PluginStatus::Error("load failed".to_string()),
            marketplace: None,
            cache_path: None,
            tools: vec![],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        });

        let infos = build_plugin_info_list();
        let test_p = infos.iter().find(|p| p.id == "test-plugin-handlers");
        assert!(test_p.is_some());
        assert_eq!(test_p.unwrap().status, "installed");
        assert!(test_p.unwrap().error.is_none());

        let err_p = infos.iter().find(|p| p.id == "err-plugin-handlers");
        assert!(err_p.is_some());
        assert_eq!(err_p.unwrap().status, "error");
        assert_eq!(err_p.unwrap().error.as_deref(), Some("load failed"));

        cc_plugins::clear_plugins();
    }

    #[test]
    #[serial_test::serial]
    fn build_skill_info_list_returns_skills() {
        cc_skills::clear_skills();
        cc_skills::register_skill(cc_skills::SkillDefinition {
            name: "test-skill".to_string(),
            source: cc_skills::SkillSource::Bundled,
            base_dir: None,
            frontmatter: cc_skills::SkillFrontmatter {
                description: "A test".to_string(),
                user_invocable: true,
                ..Default::default()
            },
            prompt_body: String::new(),
        });
        let infos = build_skill_info_list();
        let test = infos.iter().find(|s| s.name == "test-skill");
        assert!(test.is_some());
        assert_eq!(test.unwrap().source, "bundled");
        assert!(test.unwrap().user_invocable);
        cc_skills::clear_skills();
    }

    #[test]
    #[serial_test::serial]
    fn build_skill_info_list_maps_sources() {
        cc_skills::clear_skills();

        let sources = vec![
            ("bundled-sk", cc_skills::SkillSource::Bundled, "bundled"),
            ("user-sk", cc_skills::SkillSource::User, "user"),
            ("project-sk", cc_skills::SkillSource::Project, "project"),
            (
                "plugin-sk",
                cc_skills::SkillSource::Plugin("p".to_string()),
                "plugin",
            ),
            (
                "mcp-sk",
                cc_skills::SkillSource::Mcp("m".to_string()),
                "mcp",
            ),
        ];

        for (name, source, _) in &sources {
            cc_skills::register_skill(cc_skills::SkillDefinition {
                name: name.to_string(),
                source: source.clone(),
                base_dir: None,
                frontmatter: cc_skills::SkillFrontmatter {
                    description: "test".to_string(),
                    user_invocable: true,
                    ..Default::default()
                },
                prompt_body: String::new(),
            });
        }

        let infos = build_skill_info_list();
        for (name, _, expected_source) in &sources {
            let info = infos.iter().find(|s| s.name == *name);
            assert!(info.is_some(), "skill {} should be present", name);
            assert_eq!(info.unwrap().source, *expected_source);
        }

        cc_skills::clear_skills();
    }

    #[test]
    fn build_subsystem_status_snapshot_has_timestamp() {
        let snapshot = build_subsystem_status_snapshot();
        assert!(snapshot.timestamp > 0, "timestamp should be positive");
        assert!(snapshot.lsp.len() >= 6);
    }

    // ── Handler return-value tests ────────────────────────────────────

    #[test]
    fn handle_lsp_query_status_returns_server_list() {
        use cc_ipc_protocol::subsystem_events::LspCommand;
        let msgs = handle_lsp_command(LspCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::LspEvent { .. }));
    }

    #[test]
    fn handle_lsp_start_returns_info() {
        use cc_ipc_protocol::subsystem_events::LspCommand;
        let msgs = handle_lsp_command(LspCommand::StartServer {
            language_id: "rust".into(),
        });
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::SystemInfo { .. }));
    }

    #[test]
    fn apply_recommendation_decision_no_is_noop() {
        // `no` should not emit a system-info message or mutate persistence.
        let (_settings, info) = apply_recommendation_decision("foo-ls", "no");
        assert!(info.is_none(), "'no' should not produce an info message");
    }

    #[test]
    fn apply_recommendation_decision_unknown_is_warned_but_silent() {
        // Unknown decisions shouldn't blow up or leak into the UI.
        let (_settings, info) = apply_recommendation_decision("foo-ls", "banana");
        assert!(info.is_none());
    }

    #[test]
    fn apply_recommendation_decision_yes_returns_no_info() {
        // `yes` does not mutate settings; installation is handled by
        // `install_recommended_plugin` at the caller site.
        let (_settings, info) = apply_recommendation_decision("rust-analyzer", "yes");
        assert!(info.is_none(), "\"yes\" should not return info text");
    }

    #[test]
    fn handle_mcp_query_status_returns_server_list() {
        use cc_ipc_protocol::subsystem_events::McpCommand;
        let msgs = handle_mcp_command(McpCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::McpEvent { .. }));
    }

    // ── MCP config editing tests (issue #44) ─────────────────────────
    //
    // These tests drive the pure `upsert_mcp_entry` / `remove_mcp_entry`
    // helpers against a temp `CC_RUST_HOME` / cwd to avoid touching the
    // user's real settings file. They also verify `ConfigError` is emitted
    // for read-only scopes.

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    struct RuntimeMcpGuard;

    impl RuntimeMcpGuard {
        fn install(
            manager: std::sync::Arc<tokio::sync::Mutex<cc_mcp::manager::McpManager>>,
        ) -> Self {
            cc_mcp::runtime::clear_for_tests();
            cc_mcp::runtime::install_manager(manager);
            Self
        }
    }

    impl Drop for RuntimeMcpGuard {
        fn drop(&mut self) {
            cc_mcp::runtime::clear_for_tests();
        }
    }

    #[test]
    #[serial_test::serial]
    fn upsert_mcp_entry_persists_to_user_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "ctx7".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("npx".to_string()),
            args: Some(vec!["-y".to_string(), "ctx7".to_string()]),
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };

        let written = upsert_mcp_entry(cwd.path(), entry).expect("upsert ok");
        assert_eq!(written.name, "ctx7");

        let settings_path = home.path().join("settings.json");
        assert!(
            settings_path.exists(),
            "user settings.json should be created"
        );
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(on_disk["mcpServers"]["ctx7"]["command"], "npx");
        assert_eq!(on_disk["mcpServers"]["ctx7"]["args"][0], "-y");
    }

    #[test]
    #[serial_test::serial]
    fn upsert_mcp_entry_persists_to_project_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "proj-srv".to_string(),
            scope: ConfigScope::Project,
            transport: "stdio".to_string(),
            command: Some("./local.sh".to_string()),
            args: None,
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };

        upsert_mcp_entry(cwd.path(), entry).expect("upsert ok");

        let path = cwd.path().join(".cc-rust").join("settings.json");
        assert!(path.exists(), "project settings.json should be created");
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk["mcpServers"]["proj-srv"]["command"], "./local.sh");
    }

    #[test]
    #[serial_test::serial]
    fn upsert_mcp_entry_rejects_plugin_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "plugin-srv".to_string(),
            scope: ConfigScope::Plugin {
                id: "com.example.p".to_string(),
            },
            transport: "stdio".to_string(),
            command: Some("x".to_string()),
            args: None,
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };

        let err = upsert_mcp_entry(cwd.path(), entry).expect_err("plugin scope rejected");
        assert_eq!(err.0, "plugin-srv");
        assert!(err.1.contains("read-only"));
    }

    #[test]
    #[serial_test::serial]
    fn remove_mcp_entry_round_trips_user_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "ctx7".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("npx".to_string()),
            args: None,
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };
        upsert_mcp_entry(cwd.path(), entry).expect("upsert ok");

        remove_mcp_entry(cwd.path(), "ctx7", &ConfigScope::User).expect("remove ok");

        let settings_path = home.path().join("settings.json");
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let servers = on_disk
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .expect("mcpServers object");
        assert!(!servers.contains_key("ctx7"), "entry should be gone");
    }

    #[test]
    #[serial_test::serial]
    fn remove_mcp_entry_rejects_plugin_scope() {
        let cwd = tempfile::tempdir().expect("tempdir");
        let err = remove_mcp_entry(
            cwd.path(),
            "p",
            &ConfigScope::Plugin {
                id: "com.example.p".to_string(),
            },
        )
        .expect_err("plugin scope rejected");
        assert!(err.contains("read-only"));
    }

    #[test]
    #[serial_test::serial]
    fn remove_mcp_entry_errors_on_missing_file() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let err = remove_mcp_entry(cwd.path(), "nope", &ConfigScope::User)
            .expect_err("missing file should error");
        assert!(err.contains("nothing to remove"));
    }

    #[test]
    #[serial_test::serial]
    fn handle_mcp_upsert_config_emits_config_changed() {
        use cc_ipc_protocol::subsystem_events::McpCommand;
        let home = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "h-test".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("t".to_string()),
            args: None,
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };
        let msgs = handle_mcp_command(McpCommand::UpsertConfig {
            entry: Box::new(entry),
        });
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BackendMessage::McpEvent {
                event:
                    McpEvent::ConfigChanged {
                        server_name,
                        entry: Some(e),
                    },
            } => {
                assert_eq!(server_name, "h-test");
                assert_eq!(e.name, "h-test");
                assert_eq!(e.scope, ConfigScope::User);
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    #[serial_test::serial]
    fn handle_mcp_upsert_config_on_read_only_emits_config_error() {
        use cc_ipc_protocol::subsystem_events::McpCommand;

        let entry = McpServerConfigEntry {
            name: "plugin-srv".to_string(),
            scope: ConfigScope::Plugin {
                id: "com.example".to_string(),
            },
            transport: "stdio".to_string(),
            command: Some("x".to_string()),
            args: None,
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };
        let msgs = handle_mcp_command(McpCommand::UpsertConfig {
            entry: Box::new(entry),
        });
        match &msgs[0] {
            BackendMessage::McpEvent {
                event: McpEvent::ConfigError { server_name, .. },
            } => assert_eq!(server_name, "plugin-srv"),
            other => panic!("expected ConfigError, got {:?}", other),
        }
    }

    #[test]
    #[serial_test::serial]
    fn handle_mcp_query_config_returns_config_list() {
        use cc_ipc_protocol::subsystem_events::McpCommand;
        let msgs = handle_mcp_command(McpCommand::QueryConfig);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BackendMessage::McpEvent {
                event: McpEvent::ConfigList { .. },
            } => {}
            other => panic!("expected ConfigList, got {:?}", other),
        }
    }

    #[test]
    #[serial_test::serial]
    fn toggle_mcp_entry_enabled_flips_disabled_flag() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "tog-srv".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("npx".to_string()),
            args: None,
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };
        upsert_mcp_entry(cwd.path(), entry).expect("upsert ok");

        // First toggle: enable ->disabled.
        let after_disable =
            toggle_mcp_entry_enabled(cwd.path(), "tog-srv", None).expect("first toggle ok");
        assert_eq!(after_disable.disabled, Some(true));

        // Second toggle: disabled ->enabled.
        let after_enable =
            toggle_mcp_entry_enabled(cwd.path(), "tog-srv", None).expect("second toggle ok");
        assert_eq!(after_enable.disabled, Some(false));

        // Verify final on-disk value reflects the second toggle.
        let settings_path = home.path().join("settings.json");
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        // `disabled: false` serializes to false via the derive — but our
        // serializer skips `None`, so either missing or literal false is OK.
        let v = &on_disk["mcpServers"]["tog-srv"]["disabled"];
        assert!(v.is_null() || v == &serde_json::Value::Bool(false));
    }

    #[test]
    #[serial_test::serial]
    fn toggle_mcp_entry_enabled_rejects_plugin_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        // Directly ask to toggle in a read-only scope — should error even
        // when no matching entry exists in the scope.
        let err = toggle_mcp_entry_enabled(
            cwd.path(),
            "nope",
            Some(&ConfigScope::Plugin {
                id: "com.example".to_string(),
            }),
        )
        .expect_err("plugin scope rejected");
        assert!(err.contains("no MCP server named"));
    }

    #[test]
    #[serial_test::serial]
    fn handle_mcp_toggle_enabled_emits_config_changed_and_state() {
        use cc_ipc_protocol::subsystem_events::McpCommand;
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        // Seed an entry in user scope via the handler so the `cwd` used
        // to discover matches the one the toggle handler uses.
        let prev_cwd = std::env::current_dir().ok();
        std::env::set_current_dir(cwd.path()).expect("set cwd");
        let seed = McpServerConfigEntry {
            name: "h-tog".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("t".to_string()),
            args: None,
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        };
        let _ = handle_mcp_command(McpCommand::UpsertConfig {
            entry: Box::new(seed),
        });

        let msgs = handle_mcp_command(McpCommand::ToggleEnabled {
            server_name: "h-tog".to_string(),
            scope: Some(ConfigScope::User),
        });
        assert_eq!(msgs.len(), 2, "expected ConfigChanged + ServerStateChanged");
        match &msgs[0] {
            BackendMessage::McpEvent {
                event:
                    McpEvent::ConfigChanged {
                        server_name,
                        entry: Some(e),
                    },
            } => {
                assert_eq!(server_name, "h-tog");
                assert_eq!(e.disabled, Some(true));
            }
            other => panic!("unexpected first message: {:?}", other),
        }
        match &msgs[1] {
            BackendMessage::McpEvent {
                event: McpEvent::ServerStateChanged { state, .. },
            } => {
                assert_eq!(state, "disabled");
            }
            other => panic!("unexpected second message: {:?}", other),
        }

        if let Some(p) = prev_cwd {
            let _ = std::env::set_current_dir(p);
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn handle_mcp_reconnect_uses_runtime_manager_and_emits_final_state() {
        use cc_ipc_protocol::subsystem_events::McpCommand;

        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let manager =
            std::sync::Arc::new(tokio::sync::Mutex::new(cc_mcp::manager::McpManager::new()));
        let _runtime = RuntimeMcpGuard::install(manager.clone());
        std::fs::write(
            home.path().join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "rec-srv": {
                        "type": "stdio",
                        "command": "unused",
                        "disabled": true
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let msgs = handle_mcp_command_with_runtime(
            McpCommand::ReconnectServer {
                server_name: "rec-srv".to_string(),
            },
            cwd.path(),
        )
        .await;
        assert!(
            manager.lock().await.clients.is_empty(),
            "disabled reconnect must not keep a live client"
        );
        assert_eq!(msgs.len(), 2);
        match &msgs[0] {
            BackendMessage::McpEvent {
                event:
                    McpEvent::ServerStateChanged {
                        server_name, state, ..
                    },
            } => {
                assert_eq!(server_name, "rec-srv");
                assert_eq!(state, "disabled");
            }
            other => panic!("expected ServerStateChanged, got {:?}", other),
        }
        match &msgs[1] {
            BackendMessage::SystemInfo { text, .. } => {
                assert!(text.contains("rec-srv"));
                assert!(text.contains("disabled"));
            }
            other => panic!("expected SystemInfo, got {:?}", other),
        }
    }

    #[test]
    #[serial_test::serial]
    fn build_mcp_server_config_entries_tags_user_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        std::fs::write(
            home.path().join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "u-srv": {"transport": "stdio", "command": "u-cmd"}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let entries = build_mcp_server_config_entries(cwd.path());
        let entry = entries
            .iter()
            .find(|e| e.name == "u-srv")
            .expect("user entry present");
        assert_eq!(entry.scope, ConfigScope::User);
        assert_eq!(entry.command.as_deref(), Some("u-cmd"));
    }

    #[test]
    fn handle_plugin_query_status_returns_plugin_list() {
        use cc_ipc_protocol::subsystem_events::PluginCommand;
        let msgs = handle_plugin_command(PluginCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::PluginEvent { .. }));
    }

    #[test]
    fn handle_skill_query_status_returns_skill_list() {
        use cc_ipc_protocol::subsystem_events::SkillCommand;
        let msgs = handle_skill_command(SkillCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::SkillEvent { .. }));
    }
}
