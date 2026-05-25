use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }
}

/// Check if a command is safe to auto-execute (no confirmation needed)
fn is_safe_command(cmd: &str) -> bool {
    let safe_prefixes = [
        "ls", "cat", "head", "tail", "grep", "find", "wc", "echo",
        "pwd", "which", "whoami", "date", "uname", "df", "du",
        "git status", "git diff", "git log", "git branch", "git remote",
        "cargo check", "cargo test", "cargo build", "cargo clippy",
        "npm list", "npm test", "npm run",
        "python3 --version", "node --version", "rustc --version",
    ];
    let cmd_trimmed = cmd.trim();
    safe_prefixes.iter().any(|p| cmd_trimmed.starts_with(p))
}

/// Check if a command is dangerous (needs extra warning)
#[allow(dead_code)]
fn is_dangerous_command(cmd: &str) -> bool {
    let dangerous_patterns = [
        "rm -rf", "rm -r", "rmdir", "sudo ", "mkfs",
        "git push", "git reset --hard", "git clean",
        "dd if=", "chmod 777", "chown",
        "> /dev/", "curl ", "wget ",
    ];
    let cmd_trimmed = cmd.trim();
    dangerous_patterns.iter().any(|p| cmd_trimmed.contains(p))
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Execute a terminal command and return the output. Runs in the project working directory. \
         Use for running builds, tests, git operations, and other shell commands."
    }

    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute" },
                    "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
                },
                "required": ["command"]
            }),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Moderate
    }

    fn requires_confirmation(&self, args: &Value) -> bool {
        if let Some(cmd) = args["command"].as_str() {
            !is_safe_command(cmd)
        } else {
            true
        }
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let command = match args["command"].as_str() {
            Some(c) if !c.is_empty() => c.to_string(),
            _ => return ToolResult::error("Missing required parameter: command"),
        };
        let _timeout_secs = args["timeout"].as_u64().unwrap_or(context.timeout_secs);

        let output = match Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&context.working_dir)
            .output()
            .await
        {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("Failed to execute command: {}", e)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let mut result_str = String::new();
        if !stdout.is_empty() {
            result_str.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result_str.is_empty() {
                result_str.push('\n');
            }
            result_str.push_str("[stderr]\n");
            result_str.push_str(&stderr);
        }
        if exit_code != 0 {
            result_str.push_str(&format!("\n[Exit code: {}]", exit_code));
        }

        let is_error = exit_code != 0;
        let (result_str, truncated, total_bytes) =
            super::types::truncate_output(&result_str, context.max_output_bytes);

        let mut result = ToolResult::success(result_str);
        if is_error {
            result.is_error = true;
        }
        if truncated {
            result = result.with_truncation(total_bytes);
        }
        result.with_metadata("exit_code", serde_json::json!(exit_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir) -> ToolContext {
        ToolContext::new(dir.path())
    }

    #[test]
    fn test_is_safe_command() {
        assert!(is_safe_command("ls -la"));
        assert!(is_safe_command("git status"));
        assert!(is_safe_command("cat file.txt"));
        assert!(!is_safe_command("rm -rf /"));
        assert!(!is_safe_command("npm install"));
    }

    #[test]
    fn test_is_dangerous_command() {
        assert!(is_dangerous_command("rm -rf /tmp/test"));
        assert!(is_dangerous_command("sudo apt install"));
        assert!(!is_dangerous_command("ls -la"));
    }

    #[tokio::test]
    async fn test_bash_echo() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let result = tool
            .execute(serde_json::json!({ "command": "echo hello" }), &ctx(&dir))
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_pwd() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let result = tool
            .execute(serde_json::json!({ "command": "pwd" }), &ctx(&dir))
            .await;
        assert!(!result.is_error);
        assert!(result.output.contains(dir.path().to_str().unwrap()));
    }

    #[tokio::test]
    async fn test_bash_exit_code() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let result = tool
            .execute(serde_json::json!({ "command": "exit 1" }), &ctx(&dir))
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("Exit code: 1"));
    }

    #[tokio::test]
    async fn test_bash_stderr() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let result = tool
            .execute(serde_json::json!({ "command": "echo error >&2" }), &ctx(&dir))
            .await;
        assert!(result.output.contains("error"));
    }

    #[tokio::test]
    async fn test_bash_missing_command() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let result = tool.execute(serde_json::json!({}), &ctx(&dir)).await;
        assert!(result.is_error);
    }

    #[test]
    fn test_requires_confirmation() {
        let tool = BashTool::new();
        assert!(!tool.requires_confirmation(&serde_json::json!({ "command": "ls" })));
        assert!(tool.requires_confirmation(&serde_json::json!({ "command": "npm install" })));
    }
}
