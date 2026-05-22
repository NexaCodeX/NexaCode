use tauri::{command, AppHandle, Emitter};
use nexacode_core::llm::{ChatOptions, Message, ProviderConfig, ProviderType};
use futures::StreamExt;

use super::manager::{LLMManager, Chat};
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
        .map(|m| Message::new(m.role.into(), m.content))
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
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<(), String> {
    let client = manager
        .get_active_client()
        .await
        .map_err(|e| e.to_string())?;

    let messages: Vec<Message> = messages
        .into_iter()
        .map(|m| Message::new(m.role.into(), m.content))
        .collect();

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

    tokio::spawn(async move {
        let mut stream = stream;
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let _ = app.emit("chat-chunk", &chunk);
                    if chunk.finish_reason.is_some() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = app.emit("chat-error", e.to_string());
                    break;
                }
            }
        }
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

#[command]
pub async fn load_chats(
    manager: tauri::State<'_, LLMManager>,
) -> Result<Vec<Chat>, String> {
    manager
        .load_chats_from_disk()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn save_chats(
    manager: tauri::State<'_, LLMManager>,
    chats: Vec<Chat>,
) -> Result<(), String> {
    manager
        .save_chats_to_disk(chats)
        .await
        .map_err(|e| e.to_string())
}
