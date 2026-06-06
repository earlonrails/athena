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

                    let req = ChatCompletionRequest {
                        model: model_name.to_string(),
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
                        api_key_override: None,
                        base_url_override: None,
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
