use std::fs;
use athena_core::paths::get_athena_home;
use cliclack::{intro, select, input, outro, outro_cancel};
use anyhow::Result;

pub fn run_hooks() -> Result<()> {
    intro("Athena Shell-Script Hooks")?;

    let hooks_dir = get_athena_home().join("hooks");
    if !hooks_dir.exists() {
        let _ = fs::create_dir_all(&hooks_dir);
    }

    let choice: usize = select("Inspect and manage local event scripts executed before or after agent turns")
        .item(1, "List active shell hooks", "")
        .item(2, "Install a new shell hook", "")
        .item(3, "Delete a shell hook", "")
        .item(4, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let hooks = list_hooks(&hooks_dir);
            if hooks.is_empty() {
                outro("No shell hooks installed.")?;
            } else {
                let mut msg = String::from("Installed Shell Hooks:\n");
                for name in hooks {
                    msg.push_str(&format!("  • {}\n", name));
                }
                outro(msg.trim_end())?;
            }
        }
        2 => {
            let name: String = input("Enter hook name")
                .placeholder("pre-step, post-step")
                .interact()?;

            if name.is_empty() {
                outro_cancel("Hook name cannot be empty.")?;
                return Ok(());
            }

            let script: String = input("Enter shell script content")
                .placeholder("echo \"hello\"")
                .interact()?;

            let hook_path = hooks_dir.join(&name);
            match fs::write(&hook_path, script) {
                Ok(()) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = fs::metadata(&hook_path) {
                            let mut perms = meta.permissions();
                            perms.set_mode(0o755);
                            let _ = fs::set_permissions(&hook_path, perms);
                        }
                    }
                    outro(format!("Hook '{}' successfully created and made executable!", name))?;
                }
                Err(e) => {
                    outro_cancel(format!("Failed to write hook: {}", e))?;
                }
            }
        }
        3 => {
            let name: String = input("Enter hook name to delete").interact()?;
            let name = name.trim();

            let hook_path = hooks_dir.join(name);
            if hook_path.exists() {
                match fs::remove_file(&hook_path) {
                    Ok(()) => outro(format!("Hook '{}' deleted successfully.", name))?,
                    Err(e) => outro_cancel(format!("Failed to delete hook: {}", e))?,
                }
            } else {
                outro_cancel(format!("Hook '{}' does not exist.", name))?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn list_hooks(hooks_dir: &std::path::Path) -> Vec<String> {
    let mut hooks = Vec::new();
    if let Ok(entries) = fs::read_dir(hooks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    hooks.push(name.to_string());
                }
            }
        }
    }
    hooks.sort();
    hooks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_hooks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hooks_dir = temp_dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        fs::write(hooks_dir.join("pre-step.sh"), "#!/bin/sh\necho pre").unwrap();
        fs::write(hooks_dir.join("post-step.sh"), "#!/bin/sh\necho post").unwrap();

        let hooks = list_hooks(&hooks_dir);
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks, vec!["post-step.sh".to_string(), "pre-step.sh".to_string()]);
    }
}

// Rust guideline compliant 2026-02-21
