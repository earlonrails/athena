use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::sync::Arc;
use athena_tools::ToolRegistry;
use tracing::{info, error};

pub struct Handler {
    pub registry: Arc<ToolRegistry>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        // Only respond to DMs or mentions
        let is_dm = msg.guild_id.is_none();
        let is_mention = msg.mentions_me(&ctx.http).await.unwrap_or(false);

        if is_dm || is_mention {
            // Send initial "Thinking..."
            let mut thinking_msg = match msg.channel_id.say(&ctx.http, "🤔 Thinking...").await {
                Ok(m) => m,
                Err(e) => {
                    error!("Error sending thinking message: {:?}", e);
                    return;
                }
            };

            let registry = self.registry.clone();
            let text = msg.content.clone();

            tokio::spawn(async move {
                match crate::process_gateway_message(&text, registry).await {
                    Ok(response) => {
                        if let Err(e) = thinking_msg.edit(&ctx.http, serenity::builder::EditMessage::new().content(response)).await {
                            error!("Error editing discord message: {:?}", e);
                        }
                    }
                    Err(e) => {
                        if let Err(e2) = thinking_msg.edit(&ctx.http, serenity::builder::EditMessage::new().content(format!("Error: {}", e))).await {
                            error!("Error editing discord error message: {:?}", e2);
                        }
                    }
                }
            });
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("Discord connected as {}", ready.user.name);
    }
}

// Rust guideline compliant 2026-02-21
