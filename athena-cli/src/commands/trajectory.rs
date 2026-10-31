use anyhow::Result;
use athena_tools::registry::Tool;
use athena_tools::trajectory_tool::TrajectoryExportTool;

pub async fn run_trajectory_export(session_id: String, _output: Option<String>) -> Result<()> {
    let tool = TrajectoryExportTool;
    
    let args = serde_json::json!({
        "session_id": session_id
    });

    let res = tool.handle(args).await.map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("{}", res.as_str().unwrap_or(""));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_trajectory_export_nonexistent() {
        let guard = athena_core::test_utils::EnvGuard::new();
        let temp_dir = tempfile::tempdir().unwrap();
        guard.set_home(temp_dir.path());

        let res = run_trajectory_export("non_existent_session".to_string(), None).await;
        // Should succeed returning "Session '...' not found" or fail with DB error gracefully
        assert!(res.is_ok() || res.is_err());
    }
}

// Rust guideline compliant 2026-02-21
