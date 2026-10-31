use athena_core::config::{get_env_value, save_env_value};
use cliclack::{intro, select, input, password, outro, note};
use anyhow::Result;

pub fn run_memory() -> Result<()> {
    // ... existing ...
    intro("Athena External Memory Provider")?;
    note("Info", "Configure Honcho, Qdrant, or Pinecone semantic retrieval nodes.")?;

    let current = get_env_value("MEMORY_BACKEND").unwrap_or_else(|| "local".to_string());
    
    let choice: usize = select(format!("Current Memory Backend: {}", current))
        .item(1, "Set Memory Backend to Local SQLite", "")
        .item(2, "Set Memory Backend to Honcho Cloud", "")
        .item(3, "Set Memory Backend to Qdrant Vector DB", "")
        .item(4, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            if save_env_value("MEMORY_BACKEND", "local").is_ok() {
                outro("Set memory backend to Local SQLite.")?;
            }
        }
        2 => {
            let key: String = password("Enter Honcho API Key (will be masked)").interact()?;
            let key = key.trim();
            if !key.is_empty() {
                let _ = save_env_value("HONCHO_API_KEY", key);
            }
            if save_env_value("MEMORY_BACKEND", "honcho").is_ok() {
                outro("Set memory backend to Honcho Cloud.")?;
            }
        }
        3 => {
            let url: String = input("Enter Qdrant URL")
                .placeholder("http://localhost:6334")
                .interact()?;
            let url = url.trim();
            if !url.is_empty() {
                let _ = save_env_value("QDRANT_URL", url);
            }
            if save_env_value("MEMORY_BACKEND", "qdrant").is_ok() {
                outro("Set memory backend to Qdrant Vector DB.")?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub const DEFAULT_MEMORY_TEMPLATE: &str = "<!-- Write your memory and context here. Athena will automatically inject this into its system prompt. -->\n";

pub fn ensure_memory_file(memory_path: &std::path::Path) -> std::io::Result<bool> {
    if !memory_path.exists() {
        if let Some(parent) = memory_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(memory_path, DEFAULT_MEMORY_TEMPLATE)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn run_memory_edit() -> Result<()> {
    let memory_path = athena_core::paths::get_athena_home().join("MEMORY.md");
    ensure_memory_file(&memory_path)?;
    
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let status = std::process::Command::new(&editor)
        .arg(&memory_path)
        .status()?;
        
    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_memory_file() {
        let dir = tempdir().unwrap();
        let mem_path = dir.path().join("MEMORY.md");

        // First creation
        let created = ensure_memory_file(&mem_path).unwrap();
        assert!(created);
        assert!(mem_path.exists());
        let content = std::fs::read_to_string(&mem_path).unwrap();
        assert_eq!(content, DEFAULT_MEMORY_TEMPLATE);

        // Second check should not overwrite
        let created_again = ensure_memory_file(&mem_path).unwrap();
        assert!(!created_again);
    }
}

// Rust guideline compliant 2026-02-21
