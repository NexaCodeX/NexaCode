use async_trait::async_trait;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

/// Read file contents tool
///
/// Reads the contents of a file, optionally with line range.
/// Supports text files; images return a base64 preview placeholder.
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Supports line range selection with offset and limit parameters. \
         Returns file content with line numbers. Use this before editing files to understand their current state."
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
                        "description": "The path to the file to read (relative to working directory or absolute)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (1-based, default: 1)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read (default: all lines)"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let path = args["path"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if path.is_empty() {
            return ToolResult::error("Missing required parameter: path");
        }

        let file_path = context.resolve_path(&path);

        // Check if file exists
        if !file_path.exists() {
            return ToolResult::error(format!("File not found: {}", path));
        }

        // Check if it's a directory
        if file_path.is_dir() {
            return ToolResult::error(format!(
                "'{}' is a directory, not a file. Use LS to list directory contents.",
                path
            ));
        }

        // Check for image files
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg") {
            return ToolResult::success(format!(
                "[Image file: {}] Image reading via vision API is not yet supported. \
                 File size: {} bytes",
                path,
                file_path.metadata().map(|m| m.len()).unwrap_or(0)
            ));
        }

        // Read file content
        let content = match tokio::fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => {
                // Try reading as binary
                match tokio::fs::read(&file_path).await {
                    Ok(bytes) => {
                        return ToolResult::success(format!(
                            "[Binary file: {}] {} bytes. Cannot display as text.",
                            path,
                            bytes.len()
                        ));
                    }
                    Err(_) => {
                        return ToolResult::error(format!("Failed to read file '{}': {}", path, e));
                    }
                }
            }
        };

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Apply offset and limit
        let offset = args["offset"].as_u64().unwrap_or(0) as usize;
        let offset = if offset == 0 { 0 } else { offset - 1 }; // Convert 1-based to 0-based
        let limit = args["limit"].as_u64().map(|l| l as usize);

        let selected_lines: Vec<String> = lines
            .iter()
            .enumerate()
            .skip(offset)
            .take(limit.unwrap_or(usize::MAX))
            .map(|(i, line)| format!("{:>6}\t{}", i + 1, line))
            .collect();

        if selected_lines.is_empty() {
            return ToolResult::success(format!(
                "File '{}' has {} lines. Offset {} is beyond end of file.",
                path, total_lines, offset + 1
            ));
        }

        let mut output = String::new();

        // Add header info
        if offset > 0 || limit.is_some() {
            output.push_str(&format!(
                "File: {} ({} lines total, showing lines {}-{})\n",
                path,
                total_lines,
                offset + 1,
                offset + selected_lines.len()
            ));
        } else {
            output.push_str(&format!("File: {} ({} lines)\n", path, total_lines));
        }

        output.push_str(&selected_lines.join("\n"));

        // Apply truncation
        let (output, truncated, total_bytes) =
            super::types::truncate_output(&output, context.max_output_bytes);

        let mut result = ToolResult::success(output);
        if truncated {
            result = result.with_truncation(total_bytes);
        }
        result = result.with_metadata("total_lines", serde_json::json!(total_lines));

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
    async fn test_read_file() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        // Create a test file
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "line1\nline2\nline3\n").await.unwrap();

        let result = tool
            .execute(serde_json::json!({ "path": "test.txt" }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("line1"));
        assert!(result.output.contains("line2"));
        assert!(result.output.contains("line3"));
    }

    #[tokio::test]
    async fn test_read_file_with_line_numbers() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        let file_path = dir.path().join("test.rs");
        tokio::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n")
            .await
            .unwrap();

        let result = tool
            .execute(serde_json::json!({ "path": "test.rs" }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("     1\t"));
        assert!(result.output.contains("     2\t"));
        assert!(result.output.contains("     3\t"));
    }

    #[tokio::test]
    async fn test_read_file_with_offset_and_limit() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        let file_path = dir.path().join("lines.txt");
        let content: String = (1..=10).map(|i| format!("line {}\n", i)).collect();
        tokio::fs::write(&file_path, content).await.unwrap();

        let result = tool
            .execute(
                serde_json::json!({ "path": "lines.txt", "offset": 3, "limit": 2 }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("line 3"));
        assert!(result.output.contains("line 4"));
        assert!(!result.output.contains("line 1"));
        assert!(!result.output.contains("line 5"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": "nonexistent.txt" }), &ctx)
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("not found"));
    }

    #[tokio::test]
    async fn test_read_directory() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": "." }), &ctx)
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("directory"));
    }

    #[tokio::test]
    async fn test_read_image_file() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        let file_path = dir.path().join("photo.png");
        tokio::fs::write(&file_path, b"fake-png-data").await.unwrap();

        let result = tool
            .execute(serde_json::json!({ "path": "photo.png" }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("Image file"));
    }

    #[tokio::test]
    async fn test_read_empty_file() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        let file_path = dir.path().join("empty.txt");
        tokio::fs::write(&file_path, "").await.unwrap();

        let result = tool
            .execute(serde_json::json!({ "path": "empty.txt" }), &ctx)
            .await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_read_missing_path() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = ReadTool::new();

        let result = tool
            .execute(serde_json::json!({}), &ctx)
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("Missing"));
    }

    #[tokio::test]
    async fn test_read_absolute_path() {
        let dir = TempDir::new().unwrap();
        let tool = ReadTool::new();

        let file_path = dir.path().join("abs.txt");
        tokio::fs::write(&file_path, "absolute path test\n").await.unwrap();

        let ctx = ToolContext::new("/some/other/dir");
        let result = tool
            .execute(
                serde_json::json!({ "path": file_path.to_str().unwrap() }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("absolute path test"));
    }
}
