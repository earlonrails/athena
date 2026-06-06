use crate::registry::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::fs;
use tokio::process::Command;
use std::env;
use std::sync::Arc;
use athena_env::traits::{Environment, ExecutionConfig};

pub struct CodeExecutionTool {
    pub env: Option<Arc<dyn Environment>>,
}

impl CodeExecutionTool {
    pub fn new(env: Option<Arc<dyn Environment>>) -> Self {
        Self { env }
    }
}

#[async_trait]
impl Tool for CodeExecutionTool {
    fn name(&self) -> &'static str { "execute_code" }
    fn toolset(&self) -> &'static str { "code_execution" }
    fn schema(&self) -> Value {
        json!({
            "description": "Execute a short snippet of code. Supports python and node.",
            "parameters": {
                "type": "object",
                "properties": {
                    "language": { "type": "string", "enum": ["python", "node"], "description": "The programming language." },
                    "code": { "type": "string", "description": "The code to execute." },
                    "timeout_seconds": { "type": "integer", "description": "Optional timeout in seconds." }
                },
                "required": ["language", "code"]
            }
        })
    }
    async fn handle(&self, args: Value) -> Result<Value, String> {
        let language = match args.get("language").and_then(|v| v.as_str()) {
            Some(l) => l,
            None => return Ok(json!({ "error": "Missing or invalid 'language' argument" })),
        };
        let code = match args.get("code").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return Ok(json!({ "error": "Missing or invalid 'code' argument" })),
        };
        
        let timeout = args.get("timeout_seconds").and_then(|v| v.as_u64()).unwrap_or(30);

        let ext = match language {
            "python" => "py",
            "node" => "js",
            _ => return Ok(json!({ "error": "Unsupported language" })),
        };

        if let Some(ref e) = self.env {
            // Use sandboxed environment
            let path = format!("/tmp/code_{}.{}", uuid::Uuid::new_v4(), ext);
            if let Err(err) = e.write_file(&path, code.as_bytes()).await {
                return Ok(json!({ "error": format!("Failed to write code to sandbox: {}", err) }));
            }
            
            let cmd = match language {
                "python" => format!("python3 {}", path),
                "node" => format!("node {}", path),
                _ => unreachable!(),
            };
            
            let config = ExecutionConfig {
                timeout_seconds: Some(timeout),
                ..Default::default()
            };
            
            let res = e.execute(&cmd, config).await;
            
            return match res {
                Ok(out) => Ok(json!({
                    "success": out.exit_code == 0,
                    "exit_code": out.exit_code,
                    "stdout": out.stdout,
                    "stderr": out.stderr,
                })),
                Err(err) => Ok(json!({ "error": format!("Sandbox execution failed: {}", err) })),
            };
        }

        // Local fallback
        let mut temp_dir = env::temp_dir();
        temp_dir.push(format!("athena_code_eval_{}.{}", uuid::Uuid::new_v4(), ext));

        if let Err(e) = fs::write(&temp_dir, code).await {
            return Ok(json!({ "error": format!("Failed to write code to temp file: {}", e) }));
        }

        let output = match language {
            "python" => Command::new("python3").arg(&temp_dir).output().await,
            "node" => Command::new("node").arg(&temp_dir).output().await,
            _ => unreachable!(),
        };

        let _ = fs::remove_file(&temp_dir).await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                Ok(json!({
                    "success": out.status.success(),
                    "exit_code": out.status.code(),
                    "stdout": stdout,
                    "stderr": stderr,
                }))
            }
            Err(e) => Ok(json!({ "error": format!("Failed to execute code: {}", e) })),
        }
    }
}

// Register default fallback tool
inventory::submit!(crate::registry::RegisteredTool { factory: || std::sync::Arc::new(CodeExecutionTool::new(None)) });

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_code_execution_tool() {
        let tool = CodeExecutionTool::new(None);
        assert_eq!(tool.name(), "execute_code");
        assert_eq!(tool.toolset(), "code_execution");

        let schema = tool.schema();
        assert!(schema.get("description").is_some());
        assert!(schema.get("parameters").is_some());

        let result = tool.handle(json!({})).await.unwrap();
        assert_eq!(result["error"], "Missing or invalid 'language' argument");
    }
}

// Rust guideline compliant 2026-02-21
