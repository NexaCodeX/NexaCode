use async_trait::async_trait;
use glob::glob as glob_walk;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct GlobTool;

impl GlobTool { pub fn new() -> Self { Self } }

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str { "Glob" }
    fn description(&self) -> &str {
        "Find files by name pattern using glob syntax. Returns matching file paths.          Examples: '**/*.rs' finds all Rust files, 'src/**/*.ts' finds TypeScript files in src/."
    }
    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern (e.g., '**/*.rs', 'src/**/*.ts', '*.json')" },
                    "path": { "type": "string", "description": "Root directory to search in (default: working directory)" }
                },
                "required": ["pattern"]
            }),
        }
    }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Safe }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let pattern = match args["pattern"].as_str() {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => return ToolResult::error("Missing required parameter: pattern"),
        };
        let search_path = args["path"].as_str().unwrap_or(".");
        let root = context.resolve_path(search_path);
        if !root.exists() { return ToolResult::error(format!("Path not found: {}", search_path)); }

        let full_pattern = root.join(&pattern).to_string_lossy().to_string();

        let mut matches: Vec<String> = Vec::new();
        let mut total = 0;
        for entry in glob_walk(&full_pattern).unwrap_or_else(|e| {
            glob_walk(&format!("INVALID{}", e)).unwrap()
        }) {
            match entry {
                Ok(path) => {
                    total += 1;
                    if matches.len() < 100 {
                        let relative = path.strip_prefix(&root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();
                        matches.push(relative);
                    }
                }
                Err(_) => continue,
            }
        }

        if matches.is_empty() {
            return ToolResult::success(format!("No files matched pattern: {}", pattern));
        }

        let mut output = matches.join("\n");
        if total > 100 {
            output.push_str(&format!("\n\nShowing 100 of {} files. Use a more specific pattern.", total));
        }

        let mut result = ToolResult::success(output);
        result = result.with_metadata("total", serde_json::json!(total));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir) -> ToolContext { ToolContext::new(dir.path()) }

    async fn create_test_files(dir: &TempDir) {
        tokio::fs::write(dir.path().join("main.rs"), "fn main() {}").await.unwrap();
        tokio::fs::write(dir.path().join("lib.rs"), "pub mod test;").await.unwrap();
        tokio::fs::write(dir.path().join("package.json"), "{}").await.unwrap();
        tokio::fs::create_dir(dir.path().join("src")).await.unwrap();
        tokio::fs::write(dir.path().join("src/app.ts"), "console.log('hi')").await.unwrap();
        tokio::fs::write(dir.path().join("src/utils.ts"), "export const x = 1;").await.unwrap();
    }

    #[tokio::test]
    async fn test_glob_rs_files() {
        let dir = TempDir::new().unwrap(); create_test_files(&dir).await;
        let tool = GlobTool::new();
        let result = tool.execute(serde_json::json!({ "pattern": "**/*.rs" }), &ctx(&dir)).await;
        assert!(!result.is_error);
        assert!(result.output.contains("main.rs"));
        assert!(result.output.contains("lib.rs"));
    }

    #[tokio::test]
    async fn test_glob_ts_files() {
        let dir = TempDir::new().unwrap(); create_test_files(&dir).await;
        let tool = GlobTool::new();
        let result = tool.execute(serde_json::json!({ "pattern": "src/**/*.ts" }), &ctx(&dir)).await;
        assert!(!result.is_error);
        assert!(result.output.contains("app.ts"));
        assert!(result.output.contains("utils.ts"));
        assert!(!result.output.contains("main.rs"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let dir = TempDir::new().unwrap();
        let tool = GlobTool::new();
        let result = tool.execute(serde_json::json!({ "pattern": "*.xyz" }), &ctx(&dir)).await;
        assert!(!result.is_error);
        assert!(result.output.contains("No files matched"));
    }

    #[tokio::test]
    async fn test_glob_missing_pattern() {
        let dir = TempDir::new().unwrap();
        let tool = GlobTool::new();
        let result = tool.execute(serde_json::json!({}), &ctx(&dir)).await;
        assert!(result.is_error);
    }
}
