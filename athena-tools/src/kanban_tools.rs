use crate::registry::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use athena_state::kanban::KanbanDB;

pub struct KanbanCreateTaskTool;
pub struct KanbanMoveTaskTool;

#[async_trait]
impl Tool for KanbanCreateTaskTool {
    fn name(&self) -> &'static str { "kanban_create" }
    fn toolset(&self) -> &'static str { "kanban" }
    fn schema(&self) -> Value {
        json!({
            "description": "Create a new task on the Kanban board.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "assignee": { "type": "string" }
                },
                "required": ["title"]
            }
        })
    }
    async fn handle(&self, args: Value) -> Result<Value, String> {
        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or_default();
        let assignee = args.get("assignee").and_then(|v| v.as_str());

        if let Ok(db) = KanbanDB::new() {
            let _ = db.init_default_board();
            let id = uuid::Uuid::new_v4().to_string();
            match db.create_card(&id, "default", "col-todo", title, assignee) {
                Ok(_) => Ok(json!({ "status": "success", "task_id": id })),
                Err(e) => Ok(json!({ "error": format!("Failed: {}", e) })),
            }
        } else {
            Ok(json!({ "error": "Database error" }))
        }
    }
}

#[async_trait]
impl Tool for KanbanMoveTaskTool {
    fn name(&self) -> &'static str { "kanban_move" }
    fn toolset(&self) -> &'static str { "kanban" }
    fn schema(&self) -> Value {
        json!({
            "description": "Move a task to a different status column.",
            "parameters": {
                "type": "object",
                "properties": {
                    "task_id": { "type": "string" },
                    "column": { "type": "string", "enum": ["col-todo", "col-in-progress", "col-done"] }
                },
                "required": ["task_id", "column"]
            }
        })
    }
    async fn handle(&self, args: Value) -> Result<Value, String> {
        let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or_default();
        let column = args.get("column").and_then(|v| v.as_str()).unwrap_or_default();

        if let Ok(db) = KanbanDB::new() {
            match db.move_card(task_id, column) {
                Ok(_) => Ok(json!({ "status": "success" })),
                Err(e) => Ok(json!({ "error": format!("Failed: {}", e) })),
            }
        } else {
            Ok(json!({ "error": "Database error" }))
        }
    }
}

inventory::submit!(crate::registry::RegisteredTool { factory: || std::sync::Arc::new(KanbanCreateTaskTool) });
inventory::submit!(crate::registry::RegisteredTool { factory: || std::sync::Arc::new(KanbanMoveTaskTool) });

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kanban_create_and_move_tools() {
        let guard = athena_core::test_utils::EnvGuard::new();
        let temp_dir = tempfile::tempdir().unwrap();
        guard.set_home(temp_dir.path());

        let create_tool = KanbanCreateTaskTool;
        assert_eq!(create_tool.name(), "kanban_create");
        assert_eq!(create_tool.toolset(), "kanban");
        assert!(create_tool.schema().get("parameters").is_some());

        let res = create_tool.handle(json!({
            "title": "Build Autonomous Tests",
            "assignee": "athena"
        })).await.unwrap();

        assert_eq!(res["status"], "success");
        let task_id = res["task_id"].as_str().expect("task_id returned");

        let move_tool = KanbanMoveTaskTool;
        assert_eq!(move_tool.name(), "kanban_move");
        assert_eq!(move_tool.toolset(), "kanban");

        let move_res = move_tool.handle(json!({
            "task_id": task_id,
            "column": "col-in-progress"
        })).await.unwrap();

        assert_eq!(move_res["status"], "success");
    }
}

// Rust guideline compliant 2026-02-21
