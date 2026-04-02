use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

pub use octomonitor_adapter_common::{
    mask_value, probe_file, resolve_home_dir, run_command_probe, AdapterDescriptor,
    CommandProbeResult, FileProbeResult,
};

pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor {
        tool: "claude",
        preferred_mode: "hook+statusline",
        fallback_mode: "transcript",
        confidence: "live",
    }
}

/// Extracted session data from a Claude project transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    pub session_id: String,
    pub project_path: String,
    pub project_name: String,
    pub transcript_path: String,
    pub model: Option<String>,
    pub started_at: String,
    pub last_activity_at: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub message_count: u64,
    pub first_question: Option<String>,
    pub last_question: Option<String>,
    /// Sum of (user_message_timestamp → next_assistant_response_timestamp) intervals in ms.
    /// Excludes idle time between an assistant response and the next user message.
    pub active_elapsed_ms: i64,
}

/// Usage quota data from Claude HUD cache
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeQuota {
    pub plan_name: Option<String>,
    pub five_hour_used_pct: Option<f64>,
    pub seven_day_used_pct: Option<f64>,
    pub five_hour_reset_at: Option<String>,
    pub seven_day_reset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSnapshot {
    pub probed_at: String,
    pub cli_available: bool,
    pub cli_version: Option<String>,
    pub config_dir: Option<String>,
    pub config_exists: bool,
    pub projects_dir_exists: bool,
    pub active_session_hint: Option<String>,
    pub sessions: Vec<ClaudeSession>,
    pub quota: Option<ClaudeQuota>,
    pub command_probes: Vec<CommandProbeResult>,
    pub file_probes: Vec<FileProbeResult>,
}

fn mask_secret(value: &str) -> String {
    mask_value(value, 8)
}

/// Claude config directory: ~/.claude
fn claude_config_dir() -> PathBuf {
    env::var("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| resolve_home_dir(".claude"))
}

/// Decode project folder name back to workspace path.
/// Claude Code encodes every `/` as `-`, so `-Users-dev-WorkSpace-Foo`
/// becomes `/Users/dev/WorkSpace/Foo`. This is a lossy encoding — genuine
/// hyphens in directory names are indistinguishable from separators.
/// We restore the path by replacing all `-` with `/`, which matches
/// Claude Code's own decoding behaviour.
fn decode_project_folder(folder_name: &str) -> String {
    if folder_name.starts_with('-') {
        folder_name.replace('-', "/")
    } else {
        folder_name.to_string()
    }
}

/// Extract short project name from a workspace path
fn project_name_from_path(path: &str) -> String {
    path.split('/')
        .filter(|s| !s.is_empty())
        .next_back()
        .unwrap_or("unknown")
        .to_string()
}

/// Scan Claude projects directory for all available sessions
fn scan_sessions(config_dir: &Path) -> Vec<ClaudeSession> {
    let projects_dir = config_dir.join("projects");
    if !projects_dir.is_dir() {
        return vec![];
    }

    let mut sessions = Vec::new();

    let entries = match fs::read_dir(&projects_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    for entry in entries.flatten() {
        let folder_path = entry.path();
        if !folder_path.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        let workspace_path = decode_project_folder(&folder_name);
        let project_name = project_name_from_path(&workspace_path);

        // Find .jsonl transcript files in this project folder
        if let Ok(files) = fs::read_dir(&folder_path) {
            for file_entry in files.flatten() {
                let path = file_entry.path();
                if path.extension().is_none_or(|ext| ext != "jsonl") {
                    continue;
                }
                if let Some(session) = parse_claude_session(&path, &workspace_path, &project_name) {
                    sessions.push(session);
                }
            }
        }
    }

    // Sort by last_activity_at descending
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

/// Parse a single Claude transcript JSONL file (reads only first and last lines for speed)
fn parse_claude_session(
    path: &Path,
    workspace_path: &str,
    project_name: &str,
) -> Option<ClaudeSession> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id: Option<String> = None;
    let mut model: Option<String> = None;
    let mut first_timestamp: Option<String> = None;
    let mut last_timestamp: Option<String> = None;
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_write_tokens: u64 = 0;
    let mut total_cost_usd: f64 = 0.0;
    let mut message_count: u64 = 0;
    let mut first_question: Option<String> = None;
    let mut last_question: Option<String> = None;
    // Track active working time: sum of (user_msg → assistant_response) intervals
    let mut active_elapsed_ms: i64 = 0;
    let mut pending_user_ts: Option<chrono::DateTime<chrono::FixedOffset>> = None;

    for line in reader.lines().map_while(Result::ok) {
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Extract sessionId
        if session_id.is_none() {
            if let Some(sid) = val.get("sessionId").and_then(|v| v.as_str()) {
                session_id = Some(sid.to_string());
            }
        }

        let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Track timestamps
        if let Some(ts) = val.get("timestamp").and_then(|v| v.as_str()) {
            if first_timestamp.is_none() {
                first_timestamp = Some(ts.to_string());
            }
            last_timestamp = Some(ts.to_string());
        }

        // Parse current line timestamp for active-time tracking
        let line_dt = val
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok());

        if msg_type == "user" {
            // Record user message timestamp to start an active interval
            if let Some(dt) = line_dt {
                pending_user_ts = Some(dt);
            }
            // Extract user message text
            if let Some(message) = val.get("message") {
                let text = if let Some(content) = message.get("content") {
                    if let Some(s) = content.as_str() {
                        Some(s.to_string())
                    } else if let Some(arr) = content.as_array() {
                        arr.iter().find_map(|block| {
                            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                block.get("text").and_then(|t| t.as_str()).map(String::from)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(t) = text {
                    // Skip system-injected messages
                    if t.starts_with("<local-command-caveat>")
                        || t.starts_with("<system-reminder>")
                        || t.starts_with("<command-name>")
                        || t.starts_with("<task-notification>")
                    {
                        continue;
                    }
                    let truncated: String = t.chars().take(200).collect();
                    message_count += 1;
                    if first_question.is_none() {
                        first_question = Some(truncated.clone());
                    }
                    last_question = Some(truncated);
                }
            }
        }

        if msg_type == "assistant" {
            // Close an active interval: user_msg → assistant_response
            if let (Some(user_dt), Some(asst_dt)) = (pending_user_ts.take(), line_dt) {
                let interval = (asst_dt - user_dt).num_milliseconds().max(0);
                active_elapsed_ms += interval;
            }
            // Extract model
            if let Some(m) = val
                .get("message")
                .and_then(|msg| msg.get("model").and_then(|v| v.as_str()).map(String::from))
            {
                model = Some(m);
            }
            // Extract usage from assistant messages
            if let Some(usage) = val.get("message").and_then(|msg| msg.get("usage")) {
                input_tokens += usage
                    .get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                output_tokens += usage
                    .get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                cache_read_tokens += usage
                    .get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                cache_write_tokens += usage
                    .get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            }
            // Extract cost
            if let Some(cost) = val.get("costUSD").and_then(|v| v.as_f64()) {
                total_cost_usd += cost;
            }
        }
    }

    // If there's a pending user message without a response (still active), count up to now
    if let Some(user_dt) = pending_user_ts {
        let now = chrono::Utc::now().fixed_offset();
        let interval = (now - user_dt).num_milliseconds().max(0);
        active_elapsed_ms += interval;
    }

    let sid = session_id.unwrap_or_else(|| {
        // Derive from filename
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into())
    });

    let total_tokens = input_tokens + output_tokens + cache_read_tokens + cache_write_tokens;

    // Use file timestamps as fallback
    let file_meta = fs::metadata(path).ok();
    let file_modified = file_meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339());

    Some(ClaudeSession {
        session_id: sid,
        project_path: workspace_path.to_string(),
        project_name: project_name.to_string(),
        transcript_path: path.display().to_string(),
        model,
        started_at: first_timestamp.unwrap_or_else(|| file_modified.clone().unwrap_or_default()),
        last_activity_at: last_timestamp.unwrap_or_else(|| file_modified.unwrap_or_default()),
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        total_tokens,
        cost_usd: if total_cost_usd > 0.0 {
            Some(total_cost_usd)
        } else {
            None
        },
        message_count,
        first_question,
        last_question,
        active_elapsed_ms,
    })
}

/// Read Claude HUD usage cache for quota percentages
fn read_hud_usage_cache(config_dir: &Path) -> Option<ClaudeQuota> {
    let cache_path = config_dir.join("plugins/claude-hud/.usage-cache.json");
    let mut file = fs::File::open(&cache_path).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    let val: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let data = val.get("data")?;

    let plan_name = data
        .get("planName")
        .and_then(|v| v.as_str())
        .map(String::from);
    let five_hour = data.get("fiveHour").and_then(|v| v.as_f64());
    let seven_day = data.get("sevenDay").and_then(|v| v.as_f64());
    let five_hour_reset = data
        .get("fiveHourResetAt")
        .and_then(|v| v.as_str())
        .map(String::from);
    let seven_day_reset = data
        .get("sevenDayResetAt")
        .and_then(|v| v.as_str())
        .map(String::from);

    Some(ClaudeQuota {
        plan_name,
        five_hour_used_pct: five_hour,
        seven_day_used_pct: seven_day,
        five_hour_reset_at: five_hour_reset,
        seven_day_reset_at: seven_day_reset,
    })
}

/// Full probe: CLI version check + filesystem scan for config/sessions
pub fn probe() -> ClaudeSnapshot {
    let config_dir = claude_config_dir();
    let projects_dir = config_dir.join("projects");

    let version_probe = run_command_probe("claude", &["--version"]);
    let cli_available = version_probe.success;
    let cli_version = version_probe
        .stdout_snippet
        .as_ref()
        .map(|s| s.trim().to_string());

    let config_file = config_dir.join("settings.json");
    let config_probe = probe_file(&config_file);
    let projects_probe = probe_file(&projects_dir);

    // Look for recent transcript/session files (without reading content)
    let active_session_hint = find_recent_session(&config_dir);

    // Scan real sessions
    let sessions = scan_sessions(&config_dir);

    // Read HUD usage cache for quota
    let quota = read_hud_usage_cache(&config_dir);

    ClaudeSnapshot {
        probed_at: Utc::now().to_rfc3339(),
        cli_available,
        cli_version,
        config_dir: Some(config_dir.display().to_string()),
        config_exists: config_probe.exists,
        projects_dir_exists: projects_probe.exists,
        active_session_hint,
        sessions,
        quota,
        command_probes: vec![version_probe],
        file_probes: vec![config_probe, projects_probe],
    }
}

fn find_recent_session(config_dir: &Path) -> Option<String> {
    // Scan for .jsonl transcript files without reading their content
    let transcripts_dir = config_dir.join("transcripts");
    if !transcripts_dir.is_dir() {
        return None;
    }
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    if let Ok(entries) = fs::read_dir(&transcripts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
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
    newest.map(|(_, name)| format!("most recent transcript: {}", mask_secret(&name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_returns_claude() {
        let d = descriptor();
        assert_eq!(d.tool, "claude");
    }

    #[test]
    fn probe_runs_without_panic() {
        let snap = probe();
        assert!(!snap.probed_at.is_empty());
        assert!(!snap.command_probes.is_empty());
        assert!(!snap.file_probes.is_empty());
    }

    #[test]
    fn mask_secret_works() {
        assert_eq!(mask_secret("short"), "****");
        assert_eq!(mask_secret("longersecretvalue"), "long…alue");
    }

    #[test]
    fn decode_project_folder_works() {
        assert_eq!(
            decode_project_folder("-Users-dev-WorkSpace-OctoMonitor"),
            "/Users/dev/WorkSpace/OctoMonitor"
        );
        assert_eq!(decode_project_folder("plain-name"), "plain-name");
    }
}
