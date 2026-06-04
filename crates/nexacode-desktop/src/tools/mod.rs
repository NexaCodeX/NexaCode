use nexacode_core::tools::{ToolContext, ToolRegistry, default_registry};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::Mutex;
use tokio::process::Child;
use tokio::process::Command;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tauri::Emitter;

/// Shared state for tool operations
pub struct ToolState {
    pub registry: Arc<RwLock<ToolRegistry>>,
    pub context: Arc<RwLock<ToolContext>>,
    pub current_process: Arc<Mutex<Option<Child>>>,
}

impl ToolState {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        let registry = default_registry();
        let context = ToolContext::new(working_dir);
        Self {
            registry: Arc::new(RwLock::new(registry)),
            context: Arc::new(RwLock::new(context)),
            current_process: Arc::new(Mutex::new(None)),
        }
    }
}

impl Clone for ToolState {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            context: Arc::new(RwLock::new(self.context.blocking_read().clone())),
            current_process: Arc::clone(&self.current_process),
        }
    }
}

impl Default for ToolState {
    fn default() -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        Self::new(working_dir)
    }
}

// ==========================================
// Tauri command types
// ==========================================

#[derive(Debug, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub risk_level: String,
}

#[derive(Debug, Serialize)]
pub struct ToolExecutionResult {
    pub output: String,
    pub is_error: bool,
    pub truncated: Option<bool>,
    pub total_bytes: Option<u64>,
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}

impl From<nexacode_core::tools::ToolResult> for ToolExecutionResult {
    fn from(r: nexacode_core::tools::ToolResult) -> Self {
        Self {
            output: r.output,
            is_error: r.is_error,
            truncated: r.truncated,
            total_bytes: r.total_bytes,
            metadata: r.metadata,
        }
    }
}

// ==========================================
// Tauri commands
// ==========================================

#[tauri::command]
pub async fn tool_list(state: tauri::State<'_, ToolState>) -> Result<Vec<ToolInfo>, String> {
    let registry = state.registry.read().await;
    let tools = registry.definitions();

    let mut result = Vec::new();
    for def in tools {
        if let Some(tool) = registry.get(&def.name) {
            result.push(ToolInfo {
                name: def.name,
                description: def.description,
                parameters: def.parameters,
                risk_level: format!("{:?}", tool.risk_level()),
            });
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn tool_execute(
    state: tauri::State<'_, ToolState>,
    name: String,
    args: serde_json::Value,
) -> Result<ToolExecutionResult, String> {
    let registry = state.registry.read().await;
    let context = state.context.read().await;

    let result = registry
        .execute(&name, args, &context)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result.into())
}

#[tauri::command]
pub async fn tool_requires_confirmation(
    state: tauri::State<'_, ToolState>,
    name: String,
    args: serde_json::Value,
) -> Result<bool, String> {
    let registry = state.registry.read().await;
    Ok(registry.requires_confirmation(&name, &args))
}

#[tauri::command]
pub async fn tool_set_working_dir(
    state: tauri::State<'_, ToolState>,
    path: String,
) -> Result<(), String> {
    let working_dir = std::path::PathBuf::from(&path);
    if !working_dir.exists() {
        return Err(format!("Directory not found: {}", path));
    }
    let mut context = state.context.write().await;
    *context = ToolContext::new(working_dir);
    Ok(())
}

#[tauri::command]
pub async fn tool_get_working_dir(
    state: tauri::State<'_, ToolState>,
) -> Result<String, String> {
    let context = state.context.read().await;
    Ok(context.working_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn select_directory() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .pick_folder()
        .await;
    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn read_file_raw(
    state: tauri::State<'_, ToolState>,
    path: String,
) -> Result<String, String> {
    let context = state.context.read().await;
    let file_path = context.resolve_path(&path);
    if !file_path.exists() {
        return Ok(String::new());
    }
    tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub async fn read_file_backup(
    state: tauri::State<'_, ToolState>,
    session_id: String,
    path: String,
) -> Result<String, String> {
    let context = state.context.read().await;
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let backup_dir = home.join(".nexacode").join("backups").join(&session_id);

    let relative_path = nexacode_core::tools::backup::get_clean_relative_path(
        std::path::Path::new(&path),
        &context.working_dir,
    );

    let backup_path = backup_dir.join(relative_path);
    if !backup_path.exists() {
        return Err("No backup found".to_string());
    }
    tokio::fs::read_to_string(&backup_path)
        .await
        .map_err(|e| format!("Failed to read backup: {}", e))
}

#[tauri::command]
pub async fn terminal_spawn(
    state: tauri::State<'_, ToolState>,
    app: tauri::AppHandle,
    command: String,
) -> Result<(), String> {
    // Kill any existing running process first
    let _ = terminal_kill(state.clone()).await;

    let context = state.context.read().await;
    let working_dir = context.working_dir.clone();

    // Spawn process using default shell sh
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(&working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn process: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to open stderr")?;

    // Store child handle
    let mut current_process = state.current_process.lock().await;
    *current_process = Some(child);

    // Read stdout line by line
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = app_clone.emit("terminal-stdout", line);
        }
    });

    // Read stderr line by line
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = app_clone.emit("terminal-stderr", line);
        }
    });

    // Wait for completion in a background task
    let state_clone = state.inner().clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut current_process = state_clone.current_process.lock().await;
        if let Some(mut child) = current_process.take() {
            // Drop guard to avoid deadlocks
            drop(current_process);
            let status = child.wait().await;
            let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            let _ = app_clone.emit("terminal-exit", code);
            
            let mut current_process = state_clone.current_process.lock().await;
            *current_process = None;
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn terminal_kill(state: tauri::State<'_, ToolState>) -> Result<(), String> {
    let mut current_process = state.current_process.lock().await;
    if let Some(mut child) = current_process.take() {
        let _ = child.kill().await;
    }
    Ok(())
}

