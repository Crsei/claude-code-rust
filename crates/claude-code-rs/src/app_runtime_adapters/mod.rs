//! Host adapters that bind the extracted `cc-ipc` runtime facade to this binary.

use std::path::Path;
use std::sync::{Arc, Once};

use parking_lot::Mutex;

pub(crate) mod callbacks;
mod ingress;
mod query_runner;
mod sdk_mapper;

use cc_ipc::agent_handlers::{AgentRuntimeHost, AgentTaskOutput};
use cc_ipc::headless::{
    BackgroundAgentCompletion, BoxHeadlessFuture, HeadlessRuntimeConfig, HeadlessRuntimeHost,
    PendingPermissions, PendingQuestions,
};
use cc_ipc::subsystem_handlers::{
    BoxRuntimeFuture, McpRuntimeOperation, McpRuntimeReport, SubsystemRuntimeHost,
};
use cc_ipc_client::sink::FrontendSink;
use cc_ipc_protocol::protocol::{BackendMessage, FrontendMessage};
use cc_ipc_protocol::subsystem_events::{
    IdeCommand, LspCommand, McpCommand, PluginCommand, SkillCommand, SubsystemEvent,
};
use cc_ipc_protocol::subsystem_types::*;
use cc_services::prompt_suggestion::PromptSuggestionService;
use cc_types::agent_types::TeamMemberInfo;

static INSTALL: Once = Once::new();

pub fn ensure_installed() {
    INSTALL.call_once(|| {
        cc_ipc::agent_handlers::set_runtime_host(Arc::new(RootAgentHost));
        cc_ipc::subsystem_handlers::set_runtime_host(Arc::new(RootSubsystemHost));
        let mut adapters = cc_engine::agent_runtime::agent_runtime_adapters();
        adapters.agent_tree = Arc::new(RootAgentTreeRuntime);
        adapters.builtin_agents = Arc::new(RootAgentDefinitionRegistry);
        cc_engine::agent_runtime::set_agent_runtime_adapters(adapters);
    });
}

pub fn headless_config(
    engine: Arc<cc_engine::lifecycle::QueryEngine>,
    model: String,
) -> HeadlessRuntimeConfig {
    ensure_installed();
    HeadlessRuntimeConfig::new(
        Arc::new(RootHeadlessHost {
            engine,
            suggestion_svc: Arc::new(Mutex::new(PromptSuggestionService::new(true))),
        }),
        model,
    )
}

struct RootAgentDefinitionRegistry;

impl cc_engine::agent_runtime::BuiltinAgentRegistry for RootAgentDefinitionRegistry {
    fn builtin_agent_entries(&self) -> Vec<AgentDefinitionEntry> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        cc_ipc::agent_settings::list_all_agents(&cwd)
    }

    fn builtin_agent_prompt(&self, name: &str) -> Option<String> {
        cc_ipc::builtin_agents::builtin_agent_prompt(name).map(ToOwned::to_owned)
    }
}

struct RootAgentTreeRuntime;

impl cc_engine::agent_runtime::AgentTreeRuntime for RootAgentTreeRuntime {
    fn register(&self, node: cc_types::agent_types::AgentNode) {
        cc_ipc::agent_tree::AGENT_TREE.lock().register(node);
    }

    fn update_state(
        &self,
        agent_id: &str,
        state: &str,
        result_preview: Option<String>,
        duration_ms: Option<u64>,
        had_error: bool,
    ) {
        cc_ipc::agent_tree::AGENT_TREE.lock().update_state(
            agent_id,
            state,
            result_preview,
            duration_ms,
            had_error,
        );
    }

    fn snapshot(&self) -> Vec<cc_types::agent_types::AgentNode> {
        cc_ipc::agent_tree::AGENT_TREE.lock().build_snapshot()
    }

    fn active_count(&self) -> usize {
        cc_ipc::agent_tree::AGENT_TREE.lock().active_agents().len()
    }
}

struct RootAgentHost;

impl AgentRuntimeHost for RootAgentHost {
    fn cancel_agent(&self, agent_id: &str) -> Option<String> {
        cc_engine::agent::supervisor::cancel_agent(agent_id)
    }

    fn agent_output(&self, agent_id: &str) -> Option<AgentTaskOutput> {
        cc_engine::agent::supervisor::output_for_agent(agent_id).map(|task| AgentTaskOutput {
            id: task.id,
            output: task.output,
        })
    }

    fn write_team_message(&self, team_name: &str, to: &str, text: &str) -> Result<(), String> {
        let msg = cc_teams::types::TeammateMessage {
            from: "__frontend__".to_string(),
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            read: false,
            color: None,
            summary: None,
        };
        cc_teams::mailbox::write_to_mailbox(to, msg, team_name).map_err(|e| e.to_string())
    }

    fn team_members(&self, team_name: &str) -> Result<Vec<TeamMemberInfo>, String> {
        let tf = cc_teams::helpers::read_team_file(team_name).map_err(|e| e.to_string())?;
        Ok(tf
            .members
            .iter()
            .map(|m| TeamMemberInfo {
                agent_id: m.agent_id.clone(),
                agent_name: m.name.clone(),
                role: m.agent_type.clone(),
                is_active: m.is_active.unwrap_or(true),
                unread_messages: cc_teams::mailbox::read_unread_messages(&m.name, team_name)
                    .map(|v| v.len())
                    .unwrap_or(0),
            })
            .collect())
    }
}

struct RootSubsystemHost;

impl SubsystemRuntimeHost for RootSubsystemHost {
    fn handle_lsp_command(&self, cmd: LspCommand) -> Vec<BackendMessage> {
        crate::app_subsystem_handlers::handle_lsp_command(cmd)
    }

    fn handle_mcp_command(&self, cmd: McpCommand) -> Vec<BackendMessage> {
        crate::app_subsystem_handlers::handle_mcp_command(cmd)
    }

    fn handle_mcp_command_with_runtime<'a>(
        &'a self,
        cmd: McpCommand,
        cwd: &'a Path,
    ) -> BoxRuntimeFuture<'a, Vec<BackendMessage>> {
        Box::pin(crate::app_subsystem_handlers::handle_mcp_command_with_runtime(cmd, cwd))
    }

    fn handle_plugin_command(&self, cmd: PluginCommand) -> Vec<BackendMessage> {
        crate::app_subsystem_handlers::handle_plugin_command(cmd)
    }

    fn handle_skill_command(&self, cmd: SkillCommand) -> Vec<BackendMessage> {
        crate::app_subsystem_handlers::handle_skill_command(cmd)
    }

    fn handle_ide_command(&self, cmd: IdeCommand) -> Vec<BackendMessage> {
        crate::app_subsystem_handlers::handle_ide_command(cmd)
    }

    fn build_subsystem_status_snapshot(&self) -> SubsystemStatusSnapshot {
        crate::app_subsystem_handlers::build_subsystem_status_snapshot()
    }

    fn build_lsp_server_info_list(&self) -> Vec<LspServerInfo> {
        crate::app_subsystem_handlers::build_lsp_server_info_list()
    }

    fn load_lsp_recommendation_settings(&self) -> LspRecommendationSettings {
        crate::app_subsystem_handlers::load_lsp_recommendation_settings()
    }

    fn build_mcp_server_info_list(&self) -> Vec<McpServerStatusInfo> {
        crate::app_subsystem_handlers::build_mcp_server_info_list()
    }

    fn build_mcp_server_info_list_for_cwd_async<'a>(
        &'a self,
        cwd: &'a Path,
    ) -> BoxRuntimeFuture<'a, Vec<McpServerStatusInfo>> {
        Box::pin(crate::app_subsystem_handlers::build_mcp_server_info_list_for_cwd_async(cwd))
    }

    fn build_mcp_server_config_entries(&self, cwd: &Path) -> Vec<McpServerConfigEntry> {
        crate::app_subsystem_handlers::build_mcp_server_config_entries(cwd)
    }

    fn run_mcp_runtime_operation<'a>(
        &'a self,
        cwd: &'a Path,
        operation: McpRuntimeOperation,
        server_name: &'a str,
    ) -> BoxRuntimeFuture<'a, McpRuntimeReport> {
        Box::pin(async move {
            let operation = match operation {
                McpRuntimeOperation::Connect => {
                    crate::app_subsystem_handlers::McpRuntimeOperation::Connect
                }
                McpRuntimeOperation::Disconnect => {
                    crate::app_subsystem_handlers::McpRuntimeOperation::Disconnect
                }
                McpRuntimeOperation::Reconnect => {
                    crate::app_subsystem_handlers::McpRuntimeOperation::Reconnect
                }
            };
            let report = crate::app_subsystem_handlers::run_mcp_runtime_operation(
                cwd,
                operation,
                server_name,
            )
            .await;
            McpRuntimeReport {
                server_name: report.server_name,
                state: report.state,
                error: report.error,
                text: report.text,
                level: report.level,
            }
        })
    }

    fn build_plugin_info_list(&self) -> Vec<PluginInfo> {
        crate::app_subsystem_handlers::build_plugin_info_list()
    }

    fn build_skill_info_list(&self) -> Vec<SkillInfo> {
        crate::app_subsystem_handlers::build_skill_info_list()
    }

    fn build_ide_info_list(&self) -> Vec<IdeInfo> {
        crate::app_subsystem_handlers::build_ide_info_list()
    }
}

struct RootHeadlessHost {
    engine: Arc<cc_engine::lifecycle::QueryEngine>,
    suggestion_svc: Arc<Mutex<PromptSuggestionService>>,
}

impl HeadlessRuntimeHost for RootHeadlessHost {
    fn install(
        &self,
        pending_permissions: PendingPermissions,
        pending_questions: PendingQuestions,
        sink: FrontendSink,
        agent_tx: cc_types::agent_channel::AgentSender,
        subsystem_tx: tokio::sync::broadcast::Sender<SubsystemEvent>,
    ) {
        ensure_installed();
        callbacks::install_permission_callback(&self.engine, pending_permissions, sink.clone());
        callbacks::install_ask_user_callback(&self.engine, pending_questions, sink.clone());
        callbacks::install_tool_progress_callback(&self.engine, sink);
        self.engine.set_bg_agent_tx(agent_tx);
        install_root_subsystem_event_sinks(subsystem_tx);
    }

    fn ready_message(&self, model: String) -> BackendMessage {
        let app_state = self.engine.app_state();
        let keybindings = app_state
            .keybindings
            .user_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
        BackendMessage::Ready {
            session_id: self.engine.current_session_id().to_string(),
            model,
            cwd: self.engine.cwd().to_string(),
            permission_mode: app_state.tool_permission_context.mode.as_str().to_string(),
            available_models: app_state.settings.available_models.clone(),
            plan_workflow: app_state.plan_workflow.clone(),
            editor_mode: app_state.settings.editor_mode.clone(),
            view_mode: app_state.settings.view_mode.clone(),
            keybindings,
        }
    }

    fn dispatch_frontend<'a>(
        &'a self,
        msg: FrontendMessage,
        pending_permissions: &'a PendingPermissions,
        pending_questions: &'a PendingQuestions,
        sink: &'a FrontendSink,
    ) -> BoxHeadlessFuture<'a, bool> {
        Box::pin(async move {
            ingress::dispatch(
                msg,
                &self.engine,
                pending_permissions,
                pending_questions,
                &self.suggestion_svc,
                sink,
            )
            .await
        })
    }

    fn background_agent_completed(
        &self,
        agent_id: &str,
        result_preview: &str,
        had_error: bool,
        duration_ms: u64,
    ) -> Option<BackgroundAgentCompletion> {
        let tree = cc_ipc::agent_tree::AGENT_TREE.lock();
        let (is_bg, desc) = tree
            .get(agent_id)
            .map(|n| (n.is_background, n.description.clone()))
            .unwrap_or((true, "unknown".to_string()));
        drop(tree);
        if !is_bg {
            return None;
        }

        let result_text = cc_engine::agent::supervisor::output_for_agent(agent_id)
            .map(|task| task.output)
            .filter(|output| !output.is_empty())
            .unwrap_or_else(|| result_preview.to_string());

        self.engine.pending_background_results().push(
            cc_engine::agent_runtime::CompletedBackgroundAgent {
                agent_id: agent_id.to_string(),
                description: desc.clone(),
                result_text,
                had_error,
                duration: std::time::Duration::from_millis(duration_ms),
            },
        );

        Some(BackgroundAgentCompletion {
            agent_id: agent_id.to_string(),
            description: desc,
            result_preview: result_preview.to_string(),
            had_error,
            duration_ms,
        })
    }

    fn shutdown_background_agents<'a>(&'a self, reason: &'a str) -> BoxHeadlessFuture<'a, usize> {
        Box::pin(cc_engine::agent::supervisor::shutdown_all(reason))
    }
}

fn install_root_subsystem_event_sinks(event_tx: tokio::sync::broadcast::Sender<SubsystemEvent>) {
    let (lsp_tx, mut lsp_rx) = tokio::sync::broadcast::channel(128);
    cc_lsp_service::set_event_sender(lsp_tx);
    let lsp_event_tx = event_tx.clone();
    tokio::spawn(async move {
        while let Ok(event) = lsp_rx.recv().await {
            let _ = lsp_event_tx.send(lsp_event_to_subsystem(event));
        }
    });

    cc_lsp_service::ide::set_event_sender(event_tx.clone());
    cc_ipc::agent_settings_generate::set_event_sender(event_tx.clone());

    let skills_tx = event_tx.clone();
    cc_skills::set_event_callback(move |e| {
        let adapted = match e {
            cc_skills::SkillSubsystemEvent::SkillsLoaded { count } => SubsystemEvent::Skill(
                cc_ipc_protocol::subsystem_events::SkillEvent::SkillsLoaded { count },
            ),
        };
        let _ = skills_tx.send(adapted);
    });

    let mcp_tx = event_tx.clone();
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
                SubsystemEvent::Mcp(
                    cc_ipc_protocol::subsystem_events::McpEvent::ServerStateChanged {
                        server_name,
                        state,
                        error,
                    },
                )
            }
            cc_mcp::McpSubsystemEvent::ToolsDiscovered { server_name, tools } => {
                SubsystemEvent::Mcp(
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
            } => SubsystemEvent::Mcp(
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
            } => SubsystemEvent::Mcp(
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
        tokio::spawn(async move {
            manager.lock().await.set_event_sink(Some(mcp_sink));
        });
    }

    let plugin_tx = event_tx.clone();
    cc_plugins::set_event_sink(Some(Arc::new(move |event| {
        let adapted = match event {
            cc_plugins::PluginSubsystemEvent::Reloaded { count, had_error } => {
                SubsystemEvent::Plugin(cc_ipc_protocol::subsystem_events::PluginEvent::Reloaded {
                    count,
                    had_error,
                })
            }
            cc_plugins::PluginSubsystemEvent::RefreshNeeded { reason } => SubsystemEvent::Plugin(
                cc_ipc_protocol::subsystem_events::PluginEvent::RefreshNeeded { reason },
            ),
            cc_plugins::PluginSubsystemEvent::StatusChanged {
                plugin_id,
                name,
                status,
                error,
            } => SubsystemEvent::Plugin(
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

    cc_mcp::discovery::set_plugin_hook(cc_plugins::discover_plugin_mcp_servers);
    cc_mcp::discovery::set_scoped_plugin_hook(cc_plugins::discover_plugin_mcp_servers_scoped);
    cc_mcp::discovery::set_ide_hook(cc_lsp_service::ide::selected_ide_mcp_config);
}

fn lsp_event_to_subsystem(event: cc_lsp_service::LspEvent) -> SubsystemEvent {
    use cc_ipc_protocol::subsystem_events::LspEvent;
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
