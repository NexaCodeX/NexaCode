use std::path::{Path, PathBuf};
use super::types::ToolContext;

/// Lexically clean all "." and ".." segments from a path.
/// Does not access disk, works for non-existent paths.
pub fn clean_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                clean.pop();
            }
            Component::Normal(c) => {
                clean.push(c);
            }
            Component::CurDir => {}
            Component::RootDir => {
                clean.push(Component::RootDir.as_os_str());
            }
            Component::Prefix(p) => {
                clean.push(p.as_os_str());
            }
        }
    }
    clean
}

/// Compute a safe, normalized relative path to working_dir for backups.
/// Handles relative paths and "../" paths safely without escaping the backup directory.
pub fn get_clean_relative_path(path: &Path, working_dir: &Path) -> PathBuf {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    };

    let clean_abs = clean_path(&abs_path);
    let clean_working = clean_path(working_dir);

    match clean_abs.strip_prefix(&clean_working) {
        Ok(p) => p.to_path_buf(),
        Err(_) => {
            // Outside of working directory — fallback to safe filename to prevent escaping directory
            PathBuf::from(clean_abs.file_name().unwrap_or_default())
        }
    }
}

/// Backup a file before writing or editing it.
/// If a backup already exists for this session, we don't overwrite it
/// (preserving the original file state).
/// If the file does not exist, we write a creation marker to delete it on rollback.
pub async fn backup_file(path: &Path, context: &ToolContext) -> Result<(), String> {
    let session_id = match &context.session_id {
        Some(id) => id,
        None => return Ok(()), // Skip if session_id is not set
    };

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let backup_dir = home.join(".nexacode").join("backups").join(session_id);

    // Compute safe, matching relative path
    let relative_path = get_clean_relative_path(path, &context.working_dir);
    let backup_path = backup_dir.join(&relative_path);

    // If backup copy already exists, do not overwrite it.
    // We want the original content from before any edits in this session.
    if backup_path.exists() || backup_path.with_extension("nexacode_created_marker").exists() {
        return Ok(());
    }

    // Ensure parent folder exists
    if let Some(parent) = backup_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            return Err(format!("Failed to create backup directory: {}", e));
        }
    }

    if path.exists() {
        if let Err(e) = tokio::fs::copy(path, &backup_path).await {
            return Err(format!("Failed to copy file to backup: {}", e));
        }
        log::info!("[Backup] Backed up original file {:?} to {:?}", path, backup_path);
    } else {
        let marker = backup_path.with_extension("nexacode_created_marker");
        if let Err(e) = tokio::fs::write(&marker, "").await {
            return Err(format!("Failed to write creation marker: {}", e));
        }
        log::info!("[Backup] Created marker for new file: {:?}", marker);
    }

    Ok(())
}

/// Rollback all file changes in a session, restoring original contents
/// and deleting newly created files.
pub async fn rollback_session(session_id: &str, context: &ToolContext) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let backup_dir = home.join(".nexacode").join("backups").join(session_id);

    if !backup_dir.exists() {
        return Ok(()); // Nothing to rollback
    }

    log::info!("[Backup] Rolling back changes for session: {}", session_id);

    let mut dir_entries = vec![backup_dir.clone()];
    let mut files_to_restore = Vec::new();
    let mut files_to_delete = Vec::new();

    while let Some(current_dir) = dir_entries.pop() {
        let mut read_dir = match tokio::fs::read_dir(&current_dir).await {
            Ok(rd) => rd,
            Err(e) => return Err(format!("Failed to read backup dir: {}", e)),
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                dir_entries.push(path);
            } else {
                if let Some(ext) = path.extension() {
                    if ext == "nexacode_created_marker" {
                        let relative = path.strip_prefix(&backup_dir)
                            .map_err(|e| e.to_string())?
                            .with_extension(""); // Strip extension
                        let target = context.resolve_path(&relative.to_string_lossy());
                        files_to_delete.push(target);
                        continue;
                    }
                }

                let relative = path.strip_prefix(&backup_dir).map_err(|e| e.to_string())?;
                let target = context.resolve_path(&relative.to_string_lossy());
                files_to_restore.push((path.clone(), target));
            }
        }
    }

    // 1. Delete files that were newly created
    for path in files_to_delete {
        if path.exists() {
            if let Err(e) = tokio::fs::remove_file(&path).await {
                log::error!("[Backup] Failed to remove created file: {:?}, error: {}", path, e);
            } else {
                log::info!("[Backup] Deleted created file: {:?}", path);
            }
        }
    }

    // 2. Restore original files
    for (src, dest) in files_to_restore {
        if let Some(parent) = dest.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Err(e) = tokio::fs::copy(&src, &dest).await {
            return Err(format!("Failed to restore file: {}", e));
        }
        log::info!("[Backup] Restored file {:?} from {:?}", dest, src);
    }

    // 3. Clean up session backups dir
    let _ = tokio::fs::remove_dir_all(&backup_dir).await;

    Ok(())
}
