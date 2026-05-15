use std::sync::Arc;

use cc_commands::CommandContext;
use gateway::{AdapterProvider, AdapterStatus, RunEvent, RunId, RunMeta};

pub(super) fn install_command_runtime_providers() {
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
    cc_commands::runtime::set_command_metadata_provider(command_metadata_for_commands);
    cc_commands::runtime::set_worktree_status_provider(
        crate::ui::status_line_resolver::current_worktree_status,
    );
    cc_commands::runtime::set_remote_daemon_status_provider(remote_daemon_status_for_commands);
    cc_commands::runtime::set_remote_token_path_provider(
        crate::daemon::process_state::control_token_path,
    );
    cc_commands::runtime::set_tool_policy_names_provider(tool_policy_names_for_commands);
    cc_commands::runtime::set_tool_list_provider(crate::tools::registry::get_all_tools);
    cc_commands::runtime::set_fork_runner(fork_runner_for_commands);

    cc_commands::copy::set_clipboard_copy_provider(
        crate::ui::clipboard_text::copy_text_to_clipboard,
    );
    cc_commands::logout::set_onboarding_logout_clearer(onboarding_logout_clear_for_commands);
    cc_commands::skills_cmd::set_plugin_skills_provider(crate::plugins::discover_plugin_skills);
    cc_commands::ide_cmd::set_ide_command_runtime(cc_commands::ide_cmd::IdeCommandRuntime {
        detect_ides: crate::ide::detect_ides,
        selected_ide: crate::ide::selected_ide,
        select_ide: crate::ide::select_ide,
        clear_selection: crate::ide::clear_selection,
        reconnect_selected: crate::ide::reconnect_selected,
    });
    cc_commands::plugin_cmd::set_plugin_command_runtime(
        cc_commands::plugin_cmd::PluginCommandRuntime {
            load_installed_plugins: crate::plugins::loader::load_installed_plugins,
            save_installed_plugins: crate::plugins::loader::save_installed_plugins,
            get_all_plugins: crate::plugins::get_all_plugins,
            needs_refresh: crate::plugins::needs_refresh,
            find_plugin: crate::plugins::find_plugin,
            set_plugin_status: crate::plugins::set_plugin_status,
            register_plugin: crate::plugins::register_plugin,
            emit_event_external: crate::plugins::emit_event_external,
            uninstall_plugin: crate::plugins::uninstall_plugin,
        },
    );
    cc_commands::reload_plugins_cmd::set_reload_plugins_runtime(
        cc_commands::reload_plugins_cmd::ReloadPluginsRuntime {
            reload_plugins: reload_plugins_for_commands,
            discover_plugin_skills: crate::plugins::discover_plugin_skills,
        },
    );
    cc_commands::brief::set_brief_command_runtime(cc_commands::brief::BriefCommandRuntime {
        clear_prompt_cache: cc_engine::prompt_sections::clear_cache,
    });
    cc_commands::daemon_cmd::set_daemon_command_runtime(
        cc_commands::daemon_cmd::DaemonCommandRuntime {
            status_snapshot: daemon_status_snapshot_for_commands,
            state_path: crate::daemon::process_state::state_path,
            request_shutdown: crate::daemon::process_state::request_shutdown,
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
}

fn builtin_agent_entries_for_commands() -> Vec<cc_commands::runtime::BuiltinAgentEntry> {
    crate::ipc::builtin_agents::builtin_agent_entries()
        .into_iter()
        .map(|entry| cc_commands::runtime::BuiltinAgentEntry {
            name: entry.name,
            description: entry.description,
        })
        .collect()
}

fn builtin_agent_prompt_for_commands(name: &str) -> Option<String> {
    crate::ipc::builtin_agents::builtin_agent_prompt(name).map(ToOwned::to_owned)
}

fn tool_tasks_for_commands() -> Vec<cc_tasks::TaskEntry> {
    crate::tasks::global_store().list()
}

fn get_tool_task_for_commands(id: &str) -> Option<cc_tasks::TaskEntry> {
    crate::tasks::global_store().get(id)
}

fn stop_tool_task_for_commands(id: &str) -> Result<Option<cc_tasks::TaskEntry>, String> {
    crate::tasks::global_store()
        .try_stop(id)
        .map_err(|err| err.to_string())
}

fn delete_tool_task_for_commands(id: &str) -> Result<Option<cc_tasks::TaskEntry>, String> {
    crate::tasks::global_store()
        .try_delete(id)
        .map_err(|err| err.to_string())
}

fn team_task_snapshots_for_commands() -> Vec<cc_commands::runtime::TeamTaskSnapshot> {
    crate::teams::in_process::InProcessBackend::task_snapshots()
        .into_iter()
        .map(|snapshot| cc_commands::runtime::TeamTaskSnapshot {
            id: snapshot.id,
            agent_id: snapshot.agent_id,
            agent_name: snapshot.agent_name,
            team_name: snapshot.team_name,
            status: match snapshot.status {
                crate::teams::types::TaskStatus::Running => {
                    cc_commands::runtime::TeamTaskStatus::Running
                }
                crate::teams::types::TaskStatus::Stopped => {
                    cc_commands::runtime::TeamTaskStatus::Stopped
                }
                crate::teams::types::TaskStatus::Completed => {
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
    Box::pin(crate::teams::command::execute_team_command(args, ctx))
}

fn command_metadata_for_commands() -> Vec<cc_commands::CommandMetadata> {
    cc_commands::command_metadata(&super::get_all_commands())
}

fn tool_policy_names_for_commands(policy: cc_commands::runtime::CommandToolPolicy) -> Vec<String> {
    let root_policy = match policy {
        cc_commands::runtime::CommandToolPolicy::DefaultAgent => {
            crate::tools::registry::ToolPolicy::DefaultAgent
        }
        cc_commands::runtime::CommandToolPolicy::Coordinator => {
            crate::tools::registry::ToolPolicy::Coordinator
        }
    };
    crate::tools::registry::get_tools_for_policy(root_policy)
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
    let report = crate::plugins::reload_plugins();
    cc_commands::reload_plugins_cmd::ReloadReport {
        count: report.count,
        error_count: report.error_count,
        errors: report.errors,
        global_errors: report.global_errors,
        duration_ms: report.duration_ms,
    }
}

fn daemon_status_snapshot_for_commands(
) -> anyhow::Result<cc_commands::daemon_cmd::DaemonStatusSnapshot> {
    Ok(match crate::daemon::process_state::status_snapshot()? {
        crate::daemon::process_state::DaemonStatusSnapshot::Running(state) => {
            cc_commands::daemon_cmd::DaemonStatusSnapshot::Running(map_daemon_state(state))
        }
        crate::daemon::process_state::DaemonStatusSnapshot::Stale(state) => {
            cc_commands::daemon_cmd::DaemonStatusSnapshot::Stale(map_daemon_state(state))
        }
        crate::daemon::process_state::DaemonStatusSnapshot::Stopped => {
            cc_commands::daemon_cmd::DaemonStatusSnapshot::Stopped
        }
    })
}

fn map_daemon_state(
    state: crate::daemon::process_state::DaemonProcessState,
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
    let state = crate::daemon::process_state::write_sleep_state(duration_seconds, reason)?;
    Ok(cc_commands::sleep_cmd::DaemonSleepState {
        sleeping_until: state.sleeping_until,
    })
}

fn remote_daemon_status_for_commands(
) -> Result<cc_commands::remote_cmd::LocalGatewayDaemonStatus, String> {
    crate::daemon::gateway_client::LocalGatewayClient::daemon_status()
        .map(map_remote_daemon_status)
        .map_err(|error| error.to_string())
}

fn map_remote_daemon_status(
    status: crate::daemon::gateway_client::LocalGatewayDaemonStatus,
) -> cc_commands::remote_cmd::LocalGatewayDaemonStatus {
    match status {
        crate::daemon::gateway_client::LocalGatewayDaemonStatus::Running {
            pid,
            base_url,
            health_url,
        } => cc_commands::remote_cmd::LocalGatewayDaemonStatus::Running {
            pid,
            base_url,
            health_url,
        },
        crate::daemon::gateway_client::LocalGatewayDaemonStatus::Stale { pid } => {
            cc_commands::remote_cmd::LocalGatewayDaemonStatus::Stale { pid }
        }
        crate::daemon::gateway_client::LocalGatewayDaemonStatus::Stopped => {
            cc_commands::remote_cmd::LocalGatewayDaemonStatus::Stopped
        }
    }
}

fn remote_capabilities_for_commands(
) -> cc_commands::remote_cmd::RemoteFuture<cc_commands::remote_cmd::GatewayCapabilitiesSnapshot> {
    Box::pin(async {
        let client = crate::daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
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
        let client = crate::daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.adapters().await
    })
}

fn remote_connect_adapter_for_commands(
    provider: AdapterProvider,
) -> cc_commands::remote_cmd::RemoteFuture<AdapterStatus> {
    Box::pin(async move {
        let client = crate::daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.connect_adapter(provider).await
    })
}

fn remote_test_adapter_message_for_commands(
    provider: AdapterProvider,
    target: String,
    text: String,
) -> cc_commands::remote_cmd::RemoteFuture<AdapterStatus> {
    Box::pin(async move {
        let client = crate::daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.test_adapter_message(provider, target, text).await
    })
}

fn remote_show_run_for_commands(run_id: RunId) -> cc_commands::remote_cmd::RemoteFuture<RunMeta> {
    Box::pin(async move {
        let client = crate::daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.show_run(&run_id).await
    })
}

fn remote_run_events_for_commands(
    run_id: RunId,
) -> cc_commands::remote_cmd::RemoteFuture<Vec<RunEvent>> {
    Box::pin(async move {
        let client = crate::daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        client.run_events(&run_id).await
    })
}

fn remote_stop_run_for_commands(
    run_id: RunId,
) -> cc_commands::remote_cmd::RemoteFuture<cc_commands::remote_cmd::GatewayRunActionResponse> {
    Box::pin(async move {
        let client = crate::daemon::gateway_client::LocalGatewayClient::from_running_daemon()?;
        let response = client.stop_run(&run_id).await?;
        Ok(cc_commands::remote_cmd::GatewayRunActionResponse {
            run_id: response.run_id,
            status: response.status,
            action: response.action,
            diagnostic: response.diagnostic,
        })
    })
}
