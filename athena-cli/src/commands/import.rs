use std::fs::{self, File};
use std::path::PathBuf;
use athena_core::paths::get_athena_home;
use cliclack::{intro, input, confirm, outro_cancel, spinner};
use anyhow::Result;

pub fn run_import() -> Result<()> {
    intro("Athena Restore & Import Utility")?;

    let backup_str: String = input("Enter backup file path to restore")
        .default_input("./athena-backup.zip")
        .interact()?;
    let backup_str = backup_str.trim().to_string();

    let backup_path = PathBuf::from(backup_str);
    if !backup_path.exists() {
        outro_cancel(format!("Backup file {} does not exist.", backup_path.display()))?;
        return Ok(());
    }

    let home_dir = get_athena_home();
    let confirm_restore: bool = confirm(format!("Are you sure you want to restore? This will overwrite existing files in {}!", home_dir.display()))
        .interact()?;

    if !confirm_restore {
        outro_cancel("Restore cancelled.")?;
        return Ok(());
    }

    let s = spinner();
    s.start(format!("Extracting backup into {}...", home_dir.display()));

    let file = match File::open(&backup_path) {
        Ok(f) => f,
        Err(e) => {
            s.error(format!("Failed to open backup file: {}", e));
            return Ok(());
        }
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            s.error(format!("Failed to parse zip archive: {}", e));
            return Ok(());
        }
    };

    if !home_dir.exists() {
        let _ = fs::create_dir_all(&home_dir);
    }

    match extract_zip_to_dir(&mut archive, &home_dir) {
        Ok(_) => s.stop("Restore completed successfully!"),
        Err(e) => s.error(format!("Restore failed: {}", e)),
    }
    
    Ok(())
}

pub fn extract_zip_to_dir<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    target_dir: &std::path::Path,
) -> std::io::Result<usize> {
    let mut count = 0;
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let outpath = match file.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            let _ = fs::create_dir_all(&outpath);
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    let _ = fs::create_dir_all(p);
                }
            }

            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_extract_zip_to_dir() {
        let buffer = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buffer);
        let options = zip::write::FileOptions::default();
        zip.start_file("restored_file.txt", options).unwrap();
        use std::io::Write;
        zip.write_all(b"restored content").unwrap();
        let finished = zip.finish().unwrap();

        let reader = Cursor::new(finished.into_inner());
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let extracted = extract_zip_to_dir(&mut archive, temp_dir.path()).unwrap();
        assert_eq!(extracted, 1);
        let restored_path = temp_dir.path().join("restored_file.txt");
        assert!(restored_path.exists());
        assert_eq!(fs::read_to_string(restored_path).unwrap(), "restored content");
    }
}

// Rust guideline compliant 2026-02-21
