use crate::{AgentConfig, AIAgent, IterationBudget};

pub struct AIAgentBuilder {
    config: AgentConfig,
    budget: Option<IterationBudget>,
    skill_store: Option<std::sync::Arc<athena_skills::SkillStore>>,
    skill_manager: Option<std::sync::Arc<athena_skills::SkillManager>>,
}

impl AIAgentBuilder {
    pub fn new() -> Self {
        Self {
            config: AgentConfig::default(),
            budget: None,
            skill_store: None,
            skill_manager: None,
        }
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.model = model.into();
        self
    }

    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.config.base_url = Some(base_url.into());
        self
    }

    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.config.api_key = Some(key.into());
        self
    }

    pub fn max_iterations(mut self, iters: usize) -> Self {
        self.config.max_iterations = iters;
        self
    }

    pub fn budget(mut self, budget: IterationBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    pub fn skills(mut self, store: std::sync::Arc<athena_skills::SkillStore>, manager: std::sync::Arc<athena_skills::SkillManager>) -> Self {
        self.skill_store = Some(store);
        self.skill_manager = Some(manager);
        self
    }

    pub fn build(self) -> AIAgent {
        let budget = self.budget.unwrap_or_else(|| IterationBudget::new(self.config.max_iterations));
        AIAgent {
            config: self.config,
            budget,
            skill_store: self.skill_store,
            skill_manager: self.skill_manager,
        }
    }
}

impl Default for AIAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let builder = AIAgentBuilder::default();
        let agent = builder.build();
        assert_eq!(agent.config.model, "anthropic/claude-opus-4.6");
    }

    #[test]
    fn test_builder_chaining() {
        let budget = IterationBudget::new(50);
        let agent = AIAgentBuilder::new()
            .model("gpt-4o")
            .base_url("http://localhost:8080")
            .api_key("test-key")
            .max_iterations(100)
            .budget(budget)
            .build();

        assert_eq!(agent.config.model, "gpt-4o");
        assert_eq!(agent.config.base_url, Some("http://localhost:8080".to_string()));
        assert_eq!(agent.config.api_key, Some("test-key".to_string()));
        assert_eq!(agent.config.max_iterations, 100);
        assert_eq!(agent.budget.remaining(), 50);
    }
}

// Rust guideline compliant 2026-02-21
