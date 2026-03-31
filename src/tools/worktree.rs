#![allow(unused)]
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// EnterWorktree — create and switch to a git worktree for isolated work
pub struct EnterWorktreeTool;

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &str { "EnterWorktree" }
    async fn description(&self, _: &Value) -> String {
        "Create a temporary git worktree for isolated changes.".to_string()
    }
    fn input_json_schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }

    async fn call(
        &self, _input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        // Create a git worktree in a temp directory
        let temp_dir = std::env::temp_dir().join(format!("cc-worktree-{}", uuid::Uuid::new_v4()));
        let branch_name = format!("cc-worktree-{}", &uuid::Uuid::new_v4().to_string()[..8]);

        let output = tokio::process::Command::new("git")
            .args(["worktree", "add", "-b", &branch_name, temp_dir.to_str().unwrap_or("")])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok(ToolResult {
                    data: json!({
                        "worktree_path": temp_dir.display().to_string(),
                        "branch": branch_name
                    }),
                    new_messages: vec![],
                })
            }
            Ok(o) => {
                bail!("Failed to create worktree: {}", String::from_utf8_lossy(&o.stderr))
            }
            Err(e) => bail!("Failed to run git: {}", e),
        }
    }

    async fn prompt(&self) -> String { "Create an isolated git worktree.".to_string() }
}

/// ExitWorktree — clean up and leave the worktree
pub struct ExitWorktreeTool;

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &str { "ExitWorktree" }
    async fn description(&self, _: &Value) -> String {
        "Leave and clean up a git worktree.".to_string()
    }
    fn input_json_schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }

    async fn call(
        &self, _input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        Ok(ToolResult {
            data: json!("Exited worktree."),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String { "Exit and clean up a git worktree.".to_string() }
}
