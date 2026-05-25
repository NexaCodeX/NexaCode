use async_trait::async_trait;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

/// Write/Create file tool
///
/// Creates a new file or completely overwrites an existing file with the given content.
/// For precise edits, use the Edit tool instead.
pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, \
         overwrites it completely if it does. For editing specific parts of a file, \
         use the Edit tool instead. Will create parent directories as needed."
    }

    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to write (relative to working directory or absolute)"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Moderate
    }

    fn validate(&self, args: &Value) -> Result<(), String> {
        if args["path"].as_str().is_none_or(|s| s.is_empty()) {
            return Err("Missing required parameter: path".to_string());
        }
        if args["content"].as_str().is_none() {
            return Err("Missing required parameter: content".to_string());
        }
        Ok(())
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let path = args["path"].as_str().unwrap();
        let content = args["content"].as_str().unwrap();
        let file_path = context.resolve_path(path);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult::error(format!(
                        "Failed to create parent directories: {}",
                        e
                    ));
                }
            }
        }

        // Check if file already exists
        let exists = file_path.exists();

        // Write the file
        if let Err(e) = tokio::fs::write(&file_path, content).await {
            return ToolResult::error(format!("Failed to write file '{}': {}", path, e));
        }

        let line_count = content.lines().count();
        let byte_count = content.len();

        let mut result = ToolResult::success(format!(
            "File '{}' {} ({} lines, {} bytes)",
            path,
            if exists { "overwritten" } else { "created" },
            line_count,
            byte_count
        ));

        result = result.with_metadata("exists", serde_json::json!(exists));
        result = result.with_metadata("lines", serde_json::json!(line_count));
        result = result.with_metadata("bytes", serde_json::json!(byte_count));

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_context(dir: &TempDir) -> ToolContext {
        ToolContext::new(dir.path())
    }

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = WriteTool::new();

        let result = tool
            .execute(
                serde_json::json!({ "path": "hello.txt", "content": "Hello, World!" }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("created"));

        // Verify the file was created
        let content = tokio::fs::read_to_string(dir.path().join("hello.txt"))
            .await
            .unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[tokio::test]
    async fn test_write_overwrite_existing() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = WriteTool::new();

        // Create initial file
        tokio::fs::write(dir.path().join("existing.txt"), "old content")
            .await
            .unwrap();

        let result = tool
            .execute(
                serde_json::json!({ "path": "existing.txt", "content": "new content" }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("overwritten"));

        let content = tokio::fs::read_to_string(dir.path().join("existing.txt"))
            .await
            .unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_write_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = WriteTool::new();

        let result = tool
            .execute(
                serde_json::json!({ "path": "a/b/c/deep.txt", "content": "deep content" }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);

        let content = tokio::fs::read_to_string(dir.path().join("a/b/c/deep.txt"))
            .await
            .unwrap();
        assert_eq!(content, "deep content");
    }

    #[tokio::test]
    async fn test_write_multiline_content() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = WriteTool::new();

        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let result = tool
            .execute(
                serde_json::json!({ "path": "main.rs", "content": content }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("3 lines"));
    }

    #[tokio::test]
    async fn test_write_empty_content() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = WriteTool::new();

        let result = tool
            .execute(
                serde_json::json!({ "path": "empty.txt", "content": "" }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
    }

    #[test]
    fn test_validate_missing_path() {
        let tool = WriteTool::new();
        assert!(tool
            .validate(&serde_json::json!({ "content": "hi" }))
            .is_err());
    }

    #[test]
    fn test_validate_missing_content() {
        let tool = WriteTool::new();
        assert!(tool
            .validate(&serde_json::json!({ "path": "test.txt" }))
            .is_err());
    }

    #[test]
    fn test_validate_ok() {
        let tool = WriteTool::new();
        assert!(tool
            .validate(&serde_json::json!({ "path": "test.txt", "content": "hi" }))
            .is_ok());
    }
}
