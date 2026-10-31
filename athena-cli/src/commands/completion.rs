use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io::{self, Write};
use crate::Args;

pub fn run_completion() {
    println!("\nAthena Shell Completion Generator");
    println!("═════════════════════════════════════\n");
    println!("Select your shell to generate the auto-completion script:");
    println!("  1. Bash");
    println!("  2. Zsh");
    println!("  3. Fish");
    println!("  4. PowerShell");
    println!("  5. Elvish");
    println!();

    print!("  Choice [1-5]: ");
    io::stdout().flush().ok();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).ok();
    let choice = choice.trim().parse::<usize>().unwrap_or(1);

    let shell = match choice {
        1 => Shell::Bash,
        2 => Shell::Zsh,
        3 => Shell::Fish,
        4 => Shell::PowerShell,
        5 => Shell::Elvish,
        _ => Shell::Bash,
    };

    println!("\n# --- COPY THE SCRIPT BELOW ---");
    let script = generate_completion_script(shell);
    let _ = io::stdout().write_all(&script);
    println!("# --- END OF SCRIPT ---");

    println!("\nTip: To load this automatically on startup, redirect this output to a file and source it:");
    match shell {
        Shell::Bash => println!("  athena completion > ~/.athena/completion.bash\n  echo \"source ~/.athena/completion.bash\" >> ~/.bashrc"),
        Shell::Zsh => println!("  athena completion > ~/.athena/completion.zsh\n  echo \"source ~/.athena/completion.zsh\" >> ~/.zshrc"),
        Shell::Fish => println!("  athena completion > ~/.config/fish/completions/athena.fish"),
        _ => {}
    }
}

pub fn generate_completion_script(shell: Shell) -> Vec<u8> {
    let mut cmd = Args::command();
    let mut buf = Vec::new();
    generate(shell, &mut cmd, "athena", &mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_completion_script() {
        let bash_script = generate_completion_script(Shell::Bash);
        assert!(!bash_script.is_empty());
        let bash_str = String::from_utf8_lossy(&bash_script);
        assert!(bash_str.contains("athena"));
    }
}

// Rust guideline compliant 2026-02-21
