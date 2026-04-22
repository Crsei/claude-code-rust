//! Agent-definition settings — loader, writer, and IPC handler.
//!
//! Backs the `/agents` settings dialog. Parallels
//! [`super::subsystem_handlers`]' MCP config editor but for agent-definition
//! markdown files (`{scope}/agents/<name>.md`).
//!
//! File format: YAML frontmatter + markdown body.
//!
//! ```text
//! ---
//! name: reviewer
//! description: Reviews code against the plan
//! tools: Read, Grep
//! model: sonnet
//! color: blue
//! ---
//!
//! You are a thorough reviewer. …
//! ```
//!
//! Sources:
//!   * `Builtin`  — engine-provided (general-purpose, Explore, Plan, code-reviewer), read-only
//!   * `User`     — `~/.cc-rust/agents/*.md`, editable
//!   * `Project`  — `{cwd}/.cc-rust/agents/*.md`, editable
//!   * `Plugin`   — contributed by plugins, read-only (future hook; none registered today)

use std::fs;
use std::path::{Path, PathBuf};

use super::protocol::BackendMessage;
use super::subsystem_events::{AgentSettingsCommand, AgentSettingsEvent};
use super::subsystem_types::{AgentDefinitionEntry, AgentDefinitionSource};

/// Built-in subagent types the engine honours out of the box. Kept in sync
/// with [`crate::commands::agents_cmd::BUILTIN_SUBAGENTS`] — duplicated here
/// rather than imported to avoid a cyclic dependency with the command layer.
const BUILTIN_AGENTS: &[(&str, &str)] = &[
    (
        "general-purpose",
        "Default agent for multi-step research and coding tasks",
    ),
    (
        "Explore",
        "Fast codebase exploration — globbing, grepping, and file reads",
    ),
    (
        "Plan",
        "Software architect — produces an implementation plan without editing code",
    ),
    (
        "code-reviewer",
        "Reviews a completed change against the plan and coding standards",
    ),
];

// ---------------------------------------------------------------------------
// Public handler
// ---------------------------------------------------------------------------

/// Handle an `AgentSettingsCommand` from the frontend.
pub fn handle(cmd: AgentSettingsCommand) -> Vec<BackendMessage> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match cmd {
        AgentSettingsCommand::QueryList => {
            let entries = list_all_agents(&cwd);
            vec![BackendMessage::AgentSettingsEvent {
                event: AgentSettingsEvent::List { entries },
            }]
        }
        AgentSettingsCommand::Upsert { entry } => match upsert_agent(&cwd, entry) {
            Ok(saved) => vec![BackendMessage::AgentSettingsEvent {
                event: AgentSettingsEvent::Changed {
                    name: saved.name.clone(),
                    entry: Some(saved),
                },
            }],
            Err((name, error)) => {
                tracing::warn!(agent = %name, %error, "agent upsert rejected");
                vec![BackendMessage::AgentSettingsEvent {
                    event: AgentSettingsEvent::Error { name, error },
                }]
            }
        },
        AgentSettingsCommand::Delete { name, source } => {
            match delete_agent(&cwd, &name, &source) {
                Ok(()) => vec![BackendMessage::AgentSettingsEvent {
                    event: AgentSettingsEvent::Changed { name, entry: None },
                }],
                Err(error) => {
                    tracing::warn!(agent = %name, %error, "agent delete rejected");
                    vec![BackendMessage::AgentSettingsEvent {
                        event: AgentSettingsEvent::Error { name, error },
                    }]
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Discovery — list every definition across all sources
// ---------------------------------------------------------------------------

/// Return every agent the UI should know about: built-ins + user + project.
pub fn list_all_agents(cwd: &Path) -> Vec<AgentDefinitionEntry> {
    let mut out: Vec<AgentDefinitionEntry> = BUILTIN_AGENTS
        .iter()
        .map(|(name, desc)| AgentDefinitionEntry {
            name: (*name).to_string(),
            description: (*desc).to_string(),
            system_prompt: String::new(),
            tools: vec![],
            model: None,
            color: None,
            source: AgentDefinitionSource::Builtin,
            file_path: None,
        })
        .collect();

    let user_dir = cc_config::paths::data_root().join("agents");
    out.extend(load_agents_from_dir(&user_dir, AgentDefinitionSource::User));

    let project_dir = cwd.join(".cc-rust").join("agents");
    out.extend(load_agents_from_dir(
        &project_dir,
        AgentDefinitionSource::Project,
    ));

    out
}

fn load_agents_from_dir(dir: &Path, source: AgentDefinitionSource) -> Vec<AgentDefinitionEntry> {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(parsed) = parse_agent_file(&path, &raw, source.clone()) {
            out.push(parsed);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ---------------------------------------------------------------------------
// Frontmatter parsing / rendering
// ---------------------------------------------------------------------------

/// Parse a single agent markdown file. Returns `None` if the file is
/// malformed.
pub fn parse_agent_file(
    path: &Path,
    content: &str,
    source: AgentDefinitionSource,
) -> Option<AgentDefinitionEntry> {
    let (fm, body) = parse_frontmatter(content);
    let name = fm
        .iter()
        .find(|(k, _)| k == "name")
        .map(|(_, v)| v.clone())
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })?;
    let description = fm
        .iter()
        .find(|(k, _)| k == "description")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let tools = fm
        .iter()
        .find(|(k, _)| k == "tools")
        .map(|(_, v)| split_list(v))
        .unwrap_or_default();
    let model = fm
        .iter()
        .find(|(k, _)| k == "model")
        .map(|(_, v)| v.clone())
        .filter(|s| !s.is_empty());
    let color = fm
        .iter()
        .find(|(k, _)| k == "color")
        .map(|(_, v)| v.clone())
        .filter(|s| !s.is_empty());

    Some(AgentDefinitionEntry {
        name,
        description,
        system_prompt: body.trim_start_matches('\n').to_string(),
        tools,
        model,
        color,
        source,
        file_path: Some(path.to_string_lossy().to_string()),
    })
}

/// Render an `AgentDefinitionEntry` back to on-disk markdown.
fn render_agent_file(entry: &AgentDefinitionEntry) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", entry.name));
    if !entry.description.is_empty() {
        out.push_str(&format!(
            "description: {}\n",
            one_line(&entry.description)
        ));
    }
    if !entry.tools.is_empty() {
        out.push_str(&format!("tools: {}\n", entry.tools.join(", ")));
    }
    if let Some(model) = &entry.model {
        if !model.is_empty() {
            out.push_str(&format!("model: {}\n", model));
        }
    }
    if let Some(color) = &entry.color {
        if !color.is_empty() {
            out.push_str(&format!("color: {}\n", color));
        }
    }
    out.push_str("---\n\n");
    out.push_str(entry.system_prompt.trim_start_matches('\n'));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Minimal YAML-frontmatter parser. Matches the permissive shape used by
/// `cc_skills::loader::parse_frontmatter` — we intentionally reimplement
/// locally to avoid dragging a dependency on the skills crate and to return
/// the raw key list (preserving case for `name` etc.).
fn parse_frontmatter(content: &str) -> (Vec<(String, String)>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (vec![], content.to_string());
    }
    let after_open = &trimmed[3..];
    let Some(close_pos) = after_open.find("\n---") else {
        return (vec![], content.to_string());
    };
    let yaml_block = &after_open[..close_pos];
    let body = &after_open[close_pos + 4..];
    let mut out = Vec::new();
    for line in yaml_block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value)
            .to_string();
        out.push((key, value));
    }
    (out, body.to_string())
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split([',', ' '])
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn one_line(s: &str) -> String {
    s.replace('\n', " ").trim().to_string()
}

// ---------------------------------------------------------------------------
// Persistence: upsert + delete
// ---------------------------------------------------------------------------

fn upsert_agent(
    cwd: &Path,
    entry: AgentDefinitionEntry,
) -> Result<AgentDefinitionEntry, (String, String)> {
    if !entry.source.is_editable() {
        return Err((
            entry.name.clone(),
            format!(
                "source `{}` is read-only — cannot edit agent",
                entry.source.label()
            ),
        ));
    }
    validate_name(&entry.name).map_err(|e| (entry.name.clone(), e))?;

    let dir = agents_dir_for_source(cwd, &entry.source);
    fs::create_dir_all(&dir).map_err(|e| {
        (
            entry.name.clone(),
            format!("failed to create {}: {}", dir.display(), e),
        )
    })?;

    let path = dir.join(format!("{}.md", entry.name));
    let rendered = render_agent_file(&entry);
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, &rendered).map_err(|e| {
        (
            entry.name.clone(),
            format!("failed to write {}: {}", tmp.display(), e),
        )
    })?;
    fs::rename(&tmp, &path).map_err(|e| {
        (
            entry.name.clone(),
            format!(
                "failed to rename {} -> {}: {}",
                tmp.display(),
                path.display(),
                e
            ),
        )
    })?;

    let mut saved = entry;
    saved.file_path = Some(path.to_string_lossy().to_string());
    Ok(saved)
}

fn delete_agent(
    cwd: &Path,
    name: &str,
    source: &AgentDefinitionSource,
) -> Result<(), String> {
    if !source.is_editable() {
        return Err(format!(
            "source `{}` is read-only — cannot delete agent",
            source.label()
        ));
    }
    validate_name(name)?;
    let path = agents_dir_for_source(cwd, source).join(format!("{}.md", name));
    if !path.exists() {
        return Err(format!(
            "no agent file at {} — nothing to delete",
            path.display()
        ));
    }
    fs::remove_file(&path).map_err(|e| format!("failed to remove {}: {}", path.display(), e))?;
    Ok(())
}

fn agents_dir_for_source(cwd: &Path, source: &AgentDefinitionSource) -> PathBuf {
    match source {
        AgentDefinitionSource::User => cc_config::paths::data_root().join("agents"),
        AgentDefinitionSource::Project => cwd.join(".cc-rust").join("agents"),
        // Read-only scopes are rejected earlier, but return something
        // harmless just in case.
        AgentDefinitionSource::Builtin | AgentDefinitionSource::Plugin { .. } => cwd.join(""),
    }
}

/// Agent names become filenames on disk — reject anything that would escape
/// the directory or produce an ambiguous path. This is stricter than the
/// upstream TS loader; the extra caution is worth it because writes happen
/// without a confirmation prompt.
fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("agent name must not be empty".to_string());
    }
    if name.len() > 64 {
        return Err("agent name must be 64 characters or fewer".to_string());
    }
    let ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok {
        return Err(
            "agent name may only contain letters, digits, `-`, and `_`".to_string(),
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_entry(name: &str, source: AgentDefinitionSource) -> AgentDefinitionEntry {
        AgentDefinitionEntry {
            name: name.to_string(),
            description: format!("Agent {} description", name),
            system_prompt: "You are a helpful agent.\n\nDo your best work.\n".to_string(),
            tools: vec!["Read".to_string(), "Grep".to_string()],
            model: Some("sonnet".to_string()),
            color: Some("blue".to_string()),
            source,
            file_path: None,
        }
    }

    #[test]
    fn validate_name_rejects_traversal_and_weirdness() {
        assert!(validate_name("").is_err());
        assert!(validate_name("../escape").is_err());
        assert!(validate_name("slash/inside").is_err());
        assert!(validate_name("dot.inside").is_err());
        assert!(validate_name("good_name-1").is_ok());
    }

    #[test]
    fn upsert_and_reload_round_trips_project_scope() {
        let tmp = tempdir().unwrap();
        let entry = make_entry("reviewer", AgentDefinitionSource::Project);

        let saved = upsert_agent(tmp.path(), entry.clone()).unwrap();
        assert_eq!(saved.name, "reviewer");
        let expected = tmp
            .path()
            .join(".cc-rust")
            .join("agents")
            .join("reviewer.md");
        assert!(expected.exists(), "file should have been written");

        let loaded = load_agents_from_dir(
            &tmp.path().join(".cc-rust").join("agents"),
            AgentDefinitionSource::Project,
        );
        assert_eq!(loaded.len(), 1);
        let back = &loaded[0];
        assert_eq!(back.name, "reviewer");
        assert_eq!(back.description, "Agent reviewer description");
        assert_eq!(back.tools, vec!["Read".to_string(), "Grep".to_string()]);
        assert_eq!(back.model.as_deref(), Some("sonnet"));
        assert_eq!(back.color.as_deref(), Some("blue"));
        assert!(back.system_prompt.contains("You are a helpful agent."));
    }

    #[test]
    fn upsert_rejects_builtin_source() {
        let tmp = tempdir().unwrap();
        let entry = make_entry("general-purpose", AgentDefinitionSource::Builtin);
        let err = upsert_agent(tmp.path(), entry).unwrap_err();
        assert!(err.1.contains("read-only"));
    }

    #[test]
    fn delete_round_trips_and_is_rejected_for_builtin() {
        let tmp = tempdir().unwrap();
        let entry = make_entry("temp", AgentDefinitionSource::Project);
        upsert_agent(tmp.path(), entry).unwrap();

        let path = tmp
            .path()
            .join(".cc-rust")
            .join("agents")
            .join("temp.md");
        assert!(path.exists());

        delete_agent(tmp.path(), "temp", &AgentDefinitionSource::Project).unwrap();
        assert!(!path.exists());

        let err = delete_agent(tmp.path(), "general-purpose", &AgentDefinitionSource::Builtin)
            .unwrap_err();
        assert!(err.contains("read-only"));
    }

    #[test]
    fn parse_agent_file_extracts_frontmatter_and_body() {
        let raw = "---\nname: foo\ndescription: A foo agent\ntools: Read, Bash\nmodel: opus\ncolor: red\n---\n\nBody text here.\n";
        let parsed = parse_agent_file(
            &PathBuf::from("/tmp/foo.md"),
            raw,
            AgentDefinitionSource::User,
        )
        .expect("should parse");
        assert_eq!(parsed.name, "foo");
        assert_eq!(parsed.description, "A foo agent");
        assert_eq!(parsed.tools, vec!["Read".to_string(), "Bash".to_string()]);
        assert_eq!(parsed.model.as_deref(), Some("opus"));
        assert_eq!(parsed.color.as_deref(), Some("red"));
        assert!(parsed.system_prompt.starts_with("Body text here."));
    }

    #[test]
    fn list_all_agents_always_includes_builtins() {
        let tmp = tempdir().unwrap();
        let list = list_all_agents(tmp.path());
        let names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"general-purpose"));
        assert!(names.contains(&"Explore"));
        assert!(names.contains(&"Plan"));
        assert!(names.contains(&"code-reviewer"));
    }

    #[test]
    fn render_agent_file_roundtrips() {
        let entry = make_entry("rtrip", AgentDefinitionSource::User);
        let rendered = render_agent_file(&entry);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("name: rtrip"));
        assert!(rendered.contains("tools: Read, Grep"));
        assert!(rendered.contains("model: sonnet"));
        assert!(rendered.contains("color: blue"));
        assert!(rendered.contains("You are a helpful agent."));
    }

    #[test]
    fn agent_source_editable_flag() {
        assert!(!AgentDefinitionSource::Builtin.is_editable());
        assert!(AgentDefinitionSource::User.is_editable());
        assert!(AgentDefinitionSource::Project.is_editable());
        assert!(!AgentDefinitionSource::Plugin { id: "p".into() }.is_editable());
    }
}
