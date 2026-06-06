use std::path::PathBuf;
use athena_core::paths::get_athena_home;
use cliclack::{intro, select, input, outro, outro_cancel, note};
use anyhow::Result;
use athena_skills::{AgentSkillsHub, SkillManager, SkillStore};

pub fn run_skills() -> Result<()> {
    intro("Athena Semantic Skills & AgentSkills.io")?;
    note("Info", "Manage semantic skills and import/export via agentskills.io format.")?;

    let db_path = get_athena_home().join("skills.db");
    let store = SkillStore::new(&db_path)?;
    let manager = SkillManager::new(&db_path)?;

    let choice: usize = select("Options")
        .item(1, "List installed skills", "")
        .item(2, "Import from agentskills.json", "")
        .item(3, "Export to agentskills.json", "")
        .item(4, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let skills = store.get_all_skills()?;
            if skills.is_empty() {
                outro("No active skills found.")?;
            } else {
                let mut msg = String::from("Installed Skills:\n");
                for skill in skills {
                    msg.push_str(&format!(
                        "  • {} (Usage: {}, Success: {})\n      {}\n",
                        skill.name, skill.usage_count, skill.success_count, skill.description
                    ));
                }
                outro(msg.trim_end())?;
            }
        }
        2 => {
            let path_str: String = input("Enter path to agentskills.json")
                .placeholder("agentskills.json")
                .interact()?;
            let path = PathBuf::from(path_str.trim());
            
            if !path.exists() {
                outro_cancel("File does not exist.")?;
                return Ok(());
            }

            match AgentSkillsHub::import_skills(&store, &manager, &path) {
                Ok(count) => note("Success", format!("Imported {} skills.", count))?,
                Err(e) => outro_cancel(format!("Failed to import skills: {}", e))?,
            }
        }
        3 => {
            let path_str: String = input("Enter output path")
                .placeholder("exported_skills.json")
                .interact()?;
            let path = PathBuf::from(path_str.trim());

            match AgentSkillsHub::export_skills(&store, &path) {
                Ok(()) => note("Success", format!("Exported skills to {}", path.display()))?,
                Err(e) => outro_cancel(format!("Failed to export skills: {}", e))?,
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

// Rust guideline compliant 2026-02-21
