pub fn run_version() {
    let version = env!("CARGO_PKG_VERSION");
    let authors = env!("CARGO_PKG_AUTHORS");
    println!("Athena Agent v{} (Rust rewrite)", version);
    if !authors.is_empty() {
        println!("Authors: {}", authors);
    }
    println!("Inspired by the original Python Athena Agent from Nous Research.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_runs() {
        run_version();
    }
}

// Rust guideline compliant 2026-02-21
