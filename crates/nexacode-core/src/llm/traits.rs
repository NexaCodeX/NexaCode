use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use super::types::{ChatOptions, ChatResponse, Message, ModelInfo, StreamChunk, ToolAwareResponse};
use crate::tools::ToolDefinition;

pub type StreamingResponse = Pin<Box<dyn Stream<Item = Result<StreamChunk, anyhow::Error>> + Send>>;

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Chat without tools (basic conversation)
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

    /// Chat with tool definitions — the LLM may respond with text or tool calls
    async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<ToolAwareResponse, anyhow::Error>;

    /// Streaming chat with tools — same as chat_stream but with tool definitions
    /// Tool call deltas are included in StreamChunk::tool_call_delta
    async fn chat_stream_with_tools(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
        tools: Vec<ToolDefinition>,
    ) -> Result<StreamingResponse, anyhow::Error>;
}
