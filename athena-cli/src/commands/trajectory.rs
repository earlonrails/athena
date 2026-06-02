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

// Rust guideline compliant 2026-02-21
