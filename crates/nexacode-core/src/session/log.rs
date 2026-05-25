use std::path::PathBuf;
use tokio::sync::Mutex;

/// Logs LLM interactions for a given session to a file on disk.
///
/// Log files are stored at `~/.nexacode/logs/{session_id}.log`.
/// Each entry is a **pretty-printed** JSON block separated by a horizontal
/// rule, making it easy to read when opened in a text editor.
///
/// The logger uses an internal `Mutex` so that concurrent writes are
/// serialised without blocking the async runtime.
pub struct SessionLogger {
    log_path: PathBuf,
    lock: Mutex<()>,
}

impl SessionLogger {
    /// Create a new logger for the given session.
    /// The log directory will be created if it doesn't exist.
    pub fn new(session_id: &str) -> Self {
        let base_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".nexacode")
            .join("logs");

        let log_path = base_dir.join(format!("{}.log", session_id));

        Self {
            log_path,
            lock: Mutex::new(()),
        }
    }

    /// Ensure the log directory exists.
    async fn ensure_dir(&self) -> Result<(), anyhow::Error> {
        if let Some(parent) = self.log_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        Ok(())
    }

    /// Append a pretty-printed JSON entry to the log file.
    async fn append_entry(&self, entry: &serde_json::Value) {
        let _guard = self.lock.lock().await;

        if let Err(e) = self.ensure_dir().await {
            log::error!("[SessionLogger] Failed to create log dir: {}", e);
            return;
        }

        let pretty = serde_json::to_string_pretty(entry).unwrap_or_default();
        // Separate each entry with a clear divider so they are easy to scan
        let content = format!("{}\n{}\n", "─".repeat(80), pretty);

        use tokio::io::AsyncWriteExt;
        let result = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await;

        match result {
            Ok(mut file) => {
                if let Err(e) = file.write_all(content.as_bytes()).await {
                    log::error!("[SessionLogger] Failed to write log: {}", e);
                }
            }
            Err(e) => {
                log::error!("[SessionLogger] Failed to open log file {:?}: {}", self.log_path, e);
            }
        }
    }

    /// Log the start of an Agent Loop run.
    pub async fn log_run_start(&self, user_message: &str, model: &str) {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "run_start",
            "model": model,
            "user_message": user_message,
        });
        self.append_entry(&entry).await;
    }

    /// Log an LLM request (the full message list being sent).
    pub async fn log_request(
        &self,
        messages: &[crate::llm::types::Message],
        model: &str,
        iteration: usize,
    ) {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "llm_request",
            "iteration": iteration,
            "model": model,
            "messages": messages,
        });
        self.append_entry(&entry).await;
    }

    /// Log an LLM response including the thinking text and any tool calls.
    pub async fn log_response(
        &self,
        response: &crate::llm::types::ToolAwareResponse,
        iteration: usize,
    ) {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "llm_response",
            "iteration": iteration,
            "thinking": response.content,
            "tool_calls": response.tool_calls,
            "usage": response.usage,
            "stop_reason": response.stop_reason,
        });
        self.append_entry(&entry).await;
    }

    /// Log a tool execution result — what the tool returned.
    pub async fn log_tool_result(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
        output: &str,
        is_error: bool,
        iteration: usize,
    ) {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "tool_result",
            "iteration": iteration,
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "arguments": arguments,
            "output": output,
            "is_error": is_error,
        });
        self.append_entry(&entry).await;
    }

    /// Log the final completion of the agent loop.
    pub async fn log_run_completed(&self, content: &str, iterations: usize) {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "run_completed",
            "iterations": iterations,
            "final_content": content,
        });
        self.append_entry(&entry).await;
    }

    /// Log a chat-stream request (simpler than agent — just messages + model).
    pub async fn log_chat_request(
        &self,
        messages: &[crate::llm::types::Message],
        model: &str,
    ) {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "chat_request",
            "model": model,
            "messages": messages,
        });
        self.append_entry(&entry).await;
    }
}
