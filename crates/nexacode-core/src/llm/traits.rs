use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use super::types::{ChatOptions, ChatResponse, Message, ModelInfo, StreamChunk};

pub type StreamingResponse = Pin<Box<dyn Stream<Item = Result<StreamChunk, anyhow::Error>> + Send>>;

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat_stream(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<StreamingResponse, anyhow::Error>;

    async fn chat(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<ChatResponse, anyhow::Error>;

    async fn list_models(&self) -> Result<Vec<ModelInfo>, anyhow::Error>;

    fn name(&self) -> &str;
}
