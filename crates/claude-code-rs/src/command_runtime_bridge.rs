use std::sync::Arc;

use cc_commands::CommandContext;
use gateway::{AdapterProvider, AdapterStatus, RunEvent, RunId, RunMeta};

pub(crate) fn install_command_runtime_providers() {
    cc_commands::runtime::set_runtime_installer(crate::app_runtime_adapters::ensure_installed);
    cc_commands::runtime::set_lsp_runtime_providers(
        cc_ipc::subsystem_handlers::build_lsp_server_info_list,
        cc_ipc::subsystem_handlers::load_lsp_recommendation_settings,
    );
    cc_commands::runtime::set_agent_runtime_providers(
        builtin_agent_entries_for_commands,
        builtin_agent_prompt_for_commands,
    );
    cc_commands::runtime::set_task_runtime_providers(
        tool_tasks_for_commands,
        get_tool_task_for_commands,
        stop_tool_task_for_commands,
        delete_tool_task_for_commands,
        team_task_snapshots_for_commands,
    );
    cc_commands::runtime::set_team_command_executor(team_command_for_commands);
    cc_commands::runtime::set_team_context_for_session_provider(team_context_for_session);
    cc_commands::runtime::set_command_metadata_provider(command_metadata_for_commands);
    cc_commands::runtime::set_worktree_status_provider(
        crate::ui::status_line_resolver::current_worktree_status,
    );
    cc_commands::runtime::set_remote_daemon_status_provider(remote_daemon_status_for_commands);
    cc_commands::runtime::set_remote_token_path_provider(
        cc_daemon::process_state::control_token_path,
    );
    cc_commands::runtime::set_tool_policy_names_provider(tool_policy_names_for_commands);
    cc_commands::runtime::set_tool_list_provider(all_tools_for_commands);
    cc_commands::runtime::set_fork_runner(fork_runner_for_commands);

    cc_commands::copy::set_clipboard_copy_provider(
        crate::ui::clipboard_text::copy_text_to_clipboard,
    );
    cc_commands::logout::set_onboarding_logout_clearer(onboarding_logout_clear_for_commands);
    cc_commands::skills_cmd::set_plugin_skills_provider(discover_plugin_skills_for_commands);
    cc_commands::ide_cmd::set_ide_command_runtime(cc_commands::ide_cmd::IdeCommandRuntime {
        detect_ides: cc_lsp_service::ide::detect_ides,
        selected_ide: cc_lsp_service::ide::selected_ide,
        select_ide: cc_lsp_service::ide::select_ide,
        clear_selection: cc_lsp_service::ide::clear_selection,
        reconnect_selected: cc_lsp_service::ide::reconnect_selected,
    });
    cc_commands::plugin_cmd::set_plugin_command_runtime(
        cc_commands::plugin_cmd::PluginCommandRuntime {
            load_installed_plugins: cc_plugins::loader::load_installed_plugins,
            save_installed_plugins: cc_plugins::loader::save_installed_plugins,
            get_all_plugins: cc_plugins::get_all_plugins,
            needs_refresh: cc_plugins::needs_refresh,
            find_plugin: cc_plugins::find_plugin,
            set_plugin_status: cc_plugins::set_plugin_status,
            register_plugin: cc_plugins::register_plugin,
            emit_event_external: emit_plugin_event_external_for_commands,
            uninstall_plugin: cc_plugins::uninstall_plugin,
        },
    );
    cc_commands::reload_plugins_cmd::set_reload_plugins_runtime(
        cc_commands::reload_plugins_cmd::ReloadPluginsRuntime {
            reload_plugins: reload_plugins_for_commands,
            discover_plugin_skills: discover_plugin_skills_for_commands,
        },
    );
    cc_commands::brief::set_brief_command_runtime(cc_commands::brief::BriefCommandRuntime {
        clear_prompt_cache: cc_engine::prompt_sections::clear_cache,
    });
    cc_commands::daemon_cmd::set_daemon_command_runtime(
        cc_commands::daemon_cmd::DaemonCommandRuntime {
            status_snapshot: daemon_status_snapshot_for_commands,
            state_path: cc_daemon::process_state::state_path,
            request_shutdown: cc_daemon::process_state::request_shutdown,
        },
    );
    cc_commands::sleep_cmd::set_sleep_command_runtime(
        cc_commands::sleep_cmd::SleepCommandRuntime {
            write_sleep_state: sleep_state_for_commands,
        },
    );
    cc_commands::remote_cmd::set_remote_gateway_adapter(
        cc_commands::remote_cmd::RemoteGatewayAdapter {
            capabilities: remote_capabilities_for_commands,
            adapters: remote_adapters_for_commands,
            connect_adapter: remote_connect_adapter_for_commands,
            test_adapter_message: remote_test_adapter_message_for_commands,
            show_run: remote_show_run_for_commands,
            run_events: remote_run_events_for_commands,
            stop_run: remote_stop_run_for_commands,
        },
    );
    cc_commands::install_engine_command_executor();
}

fn builtin_agent_entries_for_commands() -> Vec<cc_commands::runtime::BuiltinAgentEntry> {
    cc_engine::agent_runtime::builtin_agent_entries()
        .into_iter()
        .map(|entry| cc_commands::runtime::BuiltinAgentEntry {
            name: entry.name,
            description: entry.description,
        })
        .collect()
}

fn builtin_agent_prompt_for_commands(name: &str) -> Option<String> {
    cc_engine::agent_runtime::builtin_agent_prompt(name)
}

fn tool_tasks_for_commands() -> Vec<cc_tasks::TaskEntry> {
    cc_tasks::global_store().list()
}

fn get_tool_task_for_commands(id: &str) -> Option<cc_tasks::TaskEntry> {
    cc_tasks::global_store().get(id)
}

fn stop_tool_task_for_commands(id: &str) -> Result<Option<cc_tasks::TaskEntry>, String> {
    cc_tasks::global_store()
        .try_stop(id)
        .map_err(|err| err.to_string())
}

fn delete_tool_task_for_commands(id: &str) -> Result<Option<cc_tasks::TaskEntry>, String> {
    cc_tasks::global_store()
        .try_delete(id)
        .map_err(|err| err.to_string())
}

fn team_task_snapshots_for_commands() -> Vec<cc_commands::runtime::TeamTaskSnapshot> {
    cc_teams::in_process::InProcessBackend::task_snapshots()
        .into_iter()
        .map(|snapshot| cc_commands::runtime::TeamTaskSnapshot {
            id: snapshot.id,
            agent_id: snapshot.agent_id,
            agent_name: snapshot.agent_name,
            team_name: snapshot.team_name,
            status: match snapshot.status {
                cc_teams::types::TaskStatus::Running => {
                    cc_commands::runtime::TeamTaskStatus::Running
                }
                cc_teams::types::TaskStatus::Stopped => {
                    cc_commands::runtime::TeamTaskStatus::Stopped
                }
                cc_teams::types::TaskStatus::Completed => {
                    cc_commands::runtime::TeamTaskStatus::Completed
                }
            },
            is_idle: snapshot.is_idle,
            has_error: snapshot.has_error,
            error_message: snapshot.error_message,
            prompt: snapshot.prompt,
            model: snapshot.model,
            awaiting_plan_approval: snapshot.awaiting_plan_approval,
            permission_mode: snapshot.permission_mode.as_str().to_string(),
        })
        .collect()
}

fn team_command_for_commands<'a>(
    args: &'a str,
    ctx: &'a mut CommandContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
    Box::pin(cc_teams::command::execute_team_command(args, ctx))
}

fn team_context_for_session(session_id: &str) -> Option<cc_types::teams::TeamContext> {
    cc_teams::reconnection::restore_team_context_for_session(session_id)
        .map_err(|err| {
            tracing::warn!(
                session_id,
                error = %err,
                "failed to restore team context for session"
            );
            err
        })
        .ok()
        .flatten()
}

fn command_metadata_for_commands() -> Vec<cc_commands::CommandMetadata> {
    cc_commands::command_metadata(&cc_commands::get_all_commands())
}

fn all_tools_for_commands() -> cc_engine::types::tool::Tools {
    cc_tools::registry::get_all_tools()
}

fn tool_policy_names_for_commands(policy: cc_commands::runtime::CommandToolPolicy) -> Vec<String> {
    let root_policy = match policy {
        cc_commands::runtime::CommandToolPolicy::DefaultAgent => {
            cc_tools::registry::ToolPolicy::DefaultAgent
        }
        cc_commands::runtime::CommandToolPolicy::Coordinator => {
            cc_tools::registry::ToolPolicy::Coordinator
        }
    };
    cc_tools::registry::get_tools_for_policy(root_policy)
        .iter()
        .map(|tool| tool.name().to_string())
        .collect()
}

fn onboarding_logout_clear_for_commands() -> cc_commands::logout::StepStatus {
    let store = cc_services::onboarding::OnboardingStore::open_default();
    let had_state_before = match store.load() {
        Ok(state) => !state.is_first_run() || store.path().exists(),
        Err(_) => store.path().exists(),
    };
    if !had_state_before {
        return cc_commands::logout::StepStatus::NoOp;
    }
    match store.update(|state| state.reset_for_logout()) {
        Ok(_) => cc_commands::logout::StepStatus::Cleared,
        Err(error) => cc_commands::logout::StepStatus::Failed(error.to_string()),
    }
}

fn fork_runner_for_commands(
    params: cc_commands::runtime::CommandForkParams,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = anyhow::Result<cc_commands::runtime::CommandForkOutcome>>
            + Send
            + 'static,
    >,
> {
    Box::pin(async move {
        let outcome = cc_engine::agent::fork::run_fork(cc_engine::agent::fork::ForkParams {
            prompt: params.prompt,
            cwd: params.cwd,
            model: params.model,
            fallback_model: params.fallback_model,
            tools: params.tools,
            max_turns: params.max_turns,
            parent_messages: params.parent_messages,
            append_system_prompt: params.append_system_prompt,
            custom_system_prompt: params.custom_system_prompt,
            hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
            command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
        })
        .await?;

        Ok(cc_commands::runtime::CommandForkOutcome {
            text: outcome.text,
            had_error: outcome.had_error,
            duration_ms: outcome.duration_ms,
            agent_id: outcome.agent_id,
        })
    })
}

fn reload_plugins_for_commands() -> cc_commands::reload_plugins_cmd::ReloadReport {
    let report = cc_plugins::reload_plugins();
    cc_commands::reload_plugins_cmd::ReloadReport {
        count: report.count,
        error_count: report.error_count,
        errors: report.errors,
        global_errors: report.global_errors,
        duration_ms: report.duration_ms,
    }
}

fn discover_plugin_skills_for_commands() -> Vec<cc_skills::SkillDefinition> {
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

fn emit_plugin_event_external_for_commands(
    event: cc_ipc_protocol::subsystem_events::SubsystemEvent,
) {
    use cc_ipc_protocol::subsystem_events::{PluginEvent, SubsystemEvent};

    let SubsystemEvent::Plugin(event) = event else {
        return;
    };
    let adapted = match event {
        PluginEvent::Reloaded { count, had_error } => {
            cc_plugins::PluginSubsystemEvent::Reloaded { count, had_error }
        }
        PluginEvent::RefreshNeeded { reason } => {
            cc_plugins::PluginSubsystemEvent::RefreshNeeded { reason }
        }
        PluginEvent::StatusChanged {
            plugin_id,
            name,
            status,
            error,
        } => cc_plugins::PluginSubsystemEvent::StatusChanged {
            plugin_id,
            name,
            status,
            error,
        },
        PluginEvent::PluginList { .. } => return,
    };
    cc_plugins::emit_event_external(adapted);
}

fn daemon_status_snapshot_for_commands(
) -> anyhow::Result<cc_commands::daemon_cmd::DaemonStatusSnapshot> {
    Ok(match cc_daemon::process_state::status_snapshot()? {
        cc_daemon::process_state::DaemonStatusSnapshot::Running(state) => {
            cc_commands::daemon_cmd::DaemonStatusSnapshot::Running(map_daemon_state(state))
        }
        cc_daemon::process_state::DaemonStatusSnapshot::Stale(state) => {
            cc_commands::daemon_cmd::DaemonStatusSnapshot::Stale(map_daemon_state(state))
        }
        cc_daemon::process_state::DaemonStatusSnapshot::Stopped => {
            cc_commands::daemon_cmd::DaemonStatusSnapshot::Stopped
        }
    })
}

fn map_daemon_state(
    state: cc_daemon::process_state::DaemonProcessState,
) -> cc_commands::daemon_cmd::DaemonProcessState {
    cc_commands::daemon_cmd::DaemonProcessState {
        pid: state.pid,
        health_url: state.health_url,
        workers: state
            .workers
            .into_iter()
            .map(|worker| cc_commands::daemon_cmd::DaemonWorkerSummary {
                worker_id: worker.worker_id,
                kind: worker.kind,
                pid: worker.pid,
                status: worker.status,
                updated_at: worker.updated_at,
            })
            .collect(),
    }
}

fn sleep_state_for_commands(
    duration_seconds: u64,
    reason: &str,
) -> anyhow::Result<cc_commands::sleep_cmd::DaemonSleepState> {
    let state = cc_daemon::process_state::write_sleep_state(duration_seconds, reason)?;
    Ok(cc_commands::sleep_cmd::DaemonSleepState {
        sleeping_until: state.sleeping_until,
    })
}

fn remote_daemon_status_for_commands(
) -> Result<cc_commands::remote_cmd::LocalGatewayDaemonStatus, String> {
    cc_daemon::gateway_client::LocalGatewayClient::daemon_status()
        .map(map_remote_daemon_status)
        .map_err(|error| error.to_string())
}

fn map_remote_daemon_status(
    status: cc_daemon::gateway_client::LocalGatewayDaemonStatus,
) -> cc_commands::remote_cmd::LocalGatewayDaemonStatus {
    match status {
        cc_daemon::gateway_client::LocalGatewayDaemonStatus::Running {
            pid,
            base_url,
            health_url,
        } => cc_commands::remote_cmd::LocalGatewayDaemonStatus::Running {
            pid,
            base_url,
            health_url,
        },
        cc_daemon::gateway_client::LocalGatewayDaemonStatus::Stale { pid } => {
            cc_commands::remote_cmd::LocalGatewayDaemonStatus::Stale { pid }
        }
        cc_daemon::gateway_client::LocalGatewayDaemonStatus::Stopped => {
            cc_commands::remote_cmd::LocalGatewayDaemonStatus::Stopped
        }
    }
}

fn remote_capabilities_for_commands(
) -> cc_commands::remote_cmd::RemoteFuture<cc_commands::remote_cmd::GatewayCapabilitiesSnapshot> {
    Box::pin(async {
        let client = cc_daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        let cap = client.capabilities().await?;
        Ok(cc_commands::remote_cmd::GatewayCapabilitiesSnapshot {
            version: cap.version,
            auth_mode: cap.auth_mode,
            supports_steer: cap.supports_steer,
            max_running: cap.max_running,
            max_queued: cap.max_queued,
            endpoints: cap.endpoints,
        })
    })
}

fn remote_adapters_for_commands() -> cc_commands::remote_cmd::RemoteFuture<Vec<AdapterStatus>> {
    Box::pin(async {
        let client = cc_daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.adapters().await
    })
}

fn remote_connect_adapter_for_commands(
    provider: AdapterProvider,
) -> cc_commands::remote_cmd::RemoteFuture<AdapterStatus> {
    Box::pin(async move {
        let client = cc_daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.connect_adapter(provider).await
    })
}

fn remote_test_adapter_message_for_commands(
    provider: AdapterProvider,
    target: String,
    text: String,
) -> cc_commands::remote_cmd::RemoteFuture<AdapterStatus> {
    Box::pin(async move {
        let client = cc_daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.test_adapter_message(provider, target, text).await
    })
}

fn remote_show_run_for_commands(run_id: RunId) -> cc_commands::remote_cmd::RemoteFuture<RunMeta> {
    Box::pin(async move {
        let client = cc_daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.show_run(&run_id).await
    })
}

fn remote_run_events_for_commands(
    run_id: RunId,
) -> cc_commands::remote_cmd::RemoteFuture<Vec<RunEvent>> {
    Box::pin(async move {
        let client = cc_daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.run_events(&run_id).await
    })
}

fn remote_stop_run_for_commands(
    run_id: RunId,
) -> cc_commands::remote_cmd::RemoteFuture<cc_commands::remote_cmd::GatewayRunActionResponse> {
    Box::pin(async move {
        let client = cc_daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        let response = client.stop_run(&run_id).await?;
        Ok(cc_commands::remote_cmd::GatewayRunActionResponse {
            run_id: response.run_id,
            status: response.status,
            action: response.action,
            diagnostic: response.diagnostic,
        })
    })
}
