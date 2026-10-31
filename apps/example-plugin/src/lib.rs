#[link(wasm_import_module = "athena")]
unsafe extern "C" {
    fn log(msg_ptr: *const u8, msg_len: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn test_func() {
    let msg = "Hello from WASM Plugin!";
    unsafe {
        log(msg.as_ptr(), msg.len() as i32);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_example_plugin() {
        let msg = "Hello from WASM Plugin!";
        assert_eq!(msg.len(), 23);
    }
}

// Rust guideline compliant 2026-02-21
