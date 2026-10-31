use anyhow::Result;
use athena_agent::AIAgent;
use athena_tools::ToolRegistry;
use std::sync::Arc;
use athena_providers::LLMProvider;
use serde::Deserialize;
use std::fs;
use tracing::info;

#[derive(Deserialize)]
struct BatchConfig {
    prompts: Vec<String>,
}

pub async fn run_batch(
    config_path: String,
    mut agent: AIAgent,
    registry: ToolRegistry,
    provider: Arc<dyn LLMProvider + Send + Sync>,
) -> Result<()> {
    let content = fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read batch config file: {}", e))?;
        
    let config: BatchConfig = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse batch config YAML: {}", e))?;
        
    info!("Starting batch execution of {} prompts...", config.prompts.len());
    
    for (i, prompt) in config.prompts.iter().enumerate() {
        println!("========================================");
        println!("Batch {}/{}: {}", i + 1, config.prompts.len(), prompt);
        println!("========================================");
        
        match agent.run_conversation(prompt, None, &registry, provider.clone()).await {
            Ok(res) => println!("Result:\n{}\n", res),
            Err(e) => eprintln!("Error running prompt: {}\n", e),
        }
    }
    
    info!("Batch execution completed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBatchProvider;

    #[athena_providers::async_trait]
    impl LLMProvider for MockBatchProvider {
        fn profile(&self) -> &athena_providers::ProviderProfile {
            static PROFILE: std::sync::OnceLock<athena_providers::ProviderProfile> = std::sync::OnceLock::new();
            PROFILE.get_or_init(|| athena_providers::ProviderProfile::new("mock"))
        }

        async fn fetch_models(&self, _api_key: Option<&str>, _timeout: f64) -> Result<Vec<String>, athena_providers::ProviderError> {
            Ok(vec![])
        }

        async fn create_chat_completion(
            &self,
            _request: athena_providers::ChatCompletionRequest,
        ) -> Result<athena_providers::ChatCompletionResponse, athena_providers::ProviderError> {
            Ok(athena_providers::ChatCompletionResponse {
                id: "1".into(),
                model: "mock".into(),
                choices: vec![athena_providers::Choice {
                    index: 0,
                    message: athena_providers::ChatMessage {
                        role: athena_providers::MessageRole::Assistant,
                        content: "Batch prompt response".into(),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    finish_reason: Some("stop".into()),
                }],
                usage: None,
                created: 0,
            })
        }

        async fn create_chat_completion_stream(
            &self,
            _request: athena_providers::ChatCompletionRequest,
        ) -> Result<athena_providers::ChatCompletionStream, athena_providers::ProviderError> {
            Err(athena_providers::ProviderError::ApiRequestFailed("unused".into()))
        }
    }

    #[tokio::test]
    async fn test_run_batch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let batch_file = temp_dir.path().join("batch.yaml");
        fs::write(&batch_file, "prompts:\n  - \"Prompt 1\"\n  - \"Prompt 2\"\n").unwrap();

        let agent = AIAgent::builder().model("mock".to_string()).build();
        let registry = ToolRegistry::new();
        let provider = Arc::new(MockBatchProvider);

        let res = run_batch(batch_file.to_str().unwrap().to_string(), agent, registry, provider).await;
        assert!(res.is_ok());
    }
}

// Rust guideline compliant 2026-02-21
