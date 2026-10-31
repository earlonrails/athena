use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use athena_core::config::{load_config, save_config, AthenaConfig, CronJob};
use athena_core::paths::get_athena_home;
use serde_json::Value as JsonValue;

/// Locate the Hermes home directory:
/// 1. Explicit override passed as argument
/// 2. HERMES_HOME environment variable
/// 3. Standard fallback ~/.hermes
pub fn resolve_hermes_home(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(env_val) = std::env::var("HERMES_HOME") {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.exists() {
                return Some(p);
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let hermes_dir = home.join(".hermes");
        if hermes_dir.exists() {
            return Some(hermes_dir);
        }
    }

    None
}

/// Convert relative or natural language intervals into valid 5-part cron syntax.
pub fn parse_schedule_str(raw: &str) -> String {
    let trimmed = raw.trim().to_lowercase();
    match trimmed.as_str() {
        "daily" | "every day" | "everyday" => "0 0 * * *".to_string(),
        "hourly" | "every hour" => "0 * * * *".to_string(),
        "every 1h" | "every 1 hour" => "0 * * * *".to_string(),
        "every 2h" | "every 2 hours" => "0 */2 * * *".to_string(),
        "every 3h" | "every 3 hours" => "0 */3 * * *".to_string(),
        "every 4h" | "every 4 hours" => "0 */4 * * *".to_string(),
        "every 6h" | "every 6 hours" => "0 */6 * * *".to_string(),
        "every 8h" | "every 8 hours" => "0 */8 * * *".to_string(),
        "every 12h" | "every 12 hours" => "0 */12 * * *".to_string(),
        "every 30m" | "every 30 mins" | "every 30 minutes" => "*/30 * * * *".to_string(),
        "every 15m" | "every 15 mins" | "every 15 minutes" => "*/15 * * * *".to_string(),
        "every 10m" | "every 10 mins" | "every 10 minutes" => "*/10 * * * *".to_string(),
        "every 5m" | "every 5 mins" | "every 5 minutes" => "*/5 * * * *".to_string(),
        _ => raw.trim().to_string(),
    }
}

/// Parse delivery channels from Hermes format (e.g. "telegram:-10012345", ["discord"], etc.)
pub fn parse_delivery_target(deliver_val: &JsonValue) -> (Option<Vec<String>>, Option<String>, Option<String>) {
    match deliver_val {
        JsonValue::String(s) => {
            let s = s.trim();
            if let Some((proto, chan)) = s.split_once(':') {
                let proto_lower = proto.to_lowercase();
                (Some(vec![proto_lower]), Some(chan.to_string()), None)
            } else if !s.is_empty() {
                (Some(vec![s.to_lowercase()]), None, None)
            } else {
                (None, None, None)
            }
        }
        JsonValue::Array(arr) => {
            let vec: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                .collect();
            if vec.is_empty() {
                (None, None, None)
            } else {
                (Some(vec), None, None)
            }
        }
        _ => (None, None, None),
    }
}

/// Parse a single Hermes cron job definition from a JSON object.
pub fn parse_single_hermes_job(item: &JsonValue) -> Option<CronJob> {
    let query = item
        .get("prompt")
        .or_else(|| item.get("query"))
        .or_else(|| item.get("name"))
        .and_then(|v| v.as_str())?
        .trim();

    if query.is_empty() {
        return None;
    }

    let schedule = match item.get("schedule") {
        Some(JsonValue::String(s)) => parse_schedule_str(s),
        Some(JsonValue::Object(map)) => {
            if let Some(expr) = map.get("expr").and_then(|v| v.as_str()) {
                parse_schedule_str(expr)
            } else if let Some(disp) = map.get("display").and_then(|v| v.as_str()) {
                parse_schedule_str(disp)
            } else {
                return None;
            }
        }
        _ => return None,
    };

    let mut delivery = None;
    let mut channel = item.get("channel").and_then(|v| v.as_str().map(String::from));
    let thread = item.get("thread").and_then(|v| {
        if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else if let Some(n) = v.as_i64() {
            Some(n.to_string())
        } else {
            None
        }
    });

    if let Some(deliv_val) = item.get("deliver").or_else(|| item.get("delivery")) {
        let (parsed_deliv, parsed_chan, _) = parse_delivery_target(deliv_val);
        if parsed_deliv.is_some() {
            delivery = parsed_deliv;
        }
        if channel.is_none() && parsed_chan.is_some() {
            channel = parsed_chan;
        }
    }

    Some(CronJob {
        schedule,
        query: query.to_string(),
        channel,
        thread,
        delivery,
    })
}

/// Extract all cron jobs from parsed JSON (supports Array, { "jobs": [...] }, or keyed map).
pub fn extract_cron_jobs_from_json(val: &JsonValue) -> Vec<CronJob> {
    let mut jobs = Vec::new();

    if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(job) = parse_single_hermes_job(item) {
                jobs.push(job);
            }
        }
    } else if let Some(obj) = val.as_object() {
        if let Some(jobs_arr) = obj.get("jobs").and_then(|j| j.as_array()) {
            for item in jobs_arr {
                if let Some(job) = parse_single_hermes_job(item) {
                    jobs.push(job);
                }
            }
        } else {
            for (_key, item) in obj {
                if item.is_object() {
                    if let Some(job) = parse_single_hermes_job(item) {
                        jobs.push(job);
                    }
                }
            }
        }
    }

    jobs
}

/// Check if a file should be skipped during configuration copying (e.g. database, logs, locks).
pub fn should_skip_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Database files — explicit user instruction: do not copy databases
    lower.ends_with(".db")
        || lower.ends_with(".db-wal")
        || lower.ends_with(".db-shm")
        || lower.ends_with(".sqlite")
        || lower.ends_with(".sqlite3")
        // Logs
        || lower.starts_with("logs")
        || lower.ends_with(".log")
        || lower.contains(".log.")
        // Locks & runtime
        || lower.ends_with(".lock")
        || lower == ".git"
        || lower == "cache"
        || lower == "tmp"
        || lower == "sessions"
}

/// Merge lines from a Hermes .env file into Athena .env without clobbering existing Athena keys.
pub fn merge_env_files(hermes_env: &Path, athena_env: &Path) -> Result<usize> {
    let hermes_content = fs::read_to_string(hermes_env)
        .with_context(|| format!("Failed to read {}", hermes_env.display()))?;
    let mut added_count = 0;

    let mut existing_keys = HashSet::new();
    let existing_content = if athena_env.exists() {
        let content = fs::read_to_string(athena_env)?;
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                if let Some((k, _)) = trimmed.split_once('=') {
                    existing_keys.insert(k.trim().to_string());
                }
            }
        }
        content
    } else {
        String::new()
    };

    let mut to_append = String::new();
    let mut google_key_val = None;
    let mut gemini_key_val = None;

    for line in hermes_content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some((k, v)) = trimmed.split_once('=') {
                let k_trim = k.trim();
                let v_trim = v.trim();
                if k_trim == "GOOGLE_API_KEY" && !v_trim.is_empty() {
                    google_key_val = Some(v_trim.to_string());
                } else if k_trim == "GEMINI_API_KEY" && !v_trim.is_empty() {
                    gemini_key_val = Some(v_trim.to_string());
                }
                if !existing_keys.contains(k_trim) {
                    to_append.push_str(line);
                    to_append.push('\n');
                    existing_keys.insert(k_trim.to_string());
                    added_count += 1;
                }
            }
        }
    }

    // Alias GEMINI_API_KEY <-> GOOGLE_API_KEY if one exists and the other doesn't
    if let Some(val) = google_key_val {
        if !existing_keys.contains("GEMINI_API_KEY") {
            to_append.push_str(&format!("GEMINI_API_KEY={}\n", val));
            existing_keys.insert("GEMINI_API_KEY".to_string());
            added_count += 1;
        }
    } else if let Some(val) = gemini_key_val {
        if !existing_keys.contains("GOOGLE_API_KEY") {
            to_append.push_str(&format!("GOOGLE_API_KEY={}\n", val));
            existing_keys.insert("GOOGLE_API_KEY".to_string());
            added_count += 1;
        }
    }

    if !to_append.is_empty() {
        let mut final_content = existing_content;
        if !final_content.is_empty() && !final_content.ends_with('\n') {
            final_content.push('\n');
        }
        final_content.push_str(&to_append);
        fs::write(athena_env, final_content)?;
    }

    Ok(added_count)
}

/// Recursively copy directory contents, preserving symlinks and handling nested directory symlinks.
pub fn copy_dir_all(src: &Path, dst: &Path, force: bool) -> io::Result<usize> {
    let mut copied = 0;
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let from = entry.path();
        let to = dst.join(entry.file_name());

        let is_symlink = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        if is_symlink {
            #[cfg(unix)]
            {
                if force && (to.exists() || to.is_symlink()) {
                    let _ = fs::remove_file(&to);
                    let _ = fs::remove_dir_all(&to);
                }
                if !to.exists() && !to.is_symlink() {
                    if let Ok(target) = fs::read_link(&from) {
                        if std::os::unix::fs::symlink(&target, &to).is_ok() {
                            copied += 1;
                            continue;
                        }
                    }
                }
            }
            // If symlink points to directory and could not be symlinked, copy dir contents
            if from.is_dir() {
                copied += copy_dir_all(&from, &to, force).unwrap_or(0);
            } else if (force || !to.exists()) && fs::copy(&from, &to).is_ok() {
                copied += 1;
            }
        } else if from.is_dir() {
            copied += copy_dir_all(&from, &to, force).unwrap_or(0);
        } else {
            if force || !to.exists() {
                if fs::copy(&from, &to).is_ok() {
                    copied += 1;
                }
            }
        }
    }
    Ok(copied)
}

/// Parse and merge Hermes YAML into AthenaConfig, handling both modern Athena format
/// and legacy Hermes Agent format (e.g. object-based fallback_providers, api instead of base_url, etc.)
pub fn merge_hermes_yaml(hermes_yaml: &str, athena_config: &mut AthenaConfig, force: bool) -> Result<bool> {
    // 1. Try standard AthenaConfig deserialization first
    if let Ok(hermes_cfg) = serde_yaml::from_str::<AthenaConfig>(hermes_yaml) {
        if !hermes_cfg.model.default.is_empty() && (athena_config.model.default.is_empty() || force) {
            athena_config.model = hermes_cfg.model;
        }
        for (name, prov) in hermes_cfg.providers {
            if force || !athena_config.providers.contains_key(&name) {
                athena_config.providers.insert(name, prov);
            }
        }
        for fb in hermes_cfg.fallback_providers {
            if !athena_config.fallback_providers.contains(&fb) {
                athena_config.fallback_providers.push(fb);
            }
        }
        if hermes_cfg.agent.system_prompt.is_some() && (athena_config.agent.system_prompt.is_none() || force) {
            athena_config.agent.system_prompt = hermes_cfg.agent.system_prompt;
        }
        if hermes_cfg.agent.yolo_mode {
            athena_config.agent.yolo_mode = true;
        }
        if hermes_cfg.gateway.telegram_enabled {
            athena_config.gateway.telegram_enabled = true;
        }
        if hermes_cfg.gateway.discord_enabled {
            athena_config.gateway.discord_enabled = true;
        }
        for job in hermes_cfg.cron_jobs {
            if !athena_config.cron_jobs.contains(&job) {
                athena_config.cron_jobs.push(job);
            }
        }
        return Ok(true);
    }

    // 2. Flexible legacy Hermes Agent format
    let root: serde_yaml::Value = serde_yaml::from_str(hermes_yaml)
        .with_context(|| "Failed to parse YAML structure")?;

    let mut changed = false;

    // Model
    if let Some(model_val) = root.get("model") {
        if let Some(def) = model_val.get("default").and_then(|v| v.as_str()) {
            if !def.is_empty() && (athena_config.model.default.is_empty() || force) {
                athena_config.model.default = def.to_string();
                changed = true;
            }
        }
        if let Some(prov) = model_val.get("provider").and_then(|v| v.as_str()) {
            if !prov.is_empty() && (athena_config.model.provider.is_empty() || force) {
                athena_config.model.provider = prov.to_string();
                changed = true;
            }
        }
        if let Some(base_url) = model_val.get("base_url").and_then(|v| v.as_str()) {
            let prov_name = if !athena_config.model.provider.is_empty() {
                athena_config.model.provider.clone()
            } else {
                "default".to_string()
            };
            if force || !athena_config.providers.contains_key(&prov_name) {
                athena_config.providers.insert(prov_name.clone(), athena_core::config::ProviderConfig {
                    name: prov_name,
                    base_url: Some(base_url.to_string()),
                    default_model: Some(athena_config.model.default.clone()),
                    api_key: None,
                });
                changed = true;
            }
        }
    }

    // Providers
    if let Some(providers_val) = root.get("providers").and_then(|v| v.as_mapping()) {
        for (k, v) in providers_val {
            if let Some(name) = k.as_str() {
                if force || !athena_config.providers.contains_key(name) {
                    let base_url = v.get("base_url")
                        .or_else(|| v.get("api"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    let default_model = v.get("default_model")
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    let api_key = v.get("api_key")
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    athena_config.providers.insert(name.to_string(), athena_core::config::ProviderConfig {
                        name: name.to_string(),
                        base_url,
                        default_model,
                        api_key,
                    });
                    changed = true;
                }
            }
        }
    }

    // Fallback providers (handles list of strings or list of objects e.g. [{provider: "...", model: "..."}])
    if let Some(fallbacks_val) = root.get("fallback_providers").and_then(|v| v.as_sequence()) {
        for item in fallbacks_val {
            let prov_name = if let Some(s) = item.as_str() {
                Some(s.to_string())
            } else if let Some(map) = item.as_mapping() {
                map.get(&serde_yaml::Value::String("provider".to_string()))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };
            if let Some(name) = prov_name {
                if !athena_config.fallback_providers.contains(&name) {
                    athena_config.fallback_providers.push(name);
                    changed = true;
                }
            }
        }
    }

    // Agent
    if let Some(agent_val) = root.get("agent") {
        if let Some(prompt) = agent_val.get("system_prompt").and_then(|v| v.as_str()) {
            if athena_config.agent.system_prompt.is_none() || force {
                athena_config.agent.system_prompt = Some(prompt.to_string());
                changed = true;
            }
        }
        if let Some(yolo) = agent_val.get("yolo_mode").and_then(|v| v.as_bool()) {
            if yolo {
                athena_config.agent.yolo_mode = true;
                changed = true;
            }
        }
        if let Some(turns) = agent_val.get("max_iterations")
            .or_else(|| agent_val.get("max_turns"))
            .and_then(|v| v.as_u64()) {
            if force || athena_config.agent.max_iterations == 20 {
                athena_config.agent.max_iterations = turns as u32;
                changed = true;
            }
        }
    }

    // Terminal
    if let Some(term_val) = root.get("terminal") {
        if let Some(backend) = term_val.get("backend").and_then(|v| v.as_str()) {
            if force || athena_config.terminal_backend == "local" {
                athena_config.terminal_backend = backend.to_string();
                changed = true;
            }
        }
    }

    // Gateway
    if let Some(gw_val) = root.get("gateway") {
        if let Some(tg) = gw_val.get("telegram_enabled").and_then(|v| v.as_bool()) {
            if tg {
                athena_config.gateway.telegram_enabled = true;
                changed = true;
            }
        }
        if let Some(dc) = gw_val.get("discord_enabled").and_then(|v| v.as_bool()) {
            if dc {
                athena_config.gateway.discord_enabled = true;
                changed = true;
            }
        }
    }

    Ok(changed)
}

/// Core migration runner: copies configuration and crons from Hermes to Athena.
pub fn run_hermes(explicit_source: Option<PathBuf>, force: bool) -> Result<()> {
    println!("\nAthena Hermes Migrator");
    println!("══════════════════════════\n");
    println!("Seamlessly migrating configurations and scheduled jobs from Hermes Agent to Athena.\n");

    let hermes_home = match resolve_hermes_home(explicit_source.clone()) {
        Some(path) => path,
        None => {
            if let Some(ref p) = explicit_source {
                println!("✗ Specified Hermes directory does not exist: {}", p.display());
            } else {
                println!("✗ Could not locate Hermes installation directory.");
                println!("  Expected ~/.hermes or $HERMES_HOME. You can pass --source <path> to specify it manually.");
            }
            return Ok(());
        }
    };

    let athena_home = get_athena_home();
    if !athena_home.exists() {
        fs::create_dir_all(&athena_home)
            .with_context(|| format!("Failed to create Athena home: {}", athena_home.display()))?;
    }

    println!("Source (Hermes):  {}", hermes_home.display());
    println!("Target (Athena):  {}\n", athena_home.display());

    println!("─── [1/2] Migrating Configuration Files ───");

    // 1. config.yaml
    let hermes_config_path = hermes_home.join("config.yaml");
    let athena_config_path = athena_home.join("config.yaml");

    let mut athena_config = load_config();
    let mut config_migrated = false;

    if hermes_config_path.exists() {
        match fs::read_to_string(&hermes_config_path) {
            Ok(hermes_yaml) => {
                // Backup existing athena config if present
                if athena_config_path.exists() {
                    let bak_path = athena_home.join("config.yaml.bak");
                    let _ = fs::copy(&athena_config_path, &bak_path);
                }

                match merge_hermes_yaml(&hermes_yaml, &mut athena_config, force) {
                    Ok(changed) => {
                        config_migrated = changed;
                        println!("✓ config.yaml: Merged model, provider, and agent settings.");
                    }
                    Err(e) => {
                        println!("⚠ config.yaml: Could not parse legacy YAML format ({}), skipped.", e);
                    }
                }
            }
            Err(e) => {
                println!("✗ config.yaml: Failed to read Hermes config: {}", e);
            }
        }
    } else {
        println!("- config.yaml: Not present in Hermes installation.");
    }

    // 2. .env
    let hermes_env = hermes_home.join(".env");
    let athena_env = athena_home.join(".env");
    if hermes_env.exists() {
        match merge_env_files(&hermes_env, &athena_env) {
            Ok(count) => {
                println!("✓ .env: Migrated {} environment variable(s) / API keys.", count);
            }
            Err(e) => {
                println!("✗ .env: Failed to migrate environment file: {}", e);
            }
        }
    } else {
        println!("- .env: Not present in Hermes installation.");
    }

    // 3. Custom skills
    let hermes_skills = hermes_home.join("skills");
    let athena_skills = athena_home.join("skills");
    if hermes_skills.exists() && hermes_skills.is_dir() {
        match copy_dir_all(&hermes_skills, &athena_skills, force) {
            Ok(count) => {
                println!("✓ skills/: Copied {} skill file(s) into ~/.athena/skills/.", count);
            }
            Err(e) => {
                println!("✗ skills/: Failed to copy skills: {}", e);
            }
        }
    } else {
        println!("- skills/: No custom skills directory found in Hermes.");
    }

    // 4. Other configuration files in root (mcp.json, auth.json, etc.)
    if let Ok(entries) = fs::read_dir(&hermes_home) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip known paths handled above, or database/log/locks
            if file_name == "config.yaml"
                || file_name == ".env"
                || file_name == "skills"
                || file_name == "cron"
                || should_skip_file(&file_name)
            {
                continue;
            }

            if path.is_file() {
                // Copy additional configuration files (json, yaml, toml, txt)
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "json" | "yaml" | "yml" | "toml" | "txt") {
                    let dest = athena_home.join(&file_name);
                    if force || !dest.exists() {
                        if fs::copy(&path, &dest).is_ok() {
                            println!("✓ {}: Copied to ~/.athena/{}.", file_name, file_name);
                        }
                    }
                }
            }
        }
    }

    println!("↷ Database (state.db) preserved and skipped as intended.\n");

    println!("─── [2/2] Migrating Scheduled Cron Jobs ───");

    let mut discovered_crons = Vec::new();

    // Check hermes_home/cron/jobs.json
    let cron_jobs_path = hermes_home.join("cron").join("jobs.json");
    let fallback_jobs_path = hermes_home.join("jobs.json");

    let target_json = if cron_jobs_path.exists() {
        Some(cron_jobs_path)
    } else if fallback_jobs_path.exists() {
        Some(fallback_jobs_path)
    } else {
        None
    };

    if let Some(json_path) = target_json {
        match fs::read_to_string(&json_path) {
            Ok(content) => {
                match serde_json::from_str::<JsonValue>(&content) {
                    Ok(val) => {
                        let parsed = extract_cron_jobs_from_json(&val);
                        println!("✓ Found {} cron job(s) in {}.", parsed.len(), json_path.display());
                        discovered_crons.extend(parsed);
                    }
                    Err(e) => {
                        println!("✗ Failed to parse JSON in {}: {}", json_path.display(), e);
                    }
                }
            }
            Err(e) => {
                println!("✗ Failed to read {}: {}", json_path.display(), e);
            }
        }
    }

    // Merge discovered cron jobs into athena_config
    let initial_count = athena_config.cron_jobs.len();
    let mut added_crons = 0;

    for job in discovered_crons {
        // Prevent duplicate jobs with same schedule and query
        let already_exists = athena_config
            .cron_jobs
            .iter()
            .any(|existing| existing.schedule == job.schedule && existing.query == job.query);

        if !already_exists {
            let chan_info = job
                .channel
                .as_ref()
                .map(|c| format!(" [Channel: {}]", c))
                .unwrap_or_default();
            println!("  + '{}' -> '{}'{}", job.schedule, job.query, chan_info);
            athena_config.cron_jobs.push(job);
            added_crons += 1;
        }
    }

    if added_crons > 0 || config_migrated {
        match save_config(&athena_config) {
            Ok(()) => {
                println!(
                    "✓ Successfully saved {} new cron job(s) (total {}) to ~/.athena/config.yaml.",
                    added_crons,
                    athena_config.cron_jobs.len()
                );
            }
            Err(e) => {
                println!("✗ Failed to save updated configuration: {}", e);
            }
        }
    } else if initial_count > 0 {
        println!("- No new cron jobs to add (all discovered jobs already exist).");
    } else {
        println!("- No scheduled cron jobs found in Hermes.");
    }

    println!("\n══════════════════════════════════════════════");
    println!("✓ Migration completed successfully!");
    println!("  Run `athena cron` to manage scheduled jobs.");
    println!("  Run `athena` to start the interactive agent.\n");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_schedule_str() {
        assert_eq!(parse_schedule_str("0 9 * * *"), "0 9 * * *");
        assert_eq!(parse_schedule_str("daily"), "0 0 * * *");
        assert_eq!(parse_schedule_str("hourly"), "0 * * * *");
        assert_eq!(parse_schedule_str("every 2h"), "0 */2 * * *");
        assert_eq!(parse_schedule_str("every 30m"), "*/30 * * * *");
        assert_eq!(parse_schedule_str("every 10 minutes"), "*/10 * * * *");
    }

    #[test]
    fn test_parse_delivery_target() {
        let v1 = serde_json::json!("telegram:-100123456789");
        let (proto, chan, thread) = parse_delivery_target(&v1);
        assert_eq!(proto, Some(vec!["telegram".to_string()]));
        assert_eq!(chan, Some("-100123456789".to_string()));
        assert_eq!(thread, None);

        let v2 = serde_json::json!(["slack", "discord"]);
        let (proto2, chan2, thread2) = parse_delivery_target(&v2);
        assert_eq!(proto2, Some(vec!["slack".to_string(), "discord".to_string()]));
        assert_eq!(chan2, None);
        assert_eq!(thread2, None);
    }

    #[test]
    fn test_extract_cron_jobs_array() {
        let json_data = serde_json::json!([
            {
                "id": "job1",
                "name": "Check server",
                "prompt": "check server health",
                "schedule": "0 9 * * *",
                "deliver": "telegram:-100123456"
            },
            {
                "id": "job2",
                "prompt": "clean cache",
                "schedule": {
                    "kind": "cron",
                    "expr": "every 2h"
                }
            }
        ]);

        let jobs = extract_cron_jobs_from_json(&json_data);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].query, "check server health");
        assert_eq!(jobs[0].schedule, "0 9 * * *");
        assert_eq!(jobs[0].channel, Some("-100123456".to_string()));
        assert_eq!(jobs[0].delivery, Some(vec!["telegram".to_string()]));

        assert_eq!(jobs[1].query, "clean cache");
        assert_eq!(jobs[1].schedule, "0 */2 * * *");
    }

    #[test]
    fn test_extract_cron_jobs_object_format() {
        let json_data = serde_json::json!({
            "jobs": [
                {
                    "prompt": "morning briefing",
                    "schedule": "0 8 * * *"
                }
            ]
        });

        let jobs = extract_cron_jobs_from_json(&json_data);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].query, "morning briefing");
        assert_eq!(jobs[0].schedule, "0 8 * * *");
    }

    #[test]
    fn test_should_skip_database_and_logs() {
        assert!(should_skip_file("state.db"));
        assert!(should_skip_file("state.db-wal"));
        assert!(should_skip_file("memory.sqlite"));
        assert!(should_skip_file("logs"));
        assert!(should_skip_file("agent.log"));
        assert!(should_skip_file("agent.log.2026-05-16"));
        assert!(should_skip_file(".tick.lock"));

        assert!(!should_skip_file("config.yaml"));
        assert!(!should_skip_file(".env"));
        assert!(!should_skip_file("mcp.json"));
    }

    #[test]
    fn test_merge_env_files() {
        let temp_dir = TempDir::new().unwrap();
        let hermes_env = temp_dir.path().join("hermes.env");
        let athena_env = temp_dir.path().join("athena.env");

        fs::write(&hermes_env, "OPENAI_API_KEY=hermes-key\nNEW_KEY=123\n# A comment\n").unwrap();
        fs::write(&athena_env, "OPENAI_API_KEY=athena-key\nEXISTING=456\n").unwrap();

        let count = merge_env_files(&hermes_env, &athena_env).unwrap();
        assert_eq!(count, 1); // Only NEW_KEY added

        let merged = fs::read_to_string(&athena_env).unwrap();
        assert!(merged.contains("OPENAI_API_KEY=athena-key")); // Preserved existing
        assert!(merged.contains("NEW_KEY=123")); // Added new
        assert!(merged.contains("EXISTING=456")); // Preserved existing
    }

    #[test]
    fn test_end_to_end_hermes_migration() {
        let guard = athena_core::test_utils::EnvGuard::new();
        let temp = TempDir::new().unwrap();
        let hermes_dir = temp.path().join(".hermes");
        let athena_dir = temp.path().join(".athena");
        fs::create_dir_all(&hermes_dir).unwrap();
        fs::create_dir_all(&athena_dir).unwrap();

        guard.set_home(&athena_dir);

        // 1. Setup Hermes config.yaml
        let hermes_config = r#"
model:
  default: claude-3-5-sonnet-latest
  provider: anthropic
agent:
  max_iterations: 35
  yolo_mode: true
gateway:
  telegram_enabled: true
"#;
        fs::write(hermes_dir.join("config.yaml"), hermes_config).unwrap();

        // 2. Setup Hermes .env
        fs::write(
            hermes_dir.join(".env"),
            "ANTHROPIC_API_KEY=sk-ant-test\nCUSTOM_TOKEN=xyz\n",
        )
        .unwrap();

        // 3. Setup Hermes skills
        let skill_dir = hermes_dir.join("skills").join("news-fetcher");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# News Fetcher Skill").unwrap();

        // 4. Setup Hermes mcp.json
        fs::write(hermes_dir.join("mcp.json"), "{\"mcpServers\":{}}").unwrap();

        // 5. Setup Hermes crons in cron/jobs.json
        let cron_dir = hermes_dir.join("cron");
        fs::create_dir_all(&cron_dir).unwrap();
        let jobs_json = r#"[
            {
                "name": "Daily briefing",
                "prompt": "summarize daily news",
                "schedule": "0 7 * * *",
                "deliver": "telegram:-10099887766"
            },
            {
                "name": "Health check",
                "prompt": "ping backend servers",
                "schedule": {
                    "kind": "cron",
                    "expr": "every 30m"
                }
            }
        ]"#;
        fs::write(cron_dir.join("jobs.json"), jobs_json).unwrap();

        // 6. Setup files that MUST be skipped (database, logs)
        fs::write(hermes_dir.join("state.db"), b"SQLITE_DUMMY_DATABASE_CONTENT").unwrap();
        let logs_dir = hermes_dir.join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        fs::write(logs_dir.join("agent.log"), "2026-05-16 dummy log").unwrap();

        // Run migration!
        let result = run_hermes(Some(hermes_dir.clone()), false);
        assert!(result.is_ok());

        // Assertions on Athena directory:
        let athena_cfg = load_config();
        assert_eq!(athena_cfg.model.default, "claude-3-5-sonnet-latest");
        assert_eq!(athena_cfg.model.provider, "anthropic");
        assert!(athena_cfg.gateway.telegram_enabled);
        assert_eq!(athena_cfg.cron_jobs.len(), 2);
        assert_eq!(athena_cfg.cron_jobs[0].query, "summarize daily news");
        assert_eq!(athena_cfg.cron_jobs[0].schedule, "0 7 * * *");
        assert_eq!(
            athena_cfg.cron_jobs[0].channel,
            Some("-10099887766".to_string())
        );
        assert_eq!(
            athena_cfg.cron_jobs[0].delivery,
            Some(vec!["telegram".to_string()])
        );

        assert_eq!(athena_cfg.cron_jobs[1].query, "ping backend servers");
        assert_eq!(athena_cfg.cron_jobs[1].schedule, "*/30 * * * *");

        // .env was migrated
        let env_content = fs::read_to_string(athena_dir.join(".env")).unwrap();
        assert!(env_content.contains("ANTHROPIC_API_KEY=sk-ant-test"));
        assert!(env_content.contains("CUSTOM_TOKEN=xyz"));

        // Skills were copied
        assert!(athena_dir
            .join("skills")
            .join("news-fetcher")
            .join("SKILL.md")
            .exists());

        // MCP config copied
        assert!(athena_dir.join("mcp.json").exists());

        // Database and logs were NOT copied
        assert!(!athena_dir.join("state.db").exists());
        assert!(!athena_dir.join("logs").exists());
    }

    #[test]
    fn test_merge_legacy_hermes_yaml() {
        let legacy_yaml = r#"
model:
  default: gemini-3.6-flash
  provider: gemini
  base_url: 'https://generativelanguage.googleapis.com/v1beta'
fallback_providers:
  - provider: custom:mistral
    model: mistral-small-latest
  - provider: openrouter
    model: google/gemini-3.6-flash
terminal:
  backend: local
agent:
  max_turns: 90
  yolo_mode: true
gateway:
  telegram_enabled: true
"#;
        let mut cfg = AthenaConfig::default();
        let res = merge_hermes_yaml(legacy_yaml, &mut cfg, false);
        assert!(res.is_ok());
        assert!(res.unwrap());
        assert_eq!(cfg.model.default, "gemini-3.6-flash");
        assert_eq!(cfg.model.provider, "gemini");
        assert_eq!(cfg.agent.max_iterations, 90);
        assert!(cfg.agent.yolo_mode);
        assert!(cfg.gateway.telegram_enabled);
        assert!(cfg.fallback_providers.contains(&"custom:mistral".to_string()));
        assert!(cfg.fallback_providers.contains(&"openrouter".to_string()));
    }

    #[test]
    fn test_merge_env_files_google_gemini_aliasing() {
        let temp = TempDir::new().unwrap();
        let hermes_env = temp.path().join("hermes.env");
        let athena_env = temp.path().join("athena.env");

        fs::write(&hermes_env, "GOOGLE_API_KEY=AIzaSyTest123\nOTHER_VAR=abc\n").unwrap();
        let added = merge_env_files(&hermes_env, &athena_env).unwrap();
        assert_eq!(added, 3); // GOOGLE_API_KEY, OTHER_VAR, and GEMINI_API_KEY alias

        let content = fs::read_to_string(&athena_env).unwrap();
        assert!(content.contains("GOOGLE_API_KEY=AIzaSyTest123"));
        assert!(content.contains("GEMINI_API_KEY=AIzaSyTest123"));
        assert!(content.contains("OTHER_VAR=abc"));
    }

    #[test]
    #[cfg(unix)]
    fn test_copy_dir_all_with_symlinks() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        let external_target = temp.path().join("external");

        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&external_target).unwrap();
        fs::write(external_target.join("SKILL.md"), "# External Skill").unwrap();

        // Create symlink inside src pointing to external_target directory
        std::os::unix::fs::symlink(&external_target, src.join("linked-skill")).unwrap();
        fs::write(src.join("regular.txt"), "hello").unwrap();

        let count = copy_dir_all(&src, &dst, false).unwrap();
        assert!(count >= 2);
        assert!(dst.join("regular.txt").exists());
        assert!(dst.join("linked-skill").exists());
    }
}

// Rust guideline compliant 2026-02-21
