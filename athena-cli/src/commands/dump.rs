use athena_core::paths::get_athena_home;
use athena_core::config::{load_config, get_env_value};

pub fn run_dump() {
    print!("{}", generate_dump());
}

pub fn generate_dump() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let home = get_athena_home();
    let config = load_config();

    let providers = [
        ("OPENAI_API_KEY", "OpenAI"),
        ("ANTHROPIC_API_KEY", "Anthropic"),
        ("OPENROUTER_API_KEY", "OpenRouter"),
        ("GOOGLE_API_KEY", "Google"),
        ("DEEPSEEK_API_KEY", "DeepSeek"),
        ("GROQ_API_KEY", "Groq"),
        ("MISTRAL_API_KEY", "Mistral"),
        ("XAI_API_KEY", "xAI"),
    ];

    let mut out = String::new();
    out.push_str("### Athena Agent Support Dump\n");
    out.push_str(&format!("- **Version**: `v{}`\n", version));
    out.push_str(&format!("- **OS Platform**: `{}`\n", std::env::consts::OS));
    out.push_str(&format!("- **Architecture**: `{}`\n", std::env::consts::ARCH));
    out.push_str(&format!("- **Home Directory**: `{}`\n", home.display()));
    out.push_str(&format!("- **Default Model**: `{}`\n", config.model.default));

    out.push_str("\n#### Active Credentials:\n");
    for (env_var, label) in providers {
        let configured = get_env_value(env_var).is_some();
        out.push_str(&format!("- **{}**: `{}`\n", label, if configured { "Configured" } else { "Not Configured" }));
    }

    out.push_str("\n#### Paths & Files Presence:\n");
    out.push_str(&format!("- **Config File**: `{}`\n", if home.join("config.yaml").exists() || home.join("config.json").exists() { "Present" } else { "Missing" }));
    out.push_str(&format!("- **Database**: `{}`\n", if home.join("state.db").exists() || home.join("session.db").exists() { "Present" } else { "Missing" }));
    out.push_str(&format!("- **Skills Folder**: `{}`\n", if home.join("skills").exists() { "Present" } else { "Missing" }));
    out.push_str(&format!("- **Plugins Folder**: `{}`\n", if home.join("plugins").exists() { "Present" } else { "Missing" }));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_dump() {
        let dump = generate_dump();
        assert!(dump.contains("### Athena Agent Support Dump"));
        assert!(dump.contains("Version"));
        assert!(dump.contains("Active Credentials"));
        assert!(dump.contains("Paths & Files Presence"));
    }
}

// Rust guideline compliant 2026-02-21
