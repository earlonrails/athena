use std::fs;
use athena_core::paths::get_athena_home;
use cliclack::{intro, select, confirm, outro, outro_cancel, note};
use anyhow::Result;

pub fn run_sessions() -> Result<()> {
    intro("Athena Sessions History")?;
    note("Info", "List, rename, export, delete, or prune past conversation histories.")?;

    let sessions_dir = get_athena_home().join("sessions");
    if !sessions_dir.exists() {
        let _ = fs::create_dir_all(&sessions_dir);
    }

    let mut entries = get_session_files(&sessions_dir);

    if entries.is_empty() {
        outro_cancel("No past session files found.")?;
        return Ok(());
    }

    entries.sort_by(|a, b| b.2.cmp(&a.2));

    let choice: usize = select("Current session history records")
        .item(1, "View session contents", "")
        .item(2, "Delete a session", "")
        .item(3, "Prune all sessions", "")
        .item(4, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let mut select_prompt = select("Select session to view");
            for (i, (_, name, modified)) in entries.iter().enumerate() {
                let date_str = if let Some(time) = modified {
                    let datetime: chrono::DateTime<chrono::Local> = (*time).into();
                    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                } else {
                    "Unknown date".to_string()
                };
                select_prompt = select_prompt.item(i, name.clone(), date_str);
            }
            let s_choice: usize = select_prompt.interact()?;

            let (path, name, _) = &entries[s_choice];
            if let Ok(content) = fs::read_to_string(path) {
                note(format!("SESSION CONTENT: {}", name), content)?;
            } else {
                outro_cancel("Unable to read session content")?;
            }
        }
        2 => {
            let mut select_prompt = select("Select session to delete");
            for (i, (_, name, modified)) in entries.iter().enumerate() {
                let date_str = if let Some(time) = modified {
                    let datetime: chrono::DateTime<chrono::Local> = (*time).into();
                    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                } else {
                    "Unknown date".to_string()
                };
                select_prompt = select_prompt.item(i, name.clone(), date_str);
            }
            let s_choice: usize = select_prompt.interact()?;

            let (path, name, _) = &entries[s_choice];
            if fs::remove_file(path).is_ok() {
                outro(format!("Session '{}' deleted successfully.", name))?;
            } else {
                outro_cancel("Failed to delete session.")?;
            }
        }
        3 => {
            let confirm_rm: bool = confirm("Are you sure you want to delete ALL sessions?").interact()?;
            if confirm_rm {
                let mut count = 0;
                for (path, _, _) in entries {
                    if fs::remove_file(path).is_ok() {
                        count += 1;
                    }
                }
                outro(format!("Successfully cleared {} session history files.", count))?;
            } else {
                outro_cancel("Cancelled.")?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn get_session_files(sessions_dir: &std::path::Path) -> Vec<(std::path::PathBuf, String, Option<std::time::SystemTime>)> {
    let mut entries = Vec::new();
    if let Ok(dir_entries) = fs::read_dir(sessions_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()).map(String::from) {
                    let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
                    entries.push((path, name, modified));
                }
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_session_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sessions_dir = temp_dir.path().join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();

        fs::write(sessions_dir.join("session_1.json"), "{}").unwrap();
        fs::write(sessions_dir.join("session_2.json"), "{}").unwrap();

        let files = get_session_files(&sessions_dir);
        assert_eq!(files.len(), 2);
        let names: Vec<String> = files.into_iter().map(|f| f.1).collect();
        assert!(names.contains(&"session_1.json".to_string()));
        assert!(names.contains(&"session_2.json".to_string()));
    }
}

// Rust guideline compliant 2026-02-21
