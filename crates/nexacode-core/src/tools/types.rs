use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Risk level of a tool operation, determines if user confirmation is needed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Safe to execute without confirmation (e.g., Read, LS, Grep)
    Safe,
    /// Moderate risk, may need confirmation (e.g., Write new file)
    Moderate,
    /// Dangerous, always needs confirmation (e.g., rm -rf, destructive edits)
    Dangerous,
}

/// The result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// The output of the tool (text content)
    pub output: String,
    /// Whether the tool execution resulted in an error
    pub is_error: bool,
    /// Additional metadata about the result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Whether the output was truncated due to size limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    /// Original output size in bytes before truncation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
}

impl ToolResult {
    /// Create a successful tool result
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: false,
            metadata: None,
            truncated: None,
            total_bytes: None,
        }
    }

    /// Create an error tool result
    pub fn error(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: true,
            metadata: None,
            truncated: None,
            total_bytes: None,
        }
    }

    /// Mark this result as truncated
    pub fn with_truncation(mut self, total_bytes: u64) -> Self {
        self.truncated = Some(true);
        self.total_bytes = Some(total_bytes);
        self
    }

    /// Add metadata to this result
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }
}

/// Context provided to tools during execution
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// The working directory for file operations
    pub working_dir: std::path::PathBuf,
    /// Maximum output size in bytes before truncation
    pub max_output_bytes: u64,
    /// Timeout in seconds for commands
    pub timeout_secs: u64,
    /// Current session ID for undo/backup tracking
    pub session_id: Option<String>,
}

impl ToolContext {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
            max_output_bytes: 50_000, // 50KB default
            timeout_secs: 120,
            session_id: None,
        }
    }

    /// Resolve a path relative to the working directory
    pub fn resolve_path(&self, path: &str) -> std::path::PathBuf {
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        }
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
    }
}

/// A parameter definition for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

/// Tool definition suitable for sending to LLMs (OpenAI/Anthropic function calling format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Truncation helper for tool output
pub fn truncate_output(output: &str, max_bytes: u64) -> (String, bool, u64) {
    let total_bytes = output.len() as u64;
    if total_bytes <= max_bytes {
        return (output.to_string(), false, total_bytes);
    }

    // Try to cut at a line boundary near the limit
    let byte_limit = max_bytes as usize;
    let truncated = &output.as_bytes()[..byte_limit.min(output.len())];
    let s = String::from_utf8_lossy(truncated);

    // Try to cut at the last newline
    if let Some(last_newline) = s.rfind('\n') {
        (
            format!("{}...\n[Output truncated: {} bytes total]", &s[..last_newline], total_bytes),
            true,
            total_bytes,
        )
    } else {
        (
            format!("{}...\n[Output truncated: {} bytes total]", &s[..s.len().min(500)], total_bytes),
            true,
            total_bytes,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("hello world");
        assert_eq!(result.output, "hello world");
        assert!(!result.is_error);
        assert!(result.truncated.is_none());
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("something went wrong");
        assert_eq!(result.output, "something went wrong");
        assert!(result.is_error);
    }

    #[test]
    fn test_tool_result_with_truncation() {
        let result = ToolResult::success("hello").with_truncation(1000);
        assert_eq!(result.truncated, Some(true));
        assert_eq!(result.total_bytes, Some(1000));
    }

    #[test]
    fn test_tool_result_with_metadata() {
        let result = ToolResult::success("ok")
            .with_metadata("lines", serde_json::json!(42));
        assert_eq!(result.metadata.unwrap().get("lines").unwrap(), &serde_json::json!(42));
    }

    #[test]
    fn test_tool_context_resolve_path_absolute() {
        let ctx = ToolContext::new("/home/user/project");
        let resolved = ctx.resolve_path("/etc/config");
        assert_eq!(resolved, std::path::PathBuf::from("/etc/config"));
    }

    #[test]
    fn test_tool_context_resolve_path_relative() {
        let ctx = ToolContext::new("/home/user/project");
        let resolved = ctx.resolve_path("src/main.rs");
        assert_eq!(resolved, std::path::PathBuf::from("/home/user/project/src/main.rs"));
    }

    #[test]
    fn test_truncate_output_small() {
        let (output, truncated, total) = truncate_output("hello", 100);
        assert_eq!(output, "hello");
        assert!(!truncated);
        assert_eq!(total, 5);
    }

    #[test]
    fn test_truncate_output_large() {
        let long = "line1\nline2\nline3\nline4\nline5\n";
        let (output, truncated, total) = truncate_output(long, 15);
        assert!(truncated);
        assert!(output.contains("[Output truncated"));
        assert_eq!(total, long.len() as u64);
    }
}
