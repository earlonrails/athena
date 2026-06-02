use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use async_trait::async_trait;

/// API mode types supported by providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum ApiMode {
    /// Standard OpenAI-style chat completions
    #[default]
    ChatCompletions,
    /// Anthropic Messages API
    AnthropicMessages,
    /// Codex-style responses API
    CodexResponses,
    /// Custom/unknown API mode
    Custom(String),
}


/// Authentication types for providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum AuthType {
    /// API key authentication (Bearer token)
    #[default]
    ApiKey,
    /// OAuth device code flow
    OAuthDeviceCode,
    /// External OAuth
    OAuthExternal,
    /// GitHub Copilot authentication
    Copilot,
    /// AWS SDK authentication
    AwsSdk,
    /// No authentication required
    None,
}


/// Provider profile - declarative configuration for an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    /// Unique name of the provider
    pub name: String,
    
    /// API mode this provider uses
    #[serde(default)]
    pub api_mode: ApiMode,
    
    /// Alternative names for this provider
    #[serde(default)]
    pub aliases: Vec<String>,
    
    /// Human-readable display name
    #[serde(default)]
    pub display_name: String,
    
    /// Description shown in picker
    #[serde(default)]
    pub description: String,
    
    /// Signup URL for the provider
    #[serde(default)]
    pub signup_url: String,
    
    /// Environment variables for authentication
    #[serde(default)]
    pub env_vars: Vec<String>,
    
    /// Base URL for API requests
    #[serde(default)]
    pub base_url: String,
    
    /// Explicit models endpoint URL
    #[serde(default)]
    pub models_url: String,
    
    /// Authentication type
    #[serde(default)]
    pub auth_type: AuthType,
    
    /// Whether this provider supports health checks
    #[serde(default = "default_true")]
    pub supports_health_check: bool,
    
    /// Fallback models shown when live fetch fails
    #[serde(default)]
    pub fallback_models: Vec<String>,
    
    /// Base hostname for URL-to-provider reverse mapping
    #[serde(default)]
    pub hostname: String,
    
    /// Default headers to include in requests
    #[serde(default)]
    pub default_headers: HashMap<String, String>,
    
    /// Fixed temperature value (None = use caller's default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_temperature: Option<f32>,
    
    /// Default max tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_max_tokens: Option<u64>,
    
    /// Cheap model for auxiliary tasks (compression, vision, etc.)
    #[serde(default)]
    pub default_aux_model: String,
}

fn default_true() -> bool {
    true
}

impl ProviderProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            api_mode: ApiMode::default(),
            aliases: Vec::new(),
            display_name: String::new(),
            description: String::new(),
            signup_url: String::new(),
            env_vars: Vec::new(),
            base_url: String::new(),
            models_url: String::new(),
            auth_type: AuthType::default(),
            supports_health_check: true,
            fallback_models: Vec::new(),
            hostname: String::new(),
            default_headers: HashMap::new(),
            fixed_temperature: None,
            default_max_tokens: None,
            default_aux_model: String::new(),
        }
    }
    
    /// Get the hostname derived from base_url if not explicitly set
    pub fn get_hostname(&self) -> String {
        if !self.hostname.is_empty() {
            return self.hostname.clone();
        }
        if !self.base_url.is_empty() {
            if let Ok(url) = url::Url::parse(&self.base_url) {
                return url.host().map(|h| h.to_string()).unwrap_or_default();
            }
        }
        String::new()
    }
    
    /// Prepare messages before sending to the API
    pub fn prepare_messages(&self, messages: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
        messages
    }
    
    /// Build extra body fields for the API request
    pub fn build_extra_body(
        &self,
        _session_id: Option<&str>,
        _context: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
    
    /// Build provider-specific API kwargs
    pub fn build_api_kwargs_extras(
        &self,
        _reasoning_config: Option<&HashMap<String, serde_json::Value>>,
        _context: &HashMap<String, serde_json::Value>,
    ) -> (HashMap<String, serde_json::Value>, HashMap<String, serde_json::Value>) {
        (HashMap::new(), HashMap::new())
    }
}

/// Trait for LLM providers - async interface for making requests
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Get the provider profile
    fn profile(&self) -> &ProviderProfile;
    
    /// Fetch the list of available models from the provider
    async fn fetch_models(
        &self,
        api_key: Option<&str>,
        timeout: f64,
    ) -> Result<Vec<String>, crate::ProviderError>;
    
    /// Create a chat completion request
    async fn create_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, crate::ProviderError>;
    
    /// Create a streaming chat completion request
    async fn create_chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionStream, crate::ProviderError>;
}

/// Request for chat completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u64>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub stream: bool,
    pub tools: Option<Vec<ToolDefinition>>,
    pub tool_choice: Option<ToolChoice>,
    pub extra_body: HashMap<String, serde_json::Value>,
    pub api_key_override: Option<String>,
    pub base_url_override: Option<String>,
}

/// Message in a chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub name: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

/// Role of a message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Tool call from the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolFunction,
}

/// Function definition for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,
}

/// Tool definition schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: ToolSchema,
}

/// Schema for a tool function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

/// Tool choice setting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolChoice {
    None,
    Auto,
    Required,
    Specific(String),
}

/// Response from chat completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub created: u64,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

/// A single choice in the response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Streaming response from chat completion
pub struct ChatCompletionStream {
    pub response: Box<dyn futures::Stream<Item = Result<StreamChunk, crate::ProviderError>> + Send + Unpin>,
}

/// A chunk in the streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub model: String,
    pub created: Option<u64>,
    pub choices: Vec<StreamChoice>,
}

/// A choice in a stream chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: usize,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

/// Delta for streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDelta {
    pub role: Option<MessageRole>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

/// Tool call in streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToolCall {
    pub index: Option<i32>,
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub function: Option<StreamToolFunction>,
}

/// Function in streaming tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToolFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// Sentinel value for omitting temperature
pub const OMIT_TEMPERATURE: Option<f32> = None;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_mode_default() {
        assert_eq!(ApiMode::default(), ApiMode::ChatCompletions);
    }

    #[test]
    fn test_auth_type_default() {
        assert_eq!(AuthType::default(), AuthType::ApiKey);
    }

    #[test]
    fn test_provider_profile_new() {
        let profile = ProviderProfile::new("test_provider");
        assert_eq!(profile.name, "test_provider");
        assert_eq!(profile.api_mode, ApiMode::ChatCompletions);
        assert_eq!(profile.auth_type, AuthType::ApiKey);
        assert!(profile.supports_health_check);
        assert!(profile.aliases.is_empty());
    }

    #[test]
    fn test_provider_profile_get_hostname() {
        let mut profile = ProviderProfile::new("test");
        
        // 1. Should use explicit hostname if provided
        profile.hostname = "explicit.api.com".to_string();
        profile.base_url = "https://fallback.com/api".to_string();
        assert_eq!(profile.get_hostname(), "explicit.api.com");
        
        // 2. Should derive from base_url if hostname is empty
        profile.hostname = String::new();
        assert_eq!(profile.get_hostname(), "fallback.com");

        // 3. Should return empty if URL is invalid
        profile.base_url = "not a url".to_string();
        assert_eq!(profile.get_hostname(), "");

        // 4. Should return empty if both are empty
        profile.base_url = String::new();
        assert_eq!(profile.get_hostname(), "");
    }

    #[test]
    fn test_provider_profile_prepare_messages() {
        let profile = ProviderProfile::new("test");
        let messages = vec![serde_json::json!({"role": "user", "content": "hi"})];
        let prepared = profile.prepare_messages(messages.clone());
        assert_eq!(prepared, messages);
    }

    #[test]
    fn test_provider_profile_build_extra_body() {
        let profile = ProviderProfile::new("test");
        let context = HashMap::new();
        let body = profile.build_extra_body(Some("session_123"), &context);
        assert!(body.is_empty());
    }

    #[test]
    fn test_provider_profile_build_api_kwargs_extras() {
        let profile = ProviderProfile::new("test");
        let context = HashMap::new();
        let (kwargs, headers) = profile.build_api_kwargs_extras(None, &context);
        assert!(kwargs.is_empty());
        assert!(headers.is_empty());
    }

    #[test]
    fn test_default_true() {
        assert!(default_true());
    }
}

// Rust guideline compliant 2026-02-21
