use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
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

// Rust guideline compliant 2026-02-21
