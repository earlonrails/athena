use athena_core::config::{load_config, save_config, get_env_value, save_env_value};
use cliclack::{intro, select, input, password, outro, outro_cancel, note};
use anyhow::Result;

/// Select default model — starts with provider selection, then model picker.
pub fn run_model() -> Result<()> {
    let mut config = load_config();

    let current_model = if config.model.default.is_empty() {
        "(not set)".to_string()
    } else {
        config.model.default.clone()
    };

    let current_provider = if config.model.provider.is_empty() {
        "auto".to_string()
    } else {
        config.model.provider.clone()
    };

    intro("Configure Default Model & Provider")?;
    note("Current Settings", format!("Model: {}\nProvider: {}", current_model, current_provider))?;

    // Provider selection
    let providers = [
        ("openai",      "OpenAI (GPT-4o, o1, o3, ...)"),
        ("anthropic",   "Anthropic (Claude 4, Opus, Sonnet, ...)"),
        ("openrouter",  "OpenRouter (access many providers)"),
        ("google",      "Google (Gemini)"),
        ("deepseek",    "DeepSeek"),
        ("groq",        "Groq"),
        ("mistral",     "Mistral AI"),
        ("xai",         "xAI (Grok)"),
        ("local",       "Local / Custom endpoint"),
    ];

    let mut select_prompt = select("Select provider");
    for (slug, label) in &providers {
        let hint = if current_provider == *slug { " (current)" } else { "" };
        select_prompt = select_prompt.item(*slug, format!("{}{}", label, hint), "");
    }
    let slug: &str = select_prompt.interact()?;
    config.model.provider = slug.to_string();

    // Check API key
    let env_key = provider_env_key(slug);
    if !env_key.is_empty() {
        if get_env_value(env_key).is_none() {
            let key: String = password(format!("{} not set. Enter API key", env_key)).interact()?;
            let key = key.trim();
            if !key.is_empty() {
                match save_env_value(env_key, key) {
                    Ok(()) => note("Success", format!("Saved {}", env_key))?,
                    Err(e) => outro_cancel(format!("{}", e))?,
                }
            }
        } else {
            note("Configuration", format!("{} already configured", env_key))?;
        }
    } else if slug == "local" {
        let url: String = input("Base URL")
            .placeholder("http://localhost:11434/v1")
            .interact()?;
        let url = url.trim();
        if !url.is_empty() {
            let _ = save_env_value("OPENAI_BASE_URL", url);
            note("Success", "Base URL saved")?;
        }
    }

    // Model selection
    let default_model = default_model_for_provider(slug);
    let model_input: String = input("Model")
        .default_input(default_model)
        .interact()?;
    let model_input = model_input.trim();
    config.model.default = if model_input.is_empty() {
        default_model.to_string()
    } else {
        model_input.to_string()
    };

    match save_config(&config) {
        Ok(()) => {
            outro(format!("Default model set: {} (provider: {})", config.model.default, config.model.provider))?;
        }
        Err(e) => outro_cancel(format!("Failed to save: {}", e))?,
    }
    Ok(())
}

fn provider_env_key(slug: &str) -> &'static str {
    match slug {
        "openai" => "OPENAI_API_KEY",
        "anthropic" => "ANTHROPIC_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        "google" => "GOOGLE_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        "groq" => "GROQ_API_KEY",
        "mistral" => "MISTRAL_API_KEY",
        "xai" => "XAI_API_KEY",
        _ => "",
    }
}

fn default_model_for_provider(slug: &str) -> &'static str {
    match slug {
        "openai" => "gpt-4o",
        "anthropic" => "claude-sonnet-4-20250514",
        "openrouter" => "openai/gpt-4o",
        "google" => "gemini-2.5-pro",
        "deepseek" => "deepseek-chat",
        "groq" => "llama-3.3-70b-versatile",
        "mistral" => "mistral-large-latest",
        "xai" => "grok-3",
        _ => "gpt-4o",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_env_key() {
        assert_eq!(provider_env_key("openai"), "OPENAI_API_KEY");
        assert_eq!(provider_env_key("anthropic"), "ANTHROPIC_API_KEY");
        assert_eq!(provider_env_key("google"), "GOOGLE_API_KEY");
        assert_eq!(provider_env_key("mistral"), "MISTRAL_API_KEY");
        assert_eq!(provider_env_key("unknown"), "");
    }

    #[test]
    fn test_default_model_for_provider() {
        assert_eq!(default_model_for_provider("openai"), "gpt-4o");
        assert_eq!(default_model_for_provider("anthropic"), "claude-sonnet-4-20250514");
        assert_eq!(default_model_for_provider("google"), "gemini-2.5-pro");
        assert_eq!(default_model_for_provider("mistral"), "mistral-large-latest");
        assert_eq!(default_model_for_provider("other"), "gpt-4o");
    }
}

// Rust guideline compliant 2026-02-21
