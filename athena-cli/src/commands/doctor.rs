use athena_core::config::{load_config, get_env_value};
use std::process::Command;

pub fn run_doctor() {
    println!("\nAthena Diagnostics (Doctor)");
    println!("═════════════════════════════\n");

    let mut issues_found = 0;

    // Check 1: ~/.athena directory exists
    let home = athena_core::paths::get_athena_home();
    if home.exists() {
        println!("  ✓ Home directory exists at ~/.athena");
    } else {
        println!("  ✗ Home directory ~/.athena does not exist!");
        issues_found += 1;
    }

    // Check 2: config.yaml exists and is valid
    let config_path = athena_core::paths::get_config_path();
    if config_path.exists() {
        println!("  ✓ Configuration file config.yaml exists");
        let config = load_config();
        if config.model.provider.is_empty() {
            println!("  ! Warning: Default model provider is not set.");
            issues_found += 1;
        } else {
            println!("  ✓ Active provider configured: {}", config.model.provider);
        }
    } else {
        println!("  ✗ config.yaml not found! Run 'athena setup' to create one.");
        issues_found += 1;
    }

    // Check 3: env file
    let env_path = athena_core::paths::get_env_path();
    if env_path.exists() {
        println!("  ✓ Environment file .env exists");
    } else {
        println!("  ! Warning: .env file not found.");
    }

    // Check 4: at least one API key configured
    let env_keys = [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "GOOGLE_API_KEY",
        "DEEPSEEK_API_KEY",
        "GROQ_API_KEY",
        "MISTRAL_API_KEY",
        "XAI_API_KEY",
    ];
    let mut keys_found = 0;
    for key in &env_keys {
        if get_env_value(key).is_some() {
            keys_found += 1;
        }
    }

    if keys_found > 0 {
        println!("  ✓ {} LLM API key(s) detected.", keys_found);
    } else {
        println!("  ✗ No LLM API keys configured! You won't be able to run queries. Run 'athena login'.");
        issues_found += 1;
    }

    // Check 5: execution environment dependencies
    let docker_check = Command::new("docker").arg("--version").output();
    if docker_check.is_err() {
        println!("  ! Warning: Docker is not installed or not in PATH. Container environments won't work.");
    }

    // Check 6: AGENTS.md workspace context
    if crate::context::load_agents_md().is_some() {
        println!("  ✓ AGENTS.md context loaded successfully.");
    } else {
        println!("  ! Warning: AGENTS.md not found in the workspace or home directory. Athena will not have custom contextual instructions.");
    }

    if issues_found == 0 {
        println!("\n✓ No critical issues found! Your Athena installation looks healthy.");
    } else {
        println!("\n✗ Found {} issue(s). Please follow the recommendations above to resolve them.", issues_found);
    }
}

// Rust guideline compliant 2026-02-21
