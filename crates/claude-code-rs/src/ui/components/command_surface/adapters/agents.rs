use crate::ui::agents::types::{
    AgentDefinition, AgentMemoryScope as UiAgentMemoryScope, AgentSource, AgentSourceFilter,
};
pub(crate) fn agent_entry_to_ui(
    entry: crate::ipc::subsystem_types::AgentDefinitionEntry,
) -> AgentDefinition {
    let mut agent = AgentDefinition::new(
        entry.name,
        entry.description,
        entry.system_prompt,
        match entry.source {
            crate::ipc::subsystem_types::AgentDefinitionSource::Builtin => AgentSource::BuiltIn,
            crate::ipc::subsystem_types::AgentDefinitionSource::User => AgentSource::User,
            crate::ipc::subsystem_types::AgentDefinitionSource::Project => AgentSource::Project,
            crate::ipc::subsystem_types::AgentDefinitionSource::Plugin { .. } => {
                AgentSource::Plugin
            }
        },
    );
    if !entry.tools.is_empty() {
        agent.tools = Some(entry.tools);
    }
    agent.color = entry.color;
    agent.model = entry.model;
    agent.filename = entry.filename;
    agent.base_dir = entry.file_path;
    agent.memory = entry.memory.map(|memory| match memory {
        crate::ipc::subsystem_types::AgentMemoryScope::User => UiAgentMemoryScope::User,
        crate::ipc::subsystem_types::AgentMemoryScope::Project => UiAgentMemoryScope::Project,
        crate::ipc::subsystem_types::AgentMemoryScope::Local => UiAgentMemoryScope::Local,
    });
    agent.effort = entry.effort;
    agent.permission_mode = entry.permission_mode.map(|mode| format!("{mode:?}"));
    agent.skills = entry.skills;
    agent
}

pub(crate) fn agent_source_tabs(agents: &[AgentDefinition]) -> Vec<AgentSourceFilter> {
    let candidates = [
        AgentSourceFilter::All,
        AgentSourceFilter::BuiltIn,
        AgentSourceFilter::Plugin,
        AgentSourceFilter::Source(AgentSource::User),
        AgentSourceFilter::Source(AgentSource::Project),
        AgentSourceFilter::Source(AgentSource::Local),
        AgentSourceFilter::Source(AgentSource::Policy),
        AgentSourceFilter::Source(AgentSource::Flag),
    ];

    candidates
        .into_iter()
        .filter(|filter| {
            *filter == AgentSourceFilter::All
                || agents.iter().any(|agent| match filter {
                    AgentSourceFilter::All => true,
                    AgentSourceFilter::BuiltIn => agent.source == AgentSource::BuiltIn,
                    AgentSourceFilter::Plugin => agent.source == AgentSource::Plugin,
                    AgentSourceFilter::Source(source) => agent.source == *source,
                })
        })
        .collect()
}
