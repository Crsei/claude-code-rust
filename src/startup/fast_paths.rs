//! Fast-path handlers that short-circuit full initialization.
//!
//! These run before Phase B so we can bypass tool registration, MCP
//! discovery, settings layering, etc. when the user only wants a quick
//! one-shot (e.g. `--version`, `--dump-system-prompt`) or a
//! special-purpose bridge (`--chrome-native-host`,
//! `--claude-in-chrome-mcp`).

use std::process::ExitCode;

use crate::cli::Cli;
use crate::config::settings;
use crate::startup::runtime_config::{chrome_requested, resolve_cwd};
use crate::tools::registry;

/// Run the Chrome native-messaging host bridge. Does NOT set up tracing or
/// register tools — Chrome captures stderr as error logs, so we skip every
/// side-effect beyond bridging stdin↔socket.
pub fn run_chrome_native_host() -> ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
    rt.block_on(async {
        match crate::browser::native_host::run().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("chrome-native-host error: {e:#}");
                ExitCode::FAILURE
            }
        }
    })
}

/// Run the Claude-in-Chrome stdio MCP bridge. Spawned as an MCP subprocess
/// by the cc-rust MCP manager when `--chrome` is active.
pub fn run_claude_in_chrome_mcp() -> ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
    rt.block_on(async {
        match crate::browser::mcp_bridge::run().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("claude-in-chrome-mcp error: {e:#}");
                ExitCode::FAILURE
            }
        }
    })
}

/// Print the resolved system prompt and exit. Populates the minimum state
/// required for the prompt builder (tools, MCP/browser detection, merged
/// language/style) without running the full Phase B pipeline.
pub fn run_dump_system_prompt(cli: &Cli) -> ExitCode {
    crate::plugins::init_plugins();
    let tools = registry::get_all_tools();
    let provider_default =
        crate::api::client::ApiClient::from_env().map(|c| c.config().default_model.clone());
    let model_owned = cli
        .model
        .clone()
        .or(provider_default)
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
    let model = model_owned.as_str();
    let cwd = resolve_cwd(cli);

    // Populate the browser MCP server registry from config alone (no live
    // connection). Config-flagged servers (`"browserMcp": true`) are
    // authoritative; the heuristic half would need connected tools and
    // isn't exercised here; use `--init-only` for that path.
    let cwd_path = std::path::Path::new(&cwd);
    let server_configs = crate::mcp::discovery::discover_mcp_servers(cwd_path).unwrap_or_default();
    let mut browser_servers =
        crate::browser::detection::detect_browser_servers(&server_configs, &tools);
    // Mirror the full-init path: when Chrome subsystem is requested via
    // CLI / env, pre-register the first-party server name so the
    // `# Browser Automation` prompt fires under --dump-system-prompt too.
    let chrome_config_default = settings::load_and_merge(&cwd)
        .ok()
        .and_then(|cfg| cfg.claude_in_chrome_default_enabled);
    if chrome_requested(cli, chrome_config_default) {
        browser_servers
            .insert(crate::browser::common::CLAUDE_IN_CHROME_MCP_SERVER_NAME.to_string());
    }
    crate::browser::detection::install_browser_servers(browser_servers);

    // Best-effort: load merged settings so --dump-system-prompt reflects
    // language/output_style overrides without requiring full bootstrap.
    let dump_settings = crate::config::settings::load_effective(std::path::Path::new(&cwd))
        .ok()
        .map(|loaded| loaded.effective);
    let dump_lang = dump_settings.as_ref().and_then(|s| s.language.clone());
    let dump_style = dump_settings.as_ref().and_then(|s| s.output_style.clone());
    let (parts, _, _) = crate::engine::system_prompt::build_system_prompt(
        cli.system_prompt.as_deref(),
        cli.append_system_prompt.as_deref(),
        &tools,
        model,
        &cwd,
        dump_lang.as_deref(),
        dump_style.as_deref(),
    );
    for part in &parts {
        println!("{}", part);
    }
    ExitCode::SUCCESS
}
