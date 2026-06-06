use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;
use athena_core::get_athena_home;

pub const DEFAULT_DB_NAME: &str = "state.db";

pub const SCHEMA_VERSION: i32 = 11;

pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    user_id TEXT,
    model TEXT,
    model_config TEXT,
    system_prompt TEXT,
    parent_session_id TEXT,
    started_at REAL NOT NULL,
    ended_at REAL,
    end_reason TEXT,
    message_count INTEGER DEFAULT 0,
    tool_call_count INTEGER DEFAULT 0,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0,
    cache_write_tokens INTEGER DEFAULT 0,
    reasoning_tokens INTEGER DEFAULT 0,
    billing_provider TEXT,
    billing_base_url TEXT,
    billing_mode TEXT,
    estimated_cost_usd REAL,
    actual_cost_usd REAL,
    cost_status TEXT,
    cost_source TEXT,
    pricing_version TEXT,
    title TEXT,
    api_call_count INTEGER DEFAULT 0,
    handoff_state TEXT,
    handoff_platform TEXT,
    handoff_error TEXT,
    FOREIGN KEY (parent_session_id) REFERENCES sessions(id)
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,
    content TEXT,
    tool_call_id TEXT,
    tool_calls TEXT,
    tool_name TEXT,
    timestamp REAL NOT NULL,
    token_count INTEGER,
    finish_reason TEXT,
    reasoning TEXT,
    reasoning_content TEXT,
    reasoning_details TEXT,
    codex_reasoning_items TEXT,
    codex_message_items TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
    id UNINDEXED,
    title,
    content
);

-- Trigger to insert FTS index when a new message is added
CREATE TRIGGER IF NOT EXISTS msg_insert_fts AFTER INSERT ON messages
BEGIN
    INSERT INTO sessions_fts(id, title, content) 
    SELECT 
        new.session_id,
        (SELECT title FROM sessions WHERE id = new.session_id),
        new.content
    WHERE NOT EXISTS (SELECT 1 FROM sessions_fts WHERE id = new.session_id);
    
    UPDATE sessions_fts 
    SET content = content || ' ' || new.content
    WHERE id = new.session_id AND EXISTS (SELECT 1 FROM sessions_fts WHERE id = new.session_id);
END;

CREATE TABLE IF NOT EXISTS state_meta (
    key TEXT PRIMARY KEY,
    value TEXT
);
"#;

pub struct SessionDB {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub title: Option<String>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub started_at: f64,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<String>,
    pub timestamp: f64,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: Option<String>,
    pub snippet: String,
}

impl SessionDB {
    pub fn new(db_path: Option<PathBuf>) -> Result<Self> {
        let path = db_path.unwrap_or_else(|| get_athena_home().join(DEFAULT_DB_NAME));

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_default();
        }

        let conn = Connection::open(&path)?;

        // Setup WAL mode
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");

        let db = Self {
            conn: Mutex::new(conn),
        };

        db.init_schema()?;

        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Err(rusqlite::Error::InvalidQuery),
        };
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(())
    }

    pub fn insert_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, source, started_at, title, model, system_prompt) VALUES (?1, 'cli', ?2, ?3, ?4, ?5)",
            (
                &session.id,
                session.started_at,
                session.title.as_ref(),
                session.model.as_ref(),
                session.system_prompt.as_ref(),
            ),
        )?;
        Ok(())
    }

    pub fn insert_message(&self, msg: &MessageRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (session_id, role, content, tool_calls, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                &msg.session_id,
                &msg.role,
                msg.content.as_ref(),
                msg.tool_calls.as_ref(),
                msg.timestamp,
            ),
        )?;
        Ok(())
    }

    pub fn get_session_trajectory(&self, session_id: &str) -> Result<(Session, Vec<MessageRow>)> {
        let conn = self.conn.lock().unwrap();
        
        let mut session_stmt = conn.prepare("SELECT id, title, model, system_prompt, started_at FROM sessions WHERE id = ?1")?;
        let session = session_stmt.query_row([session_id], |row| {
            Ok(Session {
                id: row.get(0)?,
                title: row.get(1)?,
                model: row.get(2)?,
                system_prompt: row.get(3)?,
                started_at: row.get(4)?,
            })
        })?;

        let mut msg_stmt = conn.prepare("SELECT id, session_id, role, content, tool_calls, timestamp FROM messages WHERE session_id = ?1 ORDER BY id ASC")?;
        let msg_iter = msg_stmt.query_map([session_id], |row| {
            Ok(MessageRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_calls: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut messages = Vec::new();
        for msg in msg_iter {
            messages.push(msg?);
        }

        Ok((session, messages))
    }

    pub fn search_sessions(&self, query: &str) -> Result<Vec<SessionSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, snippet(sessions_fts, -1, '<b>', '</b>', '...', 64) 
             FROM sessions_fts 
             WHERE sessions_fts MATCH ?1 
             ORDER BY rank LIMIT 10"
        )?;
        
        let fts_query = format!("\"{}\"", query); // basic quoting for FTS5
        let iter = stmt.query_map([fts_query], |row| {
            Ok(SessionSummary {
                session_id: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
            })
        })?;

        let mut results = Vec::new();
        for res in iter {
            results.push(res?);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_db_initialization() {
        // Create an in-memory database to test the schema creation without hitting disk
        let db = SessionDB::new(Some(PathBuf::from(":memory:"))).unwrap();

        let conn = db.conn.lock().unwrap();
        // Verify a table exists to confirm schema init succeeded
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'").unwrap();
        let exists = stmt.exists([]).unwrap();
        assert!(exists);
    }

    #[test]
    fn test_session_db_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("db.sqlite");

        // This should create the 'nested' directory
        let db = SessionDB::new(Some(nested_path.clone()));
        assert!(db.is_ok());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_session_db_default_path() {
        // We ensure it falls back gracefully when None is passed
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("ATHENA_HOME", temp_dir.path());

        let db = SessionDB::new(None);
        assert!(db.is_ok());
        assert!(temp_dir.path().join(DEFAULT_DB_NAME).exists());
    }

    #[test]
    fn test_fts5_search() {
        let db = SessionDB::new(Some(PathBuf::from(":memory:"))).unwrap();
        
        let session = Session {
            id: "sess-1".to_string(),
            title: Some("My cool session".to_string()),
            model: Some("gpt-4o".to_string()),
            system_prompt: None,
            started_at: 1000.0,
        };
        db.insert_session(&session).unwrap();
        
        let msg = MessageRow {
            id: 1,
            session_id: "sess-1".to_string(),
            role: "user".to_string(),
            content: Some("I want to build a rust application".to_string()),
            tool_calls: None,
            timestamp: 1001.0,
        };
        db.insert_message(&msg).unwrap();

        // Search for "rust"
        let results = db.search_sessions("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "sess-1");
        assert!(results[0].snippet.contains("<b>rust</b>") || results[0].snippet.contains("rust"));
        
        // Search for something not there
        let results_empty = db.search_sessions("python").unwrap();
        assert_eq!(results_empty.len(), 0);
    }
}

// Rust guideline compliant 2026-02-21
