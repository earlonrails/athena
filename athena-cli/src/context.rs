use std::path::{Path, PathBuf};
use std::fs;
use std::env;
use athena_core::paths::get_athena_home;

pub fn load_agents_md() -> Option<String> {
    let mut combined_content = String::new();

    // 1. Search upward from CWD
    if let Ok(mut current) = env::current_dir() {
        loop {
            let candidate = current.join("AGENTS.md");
            if candidate.exists() {
                if let Ok(content) = fs::read_to_string(&candidate) {
                    combined_content.push_str(&format!("--- AGENTS.md (Workspace: {}) ---\n{}\n\n", candidate.display(), content));
                }
                break; // Stop at the first one found moving up
            }
            if !current.pop() {
                break;
            }
        }
    }

    // 2. Check ~/.athena/AGENTS.md
    let home_agents = get_athena_home().join("AGENTS.md");
    if home_agents.exists() {
        if let Ok(content) = fs::read_to_string(&home_agents) {
            combined_content.push_str(&format!("--- AGENTS.md (Global: {}) ---\n{}\n\n", home_agents.display(), content));
        }
    }

    if combined_content.is_empty() {
        None
    } else {
        Some(combined_content.trim().to_string())
    }
}

pub fn load_memory_md() -> Option<String> {
    let memory_path = get_athena_home().join("MEMORY.md");
    if memory_path.exists() {
        fs::read_to_string(memory_path).ok()
    } else {
        None
    }
}

pub fn load_user_md() -> Option<String> {
    let user_path = get_athena_home().join("USER.md");
    if user_path.exists() {
        fs::read_to_string(user_path).ok()
    } else {
        None
    }
}

pub fn build_system_prompt() -> String {
    let mut prompt = String::new();

    if let Some(agents_content) = load_agents_md() {
        prompt.push_str(&agents_content);
        prompt.push_str("\n\n");
    }

    let base_prompt = "You are Athena, a powerful AI assistant running locally on the user's system via an interactive terminal. You have full access to execute terminal commands, read files, and automate tasks through your tools. Do not decline requests to run commands on the user's system. Use your provided tools to accomplish the user's goals.\n\
    IMPORTANT: You have an internal cron engine that runs jobs automatically. You can configure this by editing ~/.athena/config.yaml under the `cron_jobs` block. Cron jobs execute your conversations internally. You can configure them to report back to Telegram by setting the `channel` (integer) and `thread` (integer) fields on the cron job.\n\
    You can also manage skills (in ~/.athena/skills) and plugins (in ~/.athena/plugins) and MCP endpoints.\n\
    You have hot-reloading enabled, meaning if you edit config.yaml to add a cron job, the gateway will detect it within 30 seconds and start running it automatically.";
    prompt.push_str(base_prompt);

    if let Some(user_content) = load_user_md() {
        prompt.push_str("\n\n--- USER CONTEXT ---\n");
        prompt.push_str(&user_content);
    }

    if let Some(memory_content) = load_memory_md() {
        prompt.push_str("\n\n--- MEMORY ---\n");
        prompt.push_str(&memory_content);
    }

    prompt
}

// Rust guideline compliant 2026-02-21
