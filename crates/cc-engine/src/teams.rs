pub mod builtin_agents;

pub mod coordinator {
    use crate::config::features::{self, Feature};

    pub fn coordinator_prompt_section() -> Option<String> {
        features::enabled(Feature::Coordinator).then(coordinator_system_prompt)
    }

    pub fn default_teammate_agent_type() -> &'static str {
        "worker"
    }

    fn coordinator_system_prompt() -> String {
        concat!(
            "# Coordinator Mode\n\n",
            "You are coordinating an Agent Team. Treat yourself as the team lead: ",
            "break work into small, verifiable tasks, delegate only when parallel ",
            "work materially helps, and keep ownership of final integration and verification.\n\n",
            "## Worker Coordination\n",
            "- Spawn or address workers only for bounded tasks with clear ownership.\n",
            "- Use SendMessage for direct worker updates and concise handoffs.\n",
            "- Use TaskList to inspect active work before assigning more work.\n",
            "- Use TaskStop to cancel stale, duplicate, or unsafe worker tasks.\n"
        )
        .to_string()
    }
}

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
