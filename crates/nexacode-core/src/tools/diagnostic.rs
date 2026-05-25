use async_trait::async_trait;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct DiagnosticTool;

impl DiagnosticTool { pub fn new() -> Self { Self } }

#[async_trait]
impl Tool for DiagnosticTool {
    fn name(&self) -> &str { "Diagnostic" }
    fn description(&self) -> &str {
        "Run diagnostic commands (cargo check, npm run build, etc.) and return errors/warnings.          Automatically detects the project type and runs the appropriate check command."
    }
    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Custom diagnostic command to run" }
                },
                "required": []
            }),
        }
    }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Safe }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let custom_cmd = args["command"].as_str();

        let cmd = if let Some(c) = custom_cmd {
            c.to_string()
        } else {
            // Auto-detect project type
            let has_cargo = context.working_dir.join("Cargo.toml").exists();
            let has_package_json = context.working_dir.join("package.json").exists();
            let has_pyproject = context.working_dir.join("pyproject.toml").exists();

            if has_cargo { "cargo check 2>&1".to_string() }
            else if has_package_json { "npm run build 2>&1 || npm run check 2>&1".to_string() }
            else if has_pyproject { "python -m py_compile . 2>&1".to_string() }
            else { return ToolResult::error("Cannot auto-detect project type. Please provide a 'command' parameter."); }
        };

        let output = match tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&context.working_dir)
            .output()
            .await
        {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("Failed to run diagnostic: {}", e)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let mut result_str = if stdout.is_empty() && stderr.is_empty() {
            format!("Command: {}\nExit code: {}\n(no output)", cmd, exit_code)
        } else {
            let mut s = format!("Command: {}\n", cmd);
            if !stdout.is_empty() { s.push_str(&stdout); }
            if !stderr.is_empty() { s.push_str(&stderr); }
            s
        };

        if exit_code == 0 {
            result_str = format!("[OK] {}", result_str);
        } else {
            result_str = format!("[ERRORS] {}", result_str);
        }

        let (result_str, truncated, total_bytes) =
            super::types::truncate_output(&result_str, context.max_output_bytes);

        let mut result = ToolResult::success(result_str);
        if exit_code != 0 { result.is_error = true; }
        if truncated { result = result.with_truncation(total_bytes); }
        result = result.with_metadata("exit_code", serde_json::json!(exit_code));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir) -> ToolContext { ToolContext::new(dir.path()) }

    #[tokio::test]
    async fn test_diagnostic_custom_command() {
        let dir = TempDir::new().unwrap();
        let tool = DiagnosticTool::new();
        let result = tool.execute(serde_json::json!({ "command": "echo all good" }), &ctx(&dir)).await;
        assert!(!result.is_error);
        assert!(result.output.contains("[OK]"));
    }

    #[tokio::test]
    async fn test_diagnostic_auto_detect_no_project() {
        let dir = TempDir::new().unwrap();
        let tool = DiagnosticTool::new();
        let result = tool.execute(serde_json::json!({}), &ctx(&dir)).await;
        assert!(result.is_error);
        assert!(result.output.contains("auto-detect"));
    }

    #[tokio::test]
    async fn test_diagnostic_rust_project() {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").await.unwrap();
        let tool = DiagnosticTool::new();
        let result = tool.execute(serde_json::json!({}), &ctx(&dir)).await;
        // May succeed or fail depending on cargo, but should at least attempt cargo check
        assert!(result.output.contains("cargo check"));
    }
}
