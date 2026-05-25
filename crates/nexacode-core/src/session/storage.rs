use std::path::PathBuf;

use super::types::{Session, SessionMeta};

/// Manages session persistence on disk.
///
/// Directory structure:
/// ```text
/// ~/.nexacode/
///   sessions/
///     <session_id>.json
///     <session_id>.json
///     ...
/// ```
pub struct SessionStorage {
    sessions_dir: PathBuf,
}

impl SessionStorage {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".nexacode");
        let sessions_dir = base_dir.join("sessions");

        log::info!("Session storage directory: {:?}", sessions_dir);

        Self { sessions_dir }
    }

    pub fn with_dir(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    /// Ensure the sessions directory exists
    async fn ensure_dir(&self) -> Result<(), anyhow::Error> {
        tokio::fs::create_dir_all(&self.sessions_dir).await?;
        Ok(())
    }

    /// Get the file path for a session
    fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", session_id))
    }

    /// List all sessions (metadata only, no messages)
    pub async fn list_sessions(&self) -> Result<Vec<SessionMeta>, anyhow::Error> {
        self.ensure_dir().await?;

        let mut entries = tokio::fs::read_dir(&self.sessions_dir).await?;
        let mut metas = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        match serde_json::from_str::<Session>(&content) {
                            Ok(session) => metas.push(session.to_meta()),
                            Err(e) => {
                                log::warn!("Failed to parse session file {:?}: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to read session file {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort by updated_at descending (most recent first)
        metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(metas)
    }

    /// Load a single session by ID (including messages)
    pub async fn load_session(&self, session_id: &str) -> Result<Session, anyhow::Error> {
        let path = self.session_path(session_id);

        if !path.exists() {
            return Err(anyhow::anyhow!("Session '{}' not found", session_id));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let session: Session = serde_json::from_str(&content)?;

        Ok(session)
    }

    /// Save a session to disk
    pub async fn save_session(&self, session: &Session) -> Result<(), anyhow::Error> {
        self.ensure_dir().await?;

        let path = self.session_path(&session.id);
        let content = serde_json::to_string_pretty(session)?;

        tokio::fs::write(&path, content).await?;

        log::info!("Saved session '{}' to {:?}", session.id, path);

        Ok(())
    }

    /// Delete a session from disk
    pub async fn delete_session(&self, session_id: &str) -> Result<(), anyhow::Error> {
        let path = self.session_path(session_id);

        if !path.exists() {
            return Err(anyhow::anyhow!("Session '{}' not found", session_id));
        }

        tokio::fs::remove_file(&path).await?;

        log::info!("Deleted session '{}'", session_id);

        Ok(())
    }
}

impl Default for SessionStorage {
    fn default() -> Self {
        Self::new()
    }
}
