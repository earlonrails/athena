use crate::registry::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

pub struct TerminalTool;

#[async_trait]
impl Tool for TerminalTool {
    fn name(&self) -> &'static str { "run_command" }
    fn toolset(&self) -> &'static str { "terminal" }
    fn schema(&self) -> Value {
        json!({
            "description": "Execute a terminal command.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to run." }
                },
                "required": ["command"]
            }
        })
    }
    async fn handle(&self, args: Value) -> Result<Value, String> {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return Ok(json!({ "error": "Missing or invalid 'command' argument" })),
        };

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .arg("/C")
                .arg(command)
                .output()
                .await
        } else {
            Command::new("bash")
                .arg("-c")
                .arg(command)
                .output()
                .await
        };

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
            Err(e) => Ok(json!({ "error": format!("Failed to execute command: {}", e) })),
        }
    }
}

inventory::submit!(crate::registry::RegisteredTool { factory: || std::sync::Arc::new(TerminalTool) });

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_terminal_tool() {
        let tool = TerminalTool;
        assert_eq!(tool.name(), "run_command");
        assert_eq!(tool.toolset(), "terminal");

        let schema = tool.schema();
        assert!(schema.get("description").is_some());
        assert!(schema.get("parameters").is_some());

        let result = tool.handle(json!({})).await.unwrap();
        assert_eq!(result["error"], "Missing or invalid 'command' argument");
        
        let result = tool.handle(serde_json::json!({"command": "echo test"})).await.unwrap();
        assert_eq!(result["success"], true);
        assert!(result["stdout"].as_str().unwrap().contains("test"));
    }
}

// Rust guideline compliant 2026-02-21
