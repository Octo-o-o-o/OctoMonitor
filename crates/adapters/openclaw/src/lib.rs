use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

pub use octomonitor_adapter_common::{
    mask_value, probe_file, resolve_home_dir, run_command_probe, AdapterDescriptor,
    CommandProbeResult, FileProbeResult,
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

fn openclaw_root() -> PathBuf {
    resolve_home_dir(".openclaw")
}

fn mask_tail(value: &str) -> String {
    mask_value(value, 10)
}

fn recent_session_hint(sessions_dir: &Path) -> Option<String> {
    if !sessions_dir.is_dir() {
        return None;
    }
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    if let Ok(entries) = fs::read_dir(sessions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
                            newest = Some((modified, name));
                        }
                    }
                }
            }
        }
    }
    newest.map(|(_, name)| format!("latest session artifact: {}", mask_tail(&name)))
}

/// Derive a human-readable origin label and provider from session data
fn derive_origin(
    session_key: &str,
    session_val: &serde_json::Value,
) -> (Option<String>, Option<String>) {
    let origin = session_val.get("origin");

    // Try origin.provider first
    let origin_provider_raw = origin
        .and_then(|o| o.get("provider"))
        .and_then(|v| v.as_str());
    let origin_label_raw = origin.and_then(|o| o.get("label")).and_then(|v| v.as_str());
    let origin_surface = origin
        .and_then(|o| o.get("surface"))
        .and_then(|v| v.as_str());

    // Also check session-level label (e.g. "Cron: AI Daily Brief")
    let session_label = session_val.get("label").and_then(|v| v.as_str());

    // Determine provider from origin or session key pattern
    let provider = if let Some(p) = origin_provider_raw {
        Some(p.to_string())
    } else if session_key.contains(":cron:") {
        Some("cron".to_string())
    } else if session_key.contains(":telegram:") {
        Some("telegram".to_string())
    } else if session_key.contains(":weixin:") || session_key.contains(":wechat:") {
        Some("wechat".to_string())
    } else {
        None
    };

    // Build human-readable label
    let label = match provider.as_deref() {
        Some("telegram") => {
            // Clean up origin label - remove "id:NNNNN" suffix
            let clean_name = origin_label_raw
                .map(|l| {
                    if let Some(idx) = l.find(" id:") {
                        l[..idx].to_string()
                    } else {
                        l.to_string()
                    }
                })
                .unwrap_or_else(|| "DM".to_string());
            let surface = origin_surface.unwrap_or("telegram");
            Some(format!("{}: {}", capitalize(surface), clean_name))
        }
        Some("heartbeat") => Some("Heartbeat".to_string()),
        Some("cron") => {
            // Use session label if it starts with "Cron:", otherwise build from origin
            if let Some(sl) = session_label {
                if sl.starts_with("Cron:") {
                    Some(sl.to_string())
                } else {
                    Some(format!("Cron: {}", sl))
                }
            } else {
                origin_label_raw.map(|l| format!("Cron: {}", l))
            }
        }
        Some(other) => {
            if let Some(l) = origin_label_raw {
                Some(format!("{}: {}", capitalize(other), l))
            } else {
                Some(capitalize(other))
            }
        }
        None => {
            // Fallback: try session key pattern
            if session_key.contains(":cron:") {
                session_label.map(|l| l.to_string())
            } else {
                None
            }
        }
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

/// Human-readable description of a cron expression
fn cron_to_human(expr: &str, tz: &str) -> String {
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

    let dow_str = match dow {
        "*" => "Daily".to_string(),
        "0" => "Sun".to_string(),
        "1" => "Mon".to_string(),
        "2" => "Tue".to_string(),
        "3" => "Wed".to_string(),
        "4" => "Thu".to_string(),
        "5" => "Fri".to_string(),
        "6" => "Sat".to_string(),
        combo => {
            let days: Vec<&str> = combo
                .split(',')
                .map(|d| match d {
                    "0" => "Sun",
                    "1" => "Mon",
                    "2" => "Tue",
                    "3" => "Wed",
                    "4" => "Thu",
                    "5" => "Fri",
                    "6" => "Sat",
                    _ => d,
                })
                .collect();
            days.join(",")
        }
    };

    if dow == "*" {
        format!("Daily {}", time_str)
    } else {
        format!("{} {}", dow_str, time_str)
    }
}

/// Scan cron jobs from ~/.openclaw/cron/jobs.json
fn scan_cron_jobs(root: &Path) -> Vec<OpenClawCronJob> {
    let jobs_file = root.join("cron").join("jobs.json");
    if !jobs_file.exists() {
        return vec![];
    }
    let contents = match fs::read_to_string(&jobs_file) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let val: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let jobs = match val.get("jobs").and_then(|j| j.as_array()) {
        Some(j) => j,
        None => return vec![],
    };

    jobs.iter()
        .filter_map(|j| {
            let id = j.get("id")?.as_str()?.to_string();
            let name = j.get("name")?.as_str()?.to_string();
            let enabled = j.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let schedule = j.get("schedule")?;
            let expr = schedule.get("expr")?.as_str()?.to_string();
            let tz = schedule
                .get("tz")
                .and_then(|v| v.as_str())
                .unwrap_or("UTC")
                .to_string();
            let agent_id = j
                .get("payload")
                .and_then(|p| p.get("agentId"))
                .and_then(|v| v.as_str())
                .map(String::from);

            let schedule_human = cron_to_human(&expr, &tz);

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

/// Load agent id → display name mapping from ~/.openclaw/openclaw.json agents.list
fn load_agent_name_map(root: &Path) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let config_file = root.join("openclaw.json");
    let contents = match fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(_) => return map,
    };
    let val: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return map,
    };
    if let Some(agents) = val.pointer("/agents/list").and_then(|v| v.as_array()) {
        for agent in agents {
            if let (Some(id), Some(name)) = (
                agent.get("id").and_then(|v| v.as_str()),
                agent.get("name").and_then(|v| v.as_str()),
            ) {
                map.insert(id.to_string(), name.to_string());
            }
        }
    }
    map
}

/// Scan agent sessions from sessions.json files across all agents
fn scan_agent_sessions(root: &Path) -> Vec<OpenClawSession> {
    let agents_dir = root.join("agents");
    if !agents_dir.is_dir() {
        return vec![];
    }

    let name_map = load_agent_name_map(root);
    let mut sessions = Vec::new();

    if let Ok(agents) = fs::read_dir(&agents_dir) {
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

            if let Some(agent_sessions) =
                parse_sessions_json(&sessions_json, &agent_name, &display_name)
            {
                sessions.extend(agent_sessions);
            }
        }
    }

    // Sort by updated_at descending
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
}

/// Extract first/last user question and error info from an OpenClaw session JSONL
fn extract_questions_from_jsonl(
    path: &Path,
) -> (Option<String>, Option<String>, u64, Option<String>) {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (None, None, 0, None),
    };
    let reader = BufReader::new(file);
    let mut first_question: Option<String> = None;
    let mut last_question: Option<String> = None;
    let mut message_count: u64 = 0;
    let mut error_message: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if entry_type == "message" {
            if let Some(msg) = val.get("message") {
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
                if role == "user" {
                    if let Some(text) = extract_text_content(msg) {
                        // Skip system-injected, heartbeat, and spawn contract messages
                        if text.starts_with("[cron:")
                            || text.starts_with("[[")
                            || text.starts_with("Read HEARTBEAT")
                            || text.starts_with("<system")
                            || text.starts_with("<command")
                            || text.contains("HEARTBEAT.md")
                            || text.contains("# OpenClaw Worker Spawn Contract")
                            || text.starts_with("A new session was started via")
                            || (text.len() > 5 && text.starts_with('[') && text.contains("GMT"))
                            || text.starts_with("Conversation info (untrusted")
                        {
                            continue;
                        }
                        message_count += 1;
                        let truncated: String = text.chars().take(200).collect();
                        if first_question.is_none() {
                            first_question = Some(truncated.clone());
                        }
                        last_question = Some(truncated);
                    }
                } else if role == "assistant" {
                    // Track last assistant message for error context
                    if let Some(text) = extract_text_content(msg) {
                        // Keep updating - we want the very last one
                        if text.contains("error")
                            || text.contains("Error")
                            || text.contains("ERROR")
                            || text.contains("failed")
                            || text.contains("Failed")
                            || text.contains("无法")
                        {
                            error_message = Some(text.chars().take(300).collect());
                        }
                    }
                }
            }
        }
    }

    (first_question, last_question, message_count, error_message)
}

/// Extract text content from an OpenClaw message object
fn extract_text_content(msg: &serde_json::Value) -> Option<String> {
    let content = msg.get("content")?;
    if let Some(s) = content.as_str() {
        if s.is_empty() {
            return None;
        }
        return Some(s.to_string());
    }
    if let Some(arr) = content.as_array() {
        for block in arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
        }
    }
    None
}

fn parse_sessions_json(
    path: &Path,
    agent_name: &str,
    agent_display_name: &Option<String>,
) -> Option<Vec<OpenClawSession>> {
    let mut file = fs::File::open(path).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    let val: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let obj = val.as_object()?;

    let mut result = Vec::new();

    for (session_key, session_val) in obj {
        let updated_at = session_val.get("updatedAt").and_then(|v| v.as_i64());

        let session_id = session_val
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if session_id.is_empty() {
            continue;
        }

        let status = session_val
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let model = session_val
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from);
        let model_provider = session_val
            .get("modelProvider")
            .and_then(|v| v.as_str())
            .map(String::from);
        let label = session_val
            .get("label")
            .and_then(|v| v.as_str())
            .map(String::from);
        let started_at = session_val.get("startedAt").and_then(|v| v.as_i64());
        if status.trim().is_empty() && started_at.unwrap_or_default() <= 0 {
            continue;
        }
        let context_tokens = session_val.get("contextTokens").and_then(|v| v.as_u64());

        let workspace_dir = session_val
            .pointer("/systemPromptReport/workspaceDir")
            .and_then(|v| v.as_str())
            .map(String::from);

        let input_tokens = session_val
            .get("inputTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = session_val
            .get("outputTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_tokens = session_val
            .get("totalTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = session_val
            .get("cacheRead")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_write = session_val
            .get("cacheWrite")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cost_usd = session_val.get("estimatedCostUsd").and_then(|v| v.as_f64());

        // Try to read the JSONL session file for questions and error info
        let session_file = session_val
            .get("sessionFile")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);
        let (first_question, last_question, message_count, error_message) = session_file
            .as_ref()
            .map(|p| extract_questions_from_jsonl(p))
            .unwrap_or((None, None, 0, None));

        // Extract origin info
        let (origin_label, origin_provider) = derive_origin(session_key, session_val);

        result.push(OpenClawSession {
            session_id,
            session_key: session_key.clone(),
            agent_name: agent_name.to_string(),
            label,
            status,
            model,
            model_provider,
            transcript_path: session_file.map(|path| path.display().to_string()),
            workspace_dir,
            started_at,
            updated_at,
            context_tokens,
            first_question,
            last_question,
            message_count,
            error_message,
            input_tokens,
            output_tokens,
            total_tokens,
            cache_read,
            cache_write,
            cost_usd,
            origin_label,
            origin_provider,
            agent_display_name: agent_display_name.clone(),
        });
    }

    Some(result)
}

pub fn probe() -> OpenClawSnapshot {
    let root = openclaw_root();
    let sessions_dir = root.join("agents");
    let state_file = root.join("state.json");

    let version_probe = run_command_probe("openclaw", &["--version"]);
    let status_probe = run_command_probe("openclaw", &["status"]);

    let sessions_probe = probe_file(&sessions_dir);
    let state_probe = probe_file(&state_file);

    // Scan real agent sessions
    let sessions = scan_agent_sessions(&root);
    let cron_jobs = scan_cron_jobs(&root);

    OpenClawSnapshot {
        probed_at: Utc::now().to_rfc3339(),
        cli_available: version_probe.success,
        gateway_status_ok: status_probe.success,
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
        command_probes: vec![version_probe, status_probe],
        file_probes: vec![sessions_probe, state_probe],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let parsed = parse_sessions_json(&sessions_path, "coordinator", &None)
            .expect("sessions.json should parse");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].session_id, "real-session");

        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }
}
