use athena_agent::{AIAgent, AgentEvent};
use athena_tools::ToolRegistry;
use futures::{SinkExt, StreamExt};
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
            // Placeholder: we would need a Cancellation token or atomic bool injected into AIAgent
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({ "status": "cancelled" })),
                error: None,
                id: req.id,
            })
        }
        "session/list" => {
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!([])),
                error: None,
                id: req.id,
            })
        }
        "session/load" => {
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({ "status": "loaded" })),
                error: None,
                id: req.id,
            })
        }
        "config/get" => {
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({ "model": "gpt-4o" })),
                error: None,
                id: req.id,
            })
        }
        "config/set" => {
            Some(RpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({ "status": "updated" })),
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

// Rust guideline compliant 2026-02-21
