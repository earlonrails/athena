use std::fs;
use serde::{Deserialize, Serialize};
use athena_core::paths::get_athena_home;
use cliclack::{intro, select, input, outro, outro_cancel};
use anyhow::Result;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct WebhookData {
    subscriptions: Vec<Subscription>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Subscription {
    name: String,
    url: String,
    secret: String,
    active: bool,
}

pub fn run_webhook() -> Result<()> {
    intro("Athena Webhook Subscriptions")?;

    let webhook_file = get_athena_home().join("webhooks.json");
    let mut data = if webhook_file.exists() {
        let content = fs::read_to_string(&webhook_file).unwrap_or_default();
        serde_json::from_str::<WebhookData>(&content).unwrap_or_default()
    } else {
        WebhookData::default()
    };

    let choice: usize = select("Manage incoming and outgoing HTTP webhook hooks for third-party service dispatch")
        .item(1, "List registered webhooks", "")
        .item(2, "Register a new webhook", "")
        .item(3, "Toggle webhook status", "")
        .item(4, "Delete a webhook", "")
        .item(5, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            if data.subscriptions.is_empty() {
                outro("No webhooks registered.")?;
            } else {
                let mut msg = String::from("Registered Webhooks:\n");
                for (i, sub) in data.subscriptions.iter().enumerate() {
                    let status = if sub.active { "ACTIVE" } else { "INACTIVE" };
                    msg.push_str(&format!(
                        "  {}. {} -> {} [{}] (Secret: {})\n",
                        i + 1,
                        sub.name,
                        sub.url,
                        status,
                        sub.secret
                    ));
                }
                outro(msg.trim_end())?;
            }
        }
        2 => {
            let name: String = input("Enter custom name").interact()?;
            let url: String = input("Enter callback URL").interact()?;

            if name.is_empty() || url.is_empty() {
                outro_cancel("Name and URL cannot be empty.")?;
                return Ok(());
            }

            let secret = generate_webhook_secret();

            data.subscriptions.push(Subscription {
                name,
                url,
                secret,
                active: true,
            });

            if let Ok(serialized) = serde_json::to_string_pretty(&data) {
                let _ = fs::write(&webhook_file, serialized);
                outro("Webhook successfully registered.")?;
            }
        }
        3 => {
            if data.subscriptions.is_empty() {
                outro_cancel("No webhooks to toggle.")?;
                return Ok(());
            }

            let mut select_prompt = select("Select a webhook to toggle active status");
            for (i, sub) in data.subscriptions.iter().enumerate() {
                select_prompt = select_prompt.item(i, sub.name.clone(), if sub.active { "ACTIVE" } else { "INACTIVE" });
            }
            let sub_choice: usize = select_prompt.interact()?;

            data.subscriptions[sub_choice].active = !data.subscriptions[sub_choice].active;
            if let Ok(serialized) = serde_json::to_string_pretty(&data) {
                let _ = fs::write(&webhook_file, serialized);
                outro("Successfully toggled status.")?;
            }
        }
        4 => {
            if data.subscriptions.is_empty() {
                outro_cancel("No webhooks to delete.")?;
                return Ok(());
            }

            let mut select_prompt = select("Select a webhook to delete");
            for (i, sub) in data.subscriptions.iter().enumerate() {
                select_prompt = select_prompt.item(i, sub.name.clone(), "");
            }
            let sub_choice: usize = select_prompt.interact()?;

            let removed = data.subscriptions.remove(sub_choice);
            if let Ok(serialized) = serde_json::to_string_pretty(&data) {
                let _ = fs::write(&webhook_file, serialized);
                outro(format!("Webhook {} deleted successfully.", removed.name))?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn generate_webhook_secret() -> String {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    let mut secret = String::new();
    for _ in 0..16 {
        let idx = rand::random::<usize>() % chars.len();
        secret.push(chars[idx]);
    }
    secret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_webhook_secret() {
        let secret = generate_webhook_secret();
        assert_eq!(secret.len(), 16);
        assert!(secret.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}

// Rust guideline compliant 2026-02-21
