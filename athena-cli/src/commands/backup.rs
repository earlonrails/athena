use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use athena_core::paths::get_athena_home;
use cliclack::{intro, input, outro, outro_cancel, spinner};
use anyhow::Result;

pub fn run_backup() -> Result<()> {
    intro("Athena Backup Utility")?;

    let home_dir = get_athena_home();
    if !home_dir.exists() {
        outro_cancel(format!("No ~/.athena configuration directory found at {}.", home_dir.display()))?;
        return Ok(());
    }

    let dest_str: String = input("Enter backup destination file path")
        .default_input("./athena-backup.zip")
        .interact()?;
    let dest_str = dest_str.trim().to_string();

    let dest_path = PathBuf::from(dest_str);
    
    let s = spinner();
    s.start(format!("Creating backup zip at {}...", dest_path.display()));

    let file = match File::create(&dest_path) {
        Ok(f) => f,
        Err(e) => {
            s.error(format!("Failed to create backup file: {}", e));
            return Ok(());
        }
    };

    let mut zip = zip::ZipWriter::new(file);

    match add_directory_to_zip(&mut zip, &home_dir, &home_dir) {
        Ok(()) => {
            if let Err(e) = zip.finish() {
                s.error(format!("Failed to finalize zip archive: {}", e));
            } else {
                s.stop("Backup created successfully!");
                outro(format!("Saved to: {}", dest_path.display()))?;
            }
        }
        Err(e) => {
            s.error(format!("Failed to write files to backup: {}", e));
        }
    }
    
    Ok(())
}

fn add_directory_to_zip<W: Write + io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    base_path: &Path,
    current_dir: &Path,
) -> io::Result<()> {
    if !current_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let relative_path = path.strip_prefix(base_path).unwrap();
            let relative_str = format!("{}/", relative_path.to_str().unwrap().replace('\\', "/"));

            // Skip logs directory entirely
            if relative_str.starts_with("logs/") {
                continue;
            }

            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            let _ = zip.add_directory(&relative_str, options);

            add_directory_to_zip(zip, base_path, &path)?;
        } else {
            let relative_path = path.strip_prefix(base_path).unwrap();
            let relative_str = relative_path.to_str().unwrap().replace('\\', "/");

            let mut f = File::open(&path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;

            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file(relative_str, options)?;
            zip.write_all(&buffer)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_add_directory_to_zip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path = temp_dir.path().join("athena_home");
        std::fs::create_dir_all(src_path.join("skills")).unwrap();
        std::fs::create_dir_all(src_path.join("logs")).unwrap();

        std::fs::write(src_path.join("config.yaml"), "version: 1").unwrap();
        std::fs::write(src_path.join("skills").join("test_skill.md"), "# Skill").unwrap();
        std::fs::write(src_path.join("logs").join("agent.log"), "ignore this log").unwrap();

        let buffer = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buffer);
        add_directory_to_zip(&mut zip, &src_path, &src_path).unwrap();
        let finished = zip.finish().unwrap();

        // Read back the zip file
        let reader = Cursor::new(finished.into_inner());
        let archive = zip::ZipArchive::new(reader).unwrap();

        let mut names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        names.sort();

        assert!(names.contains(&"config.yaml".to_string()));
        assert!(names.contains(&"skills/test_skill.md".to_string()));
        // logs must be excluded
        assert!(!names.iter().any(|n| n.starts_with("logs")));
    }
}

// Rust guideline compliant 2026-02-21
