//! Post-tool, failure, stop, and event hook execution.

use anyhow::Result;
use serde_json::Value;
use tracing::{debug, warn};

use super::execution::execute_command_hook;
use super::{
    load_hook_configs, matches_tool, HookEntry, HookEventConfig, HookOutput, PostToolHookResult,
};
use crate::types::tool::ToolResult;

// ---------------------------------------------------------------------------
// Post-tool hooks
// ---------------------------------------------------------------------------

/// Run post-tool hooks after successful tool execution.
///
/// Corresponds to TypeScript: `runPostToolUseHooks()` in toolExecution.ts
///
/// Stdin includes `tool_result` field in addition to tool_name and tool_input.
/// If any hook returns `stop_reason`, returns `StopContinuation`.
pub async fn run_post_tool_hooks(
    tool_name: &str,
    input: &Value,
    result: &ToolResult,
    hook_configs: &[HookEventConfig],
) -> Result<PostToolHookResult> {
    if hook_configs.is_empty() {
        debug!(tool = tool_name, "post-tool hooks: no hooks configured");
        return Ok(PostToolHookResult::Continue);
    }

    let stdin_json = serde_json::json!({
        "tool_name": tool_name,
        "tool_input": input,
        "tool_result": result.data,
    });

    for config in hook_configs {
        if !matches_tool(config.matcher.as_deref(), tool_name) {
            continue;
        }

        for entry in &config.hooks {
            let HookEntry::Command { command, timeout } = entry;

            debug!(
                tool = tool_name,
                command = command,
                "running post-tool hook"
            );

            match execute_command_hook(command, &stdin_json, *timeout).await {
                Ok(output) => {
                    // Check for stop
                    if !output.should_continue {
                        let message = output
                            .stop_reason
                            .or(output.reason)
                            .unwrap_or_else(|| "Hook stopped continuation".to_string());
                        return Ok(PostToolHookResult::StopContinuation { message });
                    }

                    if let Some(ref stop_reason) = output.stop_reason {
                        return Ok(PostToolHookResult::StopContinuation {
                            message: stop_reason.clone(),
                        });
                    }
                }
                Err(e) => {
                    warn!(
                        tool = tool_name,
                        command = command,
                        error = %e,
                        "post-tool hook error, continuing"
                    );
                }
            }
        }
    }

    Ok(PostToolHookResult::Continue)
}

/// Run post-tool failure hooks after failed tool execution.
///
/// Corresponds to TypeScript: `runPostToolUseFailureHooks()` in toolExecution.ts
///
/// Fire-and-forget: run hooks but don't change behavior based on output.
pub async fn run_post_tool_failure_hooks(
    tool_name: &str,
    input: &Value,
    error: &str,
    hook_configs: &[HookEventConfig],
) -> Result<()> {
    if hook_configs.is_empty() {
        debug!(
            tool = tool_name,
            "post-tool failure hooks: no hooks configured"
        );
        return Ok(());
    }

    let stdin_json = serde_json::json!({
        "tool_name": tool_name,
        "tool_input": input,
        "error": error,
    });

    for config in hook_configs {
        if !matches_tool(config.matcher.as_deref(), tool_name) {
            continue;
        }

        for entry in &config.hooks {
            let HookEntry::Command { command, timeout } = entry;

            debug!(
                tool = tool_name,
                command = command,
                "running post-tool failure hook"
            );

            if let Err(e) = execute_command_hook(command, &stdin_json, *timeout).await {
                warn!(
                    tool = tool_name,
                    command = command,
                    error = %e,
                    "post-tool failure hook error"
                );
            }
        }
    }

    Ok(())
}

/// Run stop hooks (when the model stops generating).
///
/// Corresponds to TypeScript: `executeStopHooks()`
pub async fn run_stop_hooks(hook_configs: &[HookEventConfig]) -> Result<PostToolHookResult> {
    if hook_configs.is_empty() {
        return Ok(PostToolHookResult::Continue);
    }

    let stdin_json = serde_json::json!({
        "event": "Stop",
    });

    for config in hook_configs {
        // Stop hooks don't have a tool name to match against, so we only
        // run configs with matcher None or "*"
        if config.matcher.is_some() && config.matcher.as_deref() != Some("*") {
            continue;
        }

        for entry in &config.hooks {
            let HookEntry::Command { command, timeout } = entry;

            debug!(command = command, "running stop hook");

            match execute_command_hook(command, &stdin_json, *timeout).await {
                Ok(output) => {
                    if !output.should_continue {
                        let message = output
                            .stop_reason
                            .or(output.reason)
                            .unwrap_or_else(|| "Stop hook halted continuation".to_string());
                        return Ok(PostToolHookResult::StopContinuation { message });
                    }

                    if let Some(ref stop_reason) = output.stop_reason {
                        return Ok(PostToolHookResult::StopContinuation {
                            message: stop_reason.clone(),
                        });
                    }
                }
                Err(e) => {
                    warn!(command = command, error = %e, "stop hook error, continuing");
                }
            }
        }
    }

    Ok(PostToolHookResult::Continue)
}

// ---------------------------------------------------------------------------
// Generic event hook dispatcher
// ---------------------------------------------------------------------------

/// Generic hook event runner for non-tool lifecycle events.
///
/// Fires all matching hooks for the given event. Returns the merged HookOutput.
/// Errors from individual hooks are logged and skipped (fire-and-forget).
///
/// Non-tool events only match configs with `None` or `"*"` matcher (tool-specific
/// matchers like `"Bash"` are skipped).
pub async fn run_event_hooks(
    event_name: &str,
    payload: &Value,
    hook_configs: &[HookEventConfig],
) -> Result<HookOutput> {
    if hook_configs.is_empty() {
        debug!(event = event_name, "event hooks: no hooks configured");
        return Ok(HookOutput::default());
    }

    let mut last_output = HookOutput::default();

    for config in hook_configs {
        // Non-tool events: only match configs with None or "*" matcher
        if config.matcher.is_some() && config.matcher.as_deref() != Some("*") {
            continue;
        }

        for entry in &config.hooks {
            let HookEntry::Command { command, timeout } = entry;

            debug!(event = event_name, command = command, "running event hook");

            match execute_command_hook(command, payload, *timeout).await {
                Ok(output) => {
                    last_output = output;
                }
                Err(e) => {
                    warn!(event = event_name, command = command, error = %e, "event hook error");
                }
            }
        }
    }

    Ok(last_output)
}

// ---------------------------------------------------------------------------
// Convenience wrappers for non-tool lifecycle hooks
// ---------------------------------------------------------------------------

/// Fire a notification hook (convenience wrapper).
///
/// Sends `{ "title": ..., "body": ... }` to all hooks registered under the
/// `"Notification"` event key.  Fire-and-forget: errors are logged internally.
pub async fn fire_notification_hook(
    title: &str,
    body: &str,
    hooks_map: &std::collections::HashMap<String, serde_json::Value>,
) {
    let configs = load_hook_configs(hooks_map, "Notification");
    if configs.is_empty() {
        return;
    }
    let payload = serde_json::json!({
        "title": title,
        "body": body,
    });
    let _ = run_event_hooks("Notification", &payload, &configs).await;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: make a HookEventConfig with a single command that matches all tools.
    #[cfg(not(windows))]
    fn make_hook_config(command: &str) -> HookEventConfig {
        HookEventConfig {
            matcher: Some("*".to_string()),
            hooks: vec![HookEntry::Command {
                command: command.to_string(),
                timeout: 10,
            }],
        }
    }

    #[tokio::test]
    async fn test_run_post_tool_hooks_empty() {
        let tool_result = ToolResult {
            data: Value::String("ok".into()),
            new_messages: vec![],
            ..Default::default()
        };
        let result = run_post_tool_hooks("Bash", &json!({}), &tool_result, &[])
            .await
            .unwrap();
        assert!(matches!(result, PostToolHookResult::Continue));
    }

    #[tokio::test]
    async fn test_run_post_tool_failure_hooks_empty() {
        let result = run_post_tool_failure_hooks("Bash", &json!({}), "test error", &[]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_stop_hooks_empty() {
        let result = run_stop_hooks(&[]).await.unwrap();
        assert!(matches!(result, PostToolHookResult::Continue));
    }

    #[tokio::test]
    async fn test_run_event_hooks_empty() {
        let result = run_event_hooks("SessionStart", &serde_json::json!({}), &[]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.should_continue);
    }

    #[tokio::test]
    async fn test_run_event_hooks_skips_tool_specific_matchers() {
        // A config with matcher "Bash" should be skipped for non-tool events
        let configs = vec![HookEventConfig {
            matcher: Some("Bash".to_string()),
            hooks: vec![HookEntry::Command {
                command: r#"echo '{"continue":false,"reason":"should not fire"}'"#.to_string(),
                timeout: 10,
            }],
        }];

        let result = run_event_hooks("SessionStart", &json!({}), &configs).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should still be the default (continue=true) because the matcher was skipped
        assert!(output.should_continue);
    }

    // =========================================================================
    // Subprocess integration tests
    // =========================================================================

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_post_tool_hook_stops_continuation() {
        let configs = vec![make_hook_config(
            r#"echo '{"continue":false,"stop_reason":"audit complete, halt"}'"#,
        )];
        let tool_result = ToolResult {
            data: json!("file contents here"),
            new_messages: vec![],
            ..Default::default()
        };

        let result = run_post_tool_hooks(
            "Read",
            &json!({"file_path": "/etc/passwd"}),
            &tool_result,
            &configs,
        )
        .await
        .unwrap();

        match result {
            PostToolHookResult::StopContinuation { message } => {
                assert!(message.contains("audit complete"));
            }
            PostToolHookResult::Continue => {
                panic!("expected StopContinuation, got Continue");
            }
        }
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_post_tool_hook_continues() {
        let configs = vec![make_hook_config(r#"echo '{"continue":true}'"#)];
        let tool_result = ToolResult {
            data: json!("ok"),
            new_messages: vec![],
            ..Default::default()
        };

        let result = run_post_tool_hooks("Bash", &json!({"command": "ls"}), &tool_result, &configs)
            .await
            .unwrap();

        assert!(matches!(result, PostToolHookResult::Continue));
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_stop_hook_prevents_stop() {
        let configs = vec![HookEventConfig {
            matcher: None,
            hooks: vec![HookEntry::Command {
                command: r#"echo '{"continue":false,"stop_reason":"not done yet"}'"#.to_string(),
                timeout: 10,
            }],
        }];

        let result = run_stop_hooks(&configs).await.unwrap();

        match result {
            PostToolHookResult::StopContinuation { message } => {
                assert!(message.contains("not done yet"));
            }
            PostToolHookResult::Continue => {
                panic!("expected StopContinuation, got Continue");
            }
        }
    }
}
