use anyhow::{Context, Result};
use athena_providers::{ChatCompletionRequest, ChatMessage, LLMProvider, MessageRole};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{Skill, SkillManager, SkillStore};

pub struct SkillSynthesizer;

impl SkillSynthesizer {
    pub async fn synthesize(
        history: Vec<ChatMessage>,
        provider: Arc<dyn LLMProvider>,
        model_name: &str,
        store: Arc<SkillStore>,
        manager: Arc<SkillManager>,
        dedup_threshold: f32,
    ) -> Result<Option<Skill>> {
        // 1. Build the synthesis prompt
        let mut prompt_content = String::from(
            "You are an AI tasked with synthesizing a reusable skill from a successful conversation.\n\
            Review the following conversation history where an agent successfully completed a task.\n\
            Identify the core pattern, tools used, and steps taken.\n\
            Distill this into a generic, reusable skill.\n\n\
            Your output MUST be a JSON object with exactly these fields:\n\
            {\n\
              \"name\": \"A short, descriptive name (e.g. Find Process Port)\",\n\
              \"description\": \"A 1-2 sentence description of what the skill does\",\n\
              \"body\": \"The detailed markdown instructions for how to execute the skill\"\n\
            }\n\n\
            CONVERSATION HISTORY:\n"
        );

        for msg in &history {
            match msg.role {
                MessageRole::User => {
                    prompt_content.push_str(&format!("\nUSER: {}\n", msg.content));
                }
                MessageRole::Assistant => {
                    if !msg.content.is_empty() {
                        prompt_content.push_str(&format!("\nASSISTANT: {}\n", msg.content));
                    }
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            prompt_content.push_str(&format!("ASSISTANT CALLED TOOL: {} with {}\n", tc.function.name, tc.function.arguments));
                        }
                    }
                }
                MessageRole::Tool => {
                    prompt_content.push_str(&format!("\nTOOL RESULT ({}): {}\n", msg.name.as_deref().unwrap_or(""), msg.content));
                }
                _ => {}
            }
        }

        let req = ChatCompletionRequest {
            model: model_name.to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: prompt_content,
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: Some(0.2),
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

        // 2. Call active provider
        info!("Calling LLM for skill synthesis...");
        let response = provider.create_chat_completion(req).await?;
        let content = response.choices.first()
            .map(|c| c.message.content.clone())
            .context("No content in LLM response")?;

        // 3. Parse JSON response
        // Sometimes models wrap json in markdown block
        let clean_content = content.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
        let parsed: serde_json::Value = serde_json::from_str(clean_content)
            .context(format!("Failed to parse JSON from synthesis response. Raw: {}", content))?;

        let name = parsed["name"].as_str().context("Missing name")?.to_string();
        let description = parsed["description"].as_str().context("Missing description")?.to_string();
        let body = parsed["body"].as_str().context("Missing body")?.to_string();

        info!("Synthesized candidate skill: '{}'", name);

        // 4. Dedup check: embed description and compare cosine similarity
        let embedding = manager.embed_text(&description)?;
        let top_k = manager.search_with_scores(&embedding, 1)?;

        if let Some((score, skill)) = top_k.into_iter().next() {
            if score > dedup_threshold {
                info!("Skill '{}' rejected: Too similar to existing skill '{}' (score: {:.2})", 
                      name, skill.name, score);
                return Ok(None);
            }
        }

        // 5. Quality gate: Is this skill reusable?
        let gate_prompt = format!(
            "Review the following skill and answer ONLY 'yes' or 'no'.\n\
            Is this skill generic, reusable, and generally applicable for an AI agent?\n\
            Skill Name: {}\n\
            Description: {}\n\
            Body: {}",
            name, description, body
        );

        let gate_req = ChatCompletionRequest {
            model: model_name.to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: gate_prompt,
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(10),
            top_p: None,
            stop: None,
            stream: false,
            tools: None,
            tool_choice: None,
            extra_body: Default::default(),
            api_key_override: None,
            base_url_override: None,
        };

        let gate_response = provider.create_chat_completion(gate_req).await?;
        let gate_content = gate_response.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default()
            .to_lowercase();

        if !gate_content.contains("yes") {
            info!("Skill '{}' rejected by quality gate. Reason: not reusable enough.", name);
            return Ok(None);
        }

        // 6. Store via SkillStore
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let skill = Skill {
            id: Uuid::new_v4().to_string(),
            name,
            description,
            instructions: body,
            usage_count: 0,
            success_count: 0,
            created_at: now,
            updated_at: now,
        };

        store.insert_skill(&skill, &embedding)?;
        
        info!("Successfully synthesized and stored new skill: '{}' ({})", skill.name, skill.id);

        Ok(Some(skill))
    }
}
