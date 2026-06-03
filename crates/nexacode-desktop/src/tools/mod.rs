use nexacode_core::tools::{ToolContext, ToolRegistry, default_registry};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared state for tool operations
pub struct ToolState {
    pub registry: Arc<RwLock<ToolRegistry>>,
    pub context: Arc<RwLock<ToolContext>>,
}

impl ToolState {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        let registry = default_registry();
        let context = ToolContext::new(working_dir);
        Self {
            registry: Arc::new(RwLock::new(registry)),
            context: Arc::new(RwLock::new(context)),
        }
    }
}

impl Clone for ToolState {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            context: Arc::new(RwLock::new(self.context.blocking_read().clone())),
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
    let res = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new().pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(res.map(|p| p.to_string_lossy().to_string()))
}
