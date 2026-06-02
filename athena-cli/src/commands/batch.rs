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

// Rust guideline compliant 2026-02-21
