use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub use octomonitor_adapter_common::{
    mask_value, probe_file, read_jsonl_delta, resolve_home_dir, run_command_probe,
    AdapterDescriptor, CommandProbeResult, FileProbeResult, JsonlCursor,
};

pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor {
        tool: "hermes",
        preferred_mode: "gateway+status",
        fallback_mode: "sessions-scan",
        confidence: "live",
    }
}

/// A session entry from Hermes sessions/sessions.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesSession {
    pub session_id: String,
    pub session_key: String,
    pub profile_name: String,
    pub display_name: Option<String>,
    pub platform: Option<String>,
    pub chat_type: String,
    pub model: Option<String>,
    pub started_at: Option<String>,
    pub updated_at: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub first_question: Option<String>,
    pub last_question: Option<String>,
    pub message_count: u64,
    pub error_message: Option<String>,
    /// Human-readable source label, e.g. "Telegram: Yixiao"
    pub origin_label: Option<String>,
    /// Source provider, e.g. "telegram", "local", "cron"
    pub origin_provider: Option<String>,
}

/// A scheduled cron job from ~/.hermes/cron/jobs.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesCronJob {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub profile_name: String,
    pub schedule_expr: String,
    pub schedule_tz: String,
    pub schedule_human: String,
}

/// Per-profile metadata discovered during probe
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesInstance {
    pub profile_name: String,
    pub home_dir: String,
    pub gateway_running: bool,
    pub gateway_state: Option<String>,
    pub gateway_platforms: Vec<String>,
    pub config_exists: bool,
    pub session_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesSnapshot {
    pub probed_at: String,
    pub cli_available: bool,
    pub gateway_running: bool,
    pub cli_version: Option<String>,
    pub instances: Vec<HermesInstance>,
    pub sessions: Vec<HermesSession>,
    pub cron_jobs: Vec<HermesCronJob>,
    pub command_probes: Vec<CommandProbeResult>,
    pub file_probes: Vec<FileProbeResult>,
}

#[derive(Debug, Clone, Default)]
pub struct HermesProbeCache {
    session_lists: HashMap<String, CachedSessionsFile>,
}

#[derive(Debug, Clone)]
struct CachedSessionsFile {
    size_bytes: u64,
    modified_at: Option<SystemTime>,
    sessions: Vec<HermesSession>,
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

fn hermes_root() -> PathBuf {
    resolve_home_dir(".hermes")
}

/// Discover all Hermes instances: default + profiles.
fn discover_instances(root: &Path) -> Vec<(String, PathBuf)> {
    let mut instances = Vec::new();

    // Default instance — ~/.hermes itself
    if root.join("config.yaml").exists()
        || root.join("sessions").is_dir()
        || root.join("gateway.pid").exists()
    {
        instances.push(("default".to_string(), root.to_path_buf()));
    }

    // Named profiles — ~/.hermes/profiles/<name>/
    let profiles_dir = root.join("profiles");
    if profiles_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&profiles_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    instances.push((name, entry.path()));
                }
            }
        }
    }

    instances
}

// ---------------------------------------------------------------------------
// Gateway status
// ---------------------------------------------------------------------------

fn check_gateway_status(home: &Path) -> (bool, Option<String>, Vec<String>) {
    let pid_file = home.join("gateway.pid");
    let pid_exists = pid_file.exists();

    let state_file = home.join("gateway_state.json");
    let (state, platforms) = match fs::read_to_string(&state_file) {
        Ok(contents) => match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(val) => {
                let state = val
                    .get("gateway_state")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let platforms = val
                    .get("platforms")
                    .and_then(|v| v.as_object())
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default();
                (state, platforms)
            }
            Err(_) => (None, vec![]),
        },
        Err(_) => (None, vec![]),
    };

    // Running = PID file exists and state is not explicitly "stopped" or "startup_failed"
    let running = pid_exists
        && !matches!(
            state.as_deref(),
            Some("stopped") | Some("startup_failed")
        );

    (running, state, platforms)
}

// ---------------------------------------------------------------------------
// Session parsing
// ---------------------------------------------------------------------------

fn derive_origin(
    entry: &serde_json::Value,
) -> (Option<String>, Option<String>) {
    let origin = entry.get("origin");

    let platform = origin
        .and_then(|o| o.get("platform"))
        .and_then(|v| v.as_str())
        .or_else(|| entry.get("platform").and_then(|v| v.as_str()));

    let user_name = origin
        .and_then(|o| o.get("user_name"))
        .and_then(|v| v.as_str());
    let chat_name = origin
        .and_then(|o| o.get("chat_name"))
        .and_then(|v| v.as_str());

    let provider = platform.map(String::from);

    let label = match platform {
        Some("local") => Some("CLI".to_string()),
        Some(p) => {
            let name = user_name
                .or(chat_name)
                .unwrap_or("DM");
            Some(format!("{}: {}", capitalize(p), name))
        }
        None => None,
    };

    (label, provider)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn parse_sessions_json(
    path: &Path,
    profile_name: &str,
    model_from_config: Option<&str>,
) -> Option<Vec<HermesSession>> {
    let contents = fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let obj = val.as_object()?;

    let mut result = Vec::new();

    for (session_key, entry) in obj {
        let session_id = entry
            .get("session_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())?
            .to_string();

        let created_at = entry.get("created_at").and_then(|v| v.as_str()).map(String::from);
        let updated_at = entry.get("updated_at").and_then(|v| v.as_str()).map(String::from);

        // Skip entries with no meaningful data
        if created_at.is_none() && updated_at.is_none() {
            continue;
        }

        let display_name = entry.get("display_name").and_then(|v| v.as_str()).map(String::from);
        let platform = entry
            .get("platform")
            .and_then(|v| v.as_str())
            .map(String::from);
        let chat_type = entry
            .get("chat_type")
            .and_then(|v| v.as_str())
            .unwrap_or("dm")
            .to_string();

        let input_tokens = entry.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let output_tokens = entry.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let cache_read_tokens = entry.get("cache_read_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let cache_write_tokens = entry.get("cache_write_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let total_tokens = entry.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let cost_usd = entry.get("estimated_cost_usd").and_then(|v| v.as_f64());

        let (origin_label, origin_provider) = derive_origin(entry);

        result.push(HermesSession {
            session_id,
            session_key: session_key.clone(),
            profile_name: profile_name.to_string(),
            display_name,
            platform,
            chat_type,
            model: model_from_config.map(String::from),
            started_at: created_at,
            updated_at,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            total_tokens,
            cost_usd,
            first_question: None,
            last_question: None,
            message_count: 0,
            error_message: None,
            origin_label,
            origin_provider,
        });
    }

    Some(result)
}

fn file_signature(path: &Path) -> Option<(u64, Option<SystemTime>)> {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

fn load_cached_sessions(
    cache: &mut HermesProbeCache,
    sessions_json: &Path,
    profile_name: &str,
    model_from_config: Option<&str>,
) -> Option<Vec<HermesSession>> {
    let cache_key = sessions_json.display().to_string();
    let signature = file_signature(sessions_json)?;

    match cache.session_lists.get(&cache_key) {
        Some(cached) if cached.size_bytes == signature.0 && cached.modified_at == signature.1 => {
            Some(cached.sessions.clone())
        }
        _ => {
            let sessions = parse_sessions_json(sessions_json, profile_name, model_from_config)?;
            cache.session_lists.insert(
                cache_key,
                CachedSessionsFile {
                    size_bytes: signature.0,
                    modified_at: signature.1,
                    sessions: sessions.clone(),
                },
            );
            Some(sessions)
        }
    }
}

// ---------------------------------------------------------------------------
// Config parsing (extract model)
// ---------------------------------------------------------------------------

fn read_default_model(home: &Path) -> Option<String> {
    let config_path = home.join("config.yaml");
    let contents = fs::read_to_string(config_path).ok()?;
    // Simple extraction: look for "default:" under "model:" section
    let mut in_model = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "model:" || trimmed.starts_with("model:") {
            // Check if inline: "model:\n  default: xxx"
            if trimmed == "model:" {
                in_model = true;
                continue;
            }
        }
        if in_model {
            if !line.starts_with(' ') && !line.starts_with('\t') {
                break; // Left model section
            }
            if let Some(rest) = trimmed.strip_prefix("default:") {
                let model = rest.trim().trim_matches('"').trim_matches('\'');
                if !model.is_empty() {
                    return Some(model.to_string());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Cron job scanning
// ---------------------------------------------------------------------------

fn cron_to_human(schedule: &serde_json::Value) -> String {
    // Hermes uses a richer schedule format: {"kind": "cron", "cron": "0 9 * * *", ...}
    // or {"kind": "interval", "seconds": 3600, ...}
    // or {"kind": "once", "at": "2026-04-15T10:00:00", ...}
    let kind = schedule.get("kind").and_then(|v| v.as_str()).unwrap_or("unknown");
    match kind {
        "cron" => {
            let expr = schedule.get("cron").and_then(|v| v.as_str()).unwrap_or("?");
            let tz = schedule.get("timezone").and_then(|v| v.as_str()).unwrap_or("UTC");
            format_cron_expr(expr, tz)
        }
        "interval" => {
            let secs = schedule.get("seconds").and_then(|v| v.as_u64()).unwrap_or(0);
            if secs >= 3600 {
                format!("Every {}h", secs / 3600)
            } else if secs >= 60 {
                format!("Every {}m", secs / 60)
            } else {
                format!("Every {}s", secs)
            }
        }
        "once" => {
            let at = schedule.get("at").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Once at {}", at)
        }
        "daily" => {
            let hour = schedule.get("at_hour").and_then(|v| v.as_u64()).unwrap_or(0);
            let minute = schedule.get("at_minute").and_then(|v| v.as_u64()).unwrap_or(0);
            format!("Daily {:02}:{:02}", hour, minute)
        }
        _ => {
            schedule.get("display").and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| kind.to_string())
        }
    }
}

fn format_cron_expr(expr: &str, tz: &str) -> String {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() < 5 {
        return format!("{} ({})", expr, tz);
    }
    let (min, hour, _dom, _mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);

    let time_str = if hour != "*" && min != "*" {
        format!("{:0>2}:{:0>2}", hour, min)
    } else {
        return format!("{} ({})", expr, tz);
    };

    if dow == "*" {
        return format!("Daily {time_str}");
    }

    let dow_str: String = dow
        .split(',')
        .map(|d| match d {
            "0" => "Sun",
            "1" => "Mon",
            "2" => "Tue",
            "3" => "Wed",
            "4" => "Thu",
            "5" => "Fri",
            "6" => "Sat",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{dow_str} {time_str}")
}

fn scan_cron_jobs(home: &Path, profile_name: &str) -> Vec<HermesCronJob> {
    scan_cron_jobs_inner(home, profile_name).unwrap_or_default()
}

fn scan_cron_jobs_inner(home: &Path, profile_name: &str) -> Option<Vec<HermesCronJob>> {
    let contents = fs::read_to_string(home.join("cron").join("jobs.json")).ok()?;
    let val: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let jobs = val.get("jobs").and_then(|j| j.as_array())?;

    Some(
        jobs.iter()
            .filter_map(|j| {
                let id = j.get("id")?.as_str()?.to_string();
                let name = j.get("name")?.as_str()?.to_string();
                let enabled = j.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);

                let schedule = j.get("schedule")?;
                let schedule_human = cron_to_human(schedule);

                let expr = schedule
                    .get("cron")
                    .and_then(|v| v.as_str())
                    .or_else(|| j.get("schedule_display").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                let tz = schedule
                    .get("timezone")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UTC")
                    .to_string();

                Some(HermesCronJob {
                    id,
                    name,
                    enabled,
                    profile_name: profile_name.to_string(),
                    schedule_expr: expr,
                    schedule_tz: tz,
                    schedule_human,
                })
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Probe
// ---------------------------------------------------------------------------

fn scan_instance(
    profile_name: &str,
    home: &Path,
    cache: Option<&mut HermesProbeCache>,
) -> (HermesInstance, Vec<HermesSession>, Vec<HermesCronJob>) {
    let config_exists = home.join("config.yaml").exists();
    let model = read_default_model(home);
    let model_ref = model.as_deref();

    let (gw_running, gw_state, gw_platforms) = check_gateway_status(home);

    let sessions_json = home.join("sessions").join("sessions.json");
    let sessions = if sessions_json.exists() {
        match cache {
            Some(cache) => load_cached_sessions(cache, &sessions_json, profile_name, model_ref),
            None => parse_sessions_json(&sessions_json, profile_name, model_ref),
        }
        .unwrap_or_default()
    } else {
        vec![]
    };

    let cron_jobs = scan_cron_jobs(home, profile_name);

    let instance = HermesInstance {
        profile_name: profile_name.to_string(),
        home_dir: home.display().to_string(),
        gateway_running: gw_running,
        gateway_state: gw_state,
        gateway_platforms: gw_platforms,
        config_exists,
        session_count: sessions.len(),
    };

    (instance, sessions, cron_jobs)
}

pub fn probe_with_cache(cache: &mut HermesProbeCache) -> HermesSnapshot {
    let root = hermes_root();

    let version_probe = run_command_probe("hermes", &["--version"]);
    let config_probe = probe_file(&root.join("config.yaml"));
    let sessions_dir_probe = probe_file(&root.join("sessions"));

    let instances_discovered = discover_instances(&root);

    let mut all_instances = Vec::new();
    let mut all_sessions = Vec::new();
    let mut all_cron_jobs = Vec::new();
    let mut any_gateway_running = false;

    // Track which session list files are still live for cache cleanup
    let mut live_cache_keys = Vec::new();

    for (profile_name, home) in &instances_discovered {
        let sessions_json = home.join("sessions").join("sessions.json");
        if sessions_json.exists() {
            live_cache_keys.push(sessions_json.display().to_string());
        }

        let (instance, sessions, cron_jobs) = scan_instance(profile_name, home, Some(cache));
        if instance.gateway_running {
            any_gateway_running = true;
        }
        all_instances.push(instance);
        all_sessions.extend(sessions);
        all_cron_jobs.extend(cron_jobs);
    }

    // Clean stale cache entries
    cache
        .session_lists
        .retain(|key, _| live_cache_keys.contains(key));

    // Sort sessions by updated_at descending
    all_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    HermesSnapshot {
        probed_at: Utc::now().to_rfc3339(),
        cli_available: version_probe.success,
        gateway_running: any_gateway_running,
        cli_version: version_probe
            .stdout_snippet
            .as_ref()
            .map(|s| s.trim().to_string()),
        instances: all_instances,
        sessions: all_sessions,
        cron_jobs: all_cron_jobs,
        command_probes: vec![version_probe],
        file_probes: vec![config_probe, sessions_dir_probe],
    }
}

pub fn probe() -> HermesSnapshot {
    let mut cache = HermesProbeCache::default();
    probe_with_cache(&mut cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn descriptor_returns_hermes() {
        let d = descriptor();
        assert_eq!(d.tool, "hermes");
    }

    #[test]
    fn probe_runs_without_panic() {
        let snap = probe();
        assert!(!snap.probed_at.is_empty());
        assert_eq!(snap.command_probes.len(), 1);
    }

    #[test]
    fn discover_instances_finds_default_when_config_exists() {
        let temp_dir = std::env::temp_dir().join(format!(
            "octomonitor-hermes-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("create");
        fs::write(temp_dir.join("config.yaml"), "model:\n  default: test").expect("write");

        let instances = discover_instances(&temp_dir);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].0, "default");

        fs::remove_dir_all(&temp_dir).expect("cleanup");
    }

    #[test]
    fn discover_instances_finds_profiles() {
        let temp_dir = std::env::temp_dir().join(format!(
            "octomonitor-hermes-profiles-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let profiles_dir = temp_dir.join("profiles");
        fs::create_dir_all(profiles_dir.join("coder")).expect("create");
        fs::create_dir_all(profiles_dir.join("research")).expect("create");
        // No config.yaml in root, so default is not discovered
        let instances = discover_instances(&temp_dir);
        assert_eq!(instances.len(), 2);

        let names: Vec<&str> = instances.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"coder"));
        assert!(names.contains(&"research"));

        fs::remove_dir_all(&temp_dir).expect("cleanup");
    }

    #[test]
    fn parse_sessions_json_extracts_sessions() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let sessions_path = temp_dir.path().join("sessions.json");
        fs::write(
            &sessions_path,
            r#"{
  "local:cli:user": {
    "session_key": "local:cli:user",
    "session_id": "abc-123",
    "created_at": "2026-04-12T10:00:00",
    "updated_at": "2026-04-12T10:30:00",
    "display_name": "CLI User",
    "platform": "local",
    "chat_type": "dm",
    "input_tokens": 5000,
    "output_tokens": 3000,
    "total_tokens": 8000,
    "estimated_cost_usd": 0.15
  },
  "telegram:123:user:456": {
    "session_key": "telegram:123:user:456",
    "session_id": "def-456",
    "created_at": "2026-04-12T09:00:00",
    "updated_at": "2026-04-12T09:45:00",
    "platform": "telegram",
    "chat_type": "dm",
    "input_tokens": 2000,
    "output_tokens": 1000,
    "total_tokens": 3000,
    "origin": {
      "platform": "telegram",
      "user_name": "Yixiao",
      "chat_name": "Yixiao Wang"
    }
  }
}"#,
        )
        .expect("write");

        let sessions = parse_sessions_json(&sessions_path, "default", Some("gpt-5"))
            .expect("parse");

        assert_eq!(sessions.len(), 2);

        let cli_session = sessions.iter().find(|s| s.session_id == "abc-123").unwrap();
        assert_eq!(cli_session.profile_name, "default");
        assert_eq!(cli_session.platform.as_deref(), Some("local"));
        assert_eq!(cli_session.input_tokens, 5000);
        assert_eq!(cli_session.cost_usd, Some(0.15));
        assert_eq!(cli_session.model.as_deref(), Some("gpt-5"));
        assert_eq!(cli_session.origin_label.as_deref(), Some("CLI"));
        assert_eq!(cli_session.origin_provider.as_deref(), Some("local"));

        let tg_session = sessions.iter().find(|s| s.session_id == "def-456").unwrap();
        assert_eq!(tg_session.origin_label.as_deref(), Some("Telegram: Yixiao"));
        assert_eq!(tg_session.origin_provider.as_deref(), Some("telegram"));
    }

    #[test]
    fn cached_sessions_returns_same_data_on_unchanged_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let sessions_path = temp_dir.path().join("sessions.json");
        fs::write(
            &sessions_path,
            r#"{"local:cli:user": {"session_key": "local:cli:user", "session_id": "id1", "created_at": "2026-04-12T10:00:00", "updated_at": "2026-04-12T10:30:00", "input_tokens": 100, "output_tokens": 50, "total_tokens": 150}}"#,
        )
        .expect("write");

        let mut cache = HermesProbeCache::default();
        let first = load_cached_sessions(&mut cache, &sessions_path, "default", None);
        assert_eq!(first.as_ref().unwrap().len(), 1);

        // Second call should hit cache
        let second = load_cached_sessions(&mut cache, &sessions_path, "default", None);
        assert_eq!(second.as_ref().unwrap().len(), 1);
        assert_eq!(cache.session_lists.len(), 1);
    }

    #[test]
    fn gateway_status_returns_false_when_no_pid_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let (running, state, platforms) = check_gateway_status(temp_dir.path());
        assert!(!running);
        assert!(state.is_none());
        assert!(platforms.is_empty());
    }

    #[test]
    fn gateway_status_detects_running() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        fs::write(temp_dir.path().join("gateway.pid"), "12345").expect("pid");
        fs::write(
            temp_dir.path().join("gateway_state.json"),
            r#"{"gateway_state": "running", "platforms": {"telegram": {"status": "connected"}}}"#,
        )
        .expect("state");

        let (running, state, platforms) = check_gateway_status(temp_dir.path());
        assert!(running);
        assert_eq!(state.as_deref(), Some("running"));
        assert_eq!(platforms, vec!["telegram"]);
    }

    #[test]
    fn gateway_status_stopped_is_not_running() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        fs::write(temp_dir.path().join("gateway.pid"), "12345").expect("pid");
        fs::write(
            temp_dir.path().join("gateway_state.json"),
            r#"{"gateway_state": "stopped"}"#,
        )
        .expect("state");

        let (running, _, _) = check_gateway_status(temp_dir.path());
        assert!(!running);
    }

    #[test]
    fn read_default_model_extracts_from_config() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp_dir.path().join("config.yaml"),
            "model:\n  default: anthropic/claude-opus-4.6\n  provider: auto\n",
        )
        .expect("write");

        let model = read_default_model(temp_dir.path());
        assert_eq!(model.as_deref(), Some("anthropic/claude-opus-4.6"));
    }

    #[test]
    fn cron_to_human_formats_cron_kind() {
        let schedule = serde_json::json!({"kind": "cron", "cron": "0 9 * * *", "timezone": "UTC"});
        assert_eq!(cron_to_human(&schedule), "Daily 09:00");
    }

    #[test]
    fn cron_to_human_formats_interval() {
        let schedule = serde_json::json!({"kind": "interval", "seconds": 7200});
        assert_eq!(cron_to_human(&schedule), "Every 2h");
    }
}
