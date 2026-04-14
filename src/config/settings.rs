//! Settings management.
//!
//! Loads configuration from multiple sources and merges them with a defined
//! precedence order:
//!   1. Global config (`~/.cc-rust/settings.json`)       -- lowest priority
//!   2. Project config (`.cc-rust/settings.json` in CWD) -- higher priority
//!   3. Environment variables                           -- highest priority
//!
//! The merged result drives `AppState::settings`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config types
// ---------------------------------------------------------------------------

/// Global (user-level) configuration loaded from `~/.cc-rust/settings.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    /// Preferred model identifier.
    pub model: Option<String>,
    /// Backend selection ("native" or "codex").
    pub backend: Option<String>,
    /// Color theme name.
    pub theme: Option<String>,
    /// Verbose output.
    pub verbose: Option<bool>,
    /// Permission mode override (e.g. "auto", "bypass").
    pub permission_mode: Option<String>,
    /// Tools that are always allowed (list of patterns).
    pub allowed_tools: Option<Vec<String>>,
    /// Custom system prompt to prepend.
    pub system_prompt: Option<String>,
    /// Hooks configuration (keyed by hook point).
    pub hooks: Option<HashMap<String, serde_json::Value>>,
    /// API key override (not recommended -- prefer env vars).
    pub api_key: Option<String>,
    /// Additional arbitrary settings.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Project-level configuration loaded from `.cc-rust/settings.json` in the
/// working directory (or an ancestor).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// Preferred model for this project.
    pub model: Option<String>,
    /// Backend selection ("native" or "codex").
    pub backend: Option<String>,
    /// Color theme for this project.
    pub theme: Option<String>,
    /// Verbose output.
    pub verbose: Option<bool>,
    /// Permission mode override.
    pub permission_mode: Option<String>,
    /// Tools that are always allowed.
    pub allowed_tools: Option<Vec<String>>,
    /// Custom system prompt (project-level).
    pub system_prompt: Option<String>,
    /// Hooks configuration.
    pub hooks: Option<HashMap<String, serde_json::Value>>,
    /// Additional arbitrary settings.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Merged configuration with values resolved from all sources.
#[derive(Debug, Clone, Default)]
pub struct MergedConfig {
    pub model: Option<String>,
    pub backend: Option<String>,
    pub theme: Option<String>,
    pub verbose: bool,
    pub permission_mode: Option<String>,
    #[allow(dead_code)]
    pub allowed_tools: Vec<String>,
    #[allow(dead_code)]
    pub system_prompt: Option<String>,
    pub hooks: HashMap<String, serde_json::Value>,
    pub api_key: Option<String>,
    #[allow(dead_code)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Loading functions
// ---------------------------------------------------------------------------

/// Return the path to the global cc-rust settings directory (`~/.cc-rust/`).
pub fn global_claude_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".cc-rust"))
}

/// Load the global configuration from `~/.cc-rust/settings.json`.
///
/// Returns `Ok(GlobalConfig::default())` if the file does not exist.
pub fn load_global_config() -> Result<GlobalConfig> {
    let dir = global_claude_dir()?;
    let path = dir.join("settings.json");

    if !path.exists() {
        return Ok(GlobalConfig::default());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let config: GlobalConfig = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    Ok(config)
}

/// Load the project configuration from `.cc-rust/settings.json` relative to
/// `cwd`, or any ancestor directory.
///
/// Returns `Ok(ProjectConfig::default())` if no project config is found.
pub fn load_project_config(cwd: &Path) -> Result<ProjectConfig> {
    let path = find_project_config(cwd);
    match path {
        Some(p) => {
            let contents = std::fs::read_to_string(&p)
                .with_context(|| format!("Failed to read {}", p.display()))?;
            let config: ProjectConfig = serde_json::from_str(&contents)
                .with_context(|| format!("Failed to parse {}", p.display()))?;
            Ok(config)
        }
        None => Ok(ProjectConfig::default()),
    }
}

/// Search `cwd` and its ancestors for `.cc-rust/settings.json`.
fn find_project_config(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd.to_path_buf();
    loop {
        let candidate = dir.join(".cc-rust").join("settings.json");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Merge logic
// ---------------------------------------------------------------------------

/// Merge global and project configs into a single resolved `MergedConfig`.
///
/// Project settings override global settings where present. Environment
/// variables are applied last (highest priority).
pub fn merge_configs(global: &GlobalConfig, project: &ProjectConfig) -> MergedConfig {
    let mut merged = MergedConfig {
        model: project.model.clone().or_else(|| global.model.clone()),
        backend: project.backend.clone().or_else(|| global.backend.clone()),
        theme: project.theme.clone().or_else(|| global.theme.clone()),
        verbose: project.verbose.or(global.verbose).unwrap_or(false),
        permission_mode: project
            .permission_mode
            .clone()
            .or_else(|| global.permission_mode.clone()),
        allowed_tools: merge_string_lists(
            global.allowed_tools.as_deref(),
            project.allowed_tools.as_deref(),
        ),
        system_prompt: project
            .system_prompt
            .clone()
            .or_else(|| global.system_prompt.clone()),
        hooks: merge_maps(global.hooks.as_ref(), project.hooks.as_ref()),
        api_key: global.api_key.clone(),
        extra: merge_maps(Some(&global.extra), Some(&project.extra)),
    };

    // Apply environment variable overrides.
    apply_env_overrides(&mut merged);

    merged
}

/// Merge two optional string lists by concatenation (global first, project second).
fn merge_string_lists(global: Option<&[String]>, project: Option<&[String]>) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(g) = global {
        result.extend_from_slice(g);
    }
    if let Some(p) = project {
        for item in p {
            if !result.contains(item) {
                result.push(item.clone());
            }
        }
    }
    result
}

/// Merge two optional maps. Project values override global values for the
/// same key.
fn merge_maps(
    global: Option<&HashMap<String, serde_json::Value>>,
    project: Option<&HashMap<String, serde_json::Value>>,
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    if let Some(g) = global {
        result.extend(g.clone());
    }
    if let Some(p) = project {
        result.extend(p.clone());
    }
    result
}

/// Load all config sources and merge them in one call.
///
/// Convenience function for the initialization path.
pub fn load_and_merge(cwd: &str) -> Result<MergedConfig> {
    let global = load_global_config()?;
    let project = load_project_config(Path::new(cwd))?;
    Ok(merge_configs(&global, &project))
}

/// Apply environment variable overrides to merged config.
fn apply_env_overrides(merged: &mut MergedConfig) {
    if let Ok(model) = std::env::var("CLAUDE_MODEL") {
        merged.model = Some(model);
    }
    if let Ok(backend) = std::env::var("CC_BACKEND").or_else(|_| std::env::var("CLAUDE_BACKEND")) {
        merged.backend = Some(backend);
    }
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        merged.api_key = Some(key);
    }
    if let Ok(v) = std::env::var("CLAUDE_VERBOSE") {
        merged.verbose = v == "1" || v.eq_ignore_ascii_case("true");
    }
    if let Ok(mode) = std::env::var("CLAUDE_PERMISSION_MODE") {
        merged.permission_mode = Some(mode);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_project_overrides_global() {
        let global = GlobalConfig {
            model: Some("claude-sonnet".into()),
            backend: Some("native".into()),
            theme: Some("dark".into()),
            verbose: Some(false),
            ..Default::default()
        };
        let project = ProjectConfig {
            model: Some("claude-opus".into()),
            backend: Some("codex".into()),
            verbose: Some(true),
            ..Default::default()
        };

        let merged = merge_configs(&global, &project);
        assert_eq!(merged.model.as_deref(), Some("claude-opus"));
        assert_eq!(merged.backend.as_deref(), Some("codex"));
        assert_eq!(merged.theme.as_deref(), Some("dark")); // falls through to global
        assert!(merged.verbose);
    }

    #[test]
    fn test_merge_allowed_tools_deduplicates() {
        let tools = merge_string_lists(
            Some(&["Bash".into(), "FileRead".into()]),
            Some(&["FileRead".into(), "Grep".into()]),
        );
        assert_eq!(tools, vec!["Bash", "FileRead", "Grep"]);
    }

    #[test]
    fn test_empty_configs_produce_defaults() {
        let merged = merge_configs(&GlobalConfig::default(), &ProjectConfig::default());
        assert!(merged.model.is_none());
        assert!(merged.backend.is_none());
        assert!(!merged.verbose);
        assert!(merged.allowed_tools.is_empty());
    }
}
