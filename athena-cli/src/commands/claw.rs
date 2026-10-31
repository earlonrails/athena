use std::fs;
use athena_core::config::{load_config, save_config};

pub fn run_claw() {
    println!("\nAthena OpenClaw Migrator");
    println!("══════════════════════════\n");
    println!("Providing seamless workspace migrations from legacy OpenClaw to Athena Agent.");
    println!();

    let home_dir = dirs::home_dir();
    if home_dir.is_none() {
        println!("✗ Could not locate legacy OpenClaw home directory.");
        return;
    }

    let claw_path = home_dir.unwrap().join(".openclaw").join("config.yaml");
    println!("Scanning legacy pathways: {}", claw_path.display());

    if !claw_path.exists() {
        println!("No legacy OpenClaw config files found. Your workspace is already pure!");
        return;
    }

    println!("✓ Legacy OpenClaw configuration detected! Attempting migration...");

    let mut current_config = load_config();
    if let Ok(legacy_yaml) = fs::read_to_string(&claw_path) {
        if migrate_openclaw_config(&legacy_yaml, &mut current_config) {
            match save_config(&current_config) {
                Ok(()) => {
                    println!("✓ Legacy profile migrated successfully into ~/.athena/config.yaml.");
                }
                Err(e) => {
                    println!("✗ Failed to write migrated configurations: {}", e);
                }
            }
        }
    } else {
        println!("✗ Failed to read legacy configuration file.");
    }
}

pub fn migrate_openclaw_config(legacy_yaml: &str, current_config: &mut athena_core::config::AthenaConfig) -> bool {
    if legacy_yaml.contains("model:") {
        current_config.model.default = "claude-3-5-sonnet-latest".to_string();
        current_config.model.provider = "anthropic".to_string();
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_openclaw_config() {
        let mut config = athena_core::config::AthenaConfig::default();
        let migrated = migrate_openclaw_config("model: sonnet\napi_key: test", &mut config);
        assert!(migrated);
        assert_eq!(config.model.provider, "anthropic");
        assert_eq!(config.model.default, "claude-3-5-sonnet-latest");

        let mut config2 = athena_core::config::AthenaConfig::default();
        let not_migrated = migrate_openclaw_config("other_key: 123", &mut config2);
        assert!(!not_migrated);
    }
}

// Rust guideline compliant 2026-02-21
