use tauri::{command, Manager};

mod llm;

#[command]
fn greet(name: &str) -> String {
    format!("你好, {}! 欢迎使用 NexaCode 🦀", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let llm_manager = llm::LLMManager::new();

    tauri::Builder::default()
        .manage(llm_manager)
         .invoke_handler(tauri::generate_handler![
             greet,
             llm::add_provider,
             llm::remove_provider,
             llm::set_active_provider,
             llm::list_providers,
             llm::get_active_provider,
             llm::chat,
             llm::chat_stream,
             llm::list_models,
             llm::load_providers,
             llm::get_provider_config,
             llm::update_provider,
             llm::load_chats,
             llm::save_chats
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
