use std::collections::HashSet;
use std::sync::Arc;

pub use cc_tools::registry::ToolPolicy;
use tracing::warn;

use cc_engine::tools::exec;
use cc_engine::types::tool::Tools;

use crate::skills::tool::SkillTool;
use cc_lsp_service::tool::LspTool;
use cc_teams::pr_activity::{SubscribePrActivityTool, UnsubscribePrActivityTool};
use cc_teams::send_message::SendMessageTool;
use cc_teams::team_spawn::TeamSpawnTool;
use cc_tools::ask_user::AskUserQuestionTool;
use cc_tools::brief::BriefTool;
use cc_tools::config_tool::ConfigTool;
use cc_tools::fs;
use cc_tools::plan_mode::{EnterPlanModeTool, ExitPlanModeTool};
use cc_tools::send_user_message::SendUserMessageTool;
use cc_tools::sleep::SleepTool;
use cc_tools::structured_output::StructuredOutputTool;
use cc_tools::system_status::SystemStatusTool;
use cc_tools::tool_search::ToolSearchTool;
use cc_tools::web_fetch::WebFetchTool;
use cc_tools::web_search::WebSearchTool;
use cc_worktree::tool::{EnterWorktreeTool, ExitWorktreeTool};

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
    tools.push(Arc::new(SleepTool));
    tools.extend(cc_tools::tasks::tools());

    // Single-tool / small-cluster modules (not yet a sub-domain).
    tools.extend([
        Arc::new(AskUserQuestionTool) as _,
        Arc::new(cc_engine::agent::AgentTool) as _,
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

    for tool in cc_plugins::discover_plugin_tools() {
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
    if cc_teams::coordinator::is_coordinator_mode_enabled() {
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
    let Some(allowed) = cc_tools::registry::allowed_tool_names(policy) else {
        return tools;
    };
    tools
        .into_iter()
        .filter(|tool| allowed.iter().any(|name| *name == tool.name()))
        .collect()
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
