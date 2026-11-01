use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::base::*;
use crate::error::*;
use crate::registry::register_provider;

/// Groq provider profile
pub fn groq_profile() -> ProviderProfile {
    ProviderProfile {
        name: "groq".to_string(),
        api_mode: ApiMode::ChatCompletions,
        aliases: vec!["groqcloud".to_string()],
        display_name: "Groq".to_string(),
        description: "Groq — Ultra-fast LPU inference".to_string(),
        signup_url: "https://console.groq.com/keys".to_string(),
        env_vars: vec!["GROQ_API_KEY".to_string()],
        base_url: "https://api.groq.com/openai/v1".to_string(),
        models_url: "https://api.groq.com/openai/v1/models".to_string(),
        auth_type: AuthType::ApiKey,
        supports_health_check: true,
        fallback_models: vec![
            "llama-3.3-70b-versatile".to_string(),
            "llama-3.1-8b-instant".to_string(),
            "mixtral-8x7b-32768".to_string(),
        ],
        hostname: "api.groq.com".to_string(),
        default_headers: HashMap::new(),
        fixed_temperature: None,
        default_max_tokens: None,
        default_aux_model: "llama-3.1-8b-instant".to_string(),
    }
}

/// Groq provider implementation
pub struct GroqProvider {
    profile: ProviderProfile,
}

impl Default for GroqProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GroqProvider {
    pub fn new() -> Self {
        Self {
            profile: groq_profile(),
        }
    }

    pub fn new_with_profile(profile: ProviderProfile) -> Self {
        Self { profile }
    }
}

#[async_trait]
impl LLMProvider for GroqProvider {
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

/// Register the Groq provider
pub fn register() {
    register_provider(Arc::new(GroqProvider::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_profile() {
        let profile = groq_profile();
        assert_eq!(profile.name, "groq");
        assert_eq!(profile.base_url, "https://api.groq.com/openai/v1");
        assert!(profile.env_vars.contains(&"GROQ_API_KEY".to_string()));
    }

    #[test]
    fn test_groq_provider_creation() {
        let provider = GroqProvider::new();
        assert_eq!(provider.profile().name, "groq");
    }
}

// Rust guideline compliant 2026-02-21

