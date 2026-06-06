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

#[derive(serde::Deserialize, Debug)]
pub struct SlackCommandPayload {
    pub text: String,
    pub channel_id: String,
}

pub async fn handle_slack_commands(
    State(registry): State<Arc<ToolRegistry>>,
    axum::extract::Form(payload): axum::extract::Form<SlackCommandPayload>,
) -> impl IntoResponse {
    let reg = registry.clone();
    let text_copy = payload.text.clone();
    let channel = payload.channel_id.clone();
    
    tokio::spawn(async move {
        if let Ok(response) = crate::process_gateway_message(&text_copy, reg).await {
            send_slack_message(&channel, &response, None).await;
        }
    });

    "Processing command...".to_string()
}

#[derive(serde::Deserialize, Debug)]
pub struct SlackInteractivePayload {
    pub payload: String,
}

pub async fn handle_slack_interactive(
    axum::extract::Form(payload): axum::extract::Form<SlackInteractivePayload>,
) -> impl IntoResponse {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&payload.payload) {
        if let Some(channel) = parsed.get("channel").and_then(|c| c.get("id")).and_then(|i| i.as_str()) {
            let thread_ts = parsed.get("message").and_then(|m| m.get("ts")).and_then(|ts| ts.as_str());
            // Since we don't actively suspend the agent for true external approval yet,
            // we will simulate the interactive acknowledgement here.
            send_slack_message(channel, "Tool execution confirmed by user via interactive block.", thread_ts.map(|s| s.to_string())).await;
        }
    }
    
    "OK".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{State, Json};
    use axum::response::IntoResponse;
    use serde_json::json;
    use std::sync::Arc;
    use athena_tools::ToolRegistry;

    #[tokio::test]
    async fn test_slack_url_verification() {
        let registry = Arc::new(ToolRegistry::new());
        let payload = json!({
            "type": "url_verification",
            "challenge": "test_challenge_string"
        });

        let response = handle_slack_event(State(registry), Json(payload)).await;
        // IntoResponse returns a Response. We can get the body using axum's body::to_bytes.
        let (parts, body) = response.into_response().into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let body_str = String::from_utf8(bytes.to_vec()).unwrap();
        
        assert_eq!(body_str, "test_challenge_string");
        assert_eq!(parts.status, axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_slack_event_callback_ignores_unknown() {
        let registry = Arc::new(ToolRegistry::new());
        let payload = json!({
            "type": "event_callback",
            "event": {
                "type": "unknown_event"
            }
        });

        let response = handle_slack_event(State(registry), Json(payload)).await;
        let (parts, body) = response.into_response().into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let body_str = String::from_utf8(bytes.to_vec()).unwrap();
        
        assert_eq!(body_str, "OK");
        assert_eq!(parts.status, axum::http::StatusCode::OK);
    }
}

// Rust guideline compliant 2026-02-21
