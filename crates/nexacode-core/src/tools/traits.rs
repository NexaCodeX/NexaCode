use async_trait::async_trait;
use serde_json::Value;

use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

/// Core trait that all tools must implement.
///
/// Each tool provides:
/// - Identity: name, description
/// - Schema: parameters for LLM function calling
/// - Execution: async run with context
/// - Safety: risk level and confirmation logic
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name (e.g., "Read", "Write", "Bash")
    fn name(&self) -> &str;

    /// Human-readable description of what the tool does
    fn description(&self) -> &str;

    /// JSON Schema of the tool's parameters (for LLM function calling)
    fn parameters(&self) -> ToolDefinition;

    /// Risk level of this tool's operations
    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    /// Dynamically determine if this specific invocation needs user confirmation.
    /// Override this for tools that are conditionally dangerous (e.g., Bash: ls vs rm)
    fn requires_confirmation(&self, _args: &Value) -> bool {
        matches!(self.risk_level(), RiskLevel::Moderate | RiskLevel::Dangerous)
    }

    /// Validate arguments before execution. Return Err to prevent execution.
    fn validate(&self, _args: &Value) -> Result<(), String> {
        Ok(())
    }

    /// Execute the tool with the given arguments and context
    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult;
}
