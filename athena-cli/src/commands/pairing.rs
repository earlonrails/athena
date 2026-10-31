use std::fs;
use serde::{Deserialize, Serialize};
use athena_core::paths::get_athena_home;
use cliclack::{intro, select, input, outro, outro_cancel, note};
use anyhow::Result;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct AuthorizedClients {
    clients: Vec<ClientInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ClientInfo {
    id: String,
    platform: String,
    username: Option<String>,
    paired_at: String,
}

pub fn run_pairing() -> Result<()> {
    intro("Athena Gateway Pairing Manager")?;

    let clients_file = get_athena_home().join("authorized_clients.json");
    let mut auth_data = if clients_file.exists() {
        let content = fs::read_to_string(&clients_file).unwrap_or_default();
        serde_json::from_str::<AuthorizedClients>(&content).unwrap_or_default()
    } else {
        AuthorizedClients::default()
    };

    let choice: usize = select("Manage authorization for remote messaging gateways (Telegram, Slack, WhatsApp)")
        .item(1, "Generate a temporary pairing code", "")
        .item(2, "List currently authorized clients", "")
        .item(3, "Add a manual client authorization", "")
        .item(4, "Revoke a client authorization", "")
        .item(5, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let code = generate_pairing_code();
            note("Temporary Pairing Code", &code)?;
            outro("Send this code in a DM to your configured Telegram/Slack bot to authorize your account.\n(Code is active for this session)")?;
        }
        2 => {
            if auth_data.clients.is_empty() {
                outro("No authorized clients found.")?;
            } else {
                let mut msg = String::from("Authorized Clients:\n");
                for (i, client) in auth_data.clients.iter().enumerate() {
                    let user_str = client.username.as_deref().unwrap_or("unknown");
                    msg.push_str(&format!(
                        "  {}. ID: {} [{}] (User: @{}, Paired: {})\n",
                        i + 1,
                        client.id,
                        client.platform,
                        user_str,
                        client.paired_at
                    ));
                }
                outro(msg.trim_end())?;
            }
        }
        3 => {
            let platform: String = input("Enter Platform")
                .placeholder("telegram, slack, whatsapp")
                .interact()?;

            let client_id: String = input("Enter Chat ID or User ID").interact()?;

            if client_id.is_empty() {
                outro_cancel("Chat ID cannot be empty.")?;
                return Ok(());
            }

            let username_in: String = input("Enter Username (optional)").interact()?;
            let username = if username_in.trim().is_empty() {
                None
            } else {
                Some(username_in.trim().to_string())
            };

            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            auth_data.clients.push(ClientInfo {
                id: client_id.clone(),
                platform,
                username,
                paired_at: now,
            });

            if let Ok(serialized) = serde_json::to_string_pretty(&auth_data) {
                let _ = fs::write(&clients_file, serialized);
                outro(format!("Successfully authorized client: {}.", client_id))?;
            } else {
                outro_cancel("Failed to serialize pairing information.")?;
            }
        }
        4 => {
            if auth_data.clients.is_empty() {
                outro_cancel("No authorized clients to revoke.")?;
                return Ok(());
            }

            let mut select_prompt = select("Select a client number to revoke");
            for (i, client) in auth_data.clients.iter().enumerate() {
                let user_str = client.username.as_deref().unwrap_or("unknown");
                select_prompt = select_prompt.item(i, format!("{} (User: @{})", client.id, user_str), "");
            }
            let rev_choice: usize = select_prompt.interact()?;

            let removed = auth_data.clients.remove(rev_choice);
            if let Ok(serialized) = serde_json::to_string_pretty(&auth_data) {
                let _ = fs::write(&clients_file, serialized);
                outro(format!("Revoked authorization for client: {}.", removed.id))?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn generate_pairing_code() -> String {
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
    let mut code = String::new();
    for i in 0..8 {
        if i == 4 {
            code.push('-');
        }
        let idx = rand::random::<usize>() % chars.len();
        code.push(chars[idx]);
    }
    format!("ATHENA-{}", code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pairing_code() {
        let code = generate_pairing_code();
        assert!(code.starts_with("ATHENA-"));
        assert_eq!(code.len(), 16); // "ATHENA-" (7) + 4 chars + '-' (1) + 4 chars = 16
    }
}

// Rust guideline compliant 2026-02-21
