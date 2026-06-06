use athena_agent::{logger::SessionLogger, Message};
use athena_state::db::{MessageRow, Session, SessionDB};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DbSessionLogger {
    pub db: Arc<SessionDB>,
    pub model: String,
}

impl SessionLogger for DbSessionLogger {
    fn log_session(&self, messages: &[Message], system_message: Option<&str>) {
        let session_id = uuid::Uuid::new_v4().to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let session = Session {
            id: session_id.clone(),
            title: Some("Autonomous Interaction".to_string()),
            model: Some(self.model.clone()),
            system_prompt: system_message.map(|s| s.to_string()),
            started_at: timestamp,
        };

        if let Err(e) = self.db.insert_session(&session) {
            tracing::warn!("Failed to insert session: {}", e);
            return;
        }

        for msg in messages {
            let (role, content, tool_calls) = match msg {
                Message::System { content } => ("system", Some(content.clone()), None),
                Message::User { content, .. } => ("user", Some(content.clone()), None),
                Message::Assistant { content, tool_calls, .. } => {
                    let tc_str = tool_calls
                        .as_ref()
                        .map(|tc| serde_json::to_string(tc).unwrap_or_default());
                    ("assistant", content.clone(), tc_str)
                }
                Message::Tool { content, .. } => ("tool", Some(content.clone()), None),
            };

            let msg_row = MessageRow {
                id: 0,
                session_id: session_id.clone(),
                role: role.to_string(),
                content,
                tool_calls,
                timestamp,
            };
            let _ = self.db.insert_message(&msg_row);
        }
    }
}
