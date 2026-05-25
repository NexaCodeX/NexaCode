use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::llm::traits::{LLMProvider, StreamingResponse};
use crate::llm::types::{
    ChatOptions, ChatResponse, Message, MessageContent, ModelInfo, ProviderConfig, Role,
    StreamChunk, ToolAwareResponse, ToolCall, ToolCallDelta, Usage,
};
use crate::tools::ToolDefinition;

use super::base::BaseProvider;

pub struct OpenAIProvider {
    config: ProviderConfig,
    base: BaseProvider,
}

impl OpenAIProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, anyhow::Error> {
        let base = BaseProvider::new()?;
        Ok(Self { config, base })
    }

    fn get_base_url(&self) -> String {
        self.config
            .base_url
            .as_ref()
            .map(|url| url.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.config.api_key))
                .expect("Invalid API key"),
        );
        headers
    }

    /// Convert our Message to OpenAI's message format
    fn message_to_openai(msg: &Message) -> OpenAIMessage {
        // Handle tool result messages
        if msg.role == Role::Tool {
            if let MessageContent::ToolResult { tool_call_id, content, .. } = &msg.content {
                return OpenAIMessage {
                    role: "tool".to_string(),
                    content: OpenAIMessageContent::Simple(content.clone()),
                    tool_call_id: Some(tool_call_id.clone()),
                    tool_calls: None,
                    multi_content: None,
                };
            }
        }

        // Handle assistant messages with tool calls
        if msg.role == Role::Assistant {
            if let MessageContent::ToolCalls { tool_calls } = &msg.content {
                let openai_tool_calls: Vec<OpenAIToolCall> = tool_calls
                    .iter()
                    .map(|tc| OpenAIToolCall {
                        id: tc.id.clone(),
                        r#type: "function".to_string(),
                        function: FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.to_string(),
                        },
                    })
                    .collect();
                return OpenAIMessage {
                    role: "assistant".to_string(),
                    content: OpenAIMessageContent::Simple(String::new()),
                    tool_call_id: None,
                    tool_calls: Some(openai_tool_calls),
                    multi_content: None,
                };
            }
        }

        // Handle messages with images
        if let Some(images) = &msg.images {
            let mut content = vec![OpenAIMessageContentPart::Text {
                text: msg.text_content().unwrap_or("").to_string(),
            }];
            for img in images {
                content.push(OpenAIMessageContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: img.url.clone(),
                    },
                });
            }
            return OpenAIMessage {
                role: msg.role.clone().into(),
                content: OpenAIMessageContent::Simple(String::new()),
                tool_call_id: None,
                tool_calls: None,
                // We'll handle multi-content via a special field
                multi_content: Some(content),
            };
        }

        // Simple text message
        let text = msg.text_content().unwrap_or("").to_string();
        OpenAIMessage {
            role: msg.role.clone().into(),
            content: OpenAIMessageContent::Simple(text),
            tool_call_id: None,
            tool_calls: None,
            multi_content: None,
        }
    }

    /// Convert our ToolDefinition to OpenAI's tool format
    fn tool_to_openai(tool: &ToolDefinition) -> OpenAITool {
        OpenAITool {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        }
    }
}

// ==========================================
// OpenAI API types
// ==========================================

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: OpenAIMessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multi_content: Option<Vec<OpenAIMessageContentPart>>,
}

impl From<Role> for String {
    fn from(role: Role) -> String {
        match role {
            Role::System => "system".to_string(),
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
            Role::Tool => "tool".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OpenAIMessageContent {
    Simple(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAIMessageContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageUrl {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    r#type: String,
    function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAITool {
    r#type: String,
    function: FunctionDefinition,
}

#[derive(Debug, Serialize)]
struct FunctionDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
}

#[derive(Debug, Deserialize)]
struct ChatResponseData {
    choices: Vec<Choice>,
    model: String,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ResponseMessage>,
    delta: Option<ResponseDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ResponseToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ResponseToolCall {
    id: String,
    #[allow(dead_code)]
    r#type: Option<String>,
    function: ResponseFunctionCall,
}

#[derive(Debug, Deserialize)]
struct ResponseFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ResponseDelta {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    index: usize,
    id: Option<String>,
    #[allow(dead_code)]
    r#type: Option<String>,
    function: Option<DeltaFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct DeltaFunctionCall {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelData>,
}

#[derive(Debug, Deserialize)]
struct ModelData {
    id: String,
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat_stream(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<StreamingResponse, anyhow::Error> {
        let openai_messages: Vec<OpenAIMessage> =
            messages.iter().map(Self::message_to_openai).collect();

        let request = ChatRequest {
            model: options.model.clone(),
            messages: openai_messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            top_p: options.top_p,
            stop: options.stop,
            stream: true,
            tools: None,
        };

        let url = format!("{}/chat/completions", self.get_base_url());
        let headers = self.build_headers();

        log::info!("Stream request URL: {}", url);
        log::info!("Stream request model: {}", options.model);

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        log::info!("Stream response status: {}", response.status());

        let stream = response.bytes_stream().flat_map(|chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    let mut chunks = Vec::new();

                    for line in text.lines() {
                        let line = line.trim();
                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data == "[DONE]" {
                                chunks.push(Ok(StreamChunk {
                                    delta: String::new(),
                                    finish_reason: Some("stop".to_string()),
                                    tool_call_delta: None,
                                }));
                            } else if let Ok(response_data) =
                                serde_json::from_str::<ChatResponseData>(data)
                            {
                                if let Some(choice) = response_data.choices.first() {
                                    if let Some(d) = &choice.delta {
                                        if let Some(reasoning) = &d.reasoning_content {
                                            if !reasoning.is_empty() {
                                                chunks.push(Ok(StreamChunk {
                                                    delta: format!(
                                                        "[THINKING]{}[/THINKING]",
                                                        reasoning
                                                    ),
                                                    finish_reason: None,
                                                    tool_call_delta: None,
                                                }));
                                            }
                                        }
                                        if let Some(content) = &d.content {
                                            if !content.is_empty() {
                                                chunks.push(Ok(StreamChunk {
                                                    delta: content.clone(),
                                                    finish_reason: None,
                                                    tool_call_delta: None,
                                                }));
                                            }
                                        }
                                        // Handle streaming tool call deltas
                                        if let Some(tool_call_deltas) = &d.tool_calls {
                                            for tc_delta in tool_call_deltas {
                                                chunks.push(Ok(StreamChunk {
                                                    delta: String::new(),
                                                    finish_reason: None,
                                                    tool_call_delta: Some(ToolCallDelta {
                                                        index: tc_delta.index,
                                                        id: tc_delta.id.clone(),
                                                        name: tc_delta.function.as_ref().and_then(|f| f.name.clone()),
                                                        arguments_delta: tc_delta.function.as_ref().and_then(|f| f.arguments.clone()),
                                                    }),
                                                }));
                                            }
                                        }
                                    }
                                    if choice.finish_reason.is_some() {
                                        chunks.push(Ok(StreamChunk {
                                            delta: String::new(),
                                            finish_reason: choice.finish_reason.clone(),
                                            tool_call_delta: None,
                                        }));
                                    }
                                }
                            }
                        }
                    }

                    futures::stream::iter(chunks).boxed()
                }
                Err(e) => {
                    log::error!("Stream chunk error: {}", e);
                    futures::stream::iter(vec![Err(anyhow::anyhow!("{}", e))]).boxed()
                }
            }
        });

        Ok(Box::pin(stream))
    }

    async fn chat(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<ChatResponse, anyhow::Error> {
        let openai_messages: Vec<OpenAIMessage> =
            messages.iter().map(Self::message_to_openai).collect();

        let request = ChatRequest {
            model: options.model.clone(),
            messages: openai_messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            top_p: options.top_p,
            stop: options.stop,
            stream: false,
            tools: None,
        };

        let url = format!("{}/chat/completions", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        log::debug!("Chat response status: {}, body length: {}", status, response_text.len());

        let response_data: ChatResponseData = serde_json::from_str(&response_text)?;

        let message = response_data
            .choices
            .first()
            .and_then(|c| c.message.as_ref());

        let mut content = String::new();

        if let Some(msg) = message {
            if let Some(reasoning) = &msg.reasoning_content {
                content.push_str("[THINKING]");
                content.push_str(reasoning);
                content.push_str("[/THINKING]\n\n");
            }
            if let Some(text) = &msg.content {
                content.push_str(text);
            }
        }

        let usage = response_data.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ChatResponse {
            content,
            model: response_data.model,
            usage,
        })
    }

    async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<ToolAwareResponse, anyhow::Error> {
        let openai_messages: Vec<OpenAIMessage> =
            messages.iter().map(Self::message_to_openai).collect();
        let openai_tools: Vec<OpenAITool> = tools.iter().map(Self::tool_to_openai).collect();

        let request = ChatRequest {
            model: options.model.clone(),
            messages: openai_messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            top_p: options.top_p,
            stop: options.stop,
            stream: false,
            tools: if openai_tools.is_empty() {
                None
            } else {
                Some(openai_tools)
            },
        };

        let url = format!("{}/chat/completions", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        log::debug!(
            "Chat with tools response status: {}, body length: {}",
            status,
            response_text.len()
        );

        let response_data: ChatResponseData = serde_json::from_str(&response_text)?;

        let choice = response_data.choices.first();
        let message = choice.and_then(|c| c.message.as_ref());
        let finish_reason = choice.and_then(|c| c.finish_reason.clone());

        let mut content = None;
        let mut tool_calls = Vec::new();

        if let Some(msg) = message {
            if let Some(text) = &msg.content {
                if !text.is_empty() {
                    content = Some(text.clone());
                }
            }
            if let Some(tc_list) = &msg.tool_calls {
                for tc in tc_list {
                    let arguments: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
                    tool_calls.push(ToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments,
                    });
                }
            }
        }

        let usage = response_data.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ToolAwareResponse {
            content,
            tool_calls,
            model: response_data.model,
            usage,
            stop_reason: finish_reason,
        })
    }

    async fn chat_stream_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<StreamingResponse, anyhow::Error> {
        // For now, delegate to chat_stream with tools in the request
        // The stream handler already handles tool call deltas
        let openai_messages: Vec<OpenAIMessage> =
            messages.iter().map(Self::message_to_openai).collect();
        let openai_tools: Vec<OpenAITool> = tools.iter().map(Self::tool_to_openai).collect();

        let request = ChatRequest {
            model: options.model.clone(),
            messages: openai_messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            top_p: options.top_p,
            stop: options.stop,
            stream: true,
            tools: if openai_tools.is_empty() {
                None
            } else {
                Some(openai_tools)
            },
        };

        let url = format!("{}/chat/completions", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        // Reuse the same stream logic as chat_stream (which already handles tool deltas)
        let stream = response.bytes_stream().flat_map(|chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    let mut chunks = Vec::new();

                    for line in text.lines() {
                        let line = line.trim();
                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data == "[DONE]" {
                                chunks.push(Ok(StreamChunk {
                                    delta: String::new(),
                                    finish_reason: Some("stop".to_string()),
                                    tool_call_delta: None,
                                }));
                            } else if let Ok(response_data) =
                                serde_json::from_str::<ChatResponseData>(data)
                            {
                                if let Some(choice) = response_data.choices.first() {
                                    if let Some(d) = &choice.delta {
                                        if let Some(content) = &d.content {
                                            if !content.is_empty() {
                                                chunks.push(Ok(StreamChunk {
                                                    delta: content.clone(),
                                                    finish_reason: None,
                                                    tool_call_delta: None,
                                                }));
                                            }
                                        }
                                        if let Some(tool_call_deltas) = &d.tool_calls {
                                            for tc_delta in tool_call_deltas {
                                                chunks.push(Ok(StreamChunk {
                                                    delta: String::new(),
                                                    finish_reason: None,
                                                    tool_call_delta: Some(ToolCallDelta {
                                                        index: tc_delta.index,
                                                        id: tc_delta.id.clone(),
                                                        name: tc_delta.function.as_ref().and_then(|f| f.name.clone()),
                                                        arguments_delta: tc_delta.function.as_ref().and_then(|f| f.arguments.clone()),
                                                    }),
                                                }));
                                            }
                                        }
                                    }
                                    if choice.finish_reason.is_some() {
                                        chunks.push(Ok(StreamChunk {
                                            delta: String::new(),
                                            finish_reason: choice.finish_reason.clone(),
                                            tool_call_delta: None,
                                        }));
                                    }
                                }
                            }
                        }
                    }

                    futures::stream::iter(chunks).boxed()
                }
                Err(e) => {
                    futures::stream::iter(vec![Err(anyhow::anyhow!("{}", e))]).boxed()
                }
            }
        });

        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, anyhow::Error> {
        let url = format!("{}/models", self.get_base_url());
        let headers = self.build_headers();

        let response = self
            .base
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await?;

        let models_response: ModelsResponse = response.json().await?;

        Ok(models_response
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id,
                name: None,
                description: None,
            })
            .collect())
    }

    fn name(&self) -> &str {
        "OpenAI"
    }
}
