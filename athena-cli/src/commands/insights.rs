use std::fs;
use athena_core::paths::get_athena_home;

pub fn run_insights() {
    println!("\nAthena Usage Insights & Analytics");
    println!("═══════════════════════════════════\n");
    println!("Token consumption estimates, file metrics, and cost details.");
    println!();

    let home = get_athena_home();
    let (sessions_count, total_session_size, log_size) = calculate_insights(&home);

    println!("Workspace Metrics:");
    println!("  • Total Conversation Sessions : {}", sessions_count);
    println!("  • Accumulated Session Storage  : {:.2} KB", (total_session_size as f64) / 1024.0);
    println!("  • Rolling Debug Logs Size     : {:.2} KB", (log_size as f64) / 1024.0);
    println!();

    let est_tokens = (total_session_size as f64) / 4.0;
    let est_cost = est_tokens * 0.000015;

    println!("Estimated Inference Statistics:");
    println!("  • Estimated Tokens Processed  : {:.0} tokens", est_tokens);
    println!("  • Estimated Model Cost Saved  : ${:.4}", est_cost);
    println!();
}

pub fn calculate_insights(athena_home: &std::path::Path) -> (usize, u64, u64) {
    let sessions_dir = athena_home.join("sessions");
    let mut sessions_count = 0;
    let mut total_session_size = 0;
    if let Ok(entries) = fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                sessions_count += 1;
                if let Ok(meta) = entry.metadata() {
                    total_session_size += meta.len();
                }
            }
        }
    }

    let log_dir = athena_home.join("logs");
    let mut log_size = 0;
    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                if let Ok(meta) = entry.metadata() {
                    log_size += meta.len();
                }
            }
        }
    }

    (sessions_count, total_session_size, log_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_insights() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home = temp_dir.path();
        std::fs::create_dir_all(home.join("sessions")).unwrap();
        std::fs::create_dir_all(home.join("logs")).unwrap();

        std::fs::write(home.join("sessions").join("sess1.json"), "{\"turn\": 1}").unwrap();
        std::fs::write(home.join("logs").join("test.log"), "log line").unwrap();

        let (count, session_size, log_size) = calculate_insights(home);
        assert_eq!(count, 1);
        assert!(session_size > 0);
        assert!(log_size > 0);
    }
}

// Rust guideline compliant 2026-02-21
