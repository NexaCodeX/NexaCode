use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

/// Search file contents tool (like grep)
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in file contents. Supports regular expressions. \
         Returns matching lines with file paths and line numbers."
    }

    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "The regex pattern to search for" },
                    "path": { "type": "string", "description": "Directory or file to search in" },
                    "include": { "type": "string", "description": "File glob pattern to include (e.g. '*.rs')" },
                    "case_insensitive": { "type": "boolean", "description": "Case-insensitive search (default: false)" },
                    "max_results": { "type": "integer", "description": "Maximum results (default: 50)" }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let pattern = match args["pattern"].as_str() {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => return ToolResult::error("Missing required parameter: pattern"),
        };

        let search_path = args["path"].as_str().unwrap_or(".");
        let include = args["include"].as_str();
        let case_insensitive = args["case_insensitive"].as_bool().unwrap_or(false);
        let max_results = args["max_results"].as_u64().unwrap_or(50) as usize;

        let root = context.resolve_path(search_path);
        if !root.exists() {
            return ToolResult::error(format!("Path not found: {}", search_path));
        }

        let re_pattern = if case_insensitive {
            format!("(?i){}", pattern)
        } else {
            pattern.clone()
        };
        let re = match Regex::new(&re_pattern) {
            Ok(re) => re,
            Err(e) => return ToolResult::error(format!("Invalid regex pattern: {}", e)),
        };

        let mut results: Vec<String> = Vec::new();
        let mut total_matches = 0;

        if root.is_file() {
            search_file(&root, &re, &mut results, max_results, &mut total_matches);
        } else {
            search_directory(&root, &root, &re, include, &mut results, max_results, &mut total_matches);
        }

        if results.is_empty() {
            return ToolResult::success(format!("No matches found for pattern: {}", pattern));
        }

        let mut output = results.join("\n");
        if total_matches > max_results {
            output.push_str(&format!(
                "\n\nShowing {} of {} matches.",
                results.len().min(max_results),
                total_matches
            ));
        }

        let (output, truncated, total_bytes) =
            super::types::truncate_output(&output, context.max_output_bytes);

        let mut result = ToolResult::success(output);
        if truncated {
            result = result.with_truncation(total_bytes);
        }
        result.with_metadata("matches", serde_json::json!(total_matches))
    }
}

fn search_file(
    path: &std::path::Path,
    re: &Regex,
    results: &mut Vec<String>,
    max_results: usize,
    total_matches: &mut usize,
) {
    if results.len() >= max_results {
        return;
    }

    if let Ok(content) = std::fs::read_to_string(path) {
        let mut per_file = 0;
        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                *total_matches += 1;
                if results.len() < max_results && per_file < 5 {
                    results.push(format!("{}:{}\t{}", path.display(), i + 1, line.trim()));
                    per_file += 1;
                }
            }
        }
    }
}

fn search_directory(
    base: &std::path::Path,
    current: &std::path::Path,
    re: &Regex,
    include: Option<&str>,
    results: &mut Vec<String>,
    max_results: usize,
    total_matches: &mut usize,
) {
    if results.len() >= max_results {
        return;
    }

    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip hidden directories and common non-source directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.')
                || matches!(
                    name,
                    "node_modules" | "target" | "dist" | "build" | "__pycache__" | "vendor"
                )
            {
                continue;
            }
        }

        if path.is_dir() {
            search_directory(base, &path, re, include, results, max_results, total_matches);
        } else if path.is_file() {
            // Apply include filter
            if let Some(glob_pattern) = include {
                let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if let Ok(g) = glob::Pattern::new(glob_pattern) {
                    if !g.matches(fname) {
                        continue;
                    }
                }
            }

            search_file(&path, re, results, max_results, total_matches);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir) -> ToolContext {
        ToolContext::new(dir.path())
    }

    async fn create_test_files(dir: &TempDir) {
        tokio::fs::write(dir.path().join("main.rs"), "fn main() {\n    println!(\"hello\");\n}\n")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("lib.rs"), "pub fn greet() {\n    println!(\"greeting\");\n}\n")
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join("src")).await.unwrap();
        tokio::fs::write(
            dir.path().join("src/utils.rs"),
            "pub fn helper() {\n    // TODO: implement\n    println!(\"help\");\n}\n",
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_grep_basic() {
        let dir = TempDir::new().unwrap();
        create_test_files(&dir).await;
        let tool = GrepTool::new();
        let result = tool
            .execute(serde_json::json!({ "pattern": "println" }), &ctx(&dir))
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("println"));
        assert!(result.output.contains("main.rs"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "Hello World\nhello world\nHELLO WORLD\n")
            .await
            .unwrap();
        let tool = GrepTool::new();
        let result = tool
            .execute(serde_json::json!({ "pattern": "hello", "case_insensitive": true }), &ctx(&dir))
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("Hello World"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("test.txt"), "hello world")
            .await
            .unwrap();
        let tool = GrepTool::new();
        let result = tool
            .execute(serde_json::json!({ "pattern": "xyz_not_found" }), &ctx(&dir))
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("No matches"));
    }

    #[tokio::test]
    async fn test_grep_invalid_regex() {
        let dir = TempDir::new().unwrap();
        let tool = GrepTool::new();
        let result = tool
            .execute(serde_json::json!({ "pattern": "[invalid" }), &ctx(&dir))
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_grep_missing_pattern() {
        let dir = TempDir::new().unwrap();
        let tool = GrepTool::new();
        let result = tool.execute(serde_json::json!({}), &ctx(&dir)).await;
        assert!(result.is_error);
    }
}
