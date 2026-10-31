use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

pub static ENV_LOCK: Mutex<()> = Mutex::new(());

pub struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    orig_home: Option<OsString>,
    orig_cwd: Option<PathBuf>,
}

impl EnvGuard {
    pub fn new() -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let orig_home = std::env::var_os("ATHENA_HOME");
        let orig_cwd = std::env::current_dir().ok();
        Self {
            _lock: lock,
            orig_home,
            orig_cwd,
        }
    }

    pub fn set_home(&self, path: impl AsRef<Path>) {
        std::env::set_var("ATHENA_HOME", path.as_ref());
    }

    pub fn set_cwd(&self, path: impl AsRef<Path>) {
        let _ = std::env::set_current_dir(path.as_ref());
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(ref cwd) = self.orig_cwd {
            let _ = std::env::set_current_dir(cwd);
        }
        match &self.orig_home {
            Some(home) => std::env::set_var("ATHENA_HOME", home),
            None => std::env::remove_var("ATHENA_HOME"),
        }
    }
}

// Rust guideline compliant 2026-02-21
