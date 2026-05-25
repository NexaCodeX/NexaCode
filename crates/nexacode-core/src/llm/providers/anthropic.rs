use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::llm::traits::{LLMProvider, StreamingResponse};
use crate::llm::types::{
    ChatOptions, ChatResponse, Message, MessageContent, ModelInfo, ProviderConfig, Role,
    StreamChunk, ToolAwareResponse, ToolCall, ToolCallDelta, Usage,
};
use crate::tools::ToolDefinition;

use super::base::BaseProvider;

pub struct AnthropicProvider {
    config: ProviderConfig,
    base: BaseProvider,
}

impl AnthropicProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, anyhow::Error> {
        let base = BaseProvider::new()?;
        Ok(Self { config, base })
    }

    fn get_base_url(&self) -> String {
        self.config
            .base_url
            .as_ref()
            .map(|url| url.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string())
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.config.api_key).expect("Invalid API key"),
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static("2023-06-01"),
        );
        headers
    }

    /// Convert our Message to Anthropic's format, returning (system_prompt, anthropic_messages)
    fn messages_to_anthropic(
        messages: &[Message],
    ) -> Result<(Option<String>, Vec<AnthropicMessage>), anyhow::Error> {
        let mut system_prompt = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    system_prompt = Some(msg.text_content().unwrap_or("").to_string());
                }
                Role::User => {
                    if let Some(images) = &msg.images {
                        let mut content = vec![AnthropicContent::Text {
                            text: msg.text_content().unwrap_or("").to_string(),
                        }];
                        for img in images {
                            let (media_type, data) = if img.url.starts_with("data:") {
                                let parts: Vec<&str> = img.url.splitn(2, ",").collect();
                                if parts.len() != 2 {
                                    continue;
                                }
                                let mime_parts: Vec<&str> = parts[0]
                                    .split(':')
                                    .nth(1)
                                    .unwrap_or("")
                                    .split(';')
                                    .collect();
                                let mt = mime_parts.first().unwrap_or(&"").to_string();
                                (mt, parts[1].to_string())
                            } else {
                                ("image/jpeg".to_string(), img.url.clone())
                            };
                            content.push(AnthropicContent::Image {
                                source: ImageSource {
                                    type_: "base64".to_string(),
                                    media_type,
                                    data,
                                },
                            });
                        }
                        anthropic_messages.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: AnthropicMessageContent::Multi(content),
                        });
                    } else {
                        anthropic_messages.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: AnthropicMessageContent::Text {
                                text: msg.text_content().unwrap_or("").to_string(),
                            },
                        });
                    }
                }
                Role::Assistant => {
                    match &msg.content {
                        MessageContent::Text { text } => {
                            anthropic_messages.push(AnthropicMessage {
                                role: "assistant".to_string(),
                                content: AnthropicMessageContent::Multi(vec![
                                    AnthropicContent::Text { text: text.clone() },
                                ]),
                            });
                        }
                        MessageContent::ToolCalls { tool_calls } => {
                            let mut blocks: Vec<AnthropicContent> = Vec::new();
                            // If there was text content before tool calls, add it
                            // (Anthropic expects text blocks before tool_use blocks)
                            for tc in tool_calls {
                                blocks.push(AnthropicContent::ToolUse {
                                    id: tc.id.clone(),
                                    name: tc.name.clone(),
                                    input: tc.arguments.clone(),
                                });
                            }
                            anthropic_messages.push(AnthropicMessage {
                                role: "assistant".to_string(),
                                content: AnthropicMessageContent::Multi(blocks),
                            });
                        }
                        _ => {}
                    }
                }
                Role::Tool => {
                    if let MessageContent::ToolResult {
                        tool_call_id,
                        content,
                        is_error,
                    } = &msg.content
                    {
                        anthropic_messages.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: AnthropicMessageContent::Multi(vec![
                                AnthropicContent::ToolResult {
                                    tool_use_id: tool_call_id.clone(),
                                    content: content.clone(),
                                    is_error: *is_error,
                                },
                            ]),
                        });
                    }
                }
            }
        }

        Ok((system_prompt, anthropic_messages))
    }

    fn tool_to_anthropic(tool: &ToolDefinition) -> AnthropicTool {
        AnthropicTool {
            name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: tool.parameters.clone(),
        }
    }
}

// ==========================================
// Anthropic API types
// ==========================================

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicMessageContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicMessageContent {
    Text { text: String },
    Multi(Vec<AnthropicContent>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContent {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    type_: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    model: String,
    usage: AnthropicUsage,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<Delta>,
    content_block: Option<ContentBlockStart>,
    #[allow(dead_code)]
    message: Option<MessageStart>,
    index: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    text: Option<String>,
    stop_reason: Option<String>,
    partial_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContentBlockStart {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    block_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageStart {
    #[allow(dead_code)]
    usage: Option<AnthropicUsage>,
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat_stream(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<StreamingResponse, anyhow::Error> {
        let (system_prompt, anthropic_messages) = Self::messages_to_anthropic(&messages)?;

        let request = ChatRequest {
            model: options.model.clone(),
            messages: anthropic_messages,
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(4096),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop,
            stream: true,
            tools: None,
        };

        let url = format!("{}/messages", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let stream = response.bytes_stream().map(move |chunk_result| {
            let chunk = chunk_result?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        match event.event_type.as_str() {
                            "content_block_delta" => {
                                if let Some(delta) = event.delta {
                                    let text = delta.text.unwrap_or_default();
                                    return Ok(StreamChunk {
                                        delta: text,
                                        finish_reason: None,
                                        tool_call_delta: None,
                                    });
                                }
                            }
                            "message_delta" => {
                                if let Some(delta) = event.delta {
                                    if let Some(stop_reason) = delta.stop_reason {
                                        return Ok(StreamChunk {
                                            delta: String::new(),
                                            finish_reason: Some(stop_reason),
                                            tool_call_delta: None,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            Ok(StreamChunk {
                delta: String::new(),
                finish_reason: None,
                tool_call_delta: None,
            })
        });

        Ok(Box::pin(stream))
    }

    async fn chat(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<ChatResponse, anyhow::Error> {
        let (system_prompt, anthropic_messages) = Self::messages_to_anthropic(&messages)?;

        let request = ChatRequest {
            model: options.model.clone(),
            messages: anthropic_messages,
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(4096),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop,
            stream: false,
            tools: None,
        };

        let url = format!("{}/messages", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let response_data: AnthropicResponse = response.json().await?;

        let content = response_data
            .content
            .iter()
            .filter_map(|block| block.text.clone())
            .collect::<Vec<_>>()
            .join("");

        Ok(ChatResponse {
            content,
            model: response_data.model,
            usage: Some(Usage {
                prompt_tokens: response_data.usage.input_tokens,
                completion_tokens: response_data.usage.output_tokens,
                total_tokens: response_data.usage.input_tokens + response_data.usage.output_tokens,
            }),
        })
    }

    async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<ToolAwareResponse, anyhow::Error> {
        let (system_prompt, anthropic_messages) = Self::messages_to_anthropic(&messages)?;
        let anthropic_tools: Vec<AnthropicTool> = tools.iter().map(Self::tool_to_anthropic).collect();

        let request = ChatRequest {
            model: options.model.clone(),
            messages: anthropic_messages,
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(4096),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop,
            stream: false,
            tools: if anthropic_tools.is_empty() {
                None
            } else {
                Some(anthropic_tools)
            },
        };

        let url = format!("{}/messages", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let response_data: AnthropicResponse = response.json().await?;

        let mut text_content = None;
        let mut tool_calls = Vec::new();

        for block in &response_data.content {
            match block.block_type.as_deref() {
                Some("text") => {
                    if let Some(text) = &block.text {
                        if !text.is_empty() {
                            text_content = Some(text.clone());
                        }
                    }
                }
                Some("tool_use") => {
                    if let (Some(id), Some(name), Some(input)) =
                        (&block.id, &block.name, &block.input)
                    {
                        tool_calls.push(ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: input.clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        let usage = Usage {
            prompt_tokens: response_data.usage.input_tokens,
            completion_tokens: response_data.usage.output_tokens,
            total_tokens: response_data.usage.input_tokens + response_data.usage.output_tokens,
        };

        Ok(ToolAwareResponse {
            content: text_content,
            tool_calls,
            model: response_data.model,
            usage: Some(usage),
            stop_reason: response_data.stop_reason,
        })
    }

    async fn chat_stream_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<StreamingResponse, anyhow::Error> {
        let (system_prompt, anthropic_messages) = Self::messages_to_anthropic(&messages)?;
        let anthropic_tools: Vec<AnthropicTool> = tools.iter().map(Self::tool_to_anthropic).collect();

        let request = ChatRequest {
            model: options.model.clone(),
            messages: anthropic_messages,
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(4096),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop,
            stream: true,
            tools: if anthropic_tools.is_empty() {
                None
            } else {
                Some(anthropic_tools)
            },
        };

        let url = format!("{}/messages", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        // Track tool call state across streaming events
        let stream = response.bytes_stream().map(move |chunk_result| {
            let chunk = chunk_result?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        match event.event_type.as_str() {
                            "content_block_start" => {
                                // New tool call starting
                                if let Some(cb) = &event.content_block {
                                    if cb.block_type.as_deref() == Some("tool_use") {
                                        let index = event.index.unwrap_or(0);
                                        return Ok(StreamChunk {
                                            delta: String::new(),
                                            finish_reason: None,
                                            tool_call_delta: Some(ToolCallDelta {
                                                index,
                                                id: cb.id.clone(),
                                                name: cb.name.clone(),
                                                arguments_delta: None,
                                            }),
                                        });
                                    }
                                }
                            }
                            "content_block_delta" => {
                                if let Some(delta) = &event.delta {
                                    // Text content delta
                                    if let Some(text) = &delta.text {
                                        if !text.is_empty() {
                                            return Ok(StreamChunk {
                                                delta: text.clone(),
                                                finish_reason: None,
                                                tool_call_delta: None,
                                            });
                                        }
                                    }
                                    // Tool input JSON delta
                                    if let Some(partial_json) = &delta.partial_json {
                                        let index = event.index.unwrap_or(0);
                                        return Ok(StreamChunk {
                                            delta: String::new(),
                                            finish_reason: None,
                                            tool_call_delta: Some(ToolCallDelta {
                                                index,
                                                id: None,
                                                name: None,
                                                arguments_delta: Some(partial_json.clone()),
                                            }),
                                        });
                                    }
                                }
                            }
                            "message_delta" => {
                                if let Some(delta) = &event.delta {
                                    if let Some(stop_reason) = &delta.stop_reason {
                                        return Ok(StreamChunk {
                                            delta: String::new(),
                                            finish_reason: Some(stop_reason.clone()),
                                            tool_call_delta: None,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            Ok(StreamChunk {
                delta: String::new(),
                finish_reason: None,
                tool_call_delta: None,
            })
        });

        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, anyhow::Error> {
        Ok(vec![
            ModelInfo {
                id: "claude-3-5-sonnet-20241022".to_string(),
                name: Some("Claude 3.5 Sonnet".to_string()),
                description: Some("Most intelligent model".to_string()),
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: Some("Claude 3.5 Haiku".to_string()),
                description: Some("Fastest model".to_string()),
            },
            ModelInfo {
                id: "claude-3-opus-20240229".to_string(),
                name: Some("Claude 3 Opus".to_string()),
                description: Some("Powerful model".to_string()),
            },
        ])
    }

    fn name(&self) -> &str {
        "Anthropic"
    }
}
