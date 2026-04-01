#![allow(unused)]
use std::sync::Arc;

use crate::types::tool::{Tool, Tools};

use super::agent::AgentTool;
use super::ask_user::AskUserQuestionTool;
use super::bash::BashTool;
use super::file_edit::FileEditTool;
use super::file_read::FileReadTool;
use super::file_write::FileWriteTool;
use super::glob_tool::GlobTool;
use super::grep::GrepTool;
use super::notebook_edit::NotebookEditTool;
use super::tool_search::ToolSearchTool;

/// Get all base tool instances.
///
/// Corresponds to TypeScript: tools.ts `getAllBaseTools()`
/// Returns all tool implementations. The caller can filter by `is_enabled()`.
pub fn get_all_tools() -> Tools {
    let tools: Tools = vec![
        Arc::new(BashTool::new()),
        Arc::new(FileReadTool::new()),
        Arc::new(FileWriteTool::new()),
        Arc::new(FileEditTool::new()),
        Arc::new(GlobTool::new()),
        Arc::new(GrepTool),
        Arc::new(NotebookEditTool),
        Arc::new(AskUserQuestionTool),
        Arc::new(ToolSearchTool),
        Arc::new(AgentTool),
    ];

    // Filter to only enabled tools
    tools.into_iter().filter(|t| t.is_enabled()).collect()
}

/// Find a tool by name from a tool collection.
///
/// Corresponds to TypeScript pattern: `tools.find(t => t.name === name)`
pub fn find_tool_by_name(tools: &Tools, name: &str) -> Option<Arc<dyn Tool>> {
    tools.iter().find(|t| t.name() == name).cloned()
}

/// Get tools filtered for the default preset (all enabled tools).
///
/// Corresponds to TypeScript: tools.ts `getToolsForDefaultPreset()`
pub fn get_tools_for_default_preset() -> Tools {
    get_all_tools()
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

        let bash = find_tool_by_name(&tools, "Bash");
        assert!(bash.is_some(), "should find Bash tool");
        assert_eq!(bash.unwrap().name(), "Bash");

        let read = find_tool_by_name(&tools, "Read");
        assert!(read.is_some(), "should find Read tool");

        let nonexistent = find_tool_by_name(&tools, "NonExistentTool");
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
            assert!(schema.is_object(), "tool {} schema should be an object", tool.name());
            assert!(
                schema.get("properties").is_some(),
                "tool {} schema should have properties",
                tool.name()
            );
        }
    }
}
