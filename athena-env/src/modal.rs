use async_trait::async_trait;
use reqwest::Client;

use crate::traits::{Environment, ExecutionConfig, ExecutionResult, EnvError};

pub struct ModalEnv {
    id: String,
    client: Client,
    endpoint: String,
    token: String,
}

impl ModalEnv {
    pub fn new(id: impl Into<String>, endpoint: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            client: Client::new(),
            endpoint: endpoint.into(),
            token: token.into(),
        }
    }
}

#[async_trait]
impl Environment for ModalEnv {
    fn id(&self) -> &str {
        &self.id
    }
    
    async fn init(&self) -> Result<(), EnvError> {
        // Modal functions spin up instantly, no explicit init required usually
        Ok(())
    }
    
    async fn execute(&self, command: &str, _config: ExecutionConfig) -> Result<ExecutionResult, EnvError> {
        let body = serde_json::json!({
            "command": command
        });
        
        let res = self.client.post(&self.endpoint)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| EnvError::ExecutionFailed(e.to_string()))?;
            
        if !res.status().is_success() {
            return Err(EnvError::ExecutionFailed(res.status().to_string()));
        }
        
        let data: serde_json::Value = res.json().await
            .map_err(|e| EnvError::ExecutionFailed(e.to_string()))?;
            
        Ok(ExecutionResult {
            exit_code: data["exit_code"].as_i64().unwrap_or(-1) as i32,
            stdout: data["stdout"].as_str().unwrap_or_default().to_string(),
            stderr: data["stderr"].as_str().unwrap_or_default().to_string(),
        })
    }
    
    async fn write_file(&self, path: &str, content: &[u8]) -> Result<(), EnvError> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let encoded = STANDARD.encode(content);
        let escaped_path = path.replace('\'', "'\\''");
        let cmd = format!(
            "mkdir -p \"$(dirname '{}')\" && echo '{}' | base64 -d > '{}'",
            escaped_path, encoded, escaped_path
        );
        let res = self.execute(&cmd, ExecutionConfig::default()).await?;
        if res.exit_code != 0 {
            return Err(EnvError::ExecutionFailed(format!(
                "Failed to write file: {}",
                res.stderr
            )));
        }
        Ok(())
    }
    
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, EnvError> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let escaped_path = path.replace('\'', "'\\''");
        let cmd = format!("base64 '{}'", escaped_path);
        let res = self.execute(&cmd, ExecutionConfig::default()).await?;
        if res.exit_code != 0 {
            return Err(EnvError::ExecutionFailed(format!(
                "Failed to read file: {}",
                res.stderr
            )));
        }
        let clean_stdout = res.stdout.replace(['\n', '\r'], "").trim().to_string();
        STANDARD.decode(&clean_stdout).map_err(|e| {
            EnvError::ExecutionFailed(format!("Failed to decode base64 file content: {}", e))
        })
    }
    
    async fn destroy(&self) -> Result<(), EnvError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, header, body_json, path};
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use serde_json::json;

    #[tokio::test]
    async fn test_modal_env_id() {
        let env = ModalEnv::new("test-modal", "http://localhost", "test-token");
        assert_eq!(env.id(), "test-modal");
    }

    #[tokio::test]
    async fn test_modal_env_init() {
        let env = ModalEnv::new("test-modal", "http://localhost", "test-token");
        let res = env.init().await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_modal_env_execute_success() {
        let mock_server = MockServer::start().await;
        let endpoint = mock_server.uri();

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("Authorization", "Bearer test-token"))
            .and(body_json(json!({"command": "echo hello"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "exit_code": 0,
                "stdout": "hello\n",
                "stderr": ""
            })))
            .mount(&mock_server)
            .await;

        let env = ModalEnv::new("test-modal", endpoint, "test-token");
        let result = env.execute("echo hello", ExecutionConfig::default()).await.unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "hello\n");
        assert_eq!(result.stderr, "");
    }

    #[tokio::test]
    async fn test_modal_env_execute_failure() {
        let mock_server = MockServer::start().await;
        let endpoint = mock_server.uri();

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let env = ModalEnv::new("test-modal", endpoint, "test-token");
        let result = env.execute("echo hello", ExecutionConfig::default()).await;

        assert!(matches!(result, Err(EnvError::ExecutionFailed(_))));
    }

    #[tokio::test]
    async fn test_modal_env_execute_network_error() {
        // Use an invalid endpoint to force a reqwest error
        let env = ModalEnv::new("test-modal", "http://localhost:1", "test-token");
        let result = env.execute("echo hello", ExecutionConfig::default()).await;
        
        assert!(matches!(result, Err(EnvError::ExecutionFailed(_))));
    }

    #[tokio::test]
    async fn test_modal_env_execute_invalid_json() {
        let mock_server = MockServer::start().await;
        let endpoint = mock_server.uri();

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
            .mount(&mock_server)
            .await;

        let env = ModalEnv::new("test-modal", endpoint, "test-token");
        let result = env.execute("echo hello", ExecutionConfig::default()).await;

        assert!(matches!(result, Err(EnvError::ExecutionFailed(_))));
    }

    #[tokio::test]
    async fn test_modal_env_write_file() {
        let mock_server = MockServer::start().await;
        let endpoint = mock_server.uri();

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "exit_code": 0,
                "stdout": "",
                "stderr": ""
            })))
            .mount(&mock_server)
            .await;

        let env = ModalEnv::new("test-modal", endpoint, "test-token");
        let res = env.write_file("test.txt", b"hello").await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_modal_env_read_file() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let mock_server = MockServer::start().await;
        let endpoint = mock_server.uri();

        let encoded = STANDARD.encode(b"hello modal");
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "exit_code": 0,
                "stdout": encoded,
                "stderr": ""
            })))
            .mount(&mock_server)
            .await;

        let env = ModalEnv::new("test-modal", endpoint, "test-token");
        let res = env.read_file("test.txt").await.unwrap();
        assert_eq!(res, b"hello modal");
    }

    #[tokio::test]
    async fn test_modal_env_destroy() {
        let env = ModalEnv::new("test-modal", "http://localhost", "test-token");
        let res = env.destroy().await;
        assert!(res.is_ok());
    }
}

// Rust guideline compliant 2026-02-21
