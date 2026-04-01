//! NotebookEdit tool — edit cells in Jupyter notebooks (.ipynb files).
//!
//! Corresponds to TypeScript: tools/NotebookEditTool/
//!
//! Supports inserting, replacing, and deleting cells in .ipynb notebooks.
//! The notebook JSON structure follows the nbformat v4 specification.

#![allow(unused)]

use std::path::Path;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::debug;

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// NotebookEdit tool — edit Jupyter notebook cells.
pub struct NotebookEditTool;

#[derive(Deserialize)]
struct NotebookEditInput {
    /// Path to the .ipynb notebook file.
    notebook_path: String,
    /// Operation to perform: "replace" (default), "insert", "delete".
    #[serde(default = "default_operation")]
    operation: String,
    /// Index of the cell to edit (0-based).
    cell_index: usize,
    /// New source content for the cell (required for replace/insert).
    #[serde(default)]
    new_source: Option<String>,
    /// Cell type for insert operations: "code" or "markdown".
    #[serde(default)]
    cell_type: Option<String>,
}

fn default_operation() -> String {
    "replace".to_string()
}

/// Create a new notebook cell with the given type and source.
fn create_cell(cell_type: &str, source: &str) -> Value {
    let source_lines: Vec<Value> = source
        .lines()
        .enumerate()
        .map(|(i, line)| {
            // Each line ends with \n except possibly the last
            let mut l = line.to_string();
            l.push('\n');
            json!(l)
        })
        .collect();

    match cell_type {
        "markdown" => json!({
            "cell_type": "markdown",
            "metadata": {},
            "source": source_lines,
        }),
        _ => json!({
            "cell_type": "code",
            "execution_count": null,
            "metadata": {},
            "outputs": [],
            "source": source_lines,
        }),
    }
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "NotebookEdit"
    }

    async fn description(&self, _input: &Value) -> String {
        "Edit cells in Jupyter notebooks (.ipynb files). Supports replace, insert, and delete operations.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "notebook_path": {
                    "type": "string",
                    "description": "The absolute path to the .ipynb notebook file"
                },
                "operation": {
                    "type": "string",
                    "enum": ["replace", "insert", "delete"],
                    "description": "Operation to perform (default: replace)"
                },
                "cell_index": {
                    "type": "number",
                    "description": "The 0-based index of the cell to edit"
                },
                "new_source": {
                    "type": "string",
                    "description": "New source content for the cell (required for replace/insert)"
                },
                "cell_type": {
                    "type": "string",
                    "enum": ["code", "markdown"],
                    "description": "Cell type for insert operations"
                }
            },
            "required": ["notebook_path", "cell_index"]
        })
    }

    fn get_path(&self, input: &Value) -> Option<String> {
        input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
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

        // Validate file exists and is .ipynb
        if !path.exists() {
            bail!("Notebook not found: {}", params.notebook_path);
        }

        if path.extension().and_then(|e| e.to_str()) != Some("ipynb") {
            bail!(
                "File is not a Jupyter notebook (.ipynb): {}",
                params.notebook_path
            );
        }

        // Read and parse the notebook
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read notebook file")?;
        let mut notebook: Value =
            serde_json::from_str(&content).context("Failed to parse notebook JSON")?;

        let cells = notebook
            .get_mut("cells")
            .and_then(|c| c.as_array_mut())
            .context("Notebook has no 'cells' array")?;

        let cell_count = cells.len();
        let result_message;

        match params.operation.as_str() {
            "replace" => {
                if params.cell_index >= cell_count {
                    bail!(
                        "Cell index {} out of range (notebook has {} cells)",
                        params.cell_index,
                        cell_count
                    );
                }

                let new_source = params
                    .new_source
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("new_source is required for replace operation"))?;

                // Update the source of the existing cell
                let cell = &mut cells[params.cell_index];
                let source_lines: Vec<Value> = new_source
                    .lines()
                    .map(|l| {
                        let mut line = l.to_string();
                        line.push('\n');
                        json!(line)
                    })
                    .collect();
                cell["source"] = json!(source_lines);

                // Optionally update cell type
                if let Some(ref ct) = params.cell_type {
                    cell["cell_type"] = json!(ct);
                }

                // Clear outputs for code cells that were modified
                if cell.get("cell_type").and_then(|v| v.as_str()) == Some("code") {
                    cell["outputs"] = json!([]);
                    cell["execution_count"] = Value::Null;
                }

                result_message = format!(
                    "Replaced cell {} in {} ({} total cells)",
                    params.cell_index, params.notebook_path, cell_count
                );
            }

            "insert" => {
                if params.cell_index > cell_count {
                    bail!(
                        "Insert index {} out of range (notebook has {} cells, max insert index is {})",
                        params.cell_index,
                        cell_count,
                        cell_count
                    );
                }

                let new_source = params
                    .new_source
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("new_source is required for insert operation"))?;

                let cell_type = params.cell_type.as_deref().unwrap_or("code");
                let new_cell = create_cell(cell_type, new_source);

                cells.insert(params.cell_index, new_cell);

                result_message = format!(
                    "Inserted {} cell at index {} in {} ({} total cells)",
                    cell_type,
                    params.cell_index,
                    params.notebook_path,
                    cell_count + 1
                );
            }

            "delete" => {
                if params.cell_index >= cell_count {
                    bail!(
                        "Cell index {} out of range (notebook has {} cells)",
                        params.cell_index,
                        cell_count
                    );
                }

                if cell_count <= 1 {
                    bail!("Cannot delete the last cell in a notebook");
                }

                let removed = cells.remove(params.cell_index);
                let removed_type = removed
                    .get("cell_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                result_message = format!(
                    "Deleted {} cell at index {} from {} ({} cells remaining)",
                    removed_type,
                    params.cell_index,
                    params.notebook_path,
                    cell_count - 1
                );
            }

            other => {
                bail!(
                    "Unknown operation '{}'. Supported: replace, insert, delete",
                    other
                );
            }
        }

        // Write back the modified notebook with consistent formatting
        let output = serde_json::to_string_pretty(&notebook)?;
        // Notebooks typically end with a newline
        let output = format!("{}\n", output);
        tokio::fs::write(path, output).await?;

        debug!(
            operation = %params.operation,
            cell_index = params.cell_index,
            path = %params.notebook_path,
            "NotebookEdit completed"
        );

        Ok(ToolResult {
            data: json!(result_message),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Edit Jupyter notebook (.ipynb) cells. Supports replacing cell content, inserting new cells, and deleting cells.\n\n\
Usage:\n\
- Use the Read tool to read the notebook first to understand its structure\n\
- Cell numbering starts at 0\n\
- When replacing a cell, provide the complete new content\n\
- Supported cell types: code, markdown, raw".to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "NotebookEdit".to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notebook_edit_name() {
        let tool = NotebookEditTool;
        assert_eq!(tool.name(), "NotebookEdit");
    }

    #[test]
    fn test_notebook_edit_schema() {
        let tool = NotebookEditTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("notebook_path"));
        assert!(props.contains_key("operation"));
        assert!(props.contains_key("cell_index"));
        assert!(props.contains_key("new_source"));
        assert!(props.contains_key("cell_type"));
    }

    #[test]
    fn test_notebook_edit_get_path() {
        let tool = NotebookEditTool;
        let input = json!({"notebook_path": "/tmp/test.ipynb"});
        assert_eq!(tool.get_path(&input), Some("/tmp/test.ipynb".to_string()));

        assert_eq!(tool.get_path(&json!({})), None);
    }

    #[test]
    fn test_create_code_cell() {
        let cell = create_cell("code", "print('hello')");
        assert_eq!(cell["cell_type"], "code");
        assert!(cell.get("outputs").is_some());
        assert!(cell.get("execution_count").is_some());
        assert!(cell["source"].is_array());
    }

    #[test]
    fn test_create_markdown_cell() {
        let cell = create_cell("markdown", "# Title\nSome text");
        assert_eq!(cell["cell_type"], "markdown");
        assert!(cell.get("outputs").is_none()); // markdown cells don't have outputs
        assert!(cell["source"].is_array());
        assert_eq!(cell["source"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_notebook_edit_replace() {
        let dir = std::env::temp_dir().join(format!("nb_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let nb_path = dir.join("test.ipynb");

        // Create a minimal notebook
        let notebook = json!({
            "nbformat": 4,
            "nbformat_minor": 5,
            "metadata": {},
            "cells": [
                {
                    "cell_type": "code",
                    "execution_count": 1,
                    "metadata": {},
                    "outputs": [{"output_type": "stream", "text": ["hello\n"]}],
                    "source": ["print('hello')\n"]
                },
                {
                    "cell_type": "markdown",
                    "metadata": {},
                    "source": ["# Title\n"]
                }
            ]
        });
        std::fs::write(&nb_path, serde_json::to_string_pretty(&notebook).unwrap()).unwrap();

        // Create a minimal context for testing
        let tool = NotebookEditTool;
        let input = json!({
            "notebook_path": nb_path.to_str().unwrap(),
            "operation": "replace",
            "cell_index": 0,
            "new_source": "print('world')"
        });

        let ctx = make_test_context();
        let parent = make_test_assistant();
        let result = tool.call(input, &ctx, &parent, None).await.unwrap();

        assert!(result.data.as_str().unwrap().contains("Replaced cell 0"));

        // Verify the file was updated
        let updated = std::fs::read_to_string(&nb_path).unwrap();
        let updated_nb: Value = serde_json::from_str(&updated).unwrap();
        let source = &updated_nb["cells"][0]["source"];
        assert!(source.to_string().contains("world"));
        // Outputs should be cleared
        assert_eq!(updated_nb["cells"][0]["outputs"], json!([]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_notebook_edit_insert() {
        let dir = std::env::temp_dir().join(format!("nb_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let nb_path = dir.join("test.ipynb");

        let notebook = json!({
            "nbformat": 4,
            "nbformat_minor": 5,
            "metadata": {},
            "cells": [
                {"cell_type": "code", "execution_count": null, "metadata": {}, "outputs": [], "source": ["x = 1\n"]}
            ]
        });
        std::fs::write(&nb_path, serde_json::to_string_pretty(&notebook).unwrap()).unwrap();

        let tool = NotebookEditTool;
        let input = json!({
            "notebook_path": nb_path.to_str().unwrap(),
            "operation": "insert",
            "cell_index": 1,
            "new_source": "# New cell",
            "cell_type": "markdown"
        });

        let ctx = make_test_context();
        let parent = make_test_assistant();
        let result = tool.call(input, &ctx, &parent, None).await.unwrap();

        assert!(result.data.as_str().unwrap().contains("Inserted markdown cell"));

        let updated = std::fs::read_to_string(&nb_path).unwrap();
        let updated_nb: Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(updated_nb["cells"].as_array().unwrap().len(), 2);
        assert_eq!(updated_nb["cells"][1]["cell_type"], "markdown");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_notebook_edit_delete() {
        let dir = std::env::temp_dir().join(format!("nb_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let nb_path = dir.join("test.ipynb");

        let notebook = json!({
            "nbformat": 4,
            "nbformat_minor": 5,
            "metadata": {},
            "cells": [
                {"cell_type": "code", "execution_count": null, "metadata": {}, "outputs": [], "source": ["x = 1\n"]},
                {"cell_type": "code", "execution_count": null, "metadata": {}, "outputs": [], "source": ["x = 2\n"]}
            ]
        });
        std::fs::write(&nb_path, serde_json::to_string_pretty(&notebook).unwrap()).unwrap();

        let tool = NotebookEditTool;
        let input = json!({
            "notebook_path": nb_path.to_str().unwrap(),
            "operation": "delete",
            "cell_index": 0
        });

        let ctx = make_test_context();
        let parent = make_test_assistant();
        let result = tool.call(input, &ctx, &parent, None).await.unwrap();

        assert!(result.data.as_str().unwrap().contains("Deleted"));

        let updated = std::fs::read_to_string(&nb_path).unwrap();
        let updated_nb: Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(updated_nb["cells"].as_array().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Create a minimal ToolUseContext for testing.
    fn make_test_context() -> ToolUseContext {
        use crate::types::app_state::AppState;
        use std::sync::Arc;

        let state = AppState::default();
        let state_clone = state.clone();

        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test-model".to_string(),
                verbose: false,
                is_non_interactive_session: true,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: tokio::sync::watch::channel(false).1,
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(move || state_clone.clone()),
            set_app_state: Arc::new(|_| {}),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
        }
    }

    fn make_test_assistant() -> AssistantMessage {
        use crate::types::message::ContentBlock;
        AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: "test".into(),
            }],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        }
    }
}
