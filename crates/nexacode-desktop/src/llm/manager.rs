use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use nexacode_core::llm::{LLMClient, ProviderConfig};
use nexacode_core::session::SessionStorage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct SavedProvider {
    config: ProviderConfig,
    is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    providers: HashMap<String, SavedProvider>,
}

pub struct LLMManager {
    clients: Arc<RwLock<HashMap<String, Arc<LLMClient>>>>,
    active_provider: Arc<RwLock<Option<String>>>,
    config_path: std::path::PathBuf,
    session_storage: SessionStorage,
    /// Cancellation token for the currently active stream
    stream_cancellation: Arc<RwLock<Option<CancellationToken>>>,
}

impl LLMManager {
    pub fn new() -> Self {
        let config_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".nexacode");

        let config_path = config_dir.join("config.toml");
        let session_storage = SessionStorage::new();

        log::info!("Config file path: {:?}", config_path);

        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            active_provider: Arc::new(RwLock::new(None)),
            config_path,
            session_storage,
            stream_cancellation: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn load_from_disk(&self) -> Result<(), anyhow::Error> {
        if !self.config_path.exists() {
            log::info!("Config file does not exist, creating default");
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.config_path).await?;
        let config: Config = toml::from_str(&content)?;

        let mut clients = self.clients.write().await;
        let mut active = self.active_provider.write().await;

        for (name, saved_provider) in config.providers {
            match LLMClient::new(saved_provider.config) {
                Ok(client) => {
                    clients.insert(name.clone(), Arc::new(client));

                    if saved_provider.is_active {
                        *active = Some(name.clone());
                    }
                    log::info!("Loaded provider: {}", name);
                }
                Err(e) => {
                    log::error!("Failed to load provider '{}': {}", name, e);
                }
            }
        }

        Ok(())
    }

    // ==========================================
    // Stream cancellation
    // ==========================================

    /// Create a new cancellation token for a stream and store it.
    /// Returns the token so the stream handler can use it.
    pub async fn create_stream_cancellation(&self) -> CancellationToken {
        let token = CancellationToken::new();
        let mut stream_cancel = self.stream_cancellation.write().await;
        *stream_cancel = Some(token.clone());
        token
    }

    /// Cancel the currently active stream, if any.
    /// Returns true if a stream was cancelled, false if no active stream.
    pub async fn cancel_stream(&self) -> bool {
        let mut stream_cancel = self.stream_cancellation.write().await;
        if let Some(token) = stream_cancel.take() {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Clear the stream cancellation token (called when stream ends naturally)
    pub async fn clear_stream_cancellation(&self) {
        let mut stream_cancel = self.stream_cancellation.write().await;
        *stream_cancel = None;
    }

    // ==========================================
    // Session management (delegated to SessionStorage)
    // ==========================================

    pub async fn list_sessions(&self) -> Result<Vec<nexacode_core::session::SessionMeta>, anyhow::Error> {
        self.session_storage.list_sessions().await
    }

    pub async fn load_session(&self, session_id: &str) -> Result<nexacode_core::session::Session, anyhow::Error> {
        self.session_storage.load_session(session_id).await
    }

    pub async fn save_session(&self, session: &nexacode_core::session::Session) -> Result<(), anyhow::Error> {
        self.session_storage.save_session(session).await
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<(), anyhow::Error> {
        self.session_storage.delete_session(session_id).await
    }

    // ==========================================
    // Provider management
    // ==========================================

    async fn save_to_disk(&self) -> Result<(), anyhow::Error> {
        let clients = self.clients.read().await;
        let active = self.active_provider.read().await;

        let mut providers = HashMap::new();

        for (name, client) in clients.iter() {
            providers.insert(name.clone(), SavedProvider {
                config: client.get_config().clone(),
                is_active: active.as_ref() == Some(name),
            });
        }

        let config = Config { providers };
        let content = toml::to_string_pretty(&config)?;

        if let Some(parent) = self.config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.config_path, content).await?;
        log::info!("Saved config to {:?}", self.config_path);

        Ok(())
    }

    pub async fn add_provider(&self, name: String, config: ProviderConfig) -> Result<(), anyhow::Error> {
        let client = Arc::new(LLMClient::new(config)?);
        let mut clients = self.clients.write().await;
        clients.insert(name.clone(), client);

        let mut active = self.active_provider.write().await;
        if active.is_none() {
            *active = Some(name);
        }
        drop(active);
        drop(clients);

        self.save_to_disk().await?;

        Ok(())
    }

    pub async fn remove_provider(&self, name: &str) -> Result<(), anyhow::Error> {
        let mut clients = self.clients.write().await;
        clients.remove(name);

        let mut active = self.active_provider.write().await;
        if active.as_ref().map(|n| n == name).unwrap_or(false) {
            *active = clients.keys().next().cloned();
        }
        drop(active);
        drop(clients);

        self.save_to_disk().await?;

        Ok(())
    }

    pub async fn set_active_provider(&self, name: String) -> Result<(), anyhow::Error> {
        let clients = self.clients.read().await;
        if !clients.contains_key(&name) {
            return Err(anyhow::anyhow!("Provider '{}' not found", name));
        }

        let mut active = self.active_provider.write().await;
        *active = Some(name);
        drop(active);
        drop(clients);

        self.save_to_disk().await?;

        Ok(())
    }

    pub async fn get_active_client(&self) -> Result<Arc<LLMClient>, anyhow::Error> {
        let active = self.active_provider.read().await;
        let name = active.as_ref().ok_or_else(|| anyhow::anyhow!("No active provider"))?;

        let clients = self.clients.read().await;
        clients.get(name).cloned().ok_or_else(|| anyhow::anyhow!("Active provider not found"))
    }

    pub async fn list_providers(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }

    pub async fn get_active_provider_name(&self) -> Option<String> {
        self.active_provider.read().await.clone()
    }

    pub async fn get_provider_config(&self, name: &str) -> Option<ProviderConfig> {
        let clients = self.clients.read().await;
        clients.get(name).map(|c| c.get_config().clone())
    }

    pub async fn update_provider(&self, name: String, config: ProviderConfig) -> Result<(), anyhow::Error> {
        let client = Arc::new(LLMClient::new(config)?);
        let mut clients = self.clients.write().await;

        if !clients.contains_key(&name) {
            return Err(anyhow::anyhow!("Provider '{}' not found", name));
        }

        clients.insert(name, client);
        drop(clients);

        self.save_to_disk().await?;

        Ok(())
    }
}

impl Default for LLMManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LLMManager {
    fn clone(&self) -> Self {
        Self {
            clients: Arc::clone(&self.clients),
            active_provider: Arc::clone(&self.active_provider),
            config_path: self.config_path.clone(),
            session_storage: SessionStorage::new(),
            stream_cancellation: Arc::clone(&self.stream_cancellation),
        }
    }
}
