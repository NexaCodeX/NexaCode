use tauri::{command, AppHandle, Emitter};
use nexacode_core::llm::{ChatOptions, Message, ProviderConfig, ProviderType};
use nexacode_core::session::{Session, SessionMeta, SessionLogger};
use futures::StreamExt;

use super::manager::LLMManager;

#[command]
pub async fn load_providers(
    manager: tauri::State<'_, LLMManager>,
) -> Result<(), String> {
    manager
        .load_from_disk()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn add_provider(
    manager: tauri::State<'_, LLMManager>,
    name: String,
    provider_type: String,
    api_key: String,
    base_url: Option<String>,
    models: Vec<String>,
) -> Result<(), String> {
    let provider_type = match provider_type.as_str() {
        "openai" => ProviderType::OpenAI,
        "openai_compatible" => ProviderType::OpenAICompatible,
        "anthropic" => ProviderType::Anthropic,
        _ => return Err(format!("Unknown provider type: {}", provider_type)),
    };

    let mut config = ProviderConfig::new(provider_type, api_key);
    if let Some(url) = base_url {
        config = config.with_base_url(url);
    }
    if !models.is_empty() {
        config = config.with_models(models);
    }

    manager
        .add_provider(name, config)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn remove_provider(
    manager: tauri::State<'_, LLMManager>,
    name: String,
) -> Result<(), String> {
    manager
        .remove_provider(&name)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn set_active_provider(
    manager: tauri::State<'_, LLMManager>,
    name: String,
) -> Result<(), String> {
    manager
        .set_active_provider(name)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn list_providers(
    manager: tauri::State<'_, LLMManager>,
) -> Result<Vec<String>, String> {
    Ok(manager.list_providers().await)
}

#[command]
pub async fn get_active_provider(
    manager: tauri::State<'_, LLMManager>,
) -> Result<Option<String>, String> {
    Ok(manager.get_active_provider_name().await)
}

#[command]
pub async fn chat(
    manager: tauri::State<'_, LLMManager>,
    messages: Vec<ChatMessage>,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<ChatResponse, String> {
    let client = manager
        .get_active_client()
        .await
        .map_err(|e| e.to_string())?;

    let messages: Vec<Message> = messages
        .into_iter()
        .map(|m| Message::new(m.role.into(), nexacode_core::llm::MessageContent::text(m.content)))
        .collect();

    let mut options = ChatOptions::new(model);
    if let Some(temp) = temperature {
        options = options.with_temperature(temp);
    }
    if let Some(tokens) = max_tokens {
        options = options.with_max_tokens(tokens);
    }

    let response = client.chat(messages, options).await.map_err(|e| e.to_string())?;

    Ok(ChatResponse {
        content: response.content,
        model: response.model,
        usage: response.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }),
    })
}

#[command]
pub async fn chat_stream(
    app: AppHandle,
    manager: tauri::State<'_, LLMManager>,
    messages: Vec<ChatMessage>,
    model: String,
    session_id: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<(), String> {
    log::info!("[chat_stream] Called with {} messages, model={}", messages.len(), model);

    let client = manager
        .get_active_client()
        .await
        .map_err(|e| {
            log::error!("[chat_stream] Failed to get active client: {}", e);
            e.to_string()
        })?;

    let messages: Vec<Message> = messages
        .into_iter()
        .map(|m| Message::new(m.role.into(), nexacode_core::llm::MessageContent::text(m.content)))
        .collect();

    // Log the chat request to session log if session_id provided
    if let Some(ref sid) = session_id {
        let logger = SessionLogger::new(sid);
        logger.log_chat_request(&messages, &model).await;
    }

    let mut options = ChatOptions::new(model).with_stream(true);
    if let Some(temp) = temperature {
        options = options.with_temperature(temp);
    }
    if let Some(tokens) = max_tokens {
        options = options.with_max_tokens(tokens);
    }

    let stream = client
        .chat_stream(messages, options)
        .await
        .map_err(|e| e.to_string())?;

    // Create a cancellation token for this stream
    let cancel_token = manager.create_stream_cancellation().await;
    let manager_clone = manager.inner().clone();

    tokio::spawn(async move {
        let mut stream = stream;
        loop {
            // Fast-path: check if already cancelled before entering select
            if cancel_token.is_cancelled() {
                let _ = app.emit("chat-end", ());
                manager_clone.clear_stream_cancellation().await;
                return;
            }

            tokio::select! {
                biased;

                // Priority 1: Check cancellation first
                _ = cancel_token.cancelled() => {
                    // Stream was cancelled by user — stop immediately, no more events
                    let _ = app.emit("chat-end", ());
                    manager_clone.clear_stream_cancellation().await;
                    return;
                }

                // Priority 2: Process stream chunks
                chunk_result = stream.next() => {
                    match chunk_result {
                        Some(Ok(chunk)) => {
                            // Double-check cancellation before emitting
                            if cancel_token.is_cancelled() {
                                let _ = app.emit("chat-end", ());
                                manager_clone.clear_stream_cancellation().await;
                                return;
                            }
                            let _ = app.emit("chat-chunk", &chunk);
                            if chunk.finish_reason.is_some() {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            let _ = app.emit("chat-error", e.to_string());
                            break;
                        }
                        None => break,
                    }
                }
            }
        }
        // Stream completed naturally
        manager_clone.clear_stream_cancellation().await;
        let _ = app.emit("chat-end", ());
    });

    Ok(())
}

#[command]
pub async fn list_models(
    manager: tauri::State<'_, LLMManager>,
) -> Result<Vec<ModelInfo>, String> {
    let client = manager
        .get_active_client()
        .await
        .map_err(|e| e.to_string())?;

    let models = client.list_models().await.map_err(|e| e.to_string())?;

    Ok(models
        .into_iter()
        .map(|m| ModelInfo {
            id: m.id,
            name: m.name,
            description: m.description,
        })
        .collect())
}

#[command]
pub async fn get_provider_config(
    manager: tauri::State<'_, LLMManager>,
    name: String,
) -> Result<ProviderConfigResponse, String> {
    let config = manager
        .get_provider_config(&name)
        .await
        .ok_or_else(|| format!("Provider '{}' not found", name))?;

    Ok(ProviderConfigResponse {
        provider_type: match config.provider_type {
            ProviderType::OpenAI => "openai".to_string(),
            ProviderType::OpenAICompatible => "openai_compatible".to_string(),
            ProviderType::Anthropic => "anthropic".to_string(),
        },
        api_key: config.api_key,
        base_url: config.base_url,
        models: config.models,
    })
}

#[command]
pub async fn update_provider(
    manager: tauri::State<'_, LLMManager>,
    name: String,
    provider_type: String,
    api_key: String,
    base_url: Option<String>,
    models: Vec<String>,
) -> Result<(), String> {
    let provider_type = match provider_type.as_str() {
        "openai" => ProviderType::OpenAI,
        "openai_compatible" => ProviderType::OpenAICompatible,
        "anthropic" => ProviderType::Anthropic,
        _ => return Err(format!("Unknown provider type: {}", provider_type)),
    };

    let mut config = ProviderConfig::new(provider_type, api_key);
    if let Some(url) = base_url {
        config = config.with_base_url(url);
    }
    if !models.is_empty() {
        config = config.with_models(models);
    }

    manager
        .update_provider(name, config)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn chat_stream_cancel(
    manager: tauri::State<'_, LLMManager>,
) -> Result<bool, String> {
    let cancelled = manager.cancel_stream().await;
    Ok(cancelled)
}

// ==========================================
// Session commands
// ==========================================

#[command]
pub async fn list_sessions(
    manager: tauri::State<'_, LLMManager>,
) -> Result<Vec<SessionMeta>, String> {
    manager
        .list_sessions()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn load_session(
    manager: tauri::State<'_, LLMManager>,
    session_id: String,
) -> Result<Session, String> {
    manager
        .load_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn save_session(
    manager: tauri::State<'_, LLMManager>,
    session: Session,
) -> Result<(), String> {
    manager
        .save_session(&session)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn delete_session(
    manager: tauri::State<'_, LLMManager>,
    session_id: String,
) -> Result<(), String> {
    manager
        .delete_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

// ==========================================
// Shared types for Tauri commands
// ==========================================

#[derive(Debug, serde::Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl From<MessageRole> for nexacode_core::llm::Role {
    fn from(role: MessageRole) -> Self {
        match role {
            MessageRole::System => nexacode_core::llm::Role::System,
            MessageRole::User => nexacode_core::llm::Role::User,
            MessageRole::Assistant => nexacode_core::llm::Role::Assistant,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, serde::Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ProviderConfigResponse {
    pub provider_type: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub models: Vec<String>,
}
