use std::sync::Arc;

use crate::types::tool::Tools;

use super::ask_user::AskUserQuestionTool;
use super::bash::BashTool;
use super::file_edit::FileEditTool;
use super::file_read::FileReadTool;
use super::file_write::FileWriteTool;
use super::glob_tool::GlobTool;
use super::grep::GrepTool;
use super::skill::SkillTool;

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
        Arc::new(AskUserQuestionTool),
        Arc::new(SkillTool),
    ];

    // Filter to only enabled tools
    tools.into_iter().filter(|t| t.is_enabled()).collect()
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
            assert!(schema.is_object(), "tool {} schema should be an object", tool.name());
            assert!(
                schema.get("properties").is_some(),
                "tool {} schema should have properties",
                tool.name()
            );
        }
    }
}
