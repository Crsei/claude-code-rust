// ============================================================================
// Phase A+B: Process startup, fast paths, and full initialization
//
// Corresponds to: LIFECYCLE_STATE_MACHINE.md §2 (Phase A) and §3 (Phase B)
//
// Phase A: CLI arg parsing → fast path detection → immediate exit
// Phase B: Full initialization → settings, permissions, tools, AppState → REPL
// Phase I: Shutdown and cleanup (graceful_shutdown)
// ============================================================================

// 核心模块
mod types;
mod query;
mod engine;
mod tools;
mod permissions;
mod config;
mod utils;
mod session;
mod commands;
mod ui;

// 网络 / API / 认证
mod api;
mod auth;

// 技能系统
mod skills;

// 服务层
mod services;

// Phase I: Shutdown and cleanup
mod shutdown;

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
#[command(name = "claude", version, about = "Claude Code CLI", disable_version_flag = true)]
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
    let log_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .with_target(false)
        .init();

    info!("claude-code-rs v{}", env!("CARGO_PKG_VERSION"));

    // ── Fast path: --dump-system-prompt ─────────────────────────────────
    if cli.dump_system_prompt {
        let tools = registry::get_all_tools();
        let provider_default = crate::api::client::ApiClient::from_env()
            .map(|c| c.config().default_model.clone());
        let model_owned = cli.model.clone()
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
    debug!(?merged_config, "settings loaded");

    // ── B.2: Determine permission mode ───────────────────────────────
    let permission_mode = resolve_permission_mode(
        cli.permission_mode.as_deref(),
        merged_config.permission_mode.as_deref(),
    );

    // ── B.3: Register tools ──────────────────────────────────────────
    let tools = registry::get_all_tools();
    info!(count = tools.len(), "tools registered");

    // ── B.4: Create AppState ─────────────────────────────────────────
    // Resolve model: CLI arg > config > provider default > hardcoded fallback
    let detected_client = crate::api::client::ApiClient::from_env();
    let provider_default_model = detected_client
        .as_ref()
        .map(|c| c.config().default_model.clone());

    if detected_client.is_none() {
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
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    #[allow(unused_variables)]
    let app_state = AppState {
        settings: SettingsJson {
            model: Some(model.clone()),
            theme: merged_config.theme.clone(),
            verbose: Some(cli.verbose),
        },
        verbose: cli.verbose,
        main_loop_model: model.clone(),
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
    };

    // ── B.5: Init-only fast path ─────────────────────────────────────
    if cli.init_only {
        info!("init-only mode: initialization complete");
        return Ok(ExitCode::SUCCESS);
    }

    // ── B.6: Build QueryEngineConfig ─────────────────────────────────
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
        initial_messages: None,
        commands: commands::get_all_commands()
            .iter()
            .map(|c| c.name.clone())
            .collect(),
        thinking_config: None,
        json_schema: None,
        replay_user_messages: false,
        persist_session: true,
        resolved_model: Some(model.clone()),
    };

    // ── B.7: Create QueryEngine ──────────────────────────────────────
    let engine = Arc::new(QueryEngine::new(engine_config));
    info!(session = %engine.session_id, "QueryEngine created");

    // ── B.8: Handle session resume ───────────────────────────────────
    if cli.resume {
        match session::resume::get_last_session(std::path::Path::new(&cwd)) {
            Ok(Some(info)) => {
                info!(session = %info.session_id, "resuming last session");
                // Session resume would restore messages into the engine
            }
            Ok(None) => {
                warn!("no session to resume");
            }
            Err(e) => {
                warn!(error = %e, "failed to find session to resume");
            }
        }
    }
    if let Some(ref session_id) = cli.continue_session {
        info!(session = %session_id, "continuing session");
    }

    // ── B.9: Print mode (non-interactive) ────────────────────────────
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

    // ── Enter TUI ───────────────────────────────────────────────────
    // Register shutdown handler
    let shutdown_token = shutdown::register_shutdown_handler();

    let tui_result = tui::run_tui(
        engine.clone(),
        initial_prompt,
        &model,
        shutdown_token,
    )
    .await;

    // ── Phase I: Shutdown and cleanup ────────────────────────────────
    shutdown::graceful_shutdown(&engine).await;

    match tui_result {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(e) => {
            error!("TUI error: {:#}", e);
            Ok(ExitCode::FAILURE)
        }
    }
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
    cli.cwd
        .clone()
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        })
}

/// Resolve the permission mode from CLI arg or config.
fn resolve_permission_mode(
    cli_mode: Option<&str>,
    config_mode: Option<&str>,
) -> PermissionMode {
    let mode_str = cli_mode.or(config_mode).unwrap_or("default");
    match mode_str.to_lowercase().as_str() {
        "auto" => PermissionMode::Auto,
        "bypass" => PermissionMode::Bypass,
        "plan" => PermissionMode::Plan,
        _ => PermissionMode::Default,
    }
}

