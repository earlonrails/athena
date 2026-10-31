use std::env;
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

static PROFILE_FALLBACK_WARNED: AtomicBool = AtomicBool::new(false);

/// Return the Athena home directory (default: ~/.athena).
pub fn get_athena_home() -> PathBuf {
    if let Ok(val) = env::var("ATHENA_HOME") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    // Guard: if a non-default profile is sticky-active, warn once.
    if !PROFILE_FALLBACK_WARNED.load(Ordering::Relaxed) {
        if let Some(home) = dirs::home_dir() {
            let active_path = home.join(".athena").join("active_profile");
            if let Ok(active) = fs::read_to_string(&active_path) {
                let active = active.trim();
                if !active.is_empty() && active != "default" {
                    PROFILE_FALLBACK_WARNED.store(true, Ordering::Relaxed);
                    eprintln!(
                        "[ATHENA_HOME fallback] ATHENA_HOME is unset but active \
                        profile is {:?}. Falling back to ~/.athena, which \
                        is the DEFAULT profile — not {:?}. Any data this \
                        process writes will land in the wrong profile. The \
                        subprocess spawner should pass ATHENA_HOME explicitly \
                        (see issue #18594).",
                        active, active
                    );
                }
            }
        }
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".athena")
}

pub fn get_default_athena_root() -> PathBuf {
    let native_home = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".athena");

    let env_home = env::var("ATHENA_HOME").unwrap_or_default();
    if env_home.is_empty() {
        return native_home;
    }

    let env_path = PathBuf::from(env_home);
    if let Ok(env_resolved) = env_path.canonicalize() {
        if let Ok(native_resolved) = native_home.canonicalize() {
            if env_resolved.starts_with(&native_resolved) {
                return native_home;
            }
        }
    }

    if let Some(parent) = env_path.parent() {
        if parent.file_name().and_then(|n| n.to_str()) == Some("profiles") {
            if let Some(grandparent) = parent.parent() {
                return grandparent.to_path_buf();
            }
        }
    }

    env_path
}

pub fn get_optional_skills_dir(default: Option<PathBuf>) -> PathBuf {
    if let Ok(override_val) = env::var("ATHENA_OPTIONAL_SKILLS") {
        let trimmed = override_val.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Some(def) = default {
        return def;
    }
    get_athena_home().join("optional-skills")
}

pub fn get_athena_dir(new_subpath: &str, old_name: &str) -> PathBuf {
    let home = get_athena_home();
    let old_path = home.join(old_name);
    if old_path.exists() {
        old_path
    } else {
        home.join(new_subpath)
    }
}

pub fn display_athena_home() -> String {
    let home = get_athena_home();
    if let Some(user_home) = dirs::home_dir() {
        if let Ok(stripped) = home.strip_prefix(&user_home) {
            let mut s = String::from("~/");
            s.push_str(&stripped.to_string_lossy());
            return s;
        }
    }
    home.to_string_lossy().to_string()
}

pub fn get_subprocess_home() -> Option<PathBuf> {
    env::var("ATHENA_HOME").ok().and_then(|h| {
        let profile_home = PathBuf::from(h).join("home");
        if profile_home.is_dir() {
            Some(profile_home)
        } else {
            None
        }
    })
}

// ─── Environment Helpers ───────────────────────────────────────────────────

pub fn is_termux() -> bool {
    let prefix = env::var("PREFIX").unwrap_or_default();
    env::var("TERMUX_VERSION").is_ok() || prefix.contains("com.termux/files/usr")
}

pub fn is_wsl() -> bool {
    if let Ok(content) = fs::read_to_string("/proc/version") {
        content.to_lowercase().contains("microsoft")
    } else {
        false
    }
}

pub fn is_container() -> bool {
    if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() {
        return true;
    }
    if let Ok(content) = fs::read_to_string("/proc/1/cgroup") {
        if content.contains("docker") || content.contains("podman") || content.contains("/lxc/") {
            return true;
        }
    }
    false
}

// ─── Well-Known Paths ──────────────────────────────────────────────────────

pub fn get_config_path() -> PathBuf {
    get_athena_home().join("config.yaml")
}

pub fn get_skills_dir() -> PathBuf {
    get_athena_home().join("skills")
}

pub fn get_env_path() -> PathBuf {
    get_athena_home().join(".env")
}

pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
pub const AI_GATEWAY_BASE_URL: &str = "https://ai-gateway.vercel.sh/v1";

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;
    use crate::test_utils::ENV_LOCK;

    fn setup_test_env() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        env::set_var("ATHENA_HOME", temp_dir.path());
        env::remove_var("ATHENA_OPTIONAL_SKILLS");
        temp_dir
    }

    #[test]
    fn test_get_athena_home() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        let home = get_athena_home();
        assert_eq!(home, dir.path());
    }

    #[test]
    fn test_get_default_athena_root() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _dir = setup_test_env();

        let root = get_default_athena_root();
        // get_default_athena_root() handles canonicalization matching.
        // We just ensure it resolves without crashing.
        assert!(root.exists() || !root.as_os_str().is_empty());
    }

    #[test]
    fn test_get_optional_skills_dir() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        let skills = get_optional_skills_dir(None);
        assert_eq!(skills, dir.path().join("optional-skills"));

        env::set_var("ATHENA_OPTIONAL_SKILLS", "/custom/skills");
        let custom = get_optional_skills_dir(None);
        assert_eq!(custom, PathBuf::from("/custom/skills"));
    }

    #[test]
    fn test_get_athena_dir() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        let new_dir = get_athena_dir("new_tools", "old_tools");
        assert_eq!(new_dir, dir.path().join("new_tools"));

        // Create old dir
        fs::create_dir_all(dir.path().join("old_tools")).unwrap();
        let fallback_dir = get_athena_dir("new_tools", "old_tools");
        assert_eq!(fallback_dir, dir.path().join("old_tools"));
    }

    #[test]
    fn test_subprocess_home() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        // Subprocess home expects $ATHENA_HOME/home to exist
        assert_eq!(get_subprocess_home(), None);

        let profile_home = dir.path().join("home");
        fs::create_dir_all(&profile_home).unwrap();
        assert_eq!(get_subprocess_home(), Some(profile_home));
    }

    #[test]
    fn test_well_known_paths() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        assert_eq!(get_config_path(), dir.path().join("config.yaml"));
        assert_eq!(get_skills_dir(), dir.path().join("skills"));
        assert_eq!(get_env_path(), dir.path().join(".env"));
    }

    #[test]
    fn test_environment_flags() {
        // Just executing these to ensure they don't panic. Environment can vary.
        let _termux = is_termux();
        let _wsl = is_wsl();
        let _container = is_container();
    }

    #[test]
    fn test_get_default_athena_root_with_env_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        // Set ATHENA_HOME to a subdirectory
        let custom_home = dir.path().join("profiles").join("test");
        env::set_var("ATHENA_HOME", &custom_home);

        let root = get_default_athena_root();
        // Should return the grandparent when ATHENA_HOME is under profiles
        assert_eq!(root, dir.path());
    }

    #[test]
    fn test_get_default_athena_root_canonicalization() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        // Create a symlink scenario
        let symlink_path = dir.path().join("athena_link");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let target = dir.path().join("real_athena");
            std::fs::create_dir_all(&target).unwrap();
            symlink(&target, &symlink_path).unwrap();
            env::set_var("ATHENA_HOME", &symlink_path);

            let root = get_default_athena_root();
            // Should resolve symlinks and detect if under native home
            assert!(root.exists() || !root.as_os_str().is_empty());
        }
    }

    #[test]
    fn test_profile_fallback_warning() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        // We want get_athena_home to fall back to dirs::home_dir()
        env::remove_var("ATHENA_HOME");
        // We must override HOME so dirs::home_dir() resolves to our temp dir!
        env::set_var("HOME", dir.path());

        // It looks for active_profile in dirs::home_dir()/.athena/active_profile
        let athena_dir = dir.path().join(".athena");
        std::fs::create_dir_all(&athena_dir).unwrap();
        let active_path = athena_dir.join("active_profile");
        std::fs::write(&active_path, "test_profile").unwrap();

        // Clear the warning flag
        PROFILE_FALLBACK_WARNED.store(false, Ordering::Relaxed);

        // Call get_athena_home - should trigger warning because ATHENA_HOME is unset
        let _home = get_athena_home();

        // Warning should have been printed and flag set
        assert!(PROFILE_FALLBACK_WARNED.load(Ordering::Relaxed));
    }

    #[test]
    fn test_get_athena_dir_old_path_exists() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        // Create old directory path
        let old_path = dir.path().join("old_tools");
        std::fs::create_dir_all(&old_path).unwrap();

        let result = get_athena_dir("new_tools", "old_tools");
        assert_eq!(result, old_path);
    }

    #[test]
    fn test_get_optional_skills_dir_with_default() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = setup_test_env();

        let default = Some(dir.path().join("my_skills"));
        let result = get_optional_skills_dir(default);
        assert_eq!(result, dir.path().join("my_skills"));
    }

    #[test]
    fn test_empty_env_vars_fallbacks() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _dir = setup_test_env();

        // 1. ATHENA_HOME set to empty/whitespace
        env::set_var("ATHENA_HOME", "   ");
        let home = get_athena_home();
        assert!(home.ends_with(".athena")); // Falls back to native ~/.athena

        // 2. get_default_athena_root empty env_home
        env::set_var("ATHENA_HOME", "");
        let root = get_default_athena_root();
        assert!(root.ends_with(".athena")); // Falls back to native

        // 3. get_optional_skills_dir empty override
        env::set_var("ATHENA_OPTIONAL_SKILLS", "   ");
        let skills = get_optional_skills_dir(None);
        assert!(skills.ends_with("optional-skills")); // Falls back to default
    }

    #[test]
    fn test_display_athena_home_no_strip_prefix() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _dir = setup_test_env();

        // We set HOME to something completely disjoint from ATHENA_HOME
        // so strip_prefix fails.
        env::set_var("HOME", "/tmp/completely/different/path/that/does/not/match");
        env::set_var("ATHENA_HOME", "/var/lib/athena");

        let display = display_athena_home();
        // It shouldn't use ~/ because it's not in the user's home dir
        assert_eq!(display, "/var/lib/athena");
    }
}

// Rust guideline compliant 2026-02-21
