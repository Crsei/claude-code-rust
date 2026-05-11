use anyhow::Result;

use super::{CommandContext, CommandResult};
use cc_ipc::subsystem_handlers::{run_mcp_runtime_operation, McpRuntimeOperation};

// ---------------------------------------------------------------------------
// connect / disconnect / reconnect
// ---------------------------------------------------------------------------

pub(super) async fn handle_connect(rest: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    match rest.first() {
        Some(name) => {
            crate::ipc::runtime_adapters::ensure_installed();
            let report =
                run_mcp_runtime_operation(&ctx.cwd, McpRuntimeOperation::Connect, name).await;
            Ok(CommandResult::Output(report.text))
        }
        None => Ok(CommandResult::Output(
            "Usage: /mcp connect <name>".to_string(),
        )),
    }
}

pub(super) async fn handle_disconnect(
    rest: &[&str],
    ctx: &CommandContext,
) -> Result<CommandResult> {
    match rest.first() {
        Some(name) => {
            crate::ipc::runtime_adapters::ensure_installed();
            let report =
                run_mcp_runtime_operation(&ctx.cwd, McpRuntimeOperation::Disconnect, name).await;
            Ok(CommandResult::Output(report.text))
        }
        None => Ok(CommandResult::Output(
            "Usage: /mcp disconnect <name>".to_string(),
        )),
    }
}

pub(super) async fn handle_reconnect(rest: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    match rest.first() {
        Some(name) => {
            crate::ipc::runtime_adapters::ensure_installed();
            let report =
                run_mcp_runtime_operation(&ctx.cwd, McpRuntimeOperation::Reconnect, name).await;
            Ok(CommandResult::Output(report.text))
        }
        None => Ok(CommandResult::Output(
            "Usage: /mcp reconnect <name>".to_string(),
        )),
    }
}
