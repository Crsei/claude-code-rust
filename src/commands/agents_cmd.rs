//! `/agents` slash command — read-only browser for agent definitions.
//!
//! Rust issue #34. Surfaces every agent the engine can dispatch, grouped
//! by source, with override/shadow visibility when multiple sources define
//! the same name.
//!
//! In cc-rust "agent" is a superset that covers:
//! * the built-in subagent types the engine always honours (general-purpose,
//!   Explore, Plan, code-reviewer);
//! * user-invocable skills whose frontmatter flips them into forked
//!   (`context: Fork`) or agent-backed execution;
//! * active team members from `TeamContext`.
//!
//! The browser is intentionally read-only. CRUD flows can be layered on top
//! of this aggregator later; the data model we assemble already carries
//! enough metadata (source, path, active state) to support that without a
//! rewrite.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::skills::{self, SkillContext, SkillDefinition, SkillSource};
use crate::ui::browser::{render_with_footer, shorten_path, TreeNode};

pub struct AgentsHandler;

#[async_trait]
impl CommandHandler for AgentsHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut parts = args.split_whitespace();
        let sub = parts.next().unwrap_or("").to_ascii_lowercase();

        match sub.as_str() {
            "" | "list" => {
                let agents = collect_agents(ctx);
                Ok(CommandResult::Output(render_agent_tree(&agents)))
            }
            "show" | "info" => {
                let name = parts.next().unwrap_or("").trim();
                if name.is_empty() {
                    return Ok(CommandResult::Output(
                        "Usage: /agents show <name>".to_string(),
                    ));
                }
                let agents = collect_agents(ctx);
                Ok(CommandResult::Output(render_agent_detail(&agents, name)))
            }
            "sources" => Ok(CommandResult::Output(render_sources(ctx))),
            other => Ok(CommandResult::Output(format!(
                "Unknown /agents subcommand '{}'.\n\n\
                 Usage:\n  \
                 /agents                — list every agent grouped by source\n  \
                 /agents show <name>    — show details for one agent\n  \
                 /agents sources        — show where agents are loaded from\n",
                other
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AgentSource {
    Builtin,
    Skill(SkillSource),
    Team,
}

impl AgentSource {
    fn tag(&self) -> String {
        match self {
            AgentSource::Builtin => "built-in".to_string(),
            AgentSource::Skill(SkillSource::Bundled) => "bundled".to_string(),
            AgentSource::Skill(SkillSource::User) => "user".to_string(),
            AgentSource::Skill(SkillSource::Project) => "project".to_string(),
            AgentSource::Skill(SkillSource::Plugin(name)) => format!("plugin:{}", name),
            AgentSource::Skill(SkillSource::Mcp(name)) => format!("mcp:{}", name),
            AgentSource::Team => "team".to_string(),
        }
    }

    fn group_order(&self) -> u8 {
        match self {
            AgentSource::Builtin => 0,
            AgentSource::Skill(SkillSource::Bundled) => 1,
            AgentSource::Skill(SkillSource::User) => 2,
            AgentSource::Skill(SkillSource::Project) => 3,
            AgentSource::Skill(SkillSource::Plugin(_)) => 4,
            AgentSource::Skill(SkillSource::Mcp(_)) => 5,
            AgentSource::Team => 6,
        }
    }

    fn group_label(&self) -> &'static str {
        match self {
            AgentSource::Builtin => "Built-in subagent types",
            AgentSource::Skill(SkillSource::Bundled) => "Bundled skills",
            AgentSource::Skill(SkillSource::User) => "User skills",
            AgentSource::Skill(SkillSource::Project) => "Project skills",
            AgentSource::Skill(SkillSource::Plugin(_)) => "Plugin-provided",
            AgentSource::Skill(SkillSource::Mcp(_)) => "MCP-provided",
            AgentSource::Team => "Active team members",
        }
    }
}

#[derive(Debug, Clone)]
struct AgentEntry {
    name: String,
    description: String,
    source: AgentSource,
    path: Option<PathBuf>,
    /// `true` when the agent is currently usable (e.g. skill is enabled, team
    /// member is active). Built-ins are always active.
    active: bool,
    /// How this agent executes: `"inline"`, `"fork"`, `"built-in"`, `"team"`.
    execution: &'static str,
}

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

fn collect_agents(ctx: &CommandContext) -> Vec<AgentEntry> {
    let mut out = Vec::new();

    // 1. Built-in subagent types. The Rust engine currently accepts any
    //    `subagent_type` string, but the system prompt advertises this
    //    fixed palette. Surface them so users know what the model can ask
    //    for without needing to grep the prompt.
    for (name, desc) in BUILTIN_SUBAGENTS {
        out.push(AgentEntry {
            name: (*name).to_string(),
            description: (*desc).to_string(),
            source: AgentSource::Builtin,
            path: None,
            active: true,
            execution: "built-in",
        });
    }

    // 2. Skills with agent-like execution. A skill is treated as an agent
    //    when it forks into a child QueryEngine or names a specific
    //    subagent type in frontmatter. Inline-expanding skills are excluded
    //    — they're visible in `/skills` and don't behave like agents.
    for skill in skills::get_all_skills() {
        if !is_agent_skill(&skill) {
            continue;
        }
        let execution = match skill.frontmatter.context {
            SkillContext::Fork => "fork",
            SkillContext::Inline => "inline",
        };
        out.push(AgentEntry {
            name: skill.name.clone(),
            description: skill.frontmatter.description.clone(),
            source: AgentSource::Skill(skill.source.clone()),
            path: skill.base_dir.clone(),
            active: skill.is_model_invocable() || skill.is_user_invocable(),
            execution,
        });
    }

    // 3. Active team members — these are runtime agents registered under
    //    the current team context. They are explicitly active (not just
    //    available), so flag them that way.
    if let Some(team_ctx) = ctx.app_state.team_context.as_ref() {
        for member in load_team_members(team_ctx) {
            out.push(member);
        }
    }

    out
}

fn is_agent_skill(skill: &SkillDefinition) -> bool {
    matches!(skill.frontmatter.context, SkillContext::Fork)
        || skill.frontmatter.agent.is_some()
}

fn load_team_members(team_ctx: &crate::teams::types::TeamContext) -> Vec<AgentEntry> {
    // Read the team file and enumerate members. This is best-effort — if
    // the file is gone or malformed the browser simply skips it.
    let path = PathBuf::from(&team_ctx.team_file_path);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    let Ok(team_file) = serde_json::from_str::<crate::teams::types::TeamFile>(&raw) else {
        return vec![];
    };

    team_file
        .members
        .iter()
        .map(|m| AgentEntry {
            name: m.name.clone(),
            description: m
                .prompt
                .clone()
                .unwrap_or_else(|| format!("Team member ({})", m.agent_id)),
            source: AgentSource::Team,
            path: Some(path.clone()),
            active: m.is_active.unwrap_or(true),
            execution: "team",
        })
        .collect()
}

/// Built-in subagent types the engine supports out of the box. Keep this
/// list in sync with the docstrings in `src/tools/agent/mod.rs` and the
/// system prompt copy.
const BUILTIN_SUBAGENTS: &[(&str, &str)] = &[
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
// Rendering
// ---------------------------------------------------------------------------

fn render_agent_tree(agents: &[AgentEntry]) -> String {
    // Count occurrences per name so we can tag later entries as "shadowed".
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for a in agents {
        *name_counts.entry(a.name.clone()).or_default() += 1;
    }

    // Bucket by source group, preserving group precedence.
    let mut buckets: Vec<(AgentSource, Vec<&AgentEntry>)> = Vec::new();
    let mut seen: Vec<AgentSource> = Vec::new();
    // A stable, predictable order — built-in first so new users see the
    // defaults before their own overrides.
    let mut sorted: Vec<&AgentEntry> = agents.iter().collect();
    sorted.sort_by(|a, b| {
        a.source
            .group_order()
            .cmp(&b.source.group_order())
            .then_with(|| a.name.cmp(&b.name))
    });
    for entry in &sorted {
        if !seen.contains(&entry.source) {
            seen.push(entry.source.clone());
            buckets.push((entry.source.clone(), Vec::new()));
        }
        if let Some(b) = buckets.iter_mut().find(|(s, _)| s == &entry.source) {
            b.1.push(entry);
        }
    }

    // Track names that have already appeared in a higher-precedence bucket.
    // First occurrence is "active", subsequent ones are "shadowed".
    let mut winners: HashMap<String, &AgentSource> = HashMap::new();

    let mut roots: Vec<TreeNode> = Vec::new();
    for (source, entries) in &buckets {
        let mut group = TreeNode::leaf(source.group_label().to_string());
        for entry in entries {
            let is_shadowed = winners
                .get(&entry.name)
                .map(|src| *src != source)
                .unwrap_or(false);
            if !is_shadowed {
                winners.insert(entry.name.clone(), source);
            }

            let status = if !entry.active {
                "disabled"
            } else if is_shadowed {
                "shadowed"
            } else if name_counts.get(&entry.name).copied().unwrap_or(1) > 1 {
                "overrides"
            } else {
                entry.execution
            };

            let detail = if entry.description.is_empty() {
                format!("[{}]", status)
            } else {
                format!("[{}] {}", status, entry.description)
            };

            let label = if let Some(path) = entry.path.as_ref() {
                format!("{}  ({})", entry.name, shorten_path(path))
            } else {
                entry.name.clone()
            };

            group.push_child(
                TreeNode::leaf(label)
                    .with_badge(source.tag())
                    .with_detail(detail),
            );
        }
        roots.push(group);
    }

    let footer = "\
Active agents are the first occurrence in precedence order; later entries with \
the same name are tagged `shadowed`. Use `/agents show <name>` for the full \
description and path.";
    render_with_footer("Agents", &roots, footer)
}

fn render_agent_detail(agents: &[AgentEntry], name: &str) -> String {
    let matches: Vec<&AgentEntry> = agents.iter().filter(|a| a.name == name).collect();
    if matches.is_empty() {
        return format!(
            "No agent named '{}'. Run `/agents` to see every registered agent.",
            name
        );
    }

    let mut out = String::new();
    out.push_str(&format!("Agent: {}\n", name));
    out.push_str(&"─".repeat(name.len() + 7));
    out.push('\n');
    for (i, entry) in matches.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(&format!("--- shadowed by above ({}) ---\n", i));
        }
        out.push_str(&format!("  Source:      {}\n", entry.source.tag()));
        out.push_str(&format!("  Active:      {}\n", entry.active));
        out.push_str(&format!("  Execution:   {}\n", entry.execution));
        if let Some(path) = &entry.path {
            out.push_str(&format!("  Location:    {}\n", shorten_path(path)));
        }
        out.push_str(&format!(
            "  Description: {}\n",
            if entry.description.is_empty() {
                "(none)"
            } else {
                &entry.description
            }
        ));
    }
    out
}

fn render_sources(ctx: &CommandContext) -> String {
    let mut out = String::new();
    out.push_str("Agent discovery sources\n");
    out.push_str("──────────────────────\n");
    out.push_str("  [built-in]   Engine-provided subagent types (always available)\n");
    out.push_str(
        "  [bundled]    Skills compiled into cc-rust (`src/skills/bundled.rs`)\n",
    );
    out.push_str(&format!(
        "  [user]       {}\n",
        shorten_path(&crate::config::paths::skills_dir_global())
    ));
    let project_skills = ctx.cwd.join(".cc-rust").join("skills");
    out.push_str(&format!(
        "  [project]    {}\n",
        shorten_path(&project_skills)
    ));
    out.push_str("  [plugin:*]   Plugin-contributed skills (see `/plugin list`)\n");
    out.push_str("  [team]       Active team members (see `/team members`)\n");
    out.push_str(
        "\nOnly skills whose frontmatter opts into forked execution \
         (`context: Fork`) are listed as agents.\n",
    );
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;

    fn make_ctx() -> CommandContext {
        CommandContext {
            messages: vec![],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn default_lists_builtins_at_minimum() {
        let handler = AgentsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Agents"));
                assert!(s.contains("general-purpose"));
                assert!(s.contains("Explore"));
                assert!(s.contains("Plan"));
                // The built-in bucket should always be rendered.
                assert!(s.contains("Built-in subagent types"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn show_requires_name() {
        let handler = AgentsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("show", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => assert!(s.contains("Usage")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn show_returns_details_for_builtin() {
        let handler = AgentsHandler;
        let mut ctx = make_ctx();
        let result = handler
            .execute("show general-purpose", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Agent: general-purpose"));
                assert!(s.contains("Source:"));
                assert!(s.contains("built-in"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn show_unknown_reports_missing() {
        let handler = AgentsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("show nope-no-way", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => assert!(s.contains("No agent named")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_lists_usage() {
        let handler = AgentsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("banana", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown /agents"));
                assert!(s.contains("/agents show"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn agent_source_group_order_is_stable() {
        assert!(AgentSource::Builtin.group_order() < AgentSource::Team.group_order());
        assert!(
            AgentSource::Skill(SkillSource::Bundled).group_order()
                < AgentSource::Skill(SkillSource::Project).group_order()
        );
    }

    #[test]
    fn tree_marks_shadowed_when_name_appears_twice() {
        let agents = vec![
            AgentEntry {
                name: "dup".into(),
                description: "builtin".into(),
                source: AgentSource::Builtin,
                path: None,
                active: true,
                execution: "built-in",
            },
            AgentEntry {
                name: "dup".into(),
                description: "user".into(),
                source: AgentSource::Skill(SkillSource::User),
                path: None,
                active: true,
                execution: "fork",
            },
        ];
        let out = render_agent_tree(&agents);
        assert!(out.contains("overrides"));
        assert!(out.contains("shadowed"));
    }
}
