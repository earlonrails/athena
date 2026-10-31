use athena_core::config::{load_config, save_config};
use cliclack::{intro, select, input, outro, outro_cancel, note};
use anyhow::Result;

pub fn run_fallback() -> Result<()> {
    let mut config = load_config();
    intro("Fallback Providers Configuration")?;
    note("Info", format!("Fallback providers are queried sequentially when the primary provider fails.\nCurrent fallback list: {}", format_fallback_list(&config.fallback_providers)))?;

    let choice: usize = select("Options")
        .item(1, "Add provider to fallbacks", "")
        .item(2, "Remove provider from fallbacks", "")
        .item(3, "Clear fallbacks", "")
        .item(4, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let slug: String = input("Enter provider slug")
                .placeholder("anthropic, openrouter")
                .interact()?;
            let slug = slug.trim().to_string();
            if !slug.is_empty() {
                config.fallback_providers.push(slug.clone());
                let _ = save_config(&config);
                outro(format!("Added {} to fallbacks.", slug))?;
            } else {
                outro_cancel("Provider slug cannot be empty.")?;
            }
        }
        2 => {
            if config.fallback_providers.is_empty() {
                outro_cancel("No fallbacks configured.")?;
                return Ok(());
            }

            let mut select_prompt = select("Select provider to remove");
            for (i, p) in config.fallback_providers.iter().enumerate() {
                select_prompt = select_prompt.item(i, p.clone(), "");
            }
            let idx: usize = select_prompt.interact()?;
            
            let slug = config.fallback_providers.remove(idx);
            let _ = save_config(&config);
            outro(format!("Removed {} from fallbacks.", slug))?;
        }
        3 => {
            config.fallback_providers.clear();
            let _ = save_config(&config);
            outro("Fallback list cleared.")?;
        }
        _ => { outro("Goodbye!")?; }
    }
    Ok(())
}

pub fn format_fallback_list(fallbacks: &[String]) -> String {
    if fallbacks.is_empty() {
        "None configured.".to_string()
    } else {
        fallbacks.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_fallback_list() {
        assert_eq!(format_fallback_list(&[]), "None configured.");
        assert_eq!(
            format_fallback_list(&["anthropic".to_string(), "openrouter".to_string()]),
            "anthropic, openrouter"
        );
    }
}

// Rust guideline compliant 2026-02-21
