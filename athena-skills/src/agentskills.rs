use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use crate::{Skill, SkillStore, SkillManager};

#[derive(Serialize, Deserialize)]
pub struct AgentSkillIoFormat {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

pub struct AgentSkillsHub;

impl AgentSkillsHub {
    pub fn export_skills(store: &SkillStore, output_path: &Path) -> Result<()> {
        let skills = store.get_all_skills()?;
        let mut exported = Vec::new();
        
        for skill in skills {
            exported.push(AgentSkillIoFormat {
                name: skill.name,
                description: skill.description,
                instructions: skill.instructions,
            });
        }
        
        let json = serde_json::to_string_pretty(&exported)?;
        fs::write(output_path, json).context("Failed to write exported skills")?;
        
        Ok(())
    }

    pub fn import_skills(store: &SkillStore, manager: &SkillManager, input_path: &Path) -> Result<usize> {
        let content = fs::read_to_string(input_path).context("Failed to read import file")?;
        let imported: Vec<AgentSkillIoFormat> = serde_json::from_str(&content).context("Failed to parse agentskills.io JSON")?;
        
        let mut count = 0;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
            
        for item in imported {
            let id = uuid::Uuid::new_v4().to_string();
            let embedding = manager.embed_text(&item.description)?;
            
            let skill = Skill {
                id,
                name: item.name,
                description: item.description,
                instructions: item.instructions,
                usage_count: 0,
                success_count: 0,
                created_at: now,
                updated_at: now,
            };
            
            store.insert_skill(&skill, &embedding)?;
            count += 1;
        }
        
        Ok(count)
    }
}

// Rust guideline compliant 2026-02-21
