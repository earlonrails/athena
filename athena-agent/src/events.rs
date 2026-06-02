use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AgentEvent {
    TokenDelta(String),
    ToolCallStart { id: String, name: String, arguments: String },
    ToolCallComplete { id: String, name: String, result: String },
    FinalResponse(String),
    Error(String),
}

// Rust guideline compliant 2026-02-21
