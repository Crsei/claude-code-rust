//! Headless runtime — owns the `tokio::select!` main loop and all runtime state.
//!
//! [`HeadlessRuntime`] is the core object that ties together the engine,
//! permission/question bridges, the agent event bus, the subsystem event bus,
//! the prompt suggestion service, and the frontend sink.
//!
//! The previous monolithic `run_headless()` function is now a thin wrapper
//! that constructs a `HeadlessRuntime` and calls [`HeadlessRuntime::run()`].

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::io::AsyncBufReadExt;
use tracing::{debug, error, warn};

use cc_engine::lifecycle::QueryEngine;
use cc_services::prompt_suggestion::PromptSuggestionService;

use super::callbacks::{PendingPermissions, PendingQuestions};
use cc_ipc_client::sink::FrontendSink;
use cc_ipc_protocol::{BackendMessage, FrontendMessage};

fn lsp_event_to_subsystem(
    event: cc_lsp_service::LspEvent,
) -> cc_ipc_protocol::subsystem_events::SubsystemEvent {
    use cc_ipc_protocol::subsystem_events::{LspEvent, SubsystemEvent};
    let event = match event {
        cc_lsp_service::LspEvent::ServerStateChanged {
            language_id,
            state,
            error,
        } => LspEvent::ServerStateChanged {
            language_id,
            state,
            error,
        },
        cc_lsp_service::LspEvent::DocumentSynced {
            uri,
            language_id,
            version,
            change_kind,
        } => LspEvent::DocumentSynced {
            uri,
            language_id,
            version,
            change_kind,
        },
        cc_lsp_service::LspEvent::DiagnosticsPublished { uri, diagnostics } => {
            LspEvent::DiagnosticsPublished {
                uri,
                diagnostics: diagnostics.into_iter().map(lsp_diagnostic_to_ipc).collect(),
            }
        }
        cc_lsp_service::LspEvent::CompletionResults {
            request_id,
            uri,
            items,
        } => LspEvent::CompletionResults {
            request_id,
            uri,
            items: items.into_iter().map(lsp_completion_to_ipc).collect(),
        },
        cc_lsp_service::LspEvent::CommandError {
            request_id,
            message,
        } => LspEvent::CommandError {
            request_id,
            message,
        },
    };
    SubsystemEvent::Lsp(event)
}

fn lsp_diagnostic_to_ipc(
    diagnostic: cc_lsp_service::LspDiagnostic,
) -> cc_ipc_protocol::subsystem_types::LspDiagnostic {
    cc_ipc_protocol::subsystem_types::LspDiagnostic {
        range: cc_ipc_protocol::subsystem_types::DiagnosticRange {
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

fn lsp_completion_to_ipc(
    item: cc_lsp_service::CompletionItemInfo,
) -> cc_ipc_protocol::CompletionItemInfo {
    cc_ipc_protocol::CompletionItemInfo {
        label: item.label,
        kind: item.kind,
        detail: item.detail,
        documentation: item.documentation,
        insert_text: item.insert_text,
        sort_text: item.sort_text,
        filter_text: item.filter_text,
    }
}

/// The headless runtime.
///
/// All mutable runtime state lives here.  The `select!` loop in [`run()`]
/// multiplexes frontend messages, agent events, and subsystem events.
pub struct HeadlessRuntime {
    pub(crate) engine: Arc<QueryEngine>,
    pub(crate) pending_permissions: PendingPermissions,
    pub(crate) pending_questions: PendingQuestions,
    pub(crate) suggestion_svc: Arc<Mutex<PromptSuggestionService>>,
    pub(crate) sink: FrontendSink,
}

impl HeadlessRuntime {
    /// Create a new runtime.  Call [`run()`] to start the event loop.
    pub fn new(engine: Arc<QueryEngine>, sink: FrontendSink) -> Self {
        Self {
            engine,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            pending_questions: Arc::new(Mutex::new(HashMap::new())),
            suggestion_svc: Arc::new(Mutex::new(PromptSuggestionService::new(true))),
            sink,
        }
    }

    /// Run the headless event loop.
    ///
    /// Installs callbacks, sets up event channels, sends `Ready`, then enters
    /// the multiplexed `select!` loop.  Returns when the frontend sends `Quit`
    /// or stdin is closed.
    pub async fn run(&self, model: String) -> anyhow::Result<()> {
        super::runtime_adapters::ensure_installed();

        // ── 1. Install callbacks ──────────────────────────────────────
        super::callbacks::install_permission_callback(
            &self.engine,
            self.pending_permissions.clone(),
            self.sink.clone(),
        );
        super::callbacks::install_ask_user_callback(
            &self.engine,
            self.pending_questions.clone(),
            self.sink.clone(),
        );
        super::callbacks::install_tool_progress_callback(&self.engine, self.sink.clone());

        // ── 1b. Background agent channel ─────────────────────────────
        let (agent_tx, mut agent_rx) = cc_types::agent_channel::agent_channel();
        self.engine.set_bg_agent_tx(agent_tx);
        let pending_bg = self.engine.pending_background_results();

        // ── 1c. Subsystem event bus ──────────────────────────────────
        let event_bus = cc_ipc::subsystem_events::SubsystemEventBus::new();
        let mut event_rx = event_bus.subscribe();
        let (lsp_tx, mut lsp_rx) = tokio::sync::broadcast::channel(128);
        cc_lsp_service::set_event_sender(lsp_tx);
        let lsp_event_tx = event_bus.sender();
        tokio::spawn(async move {
            while let Ok(event) = lsp_rx.recv().await {
                let _ = lsp_event_tx.send(lsp_event_to_subsystem(event));
            }
        });
        let plugin_tx = event_bus.sender();
        cc_plugins::set_event_sink(Some(Arc::new(move |event| {
            let adapted = match event {
                cc_plugins::PluginSubsystemEvent::Reloaded { count, had_error } => {
                    cc_ipc_protocol::subsystem_events::SubsystemEvent::Plugin(
                        cc_ipc_protocol::subsystem_events::PluginEvent::Reloaded {
                            count,
                            had_error,
                        },
                    )
                }
                cc_plugins::PluginSubsystemEvent::RefreshNeeded { reason } => {
                    cc_ipc_protocol::subsystem_events::SubsystemEvent::Plugin(
                        cc_ipc_protocol::subsystem_events::PluginEvent::RefreshNeeded { reason },
                    )
                }
                cc_plugins::PluginSubsystemEvent::StatusChanged {
                    plugin_id,
                    name,
                    status,
                    error,
                } => cc_ipc_protocol::subsystem_events::SubsystemEvent::Plugin(
                    cc_ipc_protocol::subsystem_events::PluginEvent::StatusChanged {
                        plugin_id,
                        name,
                        status,
                        error,
                    },
                ),
            };
            let _ = plugin_tx.send(adapted);
        })));
        crate::ide::set_event_sender(event_bus.sender());
        super::agent_settings_generate::set_event_sender(event_bus.sender());
        // cc-skills lives in its own crate and no longer knows about
        // `SubsystemEvent`. Adapt its minimal event enum into ours here.
        let skills_tx = event_bus.sender();
        cc_skills::set_event_callback(move |e| {
            let adapted = match e {
                cc_skills::SkillSubsystemEvent::SkillsLoaded { count } => {
                    cc_ipc_protocol::subsystem_events::SubsystemEvent::Skill(
                        cc_ipc_protocol::subsystem_events::SkillEvent::SkillsLoaded { count },
                    )
                }
            };
            let _ = skills_tx.send(adapted);
        });
        // cc-mcp receives its event sink through the runtime-owned manager.
        let mcp_tx = event_bus.sender();
        let mcp_sink: cc_mcp::SharedMcpEventSink = Arc::new(move |e| {
            let adapted = match e {
                cc_mcp::McpSubsystemEvent::ServerStateChanged {
                    server_name,
                    state,
                    error,
                } => {
                    cc_mcp::runtime::record_server_state(
                        server_name.clone(),
                        state.clone(),
                        error.clone(),
                    );
                    cc_ipc_protocol::subsystem_events::SubsystemEvent::Mcp(
                        cc_ipc_protocol::subsystem_events::McpEvent::ServerStateChanged {
                            server_name,
                            state,
                            error,
                        },
                    )
                }
                cc_mcp::McpSubsystemEvent::ToolsDiscovered { server_name, tools } => {
                    cc_ipc_protocol::subsystem_events::SubsystemEvent::Mcp(
                        cc_ipc_protocol::subsystem_events::McpEvent::ToolsDiscovered {
                            server_name,
                            tools: tools
                                .into_iter()
                                .map(|t| cc_ipc_protocol::subsystem_types::McpToolInfo {
                                    name: t.tool_name,
                                    description: Some(t.description),
                                })
                                .collect(),
                        },
                    )
                }
                cc_mcp::McpSubsystemEvent::ResourcesDiscovered {
                    server_name,
                    resources,
                } => cc_ipc_protocol::subsystem_events::SubsystemEvent::Mcp(
                    cc_ipc_protocol::subsystem_events::McpEvent::ResourcesDiscovered {
                        server_name,
                        resources: resources
                            .into_iter()
                            .map(|r| cc_ipc_protocol::subsystem_types::McpResourceInfo {
                                uri: r.uri,
                                name: Some(r.name),
                                mime_type: r.mime_type,
                            })
                            .collect(),
                    },
                ),
                cc_mcp::McpSubsystemEvent::ChannelNotification {
                    server_name,
                    content,
                    meta,
                } => cc_ipc_protocol::subsystem_events::SubsystemEvent::Mcp(
                    cc_ipc_protocol::subsystem_events::McpEvent::ChannelNotification {
                        server_name,
                        content,
                        meta,
                    },
                ),
            };
            let _ = mcp_tx.send(adapted);
        });
        if let Some(manager) = cc_mcp::runtime::current_manager() {
            manager.lock().await.set_event_sink(Some(mcp_sink));
        }
        // Wire the plugin-contributed MCP discovery hook. Plugins return
        // `cc_mcp::McpServerConfig`, which is re-exported from
        // `cc_mcp::McpServerConfig`, so they are the same type.
        cc_mcp::discovery::set_plugin_hook(cc_plugins::discover_plugin_mcp_servers);
        // Scope-aware variant (issue #44) — preserves each server's owning
        // plugin id so `/mcp list` can attribute entries correctly.
        cc_mcp::discovery::set_scoped_plugin_hook(|| {
            cc_plugins::discover_plugin_mcp_servers_scoped()
        });
        // Wire the IDE-contributed MCP bridge hook (issue #41).
        cc_mcp::discovery::set_ide_hook(crate::ide::selected_ide_mcp_config);

        // ── 2. Send Ready ────────────────────────────────────────────
        let app_state = self.engine.app_state();
        let keybindings = app_state
            .keybindings
            .user_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
        self.sink.send(&BackendMessage::Ready {
            session_id: self.engine.current_session_id().to_string(),
            model,
            cwd: self.engine.cwd().to_string(),
            permission_mode: app_state.tool_permission_context.mode.as_str().to_string(),
            available_models: app_state.settings.available_models.clone(),
            plan_workflow: app_state.plan_workflow.clone(),
            editor_mode: app_state.settings.editor_mode.clone(),
            view_mode: app_state.settings.view_mode.clone(),
            keybindings,
        })?;

        // ── 3. Main select loop ──────────────────────────────────────
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        let mut lines = stdin.lines();

        loop {
            tokio::select! {
                // ── Branch 1: Frontend message (stdin) ──────────────
                line = lines.next_line() => {
                    let line = match line {
                        Ok(Some(line)) => line,
                        Ok(None) => {
                            debug!("headless: stdin closed, exiting");
                            break;
                        }
                        Err(e) => {
                            error!("headless: error reading stdin: {}", e);
                            break;
                        }
                    };

                    let msg: FrontendMessage = match serde_json::from_str(&line) {
                        Ok(m) => m,
                        Err(e) => {
                            warn!(
                                "headless: failed to parse FrontendMessage: {} — line: {}",
                                e, line
                            );
                            let _ = self.sink.send(&BackendMessage::Error {
                                message: format!("invalid FrontendMessage: {}", e),
                                recoverable: true,
                            });
                            continue;
                        }
                    };

                    let keep_running = super::ingress::dispatch(
                        msg,
                        &self.engine,
                        &self.pending_permissions,
                        &self.pending_questions,
                        &self.suggestion_svc,
                        &self.sink,
                    ).await;

                    if !keep_running {
                        break;
                    }
                }

                // ── Branch 2: Agent/Team events ──────────────────────
                Some(event) = agent_rx.recv() => {
                    match event {
                        cc_types::agent_channel::AgentIpcEvent::Agent(ref agent_event) => {
                            if let cc_types::agent_events::AgentEvent::Completed {
                                ref agent_id, ref result_preview, had_error, duration_ms, ..
                            } = agent_event {
                                let tree = cc_ipc::agent_tree::AGENT_TREE.lock();
                                let (is_bg, desc) = tree.get(agent_id)
                                    .map(|n| (n.is_background, n.description.clone()))
                                    .unwrap_or((true, "unknown".to_string()));
                                drop(tree);

                                if is_bg {
                                    let _ = self.sink.send(&BackendMessage::BackgroundAgentComplete {
                                        agent_id: agent_id.clone(),
                                        description: desc.clone(),
                                        result_preview: result_preview.clone(),
                                        had_error: *had_error,
                                        duration_ms: *duration_ms,
                                    });
                                    let result_text = crate::engine::agent::supervisor::output_for_agent(agent_id)
                                        .map(|task| task.output)
                                        .filter(|output| !output.is_empty())
                                        .unwrap_or_else(|| result_preview.clone());

                                    pending_bg.push(cc_engine::agent_runtime::CompletedBackgroundAgent {
                                        agent_id: agent_id.clone(),
                                        description: desc,
                                        result_text,
                                        had_error: *had_error,
                                        duration: std::time::Duration::from_millis(*duration_ms),
                                    });
                                }
                            }
                            let _ = self.sink.send(&BackendMessage::AgentEvent {
                                event: agent_event.clone(),
                            });
                        }
                        cc_types::agent_channel::AgentIpcEvent::Team(team_event) => {
                            let _ = self.sink.send(&BackendMessage::TeamEvent {
                                event: team_event,
                            });
                        }
                    }
                }

                // ── Branch 3: Subsystem events ───────────────────────
                Ok(event) = event_rx.recv() => {
                    let msg = match event {
                        cc_ipc_protocol::subsystem_events::SubsystemEvent::Lsp(e) => {
                            BackendMessage::LspEvent { event: e }
                        }
                        cc_ipc_protocol::subsystem_events::SubsystemEvent::Mcp(e) => {
                            BackendMessage::McpEvent { event: e }
                        }
                        cc_ipc_protocol::subsystem_events::SubsystemEvent::Plugin(e) => {
                            BackendMessage::PluginEvent { event: e }
                        }
                        cc_ipc_protocol::subsystem_events::SubsystemEvent::Skill(e) => {
                            BackendMessage::SkillEvent { event: e }
                        }
                        cc_ipc_protocol::subsystem_events::SubsystemEvent::Ide(e) => {
                            BackendMessage::IdeEvent { event: e }
                        }
                        cc_ipc_protocol::subsystem_events::SubsystemEvent::AgentSettings(e) => {
                            BackendMessage::AgentSettingsEvent { event: e }
                        }
                    };
                    let _ = self.sink.send(&msg);
                }
            }
        }

        let cancelled =
            crate::engine::agent::supervisor::shutdown_all("headless runtime exit").await;
        if cancelled > 0 {
            let _ = self.sink.send(&BackendMessage::SystemInfo {
                text: format!(
                    "Cancelled {} background agent(s) during shutdown cleanup.",
                    cancelled
                ),
                level: "info".to_string(),
            });
        }

        Ok(())
    }
}
