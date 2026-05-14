//! Slash-command contract and low-coupling command implementations.

pub mod agents_cmd;
pub mod browser;
pub mod clear;
pub mod config_cmd;
pub mod exit;
pub mod lsp_cmd;
pub mod mcp;
pub mod memory;
pub mod model;
pub mod plan;
pub mod plan_workflow;
pub mod session;
pub mod tasks_cmd;
pub mod team_cmd;
pub mod version;

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use cc_bootstrap::SessionId;
use cc_engine::types::app_state::AppState;
use cc_types::message::Message;

pub mod runtime {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{OnceLock, RwLock};

    use cc_ipc_protocol::subsystem_types::{LspRecommendationSettings, LspServerInfo};
    use cc_tasks::TaskEntry;

    use crate::CommandContext;

    type Installer = fn();
    type LspServersProvider = fn() -> Vec<LspServerInfo>;
    type LspRecommendationSettingsProvider = fn() -> LspRecommendationSettings;
    type BuiltinAgentsProvider = fn() -> Vec<BuiltinAgentEntry>;
    type BuiltinAgentPromptProvider = fn(&str) -> Option<String>;
    type TaskListProvider = fn() -> Vec<TaskEntry>;
    type TaskGetProvider = fn(&str) -> Option<TaskEntry>;
    type TaskMutateProvider = fn(&str) -> Result<Option<TaskEntry>, String>;
    type TeamTaskSnapshotProvider = fn() -> Vec<TeamTaskSnapshot>;
    type TeamCommandExecutor = for<'a> fn(
        &'a str,
        &'a mut CommandContext,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>>;

    #[derive(Debug, Clone)]
    pub struct BuiltinAgentEntry {
        pub name: String,
        pub description: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TeamTaskStatus {
        Running,
        Stopped,
        Completed,
    }

    #[derive(Debug, Clone)]
    pub struct TeamTaskSnapshot {
        pub id: String,
        pub agent_id: String,
        pub agent_name: String,
        pub team_name: String,
        pub status: TeamTaskStatus,
        pub is_idle: bool,
        pub has_error: bool,
        pub error_message: Option<String>,
        pub prompt: String,
        pub model: Option<String>,
        pub awaiting_plan_approval: bool,
        pub permission_mode: String,
    }

    static INSTALLER: OnceLock<RwLock<Option<Installer>>> = OnceLock::new();
    static LSP_SERVERS_PROVIDER: OnceLock<RwLock<Option<LspServersProvider>>> = OnceLock::new();
    static LSP_SETTINGS_PROVIDER: OnceLock<RwLock<Option<LspRecommendationSettingsProvider>>> =
        OnceLock::new();
    static BUILTIN_AGENTS_PROVIDER: OnceLock<RwLock<Option<BuiltinAgentsProvider>>> =
        OnceLock::new();
    static BUILTIN_AGENT_PROMPT_PROVIDER: OnceLock<RwLock<Option<BuiltinAgentPromptProvider>>> =
        OnceLock::new();
    static TASK_LIST_PROVIDER: OnceLock<RwLock<Option<TaskListProvider>>> = OnceLock::new();
    static TASK_GET_PROVIDER: OnceLock<RwLock<Option<TaskGetProvider>>> = OnceLock::new();
    static TASK_STOP_PROVIDER: OnceLock<RwLock<Option<TaskMutateProvider>>> = OnceLock::new();
    static TASK_DELETE_PROVIDER: OnceLock<RwLock<Option<TaskMutateProvider>>> = OnceLock::new();
    static TEAM_TASK_SNAPSHOT_PROVIDER: OnceLock<RwLock<Option<TeamTaskSnapshotProvider>>> =
        OnceLock::new();
    static TEAM_COMMAND_EXECUTOR: OnceLock<RwLock<Option<TeamCommandExecutor>>> = OnceLock::new();

    pub fn set_runtime_installer(installer: Installer) {
        let slot = INSTALLER.get_or_init(|| RwLock::new(None));
        if let Ok(mut guard) = slot.write() {
            *guard = Some(installer);
        }
    }

    pub fn set_lsp_runtime_providers(
        servers: LspServersProvider,
        settings: LspRecommendationSettingsProvider,
    ) {
        let servers_slot = LSP_SERVERS_PROVIDER.get_or_init(|| RwLock::new(None));
        if let Ok(mut guard) = servers_slot.write() {
            *guard = Some(servers);
        }

        let settings_slot = LSP_SETTINGS_PROVIDER.get_or_init(|| RwLock::new(None));
        if let Ok(mut guard) = settings_slot.write() {
            *guard = Some(settings);
        }
    }

    pub fn set_agent_runtime_providers(
        builtins: BuiltinAgentsProvider,
        prompt: BuiltinAgentPromptProvider,
    ) {
        set_provider(&BUILTIN_AGENTS_PROVIDER, builtins);
        set_provider(&BUILTIN_AGENT_PROMPT_PROVIDER, prompt);
    }

    pub fn set_task_runtime_providers(
        list: TaskListProvider,
        get: TaskGetProvider,
        stop: TaskMutateProvider,
        delete: TaskMutateProvider,
        team_snapshots: TeamTaskSnapshotProvider,
    ) {
        set_provider(&TASK_LIST_PROVIDER, list);
        set_provider(&TASK_GET_PROVIDER, get);
        set_provider(&TASK_STOP_PROVIDER, stop);
        set_provider(&TASK_DELETE_PROVIDER, delete);
        set_provider(&TEAM_TASK_SNAPSHOT_PROVIDER, team_snapshots);
    }

    pub fn set_team_command_executor(executor: TeamCommandExecutor) {
        set_provider(&TEAM_COMMAND_EXECUTOR, executor);
    }

    fn set_provider<T: Copy>(slot: &OnceLock<RwLock<Option<T>>>, provider: T) {
        let slot = slot.get_or_init(|| RwLock::new(None));
        if let Ok(mut guard) = slot.write() {
            *guard = Some(provider);
        }
    }

    pub(crate) fn ensure_runtime_installed() {
        let Some(slot) = INSTALLER.get() else {
            return;
        };
        let Ok(guard) = slot.read() else {
            return;
        };
        if let Some(installer) = *guard {
            installer();
        }
    }

    pub(crate) fn lsp_server_info_list() -> Vec<LspServerInfo> {
        ensure_runtime_installed();
        LSP_SERVERS_PROVIDER
            .get()
            .and_then(|slot| slot.read().ok().and_then(|guard| *guard))
            .map(|provider| provider())
            .unwrap_or_default()
    }

    pub(crate) fn lsp_recommendation_settings() -> LspRecommendationSettings {
        ensure_runtime_installed();
        LSP_SETTINGS_PROVIDER
            .get()
            .and_then(|slot| slot.read().ok().and_then(|guard| *guard))
            .map(|provider| provider())
            .unwrap_or_default()
    }

    pub(crate) fn builtin_agent_entries() -> Vec<BuiltinAgentEntry> {
        ensure_runtime_installed();
        get_provider(&BUILTIN_AGENTS_PROVIDER)
            .map(|provider| provider())
            .unwrap_or_default()
    }

    pub(crate) fn builtin_agent_prompt(name: &str) -> Option<String> {
        ensure_runtime_installed();
        get_provider(&BUILTIN_AGENT_PROMPT_PROVIDER).and_then(|provider| provider(name))
    }

    pub(crate) fn tool_tasks() -> Vec<TaskEntry> {
        ensure_runtime_installed();
        get_provider(&TASK_LIST_PROVIDER)
            .map(|provider| provider())
            .unwrap_or_default()
    }

    pub(crate) fn get_tool_task(id: &str) -> Option<TaskEntry> {
        ensure_runtime_installed();
        get_provider(&TASK_GET_PROVIDER).and_then(|provider| provider(id))
    }

    pub(crate) fn stop_tool_task(id: &str) -> Result<Option<TaskEntry>, String> {
        ensure_runtime_installed();
        get_provider(&TASK_STOP_PROVIDER)
            .map(|provider| provider(id))
            .unwrap_or(Ok(None))
    }

    pub(crate) fn delete_tool_task(id: &str) -> Result<Option<TaskEntry>, String> {
        ensure_runtime_installed();
        get_provider(&TASK_DELETE_PROVIDER)
            .map(|provider| provider(id))
            .unwrap_or(Ok(None))
    }

    pub(crate) fn team_task_snapshots() -> Vec<TeamTaskSnapshot> {
        ensure_runtime_installed();
        get_provider(&TEAM_TASK_SNAPSHOT_PROVIDER)
            .map(|provider| provider())
            .unwrap_or_default()
    }

    pub(crate) async fn execute_team_command(args: &str, ctx: &mut CommandContext) -> String {
        ensure_runtime_installed();
        match get_provider(&TEAM_COMMAND_EXECUTOR) {
            Some(executor) => executor(args, ctx).await,
            None => "Team command runtime is unavailable.".to_string(),
        }
    }

    fn get_provider<T: Copy>(slot: &OnceLock<RwLock<Option<T>>>) -> Option<T> {
        slot.get()
            .and_then(|slot| slot.read().ok().and_then(|guard| *guard))
    }
}

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

impl Command {
    pub fn metadata(&self) -> CommandMetadata {
        CommandMetadata {
            name: self.name.clone(),
            aliases: self.aliases.clone(),
            description: self.description.clone(),
        }
    }
}

/// Command metadata used by parsers and dispatchers without handler ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMetadata {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
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
    /// Clear the visible conversation by starting a fresh session.
    Clear,
    /// Exit the REPL with a goodbye message.
    Exit(String),
    /// No visible output.
    #[allow(dead_code)]
    None,
}

pub fn command<H>(name: &str, aliases: &[&str], description: &str, handler: H) -> Command
where
    H: CommandHandler + 'static,
{
    Command {
        name: name.to_string(),
        aliases: aliases.iter().map(|alias| (*alias).to_string()).collect(),
        description: description.to_string(),
        handler: Box::new(handler),
    }
}

pub fn sort_commands_for_display(commands: &mut [Command]) {
    commands.sort_by(|a, b| match (a.name.as_str(), b.name.as_str()) {
        ("init", "init") => std::cmp::Ordering::Equal,
        ("init", _) => std::cmp::Ordering::Less,
        (_, "init") => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
}

pub fn command_metadata(commands: &[Command]) -> Vec<CommandMetadata> {
    commands.iter().map(Command::metadata).collect()
}

/// Find a command by name or alias from user input.
pub fn find_command_in(input: &str, commands: &[CommandMetadata]) -> Option<usize> {
    let cmd_name = input.split_whitespace().next().unwrap_or("");

    commands
        .iter()
        .position(|c| c.name == cmd_name || c.aliases.iter().any(|a| a == cmd_name))
}

/// Parse user input into (command_index, args) if it starts with `/`.
pub fn parse_command_input_in(
    input: &str,
    commands: &[CommandMetadata],
) -> Option<(usize, String)> {
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

    find_command_in(without_slash, commands).map(|idx| (idx, args))
}

/// Concrete [`cc_types::commands::CommandDispatcher`] backed by command metadata.
pub struct DefaultCommandDispatcher {
    commands: Vec<CommandMetadata>,
}

impl DefaultCommandDispatcher {
    pub fn new(commands: Vec<CommandMetadata>) -> Self {
        Self { commands }
    }

    pub fn from_commands(commands: &[Command]) -> Self {
        Self::new(command_metadata(commands))
    }
}

impl cc_types::commands::CommandDispatcher for DefaultCommandDispatcher {
    fn parse_command_input(&self, input: &str) -> Option<cc_types::commands::ParsedCommand> {
        parse_command_input_in(input, &self.commands)
            .map(|(index, args)| cc_types::commands::ParsedCommand { index, args })
    }

    fn command_name(&self, index: usize) -> Option<String> {
        self.commands.get(index).map(|cmd| cmd.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_types::commands::CommandDispatcher;

    fn sample_commands() -> Vec<Command> {
        let mut commands = vec![
            command("help", &["h", "?"], "Show help", clear::ClearHandler),
            command("init", &[], "Init", clear::ClearHandler),
            command("config", &["settings"], "Configure", clear::ClearHandler),
        ];
        sort_commands_for_display(&mut commands);
        commands
    }

    #[test]
    fn parser_resolves_names_and_aliases() {
        let metadata = command_metadata(&sample_commands());

        assert_eq!(find_command_in("help", &metadata), Some(2));
        assert_eq!(find_command_in("settings", &metadata), Some(1));
        assert_eq!(find_command_in("missing", &metadata), None);
        assert_eq!(
            parse_command_input_in("/config set model SOTA", &metadata),
            Some((1, "set model SOTA".to_string()))
        );
        assert_eq!(parse_command_input_in("not a command", &metadata), None);
    }

    #[test]
    fn dispatcher_uses_stable_metadata_snapshot() {
        let dispatcher = DefaultCommandDispatcher::from_commands(&sample_commands());

        let parsed = dispatcher.parse_command_input("/h").unwrap();
        assert_eq!(parsed.index, 2);
        assert_eq!(parsed.args, "");
        assert_eq!(dispatcher.command_name(0).as_deref(), Some("init"));
    }
}
