//! `/lsp` command -- LSP server status cards and recommendation settings.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use cc_ipc_protocol::subsystem_types::LspServerInfo;

pub struct LspHandler;

#[async_trait]
impl CommandHandler for LspHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut parts = args.split_whitespace();
        let sub = parts.next().unwrap_or("status");
        match sub {
            "" | "status" | "servers" => Ok(CommandResult::Output(render_status())),
            "recommendations" | "recommendation" => {
                Ok(CommandResult::Output(render_recommendations()))
            }
            "help" | "?" => Ok(CommandResult::Output(usage())),
            other => Ok(CommandResult::Output(format!(
                "Unknown /lsp subcommand: '{other}'\n\n{}",
                usage()
            ))),
        }
    }
}

fn render_status() -> String {
    crate::ipc::runtime_adapters::ensure_installed();
    let servers = cc_ipc::subsystem_handlers::build_lsp_server_info_list();
    let mut lines = vec!["LSP server status".to_string()];
    if servers.is_empty() {
        lines.push("No LSP servers configured.".to_string());
    } else {
        for server in servers {
            lines.push(render_server_card(&server));
        }
    }
    lines.push(String::new());
    lines.push("Actions: /lsp recommendations, SystemStatus {\"subsystem\":\"lsp\"}".to_string());
    lines.join("\n")
}

fn render_server_card(server: &LspServerInfo) -> String {
    let mut card = format!(
        "\n[{}]\n  state: {}\n  open files: {}\n  extensions: {}",
        server.language_id,
        server.state,
        server.open_files_count,
        if server.extensions.is_empty() {
            "(none)".to_string()
        } else {
            server.extensions.join(", ")
        }
    );
    if let Some(error) = &server.error {
        card.push_str(&format!("\n  error: {error}"));
    }
    card
}

fn render_recommendations() -> String {
    crate::ipc::runtime_adapters::ensure_installed();
    let settings = cc_ipc::subsystem_handlers::load_lsp_recommendation_settings();
    let muted = if settings.muted_plugins.is_empty() {
        "(none)".to_string()
    } else {
        settings.muted_plugins.join(", ")
    };
    format!(
        "LSP recommendation settings\n  disabled: {}\n  muted plugins: {}\n\nRecommendations are shown when the backend emits a real LSP RecommendationRequest.",
        settings.disabled, muted
    )
}

fn usage() -> String {
    "Usage:\n  /lsp status\n  /lsp servers\n  /lsp recommendations".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn lsp_status_renders_server_cards() {
        let handler = LspHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("LSP server status"));
                assert!(text.contains("extensions:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn lsp_recommendations_renders_settings() {
        let handler = LspHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("recommendations", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("LSP recommendation settings"));
                assert!(text.contains("muted plugins"));
            }
            _ => panic!("expected Output"),
        }
    }
}
