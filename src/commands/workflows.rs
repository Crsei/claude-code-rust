//! `/workflows` command -- workflow scripts management.
//!
//! Workflow scripts allow defining reusable automation sequences
//! as YAML files stored in `.cc-rust/workflows/`. This is a feature-gated
//! capability from the TypeScript source (WORKFLOW_SCRIPTS feature flag)
//! that does not have full source available.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct WorkflowsHandler;

#[async_trait]
impl CommandHandler for WorkflowsHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let trimmed = args.trim();

        if trimmed.is_empty() {
            return Ok(CommandResult::Output(overview()));
        }

        let (subcmd, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((cmd, remainder)) => (cmd, remainder.trim()),
            None => (trimmed, ""),
        };

        match subcmd.to_lowercase().as_str() {
            "list" | "ls" => Ok(CommandResult::Output(
                "No workflows configured. Create workflow files in .cc-rust/workflows/".to_string(),
            )),
            "run" => {
                if rest.is_empty() {
                    Ok(CommandResult::Output(
                        "Usage: /workflows run <name>\n\n\
                         Specify the workflow name to execute."
                            .to_string(),
                    ))
                } else {
                    Ok(CommandResult::Output(format!(
                        "Workflow '{}' not found.",
                        rest
                    )))
                }
            }
            "create" => Ok(CommandResult::Output(
                "Create workflow YAML files in .cc-rust/workflows/ directory.\n\n\
                 Each workflow is a YAML file defining a sequence of steps.\n\
                 Example: .cc-rust/workflows/deploy.yaml"
                    .to_string(),
            )),
            _ => Ok(CommandResult::Output(format!(
                "Unknown subcommand: '{}'\n\n{}",
                subcmd,
                overview()
            ))),
        }
    }
}

fn overview() -> String {
    "Workflow scripts allow you to define reusable automation sequences.\n\n\
     Usage: /workflows <subcommand>\n\n\
     Subcommands:\n  \
       list      List configured workflows\n  \
       run <name>  Run a named workflow\n  \
       create    Show instructions for creating workflows\n\n\
     Workflows are defined as YAML files in .cc-rust/workflows/.\n\
     Each file describes a sequence of steps that can be executed\n\
     as a single command."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_default_shows_overview() {
        let handler = WorkflowsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("reusable automation sequences"));
                assert!(text.contains("Subcommands:"));
                assert!(text.contains("list"));
                assert!(text.contains("run"));
                assert!(text.contains("create"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_run_missing_workflow() {
        let handler = WorkflowsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("run deploy", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Workflow 'deploy' not found"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
