use serde::{Deserialize, Serialize};

/// Serialized tool call info from an agent step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolCallData {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub requires_confirmation: bool,
}

/// Serialized tool result from an agent step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolResultData {
    pub tool_call_id: String,
    pub name: String,
    pub output: String,
    pub is_error: bool,
}

/// A single agent step that can be persisted and restored in the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStepData {
    pub id: String,
    pub thinking: Option<String>,
    pub tool_call: Option<AgentToolCallData>,
    pub tool_result: Option<AgentToolResultData>,
    pub status: String,
}

/// A single chat message, optionally carrying agent steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    /// Agent execution steps associated with this assistant message.
    /// Only present when role == "assistant" and the message was produced
    /// by the Agent (Build) mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<AgentStepData>,
}

/// A full chat session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<ChatMessage>,
}

/// Lightweight metadata for listing sessions (no messages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
}

impl Session {
    pub fn new(id: String, title: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id,
            title,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn to_meta(&self) -> SessionMeta {
        SessionMeta {
            id: self.id.clone(),
            title: self.title.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            message_count: self.messages.len(),
        }
    }
}
