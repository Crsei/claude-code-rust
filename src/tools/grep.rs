#![allow(unused)]
use std::path::Path;
use anyhow::{Context, Result};
use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct GrepTool;

#[derive(Deserialize)]
struct GrepInput {
    pattern: String,
    path: Option<String>,
    glob: Option<String>,
    #[serde(rename = "type")]
    file_type: Option<String>,
    output_mode: Option<String>,
    #[serde(rename = "-C")]
    context: Option<usize>,
    #[serde(rename = "-A")]
    after_context: Option<usize>,
    #[serde(rename = "-B")]
    before_context: Option<usize>,
    #[serde(rename = "-i")]
    case_insensitive: Option<bool>,
    #[serde(rename = "-n")]
    line_numbers: Option<bool>,
    head_limit: Option<usize>,
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str { "Grep" }

    async fn description(&self, _input: &Value) -> String {
        "Search file contents with regex patterns.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern to search for" },
                "path": { "type": "string", "description": "File or directory to search" },
                "glob": { "type": "string", "description": "Glob pattern to filter files" },
                "type": { "type": "string", "description": "File type (js, py, rs, etc.)" },
                "output_mode": { "type": "string", "enum": ["content", "files_with_matches", "count"] },
                "-C": { "type": "number", "description": "Context lines before and after" },
                "-A": { "type": "number", "description": "Lines after each match" },
                "-B": { "type": "number", "description": "Lines before each match" },
                "-i": { "type": "boolean", "description": "Case insensitive" },
                "-n": { "type": "boolean", "description": "Show line numbers" },
                "head_limit": { "type": "number", "description": "Limit output entries" }
            },
            "required": ["pattern"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { true }
    fn is_read_only(&self, _input: &Value) -> bool { true }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: GrepInput = serde_json::from_value(input)?;
        let search_path = params.path.unwrap_or_else(|| ".".to_string());
        let output_mode = params.output_mode.as_deref().unwrap_or("files_with_matches");
        let head_limit = params.head_limit.unwrap_or(250);
        let case_insensitive = params.case_insensitive.unwrap_or(false);
        let context_lines = params.context.or(params.after_context).unwrap_or(0);

        let pattern_str = if case_insensitive {
            format!("(?i){}", params.pattern)
        } else {
            params.pattern.clone()
        };
        let re = Regex::new(&pattern_str).context("Invalid regex pattern")?;

        let mut walker = WalkBuilder::new(&search_path);
        walker.hidden(false).git_ignore(true);

        if let Some(ref glob_pat) = params.glob {
            let mut types_builder = ignore::types::TypesBuilder::new();
            let glob_ext = glob_pat.trim_start_matches("*.");
            // Ignore errors from add() — if the pattern is invalid, we'll fall through
            // to the manual glob matching below.
            let _ = types_builder.add("custom", &format!("*.{}", glob_ext));
            types_builder.select("custom");
            if let Ok(types) = types_builder.build() {
                walker.types(types);
            }
        }

        let mut results: Vec<String> = Vec::new();
        let mut file_count = 0;
        let mut match_count = 0;

        for entry in walker.build().flatten() {
            if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                continue;
            }
            let path = entry.path();

            if let Some(ref glob_pat) = params.glob {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !glob::Pattern::new(glob_pat).map_or(true, |p| p.matches(name)) {
                    continue;
                }
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue, // skip binary/unreadable
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut file_matches = Vec::new();

            for (i, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    file_matches.push((i + 1, *line));
                    match_count += 1;
                }
            }

            if !file_matches.is_empty() {
                file_count += 1;
                let path_str = path.display().to_string();

                match output_mode {
                    "files_with_matches" => {
                        results.push(path_str);
                    }
                    "count" => {
                        results.push(format!("{}:{}", path_str, file_matches.len()));
                    }
                    "content" | _ => {
                        for (line_num, line_content) in &file_matches {
                            let show_num = params.line_numbers.unwrap_or(true);
                            if show_num {
                                results.push(format!("{}:{}:{}", path_str, line_num, line_content));
                            } else {
                                results.push(format!("{}:{}", path_str, line_content));
                            }
                        }
                    }
                }
            }

            if head_limit > 0 && results.len() >= head_limit {
                results.truncate(head_limit);
                break;
            }
        }

        let output = if results.is_empty() {
            "No matches found.".to_string()
        } else {
            results.join("\n")
        };

        Ok(ToolResult {
            data: json!(output),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use Grep to search file contents with regex patterns.".to_string()
    }

    fn to_auto_classifier_input(&self, input: &Value) -> Value {
        let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        json!(format!("grep {}", pattern))
    }
}
