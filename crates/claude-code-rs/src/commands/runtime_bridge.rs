use cc_commands::CommandContext;

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
    crate::tools::tasks::global_store().list()
}

fn get_tool_task_for_commands(id: &str) -> Option<cc_tasks::TaskEntry> {
    crate::tools::tasks::global_store().get(id)
}

fn stop_tool_task_for_commands(id: &str) -> Result<Option<cc_tasks::TaskEntry>, String> {
    crate::tools::tasks::global_store()
        .try_stop(id)
        .map_err(|err| err.to_string())
}

fn delete_tool_task_for_commands(id: &str) -> Result<Option<cc_tasks::TaskEntry>, String> {
    crate::tools::tasks::global_store()
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
