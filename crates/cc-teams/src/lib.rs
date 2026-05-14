//! cc-teams — team coordination (Phase 7 scaffold).
//!
//! Issue #76 (`[workspace-split] Phase 7`): target destination for
//! `crates/claude-code-rs/src/teams/` plus the two teammate-specific tool
//! wrappers (`tools/send_message.rs`, `tools/team_spawn.rs`) that get pulled
//! in with the team runtime to avoid a `cc-tools -> cc-teams` edge.

pub mod tool_specs;

pub mod identity {
    use cc_types::teams::TeamContext;

    pub const TEAM_LEAD_NAME: &str = "team-lead";

    pub fn format_agent_id(agent_name: &str, team_name: &str) -> String {
        format!("{}@{}", agent_name, team_name)
    }

    pub fn lead_agent_id(team_name: &str) -> String {
        format_agent_id(TEAM_LEAD_NAME, team_name)
    }

    pub fn is_team_lead(team_ctx: Option<&TeamContext>) -> bool {
        if let Some(ctx) = team_ctx {
            if let Some(ref self_id) = ctx.self_agent_id {
                return *self_id == ctx.lead_agent_id;
            }
        }
        false
    }
}
