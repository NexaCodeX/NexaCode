use std::sync::Arc;

use crate::llm::providers::{AnthropicProvider, OpenAIProvider};
use crate::llm::traits::{LLMProvider, StreamingResponse};
use crate::llm::types::{
    ChatOptions, ChatResponse, Message, ModelInfo, ProviderConfig, ProviderType,
    ToolAwareResponse,
};
use crate::tools::ToolDefinition;

pub struct LLMClient {
    provider: Arc<dyn LLMProvider>,
    config: ProviderConfig,
}

impl Clone for LLMClient {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            config: self.config.clone(),
        }
    }
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

    /// Create a client from a pre-built provider (useful for testing with mock providers)
    pub fn from_provider(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            config: ProviderConfig::new(ProviderType::OpenAI, "mock"),
        }
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

    /// Chat with tool definitions — the LLM may respond with text or tool calls
    pub async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<ToolAwareResponse, anyhow::Error> {
        self.provider.chat_with_tools(messages, options, tools).await
    }

    /// Streaming chat with tool definitions
    pub async fn chat_stream_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<StreamingResponse, anyhow::Error> {
        self.provider.chat_stream_with_tools(messages, options, tools).await
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
