use athena_core::config::{load_config, save_config};
use athena_core::paths::get_athena_home;
use std::fs;
use cliclack::{intro, select, outro, outro_cancel, note};
use anyhow::Result;

pub fn run_slack() -> Result<()> {
    intro("Athena Slack Integration")?;
    note("Info", "Provides helpers for generating Slack Manifest JSON files and enabling the bot gateway.")?;

    let mut config = load_config();
    let status = if config.gateway.slack_enabled { "ENABLED" } else { "DISABLED" };

    let choice: usize = select(format!("Current Status: {}", status))
        .item(1, "Enable Slack gateway", "")
        .item(2, "Disable Slack gateway", "")
        .item(3, "Generate Slack Manifest JSON", "")
        .item(4, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            config.gateway.slack_enabled = true;
            if save_config(&config).is_ok() {
                outro("Slack gateway has been enabled.")?;
            } else {
                outro_cancel("Failed to save config.")?;
            }
        }
        2 => {
            config.gateway.slack_enabled = false;
            if save_config(&config).is_ok() {
                outro("Slack gateway has been disabled.")?;
            } else {
                outro_cancel("Failed to save config.")?;
            }
        }
        3 => {
            let manifest = generate_slack_manifest();
            let path = get_athena_home().join("slack-manifest.json");
            match fs::write(&path, manifest) {
                Ok(()) => {
                    note("Success", format!("Generated Slack Manifest template successfully!\nSaved to: {}", path.display()))?;
                }
                Err(e) => {
                    outro_cancel(format!("Failed to save manifest: {}", e))?;
                }
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn generate_slack_manifest() -> &'static str {
    r#"{
    "display_information": {
        "name": "Athena Agent"
    },
    "features": {
        "bot_user": {
            "display_name": "Athena",
            "always_online": true
        }
    },
    "oauth_config": {
        "scopes": {
            "bot": [
                "chat:write",
                "im:history",
                "im:write",
                "app_mentions:read"
            ]
        }
    },
    "settings": {
        "event_subscriptions": {
            "bot_events": [
                "message.im",
                "app_mention"
            ]
        }
    }
}"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slack_manifest() {
        let manifest = generate_slack_manifest();
        assert!(manifest.contains("Athena Agent"));
        assert!(manifest.contains("chat:write"));
        assert!(manifest.contains("app_mention"));
        let parsed: serde_json::Value = serde_json::from_str(manifest).unwrap();
        assert_eq!(parsed["display_information"]["name"], "Athena Agent");
    }
}

// Rust guideline compliant 2026-02-21
