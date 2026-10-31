use async_trait::async_trait;
use serde_json::Value;
use crate::registry::Tool;
use athena_state::db::SessionDB;

#[derive(Clone)]
pub struct SearchSessionsTool;

#[async_trait]
impl Tool for SearchSessionsTool {
    fn name(&self) -> &'static str {
        "search_past_conversations"
    }

    fn toolset(&self) -> &'static str {
        "memory"
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "description": "Search through the agent's past conversations/sessions using full-text search. Useful when trying to recall something from a previous interaction.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query (e.g. 'how to configure nginx', 'python script for scraping')"
                    }
                },
                "required": ["query"]
            }
        })
    }

    async fn handle(&self, args: Value) -> Result<Value, String> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'query' parameter".to_string())?;

        let db = SessionDB::new(None).map_err(|e| format!("DB Error: {}", e))?;

        match db.search_sessions(query) {
            Ok(results) => {
                if results.is_empty() {
                    return Ok(serde_json::json!("No past sessions found matching the query."));
                }
                let mut output = String::from("Found matching past sessions:\n\n");
                for res in results {
                    let title = res.title.unwrap_or_else(|| "Untitled Session".into());
                    output.push_str(&format!("Session ID: {}\nTitle: {}\nSnippet: {}\n\n", res.session_id, title, res.snippet));
                }
                Ok(serde_json::json!(output))
            }
            Err(e) => Err(format!("Database search failed: {}", e)),
        }
    }
}

inventory::submit! {
    crate::registry::RegisteredTool {
        factory: || std::sync::Arc::new(SearchSessionsTool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_sessions_tool() {
        let guard = athena_core::test_utils::EnvGuard::new();
        let temp_dir = tempfile::tempdir().unwrap();
        guard.set_home(temp_dir.path());

        let tool = SearchSessionsTool;
        assert_eq!(tool.name(), "search_past_conversations");
        assert_eq!(tool.toolset(), "memory");

        // Missing query param error handling
        let err = tool.handle(serde_json::json!({})).await.unwrap_err();
        assert!(err.contains("Missing 'query'"));

        // Normal query when empty
        let res = tool.handle(serde_json::json!({"query": "rust"})).await.unwrap();
        assert_eq!(res, serde_json::json!("No past sessions found matching the query."));

        // Save a mock session in DB and test match
        let db = SessionDB::new(None).unwrap();
        db.insert_session(&athena_state::Session {
            id: "s1".to_string(),
            title: Some("Rust Async Guide".to_string()),
            model: Some("gpt-4o".to_string()),
            system_prompt: None,
            started_at: 1000.0,
        }).unwrap();
        db.insert_message(&athena_state::MessageRow {
            id: 1,
            session_id: "s1".to_string(),
            role: "user".to_string(),
            content: Some("How do I optimize rust async code?".to_string()),
            tool_calls: None,
            timestamp: 1001.0,
        }).unwrap();

        let res = tool.handle(serde_json::json!({"query": "optimize"})).await.unwrap();
        let res_str = res.as_str().unwrap();
        assert!(res_str.contains("Rust Async Guide"));
        assert!(res_str.contains("s1"));
    }
}

// Rust guideline compliant 2026-02-21
