//! `/coordinator` command -- activate coordinator mode and bind a team.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::teams::{constants, coordinator, helpers, identity};

pub struct CoordinatorHandler;

#[async_trait]
impl CommandHandler for CoordinatorHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut parts = args.trim().splitn(2, char::is_whitespace);
        let subcommand = parts.next().unwrap_or("").trim();
        let rest = parts.next().unwrap_or("").trim();

        let output = match subcommand {
            "" | "status" => status(ctx),
            "start" | "on" => start(ctx, rest),
            "stop" | "off" => stop(ctx),
            "help" | "--help" | "-h" => usage().to_string(),
            other => format!("Unknown /coordinator subcommand: '{}'\n{}", other, usage()),
        };
        Ok(CommandResult::Output(output))
    }
}

fn usage() -> &'static str {
    "Usage: /coordinator [status|start|stop]\n\
     \n\
       /coordinator                  Show coordinator status\n\
       /coordinator start [team]     Enable coordinator mode and bind/create a team\n\
       /coordinator stop             Disable coordinator prompt/tool policy for this session"
}

fn status(ctx: &CommandContext) -> String {
    let mut lines = vec![format!(
        "Coordinator mode: {}",
        if coordinator::is_coordinator_mode_enabled() {
            "ON"
        } else {
            "off"
        }
    )];

    if let Some(team) = ctx.app_state.team_context.as_ref() {
        lines.push(format!("Active team: {}", team.team_name));
        lines.push(format!("Lead: {}", team.lead_agent_id));
        lines.push(format!("Visible teammates: {}", team.teammates.len()));
        let snapshots = crate::teams::in_process::InProcessBackend::task_snapshots();
        let running = snapshots
            .iter()
            .filter(|snapshot| snapshot.status == crate::teams::types::TaskStatus::Running)
            .count();
        let idle = snapshots
            .iter()
            .filter(|snapshot| {
                snapshot.status == crate::teams::types::TaskStatus::Running && snapshot.is_idle
            })
            .count();
        lines.push(format!(
            "Team task status: {} running ({} idle), {} total",
            running,
            idle,
            snapshots.len()
        ));
    } else {
        lines.push("Active team: none".to_string());
    }

    let tools = if coordinator::is_coordinator_mode_enabled() {
        crate::tools::registry::get_tools_for_policy(
            crate::tools::registry::ToolPolicy::Coordinator,
        )
    } else {
        crate::tools::registry::get_tools_for_policy(
            crate::tools::registry::ToolPolicy::DefaultAgent,
        )
    };
    let mut tool_names: Vec<String> = tools.iter().map(|tool| tool.name().to_string()).collect();
    tool_names.sort();
    lines.push(format!("Tool policy: {}", tool_names.join(", ")));
    lines.join("\n")
}

fn start(ctx: &mut CommandContext, rest: &str) -> String {
    coordinator::set_coordinator_mode_enabled(true);

    let team_name = rest
        .split_whitespace()
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("coordinator");

    if ctx
        .app_state
        .team_context
        .as_ref()
        .map(|team| team.team_name == team_name)
        .unwrap_or(false)
    {
        return format!(
            "Coordinator mode enabled for existing active team '{}'.",
            team_name
        );
    }

    let cwd = ctx.cwd.to_string_lossy().into_owned();
    let team_file = match helpers::create_team(
        team_name,
        Some("Coordinator mode team".to_string()),
        Some(ctx.session_id.to_string()),
        &cwd,
    ) {
        Ok(team_file) => team_file,
        Err(error) => return format!("Failed to create coordinator team: {error}"),
    };

    let team_name = team_file.name;
    let lead_agent_id = identity::lead_agent_id(&team_name);
    ctx.app_state.team_context = Some(cc_types::teams::TeamContext {
        team_name: team_name.clone(),
        team_file_path: helpers::team_config_path(&team_name)
            .to_string_lossy()
            .into_owned(),
        lead_agent_id: lead_agent_id.clone(),
        self_agent_id: Some(lead_agent_id),
        self_agent_name: Some(constants::TEAM_LEAD_NAME.to_string()),
        is_leader: Some(true),
        self_agent_color: None,
        teammates: Default::default(),
    });

    format!(
        "Coordinator mode enabled and team '{}' is active. Spawn workers with Agent(name=..., prompt=...) or /team spawn.",
        team_name
    )
}

fn stop(_ctx: &mut CommandContext) -> String {
    coordinator::set_coordinator_mode_enabled(false);
    "Coordinator mode disabled for this session. Active team state was left intact.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::config::features::{self, FeatureFlags};
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    struct FeatureOverrideGuard;

    impl Drop for FeatureOverrideGuard {
        fn drop(&mut self) {
            features::clear_runtime_override();
        }
    }

    fn make_ctx(cwd: PathBuf) -> CommandContext {
        CommandContext {
            messages: vec![],
            cwd,
            app_state: AppState::default(),
            session_id: SessionId::from_string("coordinator-test-session"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn start_enables_coordinator_and_creates_active_team() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let _features = FeatureOverrideGuard;
        features::set_runtime_override(FeatureFlags::all_disabled());

        let mut ctx = make_ctx(cwd.path().to_path_buf());
        let result = CoordinatorHandler
            .execute("start alpha", &mut ctx)
            .await
            .unwrap();

        let CommandResult::Output(text) = result else {
            panic!("expected output");
        };
        assert!(text.contains("Coordinator mode enabled"));
        assert!(coordinator::is_coordinator_mode_enabled());
        assert_eq!(
            ctx.app_state
                .team_context
                .as_ref()
                .map(|team| team.team_name.as_str()),
            Some("alpha")
        );
        assert!(helpers::team_exists("alpha"));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn stop_disables_coordinator_without_clearing_team() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let _features = FeatureOverrideGuard;
        features::set_runtime_override(FeatureFlags::all_disabled());

        let mut ctx = make_ctx(cwd.path().to_path_buf());
        CoordinatorHandler
            .execute("start beta", &mut ctx)
            .await
            .unwrap();
        let result = CoordinatorHandler.execute("stop", &mut ctx).await.unwrap();

        let CommandResult::Output(text) = result else {
            panic!("expected output");
        };
        assert!(text.contains("disabled"));
        assert!(!coordinator::is_coordinator_mode_enabled());
        assert!(ctx.app_state.team_context.is_some());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn status_reports_coordinator_tool_policy() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let _features = FeatureOverrideGuard;
        features::set_runtime_override(FeatureFlags::all_disabled());

        let mut ctx = make_ctx(cwd.path().to_path_buf());
        CoordinatorHandler
            .execute("start gamma", &mut ctx)
            .await
            .unwrap();
        let result = CoordinatorHandler
            .execute("status", &mut ctx)
            .await
            .unwrap();

        let CommandResult::Output(text) = result else {
            panic!("expected output");
        };
        assert!(text.contains("Coordinator mode: ON"));
        assert!(text.contains("Active team: gamma"));
        assert!(text.contains("Agent"));
        assert!(text.contains("SendMessage"));
        assert!(!text.contains("Bash"));
    }
}
