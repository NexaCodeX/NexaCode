use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::llm::traits::{LLMProvider, StreamingResponse};
use crate::llm::types::{ChatOptions, ChatResponse, Message, ModelInfo, ProviderConfig, StreamChunk, Usage};

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

    fn message_to_openai(msg: &Message) -> OpenAIMessage {
        if let Some(images) = &msg.images {
            let mut content = vec![OpenAIMessageContent::Text {
                text: msg.content.clone(),
            }];
            for img in images {
                content.push(OpenAIMessageContent::ImageUrl {
                    image_url: ImageUrl {
                        url: img.url.clone(),
                    },
                });
            }
            OpenAIMessage {
                role: msg.role.clone(),
                content: OpenAIMessageContent::Multi(content),
            }
        } else {
            OpenAIMessage {
                role: msg.role.clone(),
                content: OpenAIMessageContent::Text {
                    text: msg.content.clone(),
                },
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: crate::llm::types::Role,
    content: OpenAIMessageContent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OpenAIMessageContent {
    Text { text: String },
    Multi(Vec<OpenAIMessageContent>),
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageUrl {
    url: String,
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
}

#[derive(Debug, Deserialize)]
struct ResponseDelta {
    content: Option<String>,
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

        let stream = response.bytes_stream().map(move |chunk_result| {
            let chunk = chunk_result?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if data == "[DONE]" {
                        return Ok(StreamChunk {
                            delta: String::new(),
                            finish_reason: Some("stop".to_string()),
                        });
                    }

                    if let Ok(response_data) = serde_json::from_str::<ChatResponseData>(data) {
                        if let Some(choice) = response_data.choices.first() {
                            let delta = choice
                                .delta
                                .as_ref()
                                .and_then(|d| d.content.clone())
                                .unwrap_or_default();
                            return Ok(StreamChunk {
                                delta,
                                finish_reason: choice.finish_reason.clone(),
                            });
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

        let response_data: ChatResponseData = response.json().await?;

        let content = response_data
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .and_then(|m| m.content.clone())
            .unwrap_or_default();

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
