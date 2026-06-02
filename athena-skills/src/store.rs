use rusqlite::{Connection, Result as SqliteResult};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::Skill;

pub struct SkillStore {
    conn: Arc<Mutex<Connection>>,
}

impl SkillStore {
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        
        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                instructions TEXT NOT NULL,
                embedding BLOB,
                usage_count INTEGER DEFAULT 0,
                success_count INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;
        
        let _ = conn.execute("ALTER TABLE skills ADD COLUMN usage_count INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE skills ADD COLUMN success_count INTEGER DEFAULT 0", []);
        
        info!("Initialized SkillStore DB");

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_skill(&self, skill: &Skill, embedding: &[f32]) -> SqliteResult<()> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Err(rusqlite::Error::InvalidQuery),
        };
        
        let embedding_bytes: Vec<u8> = embedding
            .iter()
            .flat_map(|&f| f.to_ne_bytes().to_vec())
            .collect();
            
        conn.execute(
            "INSERT OR REPLACE INTO skills (id, name, description, instructions, usage_count, success_count, embedding, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                &skill.id,
                &skill.name,
                &skill.description,
                &skill.instructions,
                skill.usage_count,
                skill.success_count,
                &embedding_bytes,
                skill.created_at,
                skill.updated_at,
            ),
        )?;
        
        Ok(())
    }

    pub fn get_skill(&self, id: &str) -> SqliteResult<Option<Skill>> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Err(rusqlite::Error::InvalidQuery),
        };
        let mut stmt = conn.prepare(
            "SELECT id, name, description, instructions, usage_count, success_count, created_at, updated_at FROM skills WHERE id = ?1"
        )?;
        
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Skill {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                instructions: row.get(3)?,
                usage_count: row.get(4).unwrap_or(0),
                success_count: row.get(5).unwrap_or(0),
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_skills(&self) -> SqliteResult<Vec<Skill>> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Err(rusqlite::Error::InvalidQuery),
        };
        let mut stmt = conn.prepare(
            "SELECT id, name, description, instructions, usage_count, success_count, created_at, updated_at FROM skills"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok(Skill {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                instructions: row.get(3)?,
                usage_count: row.get(4).unwrap_or(0),
                success_count: row.get(5).unwrap_or(0),
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn record_use(&self, id: &str) -> SqliteResult<()> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Err(rusqlite::Error::InvalidQuery),
        };
        conn.execute("UPDATE skills SET usage_count = usage_count + 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn record_success(&self, id: &str) -> SqliteResult<()> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Err(rusqlite::Error::InvalidQuery),
        };
        conn.execute("UPDATE skills SET success_count = success_count + 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn get_all_embeddings(&self) -> SqliteResult<Vec<(String, Vec<f32>)>> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Err(rusqlite::Error::InvalidQuery),
        };
        let mut stmt = conn.prepare("SELECT id, embedding FROM skills WHERE embedding IS NOT NULL")?;
        
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            
            // Convert bytes back to f32
            let mut embedding = Vec::with_capacity(bytes.len() / 4);
            for chunk in bytes.chunks_exact(4) {
                let bytes_array: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                embedding.push(f32::from_ne_bytes(bytes_array));
            }
            
            Ok((id, embedding))
        })?;
        
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_skill(id: &str) -> Skill {
        Skill {
            id: id.to_string(),
            name: format!("Skill {}", id),
            description: format!("Desc {}", id),
            instructions: format!("Inst {}", id),
            usage_count: 0,
            success_count: 0,
            created_at: 1000,
            updated_at: 1000,
        }
    }

    #[test]
    fn test_store_init_and_get() {
        let store = SkillStore::new(":memory:").unwrap();
        
        let skill = create_test_skill("1");
        let embedding = vec![0.1f32, 0.2, 0.3];
        
        store.insert_skill(&skill, &embedding).unwrap();
        
        let retrieved = store.get_skill("1").unwrap().unwrap();
        assert_eq!(retrieved.id, "1");
        assert_eq!(retrieved.name, "Skill 1");
        assert_eq!(retrieved.description, "Desc 1");
        assert_eq!(retrieved.instructions, "Inst 1");
        assert_eq!(retrieved.created_at, 1000);
        
        let missing = store.get_skill("2").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_store_get_all_embeddings() {
        let store = SkillStore::new(":memory:").unwrap();
        
        let skill1 = create_test_skill("1");
        let embed1 = vec![0.1f32, 0.2];
        store.insert_skill(&skill1, &embed1).unwrap();
        
        let skill2 = create_test_skill("2");
        let embed2 = vec![0.3f32, 0.4];
        store.insert_skill(&skill2, &embed2).unwrap();
        
        let mut embeddings = store.get_all_embeddings().unwrap();
        embeddings.sort_by(|a, b| a.0.cmp(&b.0));
        
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].0, "1");
        assert_eq!(embeddings[0].1, embed1);
        assert_eq!(embeddings[1].0, "2");
        assert_eq!(embeddings[1].1, embed2);
    }
}

// Rust guideline compliant 2026-02-21
