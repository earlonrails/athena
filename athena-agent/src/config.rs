use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub provider: Option<String>,
    pub api_mode: Option<String>,
    pub model: String,
    pub max_tokens: Option<usize>,
    pub max_iterations: usize,
    pub tool_delay_ms: u64,
    pub enabled_toolsets: Vec<String>,
    pub disabled_toolsets: Vec<String>,
    pub save_trajectories: bool,
    pub verbose_logging: bool,
    pub quiet_mode: bool,
    pub platform: Option<String>,
    // Many more to come...
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: None,
            provider: None,
            api_mode: None,
            model: "anthropic/claude-opus-4.6".to_string(),
            max_tokens: None,
            max_iterations: 90,
            tool_delay_ms: 1000,
            enabled_toolsets: vec![],
            disabled_toolsets: vec![],
            save_trajectories: false,
            verbose_logging: false,
            quiet_mode: false,
            platform: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.base_url, None);
        assert_eq!(config.api_key, None);
        assert_eq!(config.provider, None);
        assert_eq!(config.api_mode, None);
        assert_eq!(config.model, "anthropic/claude-opus-4.6");
        assert_eq!(config.max_tokens, None);
        assert_eq!(config.max_iterations, 90);
        assert_eq!(config.tool_delay_ms, 1000);
        assert!(config.enabled_toolsets.is_empty());
        assert!(config.disabled_toolsets.is_empty());
        assert!(!config.save_trajectories);
        assert!(!config.verbose_logging);
        assert!(!config.quiet_mode);
        assert_eq!(config.platform, None);
    }
}

// Rust guideline compliant 2026-02-21
