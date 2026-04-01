//! Skill directory loader — discovers and parses SKILL.md files.
//!
//! Corresponds to TypeScript: src/skills/loadSkillsDir.ts
//!
//! Discovery rules:
//! - Skills live in directories named after the skill: `skill-name/SKILL.md`
//! - Frontmatter is YAML between `---` fences at the top of the file
//! - The directory name is the canonical skill name
//! - Single `.md` files in a skills directory are NOT loaded (must be in subdirs)
//! - Legacy `.cc-rust/commands/` directories use a different layout (command.md)

#![allow(unused)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{SkillContext, SkillDefinition, SkillFrontmatter, SkillSource};

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

/// Parse YAML-ish frontmatter from a markdown string.
///
/// Returns (frontmatter_map, body_after_frontmatter).
/// If no frontmatter is found, returns an empty map and the full text.
pub fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (HashMap::new(), content.to_string());
    }

    // Find closing ---
    let after_open = &trimmed[3..];
    let close_pos = after_open.find("\n---");
    let Some(close_pos) = close_pos else {
        return (HashMap::new(), content.to_string());
    };

    let yaml_block = &after_open[..close_pos];
    let body = &after_open[close_pos + 4..]; // skip "\n---"
    let body = body.trim_start_matches('\n');

    let mut map = HashMap::new();
    for line in yaml_block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_lowercase().replace(' ', "-");
            let value = value.trim().to_string();
            // Strip surrounding quotes
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(&value)
                .to_string();
            map.insert(key, value);
        }
    }

    (map, body.to_string())
}

/// Parse frontmatter map into structured SkillFrontmatter.
pub fn parse_skill_frontmatter(
    fm: &HashMap<String, String>,
    body: &str,
) -> SkillFrontmatter {
    let description = fm
        .get("description")
        .cloned()
        .unwrap_or_else(|| extract_first_paragraph(body));

    let when_to_use = fm.get("when-to-use").or(fm.get("when_to_use")).cloned();

    let allowed_tools = fm
        .get("allowed-tools")
        .or(fm.get("allowed_tools"))
        .map(|s| {
            s.split([',', ' '])
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let argument_hint = fm.get("argument-hint").or(fm.get("argument_hint")).cloned();

    let argument_names = fm
        .get("arguments")
        .map(|s| {
            s.split([',', ' '])
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let model = fm.get("model").cloned();

    let user_invocable = fm
        .get("user-invocable")
        .or(fm.get("user_invocable"))
        .map(|v| v != "false")
        .unwrap_or(true);

    let disable_model_invocation = fm
        .get("disable-model-invocation")
        .or(fm.get("disable_model_invocation"))
        .map(|v| v == "true")
        .unwrap_or(false);

    let context = fm
        .get("context")
        .map(|v| {
            if v == "fork" {
                SkillContext::Fork
            } else {
                SkillContext::Inline
            }
        })
        .unwrap_or_default();

    let agent = fm.get("agent").cloned();
    let effort = fm.get("effort").cloned();
    let version = fm.get("version").cloned();
    let name = fm.get("name").cloned();

    let paths = fm
        .get("paths")
        .map(|s| {
            // Simple parsing: comma-separated or bracket-list
            let s = s.trim_start_matches('[').trim_end_matches(']');
            s.split(',')
                .map(|p| p.trim().trim_matches('"').trim_matches('\'').to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();

    SkillFrontmatter {
        name,
        description,
        when_to_use,
        allowed_tools,
        argument_hint,
        argument_names,
        model,
        user_invocable,
        disable_model_invocation,
        context,
        agent,
        effort,
        version,
        paths,
    }
}

/// Extract the first non-empty paragraph from markdown as a fallback description.
fn extract_first_paragraph(body: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            return trimmed.to_string();
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Directory scanning
// ---------------------------------------------------------------------------

/// Load all skills from a directory.
///
/// Expects the layout: `dir/skill-name/SKILL.md`
///
/// Returns a Vec of parsed skill definitions.
pub fn load_skills_from_dir(dir: &Path, source: SkillSource) -> Vec<SkillDefinition> {
    let mut skills = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return skills,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_file = path.join("SKILL.md");
        if !skill_file.is_file() {
            // Also try lowercase
            let skill_file_lower = path.join("skill.md");
            if skill_file_lower.is_file() {
                if let Some(skill) = load_skill_file(&skill_file_lower, &path, &source) {
                    skills.push(skill);
                }
                continue;
            }
            continue;
        }

        if let Some(skill) = load_skill_file(&skill_file, &path, &source) {
            skills.push(skill);
        }
    }

    skills
}

/// Load legacy commands from `.cc-rust/commands/` directory.
///
/// Legacy format: `commands/command-name.md` (single file, no subdirectory).
pub fn load_legacy_commands(dir: &Path, source: SkillSource) -> Vec<SkillDefinition> {
    let mut skills = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return skills,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Directory format: dir/name/SKILL.md (preferred)
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.is_file() {
                if let Some(skill) = load_skill_file(&skill_file, &path, &source) {
                    skills.push(skill);
                }
                continue;
            }
        }

        // Single file format: dir/name.md (legacy)
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "md" {
                    if let Some(skill) = load_skill_file(&path, &path.parent().unwrap_or(&path), &source) {
                        skills.push(skill);
                    }
                }
            }
        }
    }

    skills
}

/// Load a single skill from a SKILL.md file.
fn load_skill_file(
    file_path: &Path,
    skill_dir: &Path,
    source: &SkillSource,
) -> Option<SkillDefinition> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let (fm_map, body) = parse_frontmatter(&content);
    let frontmatter = parse_skill_frontmatter(&fm_map, &body);

    // Derive name from directory or file stem
    let name = frontmatter.name.clone().unwrap_or_else(|| {
        if file_path.file_name().map(|f| f == "SKILL.md" || f == "skill.md").unwrap_or(false) {
            // Use parent directory name
            skill_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed".to_string())
        } else {
            // Use file stem
            file_path
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed".to_string())
        }
    });

    Some(SkillDefinition {
        name,
        source: source.clone(),
        base_dir: Some(skill_dir.to_path_buf()),
        frontmatter,
        prompt_body: body,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_frontmatter_basic() {
        let content = r#"---
description: A test skill
allowed-tools: Read, Grep, Bash
user-invocable: true
---

Do something useful."#;

        let (fm, body) = parse_frontmatter(content);
        assert_eq!(fm.get("description").unwrap(), "A test skill");
        assert_eq!(fm.get("allowed-tools").unwrap(), "Read, Grep, Bash");
        assert_eq!(fm.get("user-invocable").unwrap(), "true");
        assert!(body.contains("Do something useful."));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "Just plain markdown body.";
        let (fm, body) = parse_frontmatter(content);
        assert!(fm.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_frontmatter_quoted_values() {
        let content = "---\nname: \"My Skill\"\n---\nBody";
        let (fm, _body) = parse_frontmatter(content);
        assert_eq!(fm.get("name").unwrap(), "My Skill");
    }

    #[test]
    fn test_parse_skill_frontmatter() {
        let mut fm = HashMap::new();
        fm.insert("description".to_string(), "Test desc".to_string());
        fm.insert("allowed-tools".to_string(), "Read,Grep".to_string());
        fm.insert("context".to_string(), "fork".to_string());
        fm.insert("disable-model-invocation".to_string(), "true".to_string());

        let sf = parse_skill_frontmatter(&fm, "body");
        assert_eq!(sf.description, "Test desc");
        assert_eq!(sf.allowed_tools, vec!["Read", "Grep"]);
        assert_eq!(sf.context, SkillContext::Fork);
        assert!(sf.disable_model_invocation);
        assert!(sf.user_invocable); // default true
    }

    #[test]
    fn test_parse_skill_frontmatter_defaults() {
        let fm = HashMap::new();
        let sf = parse_skill_frontmatter(&fm, "First paragraph here.\n\nSecond.");
        assert_eq!(sf.description, "First paragraph here.");
        assert!(sf.user_invocable);
        assert!(!sf.disable_model_invocation);
        assert_eq!(sf.context, SkillContext::Inline);
    }

    #[test]
    fn test_parse_paths_frontmatter() {
        let mut fm = HashMap::new();
        fm.insert("paths".to_string(), "[\"src/**\", \"lib/**\"]".to_string());
        let sf = parse_skill_frontmatter(&fm, "");
        assert_eq!(sf.paths, vec!["src/**", "lib/**"]);
    }

    #[test]
    fn test_extract_first_paragraph() {
        assert_eq!(extract_first_paragraph("# Title\nParagraph"), "Paragraph");
        assert_eq!(extract_first_paragraph("Immediate text"), "Immediate text");
        assert_eq!(extract_first_paragraph(""), "");
    }

    #[test]
    fn test_load_skills_from_dir_empty() {
        let dir = std::env::temp_dir().join("test_skills_empty");
        let _ = std::fs::create_dir_all(&dir);
        let skills = load_skills_from_dir(&dir, SkillSource::User);
        // May or may not be empty depending on temp dir content
        // Just verify it doesn't panic
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_skill_from_temp_dir() {
        let base = std::env::temp_dir().join("test_skills_load");
        let skill_dir = base.join("my-skill");
        let _ = std::fs::create_dir_all(&skill_dir);

        let skill_file = skill_dir.join("SKILL.md");
        let mut f = std::fs::File::create(&skill_file).unwrap();
        writeln!(
            f,
            "---\ndescription: My test skill\nallowed-tools: Read, Bash\n---\n\nDo the thing."
        )
        .unwrap();

        let skills = load_skills_from_dir(&base, SkillSource::User);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].frontmatter.description, "My test skill");
        assert_eq!(skills[0].frontmatter.allowed_tools, vec!["Read", "Bash"]);
        assert!(skills[0].prompt_body.contains("Do the thing."));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_load_legacy_command_single_file() {
        let base = std::env::temp_dir().join("test_legacy_cmds");
        let _ = std::fs::create_dir_all(&base);

        let cmd_file = base.join("greet.md");
        let mut f = std::fs::File::create(&cmd_file).unwrap();
        writeln!(f, "---\ndescription: Greet someone\n---\n\nSay hello.").unwrap();

        let skills = load_legacy_commands(&base, SkillSource::User);
        assert!(skills.iter().any(|s| s.name == "greet"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
