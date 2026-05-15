//! Binary-only adapters for engine-owned Agent runtime hooks.

use std::sync::{Arc, Once};

use anyhow::Result;
use async_trait::async_trait;
use cc_engine::agent_runtime::{
    AgentTaskStore, AgentToolRegistry, DashboardEmitter, TeammateSpawner,
};
use cc_tasks::{TaskCreateOptions, TaskEntry, TaskRuntimeHandle, TaskStatus};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use cc_engine::types::tool::Tool;

static INSTALL: Once = Once::new();

pub fn install() {
    INSTALL.call_once(|| {
        let mut adapters = cc_engine::agent_runtime::agent_runtime_adapters();
        adapters.dashboard = Arc::new(RootDashboardEmitter);
        adapters.tools = Arc::new(RootAgentToolRegistry);
        adapters.teammate_spawner = Arc::new(RootTeammateSpawner);
        adapters.task_store = Arc::new(RootAgentTaskStore);
        cc_engine::agent_runtime::set_agent_runtime_adapters(adapters);
    });
}

struct RootDashboardEmitter;

impl DashboardEmitter for RootDashboardEmitter {
    fn emit_subagent_event(
        &self,
        kind: &str,
        agent_id: &str,
        parent_agent_id: Option<&str>,
        description: Option<&str>,
        model: Option<&str>,
        depth: usize,
        background: bool,
        payload: Option<Value>,
    ) -> Result<()> {
        crate::dashboard::emit_subagent_event(
            kind,
            agent_id,
            parent_agent_id,
            description,
            model,
            depth,
            background,
            payload,
        )
    }
}

struct RootAgentToolRegistry;

impl AgentToolRegistry for RootAgentToolRegistry {
    fn get_all_tools(&self) -> Vec<Arc<dyn cc_engine::types::tool::Tool>> {
        crate::tools::registry::get_all_tools()
    }
}

struct RootTeammateSpawner;

#[async_trait]
impl TeammateSpawner for RootTeammateSpawner {
    async fn spawn(
        &self,
        input: Value,
        ctx: &cc_engine::types::tool::ToolUseContext,
        parent: &cc_types::message::AssistantMessage,
        on_progress: Option<Box<dyn Fn(cc_engine::types::tool::ToolProgress) + Send + Sync>>,
    ) -> Result<cc_engine::types::tool::ToolResult> {
        crate::teams::team_spawn::TeamSpawnTool
            .call(input, ctx, parent, on_progress)
            .await
    }
}

struct RootAgentTaskStore;

impl AgentTaskStore for RootAgentTaskStore {
    fn try_create_with_options(
        &self,
        subject: &str,
        description: &str,
        options: TaskCreateOptions,
    ) -> Result<TaskEntry> {
        crate::tasks::global_store().try_create_with_options(subject, description, options)
    }

    fn try_update_status(&self, id: &str, status: TaskStatus) -> Result<Option<TaskEntry>> {
        crate::tasks::global_store().try_update_status(id, status)
    }

    fn register_runtime_handle(&self, id: &str, cancellation_token: CancellationToken) -> bool {
        crate::tasks::global_store().register_runtime_handle(id, cancellation_token)
    }

    fn append_output(&self, id: &str, output: &str) -> Option<TaskEntry> {
        crate::tasks::global_store().append_output(id, output)
    }

    fn try_stop(&self, id: &str) -> Result<Option<TaskEntry>> {
        crate::tasks::global_store().try_stop(id)
    }

    fn get_by_agent_id(&self, agent_id: &str) -> Option<TaskEntry> {
        crate::tasks::global_store().get_by_agent_id(agent_id)
    }

    fn unregister_runtime_handle(&self, id: &str) -> Option<TaskRuntimeHandle> {
        crate::tasks::global_store().unregister_runtime_handle(id)
    }
}
