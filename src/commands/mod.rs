//! Command registry -- slash commands for the interactive REPL.
//!
//! Commands are invoked by typing `/command_name [args]` in the user prompt.
//! Each command implements `CommandHandler` and is registered in `get_all_commands()`.

#![allow(unused)]

pub mod clear;
pub mod compact;
pub mod help;
pub mod config_cmd;
pub mod diff;

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
        assert!(cmds.len() >= 5, "Expected at least 5 commands");
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"help"));
        assert!(names.contains(&"clear"));
        assert!(names.contains(&"compact"));
        assert!(names.contains(&"config"));
        assert!(names.contains(&"diff"));
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
    }

    #[test]
    fn test_parse_command_input() {
        assert!(parse_command_input("/help").is_some());
        assert!(parse_command_input("/config set model opus").is_some());
        assert!(parse_command_input("not a command").is_none());
        assert!(parse_command_input("").is_none());
    }
}
