use std::collections::HashSet;
use std::sync::Arc;

use tracing::warn;

use crate::types::tool::Tools;

use super::ask_user::AskUserQuestionTool;
use super::brief::BriefTool;
use super::config_tool::ConfigTool;
use super::lsp::LspTool;
use super::plan_mode::{EnterPlanModeTool, ExitPlanModeTool};
use super::pr_activity::{SubscribePrActivityTool, UnsubscribePrActivityTool};
use super::send_message::SendMessageTool;
use super::send_user_message::SendUserMessageTool;
use super::skill::SkillTool;
use super::structured_output::StructuredOutputTool;
use super::system_status::SystemStatusTool;
use super::tasks::{
    TaskCreateTool, TaskGetTool, TaskListTool, TaskOutputTool, TaskStopTool, TaskUpdateTool,
    TodoWriteTool,
};
use super::team_spawn::TeamSpawnTool;
use super::tool_search::ToolSearchTool;
use super::web_fetch::WebFetchTool;
use super::web_search::WebSearchTool;
use super::worktree::{EnterWorktreeTool, ExitWorktreeTool};
use super::{exec, fs};

/// Runtime tool visibility profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    /// Default interactive/session tool pool.
    DefaultAgent,
    /// Coordinator lead tool pool. The lead delegates via Agent, communicates
    /// through mailboxes, and can inspect/stop worker tasks.
    Coordinator,
    /// Dedicated coordinator worker pool.
    CoordinatorWorker,
    /// Generic in-process teammate pool. This intentionally excludes internal
    /// team orchestration tools so teammates cannot recursively coordinate.
    InProcessTeammate,
}

/// Get all base tool instances.
///
/// Corresponds to TypeScript: tools.ts `getAllBaseTools()`.
/// Returns all tool implementations. The caller can filter by `is_enabled()`.
///
/// Structure:
/// 1. Start from each sub-domain's aggregator (`fs::tools()`, `exec::tools()`).
/// 2. Append single-tool / small-cluster modules that have not yet been
///    promoted into a sub-domain.
///
/// When adding a new tool, prefer adding it to an existing sub-domain's
/// `tools()` rather than listing it individually here. See
/// `src/tools/ARCHITECTURE.md` for placement rules.
fn base_tools() -> Tools {
    let mut tools: Tools = Tools::new();

    // Domain-grouped tools — each sub-domain owns its own list.
    tools.extend(fs::tools());
    tools.extend(exec::tools());

    // Single-tool / small-cluster modules (not yet a sub-domain).
    tools.extend([
        Arc::new(AskUserQuestionTool) as _,
        Arc::new(crate::engine::agent::AgentTool) as _,
        Arc::new(SkillTool) as _,
        Arc::new(ConfigTool) as _,
        Arc::new(StructuredOutputTool) as _,
        Arc::new(SendUserMessageTool) as _,
        Arc::new(WebFetchTool) as _,
        Arc::new(WebSearchTool) as _,
        Arc::new(EnterPlanModeTool) as _,
        Arc::new(ExitPlanModeTool) as _,
        Arc::new(EnterWorktreeTool) as _,
        Arc::new(ExitWorktreeTool) as _,
        Arc::new(TodoWriteTool) as _,
        Arc::new(TaskCreateTool) as _,
        Arc::new(TaskGetTool) as _,
        Arc::new(TaskUpdateTool) as _,
        Arc::new(TaskListTool) as _,
        Arc::new(TaskStopTool) as _,
        Arc::new(TaskOutputTool) as _,
        Arc::new(LspTool) as _,
        Arc::new(SendMessageTool) as _,
        Arc::new(SubscribePrActivityTool) as _,
        Arc::new(UnsubscribePrActivityTool) as _,
        Arc::new(TeamSpawnTool) as _,
        Arc::new(BriefTool) as _,
        Arc::new(SystemStatusTool) as _,
        Arc::new(ToolSearchTool) as _,
    ]);

    // Filter to only enabled tools.
    tools.into_iter().filter(|t| t.is_enabled()).collect()
}

/// Get all runtime tools, including plugin-contributed tools that expose an
/// executable runtime in their plugin manifest.
pub fn get_all_tools() -> Tools {
    let mut tools = base_tools();
    let mut seen: HashSet<String> = tools.iter().map(|tool| tool.name().to_string()).collect();

    for tool in crate::plugins::discover_plugin_tools() {
        let name = tool.name().to_string();
        if seen.insert(name.clone()) {
            tools.push(tool);
        } else {
            warn!(tool = %name, "skipping plugin tool with duplicate name");
        }
    }

    tools
}

/// Get tools for the active top-level session.
pub fn get_tools_for_active_session() -> Tools {
    if crate::teams::coordinator::is_coordinator_mode_enabled() {
        get_tools_for_policy(ToolPolicy::Coordinator)
    } else {
        get_tools_for_policy(ToolPolicy::DefaultAgent)
    }
}

/// Get tools filtered for a concrete runtime policy.
pub fn get_tools_for_policy(policy: ToolPolicy) -> Tools {
    filter_tools_for_policy(get_all_tools(), policy)
}

/// Filter an existing tool set for a runtime policy.
pub fn filter_tools_for_policy(tools: Tools, policy: ToolPolicy) -> Tools {
    let Some(allowed) = allowed_tools_for_policy(policy) else {
        return tools;
    };
    tools
        .into_iter()
        .filter(|tool| allowed.iter().any(|name| *name == tool.name()))
        .collect()
}

fn allowed_tools_for_policy(policy: ToolPolicy) -> Option<&'static [&'static str]> {
    match policy {
        ToolPolicy::DefaultAgent => None,
        ToolPolicy::Coordinator => Some(&[
            "Agent",
            "SendMessage",
            "TaskList",
            "TaskStop",
            "subscribe_pr_activity",
            "unsubscribe_pr_activity",
        ]),
        ToolPolicy::CoordinatorWorker => Some(&[
            "Glob",
            "Grep",
            "Read",
            "Bash",
            "Edit",
            "Write",
            "TodoWrite",
            "TaskList",
            "TaskUpdate",
            "SendMessage",
        ]),
        ToolPolicy::InProcessTeammate => Some(&[
            "Glob",
            "Grep",
            "Read",
            "Bash",
            "Edit",
            "Write",
            "TodoWrite",
            "TaskList",
            "TaskUpdate",
            "TaskOutput",
            "SendMessage",
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_names(tools: Tools) -> Vec<String> {
        tools
            .into_iter()
            .map(|tool| tool.name().to_string())
            .collect()
    }

    #[test]
    fn test_get_all_tools_not_empty() {
        let tools = get_all_tools();
        assert!(!tools.is_empty(), "should have at least one tool");
    }

    #[test]
    fn test_find_tool_by_name() {
        let tools = get_all_tools();

        let bash = tools.iter().find(|t| t.name() == "Bash");
        assert!(bash.is_some(), "should find Bash tool");
        assert_eq!(bash.unwrap().name(), "Bash");

        let read = tools.iter().find(|t| t.name() == "Read");
        assert!(read.is_some(), "should find Read tool");

        let todo_write = tools.iter().find(|t| t.name() == "TodoWrite");
        assert!(todo_write.is_some(), "should find TodoWrite tool");

        let nonexistent = tools.iter().find(|t| t.name() == "NonExistentTool");
        assert!(nonexistent.is_none(), "should not find nonexistent tool");
    }

    #[test]
    fn test_all_tools_have_unique_names() {
        let tools = get_all_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        let original_len = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), original_len, "all tool names should be unique");
    }

    #[test]
    fn test_all_tools_have_schema() {
        let tools = get_all_tools();
        for tool in &tools {
            let schema = tool.input_json_schema();
            assert!(
                schema.is_object(),
                "tool {} schema should be an object",
                tool.name()
            );
            assert!(
                schema.get("properties").is_some(),
                "tool {} schema should have properties",
                tool.name()
            );
        }
    }

    #[test]
    fn coordinator_policy_exposes_only_lead_orchestration_tools() {
        let names = tool_names(get_tools_for_policy(ToolPolicy::Coordinator));

        assert!(names.contains(&"Agent".to_string()));
        assert!(names.contains(&"SendMessage".to_string()));
        assert!(names.contains(&"TaskList".to_string()));
        assert!(names.contains(&"TaskStop".to_string()));
        assert!(!names.contains(&"Bash".to_string()));
        assert!(!names.contains(&"Write".to_string()));
        assert!(!names.contains(&"TeamSpawn".to_string()));
    }

    #[test]
    fn worker_policy_removes_internal_orchestration_tools() {
        let names = tool_names(get_tools_for_policy(ToolPolicy::CoordinatorWorker));

        assert!(names.contains(&"Read".to_string()));
        assert!(names.contains(&"Bash".to_string()));
        assert!(names.contains(&"Write".to_string()));
        assert!(names.contains(&"SendMessage".to_string()));
        assert!(names.contains(&"TaskUpdate".to_string()));
        assert!(!names.contains(&"Agent".to_string()));
        assert!(!names.contains(&"TeamSpawn".to_string()));
        assert!(!names.contains(&"TaskStop".to_string()));
    }

    #[test]
    fn in_process_teammate_policy_can_report_but_not_spawn_agents() {
        let names = tool_names(get_tools_for_policy(ToolPolicy::InProcessTeammate));

        assert!(names.contains(&"SendMessage".to_string()));
        assert!(names.contains(&"TaskList".to_string()));
        assert!(names.contains(&"TaskUpdate".to_string()));
        assert!(names.contains(&"TaskOutput".to_string()));
        assert!(!names.contains(&"Agent".to_string()));
        assert!(!names.contains(&"TeamSpawn".to_string()));
    }
}
