use axum::{
    extract::{State, Json},
    response::IntoResponse,
};
use std::sync::Arc;
use athena_tools::ToolRegistry;
use serde_json::Value;

pub async fn handle_slack_event(
    State(registry): State<Arc<ToolRegistry>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // URL Verification
    if payload.get("type").and_then(|v| v.as_str()) == Some("url_verification") {
        if let Some(challenge) = payload.get("challenge").and_then(|v| v.as_str()) {
            return challenge.to_string();
        }
    }

    // Event Callback
    if payload.get("type").and_then(|v| v.as_str()) == Some("event_callback") {
        if let Some(event) = payload.get("event") {
            let event_type = event.get("type").and_then(|v| v.as_str());
            if event_type == Some("app_mention") || event_type == Some("message") {
                if let Some(text) = event.get("text").and_then(|v| v.as_str()) {
                    let channel = event.get("channel").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let thread_ts = event.get("thread_ts").and_then(|v| v.as_str()).map(|s| s.to_string());
                    
                    let reg = registry.clone();
                    let text_copy = text.to_string();
                    
                    tokio::spawn(async move {
                        if let Ok(response) = crate::process_gateway_message(&text_copy, reg).await {
                            send_slack_message(&channel, &response, thread_ts).await;
                        }
                    });
                }
            }
        }
    }

    "OK".to_string()
}

pub async fn send_slack_message(channel: &str, text: &str, thread_ts: Option<String>) {
    let token = std::env::var("SLACK_BOT_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return;
    }

    let mut body = serde_json::json!({
        "channel": channel,
        "text": text,
    });

    if let Some(ts) = thread_ts {
        body.as_object_mut().unwrap().insert("thread_ts".to_string(), serde_json::json!(ts));
    }

    let client = reqwest::Client::new();
    let _ = client.post("https://slack.com/api/chat.postMessage")
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .await;
}

// Rust guideline compliant 2026-02-21
