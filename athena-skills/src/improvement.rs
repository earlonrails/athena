use anyhow::Result;
use athena_providers::{ChatCompletionRequest, ChatMessage, LLMProvider, MessageRole};
use std::sync::Arc;
use tracing::info;

use crate::{SkillManager, SkillStore};

pub struct SkillImprover;

impl SkillImprover {
    pub async fn run_improvement_pass(
        store: Arc<SkillStore>,
        manager: Arc<SkillManager>,
        provider: Arc<dyn LLMProvider>,
        model_name: &str,
        api_key_override: Option<String>,
        base_url_override: Option<String>,
    ) -> Result<usize> {
        let skills = store.get_all_skills()?;
        let mut improved_count = 0;

        for mut skill in skills {
            if skill.usage_count >= 5 {
                let success_rate = skill.success_count as f32 / skill.usage_count as f32;
                
                if success_rate < 0.4 {
                    info!(
                        "Skill '{}' ({}) has low success rate ({:.0}%). Triggering self-improvement.",
                        skill.name, skill.id, success_rate * 100.0
                    );

                    let prompt_content = format!(
                        "The following skill has a low success rate ({:.0}% across {} uses).\n\
                        Please rewrite the instructions (body) to be more accurate, robust, and helpful.\n\
                        Retain the original name and description unless they are fundamentally flawed.\n\
                        \n\
                        CURRENT SKILL:\n\
                        Name: {}\n\
                        Description: {}\n\
                        Body: {}\n\n\
                        Your output MUST be a JSON object with exactly these fields:\n\
                        {{\n\
                          \"name\": \"A short, descriptive name\",\n\
                          \"description\": \"A 1-2 sentence description\",\n\
                          \"body\": \"The improved, detailed markdown instructions\"\n\
                        }}",
                        success_rate * 100.0, skill.usage_count, skill.name, skill.description, skill.instructions
                    );

                    let profile = provider.profile();
                    let effective_model = if profile.fallback_models.iter().any(|m| m == model_name) || (!profile.name.is_empty() && model_name.contains(&profile.name)) {
                        model_name.to_string()
                    } else if !profile.default_aux_model.is_empty() {
                        profile.default_aux_model.clone()
                    } else if let Some(first) = profile.fallback_models.first() {
                        first.clone()
                    } else {
                        model_name.to_string()
                    };

                    let req = ChatCompletionRequest {
                        model: effective_model,
                        messages: vec![ChatMessage {
                            role: MessageRole::User,
                            content: prompt_content,
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                        }],
                        temperature: Some(0.3),
                        max_tokens: Some(2000),
                        top_p: None,
                        stop: None,
                        stream: false,
                        tools: None,
                        tool_choice: None,
                        extra_body: Default::default(),
                        api_key_override: api_key_override.clone(),
                        base_url_override: base_url_override.clone(),
                    };

                    match provider.create_chat_completion(req).await {
                        Ok(response) => {
                            if let Some(content) = response.choices.first().map(|c| c.message.content.clone()) {
                                let clean_content = content.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(clean_content) {
                                    if let (Some(name), Some(desc), Some(body)) = (
                                        parsed["name"].as_str(),
                                        parsed["description"].as_str(),
                                        parsed["body"].as_str(),
                                    ) {
                                        skill.name = name.to_string();
                                        skill.description = desc.to_string();
                                        skill.instructions = body.to_string();
                                        
                                        // Reset usage stats to evaluate new version fairly
                                        skill.usage_count = 0;
                                        skill.success_count = 0;
                                        
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs() as i64;
                                        skill.updated_at = now;

                                        // Re-embed and save
                                        if let Ok(embedding) = manager.embed_text(&skill.description) {
                                            if let Err(e) = store.insert_skill(&skill, &embedding) {
                                                tracing::error!("Failed to save improved skill: {}", e);
                                            } else {
                                                improved_count += 1;
                                                info!("Successfully improved skill '{}'", skill.name);
                                            }
                                        }
                                    }
                                } else {
                                    tracing::warn!("Failed to parse JSON from improvement response");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("LLM call failed during skill improvement: {}", e);
                        }
                    }
                }
            }
        }

        Ok(improved_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use athena_providers::{Choice, ChatCompletionResponse, ProviderError, ProviderProfile};

    struct TestMockProvider {
        profile: ProviderProfile,
    }

    #[async_trait]
    impl LLMProvider for TestMockProvider {
        fn profile(&self) -> &ProviderProfile {
            &self.profile
        }

        async fn fetch_models(&self, _api_key: Option<&str>, _timeout: f64) -> Result<Vec<String>, ProviderError> {
            Ok(vec![])
        }

        async fn create_chat_completion(
            &self,
            _request: ChatCompletionRequest,
        ) -> Result<ChatCompletionResponse, ProviderError> {
            let json_body = serde_json::json!({
                "name": "Improved Skill",
                "description": "Refined description",
                "body": "Better, updated instructions"
            }).to_string();

            Ok(ChatCompletionResponse {
                id: "test".into(),
                model: "mock".into(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage {
                        role: MessageRole::Assistant,
                        content: json_body,
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
            _request: ChatCompletionRequest,
        ) -> Result<athena_providers::ChatCompletionStream, ProviderError> {
            Err(ProviderError::ApiRequestFailed("unused".into()))
        }
    }

    #[tokio::test]
    async fn test_skill_improvement_pass() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("skills_improve.db");
        let store = Arc::new(SkillStore::new(&db_path).unwrap());
        let manager = Arc::new(SkillManager::new(&db_path).unwrap());

        // Skill 1: low usage (usage 2) -> should NOT be improved
        let skill1 = crate::Skill {
            id: "s1".into(),
            name: "Skill 1".into(),
            description: "Desc 1".into(),
            instructions: "Body 1".into(),
            usage_count: 2,
            success_count: 0,
            created_at: 10,
            updated_at: 10,
        };
        let emb1 = manager.embed_text(&skill1.description).unwrap();
        store.insert_skill(&skill1, &emb1).unwrap();

        // Skill 2: high usage (6) and low success rate (1/6 = 16.7%) -> should BE improved
        let skill2 = crate::Skill {
            id: "s2".into(),
            name: "Skill 2".into(),
            description: "Desc 2".into(),
            instructions: "Body 2".into(),
            usage_count: 6,
            success_count: 1,
            created_at: 10,
            updated_at: 10,
        };
        let emb2 = manager.embed_text(&skill2.description).unwrap();
        store.insert_skill(&skill2, &emb2).unwrap();

        let provider = Arc::new(TestMockProvider {
            profile: ProviderProfile::new("mock"),
        });

        let improved = SkillImprover::run_improvement_pass(
            store.clone(),
            manager.clone(),
            provider,
            "mock-model",
            None,
            None,
        ).await.unwrap();

        assert_eq!(improved, 1);

        let all_skills = store.get_all_skills().unwrap();
        let s2_after = all_skills.into_iter().find(|s| s.id == "s2").unwrap();
        assert_eq!(s2_after.name, "Improved Skill");
        assert_eq!(s2_after.description, "Refined description");
        assert_eq!(s2_after.instructions, "Better, updated instructions");
        assert_eq!(s2_after.usage_count, 0);
        assert_eq!(s2_after.success_count, 0);
    }
}

// Rust guideline compliant 2026-02-21
