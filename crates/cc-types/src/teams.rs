//! Team coordination data types shared across cc-rust crates.
//!
//! Only the *pure data* types that `AppState` needs live here. Runtime
//! machinery (backend, mailbox, runner, protocol, in-process task state) stays
//! in the root crate's `teams::` module until cc-teams is extracted in
//! Phase 7. Having `TeamContext` and `TeammateInfo` in cc-types lets
//! `types/app_state.rs` stop reaching into `crate::teams::types::*`, which is
//! the last blocker for moving `AppState` into cc-types.
//!
//! See issue #75 / #76 (workspace split Phase 6/7) and the "Remaining before
//! the source move" section of
//! `docs/superpowers/specs/2026-04-20-workspace-split-design.md`.

use std::collections::HashMap;

/// Runtime team context stored in `AppState::team_context` while an Agent
/// Team session is active.
///
/// Populated by `/team join|leave|...` commands and the `TeamSpawn` tool; read
/// by `send_message` / `team_spawn` tool impls and the `agents_cmd` browser to
/// resolve teammate identities and routing targets.
#[derive(Debug, Clone, Default)]
pub struct TeamContext {
    pub team_name: String,
    pub team_file_path: String,
    pub lead_agent_id: String,
    pub self_agent_id: Option<String>,
    pub self_agent_name: Option<String>,
    pub is_leader: Option<bool>,
    pub self_agent_color: Option<String>,
    pub teammates: HashMap<String, TeammateInfo>,
}

/// Runtime info about a spawned teammate, indexed by teammate name inside
/// `TeamContext::teammates`.
#[derive(Debug, Clone)]
pub struct TeammateInfo {
    pub name: String,
    pub agent_type: Option<String>,
    pub color: Option<String>,
    pub tmux_session_name: String,
    pub tmux_pane_id: String,
    pub cwd: String,
    pub worktree_path: Option<String>,
    pub spawned_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_context_default_is_empty() {
        let ctx = TeamContext::default();
        assert!(ctx.team_name.is_empty());
        assert!(ctx.team_file_path.is_empty());
        assert!(ctx.lead_agent_id.is_empty());
        assert!(ctx.self_agent_id.is_none());
        assert!(ctx.teammates.is_empty());
    }

    #[test]
    fn team_context_holds_teammates() {
        let mut ctx = TeamContext {
            team_name: "backend".into(),
            team_file_path: "/tmp/team.json".into(),
            lead_agent_id: "lead@backend".into(),
            self_agent_id: Some("self@backend".into()),
            self_agent_name: Some("self".into()),
            is_leader: Some(false),
            self_agent_color: Some("cyan".into()),
            teammates: HashMap::new(),
        };
        ctx.teammates.insert(
            "alice".into(),
            TeammateInfo {
                name: "alice".into(),
                agent_type: Some("reviewer".into()),
                color: Some("magenta".into()),
                tmux_session_name: "sess".into(),
                tmux_pane_id: "%1".into(),
                cwd: "/work".into(),
                worktree_path: None,
                spawned_at: 1713168000,
            },
        );

        let info = ctx.teammates.get("alice").expect("alice present");
        assert_eq!(info.agent_type.as_deref(), Some("reviewer"));
        assert_eq!(ctx.teammates.len(), 1);
    }
}
