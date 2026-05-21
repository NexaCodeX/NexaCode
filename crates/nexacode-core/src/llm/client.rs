use std::sync::Arc;

use crate::llm::providers::{AnthropicProvider, OpenAIProvider};
use crate::llm::traits::{LLMProvider, StreamingResponse};
use crate::llm::types::{ChatOptions, ChatResponse, Message, ModelInfo, ProviderConfig, ProviderType};

pub struct LLMClient {
    provider: Arc<dyn LLMProvider>,
    config: ProviderConfig,
}

impl LLMClient {
    pub fn new(config: ProviderConfig) -> Result<Self, anyhow::Error> {
        let provider: Arc<dyn LLMProvider> = match &config.provider_type {
            ProviderType::OpenAI | ProviderType::OpenAICompatible => {
                Arc::new(OpenAIProvider::new(config.clone())?)
            }
            ProviderType::Anthropic => {
                Arc::new(AnthropicProvider::new(config.clone())?)
            }
        };
        Ok(Self { provider, config })
    }

    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<StreamingResponse, anyhow::Error> {
        self.provider.chat_stream(messages, options).await
    }

    pub async fn chat(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<ChatResponse, anyhow::Error> {
        self.provider.chat(messages, options).await
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, anyhow::Error> {
        self.provider.list_models().await
    }

    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    pub fn get_config(&self) -> &ProviderConfig {
        &self.config
    }
}
