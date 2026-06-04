use nexacode_core::agent::{AgentConfig, AgentEvent, AgentLoop};
use nexacode_core::session::SessionLogger;
use nexacode_core::llm::Message;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::Emitter;

use super::tools::ToolState;
use super::llm::LLMManager;
use std::sync::atomic::AtomicBool;

pub struct AgentState {
    pub is_cancelled: Arc<AtomicBool>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            is_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Clone for AgentState {
    fn clone(&self) -> Self {
        Self {
            is_cancelled: Arc::clone(&self.is_cancelled),
        }
    }
}

// ==========================================
// Tauri command types
// ==========================================

/// Event emitted to the frontend during agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEventInfo {
    Thinking { content: String },
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
        requires_confirmation: bool,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        output: String,
        is_error: bool,
    },
    Completed { content: String },
    MaxIterationsReached { iterations: usize },
    Error { message: String },
}

impl From<AgentEvent> for AgentEventInfo {
    fn from(event: AgentEvent) -> Self {
        match event {
            AgentEvent::Thinking { content } => AgentEventInfo::Thinking { content },
            AgentEvent::ToolCall {
                id,
                name,
                arguments,
                requires_confirmation,
            } => AgentEventInfo::ToolCall {
                id,
                name,
                arguments,
                requires_confirmation,
            },
            AgentEvent::ToolResult {
                tool_call_id,
                name,
                result,
            } => AgentEventInfo::ToolResult {
                tool_call_id,
                name,
                output: result.output,
                is_error: result.is_error,
            },
            AgentEvent::Completed { content } => AgentEventInfo::Completed { content },
            AgentEvent::MaxIterationsReached {
                iterations,
                partial_response: _,
            } => AgentEventInfo::MaxIterationsReached { iterations },
            AgentEvent::Error { message } => AgentEventInfo::Error { message },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AgentRunRequest {
    pub session_id: Option<String>,
    pub message: Option<String>,
    pub messages: Option<Vec<super::llm::ChatMessage>>,
    pub model: String,
    pub system_prompt: Option<String>,
    pub max_iterations: Option<usize>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

// ==========================================
// Tauri commands
// ==========================================

/// Run the agent loop in the background, streaming events to the frontend.
/// Returns immediately; the frontend listens to `agent-event` and `agent-end` events.
/// Each event is emitted in real-time as it happens (not batched at the end).
#[tauri::command]
pub async fn agent_run(
    app: tauri::AppHandle,
    llm_manager: tauri::State<'_, LLMManager>,
    tool_state: tauri::State<'_, ToolState>,
    agent_state: tauri::State<'_, AgentState>,
    request: AgentRunRequest,
) -> Result<(), String> {
    agent_state.is_cancelled.store(false, std::sync::atomic::Ordering::SeqCst);
    let log_msg = request.messages.as_ref()
        .and_then(|m| m.last().map(|msg| &msg.content))
        .or(request.message.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("");

    log::info!("[agent_run] Called with message: {:?}, model: {:?}", 
        &log_msg[..log_msg.len().min(80)],
        request.model
    );

    // Get the active LLM client
    let client = llm_manager
        .get_active_client()
        .await
        .map_err(|e| {
            log::error!("[agent_run] Failed to get active client: {}", e);
            e.to_string()
        })?;

    let client = Arc::new((*client).clone());

    // Get tool registry and context
    let registry = tool_state.registry.read().await.clone();
    let mut context = tool_state.context.read().await.clone();
    context.session_id = request.session_id.clone();

    log::info!("[agent_run] Registry has {} tools", registry.len());

    // Build agent config
    let mut config = AgentConfig::new();
    if let Some(prompt) = request.system_prompt {
        config = config.with_system_prompt(prompt);
    }
    if let Some(max_iter) = request.max_iterations {
        config = config.with_max_iterations(max_iter);
    }
    if let Some(temp) = request.temperature {
        config = config.with_temperature(temp);
    }
    if let Some(tokens) = request.max_tokens {
        config = config.with_max_tokens(tokens);
    }

    // Create the agent loop
    let agent = AgentLoop::new(client, Arc::new(registry), Arc::new(context)).with_config(config);

    let model = request.model;
    let session_logger = request.session_id.map(|id| SessionLogger::new(&id));

    // Construct message history
    let history: Vec<Message> = if let Some(msgs) = request.messages {
        msgs.into_iter()
            .map(|m| Message::new(m.role.into(), nexacode_core::llm::MessageContent::text(m.content)))
            .collect()
    } else {
        vec![Message::user(request.message.unwrap_or_default())]
    };

    let is_cancelled = Arc::clone(&agent_state.is_cancelled);
    // Spawn a background task that runs the agent and emits events AS THEY HAPPEN
    tokio::spawn(async move {
        log::info!("[agent_run] Starting agent loop with {} messages, model: {:?}", 
            history.len(),
            model
        );

        let is_cancelled_clone = Arc::clone(&is_cancelled);
        agent.run_streaming(history, &model, |event| {
            let event_info: AgentEventInfo = event.into();
            log::info!("[agent_run] Emitting event: type={}", 
                serde_json::to_string(&event_info)
                    .ok()
                    .and_then(|s| s.get(9..30).map(|x| x.to_string()))
                    .unwrap_or_default()
            );
            let _ = app.emit("agent-event", &event_info);
        }, session_logger, move || {
            is_cancelled_clone.load(std::sync::atomic::Ordering::SeqCst)
        }).await;

        log::info!("[agent_run] Agent loop finished, emitting agent-end");

        // Signal that the agent has finished
        let _ = app.emit("agent-end", ());
    });

    Ok(())
}

#[tauri::command]
pub async fn agent_cancel(
    agent_state: tauri::State<'_, AgentState>,
) -> Result<(), String> {
    log::info!("[agent_cancel] Set cancellation flag to true");
    agent_state.is_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn agent_step(
    llm_manager: tauri::State<'_, LLMManager>,
    tool_state: tauri::State<'_, ToolState>,
    messages: serde_json::Value,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<serde_json::Value, String> {
    let client = llm_manager
        .get_active_client()
        .await
        .map_err(|e| e.to_string())?;
    let client = Arc::new((*client).clone());

    let registry = tool_state.registry.read().await.clone();
    let context = tool_state.context.read().await.clone();

    // Parse messages from JSON
    let parsed_messages: Vec<nexacode_core::llm::types::Message> =
        serde_json::from_value(messages).map_err(|e| format!("Invalid messages: {}", e))?;

    let mut config = AgentConfig::new();
    if let Some(temp) = temperature {
        config = config.with_temperature(temp);
    }
    if let Some(tokens) = max_tokens {
        config = config.with_max_tokens(tokens);
    }

    let agent = AgentLoop::new(client, Arc::new(registry), Arc::new(context)).with_config(config);

    let result = agent.step(parsed_messages, &model).await.map_err(|e| e.to_string())?;

    serde_json::to_value(&result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn agent_rollback(
    tool_state: tauri::State<'_, ToolState>,
    session_id: String,
) -> Result<(), String> {
    let context = tool_state.context.read().await.clone();
    nexacode_core::tools::backup::rollback_session(&session_id, &context).await
}

