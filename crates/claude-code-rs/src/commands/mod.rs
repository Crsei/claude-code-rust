//! Command registry -- slash commands for the interactive REPL.
//!
//! Commands are invoked by typing `/command_name [args]` in the user prompt.
//! Each command implements `CommandHandler` and is registered in `get_all_commands()`.

mod runtime_bridge;

use cc_commands::{command, command_metadata, sort_commands_for_display, Command};
use cc_engine::command_runtime;

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Build the full list of available commands.
pub fn get_all_commands() -> Vec<Command> {
    cc_commands::runtime::set_runtime_installer(crate::ipc::runtime_adapters::ensure_installed);
    cc_commands::runtime::set_lsp_runtime_providers(
        crate::ipc::subsystem_handlers::build_lsp_server_info_list,
        crate::ipc::subsystem_handlers::load_lsp_recommendation_settings,
    );
    runtime_bridge::install_command_runtime_providers();

    let mut commands = vec![
        command(
            "help",
            &["h", "?"],
            "Help V2: commands, quick surfaces, keys, and diagnostics hints",
            cc_commands::help::HelpHandler,
        ),
        command(
            "clear",
            &[],
            "Clear the conversation history",
            cc_commands::clear::ClearHandler,
        ),
        command(
            "config",
            &["settings"],
            "Show or modify configuration settings",
            cc_commands::config_cmd::ConfigHandler,
        ),
        command(
            "diff",
            &[],
            "Show git diff of current changes",
            cc_commands::diff::DiffHandler,
        ),
        command(
            "exit",
            &["quit", "q"],
            "Exit the REPL via the normal exit flow",
            cc_commands::exit::ExitHandler,
        ),
        command(
            "version",
            &["v"],
            "Show the current version",
            cc_commands::version::VersionHandler,
        ),
        command(
            "model",
            &[],
            "Show or switch the active model",
            cc_commands::model::ModelHandler,
        ),
        command(
            "cost",
            &["usage"],
            "Show token usage and cost for the current session",
            cc_commands::cost::CostHandler,
        ),
        command(
            "session",
            &[],
            "Show current session info or list saved sessions",
            cc_commands::session::SessionHandler,
        ),
        command(
            "resume",
            &["sessions", "preview"],
            "Resume or preview a previous saved session",
            cc_commands::resume::ResumeHandler,
        ),
        command(
            "rename",
            &[],
            "Set or clear the custom title for the current session",
            cc_commands::rename::RenameHandler,
        ),
        command(
            "rewind",
            &[],
            "Rewind the conversation to an earlier user turn",
            cc_commands::rewind::RewindHandler,
        ),
        command(
            "insights",
            &[],
            "Session history analytics (cross-session statistics)",
            cc_commands::insights::InsightsHandler,
        ),
        command(
            "files",
            &[],
            "List files referenced in the current conversation",
            cc_commands::files::FilesHandler,
        ),
        command(
            "context",
            &["ctx"],
            "Show context usage information",
            cc_commands::context::ContextHandler,
        ),
        command(
            "coordinator",
            &["coord"],
            "Enable or inspect coordinator mode for Agent Teams",
            cc_commands::coordinator::CoordinatorHandler,
        ),
        command(
            "permissions",
            &["perms"],
            "View or modify tool permission settings",
            cc_commands::permissions_cmd::PermissionsHandler,
        ),
        command(
            "plan",
            &[],
            "Enter plan mode and show or edit the plan file",
            cc_commands::plan::PlanHandler,
        ),
        command(
            "login",
            &[],
            "Authenticate (API key, Anthropic OAuth, OpenAI Codex OAuth, Bedrock, Vertex)",
            cc_commands::login::LoginHandler,
        ),
        command(
            "login-code",
            &[],
            "Complete OAuth login with authorization code",
            cc_commands::login_code::LoginCodeHandler,
        ),
        command(
            "logout",
            &[],
            "Clear stored authentication credentials",
            cc_commands::logout::LogoutHandler,
        ),
        command(
            "commit",
            &[],
            "Create a git commit from current changes",
            cc_commands::commit::CommitHandler,
        ),
        command(
            "branch",
            &["br"],
            "Fork the current conversation into a new branch",
            cc_commands::branch::BranchHandler,
        ),
        command(
            "gbranch",
            &["gitbranch"],
            "Show or switch git branches",
            cc_commands::gbranch::GitBranchHandler,
        ),
        command(
            "effort",
            &[],
            "Set the thinking effort level (low/medium/high)",
            cc_commands::effort::EffortHandler,
        ),
        command(
            "fast",
            &[],
            "Toggle fast mode on/off",
            cc_commands::fast::FastHandler,
        ),
        command(
            "memory",
            &["mem", "global-search", "quick-open"],
            "View, search, and quick-open memory/project instructions",
            cc_commands::memory::MemoryHandler,
        ),
        command(
            "skills",
            &[],
            "List available skills",
            cc_commands::skills_cmd::SkillsHandler,
        ),
        command(
            "init",
            &[],
            "Initialize project config and CLAUDE.md",
            cc_commands::init::InitHandler,
        ),
        command(
            "copy",
            &["cp"],
            "Copy the last assistant response to clipboard",
            cc_commands::copy::CopyHandler,
        ),
        command(
            "status",
            &[],
            "Show session status",
            cc_commands::status::StatusHandler,
        ),
        command(
            "export",
            &["markdown-export"],
            "Export conversation to Markdown (.md)",
            cc_commands::export::ExportHandler,
        ),
        command(
            "experimental",
            &["experiments", "exp"],
            "Inspect or override experimental feature gates",
            cc_commands::experimental::ExperimentalHandler,
        ),
        command(
            "audit-export",
            &["audit"],
            "Export session as verifiable audit record (.audit.json)",
            cc_commands::audit_export::AuditExportHandler,
        ),
        command(
            "session-export",
            &["sexport", "structured-export"],
            "Export session as structured JSON data package (.session.json)",
            cc_commands::session_export::SessionExportHandler,
        ),
        command(
            "extra-usage",
            &["eu"],
            "Show extended token usage and cost analysis",
            cc_commands::extra_usage::ExtraUsageHandler,
        ),
        command(
            "rate-limit-options",
            &["rlo", "rate-limit"],
            "Show rate limit information for the current model",
            cc_commands::rate_limit::RateLimitHandler,
        ),
        command(
            "compact",
            &[],
            "Compact conversation to reduce token usage",
            cc_commands::compact::CompactHandler,
        ),
        command(
            "mcp",
            &[],
            "MCP server management (list, status, add, edit, remove, approve, reject, connect)",
            cc_commands::mcp::McpHandler,
        ),
        command(
            "ide",
            &[],
            "Detect, select, or reconnect the IDE MCP bridge",
            cc_commands::ide_cmd::IdeHandler,
        ),
        command(
            "lsp",
            &[],
            "Show LSP server cards and recommendation settings",
            cc_commands::lsp_cmd::LspHandler,
        ),
        command(
            "chrome",
            &[],
            "Claude in Chrome (first-party integration) status + reconnect",
            cc_commands::chrome_cmd::ChromeHandler,
        ),
        command(
            "plugin",
            &[],
            "Plugin management (list, status, enable, disable)",
            cc_commands::plugin_cmd::PluginHandler,
        ),
        command(
            "reload-plugins",
            &[],
            "Hot-refresh the plugin registry",
            cc_commands::reload_plugins_cmd::ReloadPluginsHandler,
        ),
        command(
            "model-add",
            &["ma"],
            "Add a model with token pricing to .env",
            cc_commands::model_add::ModelAddHandler,
        ),
        command(
            "brief",
            &[],
            "Toggle Brief output mode (KAIROS)",
            cc_commands::brief::BriefHandler,
        ),
        command(
            "sleep",
            &[],
            "Set proactive sleep duration",
            cc_commands::sleep_cmd::SleepCmdHandler,
        ),
        command(
            "assistant",
            &["kairos"],
            "View assistant mode status",
            cc_commands::assistant::AssistantHandler,
        ),
        command(
            "daemon",
            &[],
            "View/control daemon process",
            cc_commands::daemon_cmd::DaemonCmdHandler,
        ),
        command(
            "notify",
            &[],
            "Push notification settings",
            cc_commands::notify::NotifyHandler,
        ),
        command(
            "remote",
            &[],
            "Inspect and control the local remote-control gateway",
            cc_commands::remote_cmd::RemoteHandler,
        ),
        command(
            "channels",
            &[],
            "View connected channels",
            cc_commands::channels::ChannelsHandler,
        ),
        command(
            "dream",
            &["logs"],
            "Distill daily logs into memory",
            cc_commands::dream::DreamHandler,
        ),
        command(
            "add-dir",
            &[],
            "Add a new working directory",
            cc_commands::add_dir::AddDirHandler,
        ),
        command(
            "sandbox",
            &[],
            "View or toggle sandbox + network access settings",
            cc_commands::sandbox_cmd::SandboxHandler,
        ),
        command(
            "keybindings",
            &["keys", "shortcuts"],
            "View, edit, or reload keybindings.json",
            cc_commands::keybindings_cmd::KeybindingsHandler,
        ),
        command(
            "statusline",
            &["status-line"],
            "View, edit, or test the scriptable status line",
            cc_commands::statusline_cmd::StatusLineHandler,
        ),
        command(
            "terminal-setup",
            &["term-setup", "terminal"],
            "Diagnose terminal env + print Shift+Enter / tmux / notification tips",
            cc_commands::terminal_setup::TerminalSetupHandler,
        ),
        command(
            "voice",
            &["dictation"],
            "Inspect compatibility-only voice settings (runtime voice unsupported)",
            cc_commands::voice_cmd::VoiceHandler,
        ),
        command(
            "team",
            &["teams"],
            "Manage Agent Teams (create, list, spawn, send, kill, leave, delete)",
            cc_commands::team_cmd::TeamHandler,
        ),
        command(
            "review",
            &[],
            "Review a pull request using a local gh pr workflow",
            cc_commands::review::ReviewHandler,
        ),
        command(
            "security-review",
            &["secreview"],
            "Run a focused security review of the current branch diff",
            cc_commands::security_review::SecurityReviewHandler,
        ),
        command(
            "recap",
            &[],
            "Summarize the current session (short | long)",
            cc_commands::recap::RecapHandler,
        ),
        command(
            "hooks",
            &[],
            "Read-only merged hook tree (managed + user + project + local)",
            cc_commands::hooks_cmd::HooksHandler,
        ),
        command(
            "agents",
            // Keep only the plural command. `/agents` owns both list and detail
            // modes; adding `/agent` would split docs/palette discoverability
            // without adding behavior.
            &[],
            "Browse agent definitions with source + override visibility",
            cc_commands::agents_cmd::AgentsHandler,
        ),
        command(
            "doctor",
            &["diagnostics", "diag"],
            "Aggregated diagnostics (install, auth, settings, MCP, keybindings, terminal)",
            cc_commands::doctor::DoctorHandler,
        ),
        command(
            "tasks",
            &[],
            "List and drill into background tasks (tool + team)",
            cc_commands::tasks_cmd::TasksHandler,
        ),
        // Fork-agent dependent commands.
        command(
            "btw",
            &[],
            "Ask a side question in a forked agent",
            cc_commands::btw::BtwHandler,
        ),
        command(
            "simplify",
            &[],
            "Run a multi-agent simplification review of recent changes",
            cc_commands::simplify::SimplifyHandler,
        ),
        command(
            "advisor",
            &[],
            "Show, set, or clear the advisor model",
            cc_commands::advisor::AdvisorHandler,
        ),
        // Scheduling / automation (issues #58, #60).
        command(
            "loop",
            &[],
            "Register a recurring local task or slash command and run it once",
            cc_commands::loop_cmd::LoopHandler,
        ),
        command(
            "schedule",
            &["cron"],
            "Manage local cron tasks (add, list, pause, trigger, remove)",
            cc_commands::schedule::ScheduleHandler,
        ),
        // Team onboarding (issue #63).
        command(
            "team-onboarding",
            &["teamonboarding"],
            "Generate a teammate onboarding guide from project and team state",
            cc_commands::team_onboarding::TeamOnboardingHandler,
        ),
    ];
    sort_commands_for_display(&mut commands);
    commands
}

/// Find a command by name or alias from user input.
#[cfg(test)]
pub fn find_command(input: &str) -> Option<usize> {
    cc_commands::find_command_in(input, &command_metadata(&get_all_commands()))
}

/// Parse user input into (command_index, args) if it starts with `/`.
pub fn parse_command_input(input: &str) -> Option<(usize, String)> {
    cc_commands::parse_command_input_in(input, &command_metadata(&get_all_commands()))
}

// ---------------------------------------------------------------------------
// CommandDispatcher trait implementation
// ---------------------------------------------------------------------------

/// Concrete [`cc_types::commands::CommandDispatcher`] for the full command
/// registry.  Used to inject command parsing into the engine without the
/// engine importing `commands::` directly (see issue #74, Phase 5c).
pub struct DefaultCommandDispatcher {
    inner: cc_commands::DefaultCommandDispatcher,
}

impl DefaultCommandDispatcher {
    pub fn new() -> Self {
        install_engine_command_executor();
        Self {
            inner: cc_commands::DefaultCommandDispatcher::new(
                command_metadata(&get_all_commands()),
            ),
        }
    }
}

impl Default for DefaultCommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl cc_types::commands::CommandDispatcher for DefaultCommandDispatcher {
    fn parse_command_input(&self, input: &str) -> Option<cc_types::commands::ParsedCommand> {
        cc_types::commands::CommandDispatcher::parse_command_input(&self.inner, input)
    }

    fn command_name(&self, index: usize) -> Option<String> {
        cc_types::commands::CommandDispatcher::command_name(&self.inner, index)
    }
}

pub struct EngineCommandExecutor;

#[async_trait::async_trait]
impl command_runtime::CommandExecutor for EngineCommandExecutor {
    async fn execute(
        &self,
        parsed: cc_types::commands::ParsedCommand,
        command_name: String,
        ctx: &mut command_runtime::CommandContext,
    ) -> anyhow::Result<command_runtime::CommandResult> {
        let mut commands = get_all_commands();
        let Some(command) = commands.get_mut(parsed.index) else {
            anyhow::bail!("Unknown command: /{}", command_name);
        };

        let mut root_ctx = cc_commands::CommandContext {
            messages: ctx.messages.clone(),
            cwd: ctx.cwd.clone(),
            app_state: ctx.app_state.clone(),
            session_id: ctx.session_id.clone(),
        };

        let result = command.handler.execute(&parsed.args, &mut root_ctx).await?;
        ctx.messages = root_ctx.messages;
        ctx.cwd = root_ctx.cwd;
        ctx.app_state = root_ctx.app_state;
        ctx.session_id = root_ctx.session_id;

        Ok(match result {
            cc_commands::CommandResult::Output(text) => {
                command_runtime::CommandResult::Output(text)
            }
            cc_commands::CommandResult::Query(messages) => {
                command_runtime::CommandResult::Query(messages)
            }
            cc_commands::CommandResult::Clear => command_runtime::CommandResult::Clear,
            cc_commands::CommandResult::Exit(text) => command_runtime::CommandResult::Exit(text),
            cc_commands::CommandResult::None => command_runtime::CommandResult::None,
        })
    }
}

pub fn install_engine_command_executor() {
    command_runtime::set_global_command_executor(std::sync::Arc::new(EngineCommandExecutor));
}

#[cfg(test)]
mod tests;
