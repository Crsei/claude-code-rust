// src/tools/system_status.rs

//! SystemStatus tool — lets the Agent query subsystem status.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::ipc::subsystem_handlers;
use crate::types::message::AssistantMessage;
use crate::types::tool::{Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult};

pub struct SystemStatusTool;

#[async_trait]
impl Tool for SystemStatusTool {
    fn name(&self) -> &str {
        "SystemStatus"
    }

    async fn description(&self, _input: &Value) -> String {
        "Query the current status of subsystems (LSP, MCP, plugins, skills).".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subsystem": {
                    "type": "string",
                    "enum": ["lsp", "mcp", "plugins", "skills", "agents", "teams", "all"],
                    "description": "Which subsystem to query. Defaults to 'all'."
                }
            }
        })
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let subsystem = input["subsystem"].as_str().unwrap_or("all");
        let output = format_status_output(subsystem);

        Ok(ToolResult {
            data: json!({ "status": output }),
            model_content: None,
            display_preview: Some(output.clone()),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use SystemStatus to check the current status of LSP servers, MCP servers, plugins, and skills. \
         Query a specific subsystem with the `subsystem` parameter, or use \"all\" for a full overview."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "SystemStatus".to_string()
    }
}

/// Format a human-readable status output for the given subsystem.
fn format_status_output(subsystem: &str) -> String {
    let mut parts = Vec::new();

    if subsystem == "all" || subsystem == "lsp" {
        let servers = subsystem_handlers::build_lsp_server_info_list();
        let mut section = String::from("## LSP Servers\n");
        if servers.is_empty() {
            section.push_str("No LSP servers configured.\n");
        } else {
            for s in &servers {
                section.push_str(&format!(
                    "- {}: {} (extensions: {})\n",
                    s.language_id,
                    s.state,
                    s.extensions.join(", ")
                ));
            }
        }
        parts.push(section);
    }

    if subsystem == "all" || subsystem == "mcp" {
        let servers = subsystem_handlers::build_mcp_server_info_list();
        let mut section = String::from("## MCP Servers\n");
        if servers.is_empty() {
            section.push_str("No MCP servers configured.\n");
        } else {
            for s in &servers {
                let mut line = format!(
                    "- {}: {} ({}, {} tools, {} resources)",
                    s.name, s.state, s.transport, s.tools_count, s.resources_count
                );
                if let Some(ref info) = s.server_info {
                    line.push_str(&format!(" [{}@{}]", info.name, info.version));
                }
                section.push_str(&format!("{}\n", line));
            }
        }
        parts.push(section);
    }

    if subsystem == "all" || subsystem == "plugins" {
        let plugins = subsystem_handlers::build_plugin_info_list();
        let mut section = String::from("## Plugins\n");
        if plugins.is_empty() {
            section.push_str("No plugins installed.\n");
        } else {
            for p in &plugins {
                let mut line = format!("- {}: {} (v{})", p.id, p.status, p.version);
                if !p.contributed_skills.is_empty() {
                    line.push_str(&format!("\n  Skills: {}", p.contributed_skills.join(", ")));
                }
                if !p.contributed_tools.is_empty() {
                    line.push_str(&format!("\n  Tools: {}", p.contributed_tools.join(", ")));
                }
                section.push_str(&format!("{}\n", line));
            }
        }
        parts.push(section);
    }

    if subsystem == "all" || subsystem == "skills" {
        let skills = subsystem_handlers::build_skill_info_list();
        let mut section = format!("## Skills ({} total)\n", skills.len());
        if skills.is_empty() {
            section.push_str("No skills loaded.\n");
        } else {
            for s in &skills {
                section.push_str(&format!("- {} [{}] — {}\n", s.name, s.source, s.description));
            }
        }
        parts.push(section);
    }

    if subsystem == "all" || subsystem == "agents" {
        let tree = crate::ipc::agent_tree::AGENT_TREE.lock();
        let active = tree.active_agents();
        let bg_count = active.iter().filter(|a| a.is_background).count();
        let mut section = format!(
            "## Active Agents ({} total, {} background)\n",
            active.len(),
            bg_count
        );
        if active.is_empty() {
            section.push_str("No active agents.\n");
        } else {
            for a in &active {
                section.push_str(&format!(
                    "- {}: {} [{}{}] — \"{}\" (depth {})\n",
                    a.agent_id,
                    a.state,
                    if a.is_background { "background" } else { "sync" },
                    a.agent_type
                        .as_ref()
                        .map(|t| format!(", {}", t))
                        .unwrap_or_default(),
                    a.description,
                    a.depth,
                ));
            }
        }
        parts.push(section);
    }

    if subsystem == "all" || subsystem == "teams" {
        let mut section = String::from("## Teams\n");
        section.push_str("Team status requires CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS feature.\n");
        parts.push(section);
    }

    parts.join("\n")
}

fn _count_nodes(node: &crate::ipc::agent_types::AgentNode) -> usize {
    1 + node.children.iter().map(_count_nodes).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_status_tool_name() {
        let tool = SystemStatusTool;
        assert_eq!(tool.name(), "SystemStatus");
    }

    #[test]
    fn system_status_tool_is_read_only() {
        let tool = SystemStatusTool;
        assert!(tool.is_read_only(&json!({})));
    }

    #[test]
    fn system_status_tool_schema_has_subsystem_property() {
        let tool = SystemStatusTool;
        let schema = tool.input_json_schema();
        assert!(schema["properties"]["subsystem"].is_object());
    }

    #[test]
    fn format_status_output_all_returns_all_sections() {
        let output = format_status_output("all");
        assert!(output.contains("## LSP Servers"));
        assert!(output.contains("## MCP Servers"));
        assert!(output.contains("## Plugins"));
        assert!(output.contains("## Skills"));
    }

    #[test]
    fn format_status_output_lsp_only() {
        let output = format_status_output("lsp");
        assert!(output.contains("## LSP Servers"));
        assert!(!output.contains("## MCP Servers"));
    }
}
