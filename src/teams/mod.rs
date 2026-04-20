//! Agent Teams / Multi-Agent Swarm system.
//!
//! Corresponds to TypeScript: `utils/swarm/`, `coordinator/`, and related tools.
//!
//! Provides multi-agent coordination where a Team Lead creates and manages
//! multiple Teammate agents running in parallel. Communication happens via
//! file-based mailbox IPC (`{data_root}/teams/{name}/inboxes/`).
//!
//! # Scope in rust-lite (closure policy)
//!
//! This module is **feature-gated** by the `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`
//! env var (see [`is_agent_teams_enabled`]) and **does not appear on the default
//! tool path** — [`crate::tools::send_message::SendMessageTool::is_enabled`]
//! consults the same gate, so `base_tools()` filters the tool out unless the
//! user opts in.
//!
//! The rust-lite closure intentionally keeps:
//!
//! - `in_process` backend + `mailbox` + `protocol` + `runner` + `SendMessage`
//!   tool — the minimal "same-process multi-agent mailbox" loop.
//!
//! The rust-lite closure intentionally **does not** implement:
//!
//! - tmux / iTerm2 terminal backends — the [`backend::PaneBackend`] trait
//!   is kept only as the interface placeholder for the full edition.
//! - A Team Dashboard UI component.
//! - `/team` slash commands.
//!
//! Users who need terminal-split collaboration should use the full-edition
//! claude-code; this file is the canonical record of that decision so the
//! module's status is not re-opened as an ambiguous half-implementation.
//! See `docs/IMPLEMENTATION_GAPS.md` §1.1 for the matching doc entry.

#![allow(unused)]

pub mod backend;
pub mod constants;
pub mod context;
pub mod helpers;
pub mod identity;
pub mod in_process;
pub mod mailbox;
pub mod protocol;
pub mod runner;
pub mod types;

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
