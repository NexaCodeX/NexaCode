use tauri::{command, Manager};

mod agent;
mod llm;
mod tools;

#[command]
fn greet(name: &str) -> String {
    format!("你好, {}! 欢迎使用 NexaCode 🦀", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let llm_manager = llm::LLMManager::new();
    let tool_state = tools::ToolState::default();

    tauri::Builder::default()
        .manage(llm_manager)
        .manage(tool_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            // LLM commands
            llm::add_provider,
            llm::remove_provider,
            llm::set_active_provider,
            llm::list_providers,
            llm::get_active_provider,
            llm::chat,
            llm::chat_stream,
            llm::chat_stream_cancel,
            llm::list_models,
            llm::load_providers,
            llm::get_provider_config,
            llm::update_provider,
            // Session commands
            llm::list_sessions,
            llm::load_session,
            llm::save_session,
            llm::delete_session,
            // Tool commands
            tools::tool_list,
            tools::tool_execute,
            tools::tool_requires_confirmation,
            tools::tool_set_working_dir,
            tools::tool_get_working_dir,
            tools::select_directory,
            // Agent commands
            agent::agent_run,
            agent::agent_step
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let manager = app.state::<llm::LLMManager>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = manager.load_from_disk().await {
                    log::error!("Failed to load providers from disk: {}", e);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
