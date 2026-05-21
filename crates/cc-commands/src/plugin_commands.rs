//! Bridge between plugin discovery and the dynamic command registry.
//!
//! Full plugin integration (discovery, lifecycle) is owned by Lane D.
//! This module defines the contract for registering/unregistering plugin-provided
//! commands into the [`DynamicRegistry`].

use crate::dynamic_registry::{
    CommandSource, DynamicCommandEntry, DynamicRegistry, ExecutionStrategy,
};

/// A potential plugin command that could be registered.
#[derive(Debug, Clone)]
pub struct PluginCommandCandidate {
    pub name: String,
    pub description: String,
    /// Identifies which plugin owns this command.
    pub plugin_id: String,
}

/// Register plugin commands into a `DynamicRegistry`.
///
/// Called by the plugin loader after discovering plugin-provided commands.
/// Returns any entries that were replaced during registration.
pub fn register_plugin_commands(
    registry: &mut DynamicRegistry,
    candidates: Vec<PluginCommandCandidate>,
) -> Vec<DynamicCommandEntry> {
    let entries: Vec<DynamicCommandEntry> = candidates
        .into_iter()
        .map(|candidate| DynamicCommandEntry {
            name: candidate.name,
            aliases: Vec::new(),
            description: candidate.description,
            source: CommandSource::Plugin,
            hidden: false,
            usage_score: 0.0,
            execution_strategy: ExecutionStrategy::Plugin,
        })
        .collect();

    registry.register_all(entries)
}

/// Unregister all commands from a specific plugin.
///
/// # Note
///
/// Full plugin-id tracking in the registry is a future extension (Lane D).
/// Currently this is a stub that returns 0; the plugin loader should track
/// which entries it registered and call [`DynamicRegistry::unregister`] with
/// [`CommandSource::Plugin`] for each.
pub fn unregister_plugin_commands(_registry: &mut DynamicRegistry, _plugin_id: &str) -> usize {
    // TODO(Lane D): add plugin_id field to DynamicCommandEntry so we can
    // filter by plugin. For now, callers must track their own entries.
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_plugin_commands_adds_entries() {
        let mut registry = DynamicRegistry::new();

        let candidates = vec![
            PluginCommandCandidate {
                name: "my-plugin-cmd".to_string(),
                description: "A plugin command".to_string(),
                plugin_id: "my-plugin".to_string(),
            },
            PluginCommandCandidate {
                name: "another-cmd".to_string(),
                description: "Another plugin command".to_string(),
                plugin_id: "my-plugin".to_string(),
            },
        ];

        let replaced = register_plugin_commands(&mut registry, candidates);
        assert!(replaced.is_empty());
        assert_eq!(registry.len(), 2);

        let entry = registry.find("my-plugin-cmd").unwrap();
        assert_eq!(entry.source, CommandSource::Plugin);
        assert_eq!(entry.description, "A plugin command");
    }

    #[test]
    fn plugin_commands_use_plugin_strategy() {
        let candidates = vec![PluginCommandCandidate {
            name: "test".to_string(),
            description: "Test".to_string(),
            plugin_id: "test-plugin".to_string(),
        }];

        let mut registry = DynamicRegistry::new();
        register_plugin_commands(&mut registry, candidates);

        let entry = registry.find("test").unwrap();
        assert!(matches!(
            entry.execution_strategy,
            ExecutionStrategy::Plugin
        ));
    }

    #[test]
    fn plugin_is_lower_priority_than_builtin() {
        use crate::dynamic_registry::DynamicCommandEntry;

        let mut registry = DynamicRegistry::new();

        // Register a builtin command
        registry.register(DynamicCommandEntry {
            name: "conflict".to_string(),
            aliases: vec![],
            description: "Builtin".to_string(),
            source: CommandSource::Builtin,
            hidden: false,
            usage_score: 0.0,
            execution_strategy: ExecutionStrategy::Inline,
        });

        // Register a plugin command with the same name
        let candidates = vec![PluginCommandCandidate {
            name: "conflict".to_string(),
            description: "Plugin version".to_string(),
            plugin_id: "test-plugin".to_string(),
        }];

        register_plugin_commands(&mut registry, candidates);

        // Builtin should be found (higher priority)
        let entry = registry.find("conflict").unwrap();
        assert_eq!(entry.source, CommandSource::Builtin);
        assert_eq!(entry.description, "Builtin");
    }

    #[test]
    fn unregister_plugin_commands_stub() {
        let mut registry = DynamicRegistry::new();
        let count = unregister_plugin_commands(&mut registry, "any-plugin");
        assert_eq!(count, 0);
    }
}
