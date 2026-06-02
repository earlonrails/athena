use crate::{AgentConfig, AIAgentBuilder, IterationBudget, Message, ToolCall};
use athena_tools::{ToolRegistry};
use tokio::task::JoinHandle;
use tracing::{debug};
use std::sync::Arc;
use athena_providers::{
    LLMProvider,
    base::{
        ChatCompletionRequest, ChatMessage, MessageRole, ToolDefinition,
        ToolCall as ProviderToolCall, ToolFunction as ProviderToolFunction,
    },
};

pub struct AIAgent {
    pub(crate) config: AgentConfig,
    pub(crate) budget: IterationBudget,
    pub(crate) skill_store: Option<Arc<athena_skills::SkillStore>>,
    pub(crate) skill_manager: Option<Arc<athena_skills::SkillManager>>,
}

impl AIAgent {
    pub fn builder() -> AIAgentBuilder {
        AIAgentBuilder::new()
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    pub fn base_url(&self) -> Option<&str> {
        self.config.base_url.as_deref()
    }

    pub fn api_key(&self) -> Option<&str> {
        self.config.api_key.as_deref()
    }

    pub async fn run_conversation(
        &mut self,
        user_message: &str,
        system_message: Option<&str>,
        registry: &ToolRegistry,
        provider: Arc<dyn LLMProvider>,
    ) -> Result<String, String> {
        let mut messages = Vec::new();

        let mut sys_content = system_message.unwrap_or_default().to_string();
        
        let mut used_skills = Vec::new();

        // 1. Search for relevant skills and inject them into the system prompt
        if let (Some(store), Some(manager)) = (&self.skill_store, &self.skill_manager) {
            // we use the user's initial message as the query
            if let Ok(skills) = manager.search_skills(user_message, 3) {
                if !skills.is_empty() {
                    sys_content.push_str("\n\nRELEVANT SKILLS AVAILABLE:\n");
                    for skill in skills {
                        sys_content.push_str(&format!("Skill: {}\nDescription: {}\nInstructions:\n{}\n\n", skill.name, skill.description, skill.instructions));
                        used_skills.push(skill.id.clone());
                        let _ = store.record_use(&skill.id);
                    }
                }
            }
        }

        if !sys_content.is_empty() {
            messages.push(Message::System { content: sys_content });
        }
        messages.push(Message::User { content: user_message.to_string(), name: None });

        let mut api_call_count = 0;

        while self.budget.consume() {
            debug!("Starting iteration {} / {}", api_call_count, self.config.max_iterations);
            println!("🤖 [Thinking] Consulting AI model...");

            let mut api_messages = Vec::new();
            for msg in &messages {
                match msg {
                    Message::System { content } => {
                        api_messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: content.clone(),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                    Message::User { content, name } => {
                        api_messages.push(ChatMessage {
                            role: MessageRole::User,
                            content: content.clone(),
                            name: name.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                    Message::Assistant { content, tool_calls, .. } => {
                        let provider_tool_calls = tool_calls.as_ref().map(|calls| {
                            calls.iter().map(|tc| ProviderToolCall {
                                id: tc.id.clone(),
                                r#type: "function".to_string(),
                                function: ProviderToolFunction {
                                    name: tc.function.name.clone(),
                                    arguments: tc.function.arguments.clone(),
                                },
                            }).collect()
                        });
                        
                        api_messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: content.clone().unwrap_or_default(),
                            name: None,
                            tool_calls: provider_tool_calls,
                            tool_call_id: None,
                        });
                    }
                    Message::Tool { content, tool_call_id } => {
                        api_messages.push(ChatMessage {
                            role: MessageRole::Tool,
                            content: content.clone(),
                            name: None,
                            tool_calls: None,
                            tool_call_id: Some(tool_call_id.clone()),
                        });
                    }
                }
            }

            let tool_schemas = registry.get_definitions(&std::collections::HashSet::new(), true).await;
            
            let mut api_tools = Vec::new();
            for schema in tool_schemas {
                if let Ok(tool) = serde_json::from_value::<ToolDefinition>(schema) {
                    api_tools.push(tool);
                }
            }

            let has_tools = !api_tools.is_empty();
            let request = ChatCompletionRequest {
                model: self.config.model.clone(),
                messages: api_messages,
                temperature: None,
                max_tokens: None,
                top_p: None,
                stop: None,
                stream: false,
                tools: if has_tools { Some(api_tools) } else { None },
                tool_choice: if has_tools { Some(athena_providers::ToolChoice::Auto) } else { None },
                extra_body: std::collections::HashMap::new(),
                api_key_override: self.config.api_key.clone(),
                base_url_override: self.config.base_url.clone(),
            };

            let response = match provider.create_chat_completion(request).await {
                Ok(resp) => resp,
                Err(e) => return Err(format!("API Error: {}", e)),
            };

            let choice = &response.choices[0].message;

            let mut our_tool_calls = Vec::new();
            if let Some(ref tcs) = choice.tool_calls {
                for tc in tcs {
                    our_tool_calls.push(ToolCall {
                        id: tc.id.clone(),
                        call_type: "function".to_string(),
                        function: crate::FunctionCall {
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        },
                    });
                }
            }

            let assistant_msg = Message::Assistant {
                content: if choice.content.is_empty() { None } else { Some(choice.content.clone()) },
                tool_calls: if our_tool_calls.is_empty() { None } else { Some(our_tool_calls.clone()) },
                reasoning_content: None,
            };

            messages.push(assistant_msg);
            api_call_count += 1;

            if our_tool_calls.is_empty() {
                // Done! Log the session
                self.log_session_to_db(&messages, system_message);
                
                return Ok(choice.content.clone());
            }

            // Execute tools concurrently
            let mut handles: Vec<JoinHandle<(String, String)>> = Vec::new();
            let mut turn_successful = true;
            for tc in &our_tool_calls {
                let tool_name = tc.function.name.clone();
                let args_str = tc.function.arguments.clone();
                let tool_id = tc.id.clone();
                let reg = registry.clone();

                let icon = match tool_name.as_str() {
                    "run_command" | "execute_code" => "🐳 [Sandbox]",
                    _ => "🛠️ [Calling Tool]",
                };
                println!("{} {} with args: {}", icon, tool_name, args_str);

                let handle = tokio::spawn(async move {
                    let parsed_args = serde_json::from_str(&args_str).unwrap_or_else(|_| serde_json::json!({}));
                    let result = reg.dispatch(&tool_name, parsed_args).await;
                    (tool_id, result)
                });
                handles.push(handle);
            }

            for handle in handles {
                let (tool_id, result_str) = handle.await.map_err(|e| e.to_string())?;

                // Find matching tool call to print its name and style appropriately
                let tool_name = our_tool_calls.iter()
                    .find(|tc| tc.id == tool_id)
                    .map(|tc| tc.function.name.as_str())
                    .unwrap_or("unknown");

                let icon = match tool_name {
                    "run_command" | "execute_code" => "🐳 [Sandbox Result]",
                    _ => "✔ [Tool Result]"
                };

                // Clean output preview to prevent huge spam
                let preview = if result_str.len() > 180 {
                    format!("{}...", &result_str[..180])
                } else {
                    result_str.clone()
                };
                println!("{} {}: {}", icon, tool_name, preview.trim());

                messages.push(Message::Tool {
                    content: result_str.clone(),
                    tool_call_id: tool_id,
                });

                // If any tool result contains "error", we consider the turn unsuccessful
                if result_str.to_lowercase().contains("error") {
                    turn_successful = false;
                }
            }
            
            // Record success for skills used in this turn
            if turn_successful {
                if let Some(store) = &self.skill_store {
                    for skill_id in &used_skills {
                        let _ = store.record_success(skill_id);
                    }
                }
            }
        }

        // End of run
        // Trigger skill synthesis if configured and threshold met
        if let (Some(store), Some(manager)) = (&self.skill_store, &self.skill_manager) {
            // Check threshold: e.g. >= 4 api calls means at least 3 tools used
            if api_call_count >= 4 {
                let s = store.clone();
                let m = manager.clone();
                let p = provider.clone();
                let mod_name = self.config.model.clone();
                // Pass the messages from this conversation
                let mut synthesis_history = Vec::new();
                for msg in &messages {
                    match msg {
                        Message::System { content } => synthesis_history.push(ChatMessage { role: MessageRole::System, content: content.clone(), name: None, tool_calls: None, tool_call_id: None }),
                        Message::User { content, name } => synthesis_history.push(ChatMessage { role: MessageRole::User, content: content.clone(), name: name.clone(), tool_calls: None, tool_call_id: None }),
                        Message::Assistant { content, tool_calls, .. } => {
                            let provider_tool_calls = tool_calls.as_ref().map(|calls| {
                                calls.iter().map(|tc| ProviderToolCall {
                                    id: tc.id.clone(), r#type: "function".to_string(),
                                    function: ProviderToolFunction { name: tc.function.name.clone(), arguments: tc.function.arguments.clone() },
                                }).collect()
                            });
                            synthesis_history.push(ChatMessage { role: MessageRole::Assistant, content: content.clone().unwrap_or_default(), name: None, tool_calls: provider_tool_calls, tool_call_id: None });
                        },
                        Message::Tool { content, tool_call_id } => synthesis_history.push(ChatMessage { role: MessageRole::Tool, content: content.clone(), name: None, tool_calls: None, tool_call_id: Some(tool_call_id.clone()) }),
                    }
                }
                
                tokio::spawn(async move {
                    if let Err(e) = athena_skills::SkillSynthesizer::synthesize(
                        synthesis_history,
                        p.clone(),
                        &mod_name,
                        s.clone(),
                        m.clone(),
                        0.92,
                    ).await {
                        tracing::error!("Error during skill synthesis: {}", e);
                    }
                    
                    if let Err(e) = athena_skills::SkillImprover::run_improvement_pass(
                        s.clone(),
                        m.clone(),
                        p.clone(),
                        &mod_name,
                    ).await {
                        tracing::error!("Error during skill improvement: {}", e);
                    }
                });
            }
        }

        self.log_session_to_db(&messages, system_message);
        Err("Max iterations reached".to_string())
    }

    fn log_session_to_db(&self, messages: &[Message], system_message: Option<&str>) {
        use athena_state::db::{SessionDB, Session, MessageRow};
        use std::time::{SystemTime, UNIX_EPOCH};

        if let Ok(db) = SessionDB::new(None) {
            let session_id = uuid::Uuid::new_v4().to_string();
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64();

            let session = Session {
                id: session_id.clone(),
                title: Some("Autonomous Interaction".to_string()),
                model: Some(self.config.model.clone()),
                system_prompt: system_message.map(|s| s.to_string()),
                started_at: timestamp,
            };

            if let Err(e) = db.insert_session(&session) {
                tracing::warn!("Failed to insert session: {}", e);
                return;
            }

            for msg in messages {
                let (role, content, tool_calls) = match msg {
                    Message::System { content } => ("system", Some(content.clone()), None),
                    Message::User { content, .. } => ("user", Some(content.clone()), None),
                    Message::Assistant { content, tool_calls, .. } => {
                        let tc_str = tool_calls.as_ref().map(|tc| serde_json::to_string(tc).unwrap_or_default());
                        ("assistant", content.clone(), tc_str)
                    }
                    Message::Tool { content, .. } => ("tool", Some(content.clone()), None),
                };

                let msg_row = MessageRow {
                    id: 0, // auto-incremented
                    session_id: session_id.clone(),
                    role: role.to_string(),
                    content,
                    tool_calls,
                    timestamp,
                };
                let _ = db.insert_message(&msg_row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use athena_tools::ToolRegistry;
    use serde_json::json;

    #[test]
    fn test_builder() {
        let agent = AIAgent::builder();
        // Just verify it creates a builder without panicking
        assert!(agent.build().budget.remaining() > 0);
    }

    #[test]
    fn test_model() {
        let agent = AIAgentBuilder::new()
            .model("test-model")
            .build();
        assert_eq!(agent.model(), "test-model");
    }

    #[tokio::test]
    async fn test_run_conversation_mocked() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock OpenAI chat completions endpoint
        let response_body = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello from the mocked AI!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 9,
                "completion_tokens": 12,
                "total_tokens": 21
            }
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let registry = ToolRegistry::new();
        let mut agent = AIAgentBuilder::new()
            .model("gpt-4o")
            .base_url(mock_server.uri())
            .api_key("fake-key")
            .build();

        let provider = Arc::new(athena_providers::providers::openai::OpenAIProvider::new(None, None));
        let result = agent.run_conversation("Say hello", Some("System prompt"), &registry, provider).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello from the mocked AI!");
    }

    #[tokio::test]
    async fn test_run_conversation_no_system_message() {
        let mock_server = MockServer::start().await;

        let response_body = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Response without system"
                },
                "finish_reason": "stop"
            }]
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let registry = ToolRegistry::new();
        let mut agent = AIAgentBuilder::new()
            .model("gpt-4o")
            .base_url(mock_server.uri())
            .api_key("fake-key")
            .build();

        let provider = Arc::new(athena_providers::providers::openai::OpenAIProvider::new(None, None));
        let result = agent.run_conversation("Hello", None, &registry, provider).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Response without system");
    }

    #[tokio::test]
    async fn test_run_conversation_api_error() {
        let mock_server = MockServer::start().await;

        let response_body = json!({
            "error": {
                "message": "Invalid API key",
                "type": "BadRequestError"
            }
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let registry = ToolRegistry::new();
        let mut agent = AIAgentBuilder::new()
            .model("gpt-4o")
            .base_url(mock_server.uri())
            .api_key("invalid-key")
            .build();

        let provider = Arc::new(athena_providers::providers::openai::OpenAIProvider::new(None, None));
        let result = agent.run_conversation("Hello", None, &registry, provider).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API Error"));
    }

    #[tokio::test]
    async fn test_run_conversation_with_tool_calls() {
        let mock_server = MockServer::start().await;

        // Response with tool calls
        let response_body = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Let me help you",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "run_command",
                            "arguments": "{\"command\": \"ls\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        // Second response without tool calls (completion)
        let completion_response = json!({
            "id": "chatcmpl-124",
            "object": "chat.completion",
            "created": 1677652289,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Done!"
                },
                "finish_reason": "stop"
            }]
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body.clone()))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(completion_response))
            .mount(&mock_server)
            .await;

        let registry = ToolRegistry::new();
        let mut agent = AIAgentBuilder::new()
            .model("gpt-4o")
            .base_url(mock_server.uri())
            .api_key("fake-key")
            .max_iterations(10)
            .build();

        let provider = Arc::new(athena_providers::providers::openai::OpenAIProvider::new(None, None));
        let result = agent.run_conversation("List files", None, &registry, provider).await;
        // Tool execution will fail for non-existent tools, but we're testing the path
        // The test verifies the code path for tool calls is exercised
        let _ = result;
    }

    #[tokio::test]
    async fn test_run_conversation_max_iterations() {
        let mock_server = MockServer::start().await;

        // Always respond with a tool call to keep looping
        let response_body = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Looping",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "run_command",
                            "arguments": "{}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let registry = ToolRegistry::new();
        let mut agent = AIAgentBuilder::new()
            .model("gpt-4o")
            .base_url(mock_server.uri())
            .api_key("fake-key")
            .max_iterations(2)
            .build();

        let provider = Arc::new(athena_providers::providers::openai::OpenAIProvider::new(None, None));
        let result = agent.run_conversation("Loop", None, &registry, provider).await;
        // Should eventually fail with max iterations or tool execution error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_conversation_empty_content() {
        let mock_server = MockServer::start().await;

        let response_body = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "stop"
            }]
        });

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let registry = ToolRegistry::new();
        let mut agent = AIAgentBuilder::new()
            .model("gpt-4o")
            .base_url(mock_server.uri())
            .api_key("fake-key")
            .build();

        let provider = Arc::new(athena_providers::providers::openai::OpenAIProvider::new(None, None));
        let result = agent.run_conversation("Test", None, &registry, provider).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[tokio::test]
    async fn test_missing_api_key_behavior() {
        // Ensure the environment variable is not secretly allowing it to work
        std::env::remove_var("OPENAI_API_KEY");

        // Build an agent with NO api key set
        let mut agent = AIAgent::builder()
            .model("mistral-large-latest")
            // We intentionally do not call .api_key() here!
            .max_iterations(1)
            .build();

        let registry = ToolRegistry::new();
        
        // When we run the conversation without an API key
        let provider = Arc::new(athena_providers::providers::openai::OpenAIProvider::new(None, None));
        let result = agent.run_conversation("Hello", None, &registry, provider).await;

        // It should immediately fail with an API Error instead of crashing
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("API Error"));
    }
}

// Rust guideline compliant 2026-02-21
