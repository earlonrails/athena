use axum::{
    extract::{State, Json},
    response::IntoResponse,
};
use std::sync::Arc;
use athena_tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use athena_multimedia::AudioProcessor;

#[derive(Deserialize)]
pub struct WhatsAppPayload {
    pub message: Option<String>,
    pub audio_base64: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WhatsAppResponse {
    pub response: String,
}

pub async fn handle_whatsapp_event(
    State(registry): State<Arc<ToolRegistry>>,
    Json(payload): Json<WhatsAppPayload>,
) -> impl IntoResponse {
    let mut actual_message = payload.message.unwrap_or_default();

    // If there's an audio attachment, process it first
    if let Some(audio_data) = payload.audio_base64 {
        if let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &audio_data) {
            let tmp_path = std::env::temp_dir().join(format!("wa_audio_{}.ogg", uuid::Uuid::new_v4()));
            if std::fs::write(&tmp_path, decoded).is_ok() {
                let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
                if !api_key.is_empty() {
                    let processor = AudioProcessor::new(api_key);
                    if let Ok(transcribed) = processor.transcribe(tmp_path.to_str().unwrap()).await {
                        tracing::info!("Transcribed WhatsApp audio: {}", transcribed);
                        actual_message = transcribed;
                    }
                }
                let _ = std::fs::remove_file(tmp_path);
            }
        }
    }

    if actual_message.is_empty() {
        return Json(WhatsAppResponse { response: "No message content or audio could be parsed.".to_string() });
    }

    if let Ok(response) = crate::process_gateway_message(&actual_message, registry).await {
        Json(WhatsAppResponse { response })
    } else {
        Json(WhatsAppResponse { response: "Error processing message".to_string() })
    }
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
    async fn test_whatsapp_empty_payload() {
        let registry = Arc::new(ToolRegistry::new());
        let payload = WhatsAppPayload {
            message: None,
            audio_base64: None,
        };

        let response = handle_whatsapp_event(State(registry), Json(payload)).await;
        let (parts, body) = response.into_response().into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let body_str = String::from_utf8(bytes.to_vec()).unwrap();
        
        let parsed: WhatsAppResponse = serde_json::from_str(&body_str).unwrap();
        assert_eq!(parsed.response, "No message content or audio could be parsed.");
        assert_eq!(parts.status, axum::http::StatusCode::OK);
    }
}

// Rust guideline compliant 2026-02-21
