use async_trait::async_trait;
use serde_json::Value;
use crate::registry::Tool;
use athena_state::db::SessionDB;
use std::fs;
use athena_core::paths::get_athena_home;

#[derive(Clone)]
pub struct TrajectoryExportTool;

#[async_trait]
impl Tool for TrajectoryExportTool {
    fn name(&self) -> &'static str {
        "export_session_trajectory"
    }

    fn toolset(&self) -> &'static str {
        "memory"
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "description": "Export a past session trajectory to a JSON file for training or analysis.",
            "parameters": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session ID to export"
                    }
                },
                "required": ["session_id"]
            }
        })
    }

    async fn handle(&self, args: Value) -> Result<Value, String> {
        let session_id = args.get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;

        let db = SessionDB::new(None).map_err(|e| format!("DB Error: {}", e))?;

        let (session, messages) = db.get_session_trajectory(session_id).map_err(|e| format!("Failed to get trajectory: {}", e))?;

        let mut compressed_msgs = Vec::new();
        for msg in messages {
            // Basic compression: truncate large tool outputs to 1000 chars
            let compressed_content = if let Some(ref c) = msg.content {
                if msg.role == "tool" && c.len() > 1000 {
                    Some(format!("{}... [TRUNCATED]", &c[..1000]))
                } else {
                    Some(c.clone())
                }
            } else {
                None
            };

            compressed_msgs.push(serde_json::json!({
                "role": msg.role,
                "content": compressed_content,
                "tool_calls": msg.tool_calls.as_ref().map(|tc| serde_json::from_str::<Value>(tc).unwrap_or_else(|_| serde_json::json!(tc))),
                "timestamp": msg.timestamp,
            }));
        }

        let trajectory = serde_json::json!({
            "session": {
                "id": session.id,
                "title": session.title,
                "model": session.model,
                "started_at": session.started_at,
            },
            "messages": compressed_msgs,
        });

        let export_dir = get_athena_home().join("trajectories");
        fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
        
        let file_path = export_dir.join(format!("{}.json", session_id));
        let json_str = serde_json::to_string_pretty(&trajectory).map_err(|e| e.to_string())?;
        
        fs::write(&file_path, json_str).map_err(|e| e.to_string())?;

        Ok(serde_json::json!(format!("Successfully exported compressed trajectory to {}", file_path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use athena_state::db::{Session, MessageRow};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_trajectory_export() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("ATHENA_HOME", temp_dir.path());
        
        let db = SessionDB::new(None).unwrap();
        
        let session = Session {
            id: "traj-session-1".to_string(),
            title: Some("My Trajectory".to_string()),
            model: Some("gpt-4o".to_string()),
            system_prompt: None,
            started_at: 1000.0,
        };
        db.insert_session(&session).unwrap();
        
        // Short message
        let msg1 = MessageRow {
            id: 1,
            session_id: "traj-session-1".to_string(),
            role: "user".to_string(),
            content: Some("I want to build a rust application".to_string()),
            tool_calls: None,
            timestamp: 1001.0,
        };
        db.insert_message(&msg1).unwrap();

        // Long tool message to test truncation
        let long_content = "A".repeat(1500);
        let msg2 = MessageRow {
            id: 2,
            session_id: "traj-session-1".to_string(),
            role: "tool".to_string(),
            content: Some(long_content.clone()),
            tool_calls: None,
            timestamp: 1002.0,
        };
        db.insert_message(&msg2).unwrap();

        let tool = TrajectoryExportTool;
        let args = serde_json::json!({
            "session_id": "traj-session-1"
        });

        let result = tool.handle(args).await;
        assert!(result.is_ok());

        let export_path = temp_dir.path().join("trajectories").join("traj-session-1.json");
        assert!(export_path.exists());

        let content = std::fs::read_to_string(export_path).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["session"]["id"], "traj-session-1");
        assert_eq!(parsed["messages"].as_array().unwrap().len(), 2);
        
        let msg2_exported = &parsed["messages"][1]["content"].as_str().unwrap();
        assert!(msg2_exported.ends_with("[TRUNCATED]"));
        assert_eq!(msg2_exported.len(), 1000 + 15); // 1000 chars + "... [TRUNCATED]"
    }
}

// Rust guideline compliant 2026-02-21
