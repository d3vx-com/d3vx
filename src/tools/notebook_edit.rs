//! Notebook Edit Tool
//!
//! Edit cells in Jupyter notebook (.ipynb) files.
//! Supports replace, insert, and delete operations on individual cells.

use async_trait::async_trait;
use std::fs;
use std::path::Path;
use tracing::debug;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Tool for editing Jupyter notebook (.ipynb) cells
#[derive(Clone, Default)]
pub struct NotebookEditTool {
    definition: ToolDefinition,
}

impl NotebookEditTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "notebook_edit".to_string(),
                description: "Edit cells in Jupyter notebook (.ipynb) files. \
                    Supports replace, insert, and delete operations. \
                    cell_id can be a cell 'id' field or a numeric index."
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "notebook_path": { "type": "string", "description": "Absolute path to the .ipynb file" },
                        "cell_id": { "type": "string", "description": "Cell id or numeric index to target" },
                        "new_source": { "type": "string", "description": "The new cell source content" },
                        "cell_type": { "type": "string", "enum": ["code", "markdown"], "description": "Cell type (default: code)" },
                        "edit_mode": { "type": "string", "enum": ["replace", "insert", "delete"], "description": "Edit mode (default: replace)" }
                    },
                    "required": ["notebook_path", "new_source"]
                }),
            },
        }
    }

    /// Find the index of a cell by its id field or numeric index string.
    fn find_cell_index(cells: &[serde_json::Value], cell_id: &str) -> Option<usize> {
        if let Ok(idx) = cell_id.parse::<usize>() {
            if idx < cells.len() {
                return Some(idx);
            }
        }
        cells.iter().position(|cell| {
            cell.get("id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id == cell_id)
        })
    }

    /// Build a new cell JSON value from the given source and type.
    fn build_cell(new_source: &str, cell_type: &str) -> serde_json::Value {
        let is_code = cell_type == "code";
        serde_json::json!({
            "cell_type": cell_type, "source": new_source, "metadata": {},
            "outputs": if is_code { serde_json::json!([]) } else { serde_json::Value::Null },
            "execution_count": serde_json::Value::Null
        })
    }
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let notebook_path = input["notebook_path"].as_str().unwrap_or("");
        let new_source = input["new_source"].as_str().unwrap_or("");
        let cell_type = input["cell_type"].as_str().unwrap_or("code");
        let edit_mode = input["edit_mode"].as_str().unwrap_or("replace");

        if notebook_path.is_empty() {
            return ToolResult::error("notebook_path is required");
        }
        if new_source.is_empty() && edit_mode != "delete" {
            return ToolResult::error("new_source is required");
        }

        let path = if Path::new(notebook_path).is_absolute() {
            Path::new(notebook_path).to_path_buf()
        } else {
            Path::new(&context.cwd).join(notebook_path)
        };
        if path.extension().and_then(|e| e.to_str()) != Some("ipynb") {
            return ToolResult::error("File must have .ipynb extension");
        }
        if !path.exists() {
            return ToolResult::error(format!("File not found: {}", notebook_path));
        }

        let raw = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };
        let mut notebook: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => return ToolResult::error(format!("Invalid notebook JSON: {}", e)),
        };
        let cells = match notebook.get_mut("cells").and_then(|c| c.as_array_mut()) {
            Some(arr) => arr,
            None => return ToolResult::error("Notebook does not contain a valid 'cells' array"),
        };

        let resolve_index = |cell_id: &str, mode: &str| -> Result<usize, ToolResult> {
            Self::find_cell_index(cells, cell_id).ok_or_else(|| {
                ToolResult::error(format!("Cell '{}' not found for {} mode", cell_id, mode))
            })
        };

        match edit_mode {
            "delete" => {
                let cell_id = input["cell_id"].as_str().unwrap_or("");
                if cell_id.is_empty() {
                    return ToolResult::error("cell_id is required for delete mode");
                }
                let idx = match resolve_index(cell_id, "delete") {
                    Ok(i) => i,
                    Err(e) => return e,
                };
                debug!("Deleting cell at index {}", idx);
                cells.remove(idx);
            }
            "insert" => {
                let cell_id = input["cell_id"].as_str().unwrap_or("");
                if cell_id.is_empty() {
                    return ToolResult::error("cell_id is required for insert mode");
                }
                let idx = match resolve_index(cell_id, "insert") {
                    Ok(i) => i,
                    Err(e) => return e,
                };
                debug!("Inserting new cell after index {}", idx);
                cells.insert(idx + 1, Self::build_cell(new_source, cell_type));
            }
            _ => {
                let cell_id = input["cell_id"].as_str().unwrap_or("0");
                let idx = match resolve_index(cell_id, "replace") {
                    Ok(i) => i,
                    Err(e) => return e,
                };
                debug!("Replacing cell at index {}", idx);
                cells[idx] = Self::build_cell(new_source, cell_type);
            }
        }

        let output = match serde_json::to_string_pretty(&notebook) {
            Ok(s) => s,
            Err(e) => return ToolResult::error(format!("Failed to serialize notebook: {}", e)),
        };
        match fs::write(&path, &output) {
            Ok(_) => ToolResult::success(format!(
                "Successfully applied '{}' edit to {}",
                edit_mode,
                path.display()
            ))
            .with_metadata("edit_mode", serde_json::json!(edit_mode)),
            Err(e) => ToolResult::error(format!("Failed to write notebook: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn sample_notebook() -> serde_json::Value {
        serde_json::json!({
            "nbformat": 4, "nbformat_minor": 5,
            "metadata": { "kernelspec": { "display_name": "Python 3", "language": "python", "name": "python3" } },
            "cells": [
                { "cell_type": "code", "id": "abc123", "source": "print('hello')", "metadata": {}, "outputs": [], "execution_count": null },
                { "cell_type": "markdown", "id": "def456", "source": "# Title", "metadata": {} }
            ]
        })
    }

    fn write_notebook(path: &Path) {
        fs::write(
            path,
            serde_json::to_string_pretty(&sample_notebook()).unwrap(),
        )
        .unwrap();
    }

    fn make_context(cwd: &Path) -> ToolContext {
        ToolContext {
            cwd: cwd.to_string_lossy().to_string(),
            ..Default::default()
        }
    }

    fn read_cells(path: &Path) -> Vec<serde_json::Value> {
        let nb: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        nb["cells"].as_array().unwrap().clone()
    }

    #[tokio::test]
    async fn test_invalid_path_not_ipynb() {
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": "/tmp/test.txt", "new_source": "x" }),
                &ToolContext::default(),
            )
            .await;
        assert!(r.is_error);
        assert!(r.content.contains(".ipynb"));
    }

    #[tokio::test]
    async fn test_file_not_found() {
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": "/tmp/nonexistent_abc123.ipynb", "new_source": "x" }),
                &ToolContext::default(),
            )
            .await;
        assert!(r.is_error);
        assert!(r.content.contains("not found"));
    }

    #[tokio::test]
    async fn test_missing_notebook_path() {
        let r = NotebookEditTool::new()
            .execute(json!({ "new_source": "x" }), &ToolContext::default())
            .await;
        assert!(r.is_error);
        assert!(r.content.contains("required"));
    }

    #[tokio::test]
    async fn test_invalid_json_notebook() {
        let dir = std::env::temp_dir();
        let f = dir.join("d3vx_test_nb_bad.ipynb");
        fs::write(&f, "not json").unwrap();
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": f.to_string_lossy(), "new_source": "x" }),
                &make_context(&dir),
            )
            .await;
        assert!(r.is_error);
        assert!(r.content.contains("Invalid notebook JSON"));
        fs::remove_file(&f).ok();
    }

    #[tokio::test]
    async fn test_replace_cell_by_id() {
        let dir = std::env::temp_dir();
        let f = dir.join("d3vx_test_nb_replace.ipynb");
        write_notebook(&f);
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": f.to_string_lossy(), "cell_id": "abc123",
                    "new_source": "print('replaced')", "cell_type": "code", "edit_mode": "replace"
                }),
                &make_context(&dir),
            )
            .await;
        assert!(!r.is_error, "Error: {}", r.content);
        assert_eq!(read_cells(&f)[0]["source"], "print('replaced')");
        fs::remove_file(&f).ok();
    }

    #[tokio::test]
    async fn test_replace_cell_by_index() {
        let dir = std::env::temp_dir();
        let f = dir.join("d3vx_test_nb_idx.ipynb");
        write_notebook(&f);
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": f.to_string_lossy(), "cell_id": "1",
                    "new_source": "## New Heading", "cell_type": "markdown", "edit_mode": "replace"
                }),
                &make_context(&dir),
            )
            .await;
        assert!(!r.is_error, "Error: {}", r.content);
        assert_eq!(read_cells(&f)[1]["source"], "## New Heading");
        fs::remove_file(&f).ok();
    }

    #[tokio::test]
    async fn test_insert_cell() {
        let dir = std::env::temp_dir();
        let f = dir.join("d3vx_test_nb_insert.ipynb");
        write_notebook(&f);
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": f.to_string_lossy(), "cell_id": "abc123",
                    "new_source": "import os", "cell_type": "code", "edit_mode": "insert"
                }),
                &make_context(&dir),
            )
            .await;
        assert!(!r.is_error, "Error: {}", r.content);
        let cells = read_cells(&f);
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[1]["source"], "import os");
        fs::remove_file(&f).ok();
    }

    #[tokio::test]
    async fn test_delete_cell() {
        let dir = std::env::temp_dir();
        let f = dir.join("d3vx_test_nb_delete.ipynb");
        write_notebook(&f);
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": f.to_string_lossy(), "cell_id": "abc123",
                    "new_source": "", "edit_mode": "delete"
                }),
                &make_context(&dir),
            )
            .await;
        assert!(!r.is_error, "Error: {}", r.content);
        let cells = read_cells(&f);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0]["id"], "def456");
        fs::remove_file(&f).ok();
    }

    #[tokio::test]
    async fn test_cell_not_found() {
        let dir = std::env::temp_dir();
        let f = dir.join("d3vx_test_nb_nf.ipynb");
        write_notebook(&f);
        let r = NotebookEditTool::new()
            .execute(
                json!({ "notebook_path": f.to_string_lossy(), "cell_id": "nonexistent",
                    "new_source": "x", "edit_mode": "replace"
                }),
                &make_context(&dir),
            )
            .await;
        assert!(r.is_error);
        assert!(r.content.contains("not found"));
        fs::remove_file(&f).ok();
    }
}
