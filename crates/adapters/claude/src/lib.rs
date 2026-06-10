use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

pub use octomonitor_adapter_common::{
    latest_file_name, mask_value, path_has_sensitive_component, probe_file, read_jsonl_delta,
    resolve_env_or_home, run_command_probe, truncate_chars, AdapterDescriptor, CommandProbeResult,
    FileProbeResult, JsonlCursor,
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
    /// True when the last assistant message contains a `tool_use` block
    /// with no subsequent `tool_result` from the user — indicates the
    /// session is waiting for permission approval.
    pub has_pending_tool_use: bool,
}

/// Usage quota data from Claude statusline/legacy cache integrations.
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

#[derive(Debug, Clone, Default)]
pub struct ClaudeProbeCache {
    session_files: HashMap<String, CachedClaudeSession>,
}

#[derive(Debug, Clone, Default)]
struct CachedClaudeSession {
    cursor: JsonlCursor,
    state: ClaudeSessionState,
}

#[derive(Debug, Clone, Default)]
struct ClaudeSessionState {
    session_id: Option<String>,
    model: Option<String>,
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    total_cost_usd: f64,
    message_count: u64,
    first_question: Option<String>,
    last_question: Option<String>,
    completed_active_elapsed_ms: i64,
    pending_user_ts: Option<chrono::DateTime<chrono::FixedOffset>>,
    counted_assistant_message_ids: HashSet<String>,
    counted_task_notification_ids: HashSet<String>,
    external_total_tokens: u64,
    /// True when the last assistant message had tool_use and no tool_result followed.
    has_pending_tool_use: bool,
}

fn mask_secret(value: &str) -> String {
    mask_value(value, 8)
}

fn claude_config_dir() -> PathBuf {
    resolve_env_or_home(&["CLAUDE_CONFIG_DIR"], ".claude")
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
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn scan_sessions(
    config_dir: &Path,
    mut cache: Option<&mut ClaudeProbeCache>,
) -> Vec<ClaudeSession> {
    let projects_dir = config_dir.join("projects");
    if !projects_dir.is_dir() {
        return vec![];
    }

    let Ok(entries) = fs::read_dir(&projects_dir) else {
        return vec![];
    };

    let mut sessions = Vec::new();
    let mut seen_paths = HashSet::new();

    for entry in entries.flatten() {
        let folder_path = entry.path();
        if !folder_path.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().into_owned();
        let workspace_path = decode_project_folder(&folder_name);
        let project_name = project_name_from_path(&workspace_path);

        let Ok(files) = fs::read_dir(&folder_path) else {
            continue;
        };
        for file_entry in files.flatten() {
            let path = file_entry.path();
            if path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            if path_has_sensitive_component(&path) {
                continue;
            }
            let cache_key = path.display().to_string();
            let session = match cache.as_deref_mut() {
                Some(cache) => {
                    let entry = cache.session_files.entry(cache_key.clone()).or_default();
                    update_cached_claude_session(entry, &path, &workspace_path, &project_name)
                }
                None => parse_claude_session(&path, &workspace_path, &project_name),
            };
            if let Some(session) = session {
                seen_paths.insert(cache_key);
                sessions.push(session);
            }
        }
    }

    if let Some(cache) = cache {
        cache
            .session_files
            .retain(|path, _| seen_paths.contains(path));
    }

    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn block_type(block: &serde_json::Value) -> Option<&str> {
    block.get("type").and_then(|t| t.as_str())
}

fn message_content_array(val: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
    val.get("message")?.get("content")?.as_array()
}

fn message_has_block(val: &serde_json::Value, kind: &str) -> bool {
    message_content_array(val)
        .is_some_and(|arr| arr.iter().any(|b| block_type(b) == Some(kind)))
}

fn extract_text_from_content(msg: &serde_json::Value) -> Option<String> {
    let content = msg.get("content")?;
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    content.as_array()?.iter().find_map(|block| {
        (block_type(block) == Some("text"))
            .then(|| block.get("text").and_then(|t| t.as_str()).map(String::from))
            .flatten()
    })
}

fn extract_xml_tag<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].trim())
}

fn extract_task_notification_usage(text: &str) -> Option<(String, u64)> {
    let task_id = extract_xml_tag(text, "task-id")?.to_string();
    let total_tokens = extract_xml_tag(text, "total_tokens")?.parse().ok()?;
    Some((task_id, total_tokens))
}

fn extract_assistant_message_key(val: &serde_json::Value) -> Option<String> {
    let str_at = |path: &[&str]| -> Option<String> {
        let mut cursor = val;
        for key in path {
            cursor = cursor.get(*key)?;
        }
        cursor.as_str().map(String::from)
    };
    str_at(&["message", "id"])
        .or_else(|| str_at(&["requestId"]))
        .or_else(|| str_at(&["uuid"]))
}

fn record_task_notification_usage(state: &mut ClaudeSessionState, text: &str) {
    let Some((task_id, total_tokens)) = extract_task_notification_usage(text) else {
        return;
    };
    if state.counted_task_notification_ids.insert(task_id) {
        state.external_total_tokens = state.external_total_tokens.saturating_add(total_tokens);
    }
}

fn parse_claude_session(
    path: &Path,
    workspace_path: &str,
    project_name: &str,
) -> Option<ClaudeSession> {
    let mut cached = CachedClaudeSession::default();
    update_cached_claude_session(&mut cached, path, workspace_path, project_name)
}

fn apply_claude_line(state: &mut ClaudeSessionState, line: &str) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };

    if state.session_id.is_none() {
        if let Some(sid) = val.get("sessionId").and_then(|v| v.as_str()) {
            state.session_id = Some(sid.to_string());
        }
    }

    let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

    if msg_type == "queue-operation" {
        if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
            record_task_notification_usage(state, content);
        }
        return;
    }

    if let Some(ts) = val.get("timestamp").and_then(|v| v.as_str()) {
        if state.first_timestamp.is_none() {
            state.first_timestamp = Some(ts.to_string());
        }
        state.last_timestamp = Some(ts.to_string());
    }

    let line_dt = val
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok());

    if msg_type == "user" {
        if let Some(dt) = line_dt {
            state.pending_user_ts = Some(dt);
        }
        if message_has_block(&val, "tool_result") {
            state.has_pending_tool_use = false;
        }
        if let Some(t) = val.get("message").and_then(extract_text_from_content) {
            if t.starts_with("<local-command-caveat>")
                || t.starts_with("<system-reminder>")
                || t.starts_with("<command-name>")
            {
                return;
            }
            if t.starts_with("<task-notification>") {
                record_task_notification_usage(state, &t);
                return;
            }
            let truncated = truncate_chars(&t, 200);
            state.message_count += 1;
            if state.first_question.is_none() {
                state.first_question = Some(truncated.clone());
            }
            state.last_question = Some(truncated);
        }
    }

    if msg_type == "assistant" {
        if message_has_block(&val, "tool_use") {
            state.has_pending_tool_use = true;
        }

        if let (Some(user_dt), Some(asst_dt)) = (state.pending_user_ts.take(), line_dt) {
            let interval = (asst_dt - user_dt).num_milliseconds().max(0);
            state.completed_active_elapsed_ms += interval;
        }
        if let Some(m) = val
            .get("message")
            .and_then(|msg| msg.get("model"))
            .and_then(|v| v.as_str())
        {
            state.model = Some(m.to_string());
        }
        let should_count_usage = extract_assistant_message_key(&val)
            .map(|id| state.counted_assistant_message_ids.insert(id))
            .unwrap_or(true);
        if should_count_usage {
            if let Some(usage) = val.get("message").and_then(|msg| msg.get("usage")) {
                let u = |key: &str| usage.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
                state.input_tokens += u("input_tokens");
                state.output_tokens += u("output_tokens");
                state.cache_read_tokens += u("cache_read_input_tokens");
                state.cache_write_tokens += u("cache_creation_input_tokens");
            }
            if let Some(cost) = val.get("costUSD").and_then(|v| v.as_f64()) {
                state.total_cost_usd += cost;
            }
        }
    }
}

fn build_claude_session(
    state: &ClaudeSessionState,
    path: &Path,
    workspace_path: &str,
    project_name: &str,
) -> ClaudeSession {
    let sid = state.session_id.clone().unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into())
    });
    let total_tokens = state.input_tokens
        + state.output_tokens
        + state.cache_read_tokens
        + state.cache_write_tokens
        + state.external_total_tokens;
    let fallback_ts = fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339())
        .unwrap_or_default();
    let pending_elapsed = state
        .pending_user_ts
        .map(|user_dt| (Utc::now().fixed_offset() - user_dt).num_milliseconds().max(0))
        .unwrap_or(0);
    let active_elapsed_ms = state.completed_active_elapsed_ms + pending_elapsed;

    ClaudeSession {
        session_id: sid,
        project_path: workspace_path.to_string(),
        project_name: project_name.to_string(),
        transcript_path: path.display().to_string(),
        model: state.model.clone(),
        started_at: state
            .first_timestamp
            .clone()
            .unwrap_or_else(|| fallback_ts.clone()),
        last_activity_at: state.last_timestamp.clone().unwrap_or(fallback_ts),
        input_tokens: state.input_tokens,
        output_tokens: state.output_tokens,
        cache_read_tokens: state.cache_read_tokens,
        cache_write_tokens: state.cache_write_tokens,
        total_tokens,
        cost_usd: (state.total_cost_usd > 0.0).then_some(state.total_cost_usd),
        message_count: state.message_count,
        first_question: state.first_question.clone(),
        last_question: state.last_question.clone(),
        active_elapsed_ms,
        has_pending_tool_use: state.has_pending_tool_use,
    }
}

fn update_cached_claude_session(
    cached: &mut CachedClaudeSession,
    path: &Path,
    workspace_path: &str,
    project_name: &str,
) -> Option<ClaudeSession> {
    let delta = read_jsonl_delta(path, &mut cached.cursor).ok()?;
    if delta.reset {
        cached.state = ClaudeSessionState::default();
    }
    for line in delta.lines {
        apply_claude_line(&mut cached.state, &line);
    }
    Some(build_claude_session(&cached.state, path, workspace_path, project_name))
}

fn read_legacy_hud_usage_cache(config_dir: &Path) -> Option<ClaudeQuota> {
    let cache_path = config_dir.join("plugins/claude-hud/.usage-cache.json");
    let contents = fs::read_to_string(&cache_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let data = val.get("data")?;
    let str_field = |key: &str| data.get(key).and_then(|v| v.as_str()).map(String::from);
    let f64_field = |key: &str| data.get(key).and_then(|v| v.as_f64());

    Some(ClaudeQuota {
        plan_name: str_field("planName"),
        five_hour_used_pct: f64_field("fiveHour"),
        seven_day_used_pct: f64_field("sevenDay"),
        five_hour_reset_at: str_field("fiveHourResetAt"),
        seven_day_reset_at: str_field("sevenDayResetAt"),
    })
}

pub fn probe_with_cache(cache: &mut ClaudeProbeCache) -> ClaudeSnapshot {
    let config_dir = claude_config_dir();
    let projects_dir = config_dir.join("projects");

    let version_probe = run_command_probe("claude", &["--version"]);
    let cli_available = version_probe.success;
    let cli_version = version_probe
        .stdout_snippet
        .as_ref()
        .map(|s| s.trim().to_string());

    let config_probe = probe_file(&config_dir.join("settings.json"));
    let projects_probe = probe_file(&projects_dir);

    let active_session_hint = find_recent_session(&config_dir);
    let sessions = scan_sessions(&config_dir, Some(cache));

    // Older third-party HUD integrations wrote subscriber quota to this cache.
    // Current Claude Code docs recommend statusline `rate_limits` as the live source.
    let quota = read_legacy_hud_usage_cache(&config_dir);

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

pub fn probe() -> ClaudeSnapshot {
    let mut cache = ClaudeProbeCache::default();
    probe_with_cache(&mut cache)
}

fn find_recent_session(config_dir: &Path) -> Option<String> {
    let transcripts_dir = config_dir.join("transcripts");
    let name = latest_file_name(&transcripts_dir, Some("jsonl"))?;
    Some(format!("most recent transcript: {}", mask_secret(&name)))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

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

    #[test]
    fn cached_session_updates_only_with_new_jsonl_lines() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let transcript = temp_dir.path().join("session.jsonl");
        std::fs::write(
            &transcript,
            concat!(
                "{\"sessionId\":\"s1\",\"type\":\"user\",\"timestamp\":\"2026-04-01T00:00:00Z\",\"message\":{\"content\":\"hello\"}}\n",
                "{\"type\":\"assistant\",\"timestamp\":\"2026-04-01T00:00:05Z\",\"message\":{\"model\":\"claude-3.7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1}},\"costUSD\":0.5}\n"
            ),
        )
        .expect("initial transcript");

        let workspace_path = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_path).expect("workspace dir");
        let workspace = workspace_path.display().to_string();
        let mut cached = CachedClaudeSession::default();

        let first = update_cached_claude_session(&mut cached, &transcript, &workspace, "workspace")
            .expect("first parse");
        assert_eq!(first.message_count, 1);
        assert_eq!(first.total_tokens, 18);
        assert_eq!(first.active_elapsed_ms, 5_000);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&transcript)
            .expect("append transcript");
        writeln!(
            file,
            "{{\"type\":\"user\",\"timestamp\":\"2026-04-01T00:00:10Z\",\"message\":{{\"content\":\"follow up\"}}}}"
        )
        .expect("append user");
        writeln!(
            file,
            "{{\"type\":\"assistant\",\"timestamp\":\"2026-04-01T00:00:14Z\",\"message\":{{\"model\":\"claude-3.7\",\"usage\":{{\"input_tokens\":7,\"output_tokens\":3,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0}}}},\"costUSD\":0.25}}"
        )
        .expect("append assistant");

        let second =
            update_cached_claude_session(&mut cached, &transcript, &workspace, "workspace")
                .expect("second parse");
        assert_eq!(second.message_count, 2);
        assert_eq!(second.first_question.as_deref(), Some("hello"));
        assert_eq!(second.last_question.as_deref(), Some("follow up"));
        assert_eq!(second.total_tokens, 28);
        assert_eq!(second.cost_usd, Some(0.75));
        assert_eq!(second.active_elapsed_ms, 9_000);
    }

    #[test]
    fn cached_session_resets_after_truncate() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let transcript = temp_dir.path().join("session.jsonl");
        std::fs::write(
            &transcript,
            concat!(
                "{\"sessionId\":\"s1\",\"type\":\"user\",\"timestamp\":\"2026-04-01T00:00:00Z\",\"message\":{\"content\":\"hello\"}}\n",
                "{\"type\":\"assistant\",\"timestamp\":\"2026-04-01T00:00:05Z\",\"message\":{\"model\":\"claude-3.7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1}},\"costUSD\":0.5}\n"
            ),
        )
        .expect("initial transcript");

        let workspace = temp_dir.path().display().to_string();
        let mut cached = CachedClaudeSession::default();
        update_cached_claude_session(&mut cached, &transcript, &workspace, "workspace")
            .expect("first parse");

        std::fs::write(
            &transcript,
            "{\"sessionId\":\"s2\",\"type\":\"assistant\",\"timestamp\":\"2026-04-02T00:00:02Z\",\"message\":{\"model\":\"claude-3.7\",\"usage\":{\"input_tokens\":2,\"output_tokens\":1,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0}},\"costUSD\":0.1}\n",
        )
        .expect("truncate transcript");

        let reset = update_cached_claude_session(&mut cached, &transcript, &workspace, "workspace")
            .expect("reset parse");
        assert_eq!(reset.session_id, "s2");
        assert_eq!(reset.message_count, 0);
        assert_eq!(reset.total_tokens, 3);
        assert_eq!(reset.cost_usd, Some(0.1));
        assert!(reset.first_question.is_none());
    }

    #[test]
    fn assistant_usage_counts_once_per_message_id() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let transcript = temp_dir.path().join("session.jsonl");
        std::fs::write(
            &transcript,
            concat!(
                "{\"sessionId\":\"s1\",\"type\":\"user\",\"timestamp\":\"2026-04-01T00:00:00Z\",\"message\":{\"content\":\"hello\"}}\n",
                "{\"type\":\"assistant\",\"uuid\":\"u1\",\"requestId\":\"req1\",\"timestamp\":\"2026-04-01T00:00:05Z\",\"message\":{\"id\":\"msg1\",\"model\":\"claude-3.7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1},\"content\":[{\"type\":\"thinking\",\"thinking\":\"...\"}]},\"costUSD\":0.5}\n",
                "{\"type\":\"assistant\",\"uuid\":\"u2\",\"requestId\":\"req1\",\"timestamp\":\"2026-04-01T00:00:05Z\",\"message\":{\"id\":\"msg1\",\"model\":\"claude-3.7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1},\"content\":[{\"type\":\"text\",\"text\":\"done\"}]},\"costUSD\":0.5}\n"
            ),
        )
        .expect("transcript");

        let workspace = temp_dir.path().display().to_string();
        let session =
            parse_claude_session(&transcript, &workspace, "workspace").expect("parse session");

        assert_eq!(session.total_tokens, 18);
        assert_eq!(session.cost_usd, Some(0.5));
    }

    #[test]
    fn task_notification_usage_counts_once_per_task_id() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let transcript = temp_dir.path().join("session.jsonl");
        std::fs::write(
            &transcript,
            concat!(
                "{\"sessionId\":\"s1\",\"type\":\"assistant\",\"uuid\":\"u1\",\"requestId\":\"req1\",\"timestamp\":\"2026-04-01T00:00:05Z\",\"message\":{\"id\":\"msg1\",\"model\":\"claude-3.7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1},\"content\":[{\"type\":\"text\",\"text\":\"done\"}]},\"costUSD\":0.5}\n",
                "{\"type\":\"queue-operation\",\"operation\":\"enqueue\",\"timestamp\":\"2026-04-01T00:00:06Z\",\"sessionId\":\"s1\",\"content\":\"<task-notification><task-id>task-1</task-id><usage><total_tokens>42</total_tokens><tool_uses>1</tool_uses><duration_ms>1000</duration_ms></usage></task-notification>\"}\n",
                "{\"type\":\"user\",\"timestamp\":\"2026-04-01T00:00:07Z\",\"message\":{\"content\":\"<task-notification><task-id>task-1</task-id><usage><total_tokens>42</total_tokens><tool_uses>1</tool_uses><duration_ms>1000</duration_ms></usage></task-notification>\"}}\n"
            ),
        )
        .expect("transcript");

        let workspace = temp_dir.path().display().to_string();
        let session =
            parse_claude_session(&transcript, &workspace, "workspace").expect("parse session");

        assert_eq!(session.total_tokens, 60);
        assert_eq!(session.cost_usd, Some(0.5));
    }
}
