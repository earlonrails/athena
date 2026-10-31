use athena_core::config::{load_config, save_config};
use cliclack::{intro, select, outro, outro_cancel, note};
use anyhow::Result;

pub fn run_tools() -> Result<()> {
    intro("Athena Tools Configuration")?;
    note("Info", "Configure which local capabilities (filesystem, search, execution) are enabled.")?;

    let mut config = load_config();
    let all_tools = ["filesystem_read", "filesystem_write", "web_search", "command_execution", "browser_automation"];

    let msg = format_tools_status(&config.tools.disabled, &all_tools);
    
    let choice: usize = select(msg.trim_end())
        .item(1, "Toggle a tool's enabled/disabled status", "")
        .item(2, "Enable all tools", "")
        .item(3, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let mut select_prompt = select("Select tool to toggle");
            for (i, tool) in all_tools.iter().enumerate() {
                let disabled = config.tools.disabled.contains(&tool.to_string());
                let status = if disabled { "DISABLED" } else { "ENABLED" };
                select_prompt = select_prompt.item(i, *tool, status);
            }
            let tool_choice: usize = select_prompt.interact()?;

            let selected = all_tools[tool_choice].to_string();
            if config.tools.disabled.contains(&selected) {
                config.tools.disabled.retain(|t| t != &selected);
                outro(format!("Enabled tool: {}.", selected))?;
            } else {
                config.tools.disabled.push(selected.clone());
                outro(format!("Disabled tool: {}.", selected))?;
            }

            let _ = save_config(&config);
        }
        2 => {
            config.tools.disabled.clear();
            if save_config(&config).is_ok() {
                outro("All tools successfully enabled.")?;
            } else {
                outro_cancel("Failed to save config.")?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn format_tools_status(disabled_tools: &[String], all_tools: &[&str]) -> String {
    let mut msg = String::from("Current Tools Status:\n");
    for (i, tool) in all_tools.iter().enumerate() {
        let disabled = disabled_tools.contains(&tool.to_string());
        let status = if disabled { "DISABLED" } else { "ENABLED" };
        msg.push_str(&format!("  {}. {:<22} : {}\n", i + 1, tool, status));
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tools_status() {
        let all_tools = ["filesystem_read", "command_execution"];
        let disabled = vec!["command_execution".to_string()];
        let status = format_tools_status(&disabled, &all_tools);
        assert!(status.contains("filesystem_read"));
        assert!(status.contains("ENABLED"));
        assert!(status.contains("command_execution"));
        assert!(status.contains("DISABLED"));
    }
}

// Rust guideline compliant 2026-02-21
