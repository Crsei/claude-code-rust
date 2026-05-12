//! Command registry -- slash commands for the interactive REPL.
//!
//! Commands are invoked by typing `/command_name [args]` in the user prompt.
//! Each command implements `CommandHandler` and is registered in `get_all_commands()`.

// Essential commands
pub mod context;
pub mod coordinator;
pub mod cost;
pub mod diff;
pub mod files;
pub mod help;
pub mod login;
pub mod login_code;
pub mod logout;
pub mod permissions_cmd;
pub mod resume;

// Git & workflow
pub mod branch;
pub mod commit;
pub mod gbranch;
pub mod recap;
pub mod review;
pub mod security_review;

// Model control
pub mod effort;
pub mod fast;
pub mod model_add;

// Memory & skills
pub mod skills_cmd;

// Fork-agent dependent commands (issues #37, #62)
pub mod btw;
pub mod simplify;

// Advisor model plumbing (issue #33)
pub mod advisor;

// Plan mode (issue #46)
pub mod plan;

// Scheduling / automation (issues #58, #60)
pub mod loop_cmd;
pub mod schedule;

// Team onboarding (issue #63)
pub mod team_onboarding;

// Session management
pub mod copy;
pub mod init;
pub mod insights;
pub mod rename;
pub mod rewind;
pub mod status;

// Agent Teams

// Workspace
pub mod add_dir;

// Sandbox
pub mod sandbox_cmd;

// Keybindings
pub mod keybindings_cmd;

// Read-only browsers (issues #34, #39, #40, #54)
pub mod doctor;
pub mod hooks_cmd;

// Scriptable status line (issue #11)
pub mod statusline_cmd;

// Terminal setup diagnostics (issue #12)
pub mod terminal_setup;

// Voice dictation (issue #13)
pub mod voice_cmd;

// Export
pub mod audit_export;
pub mod experimental;
pub mod export;
pub mod session_export;

// Extended info
pub mod extra_usage;
pub mod rate_limit;

// Context management
pub mod compact;

pub mod plugin_cmd;
pub mod reload_plugins_cmd;

// IDE integration (issue #41)
pub mod ide_cmd;

// First-party Chrome integration (Claude in Chrome)
pub mod chrome_cmd;

// KAIROS / assistant commands
pub mod assistant;
pub mod brief;
pub mod channels;
pub mod daemon_cmd;
pub mod dream;
pub mod notify;
pub mod remote_cmd;
mod runtime_bridge;
pub mod sleep_cmd;

pub use cc_commands::{
    command, command_metadata, model, sort_commands_for_display, Command, CommandContext,
    CommandHandler, CommandResult,
};

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
            help::HelpHandler,
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
            diff::DiffHandler,
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
            cost::CostHandler,
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
            resume::ResumeHandler,
        ),
        command(
            "rename",
            &[],
            "Set or clear the custom title for the current session",
            rename::RenameHandler,
        ),
        command(
            "rewind",
            &[],
            "Rewind the conversation to an earlier user turn",
            rewind::RewindHandler,
        ),
        command(
            "insights",
            &[],
            "Session history analytics (cross-session statistics)",
            insights::InsightsHandler,
        ),
        command(
            "files",
            &[],
            "List files referenced in the current conversation",
            files::FilesHandler,
        ),
        command(
            "context",
            &["ctx"],
            "Show context usage information",
            context::ContextHandler,
        ),
        command(
            "coordinator",
            &["coord"],
            "Enable or inspect coordinator mode for Agent Teams",
            coordinator::CoordinatorHandler,
        ),
        command(
            "permissions",
            &["perms"],
            "View or modify tool permission settings",
            permissions_cmd::PermissionsHandler,
        ),
        command(
            "plan",
            &[],
            "Enter plan mode and show or edit the plan file",
            plan::PlanHandler,
        ),
        command(
            "login",
            &[],
            "Authenticate (API key, Anthropic OAuth, OpenAI Codex OAuth, Bedrock, Vertex)",
            login::LoginHandler,
        ),
        command(
            "login-code",
            &[],
            "Complete OAuth login with authorization code",
            login_code::LoginCodeHandler,
        ),
        command(
            "logout",
            &[],
            "Clear stored authentication credentials",
            logout::LogoutHandler,
        ),
        command(
            "commit",
            &[],
            "Create a git commit from current changes",
            commit::CommitHandler,
        ),
        command(
            "branch",
            &["br"],
            "Fork the current conversation into a new branch",
            branch::BranchHandler,
        ),
        command(
            "gbranch",
            &["gitbranch"],
            "Show or switch git branches",
            gbranch::GitBranchHandler,
        ),
        command(
            "effort",
            &[],
            "Set the thinking effort level (low/medium/high)",
            effort::EffortHandler,
        ),
        command("fast", &[], "Toggle fast mode on/off", fast::FastHandler),
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
            skills_cmd::SkillsHandler,
        ),
        command(
            "init",
            &[],
            "Initialize project config and CLAUDE.md",
            init::InitHandler,
        ),
        command(
            "copy",
            &["cp"],
            "Copy the last assistant response to clipboard",
            copy::CopyHandler,
        ),
        command("status", &[], "Show session status", status::StatusHandler),
        command(
            "export",
            &["markdown-export"],
            "Export conversation to Markdown (.md)",
            export::ExportHandler,
        ),
        command(
            "experimental",
            &["experiments", "exp"],
            "Inspect or override experimental feature gates",
            experimental::ExperimentalHandler,
        ),
        command(
            "audit-export",
            &["audit"],
            "Export session as verifiable audit record (.audit.json)",
            audit_export::AuditExportHandler,
        ),
        command(
            "session-export",
            &["sexport", "structured-export"],
            "Export session as structured JSON data package (.session.json)",
            session_export::SessionExportHandler,
        ),
        command(
            "extra-usage",
            &["eu"],
            "Show extended token usage and cost analysis",
            extra_usage::ExtraUsageHandler,
        ),
        command(
            "rate-limit-options",
            &["rlo", "rate-limit"],
            "Show rate limit information for the current model",
            rate_limit::RateLimitHandler,
        ),
        command(
            "compact",
            &[],
            "Compact conversation to reduce token usage",
            compact::CompactHandler,
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
            ide_cmd::IdeHandler,
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
            chrome_cmd::ChromeHandler,
        ),
        command(
            "plugin",
            &[],
            "Plugin management (list, status, enable, disable)",
            plugin_cmd::PluginHandler,
        ),
        command(
            "reload-plugins",
            &[],
            "Hot-refresh the plugin registry",
            reload_plugins_cmd::ReloadPluginsHandler,
        ),
        command(
            "model-add",
            &["ma"],
            "Add a model with token pricing to .env",
            model_add::ModelAddHandler,
        ),
        command(
            "brief",
            &[],
            "Toggle Brief output mode (KAIROS)",
            brief::BriefHandler,
        ),
        command(
            "sleep",
            &[],
            "Set proactive sleep duration",
            sleep_cmd::SleepCmdHandler,
        ),
        command(
            "assistant",
            &["kairos"],
            "View assistant mode status",
            assistant::AssistantHandler,
        ),
        command(
            "daemon",
            &[],
            "View/control daemon process",
            daemon_cmd::DaemonCmdHandler,
        ),
        command(
            "notify",
            &[],
            "Push notification settings",
            notify::NotifyHandler,
        ),
        command(
            "remote",
            &[],
            "Inspect and control the local remote-control gateway",
            remote_cmd::RemoteHandler,
        ),
        command(
            "channels",
            &[],
            "View connected channels",
            channels::ChannelsHandler,
        ),
        command(
            "dream",
            &["logs"],
            "Distill daily logs into memory",
            dream::DreamHandler,
        ),
        command(
            "add-dir",
            &[],
            "Add a new working directory",
            add_dir::AddDirHandler,
        ),
        command(
            "sandbox",
            &[],
            "View or toggle sandbox + network access settings",
            sandbox_cmd::SandboxHandler,
        ),
        command(
            "keybindings",
            &["keys", "shortcuts"],
            "View, edit, or reload keybindings.json",
            keybindings_cmd::KeybindingsHandler,
        ),
        command(
            "statusline",
            &["status-line"],
            "View, edit, or test the scriptable status line",
            statusline_cmd::StatusLineHandler,
        ),
        command(
            "terminal-setup",
            &["term-setup", "terminal"],
            "Diagnose terminal env + print Shift+Enter / tmux / notification tips",
            terminal_setup::TerminalSetupHandler,
        ),
        command(
            "voice",
            &["dictation"],
            "Inspect compatibility-only voice settings (runtime voice unsupported)",
            voice_cmd::VoiceHandler,
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
            review::ReviewHandler,
        ),
        command(
            "security-review",
            &["secreview"],
            "Run a focused security review of the current branch diff",
            security_review::SecurityReviewHandler,
        ),
        command(
            "recap",
            &[],
            "Summarize the current session (short | long)",
            recap::RecapHandler,
        ),
        command(
            "hooks",
            &[],
            "Read-only merged hook tree (managed + user + project + local)",
            hooks_cmd::HooksHandler,
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
            doctor::DoctorHandler,
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
            btw::BtwHandler,
        ),
        command(
            "simplify",
            &[],
            "Run a multi-agent simplification review of recent changes",
            simplify::SimplifyHandler,
        ),
        command(
            "advisor",
            &[],
            "Show, set, or clear the advisor model",
            advisor::AdvisorHandler,
        ),
        // Scheduling / automation (issues #58, #60).
        command(
            "loop",
            &[],
            "Register a recurring local task or slash command and run it once",
            loop_cmd::LoopHandler,
        ),
        command(
            "schedule",
            &["cron"],
            "Manage local cron tasks (add, list, pause, trigger, remove)",
            schedule::ScheduleHandler,
        ),
        // Team onboarding (issue #63).
        command(
            "team-onboarding",
            &["teamonboarding"],
            "Generate a teammate onboarding guide from project and team state",
            team_onboarding::TeamOnboardingHandler,
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

#[cfg(test)]
mod tests;
