//! E2E tests that make REAL API calls using credentials from .env.
//!
//! These tests are IGNORED by default. They only run when:
//!   cargo test --test e2e_live_api -- --ignored
//!
//! Prerequisites:
//!   - F:\AIclassmanager\cc\rust\.env must contain valid API keys
//!   - Network access to the provider endpoints
//!   - F:\temp must exist as the workspace
//!
//! Each test is gated with `#[ignore]` so `cargo test` skips them.
//! Run selectively:
//!   cargo test --test e2e_live_api -- --ignored                    # all live tests
//!   cargo test --test e2e_live_api azure -- --ignored              # only azure
//!   cargo test --test e2e_live_api simple_question -- --ignored    # single test
//!
//! Test tiers:
//!   - Tier 1 (chat): Pure text Q&A, no tool use. Should always pass.
//!   - Tier 2 (tool): Require working tool execution pipeline.
//!     These may fail if the provider doesn't support tool_use properly
//!     (e.g. OpenAI-compat providers that don't forward tool schemas).

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Duration;

#[path = "capability_lab_support/mod.rs"]
mod capability_lab_support;
#[path = "test_workspace.rs"]
mod test_workspace;

fn workspace() -> &'static str {
    test_workspace::workspace()
}

/// Timeout for simple chat tests (no tool use).
const CHAT_TIMEOUT_SECS: u64 = 60;

/// Timeout for tool-use tests (model calls tools, multiple round-trips).
const TOOL_TIMEOUT_SECS: u64 = 120;

/// Timeout for full autonomous coding tests (model + tools + cargo test).
const AUTO_CODE_TIMEOUT_SECS: u64 = 300;

const DEFAULT_LIVE_SETTINGS_PATH: &str =
    "/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/claude-api.json";

const PROVIDER_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_SOTA_MODEL",
    "ANTHROPIC_DEFAULT_MOTA_MODEL",
    "ANTHROPIC_DEFAULT_FOTA_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "AZURE_API_KEY",
    "OPENAI_API_KEY",
    "OPENAI_CODEX_AUTH_TOKEN",
    "OPENAI_CODEX_BASE_URL",
    "OPENAI_CODEX_MODEL",
    "OPENROUTER_API_KEY",
    "GOOGLE_API_KEY",
    "DEEPSEEK_API_KEY",
    "GROQ_API_KEY",
    "ZHIPU_API_KEY",
    "DASHSCOPE_API_KEY",
    "MOONSHOT_API_KEY",
    "BAICHUAN_API_KEY",
    "MINIMAX_API_KEY",
    "YI_API_KEY",
    "SILICONFLOW_API_KEY",
    "STEPFUN_API_KEY",
    "SPARK_API_KEY",
    "CC_BACKEND",
    "CLAUDE_BACKEND",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

/// Build a command that inherits the real .env by running from the project dir.
fn live_cli() -> Command {
    let mut cmd = Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.current_dir(r"F:\AIclassmanager\cc\rust");
    cmd.timeout(Duration::from_secs(CHAT_TIMEOUT_SECS));
    cmd
}

/// Build a command configured for tool-use tests (longer timeout, bypass perms).
fn tool_cli() -> Command {
    let mut cmd = Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.current_dir(r"F:\AIclassmanager\cc\rust");
    cmd.timeout(Duration::from_secs(TOOL_TIMEOUT_SECS));
    cmd
}

fn live_settings_path() -> Option<PathBuf> {
    std::env::var_os("CC_RUST_LIVE_SETTINGS_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            let path = PathBuf::from(DEFAULT_LIVE_SETTINGS_PATH);
            path.exists().then_some(path)
        })
}

fn settings_env_live_cli(cc_home: &Path, cwd: &Path, timeout_secs: u64) -> Command {
    let mut cmd = Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.current_dir(cwd)
        .timeout(Duration::from_secs(timeout_secs))
        .env("CC_RUST_HOME", cc_home)
        .env(
            "CC_RUST_MANAGED_SETTINGS",
            cc_home.join("missing-managed-settings.json"),
        );
    for key in PROVIDER_ENV_KEYS {
        cmd.env_remove(key);
    }
    cmd
}

/// Cleanup helper: remove a file, ignoring errors.
fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn write_autocode_fixture(project: &Path) {
    std::fs::create_dir_all(project.join("src")).expect("create src dir");
    std::fs::write(
        project.join("Cargo.toml"),
        r#"[package]
name = "auto-code-e2e-fixture"
version = "0.1.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
    )
    .expect("write Cargo.toml");
    std::fs::write(
        project.join("src/lib.rs"),
        r#"pub fn discounted_total_cents(items: &[u32], discount_percent: u32) -> u32 {
    let subtotal: u32 = items.iter().sum();
    subtotal - discount_percent
}

#[cfg(test)]
mod tests {
    use super::discounted_total_cents;

    #[test]
    fn applies_percentage_discount() {
        assert_eq!(discounted_total_cents(&[400, 600], 25), 750);
    }

    #[test]
    fn caps_discount_at_one_hundred_percent() {
        assert_eq!(discounted_total_cents(&[120, 80], 150), 0);
    }
}
"#,
    )
    .expect("write src/lib.rs");
}

fn cargo_test(project: &Path) -> std::process::Output {
    StdCommand::new("cargo")
        .arg("test")
        .current_dir(project)
        .output()
        .expect("run cargo test")
}

fn mcp_tool_text(result: &cc_mcp::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| match content {
            cc_mcp::ToolCallContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// =========================================================================
//  TIER 1: Chat tests (no tool use, pure text Q&A)
// =========================================================================

#[test]
#[ignore]
fn t1_simple_question_returns_answer() {
    live_cli()
        .args([
            "-p",
            "-C",
            workspace(),
            "What is 2+2? Reply with just the number.",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4"));
}

#[test]
#[ignore]
fn t1_simple_chinese_question() {
    live_cli()
        .args(["-p", "-C", workspace(), "1+1等于几？只回复数字"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
#[ignore]
fn t1_say_exact_phrase() {
    live_cli()
        .args(["-p", "-C", workspace(), "Say exactly: HELLO_TEST_OK"])
        .assert()
        .success()
        .stdout(predicate::str::contains("HELLO_TEST_OK"));
}

#[test]
#[ignore]
fn t1_custom_system_prompt() {
    live_cli()
        .args([
            "-p",
            "-C",
            workspace(),
            "--system-prompt",
            "You are a calculator. Only output numbers, nothing else.",
            "What is 10 times 5?",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("50"));
}

#[test]
#[ignore]
fn t1_append_system_prompt() {
    live_cli()
        .args([
            "-p",
            "-C",
            workspace(),
            "--append-system-prompt",
            "Always end your response with the word ENDMARK.",
            "Say hi.",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ENDMARK"));
}

#[test]
#[ignore]
fn t1_max_turns_one() {
    live_cli()
        .args([
            "-p",
            "-C",
            workspace(),
            "--max-turns",
            "1",
            "What is 2+3? Reply with just the number.",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("5"));
}

#[test]
#[ignore]
fn t1_print_mode_clean_text() {
    let output = live_cli()
        .args(["-p", "-C", workspace(), "Say exactly: CLEAN_OUTPUT_TEST"])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CLEAN_OUTPUT_TEST"), "stdout: {}", stdout);
    // Should not leak raw tool_use JSON
    assert!(
        !stdout.contains("\"type\":\"tool_use\""),
        "raw tool_use JSON leaked: {}",
        stdout
    );
}

#[test]
#[ignore]
fn t1_env_file_provides_working_credentials() {
    live_cli()
        .args(["-p", "-C", workspace(), "Reply with exactly: ENV_AUTH_OK"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ENV_AUTH_OK"));
}

// =========================================================================
//  TIER 2: Tool-use tests (require working tool execution pipeline)
// =========================================================================

#[test]
#[ignore]
fn t2_bash_echo() {
    tool_cli()
        .args([
            "-p",
            "-C",
            workspace(),
            "--permission-mode",
            "bypass",
            "Use the Bash tool to run: echo TOOL_WORKS_OK",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("TOOL_WORKS_OK"));
}

#[test]
#[ignore]
fn t2_bash_pwd_shows_workspace() {
    tool_cli()
        .args([
            "-p",
            "-C",
            workspace(),
            "--permission-mode",
            "bypass",
            "Use the Bash tool to run: pwd",
        ])
        .assert()
        .success()
        // pwd on Windows/MSYS may show /f/temp or F:\temp
        .stdout(predicate::str::contains("temp"));
}

#[test]
#[ignore]
fn t2_read_file() {
    let test_file = Path::new(workspace()).join("_e2e_read_test.txt");
    std::fs::write(&test_file, "CANARY_READ_12345").expect("write test file");

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", workspace(),
                "--permission-mode", "bypass",
                &format!(
                    "Use the Read tool to read the file at the absolute path {}. Show me the exact contents.",
                    test_file.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("CANARY_READ_12345"));
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_write_file() {
    let test_file = Path::new(workspace()).join("_e2e_write_test.txt");
    cleanup(&test_file);

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", workspace(),
                "--permission-mode", "bypass",
                &format!(
                    "Use the Write tool to write the file at absolute path {} with exactly this content: WRITE_TEST_67890",
                    test_file.display()
                ),
            ])
            .assert()
            .success();

        // Verify on disk
        assert!(test_file.exists(), "file was not created");
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(
            content.contains("WRITE_TEST_67890"),
            "unexpected content: {}",
            content
        );
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_edit_file() {
    let test_file = Path::new(workspace()).join("_e2e_edit_test.txt");
    std::fs::write(&test_file, "Hello OLD_VALUE World").unwrap();

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", workspace(),
                "--permission-mode", "bypass",
                &format!(
                    "First use Read to read {path}, then use Edit to replace 'OLD_VALUE' with 'NEW_VALUE' in {path}.",
                    path = test_file.display()
                ),
            ])
            .assert()
            .success();

        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(
            content.contains("NEW_VALUE"),
            "edit did not apply, content: {}",
            content
        );
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_glob_finds_files() {
    let dir = Path::new(workspace());
    let file_a = dir.join("_e2e_glob_a.txt");
    let file_b = dir.join("_e2e_glob_b.txt");
    std::fs::write(&file_a, "a").unwrap();
    std::fs::write(&file_b, "b").unwrap();

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", workspace(),
                "--permission-mode", "bypass",
                &format!(
                    "Use the Glob tool with pattern '_e2e_glob_*.txt' and path '{}'. List the files found.",
                    workspace()
                ),
            ])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("_e2e_glob_a")
                    .and(predicate::str::contains("_e2e_glob_b")),
            );
    });

    cleanup(&file_a);
    cleanup(&file_b);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_grep_searches_content() {
    let test_file = Path::new(workspace()).join("_e2e_grep_test.txt");
    std::fs::write(
        &test_file,
        "line1: nothing here\nline2: NEEDLE_FOUND_42\nline3: also nothing\n",
    )
    .unwrap();

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", workspace(),
                "--permission-mode", "bypass",
                &format!(
                    "Use the Grep tool to search for the pattern 'NEEDLE_FOUND' in the file at absolute path {}. Show me the matching line.",
                    test_file.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("NEEDLE_FOUND_42"));
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_multi_tool_write_read_edit() {
    let test_file = Path::new(workspace()).join("_e2e_multi_test.txt");
    cleanup(&test_file);

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C",
                workspace(),
                "--permission-mode",
                "bypass",
                &format!(
                    "Do these steps in order using tools:\n\
                     1. Write the file {path} with content 'STEP1_DONE'\n\
                     2. Read {path} to confirm\n\
                     3. Edit {path}: replace 'STEP1_DONE' with 'ALL_STEPS_COMPLETE'\n\
                     4. Read {path} again and show the final content.",
                    path = test_file.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("ALL_STEPS_COMPLETE"));

        let content = std::fs::read_to_string(&test_file).unwrap_or_default();
        assert!(
            content.contains("ALL_STEPS_COMPLETE"),
            "file content: {}",
            content
        );
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_auto_code_edit_with_settings_env_credentials() {
    let Some(settings_path) = live_settings_path() else {
        eprintln!(
            "skipping live auto-code test: set CC_RUST_LIVE_SETTINGS_PATH or create {}",
            DEFAULT_LIVE_SETTINGS_PATH
        );
        return;
    };

    let cc_home = tempfile::tempdir().expect("cc home tempdir");
    std::fs::copy(&settings_path, cc_home.path().join("settings.json"))
        .expect("copy live settings into isolated CC_RUST_HOME");

    let project = tempfile::tempdir().expect("project tempdir");
    write_autocode_fixture(project.path());

    let initial = cargo_test(project.path());
    assert!(
        !initial.status.success(),
        "fixture should start with failing tests; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&initial.stdout),
        String::from_utf8_lossy(&initial.stderr)
    );

    let prompt = format!(
        "You are in a temporary Rust crate at {cwd}.\n\
         Complete this autonomous coding task end-to-end using tools:\n\
         1. Run `cargo test` and inspect the failure.\n\
         2. Read `src/lib.rs`.\n\
         3. Fix only `discounted_total_cents` so it applies a percentage discount and caps discounts above 100% at zero.\n\
         4. Run `cargo test` again and ensure it passes.\n\
         5. Reply with exactly AUTO_CODE_EDIT_OK.\n\
         Do not modify Cargo.toml or remove tests.",
        cwd = project.path().display()
    );

    let output = settings_env_live_cli(cc_home.path(), project.path(), AUTO_CODE_TIMEOUT_SECS)
        .args(["-p", "--permission-mode", "bypass", &prompt])
        .output()
        .expect("run live auto-code task");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "auto-code task failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("AUTO_CODE_EDIT_OK"),
        "model did not report completion\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );

    let source = std::fs::read_to_string(project.path().join("src/lib.rs")).expect("read lib.rs");
    assert!(
        !source.contains("subtotal - discount_percent"),
        "buggy implementation remained:\n{}",
        source
    );

    let final_test = cargo_test(project.path());
    assert!(
        final_test.status.success(),
        "final cargo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&final_test.stdout),
        String::from_utf8_lossy(&final_test.stderr)
    );
}

#[test]
#[ignore]
fn t2_capability_lab_mcp_skill_plugin_end_to_end_with_settings_env_credentials() {
    let Some(settings_path) = live_settings_path() else {
        eprintln!(
            "skipping live capability-lab test: set CC_RUST_LIVE_SETTINGS_PATH or create {}",
            DEFAULT_LIVE_SETTINGS_PATH
        );
        return;
    };

    let lab = capability_lab_support::CapabilityLab::new();
    let (_home_guard, _cc_home_guard) = lab.set_env();
    std::fs::copy(&settings_path, lab.cc_rust_home.join("settings.json"))
        .expect("copy live settings into isolated CC_RUST_HOME");

    lab.init_git();
    lab.write_project_mcp_settings(serde_json::json!({
        "filesystem": {
            "type": "stdio",
            "command": "npx",
            "args": ["-y", capability_lab_support::FS_SERVER_PACKAGE, lab.project_dir],
            "env": { "npm_config_cache": lab.npm_cache_dir() }
        }
    }));
    let discovered_mcp =
        cc_mcp::discovery::discover_mcp_servers(&lab.project_dir).expect("discover project MCP");
    assert!(discovered_mcp
        .iter()
        .any(|server| server.name == "filesystem"));

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let mut manager = cc_mcp::manager::McpManager::new();
        manager
            .connect_server(capability_lab_support::filesystem_server_config(
                &lab.project_dir,
                &lab.npm_cache_dir(),
            ))
            .await
            .expect("connect filesystem MCP");
        let client = manager
            .clients
            .get("filesystem")
            .expect("filesystem client connected");
        assert!(client.tools.iter().any(|tool| tool.name == "read_file"));
        let result = client
            .call_tool(
                "read_file",
                serde_json::json!({ "path": lab.project_dir.join("docs/product-brief.md") }),
            )
            .await
            .expect("read product brief through filesystem MCP");
        assert!(mcp_tool_text(&result).contains("Product Brief"));
    });

    cc_skills::clear_skills();
    cc_skills::reload_skills_with_extra(
        &lab.cc_rust_home.join("skills"),
        Some(&lab.project_dir),
        Vec::new(),
        cc_skills::SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
    );
    assert!(cc_skills::get_all_skills()
        .iter()
        .any(|skill| skill.name == "product-brief-writer"));
    cc_skills::clear_skills();

    cc_plugins::clear_plugins();
    let plugin_dir = lab.write_skill_only_plugin_fixture();
    runtime
        .block_on(cc_plugins::installation::install_plugin(
            plugin_dir.to_str().expect("plugin path utf-8"),
            Some(cc_plugins::installation::InstallScope::Project),
            Some(env!("CARGO_PKG_VERSION")),
            None,
            &Default::default(),
            &Default::default(),
        ))
        .expect("install capability plugin fixture");
    assert!(cc_plugins::installed_plugins_path().starts_with(&lab.cc_rust_home));
    assert!(cc_plugins::installed_plugins_path().is_file());
    cc_plugins::init_plugins();
    assert!(cc_plugins::get_enabled_plugins()
        .iter()
        .any(|plugin| plugin.id == "capability-skill-plugin@local"));
    assert!(cc_plugins::discover_plugin_skill_definitions()
        .iter()
        .any(|skill| skill.plugin_id == "capability-skill-plugin@local"
            && skill.name == "plugin-review"));

    let prompt = format!(
        "You are in the cc-rust capability lab project at {cwd}.\n\
         Complete this short live end-to-end task using the available tools:\n\
         1. Read docs/product-brief.md.\n\
         2. Write docs/fix-summary.md with exactly these section headings: User Impact, Technical Change, Verification.\n\
         3. In Verification, mention these configured capabilities: filesystem MCP, product-brief-writer Skill, capability-skill-plugin Plugin.\n\
         4. Reply with exactly CAPABILITY_LAB_LIVE_OK.\n\
         Do not edit files outside this project.",
        cwd = lab.project_dir.display()
    );

    let mut cmd =
        settings_env_live_cli(&lab.cc_rust_home, &lab.project_dir, AUTO_CODE_TIMEOUT_SECS);
    cmd.env("HOME", &lab.home_dir);
    let output = cmd
        .args([
            "-p",
            "--permission-mode",
            "bypass",
            "--max-turns",
            "8",
            &prompt,
        ])
        .output()
        .expect("run live capability lab task");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "capability lab live task failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("CAPABILITY_LAB_LIVE_OK"),
        "model did not report capability lab completion\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );

    let summary_path = lab.project_dir.join("docs/fix-summary.md");
    assert!(
        summary_path.is_file(),
        "product brief summary was not written"
    );
    let summary = std::fs::read_to_string(summary_path).expect("read fix summary");
    for expected in [
        "User Impact",
        "Technical Change",
        "Verification",
        "filesystem MCP",
        "product-brief-writer Skill",
        "capability-skill-plugin Plugin",
    ] {
        assert!(
            summary.contains(expected),
            "summary missing {expected:?}:\n{summary}"
        );
    }
    lab.assert_path_isolated();
}

#[test]
#[ignore]
fn t2_read_nonexistent_file_graceful() {
    tool_cli()
        .args([
            "-p",
            "-C", workspace(),
            "--permission-mode", "bypass",
            r"Use the Read tool to read F:\temp\_this_file_does_not_exist_xyz.txt and tell me what error you get.",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("not found")
                .or(predicate::str::contains("does not exist"))
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error"))
                .or(predicate::str::contains("No such file"))
                .or(predicate::str::contains("cannot")),
        );
}
