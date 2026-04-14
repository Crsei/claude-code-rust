// ============================================================================
// Phase A+B: Process startup, fast paths, and full initialization
//
// Corresponds to: LIFECYCLE_STATE_MACHINE.md §2 (Phase A) and §3 (Phase B)
//
// Phase A: CLI arg parsing → fast path detection → immediate exit
// Phase B: Full initialization → settings, permissions, tools, AppState → REPL
// Phase I: Shutdown and cleanup (graceful_shutdown)
// ============================================================================

// 进程级全局单例层 (import DAG 叶节点)
mod bootstrap;

// 核心模块
mod commands;
mod config;
mod engine;
mod permissions;
mod query;
mod session;
mod tools;
mod types;
mod ui;
mod utils;

// 上下文压缩管道
mod compact;

// 网络 / API / 认证
mod api;
mod auth;

// 技能系统
mod skills;

// 插件系统
mod plugins;

// MCP (Model Context Protocol) 服务器
mod mcp;

// LSP 协议服务层
mod lsp_service;

// 多 Agent Teams (feature-gated: CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS)
mod teams;

// 服务层
mod services;

// Phase I: Shutdown and cleanup
mod shutdown;

// IPC headless mode
mod ipc;

// KAIROS daemon
mod daemon;
mod dashboard;

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use tracing::{debug, error, info, warn};

use crate::config::settings;
use crate::engine::lifecycle::QueryEngine;
use crate::tools::registry;
use crate::types::app_state::{AppState, SettingsJson};
use crate::types::config::{QueryEngineConfig, QuerySource};
use crate::types::tool::{PermissionMode, ToolPermissionContext};
use crate::ui::tui;

// ---------------------------------------------------------------------------
// CLI argument definitions (Phase A)
// ---------------------------------------------------------------------------

/// Claude Code CLI — Rust implementation
#[derive(Parser, Debug)]
#[command(
    name = "claude",
    version,
    about = "Claude Code CLI",
    disable_version_flag = true
)]
struct Cli {
    /// Print the version and exit (fast path)
    #[arg(short = 'V', long)]
    version: bool,

    /// Print mode: output model response and exit (non-interactive)
    #[arg(short = 'p', long = "print")]
    print: bool,

    /// Resume the most recent session
    #[arg(long)]
    resume: bool,

    /// Continue a specific session by ID
    #[arg(long = "continue")]
    continue_session: Option<String>,

    /// Maximum number of turns for agentic loops
    #[arg(long)]
    max_turns: Option<usize>,

    /// Working directory override
    #[arg(short = 'C', long = "cwd")]
    cwd: Option<String>,

    /// Model override
    #[arg(short = 'm', long)]
    model: Option<String>,

    /// Custom system prompt (replaces default)
    #[arg(long = "system-prompt")]
    system_prompt: Option<String>,

    /// Append to the system prompt
    #[arg(long = "append-system-prompt")]
    append_system_prompt: Option<String>,

    /// Permission mode: default, auto, bypass
    #[arg(long = "permission-mode")]
    permission_mode: Option<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Maximum budget in USD
    #[arg(long = "max-budget")]
    max_budget: Option<f64>,

    /// Output format (text, json, stream-json)
    #[arg(long = "output-format")]
    output_format: Option<String>,

    /// Dump system prompt and exit (fast path, internal)
    #[arg(long = "dump-system-prompt", hide = true)]
    dump_system_prompt: bool,

    /// Init only: initialize and exit (fast path)
    #[arg(long = "init-only", hide = true)]
    init_only: bool,

    /// Headless mode: run without TUI, communicate via JSON on stdin/stdout
    #[arg(long, hide = true)]
    headless: bool,

    /// Run as a background daemon with HTTP server (KAIROS mode).
    #[arg(long, hide = true)]
    daemon: bool,

    /// Daemon HTTP port (default: 19836).
    #[arg(long, default_value = "19836")]
    port: u16,

    /// Inline prompt (positional argument or via stdin in print mode)
    prompt: Vec<String>,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    // Load .env in priority order (later loads do NOT override earlier ones):
    //   1. ~/.cc-rust/.env        (global user config)
    //   2. <exe-dir>/.env         (portable — next to the binary)
    //   3. <cwd>/.env             (project-local)
    if let Some(home) = dirs::home_dir() {
        let global_env = home.join(".cc-rust").join(".env");
        let _ = dotenvy::from_path(&global_env);
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_env = exe_dir.join(".env");
            let _ = dotenvy::from_path(&exe_env);
        }
    }
    let _ = dotenvy::dotenv();

    // Phase A: fast path — parse args first
    let cli = Cli::parse();

    // ── Fast path: --version ────────────────────────────────────────────
    if cli.version {
        println!("claude-code-rs {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Initialize tracing (log level based on --verbose)
    // Two layers: stderr (warn default) + file (.logs/ directory, always debug).
    let log_level = if cli.verbose { "debug" } else { "warn" };
    let stderr_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    let log_dir = std::path::Path::new(".logs");
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(log_dir);
    }
    let file_appender = tracing_appender::rolling::daily(log_dir, "cc-rust.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    tracing_subscriber::registry()
        // stderr layer — respects --verbose / RUST_LOG
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(stderr_filter),
        )
        // file layer — always debug, with timestamps + target + line numbers
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_line_number(true)
                .with_filter(tracing_subscriber::EnvFilter::new("debug")),
        )
        .init();

    info!("claude-code-rs v{}", env!("CARGO_PKG_VERSION"));

    // ── Fast path: --dump-system-prompt ─────────────────────────────────
    if cli.dump_system_prompt {
        let tools = registry::get_all_tools();
        let provider_default =
            crate::api::client::ApiClient::from_env().map(|c| c.config().default_model.clone());
        let model_owned = cli
            .model
            .clone()
            .or(provider_default)
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        let model = model_owned.as_str();
        let cwd = resolve_cwd(&cli);
        let (parts, _, _) = crate::engine::system_prompt::build_system_prompt(
            cli.system_prompt.as_deref(),
            cli.append_system_prompt.as_deref(),
            &tools,
            model,
            &cwd,
        );
        for part in &parts {
            println!("{}", part);
        }
        return ExitCode::SUCCESS;
    }

    // ── Phase B: full initialization ────────────────────────────────────
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    rt.block_on(async {
        match run_full_init(cli).await {
            Ok(code) => code,
            Err(e) => {
                error!("Fatal error: {:#}", e);
                ExitCode::FAILURE
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Phase B: Full initialization and REPL
// ---------------------------------------------------------------------------

async fn run_full_init(cli: Cli) -> anyhow::Result<ExitCode> {
    let cwd = resolve_cwd(&cli);

    // If -C / --cwd was given, switch the process working directory so that
    // all tools (Bash, Glob, Grep, etc.) operate in the target workspace.
    if cli.cwd.is_some() {
        let target = std::path::Path::new(&cwd);
        if target.is_dir() {
            std::env::set_current_dir(target)
                .with_context(|| format!("failed to set working directory to {}", cwd))?;
            info!(cwd = %cwd, "working directory changed via --cwd");
        } else {
            anyhow::bail!("--cwd path does not exist or is not a directory: {}", cwd);
        }
    }

    // ── B.1: Load settings (parallel-ready) ──────────────────────────
    let merged_config = settings::load_and_merge(&cwd)?;
    debug!(
        model = ?merged_config.model,
        permission_mode = ?merged_config.permission_mode,
        backend = ?merged_config.backend,
        "settings loaded",
    );
    let backend =
        crate::engine::codex_exec::normalize_backend(merged_config.backend.as_deref());

    // ── B.2: Determine permission mode ───────────────────────────────
    let permission_mode = resolve_permission_mode(
        cli.permission_mode.as_deref(),
        merged_config.permission_mode.as_deref(),
    );

    // ── B.3: Register tools ──────────────────────────────────────────
    let mut tools = registry::get_all_tools();
    info!(count = tools.len(), "tools registered");

    // ── B.3b: Initialize plugin system ──────────────────────────────
    plugins::init_plugins();

    // ── B.3c: Discover and connect MCP servers ──────────────────────
    let _mcp_manager = {
        use crate::mcp::discovery::discover_mcp_servers;
        use crate::mcp::manager::McpManager;
        use crate::mcp::tools::mcp_tools_to_tools;

        let cwd_path = std::path::Path::new(&cwd);
        let server_configs = discover_mcp_servers(cwd_path).unwrap_or_default();
        let mcp_manager = Arc::new(tokio::sync::Mutex::new(McpManager::new()));

        if !server_configs.is_empty() {
            info!(
                count = server_configs.len(),
                "MCP: connecting to configured servers"
            );
            let mut mgr = mcp_manager.lock().await;
            if let Err(e) = mgr.connect_all(server_configs).await {
                warn!(error = %e, "MCP: some servers failed to connect");
            }

            // Merge MCP tools with base tools
            let mcp_tool_defs = mgr.all_tools();
            if !mcp_tool_defs.is_empty() {
                let mcp_tools = mcp_tools_to_tools(mcp_tool_defs, mcp_manager.clone());
                info!(
                    count = mcp_tools.len(),
                    "MCP: discovered tools, merging with base tools"
                );
                tools.extend(mcp_tools);
            }
        }

        mcp_manager
    };

    // ── B.4: Create AppState ─────────────────────────────────────────
    // Resolve model: CLI arg > config > provider default > hardcoded fallback
    let detected_client = if crate::engine::codex_exec::is_codex_backend(&backend) {
        None
    } else {
        crate::api::client::ApiClient::from_env()
    };
    let provider_default_model = detected_client
        .as_ref()
        .map(|c| c.config().default_model.clone());

    if detected_client.is_none() && !crate::engine::codex_exec::is_codex_backend(&backend) {
        warn!("No API provider detected. Set an API key in .env, environment, or use /login.");
        eprintln!(
            "\x1b[33m⚠ No API provider detected.\x1b[0m\n  \
             Set an API key via:\n  \
             • .env file (ANTHROPIC_API_KEY, AZURE_API_KEY, OPENAI_API_KEY, ...)\n  \
             • Environment variable\n  \
             • /login command in the REPL"
        );
    }

    let model = cli
        .model
        .clone()
        .or(merged_config.model.clone())
        .or(provider_default_model)
        .unwrap_or_else(|| {
            if crate::engine::codex_exec::is_codex_backend(&backend) {
                crate::engine::codex_exec::DEFAULT_CODEX_MODEL.to_string()
            } else {
                "claude-sonnet-4-20250514".to_string()
            }
        });

    let app_state = AppState {
        settings: SettingsJson {
            model: Some(model.clone()),
            backend: Some(backend.clone()),
            theme: merged_config.theme.clone(),
            verbose: Some(cli.verbose),
        },
        verbose: cli.verbose,
        main_loop_model: model.clone(),
        main_loop_backend: backend.clone(),
        tool_permission_context: ToolPermissionContext {
            mode: permission_mode.clone(),
            additional_working_directories: HashMap::new(),
            always_allow_rules: HashMap::new(),
            always_deny_rules: HashMap::new(),
            always_ask_rules: HashMap::new(),
            is_bypass_permissions_mode_available: true,
            is_auto_mode_available: Some(true),
            pre_plan_mode: None,
        },
        thinking_enabled: None,
        fast_mode: false,
        effort_value: None,
        team_context: None,
        hooks: merged_config.hooks.clone(),
        kairos_active: false,
        is_brief_only: false,
        is_assistant_mode: false,
        autonomous_tick_ms: None,
        terminal_focus: true,
    };

    // ── B.5: Init-only fast path ─────────────────────────────────────
    if cli.init_only {
        info!("init-only mode: initialization complete");
        return Ok(ExitCode::SUCCESS);
    }

    // ── B.6: Handle session resume (before engine creation) ─────────
    let resume_messages: Option<Vec<crate::types::message::Message>> = if cli.resume {
        match session::resume::get_last_session(std::path::Path::new(&cwd)) {
            Ok(Some(info)) => {
                info!(session = %info.session_id, "resuming last session");
                match session::resume::resume_session(&info.session_id) {
                    Ok(msgs) => {
                        info!(count = msgs.len(), "loaded messages from previous session");
                        Some(msgs)
                    }
                    Err(e) => {
                        warn!(error = %e, "failed to load session messages");
                        None
                    }
                }
            }
            Ok(None) => {
                warn!("no session to resume");
                None
            }
            Err(e) => {
                warn!(error = %e, "failed to find session to resume");
                None
            }
        }
    } else if let Some(ref session_id) = cli.continue_session {
        info!(session = %session_id, "continuing session");
        match session::resume::resume_session(session_id) {
            Ok(msgs) => {
                info!(count = msgs.len(), "loaded messages for --continue");
                Some(msgs)
            }
            Err(e) => {
                warn!(error = %e, "failed to load session {}", session_id);
                None
            }
        }
    } else {
        None
    };

    // ── B.7: Build QueryEngineConfig ─────────────────────────────────
    let engine_config = QueryEngineConfig {
        cwd: cwd.clone(),
        tools: tools.clone(),
        custom_system_prompt: cli.system_prompt.clone(),
        append_system_prompt: cli.append_system_prompt.clone(),
        user_specified_model: cli.model.clone(),
        fallback_model: None,
        max_turns: cli.max_turns,
        max_budget_usd: cli.max_budget,
        task_budget: None,
        verbose: cli.verbose,
        initial_messages: resume_messages,
        commands: commands::get_all_commands()
            .iter()
            .map(|c| c.name.clone())
            .collect(),
        thinking_config: None,
        json_schema: None,
        replay_user_messages: false,
        persist_session: true,
        resolved_model: Some(model.clone()),
        auto_save_session: true,
        agent_context: None,
    };

    // ── B.8: Create QueryEngine ──────────────────────────────────────
    let engine = Arc::new(QueryEngine::new(engine_config));
    info!(session = %engine.session_id, "QueryEngine created");

    // Apply the fully-resolved AppState (with hooks, permissions, etc.)
    engine.update_app_state(|s| *s = app_state);

    // ── B.8a: Fire SessionStart hook (fire-and-forget) ──────────────
    {
        let start_configs =
            crate::tools::hooks::load_hook_configs(&merged_config.hooks, "SessionStart");
        if !start_configs.is_empty() {
            let payload = serde_json::json!({
                "session_id": engine.session_id.as_str(),
                "cwd": std::env::current_dir().unwrap_or_default().to_string_lossy(),
            });
            let _ = crate::tools::hooks::run_event_hooks("SessionStart", &payload, &start_configs)
                .await;
        }
    }

    // ── B.8.1: Initialize global ProcessState ────────────────────────
    let cwd_path = std::path::PathBuf::from(&cwd);
    let project_root =
        crate::utils::git::find_git_root(&cwd_path).unwrap_or_else(|| cwd_path.clone());
    bootstrap::init_process_state(
        cwd_path,
        project_root,
        engine.session_id.clone(),
        !cli.print,
        Some(model.clone()),
    );

    // ── B.9: Non-interactive output modes ──────────────────────────────
    // JSON output mode takes priority (SDK sends both -p and --output-format json)
    if cli.output_format.as_deref() == Some("json") {
        let prompt = cli.prompt.join(" ");
        if prompt.is_empty() {
            // Read prompt from stdin (SDK pipes it)
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            return run_json_mode(&engine, buf.trim()).await;
        }
        return run_json_mode(&engine, &prompt).await;
    }

    // Plain text print mode (-p without --output-format json)
    if cli.print {
        let prompt = cli.prompt.join(" ");
        if prompt.is_empty() {
            error!("print mode requires a prompt argument");
            return Ok(ExitCode::FAILURE);
        }
        return run_print_mode(&engine, &prompt).await;
    }

    // ── B.10: Check for inline prompt ────────────────────────────────
    let initial_prompt = if !cli.prompt.is_empty() {
        Some(cli.prompt.join(" "))
    } else {
        None
    };

    // ── Daemon mode ──
    if cli.daemon {
        use crate::config::features::{self, Feature};
        if !features::enabled(Feature::Kairos) {
            eprintln!("error: --daemon requires FEATURE_KAIROS=1");
            return Ok(ExitCode::FAILURE);
        }

        // Set KAIROS state
        engine.update_app_state(|app| {
            app.kairos_active = true;
            app.is_assistant_mode = true;
            app.autonomous_tick_ms = Some(30_000);
        });

        let mut daemon_state = daemon::state::DaemonState::new(
            engine.clone(),
            Arc::new(features::FLAGS.clone()),
            cli.port,
        );

        // Spawn team-memory-server if feature is enabled.
        let _team_memory_child = if features::enabled(Feature::TeamMemory) {
            match daemon::team_memory_proxy::spawn_team_memory_server(
                cli.port,
                std::path::Path::new(&cwd),
            )
            .await
            {
                Ok((child, tm_port, tm_secret)) => {
                    daemon_state.team_memory_port = Some(tm_port);
                    daemon_state.team_memory_secret = Some(tm_secret);
                    info!(port = tm_port, "team-memory-server started");
                    Some(child)
                }
                Err(e) => {
                    warn!(error = %e, "failed to start team-memory-server, feature disabled");
                    None
                }
            }
        } else {
            None
        };

        let http_state = daemon_state.clone();
        let tick_state = daemon_state.clone();
        let tick_enabled = features::enabled(Feature::Proactive);

        return tokio::select! {
            result = daemon::server::serve_http(http_state, cli.port) => {
                result.map(|()| ExitCode::SUCCESS).map_err(|e| e.into())
            }
            _ = daemon::tick::tick_loop(tick_state), if tick_enabled => {
                Ok(ExitCode::SUCCESS)
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("daemon shutting down");
                Ok(ExitCode::SUCCESS)
            }
        };
    }

    // ── B.10: Enter TUI or headless mode ────────────────────────────
    if cli.headless {
        return ipc::headless::run_headless(engine, model)
            .await
            .map(|()| ExitCode::SUCCESS);
    }

    // Register shutdown handler
    let shutdown_token = shutdown::register_shutdown_handler();

    let mut dashboard_companion =
        if crate::config::features::enabled(crate::config::features::Feature::SubagentDashboard) {
            match dashboard::DashboardCompanion::spawn(dashboard::DashboardConfig::default()).await
            {
                Ok(child) => Some(child),
                Err(e) => {
                    warn!(error = %e, "failed to start subagent dashboard companion");
                    None
                }
            }
        } else {
            None
        };

    let tui_result = tui::run_tui(engine.clone(), initial_prompt, &model, shutdown_token).await;

    // ── Phase I: Shutdown and cleanup ────────────────────────────────
    shutdown::graceful_shutdown(&engine).await;
    if let Some(companion) = dashboard_companion.as_mut() {
        companion.kill();
    }

    match tui_result {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(e) => {
            error!("TUI error: {:#}", e);
            Ok(ExitCode::FAILURE)
        }
    }
}

// ---------------------------------------------------------------------------
// JSON output mode (for SDK consumers -- JSONL on stdout)
// ---------------------------------------------------------------------------

async fn run_json_mode(engine: &QueryEngine, prompt: &str) -> anyhow::Result<ExitCode> {
    use futures::StreamExt;

    let stream = engine.submit_message(prompt, QuerySource::Sdk);
    let mut stream = std::pin::pin!(stream);
    let mut exit_code = ExitCode::SUCCESS;

    while let Some(msg) = stream.next().await {
        let json = serde_json::to_string(&msg).context("failed to serialize SdkMessage to JSON")?;
        println!("{}", json);

        if let crate::engine::sdk_types::SdkMessage::Result(ref result) = msg {
            if result.is_error {
                exit_code = ExitCode::FAILURE;
            }
        }
    }

    Ok(exit_code)
}

// ---------------------------------------------------------------------------
// Print mode (non-interactive)
// ---------------------------------------------------------------------------

async fn run_print_mode(engine: &QueryEngine, prompt: &str) -> anyhow::Result<ExitCode> {
    use futures::StreamExt;

    let stream = engine.submit_message(prompt, QuerySource::Sdk);
    let mut stream = std::pin::pin!(stream);

    let mut exit_code = ExitCode::SUCCESS;

    while let Some(msg) = stream.next().await {
        match &msg {
            crate::engine::sdk_types::SdkMessage::Assistant(assistant_msg) => {
                for block in &assistant_msg.message.content {
                    if let crate::types::message::ContentBlock::Text { text } = block {
                        print!("{}", text);
                    }
                }
            }
            crate::engine::sdk_types::SdkMessage::Result(result) => {
                if result.is_error {
                    exit_code = ExitCode::FAILURE;
                }
            }
            _ => {}
        }
    }

    println!(); // trailing newline
    Ok(exit_code)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the working directory from CLI args or environment.
fn resolve_cwd(cli: &Cli) -> String {
    cli.cwd.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    })
}

/// Resolve the permission mode from CLI arg or config.
fn resolve_permission_mode(cli_mode: Option<&str>, config_mode: Option<&str>) -> PermissionMode {
    let mode_str = cli_mode.or(config_mode).unwrap_or("default");
    match mode_str.to_lowercase().as_str() {
        "auto" => PermissionMode::Auto,
        "bypass" => PermissionMode::Bypass,
        "plan" => PermissionMode::Plan,
        _ => PermissionMode::Default,
    }
}
