use std::io::{self, BufRead};

pub fn run_acp() {
    println!("\nAthena Agent Client Protocol (ACP) Server");
    println!("═══════════════════════════════════════════\n");
    println!("Launching local ACP listener server via standard JSON-RPC on stdin/stdout...");
    println!("Press Ctrl+C to stop.");
    println!();

    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();

    while let Ok(n) = handle.read_line(&mut line) {
        if n == 0 {
            break;
        }

        if let Some(resp) = handle_acp_message(line.trim()) {
            println!("{}", resp);
            io::Write::flush(&mut io::stdout()).ok();
        }
        line.clear();
    }
}

pub fn handle_acp_message(line: &str) -> Option<&'static str> {
    if line.contains("\"method\":\"handshake\"") {
        Some(r#"{"jsonrpc":"2.0","result":{"protocolVersion":"0.1.0","capabilities":{"agent":true,"client":false}},"id":1}"#)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_acp_handshake() {
        let msg = r#"{"jsonrpc":"2.0","method":"handshake","id":1}"#;
        let resp = handle_acp_message(msg).unwrap();
        assert!(resp.contains("protocolVersion"));
        assert!(resp.contains("capabilities"));

        let other = r#"{"jsonrpc":"2.0","method":"ping"}"#;
        assert!(handle_acp_message(other).is_none());
    }
}

// Rust guideline compliant 2026-02-21
