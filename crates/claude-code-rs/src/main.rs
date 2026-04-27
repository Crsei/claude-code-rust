// ============================================================================
// Phase A+B: Process startup, fast paths, and full initialization
//
// Corresponds to: LIFECYCLE_STATE_MACHINE.md Section 2 (Phase A) and Section 3 (Phase B)
//
// Phase A: CLI arg parsing -> fast path detection -> immediate exit
// Phase B: Full initialization -> settings, permissions, tools, AppState -> REPL
// Phase I: Shutdown and cleanup (graceful_shutdown)
//
// Most of the heavy lifting lives in:
//   - `cli`           — argument shape (`Cli` struct + clap derives)
//   - `startup`       — logging, fast paths, runtime config helpers, print/json modes
// Keep main.rs focused on orchestration: fast-path routing, Phase B
// sequencing, and handing control off to the selected mode (daemon / web /
// headless / TUI / print / json).
// ============================================================================

// Process-wide bootstrap singleton layer (import DAG leaf node).
// Lives in its own crate (`cc-bootstrap`). Re-alias so existing
// `crate::bootstrap::...` paths continue to resolve.
use cc_bootstrap as bootstrap;

// Core modules
mod cli;
mod commands;
mod computer_use;
// `config` lives in its own crate (`cc-config`). Re-alias at the crate root so
// existing `crate::config::...` paths continue to resolve.
use cc_config as config;
mod engine;
// `keybindings` lives in its own crate (`cc-keybindings`). Re-alias at the
// crate root so existing `crate::keybindings::...` paths continue to resolve.
use cc_keybindings as keybindings;
// `permissions` lives in its own crate (`cc-permissions`).
use cc_permissions as permissions;
mod query;
// `sandbox` lives in its own crate (`cc-sandbox`). Re-alias at the crate
// root so existing `crate::sandbox::...` paths continue to resolve.
use cc_sandbox as sandbox;
// `session` lives in its own crate (`cc-session`).
use cc_session as session;
mod startup;
mod tools;
mod types;
mod ui;
// `utils` lives in its own crate (`cc-utils`). Re-alias at the crate root so
// existing `crate::utils::...` paths continue to resolve.
use cc_utils as utils;
mod voice;

// Context compaction pipeline — lives in its own crate (`cc-compact`).
// Re-alias at the crate root so existing `crate::compact::...` paths continue
// to resolve.
use cc_compact as compact;

// Network / API / auth. `auth` lives in its own crate (`cc-auth`); re-alias
// so existing `crate::auth::...` paths continue to resolve.
mod api;
use cc_auth as auth;

// Skills system lives in its own crate (`cc-skills`). Re-alias so existing
// `crate::skills::...` paths continue to resolve.
use cc_skills as skills;

// Plugin system
mod plugins;

// MCP (Model Context Protocol) server layer
mod mcp;

// Browser MCP: identification + prompt + permissions for browser-automation MCP servers
mod browser;

// LSP service layer
mod lsp_service;

// IDE detection + selection + MCP bridge (issue #41)
mod ide;

// Multi-agent Teams (feature-gated: CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS)
mod teams;

// Service layer
mod services;

// Web UI (Axum HTTP server)
mod web;

// Phase I: Shutdown and cleanup
mod shutdown;

// Observability lives in its own crate (`cc-observability`). Re-alias at the
// crate root so existing `crate::observability::...` paths continue to resolve.
use cc_observability as observability;

// IPC headless mode
mod ipc;

// KAIROS daemon
mod daemon;
mod dashboard;

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use tracing::{debug, error, info, warn};

use crate::cli::Cli;

fn resolve_startup_model(
    requested: Option<&str>,
    provider_default: Option<&str>,
    hardcoded_default: &str,
    available: &[String],
) -> String {
    for candidate in [requested, provider_default, Some(hardcoded_default)] {
        let Some(candidate) = candidate else { continue };
        if let Ok(model) = crate::commands::model::resolve_and_validate_model(candidate, available)
        {
            return model;
        }
    }

    if let Some(first_allowed) = available.first() {
        warn!(
            fallback = %first_allowed,
            "no requested/default model satisfied availableModels; falling back to the first allowed entry"
        );
        return crate::commands::model::resolve_model_alias(first_allowed);
    }

    crate::commands::model::resolve_model_alias(hardcoded_default)
}
use crate::config::settings;
use crate::engine::lifecycle::QueryEngine;
use crate::startup::runtime_config::{
    build_tool_permission_context, chrome_cli_override, resolve_cwd, resolve_permission_mode,
};
use crate::tools::registry;
use crate::types::app_state::{AppState, SettingsJson};
use crate::types::config::QueryEngineConfig;
use crate::ui::tui;

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    startup::load_env_files();

    // Wire `cc-auth`'s credentials path — it lives outside the root crate now
    // (P2, issue #71) so it can't call `crate::config::paths::credentials_path()`
    // directly. Register once, before any fast path might hit OAuth resolution.
    cc_auth::set_credentials_path(crate::config::paths::credentials_path());

    // Wire cc-permissions' descriptive-prompt callbacks. cc-permissions moved
    // out of the root crate in Phase 4 (issue #73); the Computer Use and
    // browser prompt strings still live here, so we register look-ups.
    cc_permissions::decision::set_cu_message_callback(|tool_name: &str| {
        let action = crate::computer_use::detection::extract_cu_action(tool_name)?;
        let risk = crate::computer_use::detection::classify_risk(action);
        let risk_tag = match risk {
            crate::computer_use::detection::CuRiskLevel::Medium => "[medium risk]",
            crate::computer_use::detection::CuRiskLevel::High => "[HIGH RISK]",
        };
        let description = match action {
            "screenshot" => "read the screen (take a screenshot)",
            "cursor_position" => "read the current cursor position",
            "left_click" => "click the left mouse button on your screen",
            "right_click" => "click the right mouse button on your screen",
            "middle_click" => "click the middle mouse button on your screen",
            "double_click" => "double-click the mouse on your screen",
            "type_text" | "type" => "type text using the keyboard",
            "key" => "press a keyboard shortcut",
            "scroll" => "scroll the mouse wheel",
            "mouse_move" => "move the mouse cursor",
            _ => {
                return Some(format!(
                    "Allow desktop control action '{}' {}?",
                    action, risk_tag
                ))
            }
        };
        Some(format!("Allow {} {}?", description, risk_tag))
    });
    cc_permissions::decision::set_browser_message_callback(|tool_name: &str| {
        if let Some(m) = crate::browser::permissions::browser_permission_message(tool_name) {
            return Some(m);
        }
        if let Some(rest) = tool_name.strip_prefix("mcp__") {
            if let Some((server, action)) = rest.split_once("__") {
                if crate::browser::detection::is_browser_server(server) {
                    let cat = crate::browser::permissions::classify_browser_action(action);
                    return Some(format!(
                        "Allow browser action '{}' via MCP server '{}' {}?",
                        action,
                        server,
                        cat.risk_tag()
                    ));
                }
            }
        }
        None
    });

    // Phase A: parse args first so fast paths can exit immediately
    let cli = Cli::parse();

    // Fast path: --version
    if cli.version {
        println!("claude-code-rs {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Fast path: --chrome-native-host
    // Launched by Chrome via the native-messaging manifest installed by the
    // Chrome subsystem (see src/browser/setup.rs). Skip ALL normal init:
    // no tracing to stderr (Chrome captures stderr as error logs), no
    // REPL, no HTTP server. Just bridge Chrome <-> local socket and exit
    // when Chrome closes stdin.
    if cli.chrome_native_host {
        return startup::fast_paths::run_chrome_native_host();
    }

    // Fast path: --claude-in-chrome-mcp
    // Spawned as a stdio MCP subprocess by the cc-rust MCP manager when
    // --chrome is active. Bridges MCP <-> native-host socket.
    if cli.claude_in_chrome_mcp {
        return startup::fast_paths::run_claude_in_chrome_mcp();
    }

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let _tracing_guard = {
        let _enter = rt.enter();
        startup::logging::init_tracing(cli.verbose)
    };

    info!("claude-code-rs v{}", env!("CARGO_PKG_VERSION"));

    // Fast path: --dump-system-prompt
    if cli.dump_system_prompt {
        return startup::fast_paths::run_dump_system_prompt(&cli);
    }

    let exit_code = rt.block_on(async {
        match run_full_init(cli).await {
            Ok(code) => code,
            Err(e) => {
                error!("Fatal error: {:#}", e);
                ExitCode::FAILURE
            }
        }
    });
    crate::services::langfuse::shutdown_langfuse();
    exit_code
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

    // B.1: Load layered settings (managed/user/project/local + env).
    let loaded_settings = settings::load_effective(std::path::Path::new(&cwd))?;
    let merged_config = loaded_settings.effective.clone();
    debug!(
        model = ?merged_config.model,
        permission_mode = ?merged_config.permission_mode,
        backend = ?merged_config.backend,
        layers = loaded_settings.loaded_paths.len(),
        "settings loaded",
    );
    if !loaded_settings.loaded_paths.is_empty() {
        for (src, path) in &loaded_settings.loaded_paths {
            debug!(source = src.as_str(), path = %path.display(), "settings layer");
        }
    }
    let backend = crate::engine::codex_exec::normalize_backend(merged_config.backend.as_deref());

    // B.2: Determine permission mode
    let permission_mode = resolve_permission_mode(
        cli.permission_mode.as_deref(),
        merged_config.permission_mode.as_deref(),
    );
    let chrome_enablement = crate::browser::session::resolve_enablement(
        chrome_cli_override(&cli),
        merged_config.claude_in_chrome_default_enabled,
    );
    let chrome_wanted = matches!(
        chrome_enablement,
        crate::browser::session::ChromeEnablement::Enabled
    );

    // B.3: Register tools
    plugins::init_plugins();
    let mut tools = registry::get_all_tools();
    info!(count = tools.len(), "tools registered");

    // B.3b: Initialize plugin system
    plugins::init_plugins();

    // B.3c: Initialize skills (bundled/user/project + plugin)
    skills::clear_skills();
    skills::init_skills(
        &crate::config::paths::skills_dir_global(),
        Some(std::path::Path::new(&cwd)),
    );
    let plugin_skills = plugins::discover_plugin_skills();
    if !plugin_skills.is_empty() {
        info!(
            count = plugin_skills.len(),
            "Skills: loading plugin-contributed skills"
        );
        for skill in plugin_skills {
            skills::register_skill(skill);
        }
    }

    // Start Chrome setup before registering the synthetic MCP bridge so the
    // manifest/shims are in place before the bridge begins serving requests.
    {
        use crate::browser::session::ChromeSession;

        let session = ChromeSession::new(chrome_enablement);
        if let Err(e) = session.start() {
            warn!(error = %e, "Chrome subsystem startup failed");
        }
        if crate::browser::state::is_enabled() {
            info!("Claude in Chrome subsystem active - use /chrome for status");
        }
    }

    // B.3d: Discover and connect MCP servers
    let _mcp_manager = {
        use crate::mcp::discovery::discover_mcp_servers;
        use crate::mcp::manager::McpManager;
        use crate::mcp::tools::mcp_tools_to_tools;

        let cwd_path = std::path::Path::new(&cwd);
        let mut server_configs = discover_mcp_servers(cwd_path).unwrap_or_default();
        let mcp_manager = Arc::new(tokio::sync::Mutex::new(McpManager::new()));

        // First-party Chrome integration: when --chrome is on (or env opts in),
        // register a synthetic `claude-in-chrome` MCP server that points back
        // at this same binary in `--claude-in-chrome-mcp` mode. The MCP
        // manager launches it as a stdio subprocess and talks to it like any
        // other MCP server; the bridge internally forwards over the native
        // host socket.
        if chrome_wanted {
            if let Ok(exe) = std::env::current_exe() {
                // De-dupe: if the user also put `claude-in-chrome` in
                // settings.json for some reason, the explicit config wins.
                let name = crate::browser::common::CLAUDE_IN_CHROME_MCP_SERVER_NAME;
                if !server_configs.iter().any(|c| c.name == name) {
                    server_configs.push(crate::mcp::McpServerConfig {
                        name: name.to_string(),
                        transport: "stdio".to_string(),
                        command: Some(exe.to_string_lossy().into_owned()),
                        args: Some(vec!["--claude-in-chrome-mcp".to_string()]),
                        url: None,
                        headers: None,
                        env: None,
                        browser_mcp: Some(true),
                        disabled: None,
                    });
                    info!(
                        "MCP: registered first-party claude-in-chrome bridge (spawns --claude-in-chrome-mcp subprocess)"
                    );
                }
            }
        }

        // Keep a copy of the configs so we can feed them to browser detection
        // alongside the registered tools; config flags (browserMcp: true) are
        // authoritative even if the server fails to list any recognized
        // browser-shaped tools.
        let configs_for_browser = server_configs.clone();

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

        // Install the browser MCP server registry exactly once after MCP tools
        // are folded into the tool list. Used by system-prompt injection,
        // permission prompts, and `/mcp list` styling.
        let mut browser_servers =
            crate::browser::detection::detect_browser_servers(&configs_for_browser, &tools);
        // Pre-register the first-party Chrome MCP server name when --chrome
        // (or equivalent) is on. The actual tools come online via #5; doing
        // this early means the system prompt, permissions, and /mcp list all
        // already know the capability is expected.
        if chrome_wanted {
            browser_servers
                .insert(crate::browser::common::CLAUDE_IN_CHROME_MCP_SERVER_NAME.to_string());
        }
        if !browser_servers.is_empty() {
            info!(
                count = browser_servers.len(),
                "Browser MCP: detected browser-shaped MCP server(s)"
            );
        }
        crate::browser::detection::install_browser_servers(browser_servers);

        mcp_manager
    };

    // B.3e: Register native Computer Use tools (if --computer-use)
    if cli.computer_use {
        let cu_tools = computer_use::setup::register_cu_tools();
        info!(
            count = cu_tools.len(),
            "Computer Use: registered native desktop control tools"
        );
        tools.extend(cu_tools);
    }

    // B.4: Create AppState
    // Resolve model: CLI arg > config > provider default > hardcoded fallback
    let is_codex_backend = crate::engine::codex_exec::is_codex_backend(&backend);
    let detected_client = crate::api::client::ApiClient::from_backend_result(Some(&backend))
        .context("invalid API provider configuration")?;
    let provider_default_model = detected_client
        .as_ref()
        .map(|c| c.config().default_model.clone());

    if detected_client.is_none() {
        if is_codex_backend {
            warn!("No OpenAI Codex auth detected. Set OPENAI_CODEX_AUTH_TOKEN.");
            eprintln!(
                "\x1b[33m- No OpenAI Codex auth detected.\x1b[0m\n  \
                 Set:\n  \
                 - OPENAI_CODEX_AUTH_TOKEN (required)\n  \
                 - OPENAI_CODEX_BASE_URL (optional, default: https://chatgpt.com/backend-api)\n  \
                 - OPENAI_CODEX_MODEL (optional, default: gpt-5.4)"
            );
        } else {
            warn!("No API provider detected. Set an API key in .env, environment, or use /login.");
            eprintln!(
                "\x1b[33m- No API provider detected.\x1b[0m\n  \
                 Set an API key via:\n  \
                 - .env file (ANTHROPIC_API_KEY, AZURE_API_KEY, OPENAI_API_KEY, ...)\n  \
                 - Environment variable\n  \
                 - /login command in the REPL"
            );
        }
    }

    let hardcoded_default = if is_codex_backend {
        crate::engine::codex_exec::DEFAULT_CODEX_MODEL.to_string()
    } else {
        "claude-sonnet-4-20250514".to_string()
    };
    let requested_model = cli.model.clone().or(merged_config.model.clone());
    let model = resolve_startup_model(
        requested_model.as_deref(),
        provider_default_model.as_deref(),
        &hardcoded_default,
        &merged_config.available_models,
    );

    // Mark CLI overrides (model / verbose) in the source map so /config show
    // reports them correctly.
    let mut sources = loaded_settings.sources.clone();
    if cli.model.is_some() {
        sources.insert("model".into(), settings::SettingsSource::Cli);
    }
    if cli.verbose {
        sources.insert("verbose".into(), settings::SettingsSource::Cli);
    }
    if cli.permission_mode.is_some() {
        sources.insert("permissionMode".into(), settings::SettingsSource::Cli);
    }
    // --no-network is a CLI-level override that forces network.disabled=true
    // on the sandbox section for the remainder of the session.
    let mut effective_sandbox = merged_config.sandbox.clone();
    if cli.no_network {
        effective_sandbox.network.disabled = Some(true);
        sources.insert("sandbox".into(), settings::SettingsSource::Cli);
    }

    let app_state = AppState {
        settings: SettingsJson {
            model: Some(model.clone()),
            backend: Some(backend.clone()),
            theme: merged_config.theme.clone(),
            verbose: Some(cli.verbose),
            permission_mode: merged_config.permission_mode.clone(),
            permissions: merged_config.permissions.clone(),
            sandbox: effective_sandbox,
            status_line: merged_config.status_line.clone(),
            spinner_tips: merged_config.spinner_tips.clone(),
            output_style: merged_config.output_style.clone(),
            language: merged_config.language.clone(),
            voice_enabled: merged_config.voice_enabled,
            editor_mode: merged_config.editor_mode.clone(),
            view_mode: merged_config.view_mode.clone(),
            terminal_progress_bar_enabled: merged_config.terminal_progress_bar_enabled,
            available_models: merged_config.available_models.clone(),
            effort_level: merged_config.effort_level.clone(),
            fast_mode: merged_config.fast_mode,
            fast_mode_per_session_opt_in: merged_config.fast_mode_per_session_opt_in,
            teammate_mode: merged_config.teammate_mode,
            claude_in_chrome_default_enabled: merged_config.claude_in_chrome_default_enabled,
            auto_memory_enabled: merged_config.auto_memory_enabled,
            advisor_model: merged_config.advisor_model.clone(),
            sources,
        },
        verbose: cli.verbose,
        main_loop_model: model.clone(),
        main_loop_backend: backend.clone(),
        advisor_model: merged_config.advisor_model.clone(),
        tool_permission_context: build_tool_permission_context(
            permission_mode.clone(),
            &loaded_settings,
        ),
        thinking_enabled: None,
        fast_mode: merged_config.fast_mode.unwrap_or(false),
        effort_value: merged_config.effort_level.clone(),
        team_context: None,
        hooks: merged_config.hooks.clone(),
        kairos_active: false,
        is_brief_only: false,
        is_assistant_mode: false,
        autonomous_tick_ms: None,
        terminal_focus: true,
        keybindings: crate::keybindings::KeybindingRegistry::with_user_path(Some(
            crate::config::paths::keybindings_path(),
        )),
        status_line_runner: crate::ui::status_line::StatusLineRunner::new(),
    };

    // B.5: Init-only fast path
    if cli.init_only {
        info!("init-only mode: initialization complete");
        return Ok(ExitCode::SUCCESS);
    }

    // B.6: Handle session resume (before engine creation)
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

    // B.7: Build QueryEngineConfig
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

    // B.8: Create QueryEngine
    let engine = Arc::new({
        let mut e = QueryEngine::new(engine_config);
        e.set_hook_runner(Arc::new(crate::tools::hooks::ShellHookRunner::new()));
        e.set_command_dispatcher(Arc::new(crate::commands::DefaultCommandDispatcher::new()));
        e
    });
    info!(session = %engine.session_id, "QueryEngine created");
    crate::dashboard::init_session_id(engine.session_id.as_str());

    // Apply the fully-resolved AppState (with hooks, permissions, etc.)
    engine.update_app_state(|s| *s = app_state);

    // B.8a: Fire SessionStart hook (fire-and-forget)
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

    // B.8b: Initialize audit sink
    {
        use crate::observability::{
            AuditConfig, AuditContext, AuditSink, EventKind, Outcome, SessionMeta, Stage,
        };

        let audit_config = AuditConfig::from_env();
        let source_mode = if cli.headless {
            "headless"
        } else if cli.daemon {
            "daemon"
        } else {
            "tui"
        };

        let meta = SessionMeta {
            session_id: engine.session_id.as_str().to_string(),
            started_at: chrono::Utc::now(),
            cwd: cwd.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            source: source_mode.to_string(),
        };

        let runs_dir = crate::config::paths::runs_dir(engine.session_id.as_str());
        match AuditSink::init(engine.session_id.as_str(), runs_dir, &meta, audit_config) {
            Ok(sink) => {
                let ctx = AuditContext::new(engine.session_id.as_str(), source_mode, sink);
                // Emit session.start
                ctx.emit_simple(EventKind::SessionStart, Stage::Session, Outcome::Started);
                engine.set_audit_context(ctx);
                debug!("audit sink initialized for session {}", engine.session_id);
            }
            Err(e) => {
                warn!(error = %e, "failed to initialize audit sink, continuing without audit logging");
                // Engine keeps the noop context from construction
            }
        }
    }

    // B.8.1: Initialize global ProcessState
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

    // B.9: Non-interactive output modes
    // JSON output mode takes priority (SDK sends both -p and --output-format json)
    if cli.output_format.as_deref() == Some("json") {
        let prompt = cli.prompt.join(" ");
        if prompt.is_empty() {
            // Read prompt from stdin (SDK pipes it)
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            return startup::modes::run_json_mode(&engine, buf.trim()).await;
        }
        return startup::modes::run_json_mode(&engine, &prompt).await;
    }

    // Plain text print mode (-p without --output-format json)
    if cli.print {
        let prompt = cli.prompt.join(" ");
        if prompt.is_empty() {
            error!("print mode requires a prompt argument");
            return Ok(ExitCode::FAILURE);
        }
        return startup::modes::run_print_mode(&engine, &prompt).await;
    }

    // B.10: Web UI mode
    if cli.web {
        let web_state = web::state::WebState::new(
            engine.clone(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        return match web::start_server(web_state, cli.web_port, cli.no_open).await {
            Ok(()) => Ok(ExitCode::SUCCESS),
            Err(e) => {
                error!("Web server error: {:#}", e);
                Ok(ExitCode::FAILURE)
            }
        };
    }

    // B.11: Check for inline prompt
    let initial_prompt = if !cli.prompt.is_empty() {
        Some(cli.prompt.join(" "))
    } else {
        None
    };

    // Daemon mode
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
                result.map(|()| ExitCode::SUCCESS)
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

    // B.12: Enter TUI or headless mode
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

    // Phase I: Shutdown and cleanup
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
