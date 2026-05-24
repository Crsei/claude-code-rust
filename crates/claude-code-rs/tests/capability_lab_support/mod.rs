#![allow(dead_code)]

use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

pub const FS_SERVER_PACKAGE: &str = "@modelcontextprotocol/server-filesystem@2026.1.14";
pub const SEQUENTIAL_SERVER_PACKAGE: &str =
    "@modelcontextprotocol/server-sequential-thinking@2025.12.18";
pub const PLAYWRIGHT_MCP_PACKAGE: &str = "@playwright/mcp@0.0.75";
pub const CONTEXT7_MCP_PACKAGE: &str = "@upstash/context7-mcp@2.3.0";
pub const GITHUB_MCP_PACKAGE: &str = "@modelcontextprotocol/server-github@2025.4.8";

pub struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    pub fn set_path(key: &'static str, path: &Path) -> Self {
        Self::set(key, path.to_string_lossy().as_ref())
    }

    pub fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

pub struct CapabilityLab {
    _root: TempDir,
    pub project_dir: PathBuf,
    pub home_dir: PathBuf,
    pub allthecodes_home: PathBuf,
}

impl CapabilityLab {
    pub fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("cc-rust-capability-lab-")
            .tempdir()
            .expect("create capability lab tempdir");
        let project_dir = root.path().join("cc-rust-capability-lab");
        let home_dir = root.path().join("home");
        let allthecodes_home = home_dir.join(".allthecodes");

        fs::create_dir_all(project_dir.join("src")).expect("create src");
        fs::create_dir_all(project_dir.join("tests")).expect("create tests");
        fs::create_dir_all(project_dir.join("docs")).expect("create docs");
        fs::create_dir_all(
            project_dir
                .join(".allthecodes")
                .join("skills")
                .join("product-brief-writer")
                .join("references"),
        )
        .expect("create project skill");
        fs::create_dir_all(&allthecodes_home).expect("create allthecodes home");

        write_file(
            &project_dir.join("package.json"),
            r#"{
  "name": "cc-rust-capability-lab",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "test": "node --test",
    "start": "tsx src/server.ts",
    "test:ui": "playwright test"
  },
  "dependencies": {
    "@playwright/test": "^1.56.0",
    "better-sqlite3": "^12.0.0",
    "tsx": "^4.20.0"
  },
  "devDependencies": {}
}
"#,
        );
        write_file(
            &project_dir.join("src").join("db.ts"),
            r#"export type Issue = { id: number; title: string; status: "open" | "closed" };

export const issues: Issue[] = [
  { id: 1, title: "Closed issues still render as open", status: "open" },
  { id: 2, title: "Add keyboard navigation", status: "open" },
  { id: 3, title: "Document release checklist", status: "closed" }
];

export function listIssues(): Issue[] {
  return issues;
}

export function closeIssue(id: number): Issue | undefined {
  const issue = issues.find((item) => item.id === id);
  if (issue) issue.status = "closed";
  return issue;
}
"#,
        );
        write_file(
            &project_dir.join("src").join("routes.ts"),
            r#"import { closeIssue, listIssues } from "./db.js";

export function renderIssueList() {
  return `<ul>${listIssues()
    .map((issue) => `<li data-id="${issue.id}">${issue.title}: ${issue.status}</li>`)
    .join("")}</ul>`;
}

export function closeAndRender(id: number) {
  closeIssue(id);
  return renderIssueList();
}
"#,
        );
        write_file(
            &project_dir.join("src").join("server.ts"),
            r#"import { createServer } from "node:http";
import { closeAndRender, renderIssueList } from "./routes.js";

const server = createServer((req, res) => {
  if (req.url === "/health") {
    res.end("ok");
    return;
  }
  if (req.url === "/issues") {
    res.end(renderIssueList());
    return;
  }
  if (req.url === "/issues/1/close") {
    res.end(closeAndRender(1));
    return;
  }
  res.statusCode = 404;
  res.end("not found");
});

server.listen(process.env.PORT || 0);
"#,
        );
        write_file(
            &project_dir.join("tests").join("api.spec.ts"),
            r#"import { strict as assert } from "node:assert";
import { closeAndRender } from "../src/routes.js";

assert.match(closeAndRender(1), /Closed issues still render as open: closed/);
"#,
        );
        write_file(
            &project_dir.join("docs").join("product-brief.md"),
            "# Product Brief\n\nBuild a compact issue tracker that lets users create, list, and close issues. Closed issues must visibly show `closed` in every list view.\n",
        );
        write_file(
            &project_dir.join("docs").join("prd-draft.md"),
            "# PRD Draft\n\nThe issue list is the primary operational view. Status changes must be reflected immediately after close actions.\n",
        );
        write_file(
            &project_dir.join("docs").join("report-template.md"),
            "# Fix Summary\n\n## User Impact\n\n## Technical Change\n\n## Verification\n",
        );
        write_file(
            &project_dir
                .join(".allthecodes")
                .join("skills")
                .join("product-brief-writer")
                .join("SKILL.md"),
            r#"---
name: product-brief-writer
description: Use when writing product briefs, PRD summaries, or fix-summary documents from product docs.
allowed-tools: Read, Write
version: 1.0.0
---

Use `references/style-guide.md` before writing any product-facing summary. Keep the output concise and include user impact, technical change, and verification.
"#,
        );
        write_file(
            &project_dir.join(".allthecodes").join("skills").join("product-brief-writer").join("references").join("style-guide.md"),
            "# Style Guide\n\nUse direct headings, short paragraphs, and explicitly mention acceptance criteria.\n",
        );
        write_file(
            &project_dir.join("AGENTS.md"),
            "# Capability Lab Instructions\n\nAll generated reports must be written under `docs/`. Keep persistent allthecodes state under `.allthecodes/` or `ALLTHECODES_HOME`.\n",
        );
        write_file(&project_dir.join(".allthecodes").join("settings.json"), "{}\n");

        Self {
            _root: root,
            project_dir,
            home_dir,
            allthecodes_home,
        }
    }

    pub fn set_env(&self) -> (EnvGuard, EnvGuard) {
        (
            EnvGuard::set_path("HOME", &self.home_dir),
            EnvGuard::set_path("ALLTHECODES_HOME", &self.allthecodes_home),
        )
    }

    pub fn init_git(&self) {
        run_git(&self.project_dir, &["init"]);
        run_git(
            &self.project_dir,
            &["config", "user.name", "Capability Test"],
        );
        run_git(
            &self.project_dir,
            &["config", "user.email", "capability@example.invalid"],
        );
        run_git(&self.project_dir, &["add", "."]);
        run_git(&self.project_dir, &["commit", "-m", "initial lab fixture"]);
    }

    pub fn write_project_mcp_settings(&self, mcp_servers: serde_json::Value) {
        let settings = json!({ "mcpServers": mcp_servers });
        write_file(
            &self.project_dir.join(".allthecodes").join("settings.json"),
            &serde_json::to_string_pretty(&settings).expect("serialize project settings"),
        );
    }

    pub fn write_plugin_fixture(&self) -> PathBuf {
        let plugin_dir = self
            .project_dir
            .join("fixtures")
            .join("plugins")
            .join("capability-plugin");
        fs::create_dir_all(
            plugin_dir
                .join("skills")
                .join("plugin-review")
                .join("references"),
        )
        .expect("create plugin skill");
        let plugin_manifest = json!({
            "name": "capability-plugin",
            "display_name": "Capability Plugin",
            "version": "1.0.0",
            "description": "Local plugin fixture for MCP, Skill, and Plugin integration tests.",
            "skills": [
                {
                    "name": "plugin-review",
                    "path": "skills/plugin-review/SKILL.md",
                    "description": "Review capability lab changes."
                }
            ],
            "mcp_servers": [
                {
                    "name": "plugin-sequential",
                    "command": "npx",
                    "args": ["-y", SEQUENTIAL_SERVER_PACKAGE],
                    "env": {
                        "npm_config_cache": self.npm_cache_dir()
                    }
                }
            ],
            "commands": [
                {
                    "name": "capability-review",
                    "description": "Review capability lab changes."
                }
            ]
        });
        write_file(
            &plugin_dir.join("plugin.json"),
            &serde_json::to_string_pretty(&plugin_manifest).expect("serialize plugin manifest"),
        );
        write_file(
            &plugin_dir
                .join("skills")
                .join("plugin-review")
                .join("SKILL.md"),
            r#"---
name: plugin-review
description: Use when reviewing capability lab changes.
allowed-tools: Read
version: 1.0.0
---

Review the changed issue tracker code and report user-visible risks.
"#,
        );
        write_file(
            &plugin_dir
                .join("skills")
                .join("plugin-review")
                .join("references")
                .join("rubric.md"),
            "# Review Rubric\n\nCheck behavior, tests, and path isolation.\n",
        );
        plugin_dir
    }

    pub fn write_skill_only_plugin_fixture(&self) -> PathBuf {
        let plugin_dir = self
            .project_dir
            .join("fixtures")
            .join("plugins")
            .join("capability-skill-plugin");
        fs::create_dir_all(
            plugin_dir
                .join("skills")
                .join("plugin-review")
                .join("references"),
        )
        .expect("create skill-only plugin skill");
        let plugin_manifest = json!({
            "name": "capability-skill-plugin",
            "display_name": "Capability Skill Plugin",
            "version": "1.0.0",
            "description": "Local plugin fixture that contributes a review skill without starting extra MCP servers.",
            "skills": [
                {
                    "name": "plugin-review",
                    "path": "skills/plugin-review/SKILL.md",
                    "description": "Review capability lab changes."
                }
            ],
            "commands": [
                {
                    "name": "capability-review",
                    "description": "Review capability lab changes."
                }
            ]
        });
        write_file(
            &plugin_dir.join("plugin.json"),
            &serde_json::to_string_pretty(&plugin_manifest).expect("serialize plugin manifest"),
        );
        write_file(
            &plugin_dir
                .join("skills")
                .join("plugin-review")
                .join("SKILL.md"),
            r#"---
name: plugin-review
description: Use when reviewing capability lab changes.
allowed-tools: Read
version: 1.0.0
---

Review the changed issue tracker code and report user-visible risks.
"#,
        );
        write_file(
            &plugin_dir
                .join("skills")
                .join("plugin-review")
                .join("references")
                .join("rubric.md"),
            "# Review Rubric\n\nCheck behavior, tests, and path isolation.\n",
        );
        plugin_dir
    }

    pub fn npm_cache_dir(&self) -> PathBuf {
        self.allthecodes_home.join("npm-cache")
    }

    pub fn assert_path_isolated(&self) {
        let mut forbidden = Vec::new();
        collect_forbidden_paths(&self.home_dir, &mut forbidden);
        collect_forbidden_paths(&self.project_dir, &mut forbidden);
        assert!(
            forbidden.is_empty(),
            "cc-rust capability tests wrote forbidden upstream paths: {:?}",
            forbidden
        );
    }
}

pub fn filesystem_server_config(
    project_dir: &Path,
    npm_cache_dir: &Path,
) -> cc_mcp::McpServerConfig {
    cc_mcp::McpServerConfig {
        name: "filesystem".to_string(),
        transport: "stdio".to_string(),
        command: Some("npx".to_string()),
        args: Some(vec![
            "-y".to_string(),
            FS_SERVER_PACKAGE.to_string(),
            project_dir.to_string_lossy().to_string(),
        ]),
        url: None,
        headers: None,
        oauth: None,
        env: Some(npx_env(npm_cache_dir)),
        browser_mcp: None,
        disabled: None,
    }
}

pub fn git_server_config(project_dir: &Path) -> cc_mcp::McpServerConfig {
    cc_mcp::McpServerConfig {
        name: "git".to_string(),
        transport: "stdio".to_string(),
        command: Some("uvx".to_string()),
        args: Some(vec![
            "--from".to_string(),
            "mcp-server-git".to_string(),
            "mcp-server-git".to_string(),
            "--repository".to_string(),
            project_dir.to_string_lossy().to_string(),
        ]),
        url: None,
        headers: None,
        oauth: None,
        env: None,
        browser_mcp: None,
        disabled: None,
    }
}

pub fn sequential_server_config(npm_cache_dir: &Path) -> cc_mcp::McpServerConfig {
    cc_mcp::McpServerConfig {
        name: "sequential-thinking".to_string(),
        transport: "stdio".to_string(),
        command: Some("npx".to_string()),
        args: Some(vec![
            "-y".to_string(),
            SEQUENTIAL_SERVER_PACKAGE.to_string(),
        ]),
        url: None,
        headers: None,
        oauth: None,
        env: Some(npx_env(npm_cache_dir)),
        browser_mcp: None,
        disabled: None,
    }
}

pub fn playwright_server_config(npm_cache_dir: &Path) -> cc_mcp::McpServerConfig {
    cc_mcp::McpServerConfig {
        name: "playwright".to_string(),
        transport: "stdio".to_string(),
        command: Some("npx".to_string()),
        args: Some(vec!["-y".to_string(), PLAYWRIGHT_MCP_PACKAGE.to_string()]),
        url: None,
        headers: None,
        oauth: None,
        env: Some(npx_env(npm_cache_dir)),
        browser_mcp: Some(true),
        disabled: None,
    }
}

pub fn context7_server_config(npm_cache_dir: &Path) -> cc_mcp::McpServerConfig {
    cc_mcp::McpServerConfig {
        name: "context7".to_string(),
        transport: "stdio".to_string(),
        command: Some("npx".to_string()),
        args: Some(vec!["-y".to_string(), CONTEXT7_MCP_PACKAGE.to_string()]),
        url: None,
        headers: None,
        oauth: None,
        env: Some(npx_env(npm_cache_dir)),
        browser_mcp: None,
        disabled: None,
    }
}

pub fn github_server_config(npm_cache_dir: &Path) -> cc_mcp::McpServerConfig {
    let mut env = npx_env(npm_cache_dir);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        env.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), token);
    }
    cc_mcp::McpServerConfig {
        name: "github".to_string(),
        transport: "stdio".to_string(),
        command: Some("npx".to_string()),
        args: Some(vec!["-y".to_string(), GITHUB_MCP_PACKAGE.to_string()]),
        url: None,
        headers: None,
        oauth: None,
        env: Some(env),
        browser_mcp: None,
        disabled: None,
    }
}

fn npx_env(npm_cache_dir: &Path) -> HashMap<String, String> {
    HashMap::from([(
        "npm_config_cache".to_string(),
        npm_cache_dir.to_string_lossy().to_string(),
    )])
}

pub fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }
    fs::write(path, content).unwrap_or_else(|err| panic!("write {}: {}", path.display(), err));
}

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("run git {:?}: {}", args, err));
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn collect_forbidden_paths(root: &Path, out: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        if matches!(name, ".claude" | ".Codex" | ".codex") {
            out.push(path.clone());
        }
        if path.is_dir() {
            collect_forbidden_paths(&path, out);
        }
    }
}
