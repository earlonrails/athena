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

// Rust guideline compliant 2026-02-21
