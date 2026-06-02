use axum::{
    extract::{State, Json},
    response::IntoResponse,
};
use std::sync::Arc;
use athena_tools::ToolRegistry;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct WhatsAppPayload {
    pub message: String,
}

#[derive(Serialize)]
pub struct WhatsAppResponse {
    pub response: String,
}

pub async fn handle_whatsapp_event(
    State(registry): State<Arc<ToolRegistry>>,
    Json(payload): Json<WhatsAppPayload>,
) -> impl IntoResponse {
    if let Ok(response) = crate::process_gateway_message(&payload.message, registry).await {
        Json(WhatsAppResponse { response })
    } else {
        Json(WhatsAppResponse { response: "Error processing message".to_string() })
    }
}

// Rust guideline compliant 2026-02-21
