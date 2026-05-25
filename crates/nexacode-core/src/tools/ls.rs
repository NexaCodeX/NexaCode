use async_trait::async_trait;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

/// List directory contents tool
pub struct LsTool;

impl LsTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "LS"
    }

    fn description(&self) -> &str {
        "List the contents of a directory. Shows files and subdirectories with type indicators \
         (/ for directories, * for executables). Use recursive=true for deep listing."
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
                        "description": "The directory path to list (default: working directory)"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to list recursively (default: false)"
                    }
                },
                "required": []
            }),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let path = args["path"].as_str().unwrap_or(".");
        let recursive = args["recursive"].as_bool().unwrap_or(false);

        let dir_path = context.resolve_path(path);

        if !dir_path.exists() {
            return ToolResult::error(format!("Directory not found: {}", path));
        }

        if !dir_path.is_dir() {
            return ToolResult::error(format!(
                "'{}' is a file, not a directory. Use Read to view file contents.",
                path
            ));
        }

        let mut entries = Vec::new();
        if let Err(e) = collect_entries(&dir_path, &dir_path, recursive, &mut entries) {
            return ToolResult::error(format!("Failed to list directory: {}", e));
        }

        if entries.is_empty() {
            return ToolResult::success(format!("Directory '{}' is empty", path));
        }

        let output = entries.join("\n");
        let (output, truncated, total_bytes) =
            super::types::truncate_output(&output, context.max_output_bytes);

        let mut result = ToolResult::success(output);
        if truncated {
            result = result.with_truncation(total_bytes);
        }
        result = result.with_metadata("count", serde_json::json!(entries.len()));

        result
    }
}

fn collect_entries(
    base: &std::path::Path,
    current: &std::path::Path,
    recursive: bool,
    entries: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    let mut dir_entries: Vec<std::fs::DirEntry> = std::fs::read_dir(current)?
        .filter_map(|e| e.ok())
        .collect();

    // Sort: directories first, then files, alphabetically within each group
    dir_entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    for entry in dir_entries {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy().to_string();

        // Skip hidden files/directories
        if name.starts_with('.') {
            continue;
        }

        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let relative = current
            .strip_prefix(base)
            .unwrap_or(current)
            .join(&name)
            .to_string_lossy()
            .to_string();

        if is_dir {
            entries.push(format!("{}/", relative));
            if recursive {
                collect_entries(base, &entry.path(), recursive, entries)?;
            }
        } else {
            entries.push(relative);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_context(dir: &TempDir) -> ToolContext {
        ToolContext::new(dir.path())
    }

    async fn create_test_structure(dir: &TempDir) {
        // Create files
        tokio::fs::write(dir.path().join("file1.txt"), "content1")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("file2.rs"), "fn main() {}")
            .await
            .unwrap();

        // Create subdirectory with files
        tokio::fs::create_dir(dir.path().join("src")).await.unwrap();
        tokio::fs::write(dir.path().join("src/main.rs"), "fn main() {}")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("src/lib.rs"), "pub mod test;")
            .await
            .unwrap();

        // Create nested subdirectory
        tokio::fs::create_dir(dir.path().join("src/utils")).await.unwrap();
        tokio::fs::write(dir.path().join("src/utils/mod.rs"), "pub mod helpers;")
            .await
            .unwrap();

        // Create hidden file (should be ignored)
        tokio::fs::write(dir.path().join(".hidden"), "hidden")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_ls_basic() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir).await;
        let ctx = setup_context(&dir);
        let tool = LsTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": "." }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("src/"));
        assert!(result.output.contains("file1.txt"));
        assert!(result.output.contains("file2.rs"));
        // Hidden files should not appear
        assert!(!result.output.contains(".hidden"));
        // Non-recursive: should not show src/ contents
        assert!(!result.output.contains("main.rs"));
    }

    #[tokio::test]
    async fn test_ls_recursive() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir).await;
        let ctx = setup_context(&dir);
        let tool = LsTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": ".", "recursive": true }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("src/"));
        assert!(result.output.contains("src/main.rs"));
        assert!(result.output.contains("src/lib.rs"));
        assert!(result.output.contains("src/utils/"));
        assert!(result.output.contains("src/utils/mod.rs"));
    }

    #[tokio::test]
    async fn test_ls_subdirectory() {
        let dir = TempDir::new().unwrap();
        create_test_structure(&dir).await;
        let ctx = setup_context(&dir);
        let tool = LsTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": "src" }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("main.rs"));
        assert!(result.output.contains("lib.rs"));
        assert!(result.output.contains("utils/"));
        // Should not show parent level files
        assert!(!result.output.contains("file1.txt"));
    }

    #[tokio::test]
    async fn test_ls_not_found() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = LsTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": "nonexistent" }), &ctx)
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("not found"));
    }

    #[tokio::test]
    async fn test_ls_file_instead_of_dir() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "hello")
            .await
            .unwrap();
        let ctx = setup_context(&dir);
        let tool = LsTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": "test.txt" }), &ctx)
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("file"));
    }

    #[tokio::test]
    async fn test_ls_empty_directory() {
        let dir = TempDir::new().unwrap();
        let ctx = setup_context(&dir);
        let tool = LsTool::new();

        let result = tool
            .execute(serde_json::json!({ "path": "." }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("empty"));
    }
}
