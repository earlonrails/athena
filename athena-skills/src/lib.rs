pub mod store;
pub mod manager;
pub mod synthesis;
pub mod improvement;

pub use store::*;
pub use manager::*;
pub use synthesis::*;
pub use improvement::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    #[serde(default)]
    pub usage_count: i32,
    #[serde(default)]
    pub success_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

// Rust guideline compliant 2026-02-21
