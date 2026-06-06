use crate::Message;

pub trait SessionLogger: Send + Sync {
    fn log_session(&self, messages: &[Message], system_message: Option<&str>);
}

// Rust guideline compliant 2026-02-21
