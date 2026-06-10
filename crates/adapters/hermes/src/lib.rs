use chrono::Utc;
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub use octomonitor_adapter_common::{
    capitalize, file_signature, format_cron_expr, mask_value, path_has_sensitive_component,
    probe_file, read_jsonl_delta, resolve_env_or_home, run_command_probe, AdapterDescriptor,
    CommandProbeResult, FileProbeResult, JsonlCursor,
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
    pub source_path: Option<String>,
    pub source_format: HermesSessionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HermesSessionSource {
    StateDb,
    SessionsJson,
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
    state_dbs: HashMap<String, CachedStateDb>,
}

#[derive(Debug, Clone)]
struct CachedSessionsFile {
    size_bytes: u64,
    modified_at: Option<SystemTime>,
    sessions: Vec<HermesSession>,
}

#[derive(Debug, Clone)]
struct CachedStateDb {
    size_bytes: u64,
    modified_at: Option<SystemTime>,
    sessions: Vec<HermesSession>,
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

fn hermes_root() -> PathBuf {
    resolve_env_or_home(&["HERMES_HOME"], ".hermes")
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
    if let Ok(entries) = fs::read_dir(root.join("profiles")) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                instances.push((
                    entry.file_name().to_string_lossy().into_owned(),
                    entry.path(),
                ));
            }
        }
    }

    instances
}

// ---------------------------------------------------------------------------
// Gateway status
// ---------------------------------------------------------------------------

fn check_gateway_status(home: &Path) -> (bool, Option<String>, Vec<String>) {
    let (state, platforms) = fs::read_to_string(home.join("gateway_state.json"))
        .ok()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .map(|val| {
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
        })
        .unwrap_or_default();

    // Running = PID file exists, process is alive, and state is not explicitly stopped
    let pid_alive = fs::read_to_string(home.join("gateway.pid"))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .is_some_and(is_process_alive);

    let running =
        pid_alive && !matches!(state.as_deref(), Some("stopped") | Some("startup_failed"));

    (running, state, platforms)
}

/// Check if a process with the given PID is still alive via `kill -0`.
fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Session parsing
// ---------------------------------------------------------------------------

fn derive_origin(entry: &serde_json::Value) -> (Option<String>, Option<String>) {
    let origin = entry.get("origin");
    let origin_str = |key: &str| origin.and_then(|o| o.get(key)).and_then(|v| v.as_str());

    let platform =
        origin_str("platform").or_else(|| entry.get("platform").and_then(|v| v.as_str()));

    let label = match platform {
        Some("local") => Some("CLI".to_string()),
        Some(p) => {
            let name = origin_str("user_name")
                .or_else(|| origin_str("chat_name"))
                .unwrap_or("DM");
            Some(format!("{}: {}", capitalize(p), name))
        }
        None => None,
    };

    (label, platform.map(String::from))
}

fn parse_sessions_json(
    path: &Path,
    profile_name: &str,
    model_from_config: Option<&str>,
) -> Option<Vec<HermesSession>> {
    if path_has_sensitive_component(path) {
        return None;
    }
    let contents = fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let obj = val.as_object()?;

    let mut result = Vec::new();

    for (session_key, entry) in obj {
        let get_str = |key: &str| entry.get(key).and_then(|v| v.as_str()).map(String::from);
        let get_u64 = |key: &str| entry.get(key).and_then(|v| v.as_u64()).unwrap_or(0);

        let Some(session_id) = entry
            .get("session_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
        else {
            continue;
        };

        let created_at = get_str("created_at");
        let updated_at = get_str("updated_at");

        // Skip entries with no meaningful data
        if created_at.is_none() && updated_at.is_none() {
            continue;
        }

        let chat_type = entry
            .get("chat_type")
            .and_then(|v| v.as_str())
            .unwrap_or("dm")
            .to_string();

        let (origin_label, origin_provider) = derive_origin(entry);

        result.push(HermesSession {
            session_id,
            session_key: session_key.clone(),
            profile_name: profile_name.to_string(),
            display_name: get_str("display_name"),
            platform: get_str("platform"),
            chat_type,
            model: model_from_config.map(String::from),
            started_at: created_at,
            updated_at,
            input_tokens: get_u64("input_tokens"),
            output_tokens: get_u64("output_tokens"),
            cache_read_tokens: get_u64("cache_read_tokens"),
            cache_write_tokens: get_u64("cache_write_tokens"),
            total_tokens: get_u64("total_tokens"),
            cost_usd: entry.get("estimated_cost_usd").and_then(|v| v.as_f64()),
            first_question: None,
            last_question: None,
            message_count: 0,
            error_message: None,
            origin_label,
            origin_provider,
            source_path: Some(path.display().to_string()),
            source_format: HermesSessionSource::SessionsJson,
        });
    }

    Some(result)
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
        [table],
        |_| Ok(()),
    )
    .is_ok()
}

fn table_columns(conn: &Connection, table: &str) -> HashSet<String> {
    let sql = format!("PRAGMA table_info({})", quote_ident(table));
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return HashSet::new();
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return HashSet::new();
    };
    rows.filter_map(Result::ok).collect()
}

fn select_alias(columns: &HashSet<String>, candidates: &[&str], alias: &str) -> String {
    candidates
        .iter()
        .find(|candidate| columns.contains(**candidate))
        .map(|column| format!("{} AS {}", quote_ident(column), quote_ident(alias)))
        .unwrap_or_else(|| format!("NULL AS {}", quote_ident(alias)))
}

fn row_string(row: &rusqlite::Row<'_>, name: &str) -> Option<String> {
    match row.get_ref(name).ok()? {
        ValueRef::Null => None,
        ValueRef::Text(bytes) => std::str::from_utf8(bytes)
            .ok()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from),
        ValueRef::Integer(value) => Some(value.to_string()),
        ValueRef::Real(value) => Some(value.to_string()),
        ValueRef::Blob(_) => None,
    }
}

fn row_timestamp_string(row: &rusqlite::Row<'_>, name: &str) -> Option<String> {
    match row.get_ref(name).ok()? {
        ValueRef::Null => None,
        ValueRef::Text(_) => row_string(row, name),
        ValueRef::Integer(value) => {
            normalize_timestamp_number(value).or_else(|| Some(value.to_string()))
        }
        ValueRef::Real(value) => {
            normalize_timestamp_number(value as i64).or_else(|| Some(value.to_string()))
        }
        ValueRef::Blob(_) => None,
    }
}

fn normalize_timestamp_number(value: i64) -> Option<String> {
    let seconds = if value > 1_000_000_000_000 {
        value / 1_000
    } else if value > 1_000_000_000 {
        value
    } else {
        return None;
    };
    chrono::DateTime::from_timestamp(seconds, 0).map(|dt| dt.to_rfc3339())
}

fn row_u64(row: &rusqlite::Row<'_>, name: &str) -> u64 {
    match row.get_ref(name).ok() {
        Some(ValueRef::Integer(value)) if value > 0 => value as u64,
        Some(ValueRef::Real(value)) if value > 0.0 => value as u64,
        Some(ValueRef::Text(bytes)) => std::str::from_utf8(bytes)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(0),
        _ => 0,
    }
}

fn row_f64(row: &rusqlite::Row<'_>, name: &str) -> Option<f64> {
    match row.get_ref(name).ok()? {
        ValueRef::Integer(value) => Some(value as f64),
        ValueRef::Real(value) => Some(value),
        ValueRef::Text(bytes) => std::str::from_utf8(bytes)
            .ok()
            .and_then(|value| value.trim().parse::<f64>().ok()),
        _ => None,
    }
}

#[derive(Debug, Clone, Default)]
struct MessageStats {
    count: u64,
    last_timestamp: Option<String>,
}

fn read_message_stats(conn: &Connection) -> HashMap<String, MessageStats> {
    if !table_exists(conn, "messages") {
        return HashMap::new();
    }
    let columns = table_columns(conn, "messages");
    if !columns.contains("session_id") {
        return HashMap::new();
    }

    let timestamp_expr = if columns.contains("timestamp") {
        "MAX(timestamp)"
    } else if columns.contains("created_at") {
        "MAX(created_at)"
    } else {
        "NULL"
    };
    let sql = format!(
        "SELECT session_id, COUNT(*) AS message_count, {timestamp_expr} AS last_timestamp \
         FROM messages GROUP BY session_id"
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return HashMap::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row_string(row, "session_id").unwrap_or_default(),
            MessageStats {
                count: row_u64(row, "message_count"),
                last_timestamp: row_timestamp_string(row, "last_timestamp"),
            },
        ))
    }) else {
        return HashMap::new();
    };

    rows.filter_map(Result::ok)
        .filter(|(session_id, _)| !session_id.is_empty())
        .collect()
}

fn parse_state_db(
    path: &Path,
    profile_name: &str,
    model_from_config: Option<&str>,
) -> Option<Vec<HermesSession>> {
    if path_has_sensitive_component(path) {
        return None;
    }

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    if !table_exists(&conn, "sessions") {
        return None;
    }

    let columns = table_columns(&conn, "sessions");
    let fields = [
        select_alias(&columns, &["id", "session_id"], "session_id"),
        select_alias(
            &columns,
            &["session_key", "key", "id", "session_id"],
            "session_key",
        ),
        select_alias(&columns, &["title", "display_name", "name"], "display_name"),
        select_alias(&columns, &["source", "platform"], "platform"),
        select_alias(&columns, &["chat_type", "type"], "chat_type"),
        select_alias(&columns, &["model"], "model"),
        select_alias(&columns, &["started_at", "created_at"], "started_at"),
        select_alias(
            &columns,
            &["updated_at", "last_activity_at", "ended_at"],
            "updated_at",
        ),
        select_alias(&columns, &["input_tokens", "prompt_tokens"], "input_tokens"),
        select_alias(
            &columns,
            &["output_tokens", "completion_tokens"],
            "output_tokens",
        ),
        select_alias(&columns, &["cache_read_tokens"], "cache_read_tokens"),
        select_alias(&columns, &["cache_write_tokens"], "cache_write_tokens"),
        select_alias(&columns, &["total_tokens"], "total_tokens"),
        select_alias(&columns, &["estimated_cost_usd", "cost_usd"], "cost_usd"),
        select_alias(&columns, &["error_message", "error"], "error_message"),
    ];
    let sql = format!("SELECT {} FROM sessions", fields.join(", "));
    let message_stats = read_message_stats(&conn);
    let source_path = path.display().to_string();

    let Ok(mut stmt) = conn.prepare(&sql) else {
        return None;
    };
    let rows = stmt
        .query_map([], |row| {
            let session_id = row_string(row, "session_id").unwrap_or_default();
            let session_key = row_string(row, "session_key").unwrap_or_else(|| session_id.clone());
            let stats = message_stats.get(&session_id);
            let input_tokens = row_u64(row, "input_tokens");
            let output_tokens = row_u64(row, "output_tokens");
            let cache_read_tokens = row_u64(row, "cache_read_tokens");
            let cache_write_tokens = row_u64(row, "cache_write_tokens");
            let total_tokens = row_u64(row, "total_tokens");
            Ok(HermesSession {
                session_id,
                session_key,
                profile_name: profile_name.to_string(),
                display_name: row_string(row, "display_name"),
                platform: row_string(row, "platform"),
                chat_type: row_string(row, "chat_type").unwrap_or_else(|| "dm".into()),
                model: row_string(row, "model").or_else(|| model_from_config.map(String::from)),
                started_at: row_timestamp_string(row, "started_at"),
                updated_at: row_timestamp_string(row, "updated_at")
                    .or_else(|| stats.and_then(|stat| stat.last_timestamp.clone())),
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_write_tokens,
                total_tokens: if total_tokens > 0 {
                    total_tokens
                } else {
                    input_tokens
                        .saturating_add(output_tokens)
                        .saturating_add(cache_read_tokens)
                        .saturating_add(cache_write_tokens)
                },
                cost_usd: row_f64(row, "cost_usd"),
                first_question: None,
                last_question: None,
                message_count: stats.map(|stat| stat.count).unwrap_or(0),
                error_message: row_string(row, "error_message"),
                origin_label: None,
                origin_provider: None,
                source_path: Some(source_path.clone()),
                source_format: HermesSessionSource::StateDb,
            })
        })
        .ok()?;

    Some(
        rows.filter_map(Result::ok)
            .filter(|session| !session.session_id.is_empty())
            .collect(),
    )
}

fn enrich_state_db_sessions(sessions: &mut [HermesSession], sessions_json: &[HermesSession]) {
    let by_id: HashMap<&str, &HermesSession> = sessions_json
        .iter()
        .map(|session| (session.session_id.as_str(), session))
        .collect();

    for session in sessions {
        let Some(json_session) = by_id.get(session.session_id.as_str()) else {
            continue;
        };
        if session.origin_label.is_none() {
            session.origin_label = json_session.origin_label.clone();
        }
        if session.origin_provider.is_none() {
            session.origin_provider = json_session.origin_provider.clone();
        }
        if session.display_name.is_none() {
            session.display_name = json_session.display_name.clone();
        }
        if session.model.is_none() {
            session.model = json_session.model.clone();
        }
        if session.total_tokens == 0 {
            session.input_tokens = json_session.input_tokens;
            session.output_tokens = json_session.output_tokens;
            session.cache_read_tokens = json_session.cache_read_tokens;
            session.cache_write_tokens = json_session.cache_write_tokens;
            session.total_tokens = json_session.total_tokens;
        }
        if session.cost_usd.is_none() {
            session.cost_usd = json_session.cost_usd;
        }
    }
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

fn load_cached_state_db(
    cache: &mut HermesProbeCache,
    state_db: &Path,
    profile_name: &str,
    model_from_config: Option<&str>,
) -> Option<Vec<HermesSession>> {
    let cache_key = state_db.display().to_string();
    let signature = file_signature(state_db)?;

    match cache.state_dbs.get(&cache_key) {
        Some(cached) if cached.size_bytes == signature.0 && cached.modified_at == signature.1 => {
            Some(cached.sessions.clone())
        }
        _ => {
            let sessions = parse_state_db(state_db, profile_name, model_from_config)?;
            cache.state_dbs.insert(
                cache_key,
                CachedStateDb {
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
    let contents = fs::read_to_string(home.join("config.yaml")).ok()?;
    let clean = |raw: &str| {
        let v = raw.trim().trim_matches('"').trim_matches('\'');
        (!v.is_empty()).then(|| v.to_string())
    };

    // Look for "model: <inline>" or "default: <value>" inside a "model:" block.
    let mut in_model = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "model:" {
            in_model = true;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("model:") {
            if let Some(model) = clean(rest) {
                return Some(model);
            }
        }
        if in_model {
            if !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("default:") {
                if let Some(model) = clean(rest) {
                    return Some(model);
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
    let str_or = |key: &str, fallback: &'static str| {
        schedule
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or(fallback)
    };
    let u64_or_zero = |key: &str| schedule.get(key).and_then(|v| v.as_u64()).unwrap_or(0);

    let kind = str_or("kind", "unknown");
    match kind {
        "cron" => format_cron_expr(str_or("cron", "?"), str_or("timezone", "UTC")),
        "interval" => {
            let secs = u64_or_zero("seconds");
            if secs >= 3600 {
                format!("Every {}h", secs / 3600)
            } else if secs >= 60 {
                format!("Every {}m", secs / 60)
            } else {
                format!("Every {}s", secs)
            }
        }
        "once" => format!("Once at {}", str_or("at", "?")),
        "daily" => format!(
            "Daily {:02}:{:02}",
            u64_or_zero("at_hour"),
            u64_or_zero("at_minute")
        ),
        _ => schedule
            .get("display")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| kind.to_string()),
    }
}

fn scan_cron_jobs(home: &Path, profile_name: &str) -> Vec<HermesCronJob> {
    let Ok(contents) = fs::read_to_string(home.join("cron").join("jobs.json")) else {
        return vec![];
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return vec![];
    };
    let Some(jobs) = val.get("jobs").and_then(|j| j.as_array()) else {
        return vec![];
    };

    jobs.iter()
        .filter_map(|j| {
            let id = j.get("id")?.as_str()?.to_string();
            let name = j.get("name")?.as_str()?.to_string();
            let enabled = j.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);

            let schedule = j.get("schedule")?;
            let sched_str = |key: &str| schedule.get(key).and_then(|v| v.as_str());

            let expr = sched_str("cron")
                .or_else(|| j.get("schedule_display").and_then(|v| v.as_str()))
                .or_else(|| sched_str("kind"))
                .unwrap_or("unknown")
                .to_string();
            let tz = sched_str("timezone").unwrap_or("UTC").to_string();

            Some(HermesCronJob {
                id,
                name,
                enabled,
                profile_name: profile_name.to_string(),
                schedule_expr: expr,
                schedule_tz: tz,
                schedule_human: cron_to_human(schedule),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Probe
// ---------------------------------------------------------------------------

fn scan_instance(
    profile_name: &str,
    home: &Path,
    cache: &mut HermesProbeCache,
) -> (HermesInstance, Vec<HermesSession>, Vec<HermesCronJob>) {
    let config_exists = home.join("config.yaml").exists();
    let model = read_default_model(home);

    let (gw_running, gw_state, gw_platforms) = check_gateway_status(home);

    let state_db = home.join("state.db");
    let sessions_json = home.join("sessions").join("sessions.json");
    let sessions_json_sessions = sessions_json
        .exists()
        .then(|| load_cached_sessions(cache, &sessions_json, profile_name, model.as_deref()))
        .flatten()
        .unwrap_or_default();
    let mut state_db_sessions = state_db
        .exists()
        .then(|| load_cached_state_db(cache, &state_db, profile_name, model.as_deref()))
        .flatten()
        .unwrap_or_default();

    let sessions = if state_db_sessions.is_empty() {
        sessions_json_sessions
    } else {
        enrich_state_db_sessions(&mut state_db_sessions, &sessions_json_sessions);
        state_db_sessions
    };

    let instance = HermesInstance {
        profile_name: profile_name.to_string(),
        home_dir: home.display().to_string(),
        gateway_running: gw_running,
        gateway_state: gw_state,
        gateway_platforms: gw_platforms,
        config_exists,
        session_count: sessions.len(),
    };

    (instance, sessions, scan_cron_jobs(home, profile_name))
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
    let mut live_cache_keys = std::collections::HashSet::new();

    for (profile_name, home) in &instances_discovered {
        let sessions_json = home.join("sessions").join("sessions.json");
        if sessions_json.exists() {
            live_cache_keys.insert(sessions_json.display().to_string());
        }
        let state_db = home.join("state.db");
        if state_db.exists() {
            live_cache_keys.insert(state_db.display().to_string());
        }

        let (instance, sessions, cron_jobs) = scan_instance(profile_name, home, cache);
        any_gateway_running |= instance.gateway_running;
        all_instances.push(instance);
        all_sessions.extend(sessions);
        all_cron_jobs.extend(cron_jobs);
    }

    cache
        .session_lists
        .retain(|key, _| live_cache_keys.contains(key));
    cache
        .state_dbs
        .retain(|key, _| live_cache_keys.contains(key));

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
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
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

        let sessions =
            parse_sessions_json(&sessions_path, "default", Some("gpt-5")).expect("parse");

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
        assert_eq!(tg_session.source_format, HermesSessionSource::SessionsJson);
    }

    #[test]
    fn parse_state_db_extracts_metadata_without_message_content() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("state.db");
        {
            let conn = Connection::open(&db_path).expect("open db");
            conn.execute_batch(
                r#"
                CREATE TABLE sessions (
                  id TEXT PRIMARY KEY,
                  title TEXT,
                  source TEXT,
                  model TEXT,
                  started_at INTEGER,
                  updated_at INTEGER,
                  input_tokens INTEGER,
                  output_tokens INTEGER,
                  total_tokens INTEGER,
                  estimated_cost_usd REAL
                );
                CREATE TABLE messages (
                  session_id TEXT,
                  role TEXT,
                  content TEXT,
                  timestamp INTEGER
                );
                INSERT INTO sessions VALUES (
                  'sess-1', 'SQLite session', 'local', 'gpt-5', 1770000000,
                  1770000300000, 10, 5, 15, 0.02
                );
                INSERT INTO messages VALUES
                  ('sess-1', 'user', 'secret-api-key=abcdef', 1770000010),
                  ('sess-1', 'assistant', 'done', 1770000020);
                "#,
            )
            .expect("schema");
        }

        let sessions = parse_state_db(&db_path, "default", None).expect("parse state db");

        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.session_id, "sess-1");
        assert_eq!(session.display_name.as_deref(), Some("SQLite session"));
        assert_eq!(session.platform.as_deref(), Some("local"));
        assert_eq!(session.model.as_deref(), Some("gpt-5"));
        assert_eq!(session.input_tokens, 10);
        assert_eq!(session.total_tokens, 15);
        assert_eq!(session.cost_usd, Some(0.02));
        assert_eq!(session.message_count, 2);
        assert!(session.first_question.is_none());
        assert!(session.last_question.is_none());
        assert_eq!(session.source_format, HermesSessionSource::StateDb);
        assert_eq!(
            session.source_path.as_deref(),
            Some(db_path.to_str().unwrap())
        );
        assert_eq!(
            session.started_at.as_deref(),
            Some("2026-02-02T02:40:00+00:00")
        );
        assert_eq!(
            session.updated_at.as_deref(),
            Some("2026-02-02T02:45:00+00:00")
        );
    }

    #[test]
    fn parse_state_db_handles_wal_read_only() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("state.db");
        {
            let conn = Connection::open(&db_path).expect("open db");
            conn.pragma_update(None, "journal_mode", "WAL")
                .expect("enable wal");
            conn.execute_batch(
                r#"
                CREATE TABLE sessions (
                  id TEXT PRIMARY KEY,
                  title TEXT,
                  created_at TEXT,
                  updated_at TEXT
                );
                INSERT INTO sessions VALUES (
                  'sess-wal', 'WAL session',
                  '2026-02-03T02:40:00Z',
                  '2026-02-03T02:45:00Z'
                );
                "#,
            )
            .expect("schema");
        }

        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(&db_path).expect("metadata").permissions();
            permissions.set_mode(0o444);
            fs::set_permissions(&db_path, permissions).expect("readonly");
        }

        let sessions = parse_state_db(&db_path, "default", Some("fallback-model"))
            .expect("parse read-only wal db");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess-wal");
        assert_eq!(sessions[0].model.as_deref(), Some("fallback-model"));
        assert_eq!(sessions[0].source_format, HermesSessionSource::StateDb);
    }

    #[test]
    fn state_db_sessions_are_enriched_from_sessions_json_metadata() {
        let mut state_sessions = vec![HermesSession {
            session_id: "sess-1".into(),
            session_key: "sess-1".into(),
            profile_name: "default".into(),
            display_name: None,
            platform: None,
            chat_type: "dm".into(),
            model: None,
            started_at: None,
            updated_at: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            total_tokens: 0,
            cost_usd: None,
            first_question: None,
            last_question: None,
            message_count: 0,
            error_message: None,
            origin_label: None,
            origin_provider: None,
            source_path: Some("state.db".into()),
            source_format: HermesSessionSource::StateDb,
        }];
        let json_sessions = vec![HermesSession {
            session_id: "sess-1".into(),
            session_key: "local:cli:user".into(),
            profile_name: "default".into(),
            display_name: Some("CLI User".into()),
            platform: Some("local".into()),
            chat_type: "dm".into(),
            model: Some("json-model".into()),
            started_at: None,
            updated_at: None,
            input_tokens: 12,
            output_tokens: 8,
            cache_read_tokens: 1,
            cache_write_tokens: 2,
            total_tokens: 23,
            cost_usd: Some(0.03),
            first_question: None,
            last_question: None,
            message_count: 0,
            error_message: None,
            origin_label: Some("CLI".into()),
            origin_provider: Some("local".into()),
            source_path: Some("sessions.json".into()),
            source_format: HermesSessionSource::SessionsJson,
        }];

        enrich_state_db_sessions(&mut state_sessions, &json_sessions);

        assert_eq!(state_sessions[0].display_name.as_deref(), Some("CLI User"));
        assert_eq!(state_sessions[0].origin_label.as_deref(), Some("CLI"));
        assert_eq!(state_sessions[0].origin_provider.as_deref(), Some("local"));
        assert_eq!(state_sessions[0].total_tokens, 23);
        assert_eq!(state_sessions[0].cost_usd, Some(0.03));
        assert_eq!(
            state_sessions[0].source_format,
            HermesSessionSource::StateDb
        );
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
        // Use our own PID so the liveness check passes
        let own_pid = std::process::id();
        fs::write(temp_dir.path().join("gateway.pid"), own_pid.to_string()).expect("pid");
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
    fn gateway_status_stale_pid_is_not_running() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        // Use a PID that almost certainly doesn't exist
        fs::write(temp_dir.path().join("gateway.pid"), "99999999").expect("pid");
        fs::write(
            temp_dir.path().join("gateway_state.json"),
            r#"{"gateway_state": "running"}"#,
        )
        .expect("state");

        let (running, _, _) = check_gateway_status(temp_dir.path());
        assert!(!running, "stale PID file should not report running");
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
