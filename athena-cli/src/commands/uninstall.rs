use std::fs;
use cliclack::{intro, confirm, outro, outro_cancel, note};
use anyhow::Result;

pub fn run_uninstall() -> Result<()> {
    intro("Uninstall Athena Agent")?;
    note("Warning", "This will remove the ~/.athena config directory, skills, database, and settings.")?;

    let confirm_un: bool = confirm("Are you sure you want to proceed?").interact()?;

    if !confirm_un {
        outro_cancel("Uninstall cancelled.")?;
        return Ok(());
    }

    let home = athena_core::paths::get_athena_home();
    if home.exists() {
        match uninstall_directory(&home) {
            Ok(()) => outro(format!("Removed directory {}", home.display()))?,
            Err(e) => outro_cancel(format!("Failed to remove home directory: {}", e))?,
        }
    } else {
        outro_cancel(format!("Home directory {} does not exist.", home.display()))?;
    }

    note("Next Steps", "To completely uninstall, please remove the 'athena' binary from your PATH/cargo bin directory.")?;
    Ok(())
}

pub fn uninstall_directory(home: &std::path::Path) -> std::io::Result<()> {
    fs::remove_dir_all(home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninstall_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_dir = temp_dir.path().join("fake_athena_home");
        fs::create_dir_all(&test_dir).unwrap();
        fs::write(test_dir.join("test.txt"), "hello").unwrap();

        assert!(test_dir.exists());
        assert!(uninstall_directory(&test_dir).is_ok());
        assert!(!test_dir.exists());
    }
}

// Rust guideline compliant 2026-02-21
