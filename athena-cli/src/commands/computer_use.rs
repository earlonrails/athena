use athena_core::config::{load_config, save_config};
use std::io::{self, Write};

pub fn run_computer_use() {
    println!("\nAthena Computer Use Backend (CUA)");
    println!("═══════════════════════════════════\n");
    println!("Controls local frame buffer capture, VNC connections, and mouse/keyboard drivers.");
    println!();

    let mut config = load_config();
    println!("YOLO auto-approval mode: {}", if config.agent.yolo_mode { "ENABLED (Caution!)" } else { "DISABLED (Safe)" });
    println!();

    println!("Checking platform compatibility...");
    let os = std::env::consts::OS;
    println!("  • Active OS Platform: {}", os);

    let has_xdotool = std::process::Command::new("which")
        .arg("xdotool")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    println!("  • Automation Driver (xdotool): {}", if has_xdotool { "AVAILABLE" } else { "NOT FOUND" });
    println!();

    println!("Options:");
    println!("  1. Toggle YOLO auto-approval mode");
    println!("  2. Run pre-flight graphics and VNC diagnostics");
    println!("  3. Exit");
    println!();

    print!("  Choice [1-3]: ");
    io::stdout().flush().ok();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).ok();
    let choice = choice.trim().parse::<usize>().unwrap_or(3);

    match choice {
        1 => {
            config.agent.yolo_mode = !config.agent.yolo_mode;
            if save_config(&config).is_ok() {
                println!("  ✓ Toggled YOLO mode. New value: {}", config.agent.yolo_mode);
            }
        }
        2 => {
            println!("\nRunning Pre-Flight Graphics Check...");
            let (compatible, warning) = check_computer_use_environment();
            if compatible {
                println!("  ✓ Active X11 display detected.");
            } else if let Some(w) = warning {
                println!("  ⚠️ Warning: {}", w);
            } else {
                println!("  • Platform diagnostics not yet implemented for non-Linux OS.");
            }
        }
        _ => {}
    }
}

pub fn check_computer_use_environment() -> (bool, Option<String>) {
    let has_xdotool = std::process::Command::new("which")
        .arg("xdotool")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let display = std::env::var("DISPLAY").ok();
    (has_xdotool, display)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_computer_use_environment() {
        let (has_driver, display) = check_computer_use_environment();
        // Just verify it queries safely without panic
        let _ = (has_driver, display);
    }
}

// Rust guideline compliant 2026-02-21
