use axum::{
    routing::{get, delete},
    Router,
    response::Json,
    extract::{ws::{WebSocketUpgrade, WebSocket}, State, Path},
};
use tower_http::services::ServeDir;
use tower_http::cors::CorsLayer;
use std::net::SocketAddr;
use std::sync::Arc;

use athena_core::config::{load_config, save_config, AthenaConfig};
use athena_core::paths::get_athena_home;
use athena_agent::AIAgent;
use athena_tools::ToolRegistry;
use athena_providers::LLMProvider;

#[derive(Clone)]
struct AppState {
    registry: Arc<ToolRegistry>,
    provider: Arc<dyn LLMProvider + Send + Sync>,
}

#[derive(serde::Serialize)]
struct SkillInfo {
    name: String,
    path: String,
}

#[derive(serde::Deserialize)]
struct CreateSkillReq {
    name: String,
}

#[derive(serde::Deserialize)]
struct CreatePluginReq {
    name: String,
}

pub async fn run_dashboard() {
    println!("\nAthena Web GUI Dashboard");
    println!("══════════════════════════\n");
    println!("Launching local dashboard at http://localhost:8000...");
    println!("Press Ctrl+C to stop.");
    println!();

    let mut web_dir = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .parent()
        .unwrap()
        .to_path_buf();
        
    if std::path::Path::new("apps/dashboard/dist").exists() {
        web_dir = std::path::PathBuf::from("apps/dashboard/dist");
    } else {
        web_dir = web_dir.join("dist"); 
    }

    athena_providers::registry::init_builtin_providers();
    let provider = athena_providers::registry::get_provider("openai").unwrap();
    let registry = Arc::new(ToolRegistry::new());

    let state = AppState { registry, provider };

    let app = Router::new()
        .route("/api/config", get(get_config).post(update_config))
        .route("/api/skills", get(get_skills).post(add_skill))
        .route("/api/skills/:name", delete(remove_skill))
        .route("/api/plugins", get(get_plugins).post(add_plugin))
        .route("/api/plugins/:name", delete(remove_plugin))
        .route("/api/mcp", get(get_mcp).post(update_mcp))
        .route("/api/chat", get(ws_handler))
        .route("/api/kanban", get(get_kanban).post(post_kanban))
        .fallback_service(ServeDir::new(&web_dir))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            println!("✗ Failed to bind to port 8000: {}. Is another server running?", e);
            return;
        }
    };

    let _ = open::that("http://localhost:8000");

    if let Err(e) = axum::serve(listener, app).await {
        println!("Server error: {}", e);
    }
}

async fn get_config() -> Json<AthenaConfig> {
    let config = load_config();
    Json(config)
}

async fn update_config(Json(config): Json<AthenaConfig>) -> Json<bool> {
    let res = save_config(&config);
    Json(res.is_ok())
}

async fn get_skills() -> Json<Vec<SkillInfo>> {
    let mut skills = Vec::new();
    let skills_dir = get_athena_home().join("skills");
    if let Ok(entries) = std::fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    skills.push(SkillInfo {
                        name: name.to_string(),
                        path: path.to_string_lossy().into_owned(),
                    });
                }
            }
        }
    }
    Json(skills)
}

async fn get_mcp() -> Json<crate::commands::mcp::McpServersList> {
    let mcp_file = get_athena_home().join("mcp_servers.json");
    let data = crate::commands::mcp::get_mcp_servers(&mcp_file);
    Json(data)
}

async fn update_mcp(Json(mcp_list): Json<crate::commands::mcp::McpServersList>) -> Json<bool> {
    let mcp_file = get_athena_home().join("mcp_servers.json");
    let res = crate::commands::mcp::save_mcp_servers(&mcp_file, &mcp_list);
    Json(res.is_ok())
}

async fn add_skill(Json(req): Json<CreateSkillReq>) -> Json<bool> {
    let name = req.name.trim();
    if name.is_empty() { return Json(false); }
    let skills_dir = get_athena_home().join("skills");
    let _ = std::fs::create_dir_all(&skills_dir);
    let skill_path = skills_dir.join(format!("{}.rs", name));
    let template = format!(
        "// Skill: {}\n// Description: A new custom semantic skill definition\n\npub fn execute() {{\n    println!(\"Executing {} skill...\");\n}}\n",
        name, name
    );
    Json(std::fs::write(&skill_path, template).is_ok())
}

async fn remove_skill(Path(name): Path<String>) -> Json<bool> {
    let skills_dir = get_athena_home().join("skills");
    let skill_path = skills_dir.join(name);
    Json(std::fs::remove_file(&skill_path).is_ok())
}

async fn get_plugins() -> Json<Vec<SkillInfo>> {
    let mut plugins = Vec::new();
    let plugins_dir = get_athena_home().join("plugins");
    if let Ok(entries) = std::fs::read_dir(plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    plugins.push(SkillInfo {
                        name: name.to_string(),
                        path: path.to_string_lossy().into_owned(),
                    });
                }
            }
        }
    }
    Json(plugins)
}

async fn add_plugin(Json(req): Json<CreatePluginReq>) -> Json<bool> {
    let name = req.name.trim();
    if name.is_empty() { return Json(false); }
    let plugins_dir = get_athena_home().join("plugins");
    let _ = std::fs::create_dir_all(&plugins_dir);
    let plugin_path = plugins_dir.join(format!("{}.wasm", name));
    let wasm_skeleton = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    Json(std::fs::write(&plugin_path, wasm_skeleton).is_ok())
}

async fn remove_plugin(Path(name): Path<String>) -> Json<bool> {
    let plugins_dir = get_athena_home().join("plugins");
    let plugin_path = plugins_dir.join(name);
    Json(std::fs::remove_file(&plugin_path).is_ok())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> axum::response::Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let axum::extract::ws::Message::Text(text) = msg {
            // Dynamically load config per request to pick up user changes
            let config = load_config();
            let provider_slug = config.model.provider.clone();
            let model_name = config.model.default.clone();
            
            // Get API key for active provider
            let mut api_key = None;
            if let Some(p_cfg) = config.providers.get(&provider_slug) {
                api_key = p_cfg.api_key.clone();
            }

            let mut agent_builder = AIAgent::builder()
                .model(&model_name)
                .max_iterations(config.agent.max_iterations as usize);
                
            if let Some(key) = api_key {
                agent_builder = agent_builder.api_key(&key);
            }

            if let Ok(db) = athena_state::db::SessionDB::new(None) {
                let logger = crate::logger::DbSessionLogger {
                    db: std::sync::Arc::new(db),
                    model: model_name.clone(),
                };
                agent_builder = agent_builder.logger(std::sync::Arc::new(logger));
            }

            let mut locked_agent = agent_builder.build();
            let dynamic_provider = athena_providers::registry::get_provider(&provider_slug).unwrap_or(state.provider.clone());

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            
            let user_text = text.clone();
            let reg = state.registry.clone();
            
            tokio::spawn(async move {
                let _ = locked_agent.run_conversation_stream(
                    &user_text,
                    Some("You are a helpful dashboard assistant."),
                    &reg,
                    dynamic_provider,
                    tx
                ).await;
            });

            while let Some(event) = rx.recv().await {
                if let Ok(json_str) = serde_json::to_string(&event) {
                    if socket.send(axum::extract::ws::Message::Text(json_str)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

async fn get_kanban() -> Json<serde_json::Value> {
    if let Ok(db) = athena_state::kanban::KanbanDB::new() {
        let _ = db.init_default_board();
        if let Ok(cards) = db.get_cards("default") {
            return Json(serde_json::json!({ "cards": cards }));
        }
    }
    Json(serde_json::json!({ "error": "failed to load kanban" }))
}

#[derive(serde::Deserialize)]
struct KanbanAction {
    action: String,
    task_id: Option<String>,
    title: Option<String>,
    column_id: Option<String>,
    assignee: Option<String>,
}

async fn post_kanban(axum::extract::Json(payload): axum::extract::Json<KanbanAction>) -> Json<serde_json::Value> {
    if let Ok(db) = athena_state::kanban::KanbanDB::new() {
        let _ = db.init_default_board();
        match payload.action.as_str() {
            "create" => {
                let id = uuid::Uuid::new_v4().to_string();
                if let Some(title) = &payload.title {
                    if db.create_card(&id, "default", payload.column_id.as_deref().unwrap_or("col-todo"), title, payload.assignee.as_deref()).is_ok() {
                        return Json(serde_json::json!({ "status": "success", "id": id }));
                    }
                }
            }
            "move" => {
                if let (Some(id), Some(col)) = (&payload.task_id, &payload.column_id) {
                    if db.move_card(id, col).is_ok() {
                        return Json(serde_json::json!({ "status": "success" }));
                    }
                }
            }
            "assign" => {
                if let (Some(id), Some(assignee)) = (&payload.task_id, &payload.assignee) {
                    if db.assign_card(id, assignee).is_ok() {
                        return Json(serde_json::json!({ "status": "success" }));
                    }
                }
            }
            "delete" => {
                if let Some(id) = &payload.task_id {
                    if db.delete_card(id).is_ok() {
                        return Json(serde_json::json!({ "status": "success" }));
                    }
                }
            }
            _ => {}
        }
    }
    Json(serde_json::json!({ "error": "failed to process action" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use athena_tools::ToolRegistry;
    use athena_providers::{LLMProvider, ProviderProfile, ChatCompletionResponse, ChatCompletionStream, StreamChunk, Choice, ChatMessage, MessageRole, ChatCompletionRequest, ProviderError, StreamChoice, StreamDelta};
    use async_trait::async_trait;
    use tokio::net::TcpListener;
    use tokio_tungstenite::connect_async;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    struct DummyProvider {
        profile: ProviderProfile,
    }

    #[async_trait]
    impl LLMProvider for DummyProvider {
        fn profile(&self) -> &ProviderProfile {
            &self.profile
        }
        
        async fn fetch_models(&self, _: Option<&str>, _: f64) -> Result<Vec<String>, ProviderError> {
            Ok(vec!["dummy".to_string()])
        }
        
        async fn create_chat_completion(&self, _: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
            Ok(ChatCompletionResponse {
                id: "1".into(),
                model: "dummy".into(),
                choices: vec![
                    Choice {
                        index: 0,
                        message: ChatMessage {
                            role: MessageRole::Assistant,
                            content: "Mock response".into(),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                        },
                        finish_reason: Some("stop".into()),
                    }
                ],
                usage: None,
                created: 0,
            })
        }

        async fn create_chat_completion_stream(&self, _: ChatCompletionRequest) -> Result<ChatCompletionStream, ProviderError> {
            let chunks = vec![
                Ok(StreamChunk {
                    id: "1".to_string(),
                    model: "dummy".to_string(),
                    created: None,
                    choices: vec![
                        StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: Some(MessageRole::Assistant),
                                content: Some("Hello ".to_string()),
                                tool_calls: None,
                            },
                            finish_reason: None,
                        }
                    ],
                    usage: None,
                }),
                Ok(StreamChunk {
                    id: "2".to_string(),
                    model: "dummy".to_string(),
                    created: None,
                    choices: vec![
                        StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: None,
                                content: Some("World".to_string()),
                                tool_calls: None,
                            },
                            finish_reason: Some("stop".to_string()),
                        }
                    ],
                    usage: None,
                }),
            ];
            
            let stream = futures_util::stream::iter(chunks);
            Ok(ChatCompletionStream {
                response: Box::new(stream),
            })
        }
    }

    #[tokio::test]
    async fn test_websocket_bridge_roundtrip() {
        let registry = Arc::new(ToolRegistry::new());
        let provider = Arc::new(DummyProvider { profile: ProviderProfile::new("dummy") });
        let state = AppState { registry, provider };

        let app = Router::new()
            .route("/api/chat", get(ws_handler))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let ws_url = format!("ws://{}/api/chat", addr);
        let (mut ws_stream, _) = connect_async(&ws_url).await.expect("Failed to connect");

        // Send a message
        ws_stream.send(Message::Text("Hello".to_string())).await.unwrap();

        // Receive the response chunks
        // First chunk from dummy provider
        let mut responses = Vec::new();
        while let Some(msg) = ws_stream.next().await {
            if let Ok(Message::Text(text)) = msg {
                responses.push(text);
                if responses.len() >= 2 {
                    break;
                }
            }
        }
        
        assert_eq!(responses.len(), 2);
        assert!(responses[0].contains("Hello "));
        assert!(responses[1].contains("World"));
    }
}

// Rust guideline compliant 2026-02-21
