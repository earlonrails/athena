use std::process::Command;

pub fn run_update() {
    println!("\nUpdating Athena Agent...");
    println!("═════════════════════════\n");

    let mut cmd = resolve_update_command();

    match cmd.status() {
        Ok(status) => {
            if status.success() {
                println!("\n✓ Athena Agent updated successfully!");
            } else {
                println!("\n✗ Update failed with exit code: {}", status);
            }
        }
        Err(e) => {
            println!("\n✗ Failed to execute install script: {}", e);
        }
    }
}

pub fn resolve_update_command() -> Command {
    let local_script = athena_core::paths::get_athena_home().join("install.sh");
    if local_script.exists() {
        let mut cmd = Command::new("bash");
        cmd.arg(local_script);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("curl -fsSL https://raw.githubusercontent.com/earlonrails/athena/main/install.sh | bash");
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_update_command() {
        let cmd = resolve_update_command();
        let program = cmd.get_program();
        assert!(program == "bash" || program == "sh");
    }
}

// Rust guideline compliant 2026-02-21
