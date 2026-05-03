//! Top-level agents menu.

use super::types::{AgentSource, AgentSourceFilter};
use super::utils::selection_marker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsMenuState {
    pub selected_index: usize,
    pub counts: Vec<(AgentSourceFilter, usize)>,
}

impl AgentsMenuState {
    pub fn default_with_counts(
        total: usize,
        built_in: usize,
        plugin: usize,
        project: usize,
    ) -> Self {
        Self {
            selected_index: 0,
            counts: vec![
                (AgentSourceFilter::All, total),
                (AgentSourceFilter::BuiltIn, built_in),
                (AgentSourceFilter::Plugin, plugin),
                (AgentSourceFilter::Source(AgentSource::Project), project),
            ],
        }
    }

    pub fn render(&self) -> String {
        let mut lines = vec!["Agents".to_string()];
        for (idx, (filter, count)) in self.counts.iter().enumerate() {
            lines.push(format!(
                "{} {:<18} {}",
                selection_marker(idx == self.selected_index),
                label_for_filter(*filter),
                count
            ));
        }
        lines.join("\n")
    }
}

fn label_for_filter(filter: AgentSourceFilter) -> &'static str {
    match filter {
        AgentSourceFilter::All => "All agents",
        AgentSourceFilter::BuiltIn => "Built-in",
        AgentSourceFilter::Plugin => "Plugin",
        AgentSourceFilter::Source(AgentSource::Project) => "Project",
        AgentSourceFilter::Source(AgentSource::User) => "User",
        AgentSourceFilter::Source(AgentSource::Local) => "Local",
        AgentSourceFilter::Source(AgentSource::Policy) => "Policy",
        AgentSourceFilter::Source(AgentSource::Flag) => "CLI argument",
        AgentSourceFilter::Source(AgentSource::BuiltIn) => "Built-in",
        AgentSourceFilter::Source(AgentSource::Plugin) => "Plugin",
    }
}
