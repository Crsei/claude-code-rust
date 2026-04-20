use std::collections::HashSet;
use std::sync::Arc;

use tracing::warn;

use crate::types::tool::Tools;

use super::agent::AgentTool;
use super::ask_user::AskUserQuestionTool;
use super::brief::BriefTool;
use super::config_tool::ConfigTool;
use super::lsp::LspTool;
use super::plan_mode::{EnterPlanModeTool, ExitPlanModeTool};
use super::send_message::SendMessageTool;
use super::team_spawn::TeamSpawnTool;
use super::send_user_message::SendUserMessageTool;
use super::skill::SkillTool;
use super::structured_output::StructuredOutputTool;
use super::system_status::SystemStatusTool;
use super::tasks::{
    TaskCreateTool, TaskGetTool, TaskListTool, TaskOutputTool, TaskStopTool, TaskUpdateTool,
};
use super::web_fetch::WebFetchTool;
use super::web_search::WebSearchTool;
use super::worktree::{EnterWorktreeTool, ExitWorktreeTool};
use super::{exec, fs};

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
        Arc::new(AgentTool) as _,
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
        Arc::new(TaskCreateTool) as _,
        Arc::new(TaskGetTool) as _,
        Arc::new(TaskUpdateTool) as _,
        Arc::new(TaskListTool) as _,
        Arc::new(TaskStopTool) as _,
        Arc::new(TaskOutputTool) as _,
        Arc::new(LspTool) as _,
        Arc::new(SendMessageTool) as _,
        Arc::new(TeamSpawnTool) as _,
        Arc::new(BriefTool) as _,
        Arc::new(SystemStatusTool) as _,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
