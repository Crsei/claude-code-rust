//! Agent Teams / Multi-Agent Swarm system.
//!
//! Corresponds to TypeScript: `utils/swarm/`, `coordinator/`, and related tools.
//!
//! Provides multi-agent coordination where a Team Lead creates and manages
//! multiple Teammate agents running in parallel. Communication happens via
//! file-based mailbox IPC (`~/.claude/teams/{name}/inboxes/`).
//!
//! Feature-gated by `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` env var.

#![allow(unused)]

pub mod constants;
pub mod types;
pub mod identity;
pub mod context;
pub mod protocol;
pub mod mailbox;
pub mod helpers;
pub mod backend;
pub mod in_process;
pub mod runner;

use std::env;

// ---------------------------------------------------------------------------
// Feature gate
// ---------------------------------------------------------------------------

/// Check if Agent Teams is enabled.
///
/// Enabled when `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` is set to a truthy value
/// ("1", "true", "yes").
///
/// Corresponds to TS: `isAgentSwarmsEnabled()`
/// Simplified: no GrowthBook check, just env var.
pub fn is_agent_teams_enabled() -> bool {
    env::var(constants::AGENT_TEAMS_ENV_VAR)
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_gate_default_off() {
        // Without env var set, should be disabled
        // (We can't safely set env vars in tests due to parallelism,
        //  so we just verify the function doesn't panic)
        let _ = is_agent_teams_enabled();
    }
}
