use athena_core::config::{get_env_value, save_env_value, remove_env_value};
use cliclack::{intro, select, password, confirm, outro, outro_cancel};
use anyhow::Result;

pub fn run_auth() -> Result<()> {
    intro("Athena Authentication Pool Manager")?;

    let providers = [
        ("openai",      "OPENAI_API_KEY"),
        ("anthropic",   "ANTHROPIC_API_KEY"),
        ("openrouter",  "OPENROUTER_API_KEY"),
        ("google",      "GOOGLE_API_KEY"),
        ("deepseek",    "DEEPSEEK_API_KEY"),
        ("groq",        "GROQ_API_KEY"),
        ("mistral",     "MISTRAL_API_KEY"),
        ("xai",         "XAI_API_KEY"),
    ];

    let msg = format_auth_pool_status(&providers);
    
    let choice: usize = select(msg.trim_end())
        .item(1, "Add/update a credential", "")
        .item(2, "Remove a credential", "")
        .item(3, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let mut select_prompt = select("Select provider to update");
            for (i, (name, _)) in providers.iter().enumerate() {
                select_prompt = select_prompt.item(i, *name, "");
            }
            let prov_choice: usize = select_prompt.interact()?;

            let (name, env_var) = providers[prov_choice];
            let key: String = password(format!("Enter API key for {} (will be masked)", name)).interact()?;
            let key = key.trim();

            if !key.is_empty() {
                match save_env_value(env_var, key) {
                    Ok(()) => outro("Credential saved successfully!")?,
                    Err(e) => outro_cancel(format!("Failed to save API key: {}", e))?,
                }
            } else {
                outro_cancel("Key was empty, nothing saved.")?;
            }
        }
        2 => {
            let mut select_prompt = select("Select provider to remove");
            for (i, (name, _)) in providers.iter().enumerate() {
                select_prompt = select_prompt.item(i, *name, "");
            }
            let prov_choice: usize = select_prompt.interact()?;

            let (name, env_var) = providers[prov_choice];
            let confirm_rm: bool = confirm(format!("Are you sure you want to remove credential for {}?", name)).interact()?;
            
            if confirm_rm {
                match remove_env_value(env_var) {
                    Ok(()) => outro("Credential removed successfully!")?,
                    Err(e) => outro_cancel(format!("Failed to remove API key: {}", e))?,
                }
            } else {
                outro_cancel("Cancelled.")?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn format_auth_pool_status(providers: &[(&str, &str)]) -> String {
    let mut msg = String::from("Current credentials pool status:\n");
    for (i, (name, env_var)) in providers.iter().enumerate() {
        let status = if let Some(val) = get_env_value(env_var) {
            if val.len() > 8 {
                format!("✓ Configured ({}...{})", &val[..4], &val[val.len()-4..])
            } else {
                "✓ Configured (****)".to_string()
            }
        } else {
            "✗ Not configured".to_string()
        };
        msg.push_str(&format!("  {}. {:<12} : {}\n", i + 1, name, status));
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_auth_pool_status() {
        let providers = [("test_prov", "NON_EXISTENT_KEY_12345")];
        let status = format_auth_pool_status(&providers);
        assert!(status.contains("Current credentials pool status"));
        assert!(status.contains("test_prov"));
        assert!(status.contains("✗ Not configured"));
    }
}

// Rust guideline compliant 2026-02-21
