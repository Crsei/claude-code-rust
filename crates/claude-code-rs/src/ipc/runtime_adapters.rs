//! Host adapters that bind the extracted `cc-ipc` runtime facade to this binary.

use std::path::Path;
use std::sync::{Arc, Once};

use cc_ipc::agent_handlers::{AgentRuntimeHost, AgentTaskOutput};
use cc_ipc::subsystem_handlers::{
    BoxRuntimeFuture, McpRuntimeOperation, McpRuntimeReport, SubsystemRuntimeHost,
};
use cc_ipc_protocol::protocol::BackendMessage;
use cc_ipc_protocol::subsystem_events::{
    IdeCommand, LspCommand, McpCommand, PluginCommand, SkillCommand,
};
use cc_ipc_protocol::subsystem_types::*;
use cc_types::agent_types::TeamMemberInfo;

static INSTALL: Once = Once::new();

pub fn ensure_installed() {
    INSTALL.call_once(|| {
        cc_ipc::agent_handlers::set_runtime_host(Arc::new(RootAgentHost));
        cc_ipc::subsystem_handlers::set_runtime_host(Arc::new(RootSubsystemHost));
        cc_engine::agent_runtime::set_agent_tree_runtime(Arc::new(RootAgentTreeRuntime));
    });
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
        crate::engine::agent::supervisor::cancel_agent(agent_id)
    }

    fn agent_output(&self, agent_id: &str) -> Option<AgentTaskOutput> {
        crate::engine::agent::supervisor::output_for_agent(agent_id).map(|task| AgentTaskOutput {
            id: task.id,
            output: task.output,
        })
    }

    fn write_team_message(&self, team_name: &str, to: &str, text: &str) -> Result<(), String> {
        let msg = crate::teams::types::TeammateMessage {
            from: "__frontend__".to_string(),
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            read: false,
            color: None,
            summary: None,
        };
        crate::teams::mailbox::write_to_mailbox(to, msg, team_name).map_err(|e| e.to_string())
    }

    fn team_members(&self, team_name: &str) -> Result<Vec<TeamMemberInfo>, String> {
        let tf = crate::teams::helpers::read_team_file(team_name).map_err(|e| e.to_string())?;
        Ok(tf
            .members
            .iter()
            .map(|m| TeamMemberInfo {
                agent_id: m.agent_id.clone(),
                agent_name: m.name.clone(),
                role: m.agent_type.clone(),
                is_active: m.is_active.unwrap_or(true),
                unread_messages: crate::teams::mailbox::read_unread_messages(&m.name, team_name)
                    .map(|v| v.len())
                    .unwrap_or(0),
            })
            .collect())
    }
}

struct RootSubsystemHost;

impl SubsystemRuntimeHost for RootSubsystemHost {
    fn handle_lsp_command(&self, cmd: LspCommand) -> Vec<BackendMessage> {
        crate::ipc::subsystem_handlers::handle_lsp_command(cmd)
    }

    fn handle_mcp_command(&self, cmd: McpCommand) -> Vec<BackendMessage> {
        crate::ipc::subsystem_handlers::handle_mcp_command(cmd)
    }

    fn handle_mcp_command_with_runtime<'a>(
        &'a self,
        cmd: McpCommand,
        cwd: &'a Path,
    ) -> BoxRuntimeFuture<'a, Vec<BackendMessage>> {
        Box::pin(crate::ipc::subsystem_handlers::handle_mcp_command_with_runtime(cmd, cwd))
    }

    fn handle_plugin_command(&self, cmd: PluginCommand) -> Vec<BackendMessage> {
        crate::ipc::subsystem_handlers::handle_plugin_command(cmd)
    }

    fn handle_skill_command(&self, cmd: SkillCommand) -> Vec<BackendMessage> {
        crate::ipc::subsystem_handlers::handle_skill_command(cmd)
    }

    fn handle_ide_command(&self, cmd: IdeCommand) -> Vec<BackendMessage> {
        crate::ipc::subsystem_handlers::handle_ide_command(cmd)
    }

    fn build_subsystem_status_snapshot(&self) -> SubsystemStatusSnapshot {
        crate::ipc::subsystem_handlers::build_subsystem_status_snapshot()
    }

    fn build_lsp_server_info_list(&self) -> Vec<LspServerInfo> {
        crate::ipc::subsystem_handlers::build_lsp_server_info_list()
    }

    fn load_lsp_recommendation_settings(&self) -> LspRecommendationSettings {
        crate::ipc::subsystem_handlers::load_lsp_recommendation_settings()
    }

    fn build_mcp_server_info_list(&self) -> Vec<McpServerStatusInfo> {
        crate::ipc::subsystem_handlers::build_mcp_server_info_list()
    }

    fn build_mcp_server_info_list_for_cwd_async<'a>(
        &'a self,
        cwd: &'a Path,
    ) -> BoxRuntimeFuture<'a, Vec<McpServerStatusInfo>> {
        Box::pin(crate::ipc::subsystem_handlers::build_mcp_server_info_list_for_cwd_async(cwd))
    }

    fn build_mcp_server_config_entries(&self, cwd: &Path) -> Vec<McpServerConfigEntry> {
        crate::ipc::subsystem_handlers::build_mcp_server_config_entries(cwd)
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
                    crate::ipc::subsystem_handlers::McpRuntimeOperation::Connect
                }
                McpRuntimeOperation::Disconnect => {
                    crate::ipc::subsystem_handlers::McpRuntimeOperation::Disconnect
                }
                McpRuntimeOperation::Reconnect => {
                    crate::ipc::subsystem_handlers::McpRuntimeOperation::Reconnect
                }
            };
            let report = crate::ipc::subsystem_handlers::run_mcp_runtime_operation(
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
        crate::ipc::subsystem_handlers::build_plugin_info_list()
    }

    fn build_skill_info_list(&self) -> Vec<SkillInfo> {
        crate::ipc::subsystem_handlers::build_skill_info_list()
    }

    fn build_ide_info_list(&self) -> Vec<IdeInfo> {
        crate::ipc::subsystem_handlers::build_ide_info_list()
    }
}
