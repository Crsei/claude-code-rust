//! Command registry -- slash commands for the interactive REPL.
//!
//! Commands are invoked by typing `/command_name [args]` in the user prompt.
//! Each command implements `CommandHandler` and is registered in `get_all_commands()`.

// Essential commands
pub mod clear;
pub mod config_cmd;
pub mod context;
pub mod cost;
pub mod diff;
pub mod exit;
pub mod files;
pub mod help;
pub mod login;
pub mod login_code;
pub mod logout;
pub mod model;
pub mod permissions_cmd;
pub mod resume;
pub mod session;
pub mod version;

// Git & workflow
pub mod branch;
pub mod commit;

// Model control
pub mod effort;
pub mod fast;
pub mod model_add;

// Memory & skills
pub mod memory;
pub mod skills_cmd;

// Session management
pub mod copy;
pub mod init;
pub mod status;

// Workspace
pub mod add_dir;

// Sandbox
pub mod sandbox_cmd;

// Keybindings
pub mod keybindings_cmd;

// Scriptable status line (issue #11)
pub mod statusline_cmd;

// Terminal setup diagnostics (issue #12)
pub mod terminal_setup;

// Voice dictation (issue #13)
pub mod voice_cmd;

// Export
pub mod audit_export;
pub mod export;
pub mod session_export;

// Extended info
pub mod extra_usage;
pub mod rate_limit;

// Context management
pub mod compact;

// MCP server management
pub mod mcp_cmd;
pub mod plugin_cmd;

// First-party Chrome integration (Claude in Chrome)
pub mod chrome_cmd;

// KAIROS / assistant commands
pub mod assistant;
pub mod brief;
pub mod channels;
pub mod daemon_cmd;
pub mod dream;
pub mod notify;
pub mod sleep_cmd;

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use crate::bootstrap::SessionId;
use crate::types::app_state::AppState;
use crate::types::message::Message;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A registered slash command.
pub struct Command {
    /// Primary command name (e.g. "help").
    pub name: String,
    /// Alternative names (e.g. ["h", "?"]).
    pub aliases: Vec<String>,
    /// Short description shown in /help output.
    pub description: String,
    /// The handler that executes this command.
    pub handler: Box<dyn CommandHandler>,
}

/// Trait implemented by every slash command.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Execute the command with the given arguments and context.
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult>;
}

/// Execution context passed to command handlers.
pub struct CommandContext {
    /// Current conversation messages.
    pub messages: Vec<Message>,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Application state snapshot.
    pub app_state: AppState,
    /// Current session ID.
    pub session_id: SessionId,
}

/// Result of executing a command.
pub enum CommandResult {
    /// Output text to display to the user (not sent to the model).
    Output(String),
    /// Messages to add to the conversation and then send to the model.
    Query(Vec<Message>),
    /// Clear the conversation history.
    Clear,
    /// Exit the REPL with a goodbye message.
    Exit(String),
    /// No visible output.
    #[allow(dead_code)]
    None,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Build the full list of available commands.
pub fn get_all_commands() -> Vec<Command> {
    vec![
        Command {
            name: "help".into(),
            aliases: vec!["h".into(), "?".into()],
            description: "Show available commands and their descriptions".into(),
            handler: Box::new(help::HelpHandler),
        },
        Command {
            name: "clear".into(),
            aliases: vec![],
            description: "Clear the conversation history".into(),
            handler: Box::new(clear::ClearHandler),
        },
        Command {
            name: "config".into(),
            aliases: vec!["settings".into()],
            description: "Show or modify configuration settings".into(),
            handler: Box::new(config_cmd::ConfigHandler),
        },
        Command {
            name: "diff".into(),
            aliases: vec![],
            description: "Show git diff of current changes".into(),
            handler: Box::new(diff::DiffHandler),
        },
        Command {
            name: "exit".into(),
            aliases: vec!["quit".into(), "q".into()],
            description: "Exit the REPL".into(),
            handler: Box::new(exit::ExitHandler),
        },
        Command {
            name: "version".into(),
            aliases: vec!["v".into()],
            description: "Show the current version".into(),
            handler: Box::new(version::VersionHandler),
        },
        Command {
            name: "model".into(),
            aliases: vec![],
            description: "Show or switch the active model".into(),
            handler: Box::new(model::ModelHandler),
        },
        Command {
            name: "cost".into(),
            aliases: vec!["usage".into()],
            description: "Show token usage and cost for the current session".into(),
            handler: Box::new(cost::CostHandler),
        },
        Command {
            name: "session".into(),
            aliases: vec![],
            description: "Show current session info or list saved sessions".into(),
            handler: Box::new(session::SessionHandler),
        },
        Command {
            name: "resume".into(),
            aliases: vec![],
            description: "Resume a previous session".into(),
            handler: Box::new(resume::ResumeHandler),
        },
        Command {
            name: "files".into(),
            aliases: vec![],
            description: "List files referenced in the current conversation".into(),
            handler: Box::new(files::FilesHandler),
        },
        Command {
            name: "context".into(),
            aliases: vec!["ctx".into()],
            description: "Show context usage information".into(),
            handler: Box::new(context::ContextHandler),
        },
        Command {
            name: "permissions".into(),
            aliases: vec!["perms".into()],
            description: "View or modify tool permission settings".into(),
            handler: Box::new(permissions_cmd::PermissionsHandler),
        },
        Command {
            name: "login".into(),
            aliases: vec![],
            description: "Authenticate (API key, Anthropic OAuth, OpenAI Codex OAuth)".into(),
            handler: Box::new(login::LoginHandler),
        },
        Command {
            name: "login-code".into(),
            aliases: vec![],
            description: "Complete OAuth login with authorization code".into(),
            handler: Box::new(login_code::LoginCodeHandler),
        },
        Command {
            name: "logout".into(),
            aliases: vec![],
            description: "Clear stored authentication credentials".into(),
            handler: Box::new(logout::LogoutHandler),
        },
        Command {
            name: "commit".into(),
            aliases: vec![],
            description: "Create a git commit from current changes".into(),
            handler: Box::new(commit::CommitHandler),
        },
        Command {
            name: "branch".into(),
            aliases: vec!["br".into()],
            description: "Show or switch git branches".into(),
            handler: Box::new(branch::BranchHandler),
        },
        Command {
            name: "effort".into(),
            aliases: vec![],
            description: "Set the thinking effort level (low/medium/high)".into(),
            handler: Box::new(effort::EffortHandler),
        },
        Command {
            name: "fast".into(),
            aliases: vec![],
            description: "Toggle fast mode on/off".into(),
            handler: Box::new(fast::FastHandler),
        },
        Command {
            name: "memory".into(),
            aliases: vec!["mem".into()],
            description: "View and manage CLAUDE.md project instructions".into(),
            handler: Box::new(memory::MemoryHandler),
        },
        Command {
            name: "skills".into(),
            aliases: vec![],
            description: "List available skills".into(),
            handler: Box::new(skills_cmd::SkillsHandler),
        },
        Command {
            name: "init".into(),
            aliases: vec![],
            description: "Initialize project config (.cc-rust/settings.json)".into(),
            handler: Box::new(init::InitHandler),
        },
        Command {
            name: "copy".into(),
            aliases: vec!["cp".into()],
            description: "Copy the last assistant response to clipboard".into(),
            handler: Box::new(copy::CopyHandler),
        },
        Command {
            name: "status".into(),
            aliases: vec![],
            description: "Show session status".into(),
            handler: Box::new(status::StatusHandler),
        },
        Command {
            name: "export".into(),
            aliases: vec![],
            description: "Export conversation to Markdown (.md)".into(),
            handler: Box::new(export::ExportHandler),
        },
        Command {
            name: "audit-export".into(),
            aliases: vec!["audit".into()],
            description: "Export session as verifiable audit record (.audit.json)".into(),
            handler: Box::new(audit_export::AuditExportHandler),
        },
        Command {
            name: "session-export".into(),
            aliases: vec!["sexport".into()],
            description: "Export session as structured JSON data package (.session.json)".into(),
            handler: Box::new(session_export::SessionExportHandler),
        },
        Command {
            name: "extra-usage".into(),
            aliases: vec!["eu".into()],
            description: "Show extended token usage and cost analysis".into(),
            handler: Box::new(extra_usage::ExtraUsageHandler),
        },
        Command {
            name: "rate-limit-options".into(),
            aliases: vec!["rlo".into(), "rate-limit".into()],
            description: "Show rate limit information for the current model".into(),
            handler: Box::new(rate_limit::RateLimitHandler),
        },
        Command {
            name: "compact".into(),
            aliases: vec![],
            description: "Compact conversation to reduce token usage".into(),
            handler: Box::new(compact::CompactHandler),
        },
        Command {
            name: "mcp".into(),
            aliases: vec![],
            description: "MCP server management (list, status)".into(),
            handler: Box::new(mcp_cmd::McpHandler),
        },
        Command {
            name: "chrome".into(),
            aliases: vec![],
            description: "Claude in Chrome (first-party integration) status + reconnect".into(),
            handler: Box::new(chrome_cmd::ChromeHandler),
        },
        Command {
            name: "plugin".into(),
            aliases: vec![],
            description: "Plugin management (list, status, enable, disable)".into(),
            handler: Box::new(plugin_cmd::PluginHandler),
        },
        Command {
            name: "model-add".into(),
            aliases: vec!["ma".into()],
            description: "Add a model with token pricing to .env".into(),
            handler: Box::new(model_add::ModelAddHandler),
        },
        Command {
            name: "brief".into(),
            aliases: vec![],
            description: "Toggle Brief output mode (KAIROS)".into(),
            handler: Box::new(brief::BriefHandler),
        },
        Command {
            name: "sleep".into(),
            aliases: vec![],
            description: "Set proactive sleep duration".into(),
            handler: Box::new(sleep_cmd::SleepCmdHandler),
        },
        Command {
            name: "assistant".into(),
            aliases: vec!["kairos".into()],
            description: "View assistant mode status".into(),
            handler: Box::new(assistant::AssistantHandler),
        },
        Command {
            name: "daemon".into(),
            aliases: vec![],
            description: "View/control daemon process".into(),
            handler: Box::new(daemon_cmd::DaemonCmdHandler),
        },
        Command {
            name: "notify".into(),
            aliases: vec![],
            description: "Push notification settings".into(),
            handler: Box::new(notify::NotifyHandler),
        },
        Command {
            name: "channels".into(),
            aliases: vec![],
            description: "View connected channels".into(),
            handler: Box::new(channels::ChannelsHandler),
        },
        Command {
            name: "dream".into(),
            aliases: vec![],
            description: "Distill daily logs into memory".into(),
            handler: Box::new(dream::DreamHandler),
        },
        Command {
            name: "add-dir".into(),
            aliases: vec![],
            description: "Add a new working directory".into(),
            handler: Box::new(add_dir::AddDirHandler),
        },
        Command {
            name: "sandbox".into(),
            aliases: vec![],
            description: "View or toggle sandbox + network access settings".into(),
            handler: Box::new(sandbox_cmd::SandboxHandler),
        },
        Command {
            name: "keybindings".into(),
            aliases: vec!["keys".into(), "shortcuts".into()],
            description: "View, edit, or reload keybindings.json".into(),
            handler: Box::new(keybindings_cmd::KeybindingsHandler),
        },
        Command {
            name: "statusline".into(),
            aliases: vec!["status-line".into()],
            description: "View, edit, or test the scriptable status line".into(),
            handler: Box::new(statusline_cmd::StatusLineHandler),
        },
        Command {
            name: "terminal-setup".into(),
            aliases: vec!["term-setup".into(), "terminal".into()],
            description: "Diagnose terminal env + print Shift+Enter / tmux / notification tips"
                .into(),
            handler: Box::new(terminal_setup::TerminalSetupHandler),
        },
        Command {
            name: "voice".into(),
            aliases: vec!["dictation".into()],
            description: "Inspect compatibility-only voice settings (runtime voice unsupported)"
                .into(),
            handler: Box::new(voice_cmd::VoiceHandler),
        },
    ]
}

/// Find a command by name or alias from user input.
pub fn find_command(input: &str) -> Option<usize> {
    let cmd_name = input.split_whitespace().next().unwrap_or("");
    let commands = get_all_commands();

    commands
        .iter()
        .position(|c| c.name == cmd_name || c.aliases.iter().any(|a| a == cmd_name))
}

/// Parse user input into (command_index, args) if it starts with `/`.
pub fn parse_command_input(input: &str) -> Option<(usize, String)> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let without_slash = &trimmed[1..];
    let cmd_name = without_slash.split_whitespace().next().unwrap_or("");
    let args = without_slash
        .strip_prefix(cmd_name)
        .unwrap_or("")
        .trim()
        .to_string();

    find_command(without_slash).map(|idx| (idx, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_commands_registered() {
        let cmds = get_all_commands();
        assert!(cmds.len() >= 20, "Expected at least 20 commands");
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"help"));
        assert!(names.contains(&"clear"));
        assert!(names.contains(&"config"));
        assert!(names.contains(&"diff"));
        assert!(names.contains(&"exit"));
        assert!(names.contains(&"version"));
        assert!(names.contains(&"model"));
        assert!(names.contains(&"cost"));
        assert!(names.contains(&"skills"));
        assert!(names.contains(&"mcp"));
        assert!(names.contains(&"plugin"));
    }

    #[test]
    fn test_find_command_by_name() {
        assert!(find_command("help").is_some());
        assert!(find_command("clear").is_some());
        assert!(find_command("nonexistent").is_none());
    }

    #[test]
    fn test_find_command_by_alias() {
        assert!(find_command("h").is_some());
        assert!(find_command("?").is_some());
        assert!(find_command("settings").is_some());
        assert!(find_command("quit").is_some());
        assert!(find_command("q").is_some());
        assert!(find_command("v").is_some());
        assert!(find_command("usage").is_some());
        assert!(find_command("ctx").is_some());
        assert!(find_command("perms").is_some());
        assert!(find_command("br").is_some());
        assert!(find_command("mem").is_some());
    }

    #[test]
    fn test_parse_command_input() {
        assert!(parse_command_input("/help").is_some());
        assert!(parse_command_input("/config set model opus").is_some());
        assert!(parse_command_input("not a command").is_none());
        assert!(parse_command_input("").is_none());
    }
}
