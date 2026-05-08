//! Remote-control gateway route wiring for the daemon HTTP server.

use gateway::auth::{invalid_token_error, missing_token_error, GatewayAuthMode};
use gateway::{
    api, BusySnapshot, GatewayAuthVerifier, GatewayBusySnapshotProvider, GatewayError,
    GatewayPolicy, GatewayRunner, GatewayStore, SessionKeyPolicy,
};

use super::gateway_bridge::GatewayDaemonBridge;
use super::process_state;
use super::protocol::{self, DaemonCommandKind, DaemonCommandStatus};
use super::supervisor::ASSISTANT_WORKER_ID;

#[derive(Debug, Clone, Copy)]
struct DaemonGatewayAuth;

impl GatewayAuthVerifier for DaemonGatewayAuth {
    fn mode(&self) -> GatewayAuthMode {
        GatewayAuthMode::LoopbackDaemonToken
    }

    fn verify(&self, candidate: Option<&str>) -> Result<(), GatewayError> {
        let Some(candidate) = candidate else {
            return Err(missing_token_error());
        };
        match process_state::verify_control_token(candidate) {
            Ok(true) => Ok(()),
            Ok(false) => Err(invalid_token_error()),
            Err(error) => Err(GatewayError::new(
                gateway::GatewayDiagnostic::new(
                    "control_token_unavailable",
                    "The daemon control token could not be verified.",
                    "Check daemon process state and token file permissions.",
                )
                .with_context(error.to_string()),
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct DaemonBusySnapshotProvider {
    policy: GatewayPolicy,
}

impl GatewayBusySnapshotProvider for DaemonBusySnapshotProvider {
    fn snapshot(&self) -> BusySnapshot {
        let commands = protocol::read_worker_commands(ASSISTANT_WORKER_ID).unwrap_or_default();
        let active_submits = commands
            .iter()
            .filter(|command| {
                command.kind == DaemonCommandKind::Submit
                    && matches!(
                        command.status,
                        DaemonCommandStatus::Pending | DaemonCommandStatus::Acked
                    )
            })
            .count();

        BusySnapshot {
            running: usize::from(active_submits > 0),
            queued: active_submits.saturating_sub(1),
            max_running: self.policy.max_running,
            max_queued: self.policy.max_queued,
        }
    }
}

pub fn gateway_routes() -> axum::Router {
    let config = gateway::GatewayConfig::default();
    let policy = GatewayPolicy::default();
    let runner = GatewayRunner::new(
        GatewayStore::new(config.persistence.clone(), SessionKeyPolicy::default()),
        policy.clone(),
    );
    let state = api::GatewayApiState::new(
        runner,
        GatewayDaemonBridge::assistant_worker(),
        DaemonGatewayAuth,
        DaemonBusySnapshotProvider {
            policy: policy.clone(),
        },
        policy,
        config,
    );
    api::router(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_snapshot_counts_active_submit_commands() {
        let tmp = tempfile::tempdir().unwrap();
        let previous = std::env::var("CC_RUST_HOME").ok();
        std::env::set_var("CC_RUST_HOME", tmp.path());
        protocol::enqueue_command(
            ASSISTANT_WORKER_ID,
            DaemonCommandKind::Submit,
            serde_json::json!({ "text": "hello" }),
            None,
        )
        .unwrap();

        let snapshot = DaemonBusySnapshotProvider {
            policy: GatewayPolicy::default(),
        }
        .snapshot();

        assert_eq!(snapshot.running, 1);
        assert_eq!(snapshot.queued, 0);
        match previous {
            Some(value) => std::env::set_var("CC_RUST_HOME", value),
            None => std::env::remove_var("CC_RUST_HOME"),
        }
    }
}
