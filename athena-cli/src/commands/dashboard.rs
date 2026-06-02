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

// Rust guideline compliant 2026-02-21
