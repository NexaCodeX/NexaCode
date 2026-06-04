use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

/// Registry for managing and executing tools
#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        self.tools.insert(name, Arc::new(tool));
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// List all registered tool names
    pub fn list_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get tool definitions for LLM function calling
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.parameters()).collect()
    }

    /// Get tool definitions filtered by risk level
    pub fn definitions_by_risk(&self, max_risk: RiskLevel) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|t| t.risk_level() as u8 <= max_risk as u8)
            .map(|t| t.parameters())
            .collect()
    }

    /// Execute a tool by name
    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Tool '{}' not found", name))?;

        // Validate arguments
        if let Err(e) = tool.validate(&args) {
            return Ok(ToolResult::error(format!("Validation error: {}", e)));
        }

        Ok(tool.execute(args, context).await)
    }

    /// Check if a tool requires confirmation for the given arguments
    pub fn requires_confirmation(&self, name: &str, args: &Value) -> bool {
        self.tools
            .get(name)
            .map(|t| t.requires_confirmation(args))
            .unwrap_or(true) // Unknown tools require confirmation by default
    }

    /// Get the risk level of a tool
    pub fn risk_level(&self, name: &str) -> Option<RiskLevel> {
        self.tools.get(name).map(|t| t.risk_level())
    }

    /// Number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a registry with all default tools registered
pub fn default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // File operations
    registry.register(super::read::ReadTool::new());
    registry.register(super::write::WriteTool::new());
    registry.register(super::ls::LsTool::new());
    registry.register(super::grep::GrepTool::new());
    registry.register(super::glob::GlobTool::new());

    // Code editing
    registry.register(super::edit::EditTool::new());
    registry.register(super::multi_edit::MultiEditTool::new());

    // Execution environment
    registry.register(super::bash::BashTool::new());

    // Information retrieval
    registry.register(super::web_fetch::WebFetchTool::new());

    // Diagnostics
    registry.register(super::diagnostic::DiagnosticTool::new());

    // Task management
    registry.register(super::task::TaskTool::new());

    // CodeGraph
    registry.register(super::codegraph::CodeGraphTool::new());

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    // Mock tool for testing
    struct MockTool {
        name: String,
        risk: RiskLevel,
    }

    impl MockTool {
        fn new(name: &str, risk: RiskLevel) -> Self {
            Self {
                name: name.to_string(),
                risk,
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn parameters(&self) -> ToolDefinition {
            ToolDefinition {
                name: self.name.clone(),
                description: "A mock tool for testing".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            }
        }

        fn risk_level(&self) -> RiskLevel {
            self.risk
        }

        async fn execute(&self, _args: Value, _context: &ToolContext) -> ToolResult {
            ToolResult::success(format!("{} executed", self.name))
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("TestTool", RiskLevel::Safe));
        assert!(registry.get("TestTool").is_some());
        assert!(registry.get("NonExistent").is_none());
    }

    #[test]
    fn test_list_names() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("Bravo", RiskLevel::Safe));
        registry.register(MockTool::new("Alpha", RiskLevel::Safe));
        assert_eq!(registry.list_names(), vec!["Alpha", "Bravo"]);
    }

    #[test]
    fn test_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("Tool1", RiskLevel::Safe));
        registry.register(MockTool::new("Tool2", RiskLevel::Dangerous));
        let defs = registry.definitions();
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn test_definitions_by_risk() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("Safe", RiskLevel::Safe));
        registry.register(MockTool::new("Dangerous", RiskLevel::Dangerous));
        let safe_defs = registry.definitions_by_risk(RiskLevel::Safe);
        assert_eq!(safe_defs.len(), 1);
        assert_eq!(safe_defs[0].name, "Safe");
    }

    #[tokio::test]
    async fn test_execute() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("TestTool", RiskLevel::Safe));
        let ctx = ToolContext::default();
        let result = registry
            .execute("TestTool", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert_eq!(result.output, "TestTool executed");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let registry = ToolRegistry::new();
        let ctx = ToolContext::default();
        let result = registry.execute("Missing", serde_json::json!({}), &ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_requires_confirmation() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("SafeTool", RiskLevel::Safe));
        registry.register(MockTool::new("DangerTool", RiskLevel::Dangerous));
        assert!(!registry.requires_confirmation("SafeTool", &serde_json::json!({})));
        assert!(registry.requires_confirmation("DangerTool", &serde_json::json!({})));
    }

    #[test]
    fn test_default_registry() {
        let registry = default_registry();
        assert!(registry.len() >= 10);
        assert!(registry.get("Read").is_some());
        assert!(registry.get("Write").is_some());
        assert!(registry.get("Bash").is_some());
        assert!(registry.get("Edit").is_some());
        assert!(registry.get("Glob").is_some());
    }
}
