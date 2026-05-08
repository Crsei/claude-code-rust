//! `/remote` command -- local gateway status and control.

#[path = "../daemon/gateway_client.rs"]
mod gateway_client;

use anyhow::Result;
use async_trait::async_trait;
use gateway::{AdapterProvider, AdapterState, AdapterStatus, GatewayDiagnostic, RunEvent, RunId};

use self::gateway_client::{
    gateway_paths, list_local_runs, load_local_run, read_local_events, redact_text,
    LocalGatewayClient, LocalGatewayDaemonStatus,
};
use super::{CommandContext, CommandHandler, CommandResult};

pub struct RemoteHandler;

#[async_trait]
impl CommandHandler for RemoteHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut parts = args.split_whitespace();
        let sub = parts.next().unwrap_or("status").to_ascii_lowercase();
        let output = match sub.as_str() {
            "" | "status" => render_status().await,
            "adapters" => render_adapters().await,
            "connect" => connect_adapter(parts.next()).await,
            "test-message" => {
                let provider = parts.next();
                let target = parts.next();
                let text = parts.collect::<Vec<_>>().join(" ");
                test_message(provider, target, text).await
            }
            "runs" => render_runs(parse_limit(parts.collect())),
            "show" => show_run(parts.next()).await,
            "events" => show_events(parts.next(), parse_limit(parts.collect())).await,
            "stop" => stop_run(parts.next()).await,
            "doctor" => render_doctor().await,
            "help" | "--help" | "-h" => usage(),
            other => format!("Unknown /remote subcommand '{}'.\n\n{}", other, usage()),
        };
        Ok(CommandResult::Output(redact_text(&output)))
    }
}

async fn render_status() -> String {
    let mut lines = vec!["Remote Gateway Status".to_string()];
    match LocalGatewayClient::daemon_status() {
        Ok(LocalGatewayDaemonStatus::Running {
            pid,
            base_url,
            health_url,
        }) => {
            lines.push(format!("Daemon: running (pid={pid})"));
            lines.push(format!("Gateway: {}", base_url));
            lines.push(format!("Health: {}", health_url));
            match LocalGatewayClient::from_running_daemon() {
                Ok(client) => match client.capabilities().await {
                    Ok(cap) => {
                        lines.push(format!("Version: {}", cap.version));
                        lines.push(format!("Auth: {}", cap.auth_mode));
                        lines.push(format!("Steer: {}", cap.supports_steer));
                        lines.push(format!(
                            "Capacity: running={} queued={}",
                            cap.max_running, cap.max_queued
                        ));
                        lines.push(format!("Endpoints: {}", cap.endpoints.len()));
                    }
                    Err(diag) => lines.push(format_diagnostic("Capabilities", &diag)),
                },
                Err(diag) => lines.push(format_diagnostic("Auth", &diag)),
            }
        }
        Ok(LocalGatewayDaemonStatus::Stale { pid }) => {
            lines.push(format!("Daemon: stale (pid={pid})"));
            lines.push("Gateway: unavailable".to_string());
            lines.push("Action: restart the daemon with `claude daemon restart`.".to_string());
        }
        Ok(LocalGatewayDaemonStatus::Stopped) => {
            lines.push("Daemon: stopped".to_string());
            lines.push("Gateway: unavailable".to_string());
            lines.push(
                "Action: start the daemon with `FEATURE_KAIROS=1 claude daemon start`.".to_string(),
            );
        }
        Err(error) => lines.push(format!("Daemon: error ({})", error)),
    }
    lines.join("\n")
}

async fn render_adapters() -> String {
    let client = match LocalGatewayClient::from_running_daemon() {
        Ok(client) => client,
        Err(diag) => return render_gateway_diagnostic("Adapters", &diag),
    };
    match client.adapters().await {
        Ok(adapters) => render_adapter_statuses(&adapters),
        Err(diag) => render_gateway_diagnostic("Adapters", &diag),
    }
}

async fn connect_adapter(provider: Option<&str>) -> String {
    let Some(provider) = parse_provider(provider) else {
        return "Usage: /remote connect <telegram|lark>".to_string();
    };
    let client = match LocalGatewayClient::from_running_daemon() {
        Ok(client) => client,
        Err(diag) => return render_gateway_diagnostic("Connect", &diag),
    };
    match client.connect_adapter(provider).await {
        Ok(status) => render_adapter_statuses(&[status]),
        Err(diag) => render_gateway_diagnostic("Connect", &diag),
    }
}

async fn test_message(provider: Option<&str>, target: Option<&str>, text: String) -> String {
    let Some(provider) = parse_provider(provider) else {
        return "Usage: /remote test-message <telegram|lark> <target> [text]".to_string();
    };
    let Some(target) = target.filter(|value| !value.trim().is_empty()) else {
        return "Usage: /remote test-message <telegram|lark> <target> [text]".to_string();
    };
    let client = match LocalGatewayClient::from_running_daemon() {
        Ok(client) => client,
        Err(diag) => return render_gateway_diagnostic("Test message", &diag),
    };
    let text = if text.trim().is_empty() {
        "cc-rust remote gateway test".to_string()
    } else {
        text
    };
    match client
        .test_adapter_message(provider, target.to_string(), text)
        .await
    {
        Ok(status) => render_adapter_statuses(&[status]),
        Err(diag) => render_gateway_diagnostic("Test message", &diag),
    }
}

fn render_runs(limit: usize) -> String {
    match list_local_runs(limit) {
        Ok(runs) if runs.is_empty() => "Remote runs: none".to_string(),
        Ok(runs) => {
            let mut lines = vec![format!("Remote runs (latest {})", runs.len())];
            for run in runs {
                lines.push(format!(
                    "{}  status={:?}  session={}  updated={}",
                    run.run_id, run.status, run.session_key, run.updated_at_ms
                ));
            }
            lines.join("\n")
        }
        Err(error) => format!("Remote runs unavailable: {}", error),
    }
}

async fn show_run(raw_run_id: Option<&str>) -> String {
    let Some(run_id) = parse_run_id(raw_run_id) else {
        return "Usage: /remote show <run_id>".to_string();
    };
    match LocalGatewayClient::from_running_daemon() {
        Ok(client) => match client.show_run(&run_id).await {
            Ok(meta) => render_run_meta(&meta),
            Err(diag) => render_gateway_diagnostic("Run", &diag),
        },
        Err(_) => match load_local_run(&run_id) {
            Ok(meta) => render_run_meta(&meta),
            Err(diag) => render_gateway_diagnostic("Run", &diag),
        },
    }
}

async fn show_events(raw_run_id: Option<&str>, limit: usize) -> String {
    let Some(run_id) = parse_run_id(raw_run_id) else {
        return "Usage: /remote events <run_id> [--limit N]".to_string();
    };
    let events = match LocalGatewayClient::from_running_daemon() {
        Ok(client) => client.run_events(&run_id).await,
        Err(_) => read_local_events(&run_id),
    };
    match events {
        Ok(events) => render_events(&events, limit),
        Err(diag) => render_gateway_diagnostic("Events", &diag),
    }
}

async fn stop_run(raw_run_id: Option<&str>) -> String {
    let Some(run_id) = parse_run_id(raw_run_id) else {
        return "Usage: /remote stop <run_id>".to_string();
    };
    let client = match LocalGatewayClient::from_running_daemon() {
        Ok(client) => client,
        Err(diag) => return render_gateway_diagnostic("Stop", &diag),
    };
    match client.stop_run(&run_id).await {
        Ok(response) => format!(
            "Stop requested for {}: status={:?} action={}{}",
            response.run_id,
            response.status,
            response.action,
            response
                .diagnostic
                .as_ref()
                .map(|diag| format!("\n{}", format_diagnostic("Diagnostic", diag)))
                .unwrap_or_default()
        ),
        Err(diag) => render_gateway_diagnostic("Stop", &diag),
    }
}

async fn render_doctor() -> String {
    let paths = gateway_paths();
    let mut lines = vec!["Remote Gateway Doctor".to_string()];
    lines.push(render_status().await);
    lines.push(format!("Gateway dir: {}", paths.gateway_dir.display()));
    lines.push(format!("Runs dir: {}", paths.runs_dir.display()));
    lines.push(format!("Adapters dir: {}", paths.adapters_dir.display()));
    lines.push(format!("Config path: {}", paths.config_path.display()));
    lines.push(format!(
        "Control token path: {}",
        paths.token_path.display()
    ));
    lines.join("\n")
}

fn render_adapter_statuses(adapters: &[AdapterStatus]) -> String {
    let mut lines = vec![format!("Remote adapters ({})", adapters.len())];
    for adapter in adapters {
        lines.push(format!(
            "{}  configured={}  state={}  message={}",
            adapter.provider.as_str(),
            adapter.configured,
            adapter_state(adapter.state),
            adapter.message
        ));
        if let Some(diag) = &adapter.diagnostic {
            lines.push(format_diagnostic("  Diagnostic", diag));
        }
    }
    lines.join("\n")
}

fn render_run_meta(meta: &gateway::RunMeta) -> String {
    let mut lines = vec![format!("Remote run {}", meta.run_id)];
    lines.push(format!("Status: {:?}", meta.status));
    lines.push(format!("Session: {}", meta.session_key));
    lines.push(format!("Created: {}", meta.created_at_ms));
    lines.push(format!("Updated: {}", meta.updated_at_ms));
    lines.push(format!("Source: {:?}", meta.request.source.transport));
    lines.push(format!("Thread: {}", meta.request.source.thread_id));
    lines.push(format!("Prompt: {}", first_line(&meta.request.prompt, 120)));
    lines.join("\n")
}

fn render_events(events: &[RunEvent], limit: usize) -> String {
    if events.is_empty() {
        return "Remote run events: none".to_string();
    }
    let start = events.len().saturating_sub(limit);
    let mut lines = vec![format!(
        "Remote run events (showing {})",
        events.len() - start
    )];
    for event in &events[start..] {
        lines.push(format!(
            "#{} {} {:?}",
            event.sequence, event.timestamp_ms, event.kind
        ));
    }
    lines.join("\n")
}

fn render_gateway_diagnostic(title: &str, diag: &GatewayDiagnostic) -> String {
    format!(
        "{} failed\n{}",
        title,
        format_diagnostic("Diagnostic", diag)
    )
}

fn format_diagnostic(label: &str, diag: &GatewayDiagnostic) -> String {
    let mut line = format!(
        "{}: code={} message={} action={}",
        label, diag.code, diag.message, diag.action
    );
    if let Some(context) = &diag.context {
        line.push_str(&format!(" context={}", context));
    }
    line
}

fn parse_provider(raw: Option<&str>) -> Option<AdapterProvider> {
    raw.and_then(|value| AdapterProvider::parse(&value.to_ascii_lowercase()).ok())
}

fn parse_run_id(raw: Option<&str>) -> Option<RunId> {
    raw.and_then(|value| RunId::from_string(value).ok())
}

fn parse_limit(args: Vec<&str>) -> usize {
    args.windows(2)
        .find(|pair| pair[0] == "--limit" || pair[0] == "-n")
        .and_then(|pair| pair[1].parse::<usize>().ok())
        .unwrap_or(20)
        .clamp(1, 100)
}

fn adapter_state(state: AdapterState) -> &'static str {
    match state {
        AdapterState::Unconfigured => "unconfigured",
        AdapterState::Connected => "connected",
        AdapterState::ConnectedOutboundOnly => "connected_outbound_only",
        AdapterState::Blocked => "blocked",
        AdapterState::Failed => "failed",
    }
}

fn first_line(value: &str, max: usize) -> String {
    let line = value.lines().next().unwrap_or(value).trim();
    if line.chars().count() <= max {
        return line.to_string();
    }
    format!("{}...", line.chars().take(max).collect::<String>())
}

fn usage() -> String {
    "Usage:\n  /remote status\n  /remote adapters\n  /remote connect <telegram|lark>\n  /remote test-message <telegram|lark> <target> [text]\n  /remote runs [--limit N]\n  /remote show <run_id>\n  /remote events <run_id> [--limit N]\n  /remote stop <run_id>\n  /remote doctor".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use gateway::{RemoteSource, RemoteTransport, RunPolicy, RunRequest};
    use std::path::PathBuf;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("remote-test"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn status_reports_stopped_without_token_leak() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let mut ctx = ctx();
        let result = RemoteHandler.execute("status", &mut ctx).await.unwrap();

        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Daemon: stopped"));
                assert!(!text.to_ascii_lowercase().contains("bearer"));
                assert!(!text.contains("raw-token"));
            }
            _ => panic!("expected output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn runs_lists_local_gateway_store() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let store =
            gateway::GatewayStore::default_with_policy(gateway::SessionKeyPolicy::default());
        let created = store.create_run(request()).unwrap().meta().run_id.clone();

        let mut ctx = ctx();
        let result = RemoteHandler
            .execute("runs --limit 5", &mut ctx)
            .await
            .unwrap();

        match result {
            CommandResult::Output(text) => {
                assert!(text.contains(created.as_str()));
                assert!(!text.contains("raw-token"));
            }
            _ => panic!("expected output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_prints_usage() {
        let mut ctx = ctx();
        let result = RemoteHandler.execute("wat", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown /remote"));
                assert!(text.contains("/remote status"));
            }
            _ => panic!("expected output"),
        }
    }

    fn request() -> RunRequest {
        RunRequest {
            prompt: "hello from /remote".to_string(),
            source: RemoteSource::new(
                RemoteTransport::Local,
                "local",
                "workspace",
                "remote-cmd-test",
                "user",
                "thread",
            )
            .with_metadata("token", "raw-token"),
            policy: RunPolicy::default(),
            idempotency_key: None,
        }
    }
}
