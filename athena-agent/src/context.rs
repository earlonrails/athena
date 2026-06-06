use crate::Message;
use tiktoken_rs::cl100k_base;
use std::sync::Arc;
use athena_providers::{LLMProvider, ChatCompletionRequest, ChatMessage, MessageRole};

pub struct ContextEngine {
    max_tokens: usize,
}

impl ContextEngine {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    /// Counts the approximate number of tokens in a string using the cl100k_base encoding.
    pub fn count_tokens(&self, text: &str) -> usize {
        if let Ok(bpe) = cl100k_base() {
            bpe.encode_with_special_tokens(text).len()
        } else {
            // Fallback approximation if tokenizer fails to load
            text.chars().count() / 4
        }
    }

    /// Compresses a list of messages so that the total token count is under `max_tokens`.
    /// 
    /// Simple implementation: Drops the oldest user/assistant messages if we exceed the limit.
    /// In a real scenario, this would summarize or truncate specific tool outputs.
    pub async fn compress(&self, messages: &[Message], provider: Option<Arc<dyn LLMProvider>>) -> Vec<Message> {
        let mut total_tokens = 0;
        let mut to_summarize = Vec::new();

        // 1. Calculate total tokens
        for msg in messages {
            let tokens = match msg {
                Message::System { content } => self.count_tokens(content),
                Message::User { content, .. } => self.count_tokens(content),
                Message::Assistant { content, .. } => {
                    content.as_ref().map(|c| self.count_tokens(c)).unwrap_or(0)
                }
                Message::Tool { content, .. } => self.count_tokens(content),
            };
            total_tokens += tokens;
        }

        // 2. If under limit, just return
        if total_tokens <= self.max_tokens {
            return messages.to_vec();
        }

        // 3. We are over the limit. Summarize oldest Assistant/Tool turns.
        // Never summarize System or User messages.
        let mut retained = Vec::new();
        let mut current_tokens = total_tokens;
        let mut idx = 0;
        
        while idx < messages.len() && current_tokens > self.max_tokens {
            let msg = &messages[idx];
            match msg {
                Message::System { .. } | Message::User { .. } => {
                    retained.push(msg.clone());
                }
                Message::Assistant { .. } | Message::Tool { .. } => {
                    let tokens = match msg {
                        Message::Assistant { content, .. } => content.as_ref().map(|c| self.count_tokens(c)).unwrap_or(0),
                        Message::Tool { content, .. } => self.count_tokens(content),
                        _ => 0,
                    };
                    to_summarize.push(msg.clone());
                    current_tokens -= tokens;
                }
            }
            idx += 1;
        }
        
        // Add remaining
        for i in idx..messages.len() {
            retained.push(messages[i].clone());
        }

        if to_summarize.is_empty() {
            return retained;
        }

        // Synthesize summary
        let summary_text = if let Some(provider) = provider {
            let mut prompt = String::from("Please write a concise, one-paragraph summary of the following assistant actions and tool results. Retain all factual information, IDs, and paths.\n\n");
            for m in &to_summarize {
                match m {
                    Message::Assistant { content, .. } => prompt.push_str(&format!("Assistant: {}\n", content.as_deref().unwrap_or_default())),
                    Message::Tool { content, .. } => prompt.push_str(&format!("Tool: {}\n", content)),
                    _ => {}
                }
            }
            
            let req = ChatCompletionRequest {
                model: provider.profile().default_aux_model.clone(),
                messages: vec![ChatMessage {
                    role: MessageRole::User,
                    content: prompt,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                }],
                temperature: Some(0.3),
                max_tokens: Some(512),
                top_p: None,
                stop: None,
                stream: false,
                tools: None,
                tool_choice: None,
                extra_body: std::collections::HashMap::new(),
                api_key_override: None,
                base_url_override: None,
            };
            
            match provider.create_chat_completion(req).await {
                Ok(resp) => {
                    if let Some(c) = resp.choices.first() {
                        c.message.content.clone()
                    } else {
                        "Summary generation failed".to_string()
                    }
                }
                Err(_) => "Summary generation failed".to_string()
            }
        } else {
            "Summary generated (mocked)".to_string()
        };
        
        let mut final_msgs = Vec::new();
        let mut summary_inserted = false;
        
        for msg in retained {
            // Insert summary right before the first retained Assistant/Tool message
            if !summary_inserted && matches!(msg, Message::Assistant { .. } | Message::Tool { .. }) {
                final_msgs.push(Message::Assistant {
                    content: Some(format!("<summary>{}</summary>", summary_text)),
                    tool_calls: None,
                    reasoning_content: None,
                });
                summary_inserted = true;
            }
            final_msgs.push(msg);
        }
        
        if !summary_inserted {
            final_msgs.push(Message::Assistant {
                content: Some(format!("<summary>{}</summary>", summary_text)),
                tool_calls: None,
                reasoning_content: None,
            });
        }
        
        final_msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    #[test]
    fn test_context_engine_count_tokens() {
        let engine = ContextEngine::new(100);
        let tokens = engine.count_tokens("Hello world");
        assert!(tokens > 0);
    }

    #[tokio::test]
    async fn test_context_engine_compress() {
        let engine = ContextEngine::new(20);
        let messages = vec![
            Message::System { content: "Sys".to_string() },
            // This long string will exceed 20 tokens easily
            Message::User { content: "This is a very long string that will definitely exceed twenty tokens because it has a lot of words and complex characters.".to_string(), name: None },
            Message::Assistant { content: Some("Short".to_string()), tool_calls: None, reasoning_content: None },
            Message::Tool { content: "Res".to_string(), tool_call_id: "call_1".to_string() }
        ];

        let compressed = engine.compress(&messages, None).await;
        
        // User messages are never compressed, Assistant/Tool messages are compressed first.
        // It should retain Sys, User, and then insert a mocked <summary>
        assert!(compressed.len() >= 3);
        
        // Assert User message is verbatim
        match &compressed[1] {
            Message::User { content, .. } => assert!(content.starts_with("This is a very long string")),
            _ => panic!("Expected User message"),
        }
        
        // Assert Assistant summary
        let has_summary = compressed.iter().any(|m| match m {
            Message::Assistant { content: Some(c), .. } => c.contains("<summary>"),
            _ => false,
        });
        assert!(has_summary, "Missing summary block");
    }

    #[tokio::test]
    async fn test_context_engine_compress_edge_cases() {
        let engine = ContextEngine::new(50);
        let messages = vec![
            Message::Assistant { content: None, tool_calls: Some(vec![]), reasoning_content: None },
            Message::Assistant { content: Some("Content".to_string()), tool_calls: None, reasoning_content: None },
        ];

        let compressed = engine.compress(&messages, None).await;
        
        // Ensure both fit and the empty assistant message doesn't crash token count
        assert_eq!(compressed.len(), 2);
    }
}

// Rust guideline compliant 2026-02-21
