use std::fs;
use std::io::{self, Write};
use athena_core::paths::get_athena_home;

pub fn run_plugins() {
    println!("\nAthena WASM Plugins Manager");
    println!("═════════════════════════════\n");
    println!("List, configure, and install sandboxed web-assembly agent extensions.");
    println!();

    let plugins_dir = get_athena_home().join("plugins");
    if !plugins_dir.exists() {
        let _ = fs::create_dir_all(&plugins_dir);
    }

    println!("Options:");
    println!("  1. List active WASM plugins");
    println!("  2. Register plugin template");
    println!("  3. Remove a plugin");
    println!("  4. Exit");
    println!();

    print!("  Choice [1-4]: ");
    io::stdout().flush().ok();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).ok();
    let choice = choice.trim().parse::<usize>().unwrap_or(4);

    match choice {
        1 => {
            println!("\nInstalled Plugins:");
            let plugins = list_plugin_files(&plugins_dir);
            if plugins.is_empty() {
                println!("  No WASM plugins installed.");
            } else {
                for name in plugins {
                    println!("  • {}", name);
                }
            }
        }
        2 => {
            println!("\nRegister New Plugin");
            print!("  Enter plugin name (e.g., custom-parser, code-analyzer): ");
            io::stdout().flush().ok();
            let mut name = String::new();
            io::stdin().read_line(&mut name).ok();
            let name = name.trim().to_string();

            if name.is_empty() {
                println!("  ✗ Plugin name cannot be empty.");
                return;
            }

            let plugin_path = plugins_dir.join(format!("{}.wasm", name));
            let wasm_skeleton = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
            match fs::write(&plugin_path, wasm_skeleton) {
                Ok(()) => {
                    println!("  ✓ Registered custom WASM plugin successfully!");
                    println!("  Created skeleton at: {}", plugin_path.display());
                }
                Err(e) => {
                    println!("  ✗ Failed to register plugin: {}", e);
                }
            }
        }
        3 => {
            print!("\n  Enter plugin name to remove (e.g. custom-parser.wasm): ");
            io::stdout().flush().ok();
            let mut name = String::new();
            io::stdin().read_line(&mut name).ok();
            let name = name.trim();

            let plugin_path = plugins_dir.join(name);
            if plugin_path.exists() {
                match fs::remove_file(&plugin_path) {
                    Ok(()) => println!("  ✓ Plugin '{}' removed successfully.", name),
                    Err(e) => println!("  ✗ Failed to remove plugin: {}", e),
                }
            } else {
                println!("  ✗ Plugin '{}' does not exist.", name);
            }
        }
        _ => {}
    }
}

pub fn list_plugin_files(plugins_dir: &std::path::Path) -> Vec<String> {
    let mut plugins = Vec::new();
    if let Ok(entries) = fs::read_dir(plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    plugins.push(name.to_string());
                }
            }
        }
    }
    plugins.sort();
    plugins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_plugin_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        fs::write(plugins_dir.join("analyzer.wasm"), b"wasm").unwrap();
        fs::write(plugins_dir.join("parser.wasm"), b"wasm").unwrap();

        let list = list_plugin_files(&plugins_dir);
        assert_eq!(list, vec!["analyzer.wasm".to_string(), "parser.wasm".to_string()]);
    }
}

// Rust guideline compliant 2026-02-21
