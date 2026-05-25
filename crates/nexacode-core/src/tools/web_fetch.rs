use async_trait::async_trait;
use serde_json::Value;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct WebFetchTool;

impl WebFetchTool { pub fn new() -> Self { Self } }

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "WebFetch" }
    fn description(&self) -> &str {
        "Fetch the content of a web page by URL. Returns the page text content. \
         Useful for reading documentation or API references."
    }
    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The URL to fetch" },
                    "max_length": { "type": "integer", "description": "Maximum response length in chars (default: 10000)" }
                },
                "required": ["url"]
            }),
        }
    }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Moderate }

    async fn execute(&self, args: Value, _context: &ToolContext) -> ToolResult {
        let url = match args["url"].as_str() {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => return ToolResult::error("Missing required parameter: url"),
        };
        let max_length = args["max_length"].as_u64().unwrap_or(10000) as usize;

        let response = match reqwest::get(&url).await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Failed to fetch URL: {}", e)),
        };

        let text = match response.text().await {
            Ok(t) => t,
            Err(e) => return ToolResult::error(format!("Failed to read response: {}", e)),
        };

        // Strip HTML tags (basic)
        let stripped = strip_html_tags(&text);

        let output = if stripped.len() > max_length {
            format!("{}...\n[Content truncated: {} chars total]", &stripped[..max_length], stripped.len())
        } else {
            stripped
        };

        ToolResult::success(output)
    }
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut _in_script = false;
    for ch in html.chars() {
        match ch {
            '<' => { in_tag = true; }
            '>' => {
                in_tag = false;
                result.push(' ');
            }
            _ if in_tag => {}
            _ => result.push(ch),
        }
    }
    // Collapse whitespace
    let re = regex::Regex::new(r"\s+").unwrap();
    re.replace_all(&result, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let stripped = strip_html_tags(html);
        assert!(stripped.contains("Hello"));
        assert!(stripped.contains("World"));
        assert!(!stripped.contains("<"));
    }
}
