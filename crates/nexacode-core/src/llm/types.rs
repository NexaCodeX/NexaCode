use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "tool")]
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A single tool call requested by the LLM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique ID for this tool call (used to match results)
    pub id: String,
    /// The name of the tool to call
    pub name: String,
    /// The arguments as a JSON string
    pub arguments: serde_json::Value,
}

impl ToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

/// The content of a message — can be plain text, tool calls, or a tool result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    /// Plain text content
    Text {
        text: String,
    },
    /// LLM requesting one or more tool calls
    ToolCalls {
        tool_calls: Vec<ToolCall>,
    },
    /// Result of a tool execution (sent back to the LLM)
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },
}

impl MessageContent {
    pub fn text(content: impl Into<String>) -> Self {
        MessageContent::Text { text: content.into() }
    }

    pub fn tool_calls(calls: Vec<ToolCall>) -> Self {
        MessageContent::ToolCalls { tool_calls: calls }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self {
        MessageContent::ToolResult {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error,
        }
    }

    /// Get text content if this is a Text variant
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Get tool calls if this is a ToolCalls variant
    pub fn as_tool_calls(&self) -> Option<&Vec<ToolCall>> {
        match self {
            MessageContent::ToolCalls { tool_calls } => Some(tool_calls),
            _ => None,
        }
    }
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ImageContent>>,
    /// The name of the tool that produced this result (for Tool role)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    pub fn new(role: Role, content: MessageContent) -> Self {
        Self {
            role,
            content,
            images: None,
            name: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, MessageContent::text(content))
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, MessageContent::text(content))
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, MessageContent::text(content))
    }

    pub fn assistant_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self::new(Role::Assistant, MessageContent::tool_calls(tool_calls))
    }

    pub fn tool_result(tool_call_id: impl Into<String>, name: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self {
        let mut msg = Self::new(Role::Tool, MessageContent::tool_result(tool_call_id, content, is_error));
        msg.name = Some(name.into());
        msg
    }

    pub fn with_images(mut self, images: Vec<ImageContent>) -> Self {
        self.images = Some(images);
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Convenience: get text content if available
    pub fn text_content(&self) -> Option<&str> {
        self.content.as_text()
    }

    /// Convenience: get tool calls if available
    pub fn tool_calls(&self) -> Option<&Vec<ToolCall>> {
        self.content.as_tool_calls()
    }
}

/// The response from an LLM that may contain text or tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAwareResponse {
    /// The text content (if LLM responded with text)
    pub content: Option<String>,
    /// Tool calls requested by the LLM (if LLM wants to call tools)
    pub tool_calls: Vec<ToolCall>,
    /// The model used
    pub model: String,
    /// Token usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// Stop reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

impl ToolAwareResponse {
    /// Check if this response contains tool calls
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// Get the text content, or empty string
    pub fn text_or_empty(&self) -> &str {
        self.content.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOptions {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl ChatOptions {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: None,
        }
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub delta: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// Tool call delta (for streaming tool calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_delta: Option<ToolCallDelta>,
}

/// Streaming delta for a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments_delta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderType {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "openai_compatible")]
    OpenAICompatible,
    #[serde(rename = "anthropic")]
    Anthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

impl ProviderConfig {
    pub fn new(
        provider_type: ProviderType,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            provider_type,
            api_key: api_key.into(),
            base_url: None,
            models: Vec::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    pub fn openai(api_key: impl Into<String>) -> Self {
        Self::new(ProviderType::OpenAI, api_key)
            .with_models(vec![
                "gpt-4".to_string(),
                "gpt-4o".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ])
    }

    pub fn anthropic(api_key: impl Into<String>) -> Self {
        Self::new(ProviderType::Anthropic, api_key)
            .with_models(vec![
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-5-haiku-20241022".to_string(),
                "claude-3-opus-20240229".to_string(),
            ])
    }

    pub fn openai_compatible(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self::new(ProviderType::OpenAICompatible, api_key)
            .with_base_url(base_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_system() {
        let msg = Message::system("You are a helpful assistant");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.text_content(), Some("You are a helpful assistant"));
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("Hello!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.text_content(), Some("Hello!"));
    }

    #[test]
    fn test_message_assistant_text() {
        let msg = Message::assistant("Hi there!");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.text_content(), Some("Hi there!"));
        assert!(msg.tool_calls().is_none());
    }

    #[test]
    fn test_message_assistant_tool_calls() {
        let calls = vec![
            ToolCall::new("call_1", "Read", serde_json::json!({"path": "test.rs"})),
            ToolCall::new("call_2", "Bash", serde_json::json!({"command": "ls"})),
        ];
        let msg = Message::assistant_tool_calls(calls.clone());
        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.text_content().is_none());
        assert_eq!(msg.tool_calls().unwrap().len(), 2);
        assert_eq!(msg.tool_calls().unwrap()[0].name, "Read");
    }

    #[test]
    fn test_message_tool_result() {
        let msg = Message::tool_result("call_1", "Read", "file contents here", false);
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.name, Some("Read".to_string()));
        match &msg.content {
            MessageContent::ToolResult { tool_call_id, content, is_error } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(content, "file contents here");
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult content"),
        }
    }

    #[test]
    fn test_message_tool_result_error() {
        let msg = Message::tool_result("call_2", "Bash", "command failed", true);
        match &msg.content {
            MessageContent::ToolResult { is_error, .. } => assert!(*is_error),
            _ => panic!("Expected ToolResult content"),
        }
    }

    #[test]
    fn test_tool_aware_response() {
        let response = ToolAwareResponse {
            content: Some("Hello!".to_string()),
            tool_calls: vec![],
            model: "gpt-4o".to_string(),
            usage: None,
            stop_reason: Some("stop".to_string()),
        };
        assert!(!response.has_tool_calls());
        assert_eq!(response.text_or_empty(), "Hello!");
    }

    #[test]
    fn test_tool_aware_response_with_tool_calls() {
        let response = ToolAwareResponse {
            content: None,
            tool_calls: vec![ToolCall::new("call_1", "Read", serde_json::json!({}))],
            model: "gpt-4o".to_string(),
            usage: None,
            stop_reason: Some("tool_calls".to_string()),
        };
        assert!(response.has_tool_calls());
        assert_eq!(response.text_or_empty(), "");
    }

    #[test]
    fn test_tool_call_new() {
        let tc = ToolCall::new("id_123", "Write", serde_json::json!({"path": "test.rs", "content": "hello"}));
        assert_eq!(tc.id, "id_123");
        assert_eq!(tc.name, "Write");
        assert_eq!(tc.arguments["path"], "test.rs");
    }

    #[test]
    fn test_message_content_variants() {
        let text = MessageContent::text("hello");
        assert!(text.as_text().is_some());
        assert!(text.as_tool_calls().is_none());

        let tc = MessageContent::tool_calls(vec![ToolCall::new("1", "Read", serde_json::json!({}))]);
        assert!(tc.as_text().is_none());
        assert!(tc.as_tool_calls().is_some());

        let tr = MessageContent::tool_result("1", "result", false);
        assert!(tr.as_text().is_none());
        assert!(tr.as_tool_calls().is_none());
    }

    #[test]
    fn test_serde_roundtrip_message() {
        let msg = Message::assistant_tool_calls(vec![
            ToolCall::new("call_1", "Read", serde_json::json!({"path": "main.rs"})),
        ]);
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.role, Role::Assistant);
        assert!(decoded.tool_calls().is_some());
    }
}
