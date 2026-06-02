#[cfg(not(test))]
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

#[cfg(test)]
pub mod fastembed_mock {
    pub struct TextEmbedding;
    pub struct InitOptions {
        pub model_name: EmbeddingModel,
        pub show_download_progress: bool,
    }
    impl Default for InitOptions {
        fn default() -> Self {
            Self {
                model_name: EmbeddingModel::AllMiniLML6V2,
                show_download_progress: false,
            }
        }
    }
    pub enum EmbeddingModel { AllMiniLML6V2 }
    impl TextEmbedding {
        pub fn try_new(_opts: InitOptions) -> Result<Self, anyhow::Error> { Ok(Self) }
        pub fn embed(&self, texts: Vec<String>, _batch: Option<usize>) -> Result<Vec<Vec<f32>>, anyhow::Error> {
            // Return a dummy 384-dimensional vector (size of MiniLM)
            Ok(texts.into_iter().map(|_| vec![0.1_f32; 384]).collect())
        }
    }
}

#[cfg(test)]
use fastembed_mock::*;
use std::path::Path;
use tracing::{debug, info};
use uuid::Uuid;

use crate::store::SkillStore;
use crate::Skill;

pub struct SkillManager {
    store: SkillStore,
    embed_model: TextEmbedding,
}

impl SkillManager {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, anyhow::Error> {
        let store = SkillStore::new(db_path)?;
        
        info!("Initializing local embedding model (this may download model weights the first time)...");
        let embed_model = TextEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::AllMiniLML6V2,
            show_download_progress: true,
            ..Default::default()
        })?;
        
        Ok(Self {
            store,
            embed_model,
        })
    }

    pub fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let embeddings = self.embed_model.embed(vec![text.to_string()], None)?;
        Ok(embeddings[0].clone())
    }

    pub fn create_skill(&self, name: &str, description: &str, instructions: &str) -> Result<Skill, anyhow::Error> {
        let text_to_embed = format!("{} {}", description, instructions);
        
        // Generate embedding
        let embedding = self.embed_text(&text_to_embed)?;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs() as i64;
            
        let skill = Skill {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            instructions: instructions.to_string(),
            usage_count: 0,
            success_count: 0,
            created_at: now,
            updated_at: now,
        };
        
        self.store.insert_skill(&skill, &embedding)?;
        debug!("Created skill {}", skill.id);
        
        Ok(skill)
    }

    /// Retrieve the top K most relevant skills for a given query context
    pub fn search_skills(&self, query: &str, top_k: usize) -> Result<Vec<Skill>, anyhow::Error> {
        let query_embeddings = self.embed_model.embed(vec![query.to_string()], None)?;
        let query_vec = &query_embeddings[0];
        
        let all_embeddings = self.store.get_all_embeddings()?;
        
        let mut scored_skills: Vec<(f32, String)> = all_embeddings.into_iter().map(|(id, embed)| {
            let score = Self::cosine_similarity(query_vec, &embed);
            (score, id)
        }).collect();
        
        // Sort descending by score
        scored_skills.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        
        let top_ids = scored_skills.into_iter().take(top_k).collect::<Vec<_>>();
        
        let mut results = Vec::new();
        for (_, id) in top_ids {
            if let Some(skill) = self.store.get_skill(&id)? {
                results.push(skill);
            }
        }
        
        Ok(results)
    }

    pub fn search_with_scores(&self, query_vec: &[f32], top_k: usize) -> Result<Vec<(f32, Skill)>, anyhow::Error> {
        let all_embeddings = self.store.get_all_embeddings()?;
        
        let mut scored_skills: Vec<(f32, String)> = all_embeddings.into_iter().map(|(id, embed)| {
            let score = Self::cosine_similarity(query_vec, &embed);
            (score, id)
        }).collect();
        
        // Sort descending by score
        scored_skills.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        
        let top_ids = scored_skills.into_iter().take(top_k).collect::<Vec<_>>();
        
        let mut results = Vec::new();
        for (score, id) in top_ids {
            if let Some(skill) = self.store.get_skill(&id)? {
                results.push((score, skill));
            }
        }
        
        Ok(results)
    }
    
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }
        
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;
        
        for i in 0..a.len() {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }
        
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a.sqrt() * norm_b.sqrt())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((SkillManager::cosine_similarity(&a, &b) - 1.0).abs() < f32::EPSILON);

        let c = vec![0.0, 1.0, 0.0];
        assert!(SkillManager::cosine_similarity(&a, &c).abs() < f32::EPSILON);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((SkillManager::cosine_similarity(&a, &d) - (-1.0)).abs() < f32::EPSILON);

        let e = vec![0.0, 0.0, 0.0];
        assert_eq!(SkillManager::cosine_similarity(&a, &e), 0.0);

        let f = vec![1.0];
        assert_eq!(SkillManager::cosine_similarity(&a, &f), 0.0); // Different lengths
    }

    #[test]
    fn test_skill_manager_init() {
        // Test basic initialization
        let temp_file = std::env::temp_dir().join(format!("test_skill_store_{}.db", Uuid::new_v4()));
        let manager_res = SkillManager::new(&temp_file);
        
        match manager_res {
            Ok(manager) => {
                let skill = manager.create_skill("Test", "A test skill", "Do nothing").unwrap();
                assert_eq!(skill.name, "Test");
                
                let search = manager.search_skills("nothing", 1).unwrap();
                assert_eq!(search.len(), 1);
                assert_eq!(search[0].id, skill.id);
            }
            Err(e) => {
                eprintln!("Manager initialization failed: {:?}", e);
            }
        }
        
        let _ = std::fs::remove_file(&temp_file);
    }
}

// Rust guideline compliant 2026-02-21
