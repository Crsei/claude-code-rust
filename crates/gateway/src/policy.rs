use crate::{BusyPolicy, GatewayDiagnostic, GatewayError, RunId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusySnapshot {
    pub running: usize,
    pub queued: usize,
    pub max_running: usize,
    pub max_queued: usize,
}

impl Default for BusySnapshot {
    fn default() -> Self {
        Self {
            running: 0,
            queued: 0,
            max_running: 1,
            max_queued: 32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusyDecision {
    StartNow,
    Queue,
    Interrupt,
    Reject(GatewayDiagnostic),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPolicy {
    pub max_running: usize,
    pub max_queued: usize,
    pub supports_steer: bool,
}

impl Default for GatewayPolicy {
    fn default() -> Self {
        Self {
            max_running: 1,
            max_queued: 32,
            supports_steer: false,
        }
    }
}

impl GatewayPolicy {
    pub fn evaluate_busy(
        &self,
        policy: BusyPolicy,
        snapshot: BusySnapshot,
    ) -> Result<BusyDecision, GatewayError> {
        if snapshot.running < self.max_running {
            return Ok(BusyDecision::StartNow);
        }

        match policy {
            BusyPolicy::Reject => Ok(BusyDecision::Reject(GatewayDiagnostic::new(
                "busy",
                "The gateway is already running the maximum number of runs.",
                "Retry later or submit with the queue busy policy.",
            ))),
            BusyPolicy::Queue => {
                if snapshot.queued >= self.max_queued {
                    Ok(BusyDecision::Reject(GatewayDiagnostic::new(
                        "queue_full",
                        "The gateway run queue is full.",
                        "Retry after queued runs complete or increase the queue limit.",
                    )))
                } else {
                    Ok(BusyDecision::Queue)
                }
            }
            BusyPolicy::Interrupt => Ok(BusyDecision::Interrupt),
            BusyPolicy::Steer => {
                if self.supports_steer {
                    Ok(BusyDecision::StartNow)
                } else {
                    Err(GatewayError::new(GatewayDiagnostic::new(
                        "unsupported",
                        "Mid-turn steer is not supported by this gateway.",
                        "Use queue, reject, or interrupt until steer is implemented.",
                    )))
                }
            }
        }
    }

    pub fn stale_response(run_id: &RunId) -> GatewayDiagnostic {
        GatewayDiagnostic::new(
            "stale_response",
            "The approval or ask-user response no longer matches a pending run request.",
            "Reload the run events and answer the current pending request.",
        )
        .with_context(format!("run_id={}", run_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_full_returns_stable_diagnostic() {
        let policy = GatewayPolicy::default();
        let decision = policy
            .evaluate_busy(
                BusyPolicy::Queue,
                BusySnapshot {
                    running: 1,
                    queued: 32,
                    ..BusySnapshot::default()
                },
            )
            .unwrap();

        match decision {
            BusyDecision::Reject(diagnostic) => assert_eq!(diagnostic.code, "queue_full"),
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn steer_is_explicitly_unsupported() {
        let err = GatewayPolicy::default()
            .evaluate_busy(
                BusyPolicy::Steer,
                BusySnapshot {
                    running: 1,
                    ..BusySnapshot::default()
                },
            )
            .unwrap_err();

        assert_eq!(err.diagnostic().code, "unsupported");
    }
}
