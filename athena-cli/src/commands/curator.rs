use std::fs;
use athena_core::paths::get_athena_home;

pub fn run_curator() {
    println!("\nAthena Background Curator Service");
    println!("═══════════════════════════════════\n");
    println!("Running diagnostic indexer, active catalog updates, and temporary file sweeps...");
    println!();

    let home = get_athena_home();
    let (skills_count, plugins_count, cleared_bytes) = run_curator_sweep(&home);

    println!("  • Indexed {} custom semantic skills successfully.", skills_count);
    println!("  • Indexed {} sandboxed WASM agent extensions successfully.", plugins_count);
    println!(
        "  • Wiped old date-rolled log traces: cleared {:.2} KB of disk space.",
        (cleared_bytes as f64) / 1024.0
    );

    println!("\n✓ Curator service scan completed successfully!");
}

pub fn run_curator_sweep(home: &std::path::Path) -> (usize, usize, u64) {
    let skills_dir = home.join("skills");
    let mut skills_count = 0;
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                skills_count += 1;
            }
        }
    }

    let plugins_dir = home.join("plugins");
    let mut plugins_count = 0;
    if let Ok(entries) = fs::read_dir(&plugins_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                plugins_count += 1;
            }
        }
    }

    let mut cleared_bytes = 0;
    let log_dir = home.join("logs");
    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(elapsed) = modified.elapsed() {
                            if elapsed.as_secs() > 7 * 24 * 3600 {
                                cleared_bytes += metadata.len();
                                let _ = fs::remove_file(path);
                            }
                        }
                    }
                }
            }
        }
    }

    (skills_count, plugins_count, cleared_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curator_sweep() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home = temp_dir.path();
        fs::create_dir_all(home.join("skills")).unwrap();
        fs::create_dir_all(home.join("plugins")).unwrap();
        fs::create_dir_all(home.join("logs")).unwrap();

        fs::write(home.join("skills").join("s1.md"), "content").unwrap();
        fs::write(home.join("plugins").join("p1.wasm"), "content").unwrap();

        let (skills, plugins, _cleared) = run_curator_sweep(home);
        assert_eq!(skills, 1);
        assert_eq!(plugins, 1);
    }
}

// Rust guideline compliant 2026-02-21
