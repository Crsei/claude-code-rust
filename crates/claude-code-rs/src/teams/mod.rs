//! Agent Teams / Multi-Agent Swarm system.
//!
//! Corresponds to TypeScript: `utils/swarm/`, `coordinator/`, and related tools.
//!
//! Provides multi-agent coordination where a Team Lead creates and manages
//! multiple Teammate agents running in parallel. Communication happens via
//! file-based mailbox IPC (`{data_root}/teams/{name}/inboxes/`).
//!
//! # What rust-lite implements
//!
//! - `in_process` backend + `mailbox` + `protocol` + `runner` — the
//!   "same-process multi-agent mailbox" loop. Teammates run as tokio
//!   tasks with `task_local!` identity isolation.
//! - `SendMessage` tool — routes plain-text and structured messages
//!   between teammates via the mailbox.
//! - `TeamSpawn` tool — spawns a new teammate from within a
//!   conversation, so the model can orchestrate its own sub-agents.
//! - `/team` slash command family — `create`, `list`, `status`, `spawn`,
//!   `send`, `kill`, `leave`, `delete` (see [`crate::commands::team_cmd`]).
//! - Team Dashboard (TS/Ink): `ui/src/components/TeamPanel.tsx`
//!   subscribed to `BackendMessage::TeamEvent` over IPC.
//!
//! # Enablement
//!
//! Teams activate when **either** of these holds:
//!
//! - `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` env var is set — matches the
//!   upstream TypeScript behavior.
//! - An active [`types::TeamContext`] exists on the session's `AppState`
//!   (populated by `/team create` or by the `TeamSpawn` tool). This is
//!   how conversation-triggered teams "just work" without the user
//!   needing to pre-set the env var.
//!
//! See [`is_agent_teams_enabled`] for the env-var fast path and
//! [`is_agent_teams_active`] for the runtime check that also honors
//! `AppState::team_context`.
//!
//! # Not implemented in rust-lite
//!
//! - tmux / iTerm2 terminal backends — the [`backend::PaneBackend`]
//!   trait stays as an interface placeholder for the full edition.
//!   In-process teammates are the only supported execution surface.

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

/// Check if Agent Teams is enabled via the upstream env-var switch.
///
/// True when `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` is set to a truthy value
/// ("1", "true", "yes"). This is the quick static check — call sites that
/// also want to honor a runtime-created team should use
/// [`is_agent_teams_active`] instead.
///
/// Corresponds to TS: `isAgentSwarmsEnabled()` (no GrowthBook; env var only).
///
/// Kept on the public API for compat with the upstream check and for any
/// caller that needs a non-context variant (e.g. startup-time decisions).
#[allow(dead_code)]
pub fn is_agent_teams_enabled() -> bool {
    env::var(constants::AGENT_TEAMS_ENV_VAR)
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Check if Agent Teams is active in the given app state.
///
/// Returns true when **either** the env-var opt-in is set **or** an active
/// [`types::TeamContext`] already exists. The second condition lets
/// conversation-triggered flows (`/team create`, `TeamSpawn` tool) unlock
/// team tools without the user needing to pre-export the env var.
pub fn is_agent_teams_active(app_state: &crate::types::app_state::AppState) -> bool {
    if is_agent_teams_enabled() {
        return true;
    }
    app_state
        .team_context
        .as_ref()
        .map(|tc| !tc.team_name.is_empty())
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
