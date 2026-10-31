use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use athena_core::paths::get_athena_home;
use athena_core::config::{load_config, get_env_value};

pub fn run_debug() {
    println!("\nAthena Debug & Diagnostic Packager");
    println!("═════════════════════════════════════\n");
    println!("Gathering local environment statistics, workspace directories, and model provider status...");
    println!();

    let report = generate_debug_report();
    print!("{}", report);

    // Write to a local file
    let report_path = PathBuf::from("./athena-debug-report.txt");
    if let Ok(mut file) = File::create(&report_path) {
        if file.write_all(report.as_bytes()).is_ok() {
            println!("\n✓ Diagnostic report generated successfully!");
            println!("Saved to: {}", report_path.display());
        }
    } else {
        println!("\n✗ Failed to save diagnostic report locally.");
    }
}

pub fn generate_debug_report() -> String {
    let mut report = String::new();
    report.push_str("=== ATHENA DEBUG REPORT ===\n\n");

    let version = env!("CARGO_PKG_VERSION");
    let version_line = format!("Athena Version : v{} (Rust Rewrite)\n", version);
    report.push_str(&version_line);

    let os_line = format!("OS Platform    : {}\n", std::env::consts::OS);
    let arch_line = format!("Architecture   : {}\n", std::env::consts::ARCH);
    report.push_str(&os_line);
    report.push_str(&arch_line);

    if let Ok(exe_path) = std::env::current_exe() {
        let exe_line = format!("Binary Path    : {}\n", exe_path.display());
        report.push_str(&exe_line);
    }

    let home = get_athena_home();
    let home_line = format!("Athena Home    : {}\n", home.display());
    report.push_str(&home_line);

    let config = load_config();
    let default_model_line = format!("Default Model  : {}\n", config.model.default);
    report.push_str(&default_model_line);

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

    let mut prov_report = String::from("\n=== ACTIVE PROVIDER KEYS (MASKED) ===\n");
    for (env_var, label) in providers {
        let status = if let Some(val) = get_env_value(env_var) {
            if val.len() > 8 {
                format!("ACTIVE ({}...{})", &val[..4], &val[val.len()-4..])
            } else {
                "ACTIVE (****)".to_string()
            }
        } else {
            "NOT CONFIGURED".to_string()
        };
        let line = format!("  • {:<12} : {}\n", label, status);
        prov_report.push_str(&line);
    }
    report.push_str(&prov_report);
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_debug_report() {
        let report = generate_debug_report();
        assert!(report.contains("ATHENA DEBUG REPORT"));
        assert!(report.contains("Athena Version"));
        assert!(report.contains("OS Platform"));
        assert!(report.contains("ACTIVE PROVIDER KEYS"));
    }
}

// Rust guideline compliant 2026-02-21
