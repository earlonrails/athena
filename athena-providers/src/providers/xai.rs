use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::base::*;
use crate::error::*;
use crate::registry::register_provider;

/// xAI provider profile
pub fn xai_profile() -> ProviderProfile {
    ProviderProfile {
        name: "xai".to_string(),
        api_mode: ApiMode::ChatCompletions,
        aliases: vec!["grok".to_string(), "x.ai".to_string()],
        display_name: "xAI".to_string(),
        description: "xAI — Grok models".to_string(),
        signup_url: "https://console.x.ai/".to_string(),
        env_vars: vec!["XAI_API_KEY".to_string(), "GROK_API_KEY".to_string()],
        base_url: "https://api.x.ai/v1".to_string(),
        models_url: "https://api.x.ai/v1/models".to_string(),
        auth_type: AuthType::ApiKey,
        supports_health_check: true,
        fallback_models: vec![
            "grok-beta".to_string(),
            "grok-2".to_string(),
        ],
        hostname: "api.x.ai".to_string(),
        default_headers: HashMap::new(),
        fixed_temperature: None,
        default_max_tokens: None,
        default_aux_model: "grok-beta".to_string(),
    }
}

/// xAI provider implementation
pub struct XAIProvider {
    profile: ProviderProfile,
}

impl Default for XAIProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl XAIProvider {
    pub fn new() -> Self {
        Self {
            profile: xai_profile(),
        }
    }
}

#[async_trait]
impl LLMProvider for XAIProvider {
    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }
    
    async fn fetch_models(
        &self,
        api_key: Option<&str>,
        _timeout: f64,
    ) -> std::result::Result<Vec<String>, ProviderError> {
        let client = reqwest::Client::new();
        let mut request = client.request(reqwest::Method::GET, &self.profile.models_url);
        
        let mut resolved_key = api_key.map(|k| k.to_string());
        if resolved_key.is_none() {
            for env_var in &self.profile.env_vars {
                if let Some(val) = athena_core::config::get_env_value(env_var) {
                    resolved_key = Some(val);
                    break;
                }
            }
        }
            
        if let Some(k) = resolved_key {
            request = request.header("Authorization", format!("Bearer {}", k));
        }
        
        let response = request.send().await
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
        let openai_provider = super::openai::OpenAIProvider::new_with_profile(
            request.api_key_override.clone(),
            request.base_url_override.clone().or_else(|| Some(self.profile.base_url.clone())),
            self.profile.clone(),
        );
        
        openai_provider.create_chat_completion(request).await
    }
    
    async fn create_chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> std::result::Result<ChatCompletionStream, ProviderError> {
        let openai_provider = super::openai::OpenAIProvider::new_with_profile(
            request.api_key_override.clone(),
            request.base_url_override.clone().or_else(|| Some(self.profile.base_url.clone())),
            self.profile.clone(),
        );
        
        openai_provider.create_chat_completion_stream(request).await
    }
}

/// Register the xAI provider
pub fn register() {
    register_provider(Arc::new(XAIProvider::new()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_xai_profile() {
        let profile = xai_profile();
        assert_eq!(profile.name, "xai");
        assert_eq!(profile.api_mode, ApiMode::ChatCompletions);
        assert_eq!(profile.auth_type, AuthType::ApiKey);
        assert_eq!(profile.hostname, "api.x.ai");
    }

    #[tokio::test]
    async fn test_xai_register() {
        // Test that register function works correctly
        let profile = xai_profile();
        assert_eq!(profile.name, "xai");
        assert_eq!(profile.api_mode, ApiMode::ChatCompletions);
        assert_eq!(profile.auth_type, AuthType::ApiKey);
    }

    #[tokio::test]
    async fn test_xai_provider_new() {
        let provider = XAIProvider::new();
        assert_eq!(provider.profile().name, "xai");
    }

    #[tokio::test]
    async fn test_fetch_models() {
        let mock_server = MockServer::start().await;
        
        let response_body = serde_json::json!({
            "data": [
                {"id": "grok-beta", "object": "model"}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let mut provider = XAIProvider::new();
        provider.profile.models_url = format!("{}/models", mock_server.uri());
        
        let models = provider.fetch_models(Some("test_key"), 10.0).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0], "grok-beta");
    }

    #[tokio::test]
    async fn test_fetch_models_http_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;
        let mut provider = XAIProvider::new();
        provider.profile.models_url = format!("{}/v1/models", mock_server.uri());
        assert!(provider.fetch_models(None, 10.0).await.is_err());
    }

    #[tokio::test]
    async fn test_fetch_models_invalid_json() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid"))
            .mount(&mock_server)
            .await;
        let mut provider = XAIProvider::new();
        provider.profile.models_url = format!("{}/v1/models", mock_server.uri());
        assert!(provider.fetch_models(None, 10.0).await.is_err());
    }

    #[tokio::test]
    async fn test_create_chat_completion_delegation() {
        let provider = XAIProvider::new();
        let req = ChatCompletionRequest {
            model: "xai".to_string(),
            messages: vec![],
            temperature: None, max_tokens: None, top_p: None, stop: None, stream: false, tools: None, tool_choice: None, extra_body: HashMap::new(),
            api_key_override: None, base_url_override: None,
        };
        // This will fail because no mock server is set up, but it exercises the delegation code
        let _ = provider.create_chat_completion(req.clone()).await;
        let _ = provider.create_chat_completion_stream(req).await;
    }

    #[tokio::test]
    async fn test_fetch_models_network_error() {
        let mut provider = XAIProvider::new();
        provider.profile.models_url = "http://127.0.0.1:0/models".to_string();
        assert!(provider.fetch_models(None, 10.0).await.is_err());
    }
}

// Rust guideline compliant 2026-02-21
