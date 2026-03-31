#![allow(unused)]
use std::path::Path;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct NotebookEditTool;

#[derive(Deserialize)]
struct NotebookEditInput {
    notebook_path: String,
    cell_index: usize,
    new_source: String,
    #[serde(default)]
    cell_type: Option<String>,
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str { "NotebookEdit" }

    async fn description(&self, _input: &Value) -> String {
        "Edit cells in Jupyter notebooks (.ipynb files).".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "notebook_path": { "type": "string" },
                "cell_index": { "type": "number" },
                "new_source": { "type": "string" },
                "cell_type": { "type": "string", "enum": ["code", "markdown"] }
            },
            "required": ["notebook_path", "cell_index", "new_source"]
        })
    }

    fn get_path(&self, input: &Value) -> Option<String> {
        input.get("notebook_path").and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: NotebookEditInput = serde_json::from_value(input)?;
        let path = Path::new(&params.notebook_path);

        if !path.exists() {
            bail!("Notebook not found: {}", params.notebook_path);
        }

        let content = tokio::fs::read_to_string(path).await?;
        let mut notebook: Value = serde_json::from_str(&content)
            .context("Failed to parse notebook JSON")?;

        let cells = notebook
            .get_mut("cells")
            .and_then(|c| c.as_array_mut())
            .context("Notebook has no cells array")?;

        if params.cell_index >= cells.len() {
            bail!("Cell index {} out of range (notebook has {} cells)", params.cell_index, cells.len());
        }

        let cell = &mut cells[params.cell_index];
        let source_lines: Vec<String> = params.new_source
            .lines()
            .map(|l| format!("{}\n", l))
            .collect();
        cell["source"] = json!(source_lines);

        if let Some(ref ct) = params.cell_type {
            cell["cell_type"] = json!(ct);
        }

        let output = serde_json::to_string_pretty(&notebook)?;
        tokio::fs::write(path, output).await?;

        Ok(ToolResult {
            data: json!(format!("Edited cell {} in {}", params.cell_index, params.notebook_path)),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use NotebookEdit to modify Jupyter notebook cells.".to_string()
    }
}
