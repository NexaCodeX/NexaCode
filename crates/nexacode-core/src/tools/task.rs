use async_trait::async_trait;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct TaskTool;

impl TaskTool { pub fn new() -> Self { Self } }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Task {
    id: String,
    description: String,
    status: TaskStatus,
    subtasks: Vec<Task>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str { "Task" }
    fn description(&self) -> &str {
        "Manage task lists for tracking progress on complex operations.          Create tasks, update their status, and list current progress.          Helps break down complex work into trackable steps."
    }
    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["create", "update", "list"],
                        "description": "The action to perform"
                    },
                    "tasks": {
                        "type": "array",
                        "description": "For 'create': array of task descriptions",
                        "items": { "type": "string" }
                    },
                    "task_id": {
                        "type": "string",
                        "description": "For 'update': the task ID to update"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "cancelled"],
                        "description": "For 'update': new status"
                    }
                },
                "required": ["action"]
            }),
        }
    }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Safe }

    async fn execute(&self, args: Value, _context: &ToolContext) -> ToolResult {
        let action = match args["action"].as_str() {
            Some(a) => a.to_string(),
            None => return ToolResult::error("Missing required parameter: action"),
        };

        match action.as_str() {
            "create" => {
                let descriptions = args.get("tasks").and_then(|t| t.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                    .unwrap_or_default();
                if descriptions.is_empty() { return ToolResult::error("'create' action requires 'tasks' array"); }
                let tasks: Vec<Task> = descriptions.into_iter().enumerate().map(|(i, desc)| Task {
                    id: format!("task_{}", i + 1),
                    description: desc,
                    status: TaskStatus::Pending,
                    subtasks: Vec::new(),
                }).collect();
                let output = tasks.iter().enumerate().map(|(i, t)| {
                    format!("[ ] {}: {}", i + 1, t.description)
                }).collect::<Vec<_>>().join("\n");
                ToolResult::success(format!("Created {} task(s):\n{}", tasks.len(), output))
            }
            "update" => {
                let task_id = args["task_id"].as_str().unwrap_or("unknown");
                let status = args["status"].as_str().unwrap_or("pending");
                let icon = match status {
                    "completed" => "[x]",
                    "in_progress" => "[~]",
                    "cancelled" => "[-]",
                    _ => "[ ]",
                };
                ToolResult::success(format!("Task '{}' updated to: {} {}", task_id, icon, status))
            }
            "list" => {
                ToolResult::success("No active task list. Use 'create' to create tasks.")
            }
            _ => ToolResult::error(format!("Unknown action: {}. Use 'create', 'update', or 'list'.", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &TempDir) -> ToolContext { ToolContext::new(dir.path()) }

    #[tokio::test]
    async fn test_task_create() {
        let dir = TempDir::new().unwrap();
        let tool = TaskTool::new();
        let result = tool.execute(serde_json::json!({
            "action": "create",
            "tasks": ["Read the file", "Fix the bug", "Run tests"]
        }), &ctx(&dir)).await;
        assert!(!result.is_error);
        assert!(result.output.contains("3 task"));
        assert!(result.output.contains("Read the file"));
        assert!(result.output.contains("Fix the bug"));
    }

    #[tokio::test]
    async fn test_task_update() {
        let dir = TempDir::new().unwrap();
        let tool = TaskTool::new();
        let result = tool.execute(serde_json::json!({
            "action": "update", "task_id": "task_1", "status": "completed"
        }), &ctx(&dir)).await;
        assert!(!result.is_error);
        assert!(result.output.contains("completed"));
    }

    #[tokio::test]
    async fn test_task_list() {
        let dir = TempDir::new().unwrap();
        let tool = TaskTool::new();
        let result = tool.execute(serde_json::json!({ "action": "list" }), &ctx(&dir)).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_task_unknown_action() {
        let dir = TempDir::new().unwrap();
        let tool = TaskTool::new();
        let result = tool.execute(serde_json::json!({ "action": "delete" }), &ctx(&dir)).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_task_create_empty() {
        let dir = TempDir::new().unwrap();
        let tool = TaskTool::new();
        let result = tool.execute(serde_json::json!({ "action": "create", "tasks": [] }), &ctx(&dir)).await;
        assert!(result.is_error);
    }
}
