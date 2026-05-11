/// Runtime tool visibility profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    /// Default interactive/session tool pool.
    DefaultAgent,
    /// Coordinator lead tool pool.
    Coordinator,
    /// Dedicated coordinator worker pool.
    CoordinatorWorker,
    /// Generic in-process teammate pool.
    InProcessTeammate,
}

/// Return the allow-list for policies that restrict visible tools.
pub fn allowed_tool_names(policy: ToolPolicy) -> Option<&'static [&'static str]> {
    match policy {
        ToolPolicy::DefaultAgent => None,
        ToolPolicy::Coordinator => Some(&[
            "Agent",
            "SendMessage",
            "TaskList",
            "TaskStop",
            "subscribe_pr_activity",
            "unsubscribe_pr_activity",
        ]),
        ToolPolicy::CoordinatorWorker => Some(&[
            "Glob",
            "Grep",
            "Read",
            "Bash",
            "Edit",
            "Write",
            "TodoWrite",
            "TaskList",
            "TaskUpdate",
            "SendMessage",
        ]),
        ToolPolicy::InProcessTeammate => Some(&[
            "Glob",
            "Grep",
            "Read",
            "Bash",
            "Edit",
            "Write",
            "TodoWrite",
            "TaskList",
            "TaskUpdate",
            "TaskOutput",
            "SendMessage",
        ]),
    }
}

pub fn tool_allowed(policy: ToolPolicy, name: &str) -> bool {
    allowed_tool_names(policy)
        .map(|allowed| allowed.iter().any(|allowed_name| *allowed_name == name))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_allows_all_tools() {
        assert!(tool_allowed(ToolPolicy::DefaultAgent, "Bash"));
        assert!(tool_allowed(ToolPolicy::DefaultAgent, "Agent"));
    }

    #[test]
    fn coordinator_policy_is_lead_only() {
        assert!(tool_allowed(ToolPolicy::Coordinator, "Agent"));
        assert!(tool_allowed(ToolPolicy::Coordinator, "TaskStop"));
        assert!(!tool_allowed(ToolPolicy::Coordinator, "Bash"));
    }
}
