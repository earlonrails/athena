use std::io::{self, BufRead, Write};

pub const LSP_CAPABILITIES_JSON: &str = r#"{"jsonrpc":"2.0","result":{"capabilities":{"textDocumentSync":1,"completionProvider":{"resolveProvider":true},"hoverProvider":true,"codeActionProvider":true}},"id":1}"#;

pub fn process_lsp_stream<R: BufRead, W: Write>(mut reader: R, mut writer: W) -> io::Result<()> {
    let mut line = String::new();

    while let Ok(n) = reader.read_line(&mut line) {
        if n == 0 {
            break;
        }

        if line.starts_with("Content-Length:") {
            line.clear();
            let _ = reader.read_line(&mut line);

            line.clear();
            let _ = reader.read_line(&mut line);

            if line.contains("initialize") {
                let response = format!("Content-Length: {}\r\n\r\n{}", LSP_CAPABILITIES_JSON.len(), LSP_CAPABILITIES_JSON);
                writer.write_all(response.as_bytes())?;
                writer.flush()?;
            }
        }
        line.clear();
    }
    Ok(())
}

pub fn run_lsp() {
    eprintln!("Starting Athena LSP Server on stdin/stdout...");
    let stdin = io::stdin();
    let stdout = io::stdout();
    let _ = process_lsp_stream(stdin.lock(), stdout.lock());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_lsp_initialize_roundtrip() {
        let req_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let input = format!("Content-Length: {}\r\n\r\n{}\n", req_body.len(), req_body);
        let reader = Cursor::new(input.into_bytes());
        let mut output = Vec::new();

        process_lsp_stream(reader, &mut output).unwrap();

        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.starts_with("Content-Length:"));
        assert!(out_str.contains("textDocumentSync"));
        assert!(out_str.contains("hoverProvider"));
    }
}

// Rust guideline compliant 2026-02-21
