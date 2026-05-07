//! Rust-side MCP UI surfaces.

#[allow(dead_code)]
pub mod capabilities_section;
#[allow(dead_code)]
pub mod elicitation_dialog;
#[allow(dead_code)]
pub mod index;
#[allow(dead_code)]
pub mod mcp_agent_server_menu;
#[allow(dead_code)]
pub mod mcp_list_panel;
#[allow(dead_code)]
pub mod mcp_parsing_warnings;
#[allow(dead_code)]
pub mod mcp_reconnect;
#[allow(dead_code)]
pub mod mcp_remote_server_menu;
#[allow(dead_code)]
pub mod mcp_server_approval_dialog;
#[allow(dead_code)]
pub mod mcp_server_card;
#[allow(dead_code)]
pub mod mcp_server_desktop_import_dialog;
#[allow(dead_code)]
pub mod mcp_server_dialog_copy;
#[allow(dead_code)]
pub mod mcp_server_multiselect_dialog;
#[allow(dead_code)]
pub mod mcp_settings;
#[allow(dead_code)]
pub mod mcp_stdio_server_menu;
#[allow(dead_code)]
pub mod mcp_tool_detail_view;
#[allow(dead_code)]
pub mod mcp_tool_list_view;
#[allow(dead_code)]
pub mod utils;

#[cfg(test)]
mod tests {
    use super::capabilities_section::render_capabilities_section;
    use super::elicitation_dialog::{ElicitationField, render_elicitation_dialog};
    use super::index::{McpCapability, McpServer, McpServerKind, McpServerStatus, McpTool};
    use super::mcp_agent_server_menu::render_mcp_agent_server_menu;
    use super::mcp_list_panel::render_mcp_list_panel;
    use super::mcp_parsing_warnings::render_mcp_parsing_warnings;
    use super::mcp_reconnect::render_mcp_reconnect;
    use super::mcp_remote_server_menu::render_mcp_remote_server_menu;
    use super::mcp_server_approval_dialog::render_mcp_server_approval_dialog;
    use super::mcp_server_card::render_mcp_server_card;
    use super::mcp_server_desktop_import_dialog::{
        DesktopMcpImportServer, McpServerDesktopImportState,
        render_mcp_server_desktop_import_dialog,
    };
    use super::mcp_server_dialog_copy::render_mcp_server_dialog_copy;
    use super::mcp_server_multiselect_dialog::{
        McpServerMultiselectState, render_mcp_server_multiselect_dialog,
    };
    use super::mcp_settings::{McpSettingsSummary, render_mcp_settings};
    use super::mcp_stdio_server_menu::render_mcp_stdio_server_menu;
    use super::mcp_tool_detail_view::render_mcp_tool_detail_view;
    use super::mcp_tool_list_view::render_mcp_tool_list_view;
    use super::utils::reconnect_helpers::ReconnectAttempt;

    #[test]
    fn snapshot_mcp_surfaces() {
        let mut tool = McpTool::new("query", "Run a read-only query");
        tool.input_schema = vec![
            ("sql".to_string(), "string".to_string()),
            ("limit".to_string(), "number".to_string()),
        ];
        let mut stdio = McpServer::new("database", McpServerKind::Stdio);
        stdio.command_or_url = "uvx db-mcp".to_string();
        stdio.tools = vec![tool.clone()];
        stdio.capabilities = vec![
            McpCapability {
                name: "tools".to_string(),
                count: 1,
            },
            McpCapability {
                name: "resources".to_string(),
                count: 3,
            },
        ];
        let mut remote = McpServer::new("docs", McpServerKind::Remote);
        remote.command_or_url = "https://mcp.example.com".to_string();
        remote.status = McpServerStatus::Failed;
        remote.warnings = vec!["missing auth token".to_string()];
        let mut agent = McpServer::new("agent-tools", McpServerKind::Agent);
        agent.status = McpServerStatus::Connecting;

        let rendered = [
            section(
                "list",
                render_mcp_list_panel(&[stdio.clone(), remote.clone(), agent.clone()], 1),
            ),
            section(
                "capabilities",
                render_capabilities_section(&stdio.capabilities),
            ),
            section("warnings", render_mcp_parsing_warnings(&remote.warnings)),
            section("tool-list", render_mcp_tool_list_view(&stdio.tools, 0)),
            section("tool-detail", render_mcp_tool_detail_view(&tool)),
            section("stdio-menu", render_mcp_stdio_server_menu(&stdio)),
            section("remote-menu", render_mcp_remote_server_menu(&remote)),
            section("server-card", render_mcp_server_card(&remote, true)),
            section("dialog-copy", render_mcp_server_dialog_copy()),
            section(
                "approval-dialog",
                render_mcp_server_approval_dialog("playwright", 1),
            ),
            section(
                "multiselect-dialog",
                render_mcp_server_multiselect_dialog(&McpServerMultiselectState {
                    servers: vec![
                        super::mcp_server_multiselect_dialog::McpServerMultiSelectItem {
                            name: "github".to_string(),
                            selected: true,
                        },
                        super::mcp_server_multiselect_dialog::McpServerMultiSelectItem {
                            name: "playwright".to_string(),
                            selected: false,
                        },
                        super::mcp_server_multiselect_dialog::McpServerMultiSelectItem {
                            name: "sentry".to_string(),
                            selected: true,
                        },
                    ],
                    selected_index: 1,
                }),
            ),
            section(
                "desktop-import-dialog",
                render_mcp_server_desktop_import_dialog(&McpServerDesktopImportState {
                    servers: vec![
                        DesktopMcpImportServer {
                            name: "filesystem".to_string(),
                            command_or_url: "npx @modelcontextprotocol/server-filesystem"
                                .to_string(),
                            collides: false,
                            selected: true,
                            final_name: None,
                        },
                        DesktopMcpImportServer {
                            name: "github".to_string(),
                            command_or_url: "uvx mcp-github".to_string(),
                            collides: true,
                            selected: false,
                            final_name: Some("github-1".to_string()),
                        },
                    ],
                    selected_index: 1,
                    target_scope: "~/.cc-rust/settings.json".to_string(),
                }),
            ),
            section(
                "agent-menu",
                render_mcp_agent_server_menu(&agent, "reviewer"),
            ),
            section(
                "settings",
                render_mcp_settings(&McpSettingsSummary {
                    enabled: true,
                    config_path: "~/.cc-rust/mcp.json".to_string(),
                    server_count: 3,
                    warning_count: 1,
                }),
            ),
            section(
                "reconnect",
                render_mcp_reconnect(&ReconnectAttempt {
                    server_name: "docs".to_string(),
                    attempt: 2,
                    max_attempts: 5,
                    last_error: Some("HTTP 401".to_string()),
                }),
            ),
            section(
                "elicitation",
                render_elicitation_dialog(
                    "MCP needs input",
                    &[
                        ElicitationField {
                            name: "token".to_string(),
                            prompt: "API token".to_string(),
                            value: String::new(),
                            required: true,
                        },
                        ElicitationField {
                            name: "region".to_string(),
                            prompt: "Region".to_string(),
                            value: "us".to_string(),
                            required: false,
                        },
                    ],
                    0,
                ),
            ),
        ]
        .join("\n\n");

        insta::assert_snapshot!("mcp_surfaces", rendered);
    }

    fn section(name: &str, body: impl AsRef<str>) -> String {
        format!("## {name}\n{}", body.as_ref())
    }
}
