//! SystemStatus tool formatting owned by the IPC runtime crate.

use cc_ipc_protocol::subsystem_types::*;

#[derive(Debug, Clone, Default)]
pub struct ChromeStatus {
    pub connection_label: String,
    pub extension_detected: Option<bool>,
    pub last_error: Option<String>,
}

pub fn system_status_description() -> String {
    "Query the current status of subsystems (LSP, MCP, plugins, skills, IDE, Chrome).".to_string()
}

pub fn system_status_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "subsystem": {
                "type": "string",
                "enum": ["lsp", "mcp", "plugins", "skills", "agents", "teams", "ide", "chrome", "all"],
                "description": "Which subsystem to query. Defaults to 'all'."
            }
        }
    })
}

pub fn system_status_prompt() -> String {
    "Use SystemStatus to check the current status of LSP servers, MCP servers, plugins, skills, IDE, and Chrome. \
     Query a specific subsystem with the `subsystem` parameter, or use \"all\" for a full overview."
        .to_string()
}

pub fn format_status_output(subsystem: &str, chrome: Option<ChromeStatus>) -> String {
    let mut parts = Vec::new();

    if subsystem == "all" || subsystem == "lsp" {
        parts.push(format_lsp_section(
            &crate::subsystem_handlers::build_lsp_server_info_list(),
        ));
    }

    if subsystem == "all" || subsystem == "mcp" {
        parts.push(format_mcp_section(
            &crate::subsystem_handlers::build_mcp_server_info_list(),
        ));
    }

    if subsystem == "all" || subsystem == "plugins" {
        parts.push(format_plugin_section(
            &crate::subsystem_handlers::build_plugin_info_list(),
        ));
    }

    if subsystem == "all" || subsystem == "skills" {
        parts.push(format_skill_section(
            &crate::subsystem_handlers::build_skill_info_list(),
        ));
    }

    if subsystem == "all" || subsystem == "agents" {
        let tree = crate::agent_tree::AGENT_TREE.lock();
        parts.push(format_agent_section(&tree.active_agents()));
    }

    if subsystem == "all" || subsystem == "teams" {
        parts.push(format_team_section());
    }

    if subsystem == "all" || subsystem == "ide" {
        parts.push(format_ide_section(
            &crate::subsystem_handlers::build_ide_info_list(),
        ));
    }

    if subsystem == "all" || subsystem == "chrome" {
        parts.push(format_chrome_section(chrome));
    }

    parts.join("\n")
}

fn format_lsp_section(servers: &[LspServerInfo]) -> String {
    let mut section = String::from("## LSP Servers\n");
    if servers.is_empty() {
        section.push_str("No LSP servers configured.\n");
    } else {
        for s in servers {
            section.push_str(&format!(
                "- {}: {} (open_files={}, extensions: {})\n",
                s.language_id,
                s.state,
                s.open_files_count,
                s.extensions.join(", ")
            ));
        }
    }
    section
}

fn format_mcp_section(servers: &[McpServerStatusInfo]) -> String {
    let mut section = String::from("## MCP Servers\n");
    if servers.is_empty() {
        section.push_str("No MCP servers configured.\n");
    } else {
        for s in servers {
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
    section
}

fn format_plugin_section(plugins: &[PluginInfo]) -> String {
    let mut section = String::from("## Plugins\n");
    if plugins.is_empty() {
        section.push_str("No plugins installed.\n");
    } else {
        for p in plugins {
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
    section
}

fn format_skill_section(skills: &[SkillInfo]) -> String {
    let mut section = format!("## Skills ({} total)\n", skills.len());
    if skills.is_empty() {
        section.push_str("No skills loaded.\n");
    } else {
        for s in skills {
            section.push_str(&format!(
                "- {} [{}] - {}\n",
                s.name, s.source, s.description
            ));
        }
    }
    section
}

fn format_agent_section(active: &[&cc_types::agent_types::AgentNode]) -> String {
    let bg_count = active.iter().filter(|a| a.is_background).count();
    let mut section = format!(
        "## Active Agents ({} total, {} background)\n",
        active.len(),
        bg_count
    );
    if active.is_empty() {
        section.push_str("No active agents.\n");
    } else {
        for a in active {
            section.push_str(&format!(
                "- {}: {} [{}{}] - \"{}\" (depth {})\n",
                a.agent_id,
                a.state,
                if a.is_background {
                    "background"
                } else {
                    "sync"
                },
                a.agent_type
                    .as_ref()
                    .map(|t| format!(", {}", t))
                    .unwrap_or_default(),
                a.description,
                a.depth,
            ));
        }
    }
    section
}

fn format_team_section() -> String {
    let mut section = String::from("## Teams\n");
    section.push_str("Team status requires CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS feature.\n");
    section
}

fn format_ide_section(ides: &[IdeInfo]) -> String {
    let mut section = String::from("## IDE Integrations\n");
    if ides.is_empty() {
        section.push_str("No IDE integrations detected.\n");
    } else {
        for ide in ides {
            section.push_str(&format!(
                "- {}: installed={} running={} selected={} connection={}\n",
                ide.id,
                ide.installed,
                ide.running,
                ide.selected,
                ide.connection_state.as_deref().unwrap_or("disconnected")
            ));
        }
    }
    section
}

fn format_chrome_section(chrome: Option<ChromeStatus>) -> String {
    let mut section = String::from("## Chrome Integration\n");
    if let Some(chrome) = chrome {
        section.push_str(&format!("Connection: {}\n", chrome.connection_label));
        section.push_str(&format!(
            "Extension detected: {}\n",
            chrome
                .extension_detected
                .map(|value| value.to_string())
                .unwrap_or_else(|| "not checked".to_string())
        ));
        if let Some(error) = chrome.last_error.as_deref() {
            section.push_str(&format!("Last error: {error}\n"));
        }
    } else {
        section.push_str("Connection: unavailable\n");
        section.push_str("Extension detected: not checked\n");
    }
    section
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_subsystem_property() {
        let schema = system_status_schema();
        assert!(schema["properties"]["subsystem"].is_object());
    }

    #[test]
    fn all_output_has_core_sections() {
        let output = format_status_output("all", None);
        assert!(output.contains("## LSP Servers"));
        assert!(output.contains("## MCP Servers"));
        assert!(output.contains("## Chrome Integration"));
    }
}
