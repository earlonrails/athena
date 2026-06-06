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

#[derive(Clone)]
pub struct AIAgent {
    pub(crate) config: AgentConfig,
    pub(crate) budget: IterationBudget,
    pub(crate) skill_store: Option<Arc<athena_skills::SkillStore>>,
    pub(crate) skill_manager: Option<Arc<athena_skills::SkillManager>>,
    pub(crate) logger: Option<Arc<dyn crate::logger::SessionLogger>>,
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
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        
        let mut self_clone = AIAgent {
            config: self.config.clone(),
            budget: crate::IterationBudget::new(self.config.max_iterations as usize),
            skill_store: self.skill_store.clone(),
            skill_manager: self.skill_manager.clone(),
            logger: self.logger.clone(),
        };

        let user_msg_clone = user_message.to_string();
        let sys_msg_clone = system_message.map(|s| s.to_string());
        let registry_clone = registry.clone();
        let provider_clone = provider.clone();

        tokio::spawn(async move {
            let _ = self_clone.run_conversation_stream(
                &user_msg_clone,
                sys_msg_clone.as_deref(),
                &registry_clone,
                provider_clone,
                tx
            ).await;
        });

        let mut final_response = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                crate::events::AgentEvent::FinalResponse(content) => {
                    final_response = content;
                }
                crate::events::AgentEvent::Error(err) => {
                    return Err(err);
                }
                _ => {}
            }
        }

        Ok(final_response)
    }

    pub async fn run_conversation_stream(
        &mut self,
        user_message: &str,
        system_message: Option<&str>,
        registry: &ToolRegistry,
        provider: Arc<dyn LLMProvider>,
        tx: tokio::sync::mpsc::UnboundedSender<crate::events::AgentEvent>,
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
            
            let max_t = self.config.max_tokens.unwrap_or(80000);
            let ctx_engine = crate::context::ContextEngine::new(max_t);
            let compressed_messages = ctx_engine.compress(&messages, Some(provider.clone())).await;

            let mut api_messages = Vec::new();
            for msg in &compressed_messages {
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
                stream: true,
                tools: if has_tools { Some(api_tools) } else { None },
                tool_choice: if has_tools { Some(athena_providers::ToolChoice::Auto) } else { None },
                extra_body: std::collections::HashMap::new(),
                api_key_override: self.config.api_key.clone(),
                base_url_override: self.config.base_url.clone(),
            };

            let mut stream_res = match provider.create_chat_completion_stream(request).await {
                Ok(resp) => resp.response,
                Err(e) => {
                    let _ = tx.send(crate::events::AgentEvent::Error(format!("API Error: {}", e)));
                    return Err(format!("API Error: {}", e));
                }
            };

            use futures::StreamExt;
            let mut final_content = String::new();
            let mut tool_calls_map: std::collections::BTreeMap<usize, ProviderToolCall> = std::collections::BTreeMap::new();

            while let Some(chunk_res) = stream_res.next().await {
                let chunk = match chunk_res {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(crate::events::AgentEvent::Error(e.to_string()));
                        return Err(format!("Stream Error: {}", e));
                    }
                };
                
                if let Some(usage) = &chunk.usage {
                    let read = usage.cache_read_input_tokens.unwrap_or(0);
                    let creation = usage.cache_creation_input_tokens.unwrap_or(0);
                    if read > 0 || creation > 0 {
                        let _ = tx.send(crate::events::AgentEvent::TokenUsage {
                            cache_read: read,
                            cache_creation: creation,
                        });
                    }
                }
                
                if let Some(delta) = chunk.choices.first().map(|c| &c.delta) {
                    if let Some(content) = &delta.content {
                        final_content.push_str(content);
                        let _ = tx.send(crate::events::AgentEvent::TokenDelta(content.clone()));
                    }
                    if let Some(tcs) = &delta.tool_calls {
                        for tc in tcs {
                            let idx = tc.index.unwrap_or(0) as usize;
                            let entry = tool_calls_map.entry(idx).or_insert_with(|| ProviderToolCall {
                                id: String::new(),
                                r#type: "function".to_string(),
                                function: ProviderToolFunction {
                                    name: String::new(),
                                    arguments: String::new(),
                                }
                            });
                            if let Some(id) = &tc.id {
                                entry.id.push_str(id);
                            }
                            if let Some(f) = &tc.function {
                                if let Some(name) = &f.name {
                                    entry.function.name.push_str(name);
                                }
                                if let Some(args) = &f.arguments {
                                    entry.function.arguments.push_str(args);
                                }
                            }
                        }
                    }
                }
            }

            let mut our_tool_calls = Vec::new();
            for (_, tc) in tool_calls_map {
                our_tool_calls.push(crate::ToolCall {
                    id: tc.id.clone(),
                    call_type: tc.r#type.clone(),
                    function: crate::FunctionCall {
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    },
                });
            }

            let choice = athena_providers::base::ChatMessage {
                role: athena_providers::base::MessageRole::Assistant,
                content: final_content.clone(),
                name: None,
                tool_calls: if our_tool_calls.is_empty() { None } else { Some(our_tool_calls.iter().map(|tc| ProviderToolCall {
                    id: tc.id.clone(),
                    r#type: tc.call_type.clone(),
                    function: ProviderToolFunction {
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    }
                }).collect()) },
                tool_call_id: None,
            };

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
                let history = messages.iter().filter_map(|m| {
                    match m {
                        Message::User { content, .. } => Some(ChatMessage { role: MessageRole::User, content: content.clone(), name: None, tool_calls: None, tool_call_id: None }),
                        Message::Assistant { content, .. } => Some(ChatMessage { role: MessageRole::Assistant, content: content.clone().unwrap_or_default(), name: None, tool_calls: None, tool_call_id: None }),
                        _ => None
                    }
                }).collect::<Vec<_>>();

                let p_clone = provider.clone();
                let m_clone = self.config.model.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = athena_skills::MemoryNudge::run(&history, p_clone, &m_clone).await {
                        tracing::error!("Error during memory nudge: {}", e);
                    }
                });

                self.log_session_to_db(&messages, system_message);
                
                let _ = tx.send(crate::events::AgentEvent::FinalResponse(final_content.clone()));
                return Ok(final_content);
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

        let history = messages.iter().filter_map(|m| {
            match m {
                Message::User { content, .. } => Some(ChatMessage { role: MessageRole::User, content: content.clone(), name: None, tool_calls: None, tool_call_id: None }),
                Message::Assistant { content, .. } => Some(ChatMessage { role: MessageRole::Assistant, content: content.clone().unwrap_or_default(), name: None, tool_calls: None, tool_call_id: None }),
                _ => None
            }
        }).collect::<Vec<_>>();

        let p_clone = provider.clone();
        let m_clone = self.config.model.clone();
        
        tokio::spawn(async move {
            if let Err(e) = athena_skills::MemoryNudge::run(&history, p_clone, &m_clone).await {
                tracing::error!("Error during memory nudge: {}", e);
            }
        });

        self.log_session_to_db(&messages, system_message);
        Err("Max iterations reached".to_string())
    }

    fn log_session_to_db(&self, messages: &[Message], system_message: Option<&str>) {
        if let Some(logger) = &self.logger {
            logger.log_session(messages, system_message);
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
        let response_body = "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello from the mocked AI!\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_bytes(response_body.as_bytes())
                .insert_header("Content-Type", "text/event-stream"))
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
        
        assert!(result.is_ok(), "Result failed: {:?}", result.unwrap_err());
        assert_eq!(result.unwrap(), "Hello from the mocked AI!");
    }

    #[tokio::test]
    async fn test_run_conversation_no_system_message() {
        let mock_server = MockServer::start().await;

        let response_body = "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Response without system\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_bytes(response_body.as_bytes())
                .insert_header("Content-Type", "text/event-stream"))
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

        let response_body = "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string(response_body)
                .insert_header("Content-Type", "text/event-stream"))
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
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("API Error") || err_msg.contains("Streaming error") || err_msg.contains("400"), "Unexpected error: {}", err_msg);
    }

    #[tokio::test]
    async fn test_run_conversation_with_tool_calls() {
        let mock_server = MockServer::start().await;

        // Response with tool calls
        let response_body = "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Let me help you\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"run_command\",\"arguments\":\"{\\\"command\\\": \\\"ls\\\"}\"}}]},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";

        // Second response without tool calls (completion)
        let completion_response = "data: {\"id\":\"chatcmpl-124\",\"object\":\"chat.completion.chunk\",\"created\":1677652289,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Done!\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-124\",\"object\":\"chat.completion.chunk\",\"created\":1677652289,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_bytes(response_body.as_bytes())
                .insert_header("Content-Type", "text/event-stream"))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_bytes(completion_response.as_bytes())
                .insert_header("Content-Type", "text/event-stream"))
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
        let response_body = "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Looping\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"run_command\",\"arguments\":\"{}\"}}]},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string(response_body)
                .insert_header("Content-Type", "text/event-stream"))
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

        let response_body = "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1677652288,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_bytes(response_body.as_bytes())
                .insert_header("Content-Type", "text/event-stream"))
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
        assert!(result.is_err(), "Expected error but got {:?}", result);
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("API Error") || err_msg.contains("Streaming error") || err_msg.contains("401"), "Unexpected error: {}", err_msg);
    }
}

// Rust guideline compliant 2026-02-21
