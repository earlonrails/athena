use athena_agent::{AIAgent, AgentEvent};
use athena_tools::ToolRegistry;
use futures::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{stdin, stdout, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::accept_async;
use tracing::{error, info};

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    pub id: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

pub async fn handle_request(
    line: &str,
    agent: Arc<Mutex<AIAgent>>,
    registry: Arc<ToolRegistry>,
    provider: Arc<dyn athena_providers::LLMProvider + Send + Sync>,
    broadcaster: broadcast::Sender<String>,
) -> Option<RpcResponse> {
    if line.trim().is_empty() {
        return None;
    }

    let req: RpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            error!("Invalid JSON RPC: {}", e);
            return None;
        }
    };

    match req.method.as_str() {
        "agent/run" => {
            let prompt = req
                .params
                .and_then(|p| p.get("prompt").cloned())
                .and_then(|p| p.as_str().map(|s| s.to_string()))
                .unwrap_or_default();

            let req_id = req.id;
            let mut locked_agent = agent.lock().await;

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            
            let res = locked_agent
                .run_conversation_stream(
                    &prompt,
                    Some("You are Athena TUI."),
                    &registry,
                    provider,
                    tx,
                )
                .await;

            // Spawn task to forward streaming tokens as RPC notifications
            let tx_broadcaster = broadcaster.clone();
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    let method = match &event {
                        AgentEvent::TokenDelta(_) => "agent.token",
                        AgentEvent::ToolCallStart { .. } => "agent.tool_start",
                        AgentEvent::ToolCallComplete { .. } => "agent.tool_complete",
                        AgentEvent::FinalResponse(_) => "agent.final",
                        AgentEvent::TokenUsage { .. } => continue,
                        AgentEvent::Error(_) => "agent.error",
                    };
                    let notif = RpcNotification {
                        jsonrpc: "2.0".into(),
                        method: method.into(),
                        params: serde_json::to_value(&event).unwrap_or_default(),
                    };
                    let notif_str = serde_json::to_string(&notif).unwrap_or_default();
                    let _ = tx_broadcaster.send(notif_str);
                }
            });

            let response_value = match res {
                Ok(_) => serde_json::json!({ "status": "Started" }),
                Err(e) => serde_json::json!({ "error": e }),
            };

            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(response_value),
                error: None,
                id: req_id,
            })
        }
        "agent/cancel" => {
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({ "status": "cancelled" })),
                error: None,
                id: req.id,
            })
        }
        "session/list" => {
            let list = match athena_state::SessionDB::new(None) {
                Ok(db) => db.list_recent_sessions(50).unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::to_value(&list).unwrap_or_else(|_| serde_json::json!([]))),
                error: None,
                id: req.id,
            })
        }
        "session/load" => {
            let session_id = req
                .params
                .as_ref()
                .and_then(|p| p.get("session_id"))
                .and_then(|s| s.as_str());

            let result = if let Some(id) = session_id {
                match athena_state::SessionDB::new(None) {
                    Ok(db) => match db.get_session_trajectory(id) {
                        Ok((sess, msgs)) => serde_json::json!({
                            "session": sess,
                            "messages": msgs,
                            "status": "loaded"
                        }),
                        Err(e) => serde_json::json!({ "error": format!("Session not found: {}", e) }),
                    },
                    Err(e) => serde_json::json!({ "error": format!("DB error: {}", e) }),
                }
            } else {
                serde_json::json!({ "error": "Missing 'session_id' parameter" })
            };

            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(result),
                error: None,
                id: req.id,
            })
        }
        "config/get" => {
            let config = athena_core::config::load_config();
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::to_value(&config).unwrap_or_default()),
                error: None,
                id: req.id,
            })
        }
        "config/set" => {
            let mut status = "updated";
            if let Some(params) = req.params {
                if let Ok(config) = serde_json::from_value::<athena_core::config::AthenaConfig>(params) {
                    let _ = athena_core::config::save_config(&config);
                } else {
                    status = "partial_update";
                }
            }
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({ "status": status })),
                error: None,
                id: req.id,
            })
        }
        _ => Some(RpcResponse {
            jsonrpc: "2.0".into(),
            result: None,
            error: Some(serde_json::json!({ "message": "Method not found" })),
            id: req.id,
        }),
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let (tx, _rx) = broadcast::channel::<String>(100);

    // WebSocket Server on 8765
    let tx_ws = tx.clone();
    tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:8765").await.unwrap();
        info!("WebSocket bridge listening on ws://127.0.0.1:8765");
        while let Ok((stream, _)) = listener.accept().await {
            let mut ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    error!("WebSocket handshake failed: {}", e);
                    continue;
                }
            };
            let mut rx_ws = tx_ws.subscribe();
            tokio::spawn(async move {
                while let Ok(msg) = rx_ws.recv().await {
                    if ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
            });
        }
    });

    let stdin = stdin();
    let mut reader = BufReader::new(stdin).lines();
    let _std_out = stdout();

    let registry = Arc::new(ToolRegistry::new());
    let agent_builder = AIAgent::builder().model("gpt-4o").max_iterations(20);
    let agent = Arc::new(Mutex::new(agent_builder.build()));
    
    athena_providers::registry::init_builtin_providers();
    let provider = athena_providers::registry::get_provider("openai").unwrap();

    // Loop for stdio JSON RPC
    let tx_stdio = tx.clone();
    
    // Spawn a task to listen to broadcasts and print them to stdout
    let mut rx_stdio = tx_stdio.subscribe();
    tokio::spawn(async move {
        let mut out = tokio::io::stdout();
        while let Ok(msg) = rx_stdio.recv().await {
            let _ = out.write_all(format!("{}\n", msg).as_bytes()).await;
            let _ = out.flush().await;
        }
    });

    loop {
        match reader.next_line().await {
            Ok(Some(line)) => {
                let agent_clone = Arc::clone(&agent);
                let reg_clone = Arc::clone(&registry);
                let provider_clone = Arc::clone(&provider);
                let tx_clone = tx.clone();

                tokio::spawn(async move {
                    if let Some(rpc_res) = handle_request(&line, agent_clone, reg_clone, provider_clone, tx_clone.clone()).await {
                        let res_str = serde_json::to_string(&rpc_res).unwrap_or_default();
                        // Send response to broadcaster so it goes to stdio and WS
                        let _ = tx_clone.send(res_str);
                    }
                });
            }
            Ok(None) => break,
            Err(e) => {
                error!("Error reading stdin: {}", e);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyProvider;

    #[athena_providers::async_trait]
    impl athena_providers::LLMProvider for DummyProvider {
        fn profile(&self) -> &athena_providers::ProviderProfile {
            static PROFILE: std::sync::OnceLock<athena_providers::ProviderProfile> = std::sync::OnceLock::new();
            PROFILE.get_or_init(|| athena_providers::ProviderProfile::new("dummy"))
        }

        async fn fetch_models(&self, _api_key: Option<&str>, _timeout: f64) -> Result<Vec<String>, athena_providers::ProviderError> {
            Ok(vec![])
        }

        async fn create_chat_completion(
            &self,
            _request: athena_providers::ChatCompletionRequest,
        ) -> Result<athena_providers::ChatCompletionResponse, athena_providers::ProviderError> {
            Ok(athena_providers::ChatCompletionResponse {
                id: "1".into(),
                model: "dummy".into(),
                choices: vec![],
                usage: None,
                created: 0,
            })
        }

        async fn create_chat_completion_stream(
            &self,
            _request: athena_providers::ChatCompletionRequest,
        ) -> Result<athena_providers::ChatCompletionStream, athena_providers::ProviderError> {
            Err(athena_providers::ProviderError::ApiRequestFailed("unused".into()))
        }
    }

    #[tokio::test]
    async fn test_rpc_request_parsing_and_dispatch() {
        let agent = Arc::new(Mutex::new(AIAgent::builder().model("gpt-4o".to_string()).build()));
        let registry = Arc::new(ToolRegistry::new());
        let provider = Arc::new(DummyProvider);
        let (tx, _rx) = broadcast::channel(16);

        // Test cancel
        let req_json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent/cancel",
            "id": 1
        }).to_string();
        let resp = handle_request(&req_json, agent.clone(), registry.clone(), provider.clone(), tx.clone()).await.unwrap();
        assert_eq!(resp.id, Some(1));
        assert_eq!(resp.result.unwrap()["status"], "cancelled");

        // Test config/get
        let req_config = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "config/get",
            "id": 2
        }).to_string();
        let resp_config = handle_request(&req_config, agent.clone(), registry.clone(), provider.clone(), tx.clone()).await.unwrap();
        assert_eq!(resp_config.id, Some(2));
        assert!(resp_config.result.is_some());

        // Test session/list
        let req_list = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/list",
            "id": 3
        }).to_string();
        let resp_list = handle_request(&req_list, agent.clone(), registry.clone(), provider.clone(), tx.clone()).await.unwrap();
        assert_eq!(resp_list.id, Some(3));
        assert!(resp_list.result.is_some());

        // Test invalid JSON-RPC method
        let req_unknown = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "non_existent_method",
            "id": 4
        }).to_string();
        let resp_unknown = handle_request(&req_unknown, agent.clone(), registry.clone(), provider.clone(), tx.clone()).await.unwrap();
        assert_eq!(resp_unknown.id, Some(4));
        assert_eq!(resp_unknown.error.unwrap()["message"], "Method not found");

        // Test empty line
        let empty_resp = handle_request("", agent, registry, provider, tx).await;
        assert!(empty_resp.is_none());
    }
}

// Rust guideline compliant 2026-02-21
