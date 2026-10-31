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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agentskills_export_and_import() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path1 = temp_dir.path().join("skills1.db");
        let db_path2 = temp_dir.path().join("skills2.db");
        let export_path = temp_dir.path().join("exported.json");

        let store1 = SkillStore::new(&db_path1).unwrap();
        let manager1 = SkillManager::new(&db_path1).unwrap();

        let skill = Skill {
            id: "s-1".to_string(),
            name: "Rust Testing Skill".to_string(),
            description: "Teaches how to write rust unit tests".to_string(),
            instructions: "Use #[test] and assert!".to_string(),
            usage_count: 3,
            success_count: 3,
            created_at: 100,
            updated_at: 100,
        };
        let embedding = manager1.embed_text(&skill.description).unwrap();
        store1.insert_skill(&skill, &embedding).unwrap();

        // Test export
        AgentSkillsHub::export_skills(&store1, &export_path).unwrap();
        assert!(export_path.exists());

        let content = fs::read_to_string(&export_path).unwrap();
        assert!(content.contains("Rust Testing Skill"));

        // Test import
        let store2 = SkillStore::new(&db_path2).unwrap();
        let manager2 = SkillManager::new(&db_path2).unwrap();
        let imported_count = AgentSkillsHub::import_skills(&store2, &manager2, &export_path).unwrap();
        assert_eq!(imported_count, 1);

        let skills2 = store2.get_all_skills().unwrap();
        assert_eq!(skills2.len(), 1);
        assert_eq!(skills2[0].name, "Rust Testing Skill");
        assert_eq!(skills2[0].instructions, "Use #[test] and assert!");
    }
}

// Rust guideline compliant 2026-02-21
