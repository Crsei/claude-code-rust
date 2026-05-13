pub mod detection {
    use std::sync::Arc;

    use crate::types::tool::Tool;

    const COMPUTER_USE_SERVER: &str = "computer-use";
    const COMPUTER_USE_PREFIX: &str = "mcp__computer-use__";

    pub fn computer_use_system_prompt(tools: &[Arc<dyn Tool>]) -> Option<String> {
        let tool_list = tools
            .iter()
            .filter_map(|tool| {
                let name = tool.user_facing_name(None);
                let action = name.strip_prefix(COMPUTER_USE_PREFIX)?;
                Some(format!("- `{}` ({})", name, action))
            })
            .collect::<Vec<_>>();

        if tool_list.is_empty() {
            return None;
        }

        Some(format!(
            "# Computer Use\n\n\
             You have access to Computer Use tools from the `{}` MCP server.\n\n\
             Available Computer Use tools:\n\
             {}\n\n\
             - Always take a screenshot first before acting.\n\
             - After an input action, observe again to verify the result.\n",
            COMPUTER_USE_SERVER,
            tool_list.join("\n")
        ))
    }
}
