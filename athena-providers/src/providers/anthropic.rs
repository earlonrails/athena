use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use futures::StreamExt;
use eventsource_stream::Eventsource;

use crate::base::*;
use crate::error::*;
use crate::registry::register_provider;

/// Anthropic provider profile
pub fn anthropic_profile() -> ProviderProfile {
    let mut default_headers = HashMap::new();
    default_headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());
    
    ProviderProfile {
        name: "anthropic".to_string(),
        api_mode: ApiMode::AnthropicMessages,
        aliases: vec![
            "claude".to_string(),
            "claude-oauth".to_string(),
            "claude-code".to_string(),
        ],
        display_name: "Anthropic".to_string(),
        description: "Anthropic — Claude models via Messages API".to_string(),
        signup_url: "https://www.anthropic.com/".to_string(),
        env_vars: vec![
            "ANTHROPIC_API_KEY".to_string(),
            "ANTHROPIC_TOKEN".to_string(),
            "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
        ],
        base_url: "https://api.anthropic.com".to_string(),
        models_url: "https://api.anthropic.com/v1/models".to_string(),
        auth_type: AuthType::ApiKey,
        supports_health_check: true,
        fallback_models: vec![
            "claude-3-5-sonnet-20251022".to_string(),
            "claude-3-7-sonnet-20250219".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
        ],
        hostname: "api.anthropic.com".to_string(),
        default_headers,
        fixed_temperature: None,
        default_max_tokens: None,
        default_aux_model: "claude-3-haiku-20240307".to_string(),
    }
}

/// Anthropic provider implementation
pub struct AnthropicProvider {
    profile: ProviderProfile,
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self {
            profile: anthropic_profile(),
        }
    }
    
    pub fn new_with_profile(profile: ProviderProfile) -> Self {
        Self { profile }
    }

    fn build_anthropic_request(&self, request: &ChatCompletionRequest) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        let mut messages = Vec::new();
        let mut system_prompt = String::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    system_prompt.push_str(&msg.content);
                    system_prompt.push('\n');
                }
                MessageRole::User => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": msg.content
                    }));
                }
                MessageRole::Assistant => {
                    let mut content = Vec::new();
                    if !msg.content.is_empty() {
                        content.push(serde_json::json!({
                            "type": "text",
                            "text": msg.content
                        }));
                    }
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                                .unwrap_or_else(|_| serde_json::json!({}));
                            content.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input
                            }));
                        }
                    }
                    if content.is_empty() {
                        content.push(serde_json::json!({
                            "type": "text",
                            "text": ""
                        }));
                    }
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": content
                    }));
                }
                MessageRole::Tool => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.tool_call_id.clone().unwrap_or_default(),
                            "content": msg.content
                        }]
                    }));
                }
            }
        }
        
        if !system_prompt.is_empty() {
            body["system"] = serde_json::Value::String(system_prompt.trim().to_string());
        }

        // Merge adjacent messages of the same role (Anthropic requires alternating roles)
        let mut merged_messages: Vec<serde_json::Value> = Vec::new();
        for msg in messages {
            if let Some(last) = merged_messages.last_mut() {
                if last["role"] == msg["role"] {
                    let mut last_content = if last["content"].is_array() {
                        if let Some(arr) = last["content"].as_array() {
                            arr.clone()
                        } else {
                            vec![serde_json::json!({"type": "text", "text": last["content"].as_str().unwrap_or_default()})]
                        }
                    } else {
                        vec![serde_json::json!({"type": "text", "text": last["content"].as_str().unwrap_or_default()})]
                    };
                    
                    let msg_content = if msg["content"].is_array() {
                        if let Some(arr) = msg["content"].as_array() {
                            arr.clone()
                        } else {
                            vec![serde_json::json!({"type": "text", "text": msg["content"].as_str().unwrap_or_default()})]
                        }
                    } else {
                        vec![serde_json::json!({"type": "text", "text": msg["content"].as_str().unwrap_or_default()})]
                    };
                    
                    last_content.extend(msg_content);
                    *last = serde_json::json!({
                        "role": last["role"].clone(),
                        "content": last_content
                    });
                    continue;
                }
            }
            merged_messages.push(msg);
        }

        body["messages"] = serde_json::Value::Array(merged_messages);

        if let Some(temp) = request.temperature {
            if let Some(n) = serde_json::Number::from_f64(temp as f64) {
                body["temperature"] = serde_json::Value::Number(n);
            }
        }
        if let Some(top_p) = request.top_p {
            if let Some(n) = serde_json::Number::from_f64(top_p as f64) {
                body["top_p"] = serde_json::Value::Number(n);
            }
        }
        if let Some(stop) = &request.stop {
            body["stop_sequences"] = serde_json::Value::Array(
                stop.iter().map(|s| serde_json::Value::String(s.clone())).collect()
            );
        }

        if let Some(tools) = &request.tools {
            let anthropic_tools: Vec<_> = tools.iter().map(|t| {
                let mut tool_obj = serde_json::json!({
                    "name": t.function.name,
                    "input_schema": t.function.parameters
                });
                if let Some(desc) = &t.function.description {
                    tool_obj["description"] = serde_json::Value::String(desc.clone());
                }
                tool_obj
            }).collect();
            if !anthropic_tools.is_empty() {
                body["tools"] = serde_json::Value::Array(anthropic_tools);
            }
        }

        if let Some(tool_choice) = &request.tool_choice {
            match tool_choice {
                ToolChoice::Auto => {
                    body["tool_choice"] = serde_json::json!({"type": "auto"});
                }
                ToolChoice::Required => {
                    body["tool_choice"] = serde_json::json!({"type": "any"});
                }
                ToolChoice::Specific(name) => {
                    body["tool_choice"] = serde_json::json!({"type": "tool", "name": name});
                }
                _ => {}
            }
        }

        for (key, value) in &request.extra_body {
            body[key] = value.clone();
        }

        body
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }
    
    async fn fetch_models(
        &self,
        api_key: Option<&str>,
        _timeout: f64,
    ) -> std::result::Result<Vec<String>, ProviderError> {
        let client = reqwest::Client::new();
        let url = if self.profile.models_url.is_empty() {
            "https://api.anthropic.com/v1/models".to_string()
        } else {
            self.profile.models_url.clone()
        };
        
        let mut request = client.request(reqwest::Method::GET, &url);
        
        let mut resolved_key = api_key.map(|k| k.to_string());
        if resolved_key.is_none() {
            for env_var in &self.profile.env_vars {
                if let Some(val) = athena_core::config::get_env_value(env_var) {
                    resolved_key = Some(val);
                    break;
                }
            }
        }
        
        if let Some(key) = resolved_key {
            request = request.header("x-api-key", key);
        }
        
        request = request
            .header("anthropic-version", "2023-06-01")
            .header("Accept", "application/json");
        
        for (key, value) in &self.profile.default_headers {
            request = request.header(key, value);
        }
        
        let req = request.build().map_err(|e| ProviderError::ApiRequestFailed(e.to_string()))?;
        let response = client.execute(req)
            .await
            .map_err(|e| ProviderError::ApiRequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(ProviderError::ApiRequestFailed(format!(
                "HTTP {}: {}", 
                response.status(), 
                response.text().await.unwrap_or_default()
            )));
        }
        
        let data: serde_json::Value = response.json().await
            .map_err(|e| ProviderError::InvalidResponseFormat(e.to_string()))?;
        
        let default_vec = Vec::new();
        let models = data.get("data").and_then(|d| d.as_array()).unwrap_or(&default_vec);
        
        Ok(models.iter()
            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
            .collect())
    }
    
    async fn create_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> std::result::Result<ChatCompletionResponse, ProviderError> {
        let body = self.build_anthropic_request(&request);
        
        let client = reqwest::Client::new();
        let url = format!("{}/v1/messages", request.base_url_override.as_deref().unwrap_or(&self.profile.base_url));
        
        let mut request_builder = client.request(reqwest::Method::POST, &url);
        
        let mut resolved_key = request.api_key_override.clone();
        if resolved_key.is_none() {
            for env_var in &self.profile.env_vars {
                if let Some(val) = athena_core::config::get_env_value(env_var) {
                    resolved_key = Some(val);
                    break;
                }
            }
        }
        
        if let Some(api_key) = resolved_key {
            request_builder = request_builder.header("x-api-key", api_key);
        }
        
        request_builder = request_builder
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");
        
        for (key, value) in &self.profile.default_headers {
            request_builder = request_builder.header(key, value);
        }
        
        let response = request_builder
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::ApiRequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(ProviderError::ApiRequestFailed(format!(
                "HTTP {}: {}", 
                response.status(), 
                response.text().await.unwrap_or_default()
            )));
        }
        
        let data: serde_json::Value = response.json().await
            .map_err(|e| ProviderError::InvalidResponseFormat(e.to_string()))?;
        
        let default_vec = Vec::new();
        let content_blocks = data.get("content").and_then(|c| c.as_array()).unwrap_or(&default_vec);
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for block in content_blocks {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                    text_content.push_str(t);
                }
            } else if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                if let (Some(id), Some(name), Some(input)) = (
                    block.get("id").and_then(|i| i.as_str()),
                    block.get("name").and_then(|n| n.as_str()),
                    block.get("input")
                ) {
                    tool_calls.push(ToolCall {
                        id: id.to_string(),
                        r#type: "function".to_string(),
                        function: ToolFunction {
                            name: name.to_string(),
                            arguments: serde_json::to_string(input).unwrap_or_default(),
                        }
                    });
                }
            }
        }
        
        let choice = Choice {
            index: 0,
            message: ChatMessage {
                role: MessageRole::Assistant,
                content: text_content,
                name: None,
                tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                tool_call_id: None,
            },
            finish_reason: data.get("stop_reason").and_then(|s| s.as_str()).map(|s| s.to_string()),
        };
        
        let usage = data.get("usage").map(|u| Usage {
                prompt_tokens: u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0),
                completion_tokens: u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0),
                total_tokens: u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) +
                             u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0),
            });
        
        Ok(ChatCompletionResponse {
            id: data.get("id").and_then(|id| id.as_str()).unwrap_or_default().to_string(),
            model: data.get("model").and_then(|m| m.as_str()).unwrap_or_default().to_string(),
            created: 0,
            choices: vec![choice],
            usage,
        })
    }
    
    async fn create_chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> std::result::Result<ChatCompletionStream, ProviderError> {
        let mut body = self.build_anthropic_request(&request);
        body["stream"] = serde_json::Value::Bool(true);
        
        let client = reqwest::Client::new();
        let url = format!("{}/v1/messages", request.base_url_override.as_deref().unwrap_or(&self.profile.base_url));
        
        let mut request_builder = client.request(reqwest::Method::POST, &url);
        
        let mut resolved_key = request.api_key_override.clone();
        if resolved_key.is_none() {
            for env_var in &self.profile.env_vars {
                if let Some(val) = athena_core::config::get_env_value(env_var) {
                    resolved_key = Some(val);
                    break;
                }
            }
        }
        
        if let Some(api_key) = resolved_key {
            request_builder = request_builder.header("x-api-key", api_key);
        }
        
        request_builder = request_builder
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");
        
        for (key, value) in &self.profile.default_headers {
            request_builder = request_builder.header(key, value);
        }
        
        let response = request_builder
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::ApiRequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(ProviderError::ApiRequestFailed(format!(
                "HTTP {}: {}", 
                response.status(), 
                response.text().await.unwrap_or_default()
            )));
        }

        let stream = response.bytes_stream().eventsource();
        
        let mapped_stream = stream.map(|event_res| {
            let event = event_res.map_err(|e| ProviderError::StreamingError(e.to_string()))?;
            let data: serde_json::Value = serde_json::from_str(&event.data)
                .map_err(|e| ProviderError::StreamingError(e.to_string()))?;
                
            let event_type = if event.event.is_empty() || event.event == "message" {
                data.get("type").and_then(|t| t.as_str()).unwrap_or_default().to_string()
            } else {
                event.event
            };
            
            let mut choices = Vec::new();
            
            match event_type.as_str() {
                "message_start" => {
                    let role = data["message"]["role"].as_str().map(|r| match r {
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        _ => MessageRole::Assistant,
                    });
                    choices.push(StreamChoice {
                        index: 0,
                        delta: StreamDelta {
                            role,
                            content: None,
                            tool_calls: None,
                        },
                        finish_reason: None,
                    });
                }
                "content_block_start"
                    if data["content_block"]["type"] == "tool_use" => {
                        let id = data["content_block"]["id"].as_str().map(|s| s.to_string());
                        let name = data["content_block"]["name"].as_str().map(|s| s.to_string());
                        choices.push(StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: None,
                                content: None,
                                tool_calls: Some(vec![StreamToolCall {
                                    index: Some(0),
                                    id,
                                    r#type: Some("function".to_string()),
                                    function: Some(StreamToolFunction {
                                        name,
                                        arguments: Some(String::new()),
                                    }),
                                }]),
                            },
                            finish_reason: None,
                        });
                    }
                "content_block_delta" => {
                    if data["delta"]["type"] == "text_delta" {
                        if let Some(text) = data["delta"]["text"].as_str() {
                            choices.push(StreamChoice {
                                index: 0,
                                delta: StreamDelta {
                                    role: None,
                                    content: Some(text.to_string()),
                                    tool_calls: None,
                                },
                                finish_reason: None,
                            });
                        }
                    } else if data["delta"]["type"] == "input_json_delta" {
                        if let Some(partial_json) = data["delta"]["partial_json"].as_str() {
                            choices.push(StreamChoice {
                                index: 0,
                                delta: StreamDelta {
                                    role: None,
                                    content: None,
                                    tool_calls: Some(vec![StreamToolCall {
                                        index: Some(0),
                                        id: None,
                                        r#type: None,
                                        function: Some(StreamToolFunction {
                                            name: None,
                                            arguments: Some(partial_json.to_string()),
                                        }),
                                    }]),
                                },
                                finish_reason: None,
                            });
                        }
                    }
                }
                "message_delta" => {
                    if let Some(stop_reason) = data["delta"]["stop_reason"].as_str() {
                        choices.push(StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: None,
                                content: None,
                                tool_calls: None,
                            },
                            finish_reason: Some(stop_reason.to_string()),
                        });
                    }
                }
                _ => {} // Ignore other event types
            }
            
            Ok(StreamChunk {
                id: data["message"]["id"].as_str().unwrap_or_default().to_string(),
                model: String::new(),
                created: None,
                choices,
            })
        });

        Ok(ChatCompletionStream {
            response: Box::new(mapped_stream),
        })
    }
}

/// Register the Anthropic provider
pub fn register() {
    register_provider(Arc::new(AnthropicProvider::new()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};

    #[tokio::test]
    async fn test_anthropic_profile() {
        let profile = anthropic_profile();
        assert_eq!(profile.name, "anthropic");
        assert_eq!(profile.api_mode, ApiMode::AnthropicMessages);
        assert_eq!(profile.auth_type, AuthType::ApiKey);
        assert_eq!(profile.default_headers.get("anthropic-version").unwrap(), "2023-06-01");
    }

    #[tokio::test]
    async fn test_anthropic_register() {
        // Test that register function works correctly
        let profile = anthropic_profile();
        assert_eq!(profile.name, "anthropic");
        assert_eq!(profile.api_mode, ApiMode::AnthropicMessages);
        assert_eq!(profile.auth_type, AuthType::ApiKey);
    }

    #[tokio::test]
    async fn test_anthropic_new_with_profile() {
        let profile = anthropic_profile();
        let provider = AnthropicProvider::new_with_profile(profile);
        assert_eq!(provider.profile().name, "anthropic");
    }

    #[tokio::test]
    async fn test_anthropic_provider_new() {
        let provider = AnthropicProvider::new();
        assert_eq!(provider.profile().name, "anthropic");

        let custom_profile = ProviderProfile::new("custom_anthropic");
        let custom_provider = AnthropicProvider::new_with_profile(custom_profile);
        assert_eq!(custom_provider.profile().name, "custom_anthropic");
    }

    #[tokio::test]
    async fn test_build_anthropic_request() {
        let provider = AnthropicProvider::new();
        
        let request = ChatCompletionRequest {
            model: "claude-3-opus-20240229".to_string(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::System,
                    content: "sys_prompt".to_string(),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: MessageRole::User,
                    content: "user_msg".to_string(),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.5),
            max_tokens: Some(1024),
            top_p: Some(0.5),
            stop: Some(vec!["stop1".to_string()]),
            stream: false,
            tools: None,
            tool_choice: None,
            extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };

        let body = provider.build_anthropic_request(&request);
        
        assert_eq!(body["model"], "claude-3-opus-20240229");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["system"], "sys_prompt");
        assert_eq!(body["temperature"], 0.5);
        assert_eq!(body["top_p"], 0.5);
        assert!(body["stop_sequences"].is_array());
        
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "user_msg");
    }

    #[tokio::test]
    async fn test_fetch_models() {
        let mock_server = MockServer::start().await;
        
        let response_body = serde_json::json!({
            "data": [
                {"type": "model", "id": "claude-3-opus-20240229", "display_name": "Claude 3 Opus"}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let mut profile = anthropic_profile();
        profile.models_url = format!("{}/models", mock_server.uri());
        let provider = AnthropicProvider::new_with_profile(profile);
        
        let models = provider.fetch_models(None, 10.0).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0], "claude-3-opus-20240229");
    }
    #[tokio::test]
    async fn test_create_chat_completion() {
        let mock_server = MockServer::start().await;
        
        let response_body = serde_json::json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-opus-20240229",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 10,
                "output_tokens": 20
            },
            "content": [
                {
                    "type": "text",
                    "text": "Hello Anthropic!"
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let mut profile = anthropic_profile();
        profile.base_url = mock_server.uri();
        let provider = AnthropicProvider::new_with_profile(profile);
        
        let request = ChatCompletionRequest {
            model: "claude-3-opus-20240229".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Hi".to_string(),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: Some(1024),
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            tool_choice: None,
            extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };

        let response = provider.create_chat_completion(request).await.unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.content, "Hello Anthropic!");
        assert_eq!(response.usage.as_ref().unwrap().total_tokens, 30);
    }

    #[tokio::test]
    async fn test_create_chat_completion_stream() {
        let mock_server = MockServer::start().await;
        
        let sse_data = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-3\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":15}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(sse_data.as_bytes())
                    .insert_header("Content-Type", "text/event-stream")
            )
            .mount(&mock_server)
            .await;

        let mut profile = anthropic_profile();
        profile.base_url = mock_server.uri();
        let provider = AnthropicProvider::new_with_profile(profile);
        
        let request = ChatCompletionRequest {
            model: "claude-3-opus-20240229".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Hi".to_string(),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: Some(1024),
            top_p: None,
            stop: None,
            stream: true,
            tools: None,
            tool_choice: None,
            extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };

        let stream_res = provider.create_chat_completion_stream(request).await.unwrap();
        let mut stream = stream_res.response;
        
        use futures::StreamExt;
        
        let mut chunks = Vec::new();
        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.unwrap();
            if !chunk.choices.is_empty() {
                chunks.push(chunk);
            }
        }
        
        assert!(chunks.len() >= 3);
        
        // Chunk 1: message_start
        assert_eq!(chunks[0].choices[0].delta.role, Some(MessageRole::Assistant));
        
        // Chunk 2: content_block_delta
        assert_eq!(chunks[1].choices[0].delta.content.as_deref(), Some("Hello"));
        
        // Chunk 3: message_delta (stop reason)
        assert_eq!(chunks[2].choices[0].finish_reason.as_deref(), Some("end_turn"));
    }
    #[test]
    fn test_map_messages_with_tools() {
        let profile = anthropic_profile();
        let provider = AnthropicProvider::new_with_profile(profile);
        
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "".to_string(),
            name: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_123".to_string(),
                r#type: "function".to_string(),
                function: ToolFunction {
                    name: "get_weather".to_string(),
                    arguments: "{\"location\":\"SF\"}".to_string(),
                }
            }]),
            tool_call_id: None,
        };
        
        let tool_msg = ChatMessage {
            role: MessageRole::Tool,
            content: "Sunny".to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
        };

        let req = ChatCompletionRequest {
            model: "claude".into(),
            messages: vec![msg, tool_msg],
            temperature: None, max_tokens: Some(10), top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };
        
        let body = provider.build_anthropic_request(&req);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn test_create_chat_completion_with_tools_and_errors() {
        let mock_server = MockServer::start().await;
        
        let response_body = serde_json::json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "model": "claude-3",
            "stop_reason": "tool_use",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 10,
                "output_tokens": 20
            },
            "content": [
                {
                    "type": "tool_use",
                    "id": "call_123",
                    "name": "get_weather",
                    "input": {"location": "SF"}
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let mut profile = anthropic_profile();
        profile.base_url = mock_server.uri();
        let provider = AnthropicProvider::new_with_profile(profile);
        
        let request = ChatCompletionRequest {
            model: "claude-3-opus-20240229".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "Weather?".to_string(),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: Some(100),
            top_p: None,
            stop: None,
            stream: false,
            tools: Some(vec![crate::ToolDefinition {
                r#type: "function".to_string(),
                function: crate::ToolSchema {
                    name: "get_weather".to_string(),
                    description: Some("Get the weather".to_string()),
                    parameters: serde_json::json!({}),
                }
            }]),
            tool_choice: None,
            extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };

        let response = provider.create_chat_completion(request).await.unwrap();
        let tool_calls = response.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");
        
        // 500 error tests
        let error_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&error_server)
            .await;
            
        let mut err_profile = anthropic_profile();
        err_profile.base_url = error_server.uri();
        let err_provider = AnthropicProvider::new_with_profile(err_profile);
        let req2 = ChatCompletionRequest {
            model: "claude".to_string(),
            messages: vec![],
            temperature: None, max_tokens: Some(1), top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };
        assert!(err_provider.create_chat_completion(req2.clone()).await.is_err());
        assert!(err_provider.create_chat_completion_stream(req2).await.is_err());
    }

    #[test]
    fn test_build_anthropic_request_tool_choice() {
        let provider = AnthropicProvider::new();
        
        let mut request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![ChatMessage { role: MessageRole::User, content: "Hi".to_string(), name: None, tool_calls: None, tool_call_id: None }],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: Some(ToolChoice::Auto), extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };

        let body1 = provider.build_anthropic_request(&request);
        assert_eq!(body1["tool_choice"]["type"], "auto");

        request.tool_choice = Some(ToolChoice::Required);
        let body2 = provider.build_anthropic_request(&request);
        assert_eq!(body2["tool_choice"]["type"], "any");

        request.tool_choice = Some(ToolChoice::Specific("my_tool".to_string()));
        let body3 = provider.build_anthropic_request(&request);
        assert_eq!(body3["tool_choice"]["type"], "tool");
        assert_eq!(body3["tool_choice"]["name"], "my_tool");
    }

    #[test]
    fn test_build_anthropic_request_merge_messages() {
        let provider = AnthropicProvider::new();
        
        let request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![
                ChatMessage { role: MessageRole::User, content: "Hi 1".to_string(), name: None, tool_calls: None, tool_call_id: None },
                ChatMessage { role: MessageRole::User, content: "Hi 2".to_string(), name: None, tool_calls: None, tool_call_id: None },
                ChatMessage { role: MessageRole::Assistant, content: "Res 1".to_string(), name: None, tool_calls: None, tool_call_id: None },
                ChatMessage { role: MessageRole::Assistant, content: "Res 2".to_string(), name: None, tool_calls: None, tool_call_id: None },
            ],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };

        let body = provider.build_anthropic_request(&request);
        let msgs = body["messages"].as_array().unwrap();
        // Should merge into 1 user and 1 assistant
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "assistant");
        
        // user content should be array of length 2
        assert_eq!(msgs[0]["content"].as_array().unwrap().len(), 2);
        assert_eq!(msgs[1]["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_build_anthropic_request_extra_body() {
        let provider = AnthropicProvider::new();
        let mut extra_body = HashMap::new();
        extra_body.insert("custom_field".to_string(), serde_json::json!("custom_value"));
        
        let request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: None,
            extra_body,
            api_key_override: None, base_url_override: None,
        };

        let body = provider.build_anthropic_request(&request);
        assert_eq!(body["custom_field"], "custom_value");
    }

    #[tokio::test]
    async fn test_fetch_models_errors() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;
        let mut profile = anthropic_profile();
        profile.models_url = format!("{}/models", mock_server.uri());
        let provider = AnthropicProvider::new_with_profile(profile.clone());
        assert!(provider.fetch_models(None, 10.0).await.is_err());
        
        // test invalid json
        let mock_server2 = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid"))
            .mount(&mock_server2)
            .await;
        profile.models_url = format!("{}/models", mock_server2.uri());
        let provider2 = AnthropicProvider::new_with_profile(profile);
        assert!(provider2.fetch_models(None, 10.0).await.is_err());
    }

    #[tokio::test]
    async fn test_create_chat_completion_errors() {
        let error_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&error_server)
            .await;
        let mut profile = anthropic_profile();
        profile.base_url = error_server.uri();
        let err_provider = AnthropicProvider::new_with_profile(profile.clone());
        let req2 = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![ChatMessage { role: MessageRole::User, content: "Hi".to_string(), name: None, tool_calls: None, tool_call_id: None }],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };
        assert!(err_provider.create_chat_completion(req2.clone()).await.is_err());
        assert!(err_provider.create_chat_completion_stream(req2).await.is_err());
    }

    #[tokio::test]
    async fn test_create_chat_completion_stream_tool_use() {
        let mock_server = MockServer::start().await;
        
        let sse_data = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-3\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"get_weather\",\"input\":{}}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"loc\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"ation\\\":\\\"SF\\\"}\"}}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":15}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(sse_data.as_bytes())
                    .insert_header("Content-Type", "text/event-stream")
            )
            .mount(&mock_server)
            .await;

        let mut profile = anthropic_profile();
        profile.base_url = mock_server.uri();
        let provider = AnthropicProvider::new_with_profile(profile);
        
        let request = ChatCompletionRequest {
            model: "claude-3".to_string(),
            messages: vec![ChatMessage { role: MessageRole::User, content: "Hi".to_string(), name: None, tool_calls: None, tool_call_id: None }],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: true, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };

        let stream_res = provider.create_chat_completion_stream(request).await.unwrap();
        let mut stream = stream_res.response;
        
        use futures::StreamExt;
        
        let chunk1 = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk1.choices[0].delta.role, Some(MessageRole::Assistant));
        
        let chunk2 = stream.next().await.unwrap().unwrap();
        let tc1 = chunk2.choices[0].delta.tool_calls.as_ref().unwrap();
        assert_eq!(tc1[0].id.as_deref(), Some("toolu_1"));
        assert_eq!(tc1[0].function.as_ref().unwrap().name.as_deref(), Some("get_weather"));

        let chunk3 = stream.next().await.unwrap().unwrap();
        let tc2 = chunk3.choices[0].delta.tool_calls.as_ref().unwrap();
        assert_eq!(tc2[0].function.as_ref().unwrap().arguments.as_deref(), Some("{\"loc"));
        
        let chunk4 = stream.next().await.unwrap().unwrap();
        let tc3 = chunk4.choices[0].delta.tool_calls.as_ref().unwrap();
        assert_eq!(tc3[0].function.as_ref().unwrap().arguments.as_deref(), Some("ation\":\"SF\"}"));
    }

    #[tokio::test]
    async fn test_anthropic_network_errors() {
        let mut profile = anthropic_profile();
        profile.base_url = "http://127.0.0.1:0".to_string(); // Invalid URL, will cause reqwest execute to fail
        profile.models_url = "http://127.0.0.1:0/models".to_string();
        let provider = AnthropicProvider::new_with_profile(profile);
        
        assert!(provider.fetch_models(None, 10.0).await.is_err());
        
        let req = ChatCompletionRequest {
            model: "claude".to_string(),
            messages: vec![ChatMessage { role: MessageRole::User, content: "Hi".to_string(), name: None, tool_calls: None, tool_call_id: None }],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };
        assert!(provider.create_chat_completion(req.clone()).await.is_err());
        assert!(provider.create_chat_completion_stream(req).await.is_err());
    }

    #[tokio::test]
    async fn test_anthropic_build_empty_assistant() {
        let provider = AnthropicProvider::new();
        let req = ChatCompletionRequest {
            model: "claude".to_string(),
            messages: vec![ChatMessage { role: MessageRole::Assistant, content: "".to_string(), name: None, tool_calls: Some(vec![]), tool_call_id: None }],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };
        let body = provider.build_anthropic_request(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["content"][0]["type"], "text");
        assert_eq!(msgs[0]["content"][0]["text"], "");
    }
    
    #[tokio::test]
    async fn test_anthropic_merge_arrays() {
        let provider = AnthropicProvider::new();
        
        // This creates an array content in the first message by having a tool call
        let msg1 = ChatMessage {
            role: MessageRole::Assistant,
            content: "Text".to_string(),
            name: None,
            tool_calls: Some(vec![ToolCall {
                id: "1".to_string(), r#type: "function".to_string(),
                function: ToolFunction { name: "test".to_string(), arguments: "{}".to_string() }
            }]),
            tool_call_id: None,
        };
        // This merges another text into the assistant message
        let msg2 = ChatMessage { role: MessageRole::Assistant, content: "More text".to_string(), name: None, tool_calls: None, tool_call_id: None };
        
        let req = ChatCompletionRequest {
            model: "claude".to_string(),
            messages: vec![msg1, msg2],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };
        
        let body = provider.build_anthropic_request(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["content"].as_array().unwrap().len(), 3);
    }
    
    #[tokio::test]
    async fn test_anthropic_stream_invalid_json() {
        let mock_server = MockServer::start().await;
        let sse_data = concat!(
            "event: message_start\n",
            "data: {invalid json}\n\n",
            "data: [DONE]\n\n"
        );
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(sse_data.as_bytes()).insert_header("Content-Type", "text/event-stream"))
            .mount(&mock_server).await;
            
        let mut profile = anthropic_profile();
        profile.base_url = mock_server.uri();
        let provider = AnthropicProvider::new_with_profile(profile);
        let req = ChatCompletionRequest {
            model: "claude".to_string(),
            messages: vec![ChatMessage { role: MessageRole::User, content: "Hi".to_string(), name: None, tool_calls: None, tool_call_id: None }],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: true, tools: None, tool_choice: None, extra_body: HashMap::new(), api_key_override: None, base_url_override: None,
        };
        let mut stream = provider.create_chat_completion_stream(req).await.unwrap().response;
        use futures::StreamExt;
        assert!(stream.next().await.unwrap().is_err());
    }
}

// Rust guideline compliant 2026-02-21
