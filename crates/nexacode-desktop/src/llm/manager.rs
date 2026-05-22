use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use nexacode_core::llm::{LLMClient, ProviderConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub date: String,
    pub messages: Vec<ChatMessage>,
}

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
    chats_path: std::path::PathBuf,
}

impl LLMManager {
    pub fn new() -> Self {
        let config_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".nexacode");
        
        let config_path = config_dir.join("config.toml");
        let chats_path = config_dir.join("chats.json");
        
        log::info!("Config file path: {:?}", config_path);
        log::info!("Chats file path: {:?}", chats_path);
        
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            active_provider: Arc::new(RwLock::new(None)),
            config_path,
            chats_path,
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

    pub async fn load_chats_from_disk(&self) -> Result<Vec<Chat>, anyhow::Error> {
        if !self.chats_path.exists() {
            log::info!("Chats file does not exist, returning empty list");
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&self.chats_path).await?;
        let chats: Vec<Chat> = serde_json::from_str(&content)?;

        log::info!("Loaded {} chats from disk", chats.len());
        Ok(chats)
    }

    pub async fn save_chats_to_disk(&self, chats: Vec<Chat>) -> Result<(), anyhow::Error> {
        let content = serde_json::to_string_pretty(&chats)?;

        if let Some(parent) = self.chats_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.chats_path, content).await?;
        log::info!("Saved {} chats to {:?}", chats.len(), self.chats_path);

        Ok(())
    }

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
            chats_path: self.chats_path.clone(),
        }
    }
}
