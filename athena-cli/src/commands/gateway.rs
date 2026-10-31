use crate::GatewayCommands;
use cliclack::{intro, select, outro, outro_cancel, spinner, note};
use std::process::{Command, Stdio};
use std::path::PathBuf;

pub fn run_gateway(command: &Option<GatewayCommands>) {
    match command {
        Some(GatewayCommands::Install) => install_gateway(),
        Some(GatewayCommands::Start) => start_gateway(),
        Some(GatewayCommands::Stop) => stop_gateway(),
        Some(GatewayCommands::Status) => status_gateway(),
        Some(GatewayCommands::Logs) => logs_gateway(),
        Some(GatewayCommands::Run) => run_foreground_gateway(),
        None => interactive_menu(),
    }
}

fn interactive_menu() {
    intro("Athena Gateway Manager").ok();

    let choice = select("What would you like to do?")
        .item("status", "Check gateway status", "")
        .item("start", "Start background gateway", "")
        .item("stop", "Stop background gateway", "")
        .item("logs", "View gateway logs", "")
        .item("run", "Run gateway in foreground", "")
        .item("install", "Install systemd user service", "")
        .item("exit", "Exit", "")
        .interact();

    match choice {
        Ok("status") => status_gateway(),
        Ok("start") => start_gateway(),
        Ok("stop") => stop_gateway(),
        Ok("logs") => logs_gateway(),
        Ok("run") => run_foreground_gateway(),
        Ok("install") => install_gateway(),
        _ => {
            outro("Goodbye!").ok();
        }
    }
}

fn install_gateway() {
    intro("Installing Athena Gateway Daemon").ok();

    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("athena"));
    let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("/usr/local/bin"));
    let gateway_binary = exe_dir.join("athena-gateway");

    if !gateway_binary.exists() {
        note("Warning", format!("Could not find athena-gateway at {:?}. Will assume it's in your PATH.", gateway_binary)).ok();
    }

    let service_content = generate_systemd_service_file(&gateway_binary);

    let systemd_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"))
        .join("systemd/user");

    let s = spinner();
    s.start("Creating systemd user service...");

    if let Err(e) = std::fs::create_dir_all(&systemd_dir) {
        s.error(format!("Failed to create systemd config directory: {}", e));
        return;
    }

    let service_path = systemd_dir.join("athena-gateway.service");
    if let Err(e) = std::fs::write(&service_path, service_content) {
        s.error(format!("Failed to write service file: {}", e));
        return;
    }
    s.stop("Service file created.");

    // Daemon reload
    let s = spinner();
    s.start("Reloading systemd daemon...");
    let _ = Command::new("systemctl").args(&["--user", "daemon-reload"]).output();
    s.stop("Daemon reloaded.");

    // Enable
    let s = spinner();
    s.start("Enabling athena-gateway service...");
    let _ = Command::new("systemctl").args(&["--user", "enable", "athena-gateway"]).output();
    s.stop("Service enabled.");

    // Start
    let s = spinner();
    s.start("Starting athena-gateway service...");
    let out = Command::new("systemctl").args(&["--user", "start", "athena-gateway"]).output();
    if let Ok(output) = out {
        if output.status.success() {
            s.stop("Service started successfully.");
            outro("Athena Gateway is now running in the background! Use 'athena gateway logs' to view output.").ok();
        } else {
            s.error(format!("Failed to start service: {}", String::from_utf8_lossy(&output.stderr)));
        }
    } else {
        s.error("Failed to execute systemctl.");
    }
}

fn start_gateway() {
    intro("Starting Athena Gateway").ok();
    let s = spinner();
    s.start("Executing systemctl --user start athena-gateway...");
    let out = Command::new("systemctl").args(&["--user", "start", "athena-gateway"]).output();
    if let Ok(output) = out {
        if output.status.success() {
            s.stop("Gateway started.");
            outro("Success!").ok();
        } else {
            s.error(format!("Failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
    } else {
        s.error("Could not run systemctl.");
    }
}

fn stop_gateway() {
    intro("Stopping Athena Gateway").ok();
    let s = spinner();
    s.start("Executing systemctl --user stop athena-gateway...");
    let out = Command::new("systemctl").args(&["--user", "stop", "athena-gateway"]).output();
    if let Ok(output) = out {
        if output.status.success() {
            s.stop("Gateway stopped.");
            outro("Success!").ok();
        } else {
            s.error(format!("Failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
    } else {
        s.error("Could not run systemctl.");
    }
}

fn status_gateway() {
    intro("Athena Gateway Status").ok();
    let out = Command::new("systemctl").args(&["--user", "status", "athena-gateway"]).output();
    if let Ok(output) = out {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            note("Service Status", stdout).ok();
            outro("Gateway is active.").ok();
        } else {
            if !stdout.is_empty() {
                note("Service Status", stdout).ok();
            } else {
                note("Error", stderr).ok();
            }
            outro_cancel("Gateway is not running or not installed.").ok();
        }
    } else {
        outro_cancel("Could not run systemctl.").ok();
    }
}

fn logs_gateway() {
    println!("Following logs for athena-gateway (Press Ctrl+C to stop)...");
    let mut child = Command::new("journalctl")
        .args(&["--user", "-u", "athena-gateway", "-f"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to start journalctl");
        
    let _ = child.wait();
}

fn run_foreground_gateway() {
    intro("Running Athena Gateway in Foreground").ok();

    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("athena"));
    let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("/usr/local/bin"));
    let gateway_binary = exe_dir.join("athena-gateway");

    if !gateway_binary.exists() {
        note("Warning", format!("Could not find athena-gateway at {:?}. Will assume it's in your PATH.", gateway_binary)).ok();
    }

    note("Info", "Starting Athena Telegram Gateway in foreground.\nThis gateway connects via Telegram long-polling and does NOT bind to a local HTTP port.").ok();

    let cmd = if gateway_binary.exists() { 
        gateway_binary.to_str().unwrap() 
    } else { 
        "athena-gateway" 
    };

    let mut child = match Command::new(cmd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn() {
            Ok(c) => c,
            Err(e) => {
                outro_cancel(format!("Failed to start athena-gateway (tried '{}'). Error: {}. Is the gateway binary installed or in your PATH?", cmd, e)).ok();
                return;
            }
        };
        
    let _ = child.wait();
    outro("Gateway stopped.").ok();
}

pub fn generate_systemd_service_file(binary_path: &std::path::Path) -> String {
    format!(
r#"[Unit]
Description=Athena Background Gateway
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=5
Environment="RUST_LOG=info"

[Install]
WantedBy=default.target
"#,
        binary_path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_generate_systemd_service_file() {
        let path = Path::new("/usr/local/bin/athena-gateway");
        let service = generate_systemd_service_file(path);
        assert!(service.contains("Description=Athena Background Gateway"));
        assert!(service.contains("ExecStart=/usr/local/bin/athena-gateway"));
        assert!(service.contains("Restart=on-failure"));
        assert!(service.contains("WantedBy=default.target"));
    }
}

// Rust guideline compliant 2026-02-21

