// ============================================================================
// Phase A+B: Process startup, fast paths, and full initialization
//
// Corresponds to: LIFECYCLE_STATE_MACHINE.md §2 (Phase A) and §3 (Phase B)
//
// Phase A: CLI arg parsing → fast path detection → immediate exit
// Phase B: Full initialization → settings, permissions, tools, AppState → REPL
// Phase I: Shutdown and cleanup (graceful_shutdown)
// ============================================================================

// Phase 0-8: Local/offline modules
mod types;
mod query;
mod engine;
mod tools;
mod permissions;
mod config;
mod compact;
mod utils;
mod session;
mod commands;
mod ui;

// Phase 9-13: Network modules (low priority)
mod api;
mod auth;
mod mcp;
mod analytics;
mod remote;

// Phase I: Shutdown and cleanup
mod shutdown;

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use tracing::{debug, error, info, warn};

use crate::config::settings;
use crate::engine::lifecycle::QueryEngine;
use crate::tools::registry;
use crate::types::app_state::{AppState, SettingsJson};
use crate::types::config::{QueryEngineConfig, QuerySource};
use crate::types::tool::{PermissionMode, ToolPermissionContext};

// ---------------------------------------------------------------------------
// CLI argument definitions (Phase A)
// ---------------------------------------------------------------------------

/// Claude Code CLI — Rust implementation
#[derive(Parser, Debug)]
#[command(name = "claude", version, about = "Claude Code CLI")]
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
        let model = cli.model.as_deref().unwrap_or("claude-sonnet-4-20250514");
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
    let model = cli
        .model
        .clone()
        .or(merged_config.model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

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
        include_partial_messages: false,
        persist_session: true,
    };

    // ── B.7: Create QueryEngine ──────────────────────────────────────
    let engine = QueryEngine::new(engine_config);
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

    // ── Enter REPL loop ──────────────────────────────────────────────
    // Register shutdown handler
    let shutdown_token = shutdown::register_shutdown_handler();

    let result = run_repl_loop(&engine, initial_prompt, shutdown_token).await;

    // ── Phase I: Shutdown and cleanup ────────────────────────────────
    shutdown::graceful_shutdown(&engine).await;

    result
}

// ---------------------------------------------------------------------------
// REPL loop
// ---------------------------------------------------------------------------

async fn run_repl_loop(
    engine: &QueryEngine,
    initial_prompt: Option<String>,
    shutdown_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<ExitCode> {
    use futures::StreamExt;

    let mut first_prompt = initial_prompt;

    loop {
        // Check if shutdown was requested
        if shutdown_token.is_cancelled() {
            info!("shutdown requested, exiting REPL");
            break;
        }

        // Get user input
        let prompt = if let Some(p) = first_prompt.take() {
            p
        } else {
            // Read from stdin
            match read_user_input() {
                Some(input) => input,
                None => {
                    // EOF (Ctrl-D)
                    info!("EOF received, exiting");
                    break;
                }
            }
        };

        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Special REPL commands
        if trimmed == "/quit" || trimmed == "/exit" {
            break;
        }

        // Reset abort for new turn
        engine.reset_abort();

        // Submit to QueryEngine
        let stream = engine.submit_message(trimmed, QuerySource::ReplMainThread);
        let mut stream = std::pin::pin!(stream);

        while let Some(msg) = stream.next().await {
            // In a full UI, we'd render these messages.
            // For now, print assistant text output.
            match &msg {
                crate::engine::sdk_types::SdkMessage::Assistant(assistant_msg) => {
                    for block in &assistant_msg.message.content {
                        if let crate::types::message::ContentBlock::Text { text } = block {
                            println!("{}", text);
                        }
                    }
                }
                crate::engine::sdk_types::SdkMessage::Result(result) => {
                    if result.is_error {
                        eprintln!("Error: {}", result.result);
                    }
                    debug!(
                        turns = result.num_turns,
                        cost = result.total_cost_usd,
                        duration_ms = result.duration_ms,
                        "query completed"
                    );
                }
                _ => {}
            }
        }
    }

    Ok(ExitCode::SUCCESS)
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

/// Read a line of user input from stdin.
fn read_user_input() -> Option<String> {
    use std::io::Write;
    print!("\n> ");
    std::io::stdout().flush().ok();

    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(0) => None, // EOF
        Ok(_) => Some(input.trim_end().to_string()),
        Err(_) => None,
    }
}
