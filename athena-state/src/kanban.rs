use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use athena_core::paths::get_athena_home;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanBoard {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanColumn {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub order_idx: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanCard {
    pub id: String,
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub assignee: Option<String>,
}

pub struct KanbanDB {
    conn: Connection,
}

impl KanbanDB {
    pub fn new() -> Result<Self> {
        let db_path = get_athena_home().join("kanban.db");
        Self::open(db_path)
    }

    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init_tables(&conn)?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_tables(&conn)?;
        Ok(Self { conn })
    }

    fn init_tables(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kanban_boards (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kanban_columns (
                id TEXT PRIMARY KEY,
                board_id TEXT NOT NULL,
                name TEXT NOT NULL,
                order_idx INTEGER NOT NULL,
                FOREIGN KEY (board_id) REFERENCES kanban_boards (id) ON DELETE CASCADE
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kanban_cards (
                id TEXT PRIMARY KEY,
                board_id TEXT NOT NULL,
                column_id TEXT NOT NULL,
                title TEXT NOT NULL,
                assignee TEXT,
                FOREIGN KEY (board_id) REFERENCES kanban_boards (id) ON DELETE CASCADE,
                FOREIGN KEY (column_id) REFERENCES kanban_columns (id) ON DELETE CASCADE
            )",
            [],
        )?;
        
        Ok(())
    }

    pub fn init_default_board(&self) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT count(*) FROM kanban_boards")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        
        if count == 0 {
            let board_id = "default".to_string();
            self.create_board(&board_id, "Main Board")?;
            self.create_column("col-todo", &board_id, "Todo", 0)?;
            self.create_column("col-in-progress", &board_id, "In Progress", 1)?;
            self.create_column("col-done", &board_id, "Done", 2)?;
        }
        
        Ok(())
    }

    pub fn create_board(&self, id: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO kanban_boards (id, name) VALUES (?1, ?2)",
            params![id, name],
        )?;
        Ok(())
    }

    pub fn create_column(&self, id: &str, board_id: &str, name: &str, order_idx: i32) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO kanban_columns (id, board_id, name, order_idx) VALUES (?1, ?2, ?3, ?4)",
            params![id, board_id, name, order_idx],
        )?;
        Ok(())
    }

    pub fn create_card(&self, id: &str, board_id: &str, column_id: &str, title: &str, assignee: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kanban_cards (id, board_id, column_id, title, assignee) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, board_id, column_id, title, assignee],
        )?;
        Ok(())
    }

    pub fn get_cards(&self, board_id: &str) -> Result<Vec<KanbanCard>> {
        let mut stmt = self.conn.prepare("SELECT id, board_id, column_id, title, assignee FROM kanban_cards WHERE board_id = ?1")?;
        let rows = stmt.query_map(params![board_id], |row| {
            Ok(KanbanCard {
                id: row.get(0)?,
                board_id: row.get(1)?,
                column_id: row.get(2)?,
                title: row.get(3)?,
                assignee: row.get(4)?,
            })
        })?;
        
        let mut cards = Vec::new();
        for r in rows {
            cards.push(r?);
        }
        Ok(cards)
    }

    pub fn move_card(&self, card_id: &str, new_column_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE kanban_cards SET column_id = ?1 WHERE id = ?2",
            params![new_column_id, card_id],
        )?;
        Ok(())
    }

    pub fn delete_card(&self, card_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM kanban_cards WHERE id = ?1", params![card_id])?;
        Ok(())
    }

    pub fn assign_card(&self, card_id: &str, assignee: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE kanban_cards SET assignee = ?1 WHERE id = ?2",
            params![assignee, card_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kanban_in_memory_crud() {
        let db = KanbanDB::open_in_memory().expect("in-memory db failed");
        db.init_default_board().expect("failed init default board");

        // Should have default board and 3 columns
        let cards = db.get_cards("default").expect("failed to get cards");
        assert_eq!(cards.len(), 0);

        // Create card
        db.create_card("c1", "default", "col-todo", "Write Rust tests", Some("kevin"))
            .expect("failed create card");

        let cards = db.get_cards("default").expect("failed get cards");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, "c1");
        assert_eq!(cards[0].column_id, "col-todo");
        assert_eq!(cards[0].title, "Write Rust tests");
        assert_eq!(cards[0].assignee.as_deref(), Some("kevin"));

        // Move card
        db.move_card("c1", "col-in-progress").expect("failed to move card");
        let cards = db.get_cards("default").unwrap();
        assert_eq!(cards[0].column_id, "col-in-progress");

        // Assign card
        db.assign_card("c1", "athena-agent").expect("failed to assign card");
        let cards = db.get_cards("default").unwrap();
        assert_eq!(cards[0].assignee.as_deref(), Some("athena-agent"));

        // Delete card
        db.delete_card("c1").expect("failed to delete card");
        let cards = db.get_cards("default").unwrap();
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn test_kanban_custom_board_and_column() {
        let db = KanbanDB::open_in_memory().expect("in-memory db failed");
        db.create_board("b-sprint", "Sprint 42").unwrap();
        db.create_column("col-backlog", "b-sprint", "Backlog", 0).unwrap();
        db.create_card("task-1", "b-sprint", "col-backlog", "Sprint planning", None).unwrap();

        let cards = db.get_cards("b-sprint").unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Sprint planning");
        assert_eq!(cards[0].assignee, None);
    }
}

// Rust guideline compliant 2026-02-21
