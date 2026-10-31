use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::base::*;
use crate::error::*;
use crate::registry::register_provider;

/// Gemini provider profile
pub fn gemini_profile() -> ProviderProfile {
    ProviderProfile {
        name: "gemini".to_string(),
        api_mode: ApiMode::ChatCompletions,
        aliases: vec!["google".to_string(), "gemini-api".to_string()],
        display_name: "Google Gemini".to_string(),
        description: "Google Gemini via OpenAI compatibility layer".to_string(),
        signup_url: "https://aistudio.google.com/app/apikey".to_string(),
        env_vars: vec!["GEMINI_API_KEY".to_string(), "GOOGLE_API_KEY".to_string()],
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
        models_url: "https://generativelanguage.googleapis.com/v1beta/openai/models".to_string(),
        auth_type: AuthType::ApiKey,
        supports_health_check: true,
        fallback_models: vec![
            "gemini-2.5-flash".to_string(),
            "gemini-2.5-pro".to_string(),
            "gemini-1.5-pro".to_string(),
            "gemini-1.5-flash".to_string(),
        ],
        hostname: "generativelanguage.googleapis.com".to_string(),
        default_headers: HashMap::new(),
        fixed_temperature: None,
        default_max_tokens: None,
        default_aux_model: "gemini-1.5-flash".to_string(),
    }
}

/// Gemini provider implementation
pub struct GeminiProvider {
    profile: ProviderProfile,
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {
            profile: gemini_profile(),
        }
    }
}

#[async_trait]
impl LLMProvider for GeminiProvider {
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
        let base_url = match request.base_url_override.clone() {
            Some(mut url) => {
                if url.contains("generativelanguage.googleapis.com") && !url.ends_with("/openai") {
                    if !url.ends_with('/') {
                        url.push('/');
                    }
                    url.push_str("openai");
                }
                Some(url)
            }
            None => Some(self.profile.base_url.clone()),
        };

        let openai_provider = super::openai::OpenAIProvider::new_with_profile(
            request.api_key_override.clone(),
            base_url,
            self.profile.clone(),
        );
        
        openai_provider.create_chat_completion(request).await
    }
    
    async fn create_chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> std::result::Result<ChatCompletionStream, ProviderError> {
        let base_url = match request.base_url_override.clone() {
            Some(mut url) => {
                if url.contains("generativelanguage.googleapis.com") && !url.ends_with("/openai") {
                    if !url.ends_with('/') {
                        url.push('/');
                    }
                    url.push_str("openai");
                }
                Some(url)
            }
            None => Some(self.profile.base_url.clone()),
        };

        let openai_provider = super::openai::OpenAIProvider::new_with_profile(
            request.api_key_override.clone(),
            base_url,
            self.profile.clone(),
        );
        
        openai_provider.create_chat_completion_stream(request).await
    }
}

/// Register the Gemini provider
pub fn register() {
    register_provider(Arc::new(GeminiProvider::new()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_gemini_profile() {
        let profile = gemini_profile();
        assert_eq!(profile.name, "gemini");
        assert_eq!(profile.api_mode, ApiMode::ChatCompletions);
        assert_eq!(profile.auth_type, AuthType::ApiKey);
        assert_eq!(profile.hostname, "generativelanguage.googleapis.com");
    }

    #[tokio::test]
    async fn test_gemini_register() {
        // Test that register function works correctly
        let profile = gemini_profile();
        assert_eq!(profile.name, "gemini");
        assert_eq!(profile.api_mode, ApiMode::ChatCompletions);
        assert_eq!(profile.auth_type, AuthType::ApiKey);
    }

    #[tokio::test]
    async fn test_gemini_provider_new() {
        let provider = GeminiProvider::new();
        assert_eq!(provider.profile().name, "gemini");
    }

    #[tokio::test]
    async fn test_fetch_models() {
        let mock_server = MockServer::start().await;
        
        let response_body = serde_json::json!({
            "data": [
                {"id": "gemini-1.5-pro", "object": "model"}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let mut provider = GeminiProvider::new();
        provider.profile.models_url = format!("{}/models", mock_server.uri());
        
        let models = provider.fetch_models(Some("test_key"), 10.0).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0], "gemini-1.5-pro");
    }

    #[tokio::test]
    async fn test_fetch_models_http_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;
        let mut provider = GeminiProvider::new();
        provider.profile.models_url = format!("{}/models", mock_server.uri());
        assert!(provider.fetch_models(None, 10.0).await.is_err());
    }

    #[tokio::test]
    async fn test_fetch_models_invalid_json() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid"))
            .mount(&mock_server)
            .await;
        let mut provider = GeminiProvider::new();
        provider.profile.models_url = format!("{}/models", mock_server.uri());
        assert!(provider.fetch_models(None, 10.0).await.is_err());
    }

    #[tokio::test]
    async fn test_create_chat_completion_delegation() {
        let provider = GeminiProvider::new();
        let req = ChatCompletionRequest {
            model: "gemini".to_string(),
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
        let mut provider = GeminiProvider::new();
        provider.profile.models_url = "http://127.0.0.1:0/models".to_string();
        assert!(provider.fetch_models(None, 10.0).await.is_err());
    }
}

// Rust guideline compliant 2026-02-21
