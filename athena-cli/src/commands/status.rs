use athena_core::config::{load_config, get_env_value};
use std::process::Command;

pub fn run_status() {
    let config = load_config();
    print!("{}", get_status_overview(&config));
}

pub fn get_status_overview(config: &athena_core::config::AthenaConfig) -> String {
    let mut out = String::new();
    out.push_str("\nAthena Component Status\n");
    out.push_str("═════════════════════════\n\n");

    // 1. LLM Provider Connection Status
    out.push_str("  LLM Providers:\n");
    let providers = [
        ("OpenAI API", "OPENAI_API_KEY"),
        ("Anthropic API", "ANTHROPIC_API_KEY"),
        ("OpenRouter API", "OPENROUTER_API_KEY"),
        ("Google API", "GOOGLE_API_KEY"),
    ];

    for (name, env_var) in &providers {
        let is_configured = get_env_value(env_var).is_some();
        let status = if is_configured { "✓ Configured" } else { "✗ Not configured" };
        out.push_str(&format!("    • {:<18} : {}\n", name, status));
    }
    out.push('\n');

    // 2. Gateway/Messaging Platforms Status
    out.push_str("  Gateways:\n");
    out.push_str(&format!("    • Telegram Gateway   : {}\n", if config.gateway.telegram_enabled { "✓ Enabled" } else { "✗ Disabled" }));
    out.push_str(&format!("    • Discord Gateway    : {}\n", if config.gateway.discord_enabled { "✓ Enabled" } else { "✗ Disabled" }));
    out.push_str(&format!("    • WhatsApp Gateway   : {}\n\n", if config.gateway.whatsapp_enabled { "✓ Enabled" } else { "✗ Disabled" }));

    // 3. Execution Environment dependencies
    out.push_str("  Dependencies:\n");

    // Check docker
    let docker_check = Command::new("docker").arg("--version").output();
    let docker_status = match docker_check {
        Ok(out) => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            format!("✓ Installed ({})", version)
        }
        Err(_) => "✗ Not installed".to_string(),
    };
    out.push_str(&format!("    • Docker Engine      : {}\n", docker_status));

    // Check git
    let git_check = Command::new("git").arg("--version").output();
    let git_status = match git_check {
        Ok(out) => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            format!("✓ Installed ({})", version)
        }
        Err(_) => "✗ Not installed".to_string(),
    };
    out.push_str(&format!("    • Git CLI            : {}\n", git_status));

    // Check node
    let node_check = Command::new("node").arg("--version").output();
    let node_status = match node_check {
        Ok(out) => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            format!("✓ Installed ({})", version)
        }
        Err(_) => "✗ Not installed".to_string(),
    };
    out.push_str(&format!("    • Node.js            : {}\n", node_status));

    out.push_str("\nAll status checks completed.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_status_overview() {
        let config = athena_core::config::AthenaConfig::default();
        let status = get_status_overview(&config);
        assert!(status.contains("Athena Component Status"));
        assert!(status.contains("LLM Providers:"));
        assert!(status.contains("Gateways:"));
        assert!(status.contains("Dependencies:"));
    }
}

// Rust guideline compliant 2026-02-21
