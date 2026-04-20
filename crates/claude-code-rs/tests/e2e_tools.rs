//! E2E tests for tool registration and system prompt tool sections.
//!
//! These tests verify that tools are correctly registered and appear
//! in the system prompt. They use --dump-system-prompt to inspect
//! the prompt without requiring an API key.
//!
//! Workspace: F:\temp
//! Run with: cargo test --test e2e_tools

use assert_cmd::Command;
use predicates::prelude::*;

#[path = "test_workspace.rs"]
mod test_workspace;

fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn workspace() -> &'static str {
    test_workspace::workspace()
}

/// Strips all API keys so no provider is detected — tests run offline.
fn strip_api_keys(cmd: &mut Command) -> &mut Command {
    cmd.env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env("OPENROUTER_API_KEY", "")
        .env("GOOGLE_API_KEY", "")
        .env("DEEPSEEK_API_KEY", "")
}

// =========================================================================
// 1. Tool registration — core tools appear in system prompt
// =========================================================================

/// The system prompt from --dump-system-prompt should mention each tool.
/// This is a smoke test that the tool registry is wired correctly.
#[test]
fn system_prompt_contains_bash_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bash"));
}

#[test]
fn system_prompt_contains_read_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Read"));
}

#[test]
fn system_prompt_contains_write_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Write"));
}

#[test]
fn system_prompt_contains_edit_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Edit"));
}

#[test]
fn system_prompt_contains_glob_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Glob"));
}

#[test]
fn system_prompt_contains_grep_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Grep"));
}

#[test]
fn system_prompt_contains_askuser_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("AskUser"));
}

#[test]
fn system_prompt_contains_skill_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Skill"));
}

// =========================================================================
// 1b. Phase 2-5 migrated tools appear in system prompt
// =========================================================================

#[test]
fn system_prompt_contains_agent_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## Agent"));
}

#[test]
fn system_prompt_contains_agent_schema() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("subagent_type")
                .and(predicate::str::contains("isolation"))
                .and(predicate::str::contains("run_in_background")),
        );
}

#[test]
fn system_prompt_contains_webfetch_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## WebFetch"));
}

#[test]
fn system_prompt_contains_websearch_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## WebSearch"));
}

#[test]
fn system_prompt_contains_enter_plan_mode_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## EnterPlanMode"));
}

#[test]
fn system_prompt_contains_exit_plan_mode_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## ExitPlanMode"));
}

#[test]
fn system_prompt_contains_task_create_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## TaskCreate"));
}

#[test]
fn system_prompt_contains_task_update_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## TaskUpdate"));
}

#[test]
fn system_prompt_contains_task_list_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## TaskList"));
}

#[test]
fn system_prompt_contains_enter_worktree_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## EnterWorktree"));
}

#[test]
fn system_prompt_contains_exit_worktree_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## ExitWorktree"));
}

// =========================================================================
// 1c. Phase 6-8 migrated tools appear in system prompt
// =========================================================================

#[test]
fn system_prompt_contains_lsp_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## LSP"));
}

#[test]
fn system_prompt_contains_lsp_schema() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("goToDefinition")
                .and(predicate::str::contains("filePath"))
                .and(predicate::str::contains("operation")),
        );
}

// NOTE: SendMessage tool is feature-gated — only appears when
// CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS is set. Default: hidden.
#[test]
fn system_prompt_omits_send_message_by_default() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("## SendMessage").not());
}

// =========================================================================
// 2. System prompt structure checks
// =========================================================================

#[test]
fn system_prompt_contains_environment_section() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Environment").or(predicate::str::contains("environment")),
        );
}

#[test]
fn system_prompt_contains_cwd_path() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        // The system prompt should embed the working directory
        .stdout(predicate::str::contains("temp").or(predicate::str::contains("F:")));
}

#[test]
fn system_prompt_mentions_permission_model() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("permission").or(predicate::str::contains("Permission")));
}

// =========================================================================
// 3. Tool-dependent print mode (requires API — expected to fail gracefully)
// =========================================================================

/// Print mode with a prompt but no API key: should not crash/panic,
/// and should report the API error in output.
#[test]
fn print_mode_bash_tool_no_crash_without_api() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["-p", "-C", workspace(), "run ls in bash"])
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .assert()
        .stdout(
            predicate::str::contains("no API client configured")
                .or(predicate::str::contains("API error")),
        );
}

#[test]
fn print_mode_read_tool_no_crash_without_api() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["-p", "-C", workspace(), "read the file test.txt"])
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .assert()
        .stdout(
            predicate::str::contains("no API client configured")
                .or(predicate::str::contains("API error")),
        );
}

// =========================================================================
// 4. Multiple flags combined
// =========================================================================

#[test]
fn dump_system_prompt_with_verbose_and_model() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args([
        "--dump-system-prompt",
        "-v",
        "-m",
        "test-model",
        "-C",
        workspace(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::is_empty().not());
}

#[test]
fn init_only_with_cwd_and_permission_mode() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args([
        "--init-only",
        "-C",
        workspace(),
        "--permission-mode",
        "auto",
    ])
    .assert()
    .success();
}
