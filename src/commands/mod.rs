//! Command registry -- slash commands for the interactive REPL.
//!
//! Commands are invoked by typing `/command_name [args]` in the user prompt.
//! Each command implements `CommandHandler` and is registered in `get_all_commands()`.

#![allow(unused)]

pub mod clear;
pub mod compact;
pub mod context;
pub mod cost;
pub mod exit;
pub mod files;
pub mod help;
pub mod hooks_cmd;
pub mod config_cmd;
pub mod diff;
pub mod login;
pub mod logout;
pub mod model;
pub mod permissions_cmd;
pub mod resume;
pub mod session;
pub mod version;

// Phase 14B — second batch commands
pub mod branch;
pub mod commit;
pub mod effort;
pub mod export;
pub mod fast;
pub mod memory;
pub mod plan;
pub mod rename;
pub mod review;
pub mod stats;

// Phase 14C — third batch commands
pub mod add_dir;
pub mod color;
pub mod copy;
pub mod doctor;
pub mod init;
pub mod rewind;
pub mod skills_cmd;
pub mod status;
pub mod tasks_cmd;
pub mod theme;

// Phase 14D — fourth batch commands
pub mod feedback;
pub mod force_snip;
pub mod fork;
pub mod keybindings_cmd;
pub mod mcp_cmd;
pub mod output_style;
pub mod plugin_cmd;
pub mod sandbox_cmd;
pub mod tag;
pub mod think_back;

// Phase 14E — fifth batch commands
pub mod agents;
pub mod brief;
pub mod commit_push_pr;
pub mod ide;
pub mod pr_comments;
pub mod privacy_settings;
pub mod proactive;
pub mod security_review;
pub mod upgrade;
pub mod vim_cmd;

// Phase 14F — feature-gated stub commands
pub mod buddy;
pub mod peers;
pub mod subscribe_pr;
pub mod torch;
pub mod workflows;

// Phase 14G — seventh batch commands
pub mod install_github_app;
pub mod install_slack_app;
pub mod statusline;
pub mod thinkback_play;
pub mod ultraplan;
pub mod ultrareview;

// Phase 14H — eighth batch commands
pub mod advisor;
pub mod btw;
pub mod insights;
pub mod passes;
pub mod reload_plugins;
pub mod voice;

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

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
            name: "compact".into(),
            aliases: vec![],
            description: "Compact conversation context to reduce token usage".into(),
            handler: Box::new(compact::CompactHandler),
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
            name: "hooks".into(),
            aliases: vec![],
            description: "View and manage tool execution hooks".into(),
            handler: Box::new(hooks_cmd::HooksHandler),
        },
        Command {
            name: "login".into(),
            aliases: vec![],
            description: "Authenticate with Anthropic (API key or auth token)".into(),
            handler: Box::new(login::LoginHandler),
        },
        Command {
            name: "logout".into(),
            aliases: vec![],
            description: "Clear stored authentication credentials".into(),
            handler: Box::new(logout::LogoutHandler),
        },
        // --- Second batch commands ---
        Command {
            name: "commit".into(),
            aliases: vec![],
            description: "Create a git commit from current changes".into(),
            handler: Box::new(commit::CommitHandler),
        },
        Command {
            name: "review".into(),
            aliases: vec![],
            description: "Request a code review of current changes".into(),
            handler: Box::new(review::ReviewHandler),
        },
        Command {
            name: "branch".into(),
            aliases: vec!["br".into()],
            description: "Show or switch git branches".into(),
            handler: Box::new(branch::BranchHandler),
        },
        Command {
            name: "export".into(),
            aliases: vec![],
            description: "Export conversation to a file".into(),
            handler: Box::new(export::ExportHandler),
        },
        Command {
            name: "rename".into(),
            aliases: vec![],
            description: "Rename the current session".into(),
            handler: Box::new(rename::RenameHandler),
        },
        Command {
            name: "stats".into(),
            aliases: vec![],
            description: "Show session usage statistics".into(),
            handler: Box::new(stats::StatsHandler),
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
            name: "plan".into(),
            aliases: vec![],
            description: "Toggle plan mode (read-only tools)".into(),
            handler: Box::new(plan::PlanHandler),
        },
        // --- Third batch commands ---
        Command {
            name: "add-dir".into(),
            aliases: vec!["add_dir".into()],
            description: "Add a directory to the workspace".into(),
            handler: Box::new(add_dir::AddDirHandler),
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
            name: "doctor".into(),
            aliases: vec!["diag".into()],
            description: "Run diagnostics checks".into(),
            handler: Box::new(doctor::DoctorHandler),
        },
        Command {
            name: "tasks".into(),
            aliases: vec![],
            description: "List current tasks".into(),
            handler: Box::new(tasks_cmd::TasksHandler),
        },
        Command {
            name: "status".into(),
            aliases: vec![],
            description: "Show session status".into(),
            handler: Box::new(status::StatusHandler),
        },
        Command {
            name: "theme".into(),
            aliases: vec![],
            description: "Switch UI theme".into(),
            handler: Box::new(theme::ThemeHandler),
        },
        Command {
            name: "color".into(),
            aliases: vec![],
            description: "Toggle color mode".into(),
            handler: Box::new(color::ColorHandler),
        },
        Command {
            name: "rewind".into(),
            aliases: vec!["undo".into()],
            description: "Remove the last N message pairs".into(),
            handler: Box::new(rewind::RewindHandler),
        },
        Command {
            name: "skills".into(),
            aliases: vec![],
            description: "List available skills".into(),
            handler: Box::new(skills_cmd::SkillsHandler),
        },
        // --- Fourth batch commands ---
        Command {
            name: "mcp".into(),
            aliases: vec![],
            description: "MCP server management".into(),
            handler: Box::new(mcp_cmd::McpHandler),
        },
        Command {
            name: "plugin".into(),
            aliases: vec!["plugins".into()],
            description: "Plugin management".into(),
            handler: Box::new(plugin_cmd::PluginHandler),
        },
        Command {
            name: "keybindings".into(),
            aliases: vec!["keys".into()],
            description: "Show current key bindings".into(),
            handler: Box::new(keybindings_cmd::KeybindingsHandler),
        },
        Command {
            name: "feedback".into(),
            aliases: vec![],
            description: "Show how to give feedback".into(),
            handler: Box::new(feedback::FeedbackHandler),
        },
        Command {
            name: "tag".into(),
            aliases: vec![],
            description: "Tag the current session with a label".into(),
            handler: Box::new(tag::TagHandler),
        },
        Command {
            name: "think-back".into(),
            aliases: vec!["thinking".into()],
            description: "Review model's thinking from the conversation".into(),
            handler: Box::new(think_back::ThinkBackHandler),
        },
        Command {
            name: "sandbox".into(),
            aliases: vec![],
            description: "Toggle sandbox mode".into(),
            handler: Box::new(sandbox_cmd::SandboxHandler),
        },
        Command {
            name: "force-snip".into(),
            aliases: vec!["snip".into()],
            description: "Force snip conversation history".into(),
            handler: Box::new(force_snip::ForceSnipHandler),
        },
        Command {
            name: "fork".into(),
            aliases: vec![],
            description: "Fork the current conversation into a new session".into(),
            handler: Box::new(fork::ForkHandler),
        },
        Command {
            name: "output-style".into(),
            aliases: vec!["style".into()],
            description: "Configure output formatting".into(),
            handler: Box::new(output_style::OutputStyleHandler),
        },
        // --- Fifth batch commands ---
        Command {
            name: "agents".into(),
            aliases: vec!["team".into()],
            description: "List and manage agent teams".into(),
            handler: Box::new(agents::AgentsHandler),
        },
        Command {
            name: "upgrade".into(),
            aliases: vec![],
            description: "Check for upgrades".into(),
            handler: Box::new(upgrade::UpgradeHandler),
        },
        Command {
            name: "ide".into(),
            aliases: vec![],
            description: "IDE integration info".into(),
            handler: Box::new(ide::IdeHandler),
        },
        Command {
            name: "privacy-settings".into(),
            aliases: vec!["privacy".into()],
            description: "Show or toggle privacy and telemetry settings".into(),
            handler: Box::new(privacy_settings::PrivacySettingsHandler),
        },
        Command {
            name: "security-review".into(),
            aliases: vec!["sec-review".into()],
            description: "Request a security review of recent changes".into(),
            handler: Box::new(security_review::SecurityReviewHandler),
        },
        Command {
            name: "pr-comments".into(),
            aliases: vec!["pr-review".into()],
            description: "Review and respond to PR comments".into(),
            handler: Box::new(pr_comments::PrCommentsHandler),
        },
        Command {
            name: "commit-push-pr".into(),
            aliases: vec!["cpp".into()],
            description: "Commit, push, and create a pull request".into(),
            handler: Box::new(commit_push_pr::CommitPushPrHandler),
        },
        Command {
            name: "brief".into(),
            aliases: vec![],
            description: "Toggle brief output mode".into(),
            handler: Box::new(brief::BriefHandler),
        },
        Command {
            name: "proactive".into(),
            aliases: vec![],
            description: "Toggle proactive suggestions".into(),
            handler: Box::new(proactive::ProactiveHandler),
        },
        Command {
            name: "vim".into(),
            aliases: vec![],
            description: "Toggle vim keybinding mode".into(),
            handler: Box::new(vim_cmd::VimHandler),
        },
        // --- Feature-gated stub commands ---
        Command {
            name: "workflows".into(),
            aliases: vec!["wf".into()],
            description: "Manage workflow scripts".into(),
            handler: Box::new(workflows::WorkflowsHandler),
        },
        Command {
            name: "subscribe-pr".into(),
            aliases: vec!["sub-pr".into()],
            description: "Subscribe to pull request updates".into(),
            handler: Box::new(subscribe_pr::SubscribePrHandler),
        },
        Command {
            name: "peers".into(),
            aliases: vec![],
            description: "Peer session management".into(),
            handler: Box::new(peers::PeersHandler),
        },
        Command {
            name: "buddy".into(),
            aliases: vec![],
            description: "AI buddy companion mode".into(),
            handler: Box::new(buddy::BuddyHandler),
        },
        Command {
            name: "torch".into(),
            aliases: vec![],
            description: "Hand off context to another session".into(),
            handler: Box::new(torch::TorchHandler),
        },
        // --- Seventh batch commands ---
        Command {
            name: "statusline".into(),
            aliases: vec![],
            description: "Set up status line via subagent".into(),
            handler: Box::new(statusline::StatuslineHandler),
        },
        Command {
            name: "ultrareview".into(),
            aliases: vec![],
            description: "Remote bug finder (requires Claude Code on the web)".into(),
            handler: Box::new(ultrareview::UltrareviewHandler),
        },
        Command {
            name: "ultraplan".into(),
            aliases: vec![],
            description: "Remote multi-agent planning (requires Claude Code on the web)".into(),
            handler: Box::new(ultraplan::UltraplanHandler),
        },
        Command {
            name: "thinkback-play".into(),
            aliases: vec![],
            description: "Play thinkback animation".into(),
            handler: Box::new(thinkback_play::ThinkbackPlayHandler),
        },
        Command {
            name: "install-github-app".into(),
            aliases: vec![],
            description: "Set up GitHub Actions integration".into(),
            handler: Box::new(install_github_app::InstallGithubAppHandler),
        },
        Command {
            name: "install-slack-app".into(),
            aliases: vec![],
            description: "Install the Claude Slack app".into(),
            handler: Box::new(install_slack_app::InstallSlackAppHandler),
        },
        // --- Eighth batch commands ---
        Command {
            name: "voice".into(),
            aliases: vec![],
            description: "Toggle voice mode".into(),
            handler: Box::new(voice::VoiceHandler),
        },
        Command {
            name: "advisor".into(),
            aliases: vec![],
            description: "Configure advisor model".into(),
            handler: Box::new(advisor::AdvisorHandler),
        },
        Command {
            name: "btw".into(),
            aliases: vec![],
            description: "Ask a quick side question".into(),
            handler: Box::new(btw::BtwHandler),
        },
        Command {
            name: "insights".into(),
            aliases: vec![],
            description: "Analyze the current session".into(),
            handler: Box::new(insights::InsightsHandler),
        },
        Command {
            name: "passes".into(),
            aliases: vec!["referral".into()],
            description: "Show referral program information".into(),
            handler: Box::new(passes::PassesHandler),
        },
        Command {
            name: "reload-plugins".into(),
            aliases: vec![],
            description: "Reload plugins from disk".into(),
            handler: Box::new(reload_plugins::ReloadPluginsHandler),
        },
    ]
}

/// Find a command by name or alias from user input.
///
/// The `input` should be the text after the leading `/`, e.g. `"help"` or
/// `"config set model claude-opus"`. Returns a reference to the matching
/// `Command` from the global registry, or `None`.
///
/// Note: This creates the command list on each call. In a real application
/// you would cache the registry in a `LazyLock` or similar.
pub fn find_command(input: &str) -> Option<usize> {
    let cmd_name = input.split_whitespace().next().unwrap_or("");
    let commands = get_all_commands();

    commands
        .iter()
        .position(|c| c.name == cmd_name || c.aliases.iter().any(|a| a == cmd_name))
}

/// Parse user input into (command_index, args) if it starts with `/`.
///
/// Returns `None` if the input does not start with `/` or no matching command
/// is found.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_commands_registered() {
        let cmds = get_all_commands();
        assert!(cmds.len() >= 74, "Expected at least 74 commands");
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"help"));
        assert!(names.contains(&"clear"));
        assert!(names.contains(&"compact"));
        assert!(names.contains(&"config"));
        assert!(names.contains(&"diff"));
        assert!(names.contains(&"exit"));
        assert!(names.contains(&"version"));
        assert!(names.contains(&"model"));
        assert!(names.contains(&"cost"));
        assert!(names.contains(&"session"));
        assert!(names.contains(&"resume"));
        assert!(names.contains(&"files"));
        assert!(names.contains(&"context"));
        assert!(names.contains(&"permissions"));
        assert!(names.contains(&"hooks"));
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
    fn test_second_batch_commands_registered() {
        let cmds = get_all_commands();
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"commit"));
        assert!(names.contains(&"review"));
        assert!(names.contains(&"branch"));
        assert!(names.contains(&"export"));
        assert!(names.contains(&"rename"));
        assert!(names.contains(&"stats"));
        assert!(names.contains(&"effort"));
        assert!(names.contains(&"fast"));
        assert!(names.contains(&"memory"));
        assert!(names.contains(&"plan"));
    }

    #[test]
    fn test_parse_command_input() {
        assert!(parse_command_input("/help").is_some());
        assert!(parse_command_input("/config set model opus").is_some());
        assert!(parse_command_input("not a command").is_none());
        assert!(parse_command_input("").is_none());
    }
}
