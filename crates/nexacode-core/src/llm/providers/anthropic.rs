use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::llm::traits::{LLMProvider, StreamingResponse};
use crate::llm::types::{ChatOptions, ChatResponse, Message, ModelInfo, ProviderConfig, Role, StreamChunk, Usage};

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

    fn message_to_anthropic(msg: &Message) -> Result<AnthropicMessage, anyhow::Error> {
        let role = match msg.role {
            Role::System => {
                return Err(anyhow::anyhow!(
                    "System message should be passed separately to Anthropic API"
                ))
            }
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        if let Some(images) = &msg.images {
            let mut content = vec![AnthropicContent::Text {
                text: msg.content.clone(),
            }];
            for img in images {
                let (media_type, data) = if img.url.starts_with("data:") {
                    let parts: Vec<&str> = img.url.splitn(2, ",").collect();
                    if parts.len() != 2 {
                        continue;
                    }
                    let mime_parts: Vec<&str> = parts[0].split(":").nth(1).unwrap_or("").split(";").collect();
                    let media_type = mime_parts.first().unwrap_or(&"").to_string();
                    (media_type, parts[1].to_string())
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
            Ok(AnthropicMessage {
                role: role.to_string(),
                content: AnthropicMessageContent::Multi(content),
            })
        } else {
            Ok(AnthropicMessage {
                role: role.to_string(),
                content: AnthropicMessageContent::Text {
                    text: msg.content.clone(),
                },
            })
        }
    }
}

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
#[serde(untagged)]
enum AnthropicContent {
    Text { text: String },
    Image { source: ImageSource },
}

#[derive(Debug, Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    type_: String,
    media_type: String,
    data: String,
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
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    model: String,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: Option<String>,
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
    #[allow(dead_code)]
    message: Option<MessageStart>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    text: Option<String>,
    stop_reason: Option<String>,
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
        let mut system_prompt = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            if msg.role == Role::System {
                system_prompt = Some(msg.content);
            } else {
                anthropic_messages.push(Self::message_to_anthropic(&msg)?);
            }
        }

        let request = ChatRequest {
            model: options.model.clone(),
            messages: anthropic_messages,
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(4096),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop,
            stream: true,
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
                                    });
                                }
                            }
                            "message_delta" => {
                                if let Some(delta) = event.delta {
                                    if let Some(stop_reason) = delta.stop_reason {
                                        return Ok(StreamChunk {
                                            delta: String::new(),
                                            finish_reason: Some(stop_reason),
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
            })
        });

        Ok(Box::pin(stream))
    }

    async fn chat(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<ChatResponse, anyhow::Error> {
        let mut system_prompt = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            if msg.role == Role::System {
                system_prompt = Some(msg.content);
            } else {
                anthropic_messages.push(Self::message_to_anthropic(&msg)?);
            }
        }

        let request = ChatRequest {
            model: options.model.clone(),
            messages: anthropic_messages,
            system: system_prompt,
            max_tokens: options.max_tokens.unwrap_or(4096),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: options.stop,
            stream: false,
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
