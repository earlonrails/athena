use std::fs;
use std::io::{self, Write};
use athena_core::paths::get_athena_home;

pub fn run_checkpoints() {
    println!("\nAthena Checkpoints Manager");
    println!("════════════════════════════\n");
    println!("List, inspect, and prune previous state snapshots saved at ~/.athena/checkpoints/.");
    println!();

    let checkpoints_dir = get_athena_home().join("checkpoints");
    if !checkpoints_dir.exists() {
        println!("No checkpoints folder found at {}.", checkpoints_dir.display());
        return;
    }

    let mut entries = get_checkpoints(&checkpoints_dir);

    if entries.is_empty() {
        println!("No checkpoints recorded yet.");
        return;
    }

    // Sort by modified time descending (newest first)
    entries.sort_by(|a, b| b.2.cmp(&a.2));

    println!("Current snapshots:");
    for (i, (_, name, created, size)) in entries.iter().enumerate() {
        let created_str = if let Some(time) = created {
            let datetime: chrono::DateTime<chrono::Local> = (*time).into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        } else {
            "Unknown date".to_string()
        };
        let size_str = if *size > 0 {
            format!(" ({:.2} KB)", (*size as f64) / 1024.0)
        } else {
            "".to_string()
        };
        println!("  {}. {}{} - {}", i + 1, name, size_str, created_str);
    }
    println!();

    println!("Options:");
    println!("  1. Delete a specific checkpoint");
    println!("  2. Prune all checkpoints");
    println!("  3. Exit");
    println!();

    print!("  Choice [1-3]: ");
    io::stdout().flush().ok();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).ok();
    let choice = choice.trim().parse::<usize>().unwrap_or(3);

    match choice {
        1 => {
            print!("  Select checkpoint number to delete [1-{}]: ", entries.len());
            io::stdout().flush().ok();
            let mut snap_choice = String::new();
            io::stdin().read_line(&mut snap_choice).ok();
            let snap_choice = snap_choice.trim().parse::<usize>().unwrap_or(0);

            if snap_choice < 1 || snap_choice > entries.len() {
                println!("  ✗ Invalid choice.");
                return;
            }

            let (path, name, _, _) = &entries[snap_choice - 1];
            let res = if path.is_file() {
                fs::remove_file(path)
            } else {
                fs::remove_dir_all(path)
            };

            match res {
                Ok(()) => println!("  ✓ Deleted checkpoint {} successfully.", name),
                Err(e) => println!("  ✗ Failed to delete checkpoint: {}", e),
            }
        }
        2 => {
            print!("  Are you sure you want to delete ALL checkpoints? [y/N]: ");
            io::stdout().flush().ok();
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm).ok();
            if confirm.trim().to_lowercase() == "y" {
                let mut count = 0;
                for (path, _, _, _) in entries {
                    let res = if path.is_file() {
                        fs::remove_file(&path)
                    } else {
                        fs::remove_dir_all(&path)
                    };
                    if res.is_ok() {
                        count += 1;
                    }
                }
                println!("  ✓ Successfully cleared {} checkpoints.", count);
            }
        }
        _ => {}
    }
}

pub fn get_checkpoints(checkpoints_dir: &std::path::Path) -> Vec<(std::path::PathBuf, String, Option<std::time::SystemTime>, u64)> {
    let mut entries = Vec::new();
    if let Ok(dir_entries) = fs::read_dir(checkpoints_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()).map(String::from) {
                if let Ok(metadata) = entry.metadata() {
                    let created = metadata.created().or_else(|_| metadata.modified()).ok();
                    let size = if path.is_file() { metadata.len() } else { 0 };
                    entries.push((path, name, created, size));
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
    fn test_get_checkpoints() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cp_dir = temp_dir.path().join("checkpoints");
        fs::create_dir_all(&cp_dir).unwrap();

        fs::write(cp_dir.join("cp_1.tar.gz"), b"checkpoint 1").unwrap();
        fs::write(cp_dir.join("cp_2.tar.gz"), b"checkpoint 2").unwrap();

        let list = get_checkpoints(&cp_dir);
        assert_eq!(list.len(), 2);
        let names: Vec<String> = list.into_iter().map(|c| c.1).collect();
        assert!(names.contains(&"cp_1.tar.gz".to_string()));
        assert!(names.contains(&"cp_2.tar.gz".to_string()));
    }
}

// Rust guideline compliant 2026-02-21
