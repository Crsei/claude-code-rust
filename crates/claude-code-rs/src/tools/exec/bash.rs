use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::permissions::dangerous::is_dangerous_command;
use crate::sandbox::{make_runner, policy_from_app_state, preflight_shell_command};
use crate::types::message::AssistantMessage;
use crate::types::tool::{
    InterruptBehavior, PermissionResult, Tool, ToolProgress, ToolResult, ToolUseContext,
    ValidationResult,
};
use crate::utils::bash::{
    extract_command_name, extract_command_prefixes, has_malformed_tokens, has_unterminated_quotes,
    is_command_parseable, parse_command, resolve_timeout, rewrite_windows_null_redirect,
    should_add_stdin_redirect, split_compound_command,
};
use crate::utils::shell::{build_shell_env, detect_default_shell};

/// Truncate output using head+tail strategy.
/// Keeps first `head_lines` lines and last `tail_lines` lines,
/// inserting a separator showing how many lines were omitted.
/// Falls back to character-level truncation if needed.
pub(crate) fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        return output.to_string();
    }

    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    const DEFAULT_HEAD_LINES: usize = 200;
    const TAIL_LINES: usize = 100;

    // If there aren't enough lines for a meaningful head+tail split,
    // fall back to char-level truncation directly.
    if total_lines <= DEFAULT_HEAD_LINES + TAIL_LINES {
        let mut result = output[..max_chars].to_string();
        // Trim to the last newline to avoid cutting mid-line
        if let Some(pos) = result.rfind('\n') {
            result.truncate(pos);
        }
        result.push_str("\n... (output truncated)");
        return result;
    }

    let tail_start = total_lines - TAIL_LINES;
    let tail_part: String = lines[tail_start..].join("\n");

    // Try with full head_lines first, then reduce proportionally
    let mut head_lines = DEFAULT_HEAD_LINES;
    loop {
        if head_lines == 0 {
            // Final fallback: pure char-level truncation
            let mut result = output[..max_chars].to_string();
            if let Some(pos) = result.rfind('\n') {
                result.truncate(pos);
            }
            result.push_str("\n... (output truncated)");
            return result;
        }

        let head_part: String = lines[..head_lines].join("\n");
        let omitted = total_lines - head_lines - TAIL_LINES;
        let separator = format!("\n\n... ({} lines omitted) ...\n\n", omitted);
        let candidate = format!("{}{}{}", head_part, separator, tail_part);

        if candidate.len() <= max_chars {
            return candidate;
        }

        // Reduce head_lines by half each iteration
        head_lines /= 2;
    }
}

/// BashTool — Execute shell commands
///
/// Corresponds to TypeScript: tools/BashTool
pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        BashTool
    }

    fn parse_input(input: &Value) -> (String, Option<u64>, Option<String>) {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timeout_ms = input.get("timeout").and_then(|v| v.as_u64());
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (command, timeout_ms, description)
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    async fn description(&self, _input: &Value) -> String {
        "Executes a given bash command and returns its output.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (max 600000)"
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does"
                }
            },
            "required": ["command"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn get_path(&self, _input: &Value) -> Option<String> {
        None
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
        if !is_command_parseable(command) {
            return ValidationResult::Error {
                message: if command.is_empty() {
                    "Command must not be empty".to_string()
                } else {
                    "Command exceeds maximum parseable length".to_string()
                },
                error_code: 1,
            };
        }
        if has_unterminated_quotes(command) {
            return ValidationResult::Error {
                message: "Command has unterminated quotes".to_string(),
                error_code: 1,
            };
        }
        // Check each parsed token for unbalanced brackets/braces
        if let Ok(tokens) = parse_command(command) {
            for token in &tokens {
                if has_malformed_tokens(token) {
                    return ValidationResult::Error {
                        message: format!(
                            "Command contains malformed token with unbalanced brackets: {}",
                            token
                        ),
                        error_code: 1,
                    };
                }
            }
        }
        ValidationResult::Ok
    }

    async fn check_permissions(&self, input: &Value, ctx: &ToolUseContext) -> PermissionResult {
        let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");

        // Check each sub-command in a compound command for dangerous patterns
        for subcmd in split_compound_command(command) {
            if let Some(reason) = is_dangerous_command(&subcmd) {
                return PermissionResult::Ask {
                    message: format!("Dangerous command detected: {}", reason),
                };
            }
        }

        // Check per-command prefix deny rules from permission context.
        // Rules like "Bash(prefix:rm)" block commands starting with "rm".
        let app_state = (ctx.get_app_state)();
        let prefixes = extract_command_prefixes(command);
        for deny_rules in app_state.tool_permission_context.always_deny_rules.values() {
            for rule in deny_rules {
                if let Some(prefix_pat) = rule
                    .strip_prefix("Bash(prefix:")
                    .and_then(|s| s.strip_suffix(')'))
                {
                    if prefixes.iter().any(|p| p.starts_with(prefix_pat)) {
                        return PermissionResult::Deny {
                            message: format!("Denied by rule: {}", rule),
                        };
                    }
                }
            }
        }

        // Sandbox escape-hatch gate: when the caller passed
        // `dangerouslyDisableSandbox: true` but settings forbid it, deny.
        let wants_escape = input
            .get("dangerouslyDisableSandbox")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if wants_escape
            && !app_state
                .settings
                .sandbox
                .allow_unsandboxed_commands
                .unwrap_or(true)
        {
            return PermissionResult::Deny {
                message: "sandbox.allowUnsandboxedCommands=false rejects \
                          dangerouslyDisableSandbox"
                    .to_string(),
            };
        }

        PermissionResult::Allow {
            updated_input: input.clone(),
        }
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let (command, timeout_ms, _description) = Self::parse_input(&input);

        if command.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "Command must not be empty" }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        // Detect the best available shell for the current platform
        let shell = detect_default_shell();

        // Rewrite Windows CMD-style `>nul` to POSIX `/dev/null` for POSIX shells
        let mut command = if shell.kind.is_posix() {
            rewrite_windows_null_redirect(&command)
        } else {
            command
        };

        // Add stdin redirect (< /dev/null) to prevent interactive hangs,
        // unless the command uses heredoc or already has a stdin redirect
        if shell.needs_stdin_redirect && should_add_stdin_redirect(&command) {
            command = format!("{} < /dev/null", command);
        }
        let mut cmd = Command::new(&shell.path);
        for arg in &shell.exec_args {
            cmd.arg(arg);
        }
        cmd.arg(&command);

        // Inject shell environment (TERM, LANG, GIT_PAGER=cat, CLAUDE_CODE=1, etc.)
        for (k, v) in build_shell_env() {
            cmd.env(&k, &v);
        }

        // Sandbox integration. Build the effective policy from the current
        // AppState + cwd and wrap the command when the sandbox is active.
        // The `dangerouslyDisableSandbox` escape hatch (already gated by
        // check_permissions) bypasses wrapping but still records a warning.
        let escape = input
            .get("dangerouslyDisableSandbox")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let app_state_arc = (ctx.get_app_state)();
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let policy = policy_from_app_state(
            &app_state_arc.tool_permission_context,
            &app_state_arc.settings.sandbox,
            cwd.clone(),
            false,
        );

        if let Err(err) = preflight_shell_command(&policy, &command) {
            return Ok(ToolResult {
                data: json!({
                    "error": err.to_string(),
                    "sandbox_blocked": true,
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        // Excluded-commands check: if the command matches, short-circuit the
        // sandbox wrapping (runs in the host) unless unsandboxed is forbidden.
        let is_excluded = policy.is_excluded_command(&command);
        if is_excluded && !policy.allow_unsandboxed_commands {
            return Ok(ToolResult {
                data: json!({
                    "error": crate::sandbox::SandboxError::EscapeHatchDisabled {
                        command: command.clone()
                    }
                    .to_string(),
                    "sandbox_blocked": true,
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        let mut sandbox_description = String::from("unsandboxed");
        if !escape && !is_excluded {
            if let Some(runner) = make_runner(&policy) {
                match runner.prepare(cmd, &policy, &cwd) {
                    Ok(prepared) => {
                        sandbox_description = prepared.description;
                        cmd = prepared.cmd;
                        // Piped stdio must be re-applied because the wrapper command
                        // is freshly constructed.
                        cmd.stdout(std::process::Stdio::piped());
                        cmd.stderr(std::process::Stdio::piped());
                    }
                    Err(e) => {
                        return Ok(ToolResult {
                            data: json!({
                                "error": e.to_string(),
                                "sandbox_blocked": true,
                            }),
                            new_messages: vec![],
                            ..Default::default()
                        });
                    }
                }
            }
        } else {
            // Capture stdout and stderr on the unwrapped command.
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
        }
        tracing::debug!(sandbox = %sandbox_description, "bash tool exec");

        let timeout_duration = resolve_timeout(timeout_ms);

        let result = tokio::time::timeout(timeout_duration, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                let mut combined = String::new();
                if !stdout.is_empty() {
                    combined.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&stderr);
                }

                // Truncate if too large using head+tail strategy
                let max_chars = self.max_result_size_chars();
                combined = truncate_output(&combined, max_chars);

                Ok(ToolResult {
                    data: json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": exit_code,
                        "output": combined,
                    }),
                    new_messages: vec![],
                    ..Default::default()
                })
            }
            Ok(Err(e)) => Ok(ToolResult {
                data: json!({ "error": format!("Failed to execute command: {}", e) }),
                new_messages: vec![],
                ..Default::default()
            }),
            Err(_) => Ok(ToolResult {
                data: json!({ "error": format!("Command timed out after {}ms", timeout_duration.as_millis()) }),
                new_messages: vec![],
                ..Default::default()
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Executes a given bash command and returns its output.\n\n\
The working directory persists between commands, but shell state does not. The shell environment is initialized from the user's profile (bash or zsh).\n\n\
IMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands, unless explicitly instructed or after you have verified that a dedicated tool cannot accomplish your task. Instead, use the appropriate dedicated tool as this will provide a much better experience for the user:\n\n\
 - File search: Use Glob (NOT find or ls)\n\
 - Content search: Use Grep (NOT grep or rg)\n\
 - Read files: Use Read (NOT cat/head/tail)\n\
 - Edit files: Use Edit (NOT sed/awk)\n\
 - Write files: Use Write (NOT echo >/cat <<EOF)\n\
 - Communication: Output text directly (NOT echo/printf)\n\
While the Bash tool can do similar things, it's better to use the built-in tools as they provide a better user experience and make it easier to review tool calls and give permission.\n\n\
# Instructions\n\
 - If your command will create new directories or files, first use this tool to run `ls` to verify the parent directory exists and is the correct location.\n\
 - Always quote file paths that contain spaces with double quotes in your command (e.g., cd \"path with spaces/file.txt\")\n\
 - Try to maintain your current working directory throughout the session by using absolute paths and avoiding usage of `cd`. You may use `cd` if the User explicitly requests it.\n\
 - You may specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). By default, your command will timeout after 120000ms (2 minutes).\n\
 - You can use the `run_in_background` parameter to run the command in the background. Only use this if you don't need the result immediately and are OK being notified when the command completes later. You do not need to check the output right away - you'll be notified when it finishes. You do not need to use '&' at the end of the command when using this parameter.\n\
 - When issuing multiple commands:\n\
  - If the commands are independent and can run in parallel, make multiple Bash tool calls in a single message. Example: if you need to run \"git status\" and \"git diff\", send a single message with two Bash tool calls in parallel.\n\
  - If the commands depend on each other and must run sequentially, use a single Bash call with '&&' to chain them together.\n\
  - Use ';' only when you need to run commands sequentially but don't care if earlier commands fail.\n\
  - DO NOT use newlines to separate commands (newlines are ok in quoted strings).\n\
 - For git commands:\n\
  - Prefer to create a new commit rather than amending an existing commit.\n\
  - Before running destructive operations (e.g., git reset --hard, git push --force, git checkout --), consider whether there is a safer alternative that achieves the same goal. Only use destructive operations when they are truly the best approach.\n\
  - Never skip hooks (--no-verify) or bypass signing (--no-gpg-sign, -c commit.gpgsign=false) unless the user has explicitly asked for it. If a hook fails, investigate and fix the underlying issue.\n\
 - Avoid unnecessary `sleep` commands:\n\
  - Do not sleep between commands that can run immediately — just run them.\n\
  - If your command is long running and you would like to be notified when it finishes — use `run_in_background`. No sleep needed.\n\
  - Do not retry failing commands in a sleep loop — diagnose the root cause.\n\
  - If waiting for a background task you started with `run_in_background`, you will be notified when it completes — do not poll.\n\
  - If you must poll an external process, use a check command (e.g. `gh run view`) rather than sleeping first.\n\
  - If you must sleep, keep the duration short (1-5 seconds) to avoid blocking the user.\n\n\n\
# Committing changes with git\n\n\
Only create commits when requested by the user. If unclear, ask first. When the user asks you to create a new git commit, follow these steps carefully:\n\n\
You can call multiple tools in a single response. When multiple independent pieces of information are requested and all commands are likely to succeed, run multiple tool calls in parallel for optimal performance. The numbered steps below indicate which commands should be batched in parallel.\n\n\
Git Safety Protocol:\n\
- NEVER update the git config\n\
- NEVER run destructive git commands (push --force, reset --hard, checkout ., restore ., clean -f, branch -D) unless the user explicitly requests these actions. Taking unauthorized destructive actions is unhelpful and can result in lost work, so it's best to ONLY run these commands when given direct instructions \n\
- NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless the user explicitly requests it\n\
- NEVER run force push to main/master, warn the user if they request it\n\
- CRITICAL: Always create NEW commits rather than amending, unless the user explicitly requests a git amend. When a pre-commit hook fails, the commit did NOT happen — so --amend would modify the PREVIOUS commit, which may result in destroying work or losing previous changes. Instead, after hook failure, fix the issue, re-stage, and create a NEW commit\n\
- When staging files, prefer adding specific files by name rather than using \"git add -A\" or \"git add .\", which can accidentally include sensitive files (.env, credentials) or large binaries\n\
- NEVER commit changes unless the user explicitly asks you to. It is VERY IMPORTANT to only commit when explicitly asked, otherwise the user will feel that you are being too proactive\n\n\
1. Run the following bash commands in parallel, each using the Bash tool:\n\
  - Run a git status command to see all untracked files. IMPORTANT: Never use the -uall flag as it can cause memory issues on large repos.\n\
  - Run a git diff command to see both staged and unstaged changes that will be committed.\n\
  - Run a git log command to see recent commit messages, so that you can follow this repository's commit message style.\n\
2. Analyze all staged changes (both previously staged and newly added) and draft a commit message:\n\
  - Summarize the nature of the changes (eg. new feature, enhancement to an existing feature, bug fix, refactoring, test, docs, etc.). Ensure the message accurately reflects the changes and their purpose (i.e. \"add\" means a wholly new feature, \"update\" means an enhancement to an existing feature, \"fix\" means a bug fix, etc.).\n\
  - Do not commit files that likely contain secrets (.env, credentials.json, etc). Warn the user if they specifically request to commit those files\n\
  - Draft a concise (1-2 sentences) commit message that focuses on the \"why\" rather than the \"what\"\n\
  - Ensure it accurately reflects the changes and their purpose\n\
3. Run the following commands in parallel:\n\
   - Add relevant untracked files to the staging area.\n\
   - Create the commit with a message ending with:\n\
   Co-Authored-By: Claude <noreply@anthropic.com>\n\
   - Run git status after the commit completes to verify success.\n\
   Note: git status depends on the commit completing, so run it sequentially after the commit.\n\
4. If the commit fails due to pre-commit hook: fix the issue and create a NEW commit\n\n\
Important notes:\n\
- NEVER run additional commands to read or explore code, besides git bash commands\n\
- NEVER use the TodoWrite or Agent tools\n\
- DO NOT push to the remote repository unless the user explicitly asks you to do so\n\
- IMPORTANT: Never use git commands with the -i flag (like git rebase -i or git add -i) since they require interactive input which is not supported.\n\
- IMPORTANT: Do not use --no-edit with git rebase commands, as the --no-edit flag is not a valid option for git rebase.\n\
- If there are no changes to commit (i.e., no untracked files and no modifications), do not create an empty commit\n\
- In order to ensure good formatting, ALWAYS pass the commit message via a HEREDOC, a la this example:\n\
<example>\n\
git commit -m \"$(cat <<'EOF'\n\
   Commit message here.\n\n\
   Co-Authored-By: Claude <noreply@anthropic.com>\n\
   EOF\n\
   )\"\n\
</example>\n\n\
# Creating pull requests\n\
Use the gh command via the Bash tool for ALL GitHub-related tasks including working with issues, pull requests, checks, and releases. If given a Github URL use the gh command to get the information needed.\n\n\
IMPORTANT: When the user asks you to create a pull request, follow these steps carefully:\n\n\
1. Run the following bash commands in parallel using the Bash tool, in order to understand the current state of the branch since it diverged from the main branch:\n\
   - Run a git status command to see all untracked files (never use -uall flag)\n\
   - Run a git diff command to see both staged and unstaged changes that will be committed\n\
   - Check if the current branch tracks a remote branch and is up to date with the remote, so you know if you need to push to the remote\n\
   - Run a git log command and `git diff [base-branch]...HEAD` to understand the full commit history for the current branch (from the time it diverged from the base branch)\n\
2. Analyze all changes that will be included in the pull request, making sure to look at all relevant commits (NOT just the latest commit, but ALL commits that will be included in the pull request!!!), and draft a pull request title and summary:\n\
   - Keep the PR title short (under 70 characters)\n\
   - Use the description/body for details, not the title\n\
3. Run the following commands in parallel:\n\
   - Create new branch if needed\n\
   - Push to remote with -u flag if needed\n\
   - Create PR using gh pr create with the format below. Use a HEREDOC to pass the body to ensure correct formatting.\n\
<example>\n\
gh pr create --title \"the pr title\" --body \"$(cat <<'EOF'\n\
## Summary\n\
<1-3 bullet points>\n\n\
## Test plan\n\
[Bulleted markdown checklist of TODOs for testing the pull request...]\n\n\n\
🤖 Generated with [Claude Code](https://claude.com/claude-code)\n\
EOF\n\
)\"\n\
</example>\n\n\
Important:\n\
- DO NOT use the TodoWrite or Agent tools\n\
- Return the PR URL when you're done, so the user can see it\n\n\
# Other common operations\n\
- View comments on a Github PR: gh api repos/foo/bar/pulls/123/comments".to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(name) = input
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .and_then(|cmd| extract_command_name(cmd))
        {
            format!("Bash({})", name)
        } else {
            "Bash".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_output() {
        let output = "Hello, world!\nLine 2\nLine 3\n";
        let result = truncate_output(output, 1000);
        assert_eq!(result, output);
    }

    #[test]
    fn test_truncate_head_tail() {
        // Generate 500 lines of output
        let lines: Vec<String> = (1..=500).map(|i| format!("Line {}", i)).collect();
        let output = lines.join("\n");
        // Use a generous limit so head+tail fits but full output doesn't
        let max_chars = output.len() / 2;
        let result = truncate_output(&output, max_chars);

        // Should contain the separator
        assert!(
            result.contains("lines omitted"),
            "Expected separator with 'lines omitted' in truncated output"
        );

        // Should start with the first line
        assert!(
            result.starts_with("Line 1\n"),
            "Expected output to start with 'Line 1'"
        );

        // Should end with the last line
        assert!(
            result.ends_with("Line 500"),
            "Expected output to end with 'Line 500'"
        );

        // Should be within the limit
        assert!(
            result.len() <= max_chars,
            "Truncated output ({}) exceeds max_chars ({})",
            result.len(),
            max_chars
        );
    }

    #[test]
    fn test_truncate_preserves_lines() {
        // Generate 400 lines
        let lines: Vec<String> = (1..=400).map(|i| format!("Line {:04}", i)).collect();
        let output = lines.join("\n");
        let max_chars = output.len() / 2;
        let result = truncate_output(&output, max_chars);

        // Every line in the result should be complete (not cut mid-line)
        for line in result.lines() {
            if line.contains("omitted") {
                // This is the separator line, skip it
                continue;
            }
            if line.is_empty() {
                // Empty lines around the separator
                continue;
            }
            assert!(
                line.starts_with("Line "),
                "Found a line that doesn't start with 'Line ': '{}'",
                line
            );
        }
    }
}
