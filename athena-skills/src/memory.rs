use anyhow::{Context, Result};
use athena_providers::{ChatCompletionRequest, ChatMessage, LLMProvider, MessageRole};
use std::sync::Arc;
use tracing::info;
use std::fs::OpenOptions;
use std::io::Write;
use athena_core::paths::get_athena_home;

pub struct MemoryNudge;

impl MemoryNudge {
    pub async fn run(history: &[ChatMessage], provider: Arc<dyn LLMProvider>, model_name: &str) -> Result<()> {
        let max_turns = 10;
        let recent_history: Vec<_> = history.iter().rev().take(max_turns).rev().collect();
        
        let mut prompt_content = String::from(
            "You are an AI tasked with maintaining a long-term memory file for future interactions.\n\
            Review the following recent conversation history.\n\
            Identify up to 3 important facts, user preferences, or task contexts that are worth remembering long-term.\n\
            If there are no new facts worth remembering, respond with the exact string \"NO_FACTS\".\n\
            Otherwise, return a bulleted list of facts (starting with '- '). Do not include any conversational filler.\n\n\
            CONVERSATION HISTORY:\n"
        );

        for msg in recent_history {
            match msg.role {
                MessageRole::User => {
                    prompt_content.push_str(&format!("USER: {}\n", msg.content));
                }
                MessageRole::Assistant => {
                    if !msg.content.is_empty() {
                        prompt_content.push_str(&format!("ASSISTANT: {}\n", msg.content));
                    }
                }
                _ => {}
            }
        }

        let req = ChatCompletionRequest {
            model: model_name.to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: prompt_content,
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: Some(0.1),
            max_tokens: Some(300),
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            tool_choice: None,
            extra_body: Default::default(),
            api_key_override: None,
            base_url_override: None,
        };

        info!("Calling LLM for memory nudge...");
        let response = provider.create_chat_completion(req).await?;
        let content = response.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default()
            .trim()
            .to_string();

        if content.is_empty() || content.contains("NO_FACTS") {
            info!("Memory Nudge: No new facts to record.");
            return Ok(());
        }

        let memory_path = get_athena_home().join("MEMORY.md");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&memory_path)
            .context("Failed to open MEMORY.md for appending")?;

        writeln!(file, "\n{}", content)?;
        info!("Memory Nudge: Successfully appended facts to MEMORY.md");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use athena_providers::providers::openai::OpenAIProvider;

    #[tokio::test]
    async fn test_memory_nudge() {
        let mock_server = MockServer::start().await;

        let response_body = serde_json::json!({
            "id": "chatcmpl-nudge",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "- User prefers dark mode\n- User lives in Seattle"
                },
                "finish_reason": "stop"
            }]
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        // Override ATHENA_HOME to a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("ATHENA_HOME", temp_dir.path());
        
        let provider = Arc::new(OpenAIProvider::new(Some("test-key".to_string()), Some(mock_server.uri())));

        let history = vec![
            ChatMessage {
                role: MessageRole::User,
                content: "Remember I like dark mode.".to_string(),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let result = MemoryNudge::run(&history, provider, "gpt-4o").await;
        assert!(result.is_ok(), "Memory Nudge failed");

        // Verify MEMORY.md was created and contains the facts
        let memory_path = temp_dir.path().join("MEMORY.md");
        assert!(memory_path.exists());
        
        let contents = std::fs::read_to_string(memory_path).unwrap();
        assert!(contents.contains("User prefers dark mode"));
        assert!(contents.contains("User lives in Seattle"));
    }
}

// Rust guideline compliant 2026-02-21
