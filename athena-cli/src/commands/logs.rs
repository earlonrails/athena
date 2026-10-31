use std::fs;
use std::io::{BufRead, BufReader};
use athena_core::paths::get_athena_home;
use cliclack::{intro, select, outro_cancel, note};
use anyhow::Result;

pub fn run_logs() -> Result<()> {
    intro("Athena Log Viewer")?;

    let log_dir = get_athena_home().join("logs");
    if !log_dir.exists() {
        outro_cancel(format!("No logs directory found at {}.", log_dir.display()))?;
        return Ok(());
    }

    let mut entries = get_log_files(&log_dir);

    if entries.is_empty() {
        outro_cancel(format!("No log files found in {}.", log_dir.display()))?;
        return Ok(());
    }

    entries.sort_by(|a, b| b.2.cmp(&a.2));

    let mut select_prompt = select("Select a log file to view:");
    for (i, (_, filename, _)) in entries.iter().enumerate() {
        select_prompt = select_prompt.item(i, filename.clone(), "");
    }
    
    let choice: usize = select_prompt.interact()?;
    let (selected_path, selected_name, _) = &entries[choice];

    if let Ok(file) = fs::File::open(selected_path) {
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().flatten().collect();
        let start = lines.len().saturating_sub(50);
        
        let mut log_content = String::new();
        for line in &lines[start..] {
            log_content.push_str(&format!("{}\n", line));
        }
        
        if lines.is_empty() {
            note(format!("Showing last 50 lines of {}", selected_name), "(Empty file)")?;
        } else {
            note(format!("Showing last 50 lines of {}", selected_name), log_content.trim_end())?;
        }
    } else {
        outro_cancel("Failed to open log file.")?;
    }
    
    Ok(())
}

pub fn get_log_files(log_dir: &std::path::Path) -> Vec<(std::path::PathBuf, String, std::time::SystemTime)> {
    let mut entries = Vec::new();
    if let Ok(dir_entries) = fs::read_dir(log_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()).map(String::from) {
                    if filename.contains("log") {
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                entries.push((path.clone(), filename, modified));
                            }
                        }
                    }
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
    fn test_get_log_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_dir = temp_dir.path().join("logs");
        fs::create_dir_all(&log_dir).unwrap();

        fs::write(log_dir.join("agent.log"), "log line 1\nlog line 2").unwrap();
        fs::write(log_dir.join("other.txt"), "ignored").unwrap();

        let files = get_log_files(&log_dir);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, "agent.log");
    }
}

// Rust guideline compliant 2026-02-21
