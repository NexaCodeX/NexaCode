use async_trait::async_trait;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct EditTool;

impl EditTool { pub fn new() -> Self { Self } }

#[derive(Debug)]
enum EditOperation {
    Replace { old_text: String, new_text: String },
    #[allow(dead_code)]
    InsertAfter { text: String, new_text: String },
    #[allow(dead_code)]
    DeleteLines { start: usize, end: usize },
}

fn parse_edit_args(args: &Value) -> Result<(String, Vec<EditOperation>), String> {
    let path = args["path"].as_str().ok_or("Missing required parameter: path")?.to_string();
    let mut ops = Vec::new();

    // SEARCH/REPLACE mode
    if let Some(old) = args["old_text"].as_str() {
        let new = args["new_text"].as_str().unwrap_or("");
        ops.push(EditOperation::Replace { old_text: old.to_string(), new_text: new.to_string() });
    } else if let Some(edits) = args.get("edits").and_then(|e| e.as_array()) {
        for edit in edits {
            if let Some(old) = edit["old_text"].as_str() {
                let new = edit["new_text"].as_str().unwrap_or("");
                ops.push(EditOperation::Replace { old_text: old.to_string(), new_text: new.to_string() });
            }
        }
    }

    if ops.is_empty() { return Err("No edit operations provided. Use old_text/new_text or edits array.".to_string()); }
    Ok((path, ops))
}

fn apply_edits(content: &str, ops: &[EditOperation]) -> Result<String, String> {
    let mut result = content.to_string();
    for op in ops {
        match op {
            EditOperation::Replace { old_text, new_text } => {
                let count = result.matches(old_text).count();
                if count == 0 { return Err(format!("old_text not found in file: {:?}", old_text)); }
                if count > 1 { return Err(format!("old_text found {} times, must be unique: {:?}", count, old_text)); }
                result = result.replace(old_text, new_text);
            }
            _ => return Err("Unsupported edit operation".to_string()),
        }
    }
    Ok(result)
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str { "Edit" }
    fn description(&self) -> &str {
        "Make precise edits to a file using SEARCH/REPLACE. Provide old_text to find and new_text to replace with. \
         The old_text must match exactly and be unique in the file. You must Read the file first before editing."
    }
    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the file to edit" },
                    "old_text": { "type": "string", "description": "The exact text to find (must be unique in the file)" },
                    "new_text": { "type": "string", "description": "The text to replace with" },
                    "edits": { "type": "array", "description": "Array of {old_text, new_text} for multiple edits in one file",
                        "items": { "type": "object", "properties": {
                            "old_text": { "type": "string" }, "new_text": { "type": "string" }
                        }, "required": ["old_text", "new_text"]
                    }}
                },
                "required": ["path"]
            }),
        }
    }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Moderate }

    fn validate(&self, args: &Value) -> Result<(), String> {
        let has_old = args["old_text"].as_str().is_some();
        let has_edits = args.get("edits").and_then(|e| e.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
        if !has_old && !has_edits { return Err("Must provide either old_text/new_text or edits array".to_string()); }
        Ok(())
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let (path, ops) = match parse_edit_args(&args) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(e),
        };
        let file_path = context.resolve_path(&path);
        if !file_path.exists() { return ToolResult::error(format!("File not found: {}", path)); }

        let content = match tokio::fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        let new_content = match apply_edits(&content, &ops) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        if let Err(e) = tokio::fs::write(&file_path, &new_content).await {
            return ToolResult::error(format!("Failed to write file: {}", e));
        }

        ToolResult::success(format!("Edited {} ({} operation(s))", path, ops.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir) -> ToolContext { ToolContext::new(dir.path()) }

    #[tokio::test]
    async fn test_edit_replace() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "hello world").await.unwrap();
        let tool = EditTool::new();
        let result = tool.execute(serde_json::json!({
            "path": "test.txt", "old_text": "hello", "new_text": "goodbye"
        }), &ctx(&dir)).await;
        assert!(!result.is_error);
        let content = tokio::fs::read_to_string(dir.path().join("test.txt")).await.unwrap();
        assert_eq!(content, "goodbye world");
    }

    #[tokio::test]
    async fn test_edit_not_found() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "hello world").await.unwrap();
        let tool = EditTool::new();
        let result = tool.execute(serde_json::json!({
            "path": "test.txt", "old_text": "not_here", "new_text": "replacement"
        }), &ctx(&dir)).await;
        assert!(result.is_error);
        assert!(result.output.contains("not found"));
    }

    #[tokio::test]
    async fn test_edit_multiple_matches() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "ha ha ha").await.unwrap();
        let tool = EditTool::new();
        let result = tool.execute(serde_json::json!({
            "path": "test.txt", "old_text": "ha", "new_text": "he"
        }), &ctx(&dir)).await;
        assert!(result.is_error);
        assert!(result.output.contains("3 times"));
    }

    #[tokio::test]
    async fn test_edit_file_not_found() {
        let dir = TempDir::new().unwrap();
        let tool = EditTool::new();
        let result = tool.execute(serde_json::json!({
            "path": "missing.txt", "old_text": "x", "new_text": "y"
        }), &ctx(&dir)).await;
        assert!(result.is_error);
    }

    #[test]
    fn test_validate_no_edits() {
        let tool = EditTool::new();
        assert!(tool.validate(&serde_json::json!({ "path": "test.txt" })).is_err());
    }

    #[test]
    fn test_apply_edits_simple() {
        let result = apply_edits("hello world", &[EditOperation::Replace {
            old_text: "hello".to_string(), new_text: "goodbye".to_string()
        }]).unwrap();
        assert_eq!(result, "goodbye world");
    }
}
