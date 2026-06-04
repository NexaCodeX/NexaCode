use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct MultiEditTool;

impl MultiEditTool { pub fn new() -> Self { Self } }

#[derive(Debug, Clone)]
struct FileEdit {
    path: String,
    old_text: String,
    new_text: String,
}

fn parse_multi_edits(args: &Value) -> Result<Vec<FileEdit>, String> {
    let edits = args.get("edits").and_then(|e| e.as_array())
        .ok_or("Missing required parameter: edits (array of {path, old_text, new_text})")?;
    if edits.is_empty() { return Err("edits array cannot be empty".to_string()); }
    let mut result = Vec::new();
    for edit in edits {
        let path = edit["path"].as_str().ok_or("Each edit must have a 'path'")?.to_string();
        let old_text = edit["old_text"].as_str().ok_or("Each edit must have 'old_text'")?.to_string();
        let new_text = edit["new_text"].as_str().unwrap_or("").to_string();
        result.push(FileEdit { path, old_text, new_text });
    }
    Ok(result)
}

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str { "MultiEdit" }
    fn description(&self) -> &str {
        "Make edits across multiple files atomically. All edits must succeed or none are applied.          Provide an array of {path, old_text, new_text} edits."
    }
    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "edits": {
                        "type": "array",
                        "description": "Array of edits to apply atomically",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "File path" },
                                "old_text": { "type": "string", "description": "Text to find (must be unique)" },
                                "new_text": { "type": "string", "description": "Text to replace with" }
                            },
                            "required": ["path", "old_text", "new_text"]
                        }
                    }
                },
                "required": ["edits"]
            }),
        }
    }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Dangerous }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let edits = match parse_multi_edits(&args) {
            Ok(e) => e,
            Err(e) => return ToolResult::error(e),
        };

        // Phase 1: Read all files and validate edits
        let mut file_contents: HashMap<String, String> = HashMap::new();
        for edit in &edits {
            let file_path = context.resolve_path(&edit.path);
            if !file_path.exists() {
                return ToolResult::error(format!("File not found: {}", edit.path));
            }
            if !file_contents.contains_key(&edit.path) {
                match tokio::fs::read_to_string(&file_path).await {
                    Ok(c) => { file_contents.insert(edit.path.clone(), c); }
                    Err(e) => return ToolResult::error(format!("Failed to read '{}': {}", edit.path, e)),
                }
            }
        }

        // Phase 2: Validate all old_texts exist and are unique
        let mut modified_contents: HashMap<String, String> = file_contents.clone();
        for edit in &edits {
            let content = modified_contents.get(&edit.path).unwrap();
            let count = content.matches(&edit.old_text).count();
            if count == 0 {
                return ToolResult::error(format!("old_text not found in '{}': {:?}", edit.path, edit.old_text));
            }
            if count > 1 {
                return ToolResult::error(format!("old_text found {} times in '{}' (must be unique)", count, edit.path));
            }
            let new_content = content.replace(&edit.old_text, &edit.new_text);
            modified_contents.insert(edit.path.clone(), new_content);
        }

        // Phase 3: Write all files (atomic - all or nothing)
        let mut written = Vec::new();
        for (path, content) in &modified_contents {
            let file_path = context.resolve_path(path);

            // Backup file before edit
            if let Err(e) = super::backup::backup_file(&file_path, context).await {
                log::error!("[MultiEditTool] Backup failed for '{}': {}", path, e);
            }

            match tokio::fs::write(&file_path, content).await {
                Ok(_) => written.push(path.clone()),
                Err(e) => {
                    // Rollback: restore already-written files
                    for wp in &written {
                        let original = file_contents.get(wp).unwrap();
                        let _ = tokio::fs::write(context.resolve_path(wp), original).await;
                    }
                    return ToolResult::error(format!("Failed to write '{}': {} (rolled back {} files)", path, e, written.len()));
                }
            }
        }

        ToolResult::success(format!("Applied {} edit(s) across {} file(s)", edits.len(), modified_contents.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir) -> ToolContext { ToolContext::new(dir.path()) }

    #[tokio::test]
    async fn test_multi_edit_two_files() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("a.txt"), "hello from a").await.unwrap();
        tokio::fs::write(dir.path().join("b.txt"), "hello from b").await.unwrap();
        let tool = MultiEditTool::new();
        let result = tool.execute(serde_json::json!({
            "edits": [
                { "path": "a.txt", "old_text": "hello", "new_text": "goodbye" },
                { "path": "b.txt", "old_text": "hello", "new_text": "hi" }
            ]
        }), &ctx(&dir)).await;
        assert!(!result.is_error);
        assert_eq!(tokio::fs::read_to_string(dir.path().join("a.txt")).await.unwrap(), "goodbye from a");
        assert_eq!(tokio::fs::read_to_string(dir.path().join("b.txt")).await.unwrap(), "hi from b");
    }

    #[tokio::test]
    async fn test_multi_edit_same_file() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "alpha beta gamma").await.unwrap();
        let tool = MultiEditTool::new();
        let result = tool.execute(serde_json::json!({
            "edits": [
                { "path": "test.txt", "old_text": "alpha", "new_text": "ALPHA" },
                { "path": "test.txt", "old_text": "beta", "new_text": "BETA" }
            ]
        }), &ctx(&dir)).await;
        assert!(!result.is_error);
        let content = tokio::fs::read_to_string(dir.path().join("test.txt")).await.unwrap();
        assert_eq!(content, "ALPHA BETA gamma");
    }

    #[tokio::test]
    async fn test_multi_edit_not_unique() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "ha ha").await.unwrap();
        let tool = MultiEditTool::new();
        let result = tool.execute(serde_json::json!({
            "edits": [{ "path": "test.txt", "old_text": "ha", "new_text": "he" }]
        }), &ctx(&dir)).await;
        assert!(result.is_error);
        assert!(result.output.contains("unique"));
    }

    #[tokio::test]
    async fn test_multi_edit_missing_edits() {
        let dir = TempDir::new().unwrap();
        let tool = MultiEditTool::new();
        let result = tool.execute(serde_json::json!({}), &ctx(&dir)).await;
        assert!(result.is_error);
    }
}
