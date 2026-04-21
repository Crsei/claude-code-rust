//! Skill system — discovery, loading, and execution of user-defined skills.
//!
//! Corresponds to TypeScript: src/skills/ directory
//!
//! Skills are markdown files (SKILL.md) placed in well-known directories that
//! extend the assistant's capabilities.  They can be:
//!
//! - **Bundled** — shipped with the binary, registered at startup
//! - **User-defined** — loaded from `~/.cc-rust/skills/`
//! - **Project-defined** — loaded from `.cc-rust/skills/` in project tree
//! - **Plugin-provided** — contributed by installed plugins
//!
//! Each skill has a name, description, optional frontmatter, and a markdown
//! prompt body that is expanded into the conversation when invoked.

#![allow(unused)]

pub mod bundled;
pub mod loader;

use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Source of a skill definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillSource {
    Bundled,
    User,
    Project,
    Plugin(String),
    Mcp(String),
}

/// Execution context for a skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillContext {
    /// Expand the skill prompt inline in the current conversation.
    Inline,
    /// Run the skill in a forked sub-agent with isolated context.
    Fork,
}

impl Default for SkillContext {
    fn default() -> Self {
        Self::Inline
    }
}

/// Parsed frontmatter fields from a SKILL.md file.
///
/// Corresponds to TypeScript: `parseSkillFrontmatterFields()` return type
/// in `src/skills/loadSkillsDir.ts`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    /// Display name (overrides directory name).
    pub name: Option<String>,
    /// Human-readable description.
    pub description: String,
    /// When the model should use this skill.
    pub when_to_use: Option<String>,
    /// Allowed tool names (empty = all tools allowed).
    pub allowed_tools: Vec<String>,
    /// Hint text for argument input.
    pub argument_hint: Option<String>,
    /// Named argument placeholders.
    pub argument_names: Vec<String>,
    /// Model override (e.g. "claude-sonnet-4-20250514").
    pub model: Option<String>,
    /// Whether the user can invoke this skill via `/skill-name`.
    pub user_invocable: bool,
    /// Whether the model can invoke this skill autonomously.
    pub disable_model_invocation: bool,
    /// Execution context.
    pub context: SkillContext,
    /// Agent type for forked execution.
    pub agent: Option<String>,
    /// Effort level override.
    pub effort: Option<String>,
    /// Version string.
    pub version: Option<String>,
    /// Path glob patterns — skill only visible when matching files are touched.
    pub paths: Vec<String>,
}

/// A fully loaded skill definition.
#[derive(Debug, Clone)]
pub struct SkillDefinition {
    /// Canonical skill name (e.g. "commit", "simplify").
    pub name: String,
    /// Where this skill was loaded from.
    pub source: SkillSource,
    /// Directory containing the SKILL.md (for variable substitution).
    pub base_dir: Option<PathBuf>,
    /// Parsed frontmatter.
    pub frontmatter: SkillFrontmatter,
    /// Raw markdown prompt body (after frontmatter).
    pub prompt_body: String,
}

impl SkillDefinition {
    /// Whether this skill is user-invocable (can be called via `/name`).
    pub fn is_user_invocable(&self) -> bool {
        self.frontmatter.user_invocable
    }

    /// Whether this skill is model-invocable (the model can call it).
    pub fn is_model_invocable(&self) -> bool {
        !self.frontmatter.disable_model_invocation
            && (!self.frontmatter.description.is_empty() || self.frontmatter.when_to_use.is_some())
    }

    /// Get the display name (frontmatter name or canonical name).
    pub fn display_name(&self) -> &str {
        self.frontmatter.name.as_deref().unwrap_or(&self.name)
    }

    /// Expand the prompt body with argument substitution.
    pub fn expand_prompt(&self, args: &str, session_id: Option<&str>) -> String {
        let mut body = self.prompt_body.clone();

        // Substitute ${CLAUDE_SKILL_DIR}
        if let Some(dir) = &self.base_dir {
            let dir_str = dir.to_string_lossy().replace('\\', "/");
            body = body.replace("${CLAUDE_SKILL_DIR}", &dir_str);
        }

        // Substitute ${CLAUDE_SESSION_ID}
        if let Some(sid) = session_id {
            body = body.replace("${CLAUDE_SESSION_ID}", sid);
        }

        // Substitute positional arguments
        if !args.is_empty() {
            body = body.replace("$ARGUMENTS", args);
            // Named arguments: ${ARG_NAME}
            let arg_parts: Vec<&str> = args
                .splitn(self.frontmatter.argument_names.len().max(1), ' ')
                .collect();
            for (i, name) in self.frontmatter.argument_names.iter().enumerate() {
                let val = arg_parts.get(i).copied().unwrap_or("");
                body = body.replace(&format!("${{{}}}", name), val);
            }
        }

        body
    }
}

// ---------------------------------------------------------------------------
// Global skill registry
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Subsystem event emission
// ---------------------------------------------------------------------------
//
// cc-skills used to hold a `broadcast::Sender<crate::ipc::subsystem_events::SubsystemEvent>`
// directly. Once `skills` moved into its own crate (issue #71), referencing
// the root crate's `ipc` module would have been a cycle. The host now
// registers a simple callback that receives cc-skills's own minimal event
// enum and is responsible for adapting it into `SubsystemEvent`.

/// Minimal event set emitted by the skill subsystem. The host adapts these
/// into its own subsystem-event wrapper.
#[derive(Debug, Clone)]
pub enum SkillSubsystemEvent {
    /// Skills were loaded / reloaded.
    SkillsLoaded { count: usize },
}

type EventCallback = Box<dyn Fn(SkillSubsystemEvent) + Send + Sync>;

static EVENT_CALLBACK: LazyLock<Mutex<Option<EventCallback>>> = LazyLock::new(|| Mutex::new(None));

/// Register the host's event adapter. Replaces any previous callback.
pub fn set_event_callback<F>(cb: F)
where
    F: Fn(SkillSubsystemEvent) + Send + Sync + 'static,
{
    *EVENT_CALLBACK.lock() = Some(Box::new(cb));
}

/// Emit an event through the registered callback (no-op if unset).
fn emit_event(event: SkillSubsystemEvent) {
    if let Some(cb) = EVENT_CALLBACK.lock().as_ref() {
        cb(event);
    }
}

/// Global skill registry — holds all loaded skills, keyed by name.
static REGISTRY: LazyLock<Mutex<Vec<SkillDefinition>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Register a skill in the global registry.
pub fn register_skill(skill: SkillDefinition) {
    let mut reg = REGISTRY.lock();
    // Deduplicate by name — first registration wins
    if !reg.iter().any(|s| s.name == skill.name) {
        reg.push(skill);
    }
}

/// Get all registered skills.
pub fn get_all_skills() -> Vec<SkillDefinition> {
    REGISTRY.lock().clone()
}

/// Find a skill by name.
pub fn find_skill(name: &str) -> Option<SkillDefinition> {
    REGISTRY.lock().iter().find(|s| s.name == name).cloned()
}

/// Get user-invocable skills (for slash command listing).
pub fn get_user_invocable_skills() -> Vec<SkillDefinition> {
    get_all_skills()
        .into_iter()
        .filter(|s| s.is_user_invocable())
        .collect()
}

/// Get model-invocable skills (for SkillTool prompt).
pub fn get_model_invocable_skills() -> Vec<SkillDefinition> {
    get_all_skills()
        .into_iter()
        .filter(|s| s.is_model_invocable())
        .collect()
}

/// Clear all skills (used when refreshing).
pub fn clear_skills() {
    REGISTRY.lock().clear();
}

/// Initialize the skill system — loads bundled + directory skills.
///
/// `user_skills_dir` is the path that used to be resolved internally via
/// `crate::config::paths::skills_dir_global()`. The host passes it in so
/// cc-skills stays decoupled from the root crate's path layer.
pub fn init_skills(
    user_skills_dir: &std::path::Path,
    project_dir: Option<&std::path::Path>,
) {
    // 1. Register bundled skills
    bundled::register_bundled_skills();

    // 2. Load user skills from the host-provided directory
    if user_skills_dir.is_dir() {
        let skills = loader::load_skills_from_dir(user_skills_dir, SkillSource::User);
        for skill in skills {
            register_skill(skill);
        }
    }

    // 3. Load project skills from .cc-rust/skills/
    if let Some(proj) = project_dir {
        let project_skills_dir = proj.join(".cc-rust").join("skills");
        if project_skills_dir.is_dir() {
            let skills = loader::load_skills_from_dir(&project_skills_dir, SkillSource::Project);
            for skill in skills {
                register_skill(skill);
            }
        }
    }

    // 4. Emit skills-loaded event through the host-registered callback
    let count = get_all_skills().len();
    emit_event(SkillSubsystemEvent::SkillsLoaded { count });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_skill(name: &str) -> SkillDefinition {
        SkillDefinition {
            name: name.to_string(),
            source: SkillSource::Bundled,
            base_dir: None,
            frontmatter: SkillFrontmatter {
                description: "Test skill".to_string(),
                user_invocable: true,
                ..Default::default()
            },
            prompt_body: "Do the thing.".to_string(),
        }
    }

    #[test]
    fn test_skill_display_name() {
        let mut skill = make_skill("test");
        assert_eq!(skill.display_name(), "test");

        skill.frontmatter.name = Some("Custom Name".to_string());
        assert_eq!(skill.display_name(), "Custom Name");
    }

    #[test]
    fn test_skill_is_model_invocable() {
        let skill = make_skill("test");
        assert!(skill.is_model_invocable());

        let mut skill2 = make_skill("test2");
        skill2.frontmatter.disable_model_invocation = true;
        assert!(!skill2.is_model_invocable());

        let mut skill3 = make_skill("test3");
        skill3.frontmatter.description = String::new();
        skill3.frontmatter.when_to_use = None;
        assert!(!skill3.is_model_invocable());
    }

    #[test]
    fn test_expand_prompt_arguments() {
        let mut skill = make_skill("greet");
        skill.prompt_body = "Hello $ARGUMENTS, welcome!".to_string();
        let result = skill.expand_prompt("world", None);
        assert_eq!(result, "Hello world, welcome!");
    }

    #[test]
    fn test_expand_prompt_named_args() {
        let mut skill = make_skill("greet");
        skill.frontmatter.argument_names = vec!["NAME".to_string(), "LANG".to_string()];
        skill.prompt_body = "Hi ${NAME}, you speak ${LANG}.".to_string();
        let result = skill.expand_prompt("Alice Rust", None);
        assert_eq!(result, "Hi Alice, you speak Rust.");
    }

    #[test]
    fn test_expand_prompt_session_id() {
        let mut skill = make_skill("test");
        skill.prompt_body = "Session: ${CLAUDE_SESSION_ID}".to_string();
        let result = skill.expand_prompt("", Some("abc-123"));
        assert_eq!(result, "Session: abc-123");
    }

    #[test]
    fn test_expand_prompt_skill_dir() {
        let mut skill = make_skill("test");
        skill.base_dir = Some(PathBuf::from("/home/user/.cc-rust/skills/test"));
        skill.prompt_body = "Dir: ${CLAUDE_SKILL_DIR}".to_string();
        let result = skill.expand_prompt("", None);
        assert_eq!(result, "Dir: /home/user/.cc-rust/skills/test");
    }

    #[test]
    fn test_skill_source_variants() {
        let sources = vec![
            SkillSource::Bundled,
            SkillSource::User,
            SkillSource::Project,
            SkillSource::Plugin("my-plugin".to_string()),
            SkillSource::Mcp("my-server".to_string()),
        ];
        assert_eq!(sources.len(), 5);
    }

    #[test]
    fn test_skill_context_default() {
        assert_eq!(SkillContext::default(), SkillContext::Inline);
    }
}
