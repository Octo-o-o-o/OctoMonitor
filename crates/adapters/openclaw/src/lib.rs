use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

pub use octomonitor_adapter_common::{
    capitalize, file_signature, format_cron_expr, latest_file_name, mask_value,
    path_has_sensitive_component, probe_file, read_jsonl_delta, resolve_env_or_home,
    run_command_probe, truncate_chars, AdapterDescriptor, CommandProbeResult, FileProbeResult,
    JsonlCursor,
};

pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor {
        tool: "openclaw",
        preferred_mode: "gateway+status",
        fallback_mode: "sessions-scan",
        confidence: "live",
    }
}

/// Extracted agent session from OpenClaw sessions.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawSession {
    pub session_id: String,
    pub session_key: String,
    pub agent_name: String,
    pub label: Option<String>,
    pub status: String,
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub transcript_path: Option<String>,
    pub workspace_dir: Option<String>,
    pub started_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub context_tokens: Option<u64>,
    pub first_question: Option<String>,
    pub last_question: Option<String>,
    pub message_count: u64,
    pub error_message: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost_usd: Option<f64>,
    /// Human-readable source label, e.g. "Telegram: Yixiao Wang", "Cron: AI Daily Brief"
    pub origin_label: Option<String>,
    /// Source provider, e.g. "telegram", "cron", "heartbeat", "direct"
    pub origin_provider: Option<String>,
    /// Human-readable display name from openclaw.json agents.list, e.g. "Athena" for agent id "dev"
    pub agent_display_name: Option<String>,
}

/// A scheduled cron job from ~/.openclaw/cron/jobs.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawCronJob {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub agent_id: Option<String>,
    pub schedule_expr: String,
    pub schedule_tz: String,
    /// Human-readable schedule description
    pub schedule_human: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawSnapshot {
    pub probed_at: String,
    pub cli_available: bool,
    pub gateway_status_ok: bool,
    pub gateway_status: Option<String>,
    pub gateway_status_detail: Option<String>,
    pub cli_version: Option<String>,
    pub workspace_dir: Option<String>,
    pub sessions_dir_exists: bool,
    pub state_file_exists: bool,
    pub recent_session_hint: Option<String>,
    pub sessions: Vec<OpenClawSession>,
    pub cron_jobs: Vec<OpenClawCronJob>,
    pub command_probes: Vec<CommandProbeResult>,
    pub file_probes: Vec<FileProbeResult>,
}

#[derive(Debug, Clone, Default)]
pub struct OpenClawProbeCache {
    session_lists: HashMap<String, CachedSessionsFile>,
    transcript_files: HashMap<String, CachedOpenClawTranscript>,
}

#[derive(Debug, Clone)]
struct CachedSessionsFile {
    size_bytes: u64,
    modified_at: Option<SystemTime>,
    sessions: Vec<OpenClawSession>,
}

#[derive(Debug, Clone, Default)]
struct CachedOpenClawTranscript {
    cursor: JsonlCursor,
    state: OpenClawTranscriptState,
}

#[derive(Debug, Clone, Default)]
struct OpenClawTranscriptState {
    first_question: Option<String>,
    last_question: Option<String>,
    message_count: u64,
    error_message: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct OpenClawGatewayStatusProbe {
    reachable: bool,
    status: Option<String>,
    detail: Option<String>,
}

fn openclaw_root() -> PathBuf {
    resolve_env_or_home(&["OPENCLAW_STATE_DIR", "OPENCLAW_HOME"], ".openclaw")
}

fn mask_tail(value: &str) -> String {
    mask_value(value, 10)
}

fn recent_session_hint(sessions_dir: &Path) -> Option<String> {
    let name = latest_file_name(sessions_dir, None)?;
    Some(format!("latest session artifact: {}", mask_tail(&name)))
}

fn get_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

fn get_u64(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn parse_gateway_status_payload(stdout: &str) -> OpenClawGatewayStatusProbe {
    let Ok(value) = serde_json::from_str::<Value>(stdout) else {
        return OpenClawGatewayStatusProbe {
            reachable: false,
            status: Some("warning".into()),
            detail: Some("gateway status returned invalid JSON".into()),
        };
    };

    let ptr_str = |p: &str| value.pointer(p).and_then(Value::as_str);
    let ptr_bool = |p: &str| value.pointer(p).and_then(Value::as_bool);
    let ptr_array_len = |p: &str| {
        value
            .pointer(p)
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    };

    let runtime_status = ptr_str("/service/runtime/status");
    let runtime_state = ptr_str("/service/runtime/state");
    let config_ok = ptr_bool("/service/configAudit/ok").unwrap_or(true);
    let config_issue_count = ptr_array_len("/service/configAudit/issues");
    let rpc_ok = ptr_bool("/rpc/ok").unwrap_or(false);
    let health_ok = ptr_bool("/health/healthy").unwrap_or(rpc_ok);
    let stale_gateway_pid_count = ptr_array_len("/health/staleGatewayPids");

    let service_running = matches!(runtime_status, Some("running"));
    let transition_state = matches!(runtime_status, Some("starting" | "restarting"));
    let warning = (service_running || rpc_ok || transition_state)
        && (!config_ok
            || !health_ok
            || stale_gateway_pid_count > 0
            || matches!(runtime_state, Some("degraded" | "failed")));

    let status = if warning || transition_state {
        "warning"
    } else if service_running || rpc_ok {
        "running"
    } else {
        "stopped"
    };

    let mut detail_parts = Vec::new();
    if let Some(rs) = runtime_status {
        let mut service_part = format!("service {rs}");
        if let Some(state) = runtime_state.filter(|s| !s.is_empty()) {
            service_part.push_str(&format!(" ({state})"));
        }
        detail_parts.push(service_part);
    }
    detail_parts.push(if rpc_ok { "rpc reachable" } else { "rpc unreachable" }.into());
    if !health_ok {
        detail_parts.push("health unhealthy".into());
    }
    if !config_ok {
        detail_parts.push(if config_issue_count > 0 {
            format!("config issues: {config_issue_count}")
        } else {
            "config issues".into()
        });
    }
    if stale_gateway_pid_count > 0 {
        detail_parts.push(format!("stale gateway pids: {stale_gateway_pid_count}"));
    }

    OpenClawGatewayStatusProbe {
        reachable: rpc_ok,
        status: Some(status.into()),
        detail: (!detail_parts.is_empty()).then(|| detail_parts.join(" | ")),
    }
}

fn run_gateway_status_probe() -> (CommandProbeResult, OpenClawGatewayStatusProbe) {
    const COMMAND: &str = "openclaw gateway status --json";
    let snippet = |s: &str| (!s.is_empty()).then(|| truncate_chars(s, 200));
    match Command::new("openclaw")
        .args(["gateway", "status", "--json"])
        .output()
    {
        Ok(output) => {
            let success = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let parsed = if success {
                parse_gateway_status_payload(&stdout)
            } else {
                OpenClawGatewayStatusProbe::default()
            };
            let probe = CommandProbeResult {
                command: COMMAND.into(),
                success,
                stdout_snippet: snippet(&stdout),
                error: (!success).then(|| snippet(&stderr)).flatten(),
            };
            (probe, parsed)
        }
        Err(error) => (
            CommandProbeResult {
                command: COMMAND.into(),
                success: false,
                stdout_snippet: None,
                error: Some(error.to_string()),
            },
            OpenClawGatewayStatusProbe::default(),
        ),
    }
}

/// Derive a human-readable origin label and provider from session data
fn derive_origin(session_key: &str, session_val: &Value) -> (Option<String>, Option<String>) {
    let origin = session_val.get("origin");
    let origin_provider_raw = origin.and_then(|o| o.get("provider")).and_then(Value::as_str);
    let origin_label_raw = origin.and_then(|o| o.get("label")).and_then(Value::as_str);
    let origin_surface = origin.and_then(|o| o.get("surface")).and_then(Value::as_str);
    let session_label = session_val.get("label").and_then(Value::as_str);

    let provider = origin_provider_raw.map(String::from).or_else(|| {
        const PATTERNS: &[(&str, &str)] = &[
            (":cron:", "cron"),
            (":telegram:", "telegram"),
            (":weixin:", "wechat"),
            (":wechat:", "wechat"),
        ];
        PATTERNS
            .iter()
            .find(|(pattern, _)| session_key.contains(pattern))
            .map(|(_, name)| (*name).to_string())
    });

    let label = match provider.as_deref() {
        Some("telegram") => {
            let clean_name = origin_label_raw
                .map(|l| l.split(" id:").next().unwrap_or(l).to_string())
                .unwrap_or_else(|| "DM".to_string());
            let surface = origin_surface.unwrap_or("telegram");
            Some(format!("{}: {}", capitalize(surface), clean_name))
        }
        Some("heartbeat") => Some("Heartbeat".to_string()),
        Some("cron") => match session_label {
            Some(sl) if sl.starts_with("Cron:") => Some(sl.to_string()),
            Some(sl) => Some(format!("Cron: {sl}")),
            None => origin_label_raw.map(|l| format!("Cron: {l}")),
        },
        Some(other) => Some(match origin_label_raw {
            Some(l) => format!("{}: {}", capitalize(other), l),
            None => capitalize(other),
        }),
        None => None,
    };

    (label, provider)
}

fn scan_cron_jobs(root: &Path) -> Vec<OpenClawCronJob> {
    let Ok(contents) = fs::read_to_string(root.join("cron").join("jobs.json")) else {
        return vec![];
    };
    let Ok(val) = serde_json::from_str::<Value>(&contents) else {
        return vec![];
    };
    let Some(jobs) = val.get("jobs").and_then(Value::as_array) else {
        return vec![];
    };

    jobs.iter()
        .filter_map(|j| {
            let id = j.get("id")?.as_str()?.to_string();
            let name = j.get("name")?.as_str()?.to_string();
            let enabled = j.get("enabled").and_then(Value::as_bool).unwrap_or(false);
            let schedule = j.get("schedule")?;
            let expr = schedule.get("expr")?.as_str()?.to_string();
            let tz = schedule
                .get("tz")
                .and_then(Value::as_str)
                .unwrap_or("UTC")
                .to_string();
            let agent_id = j
                .get("payload")
                .and_then(|p| p.get("agentId"))
                .and_then(Value::as_str)
                .map(String::from);

            let schedule_human = format_cron_expr(&expr, &tz);

            Some(OpenClawCronJob {
                id,
                name,
                enabled,
                agent_id,
                schedule_expr: expr,
                schedule_tz: tz,
                schedule_human,
            })
        })
        .collect()
}

fn load_agent_name_map(root: &Path) -> HashMap<String, String> {
    let Ok(contents) = fs::read_to_string(root.join("openclaw.json")) else {
        return HashMap::new();
    };
    let Ok(val) = serde_json::from_str::<Value>(&contents) else {
        return HashMap::new();
    };
    let Some(agents) = val.pointer("/agents/list").and_then(Value::as_array) else {
        return HashMap::new();
    };
    agents
        .iter()
        .filter_map(|agent| {
            let id = agent.get("id").and_then(Value::as_str)?.to_string();
            let name = agent.get("name").and_then(Value::as_str)?.to_string();
            Some((id, name))
        })
        .collect()
}

fn scan_agent_sessions(
    root: &Path,
    mut cache: Option<&mut OpenClawProbeCache>,
) -> Vec<OpenClawSession> {
    let agents_dir = root.join("agents");
    if !agents_dir.is_dir() {
        return vec![];
    }

    let name_map = load_agent_name_map(root);
    let mut sessions = Vec::new();
    let mut seen_session_lists = HashSet::new();

    let Ok(agents) = fs::read_dir(&agents_dir) else {
        return sessions;
    };
    for agent_entry in agents.flatten() {
        if !agent_entry.path().is_dir() {
            continue;
        }
        let agent_name = agent_entry.file_name().to_string_lossy().to_string();
        let display_name = name_map.get(&agent_name).cloned();
        let sessions_json = agent_entry.path().join("sessions").join("sessions.json");

        if !sessions_json.exists() {
            continue;
        }
        seen_session_lists.insert(sessions_json.display().to_string());

        let agent_sessions = match cache.as_deref_mut() {
            Some(cache) => {
                load_cached_agent_sessions(cache, &sessions_json, &agent_name, &display_name)
            }
            None => parse_sessions_json_base(&sessions_json, &agent_name, &display_name),
        };
        if let Some(agent_sessions) = agent_sessions {
            sessions.extend(agent_sessions);
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    if let Some(cache) = cache {
        let live_transcripts: HashSet<_> = sessions
            .iter()
            .filter_map(|session| session.transcript_path.clone())
            .collect();
        cache
            .session_lists
            .retain(|path, _| seen_session_lists.contains(path));
        cache
            .transcript_files
            .retain(|path, _| live_transcripts.contains(path));
    }
    sessions
}

fn is_synthetic_user_message(text: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "[cron:",
        "[[",
        "Read HEARTBEAT",
        "<system",
        "<command",
        "A new session was started via",
        "Conversation info (untrusted",
    ];
    if PREFIXES.iter().any(|p| text.starts_with(p)) {
        return true;
    }
    if text.contains("HEARTBEAT.md") || text.contains("# OpenClaw Worker Spawn Contract") {
        return true;
    }
    text.len() > 5 && text.starts_with('[') && text.contains("GMT")
}

fn apply_openclaw_transcript_line(state: &mut OpenClawTranscriptState, line: &str) {
    let Ok(val) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if val.get("type").and_then(Value::as_str) != Some("message") {
        return;
    }
    let Some(msg) = val.get("message") else {
        return;
    };
    let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
    let Some(text) = extract_text_content(msg) else {
        return;
    };

    match role {
        "user" => {
            if is_synthetic_user_message(&text) {
                return;
            }
            state.message_count += 1;
            let truncated = truncate_chars(&text, 200);
            if state.first_question.is_none() {
                state.first_question = Some(truncated.clone());
            }
            state.last_question = Some(truncated);
        }
        "assistant" => {
            let lower = text.to_lowercase();
            if lower.contains("error") || lower.contains("failed") || text.contains("无法") {
                state.error_message = Some(truncate_chars(&text, 300));
            }
        }
        _ => {}
    }
}

fn extract_text_content(msg: &Value) -> Option<String> {
    let content = msg.get("content")?;
    if let Some(s) = content.as_str() {
        return (!s.is_empty()).then(|| s.to_string());
    }
    content.as_array()?.iter().find_map(|block| {
        if block.get("type").and_then(Value::as_str) != Some("text") {
            return None;
        }
        block
            .get("text")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(String::from)
    })
}

fn parse_sessions_json_base(
    path: &Path,
    agent_name: &str,
    agent_display_name: &Option<String>,
) -> Option<Vec<OpenClawSession>> {
    if path_has_sensitive_component(path) {
        return None;
    }
    let contents = fs::read_to_string(path).ok()?;
    let val: Value = serde_json::from_str(&contents).ok()?;
    let obj = val.as_object()?;

    let mut result = Vec::new();

    for (session_key, session_val) in obj {
        let Some(session_id) = session_val
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
        else {
            continue;
        };

        let status = session_val
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let started_at = session_val.get("startedAt").and_then(Value::as_i64);
        if status.trim().is_empty() && started_at.unwrap_or_default() <= 0 {
            continue;
        }

        let workspace_dir = session_val
            .pointer("/systemPromptReport/workspaceDir")
            .and_then(Value::as_str)
            .map(String::from);

        let transcript_path = session_val
            .get("sessionFile")
            .and_then(Value::as_str)
            .map(|s| PathBuf::from(s).display().to_string());

        let (origin_label, origin_provider) = derive_origin(session_key, session_val);

        result.push(OpenClawSession {
            session_id,
            session_key: session_key.clone(),
            agent_name: agent_name.to_string(),
            label: get_str(session_val, "label"),
            status,
            model: get_str(session_val, "model"),
            model_provider: get_str(session_val, "modelProvider"),
            transcript_path,
            workspace_dir,
            started_at,
            updated_at: session_val.get("updatedAt").and_then(Value::as_i64),
            context_tokens: session_val.get("contextTokens").and_then(Value::as_u64),
            first_question: None,
            last_question: None,
            message_count: 0,
            error_message: None,
            input_tokens: get_u64(session_val, "inputTokens"),
            output_tokens: get_u64(session_val, "outputTokens"),
            total_tokens: get_u64(session_val, "totalTokens"),
            cache_read: get_u64(session_val, "cacheRead"),
            cache_write: get_u64(session_val, "cacheWrite"),
            cost_usd: session_val.get("estimatedCostUsd").and_then(Value::as_f64),
            origin_label,
            origin_provider,
            agent_display_name: agent_display_name.clone(),
        });
    }

    Some(result)
}

fn update_cached_openclaw_transcript<'a>(
    cached: &'a mut CachedOpenClawTranscript,
    path: &Path,
) -> &'a OpenClawTranscriptState {
    if let Ok(delta) = read_jsonl_delta(path, &mut cached.cursor) {
        if delta.reset {
            cached.state = OpenClawTranscriptState::default();
        }
        for line in delta.lines {
            apply_openclaw_transcript_line(&mut cached.state, &line);
        }
    }
    &cached.state
}

fn load_cached_agent_sessions(
    cache: &mut OpenClawProbeCache,
    sessions_json: &Path,
    agent_name: &str,
    agent_display_name: &Option<String>,
) -> Option<Vec<OpenClawSession>> {
    let cache_key = sessions_json.display().to_string();
    let signature = file_signature(sessions_json)?;
    let base_sessions = match cache.session_lists.get(&cache_key) {
        Some(cached) if cached.size_bytes == signature.0 && cached.modified_at == signature.1 => {
            cached.sessions.clone()
        }
        _ => {
            let sessions = parse_sessions_json_base(sessions_json, agent_name, agent_display_name)?;
            cache.session_lists.insert(
                cache_key,
                CachedSessionsFile {
                    size_bytes: signature.0,
                    modified_at: signature.1,
                    sessions: sessions.clone(),
                },
            );
            sessions
        }
    };

    Some(
        base_sessions
            .into_iter()
            .map(|mut session| {
                if let Some(path) = session.transcript_path.as_deref() {
                    let cached = cache
                        .transcript_files
                        .entry(path.to_string())
                        .or_default();
                    let state = update_cached_openclaw_transcript(cached, Path::new(path));
                    session.first_question = state.first_question.clone();
                    session.last_question = state.last_question.clone();
                    session.message_count = state.message_count;
                    session.error_message = state.error_message.clone();
                }
                session
            })
            .collect(),
    )
}

pub fn probe_with_cache(cache: &mut OpenClawProbeCache) -> OpenClawSnapshot {
    let root = openclaw_root();
    let sessions_dir = root.join("agents");
    let state_file = root.join("state.json");

    let version_probe = run_command_probe("openclaw", &["--version"]);
    let (gateway_probe, gateway_state) = run_gateway_status_probe();

    let sessions_probe = probe_file(&sessions_dir);
    let state_probe = probe_file(&state_file);

    let sessions = scan_agent_sessions(&root, Some(cache));
    let cron_jobs = scan_cron_jobs(&root);

    OpenClawSnapshot {
        probed_at: Utc::now().to_rfc3339(),
        cli_available: version_probe.success,
        gateway_status_ok: gateway_state.reachable,
        gateway_status: gateway_state.status,
        gateway_status_detail: gateway_state.detail,
        cli_version: version_probe
            .stdout_snippet
            .as_ref()
            .map(|s| s.trim().to_string()),
        workspace_dir: Some(root.display().to_string()),
        sessions_dir_exists: sessions_probe.exists,
        state_file_exists: state_probe.exists,
        recent_session_hint: recent_session_hint(&sessions_dir),
        sessions,
        cron_jobs,
        command_probes: vec![version_probe, gateway_probe],
        file_probes: vec![sessions_probe, state_probe],
    }
}

pub fn probe() -> OpenClawSnapshot {
    let mut cache = OpenClawProbeCache::default();
    probe_with_cache(&mut cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn descriptor_returns_openclaw() {
        let d = descriptor();
        assert_eq!(d.tool, "openclaw");
    }

    #[test]
    fn probe_runs_without_panic() {
        let snap = probe();
        assert!(!snap.probed_at.is_empty());
        assert_eq!(snap.command_probes.len(), 2);
    }

    #[test]
    fn parse_sessions_json_skips_unstarted_blank_status_placeholders() {
        let temp_dir = std::env::temp_dir().join(format!(
            "octomonitor-openclaw-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("current time should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");

        let sessions_path = temp_dir.join("sessions.json");
        fs::write(
            &sessions_path,
            r#"{
  "agent:coordinator:cron:placeholder": {
    "sessionId": "placeholder-session",
    "updatedAt": 1775053801774,
    "label": "Cron: Placeholder"
  },
  "agent:coordinator:cron:real-run": {
    "sessionId": "real-session",
    "status": "running",
    "updatedAt": 1775053801774,
    "startedAt": 1775050201774,
    "label": "Cron: Real Run"
  }
}"#,
        )
        .expect("sessions.json should be written");

        let parsed = parse_sessions_json_base(&sessions_path, "coordinator", &None)
            .expect("sessions.json should parse");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].session_id, "real-session");

        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn cached_transcript_updates_only_new_jsonl_lines() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let transcript = temp_dir.path().join("session.jsonl");
        std::fs::write(
            &transcript,
            "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"first question\"}}\n",
        )
        .expect("initial transcript");

        let mut cached = CachedOpenClawTranscript::default();
        let first = update_cached_openclaw_transcript(&mut cached, &transcript);
        assert_eq!(first.first_question.as_deref(), Some("first question"));
        assert_eq!(first.message_count, 1);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&transcript)
            .expect("append transcript");
        writeln!(
            file,
            "{{\"type\":\"message\",\"message\":{{\"role\":\"assistant\",\"content\":\"failed to run\"}}}}"
        )
        .expect("append assistant");
        writeln!(
            file,
            "{{\"type\":\"message\",\"message\":{{\"role\":\"user\",\"content\":\"second question\"}}}}"
        )
        .expect("append user");

        let second = update_cached_openclaw_transcript(&mut cached, &transcript);
        assert_eq!(second.first_question.as_deref(), Some("first question"));
        assert_eq!(second.last_question.as_deref(), Some("second question"));
        assert_eq!(second.message_count, 2);
        assert_eq!(second.error_message.as_deref(), Some("failed to run"));
    }

    #[test]
    fn parse_gateway_status_marks_running_when_rpc_is_reachable_and_healthy() {
        let parsed = parse_gateway_status_payload(
            r#"{
  "service": {
    "runtime": { "status": "running", "state": "active" },
    "configAudit": { "ok": true, "issues": [] }
  },
  "rpc": { "ok": true },
  "health": { "healthy": true, "staleGatewayPids": [] }
}"#,
        );

        assert!(parsed.reachable);
        assert_eq!(parsed.status.as_deref(), Some("running"));
        assert_eq!(
            parsed.detail.as_deref(),
            Some("service running (active) | rpc reachable")
        );
    }

    #[test]
    fn parse_gateway_status_marks_warning_when_gateway_is_live_but_unhealthy() {
        let parsed = parse_gateway_status_payload(
            r#"{
  "service": {
    "runtime": { "status": "running", "state": "degraded" },
    "configAudit": { "ok": false, "issues": ["port mismatch"] }
  },
  "rpc": { "ok": true },
  "health": { "healthy": false, "staleGatewayPids": [123] }
}"#,
        );

        assert!(parsed.reachable);
        assert_eq!(parsed.status.as_deref(), Some("warning"));
        assert_eq!(
            parsed.detail.as_deref(),
            Some(
                "service running (degraded) | rpc reachable | health unhealthy | config issues: 1 | stale gateway pids: 1"
            )
        );
    }
}
