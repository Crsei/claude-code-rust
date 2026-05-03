//! Hook matcher selection and rendering.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookMatcher {
    pub tool_name: Option<String>,
    pub command_pattern: Option<String>,
}

impl HookMatcher {
    pub fn all_tools() -> Self {
        Self {
            tool_name: None,
            command_pattern: None,
        }
    }

    pub fn for_tool(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: Some(tool_name.into()),
            command_pattern: None,
        }
    }

    pub fn label(&self) -> String {
        match (&self.tool_name, &self.command_pattern) {
            (Some(tool), Some(pattern)) => format!("{tool} matching {pattern}"),
            (Some(tool), None) => tool.clone(),
            (None, Some(pattern)) => format!("commands matching {pattern}"),
            (None, None) => "All tools".to_string(),
        }
    }
}

pub fn render_select_matcher_mode(matchers: &[HookMatcher], selected_index: usize) -> String {
    let mut lines = vec!["Select matcher".to_string()];
    for (idx, matcher) in matchers.iter().enumerate() {
        let marker = if idx == selected_index { ">" } else { " " };
        lines.push(format!("{marker} {}", matcher.label()));
    }
    lines.join("\n")
}
