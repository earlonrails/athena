use athena_core::config::{load_config, save_config};
use cliclack::{intro, select, input, outro, outro_cancel, note};
use anyhow::Result;

pub fn run_config_show() -> Result<()> {
    let config = load_config();
    intro("Current Athena Configuration")?;
    note("Configuration Details", format_config_details(&config))?;
    Ok(())
}

pub fn format_config_details(config: &athena_core::config::AthenaConfig) -> String {
    let mut details = String::new();
    details.push_str(&format!("Profile:          {:?}\n", config.active_profile.as_deref().unwrap_or("default")));
    details.push_str(&format!("Active Provider:  {}\n", config.model.provider));
    details.push_str(&format!("Default Model:    {}\n", config.model.default));
    details.push_str(&format!("Terminal Backend: {}\n", config.terminal_backend));
    details.push_str(&format!("Max Iterations:   {}\n", config.agent.max_iterations));
    details.push_str(&format!("Telegram Enabled: {}\n", config.gateway.telegram_enabled));
    details.push_str(&format!("Discord Enabled:  {}\n", config.gateway.discord_enabled));
    details.push_str(&format!("Slack Enabled:    {}\n", config.gateway.slack_enabled));
    details.push_str(&format!("WhatsApp Enabled: {}\n", config.gateway.whatsapp_enabled));
    details.push_str(&format!("Config Version:   {}", config.config_version));
    details
}

pub fn run_config_edit() -> Result<()> {
    intro("Edit Configuration Fields")?;

    let mut config = load_config();

    let backend: String = select(format!("Change Terminal Backend (current: {})", config.terminal_backend))
        .item("local".to_string(), "local", "")
        .item("docker".to_string(), "docker", "")
        .item("ssh".to_string(), "ssh", "")
        .item("modal".to_string(), "modal", "")
        .interact()?;
        
    if backend != config.terminal_backend {
        config.terminal_backend = backend.to_string();
    }

    let default_val = config.agent.max_iterations.to_string();
    let iterations: String = input(format!("Change Max Iterations (current: {})", config.agent.max_iterations))
        .default_input(&default_val)
        .interact()?;
        
    let iterations = iterations.trim();
    if !iterations.is_empty() {
        if let Ok(num) = iterations.parse::<u32>() {
            config.agent.max_iterations = num;
        }
    }

    match save_config(&config) {
        Ok(()) => outro("Config successfully updated at ~/.athena/config.yaml")?,
        Err(e) => outro_cancel(format!("Failed to save config: {}", e))?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_config_details() {
        let config = athena_core::config::AthenaConfig::default();
        let formatted = format_config_details(&config);
        assert!(formatted.contains("Active Provider:"));
        assert!(formatted.contains("Terminal Backend:"));
        assert!(formatted.contains("Max Iterations:"));
    }
}

// Rust guideline compliant 2026-02-21
