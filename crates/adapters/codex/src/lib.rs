use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

pub use octomonitor_adapter_common::{
    path_has_sensitive_component, probe_file, read_jsonl_delta, resolve_env_or_home,
    run_command_probe, truncate_chars, AdapterDescriptor, CommandProbeResult, FileProbeResult,
    JsonlCursor,
};

pub mod events;
pub use events::{
    dedupe_adjacent, parse_exec_output, parse_session_event_line, read_tail_events, CodexEvent,
    CodexEventKind, ExecOutput, TailReadResult, DEFAULT_MAX_EVENTS, DEFAULT_TAIL_BYTE_LIMIT,
};

pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor {
        tool: "codex",
        preferred_mode: "app-server+hook",
        fallback_mode: "local-state",
        confidence: "live",
    }
}

/// Adapter-level progress hint for a Codex session. Maps to `RunState` in the server.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexProgressKind {
    /// A turn is currently in flight (tool call, reasoning, or open task marker).
    Running,
    /// A user message is outstanding but no response has been observed yet.
    Waiting,
    /// The last observed terminal event was a successful assistant reply.
    Completed,
    /// The last observed terminal event was a turn/task abort.
    Aborted,
    /// Not enough signal to classify confidently.
    Unknown,
}

/// Extracted session data from a Codex JSONL session file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSession {
    pub session_id: String,
    pub thread_name: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub cli_version: Option<String>,
    pub transcript_path: String,
    pub started_at: String,
    pub last_activity_at: String,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    /// 5-hour window used percentage (from rate_limits.primary)
    pub five_hour_used_pct: Option<f64>,
    /// 7-day/weekly window used percentage (from rate_limits.secondary)
    pub seven_day_used_pct: Option<f64>,
    pub five_hour_resets_at: Option<i64>,
    pub seven_day_resets_at: Option<i64>,
    pub plan_type: Option<String>,
    pub first_question: Option<String>,
    pub last_question: Option<String>,
    pub message_count: u64,
    /// Sum of (user_message → next_response) intervals in ms, excluding idle gaps.
    pub active_elapsed_ms: i64,
    /// True when a `function_call` with `require_escalated` has no matching
    /// `function_call_output` — the session is waiting for user approval.
    pub has_pending_approval: bool,
    /// Adapter-level progress classification (maps to `RunState` in server).
    pub progress_kind: CodexProgressKind,
    /// Short, UI-safe progress reason (≤ 80 chars). No raw output or secrets.
    pub progress_reason: Option<String>,
    /// Most recent tool names, newest last, capped at 3.
    pub recent_tools: Vec<String>,
    /// True when at least one task_started has not yet been matched, or (in
    /// absence of task markers) a user_message is pending a response.
    pub turn_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSnapshot {
    pub probed_at: String,
    pub cli_available: bool,
    pub app_server_available: bool,
    pub cli_version: Option<String>,
    pub config_dir: Option<String>,
    pub config_exists: bool,
    pub history_exists: bool,
    pub recent_history_hint: Option<String>,
    pub sessions: Vec<CodexSession>,
    pub command_probes: Vec<CommandProbeResult>,
    pub file_probes: Vec<FileProbeResult>,
}

#[derive(Debug, Clone, Default)]
pub struct CodexProbeCache {
    session_files: HashMap<String, CachedCodexSession>,
    thread_index: CachedThreadIndex,
}

#[derive(Debug, Clone, Default)]
struct CachedThreadIndex {
    cursor: JsonlCursor,
    names: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
struct CachedCodexSession {
    cursor: JsonlCursor,
    state: CodexSessionState,
}

#[derive(Debug, Clone, Default)]
struct CodexSessionState {
    session_id: Option<String>,
    cwd: Option<String>,
    model: Option<String>,
    cli_version: Option<String>,
    started_at: Option<String>,
    last_timestamp: Option<String>,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    five_hour_used_pct: Option<f64>,
    seven_day_used_pct: Option<f64>,
    five_hour_resets_at: Option<i64>,
    seven_day_resets_at: Option<i64>,
    plan_type: Option<String>,
    first_question: Option<String>,
    last_question: Option<String>,
    message_count: u64,
    completed_active_elapsed_ms: i64,
    pending_user_ts: Option<chrono::DateTime<chrono::FixedOffset>>,
    /// call_ids of function_calls with `require_escalated` that have no matching output yet.
    pending_escalated_calls: HashSet<String>,

    /// Number of `task_started` markers without a matching terminator. `> 0` means
    /// a turn is in-flight.
    open_tasks: u32,
    /// Whether the last terminal event was an assistant reply (agent_message or
    /// task_complete). Used to distinguish "just completed" from "idle mid-turn".
    last_completed_ok: bool,
    /// Whether the last terminal event was a turn_aborted / task_aborted.
    last_aborted: bool,
    /// Short progress reason derived from the most recent salient event, ≤ 80 chars.
    progress_reason: Option<String>,
    /// Recent tool names (newest last), capped at 3 entries.
    recent_tools: Vec<String>,
    /// Session has seen at least one task_started or task_complete marker. When
    /// false we fall back to the user-message heuristic for turn_open.
    has_task_markers: bool,
}

fn codex_config_dir() -> PathBuf {
    resolve_env_or_home(&["CODEX_HOME"], ".codex")
}

fn apply_thread_index_line(names: &mut HashMap<String, String>, line: &str) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };
    if let (Some(id), Some(name)) = (
        val.get("id").and_then(|v| v.as_str()),
        val.get("thread_name").and_then(|v| v.as_str()),
    ) {
        names.insert(id.to_string(), truncate_chars(name, 80));
    }
}

fn load_thread_index(config_dir: &Path) -> HashMap<String, String> {
    let mut index = HashMap::new();
    let index_path = config_dir.join("session_index.jsonl");
    if let Ok(file) = fs::File::open(&index_path) {
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            apply_thread_index_line(&mut index, &line);
        }
    }
    index
}

fn load_thread_index_with_cache(
    config_dir: &Path,
    cache: Option<&mut CachedThreadIndex>,
) -> HashMap<String, String> {
    let index_path = config_dir.join("session_index.jsonl");
    let Some(cache) = cache else {
        return load_thread_index(config_dir);
    };
    if !index_path.exists() {
        cache.cursor = JsonlCursor::default();
        cache.names.clear();
        return HashMap::new();
    }

    let Ok(delta) = read_jsonl_delta(&index_path, &mut cache.cursor) else {
        return cache.names.clone();
    };
    if delta.reset {
        cache.names.clear();
    }
    for line in delta.lines {
        apply_thread_index_line(&mut cache.names, &line);
    }
    cache.names.clone()
}

fn scan_sessions(config_dir: &Path, mut cache: Option<&mut CodexProbeCache>) -> Vec<CodexSession> {
    let sessions_dir = config_dir.join("sessions");
    if !sessions_dir.is_dir() {
        return vec![];
    }

    let thread_index = load_thread_index_with_cache(
        config_dir,
        cache.as_deref_mut().map(|c| &mut c.thread_index),
    );
    let mut sessions = Vec::new();
    let mut seen_paths = HashSet::new();

    // Walk year/month/day directories
    scan_flat_sessions(
        &sessions_dir,
        &thread_index,
        &mut sessions,
        cache.as_deref_mut(),
        &mut seen_paths,
    );

    // Also scan archived_sessions
    let archived_dir = config_dir.join("archived_sessions");
    if archived_dir.is_dir() {
        scan_flat_sessions(
            &archived_dir,
            &thread_index,
            &mut sessions,
            cache.as_deref_mut(),
            &mut seen_paths,
        );
    }

    if let Some(cache) = cache {
        cache
            .session_files
            .retain(|path, _| seen_paths.contains(path));
    }

    // Sort by last_activity_at descending
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn scan_flat_sessions(
    dir: &Path,
    thread_index: &HashMap<String, String>,
    sessions: &mut Vec<CodexSession>,
    mut cache: Option<&mut CodexProbeCache>,
    seen_paths: &mut HashSet<String>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Recurse into subdirectories
                scan_flat_sessions(
                    &path,
                    thread_index,
                    sessions,
                    cache.as_deref_mut(),
                    seen_paths,
                );
                continue;
            }
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
                    update_cached_codex_session(entry, &path, thread_index)
                }
                None => parse_codex_session(&path, thread_index),
            };
            if let Some(session) = session {
                seen_paths.insert(cache_key);
                sessions.push(session);
            }
        }
    }
}

fn extract_text_content(payload: &serde_json::Value) -> Option<String> {
    let content = payload.get("content").or_else(|| payload.get("text"))?;
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = content.as_array() {
        return arr
            .iter()
            .find_map(|block| block.get("text").and_then(|t| t.as_str()).map(String::from));
    }
    None
}

fn parse_codex_session(
    path: &Path,
    thread_index: &HashMap<String, String>,
) -> Option<CodexSession> {
    let mut cached = CachedCodexSession::default();
    update_cached_codex_session(&mut cached, path, thread_index)
}

/// Helper for fields that are an `Option<String>` extracted via `as_str` on a
/// JSON value. Keeps the apply_* functions readable.
fn str_field(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Close any open user→assistant interval with the given line timestamp, then
/// mark the turn as cleanly completed. Used wherever an assistant reply lands
/// (turn_complete, assistant_message, agent_message, token_count after reply).
fn close_pending_turn_at(state: &mut CodexSessionState, timestamp: Option<&str>) {
    let line_dt = timestamp.and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok());
    if let (Some(user_dt), Some(asst_dt)) = (state.pending_user_ts.take(), line_dt) {
        state.completed_active_elapsed_ms += (asst_dt - user_dt).num_milliseconds().max(0);
    }
}

fn apply_codex_line(state: &mut CodexSessionState, line: &str) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };

    let timestamp = val.get("timestamp").and_then(|v| v.as_str());
    if let Some(ts) = timestamp {
        if state.started_at.is_none() {
            state.started_at = Some(ts.to_string());
        }
        state.last_timestamp = Some(ts.to_string());
    }

    let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let payload = val.get("payload");
    match msg_type {
        "session_meta" => {
            if let Some(payload) = payload {
                state.session_id = str_field(payload, "id");
                state.cwd = str_field(payload, "cwd");
                state.cli_version = str_field(payload, "cli_version");
            }
        }
        "turn_context" => {
            if let Some(payload) = payload {
                let model = payload
                    .get("model")
                    .or_else(|| payload.pointer("/info/model"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                if model.is_some() {
                    state.model = model;
                }
            }
        }
        "event_msg" => {
            if let Some(payload) = payload {
                apply_event_msg(state, timestamp, payload);
            }
        }
        "turn_complete" | "assistant_message" | "assistant_msg" => {
            close_pending_turn_at(state, timestamp);
            state.last_completed_ok = true;
            state.last_aborted = false;
        }
        "user_message" | "user_msg" => {
            handle_user_message(state, timestamp, payload);
        }
        "response_item" => {
            if let Some(payload) = payload {
                apply_response_item(state, payload);
            }
        }
        _ => {}
    }
}

fn apply_event_msg(
    state: &mut CodexSessionState,
    timestamp: Option<&str>,
    payload: &serde_json::Value,
) {
    let event_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match event_type {
        "token_count" => {
            close_pending_turn_at(state, timestamp);
            if let Some(usage) = payload.pointer("/info/total_token_usage") {
                let u64_at = |k: &str| usage.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
                state.input_tokens = u64_at("input_tokens");
                state.cached_input_tokens = u64_at("cached_input_tokens");
                state.output_tokens = u64_at("output_tokens");
                state.total_tokens = u64_at("total_tokens");
            }
            if let Some(rate_limits) = payload.get("rate_limits") {
                if let Some(primary) = rate_limits.get("primary") {
                    state.five_hour_used_pct = primary.get("used_percent").and_then(|v| v.as_f64());
                    state.five_hour_resets_at = primary.get("resets_at").and_then(|v| v.as_i64());
                }
                if let Some(secondary) = rate_limits.get("secondary") {
                    state.seven_day_used_pct =
                        secondary.get("used_percent").and_then(|v| v.as_f64());
                    state.seven_day_resets_at = secondary.get("resets_at").and_then(|v| v.as_i64());
                }
                state.plan_type = str_field(rate_limits, "plan_type");
            }
        }
        "user_message" => {
            handle_user_message(state, timestamp, Some(payload));
        }
        "agent_message" => {
            close_pending_turn_at(state, timestamp);
            state.last_completed_ok = true;
            state.last_aborted = false;
            // Safety: last_tail must be a short *status* line, not assistant
            // content. We intentionally do NOT echo message text here —
            // that would leak up to 80 chars of model output into the run
            // list. See docs/history/implementation-plan-monitoring-inspiration.md §8#4.
            state.progress_reason = Some("Assistant replied".to_string());
        }
        "task_started" => {
            state.has_task_markers = true;
            state.open_tasks = state.open_tasks.saturating_add(1);
            state.last_completed_ok = false;
            state.last_aborted = false;
            state.progress_reason = Some("Task started".to_string());
        }
        "task_complete" => {
            state.has_task_markers = true;
            state.open_tasks = state.open_tasks.saturating_sub(1);
            state.last_completed_ok = true;
            state.last_aborted = false;
            // Safety: never echo `last_agent_message` into last_tail — same
            // reasoning as agent_message above.
            state.progress_reason = Some("Task complete".to_string());
        }
        "task_aborted" | "turn_aborted" => {
            state.has_task_markers = true;
            state.open_tasks = state.open_tasks.saturating_sub(1);
            state.last_completed_ok = false;
            state.last_aborted = true;
            let reason = payload
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("aborted");
            state.progress_reason = Some(truncate_chars(&format!("Turn aborted: {reason}"), 80));
        }
        _ => {}
    }
}

fn apply_response_item(state: &mut CodexSessionState, payload: &serde_json::Value) {
    let item_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let call_id = payload.get("call_id").and_then(|v| v.as_str());
    match item_type {
        "function_call" | "custom_tool_call" => {
            let args = payload
                .get("arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if args.contains("require_escalated") {
                if let Some(id) = call_id {
                    state.pending_escalated_calls.insert(id.to_string());
                }
            }
            if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
                push_recent_tool(state, name);
                state.progress_reason = Some(truncate_chars(&format!("Running tool: {name}"), 80));
                state.last_completed_ok = false;
            }
        }
        "function_call_output" | "custom_tool_call_output" => {
            if let Some(id) = call_id {
                state.pending_escalated_calls.remove(id);
            }
        }
        _ => {}
    }
}

fn handle_user_message(
    state: &mut CodexSessionState,
    timestamp: Option<&str>,
    payload: Option<&serde_json::Value>,
) {
    if let Some(dt) = timestamp.and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok()) {
        if let Some(prev_user_dt) = state.pending_user_ts.take() {
            state.completed_active_elapsed_ms += (dt - prev_user_dt).num_milliseconds().max(0);
        }
        state.pending_user_ts = Some(dt);
    }
    state.message_count += 1;
    state.last_completed_ok = false;
    state.last_aborted = false;

    let Some(payload) = payload else { return };
    let text = payload
        .get("message")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| extract_text_content(payload));
    if let Some(t) = text {
        let truncated = truncate_chars(&t, 200);
        if state.first_question.is_none() {
            state.first_question = Some(truncated.clone());
        }
        state.last_question = Some(truncated);
    }
}

fn push_recent_tool(state: &mut CodexSessionState, name: &str) {
    if state.recent_tools.last().map(String::as_str) == Some(name) {
        return;
    }
    state.recent_tools.push(name.to_string());
    if state.recent_tools.len() > 3 {
        state.recent_tools.remove(0);
    }
}

fn build_codex_session(
    state: &CodexSessionState,
    path: &Path,
    thread_index: &HashMap<String, String>,
) -> Option<CodexSession> {
    let sid = state.session_id.clone()?;
    let thread_name = thread_index.get(&sid).cloned();
    let first_question = state.first_question.clone().or_else(|| thread_name.clone());
    let last_question = state
        .last_question
        .clone()
        .or_else(|| first_question.clone());
    let pending_ms = state
        .pending_user_ts
        .map(|user_dt| {
            (chrono::Utc::now().fixed_offset() - user_dt)
                .num_milliseconds()
                .max(0)
        })
        .unwrap_or(0);
    let active_elapsed_ms = state.completed_active_elapsed_ms + pending_ms;
    Some(CodexSession {
        session_id: sid,
        thread_name,
        cwd: state.cwd.clone(),
        model: state.model.clone(),
        cli_version: state.cli_version.clone(),
        transcript_path: path.display().to_string(),
        started_at: state.started_at.clone().unwrap_or_default(),
        last_activity_at: state.last_timestamp.clone().unwrap_or_default(),
        input_tokens: state.input_tokens,
        cached_input_tokens: state.cached_input_tokens,
        output_tokens: state.output_tokens,
        total_tokens: state.total_tokens,
        five_hour_used_pct: state.five_hour_used_pct,
        seven_day_used_pct: state.seven_day_used_pct,
        five_hour_resets_at: state.five_hour_resets_at,
        seven_day_resets_at: state.seven_day_resets_at,
        plan_type: state.plan_type.clone(),
        first_question,
        last_question,
        message_count: state.message_count,
        active_elapsed_ms,
        has_pending_approval: !state.pending_escalated_calls.is_empty(),
        progress_kind: infer_codex_progress_kind(state),
        progress_reason: state.progress_reason.clone(),
        recent_tools: state.recent_tools.clone(),
        turn_open: codex_turn_open(state),
    })
}

/// `true` when the session has an in-flight turn, preferring explicit task
/// markers over the user-message heuristic.
fn codex_turn_open(state: &CodexSessionState) -> bool {
    if state.has_task_markers {
        state.open_tasks > 0
    } else {
        state.pending_user_ts.is_some() && !state.last_completed_ok
    }
}

fn infer_codex_progress_kind(state: &CodexSessionState) -> CodexProgressKind {
    if state.last_aborted {
        return CodexProgressKind::Aborted;
    }
    if codex_turn_open(state) {
        return CodexProgressKind::Running;
    }
    if state.last_completed_ok {
        return CodexProgressKind::Completed;
    }
    if state.pending_user_ts.is_some() {
        return CodexProgressKind::Waiting;
    }
    CodexProgressKind::Unknown
}

fn update_cached_codex_session(
    cached: &mut CachedCodexSession,
    path: &Path,
    thread_index: &HashMap<String, String>,
) -> Option<CodexSession> {
    let delta = read_jsonl_delta(path, &mut cached.cursor).ok()?;
    if delta.reset {
        cached.state = CodexSessionState::default();
    }
    for line in delta.lines {
        apply_codex_line(&mut cached.state, &line);
    }
    build_codex_session(&cached.state, path, thread_index)
}

pub fn probe_with_cache(cache: &mut CodexProbeCache) -> CodexSnapshot {
    let config_dir = codex_config_dir();

    let version_probe = run_command_probe("codex", &["--version"]);
    let app_server_probe = run_command_probe("codex", &["app-server", "--help"]);
    let cli_available = version_probe.success;
    let app_server_available = app_server_probe.success;
    let cli_version = version_probe
        .stdout_snippet
        .as_ref()
        .map(|s| s.trim().to_string());

    let config_file = config_dir.join("config.toml");
    let config_json = config_dir.join("config.json");
    let history_file = config_dir.join("history.jsonl");
    // Check both config.toml (newer) and config.json (older)
    let config_probe = probe_file(&config_file);
    let config_json_probe = probe_file(&config_json);
    let history_probe = probe_file(&history_file);

    let config_exists = config_probe.exists || config_json_probe.exists;

    let recent_history_hint = history_probe
        .exists
        .then_some(history_probe.modified_at.as_ref())
        .flatten()
        .map(|t| format!("history last modified: {t}"));

    // Scan real session files
    let sessions = scan_sessions(&config_dir, Some(cache));

    CodexSnapshot {
        probed_at: Utc::now().to_rfc3339(),
        cli_available,
        app_server_available,
        cli_version,
        config_dir: Some(config_dir.display().to_string()),
        config_exists,
        history_exists: history_probe.exists,
        recent_history_hint,
        sessions,
        command_probes: vec![version_probe, app_server_probe],
        file_probes: vec![config_probe, config_json_probe, history_probe],
    }
}

pub fn probe() -> CodexSnapshot {
    let mut cache = CodexProbeCache::default();
    probe_with_cache(&mut cache)
}

/// Returns `true` when the candidate title is likely unhelpful/noisy for a run list.
/// Catches JSON blobs, markdown fences, system prompt markers, long structured single
/// lines, and values that are empty or only punctuation/whitespace.
pub fn looks_noisy_title(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }

    let first = trimmed.chars().next().unwrap_or(' ');
    if matches!(first, '{' | '[' | '<') || trimmed.starts_with("```") {
        return true;
    }

    let upper_head: String = trimmed.chars().take(16).collect::<String>().to_uppercase();
    const NOISY_PREFIXES: &[&str] = &[
        "SYSTEM:",
        "SYSTEM ",
        "[SYSTEM",
        "<SYSTEM",
        "INSTRUCTION:",
        "INSTRUCTIONS:",
        "ASSISTANT:",
        "USER:",
    ];
    if NOISY_PREFIXES.iter().any(|p| upper_head.starts_with(p)) {
        return true;
    }

    let first_line = trimmed.lines().next().unwrap_or("");
    if first_line.chars().count() > 240 && !first_line.contains(' ') {
        return true;
    }

    let has_any_word_char = trimmed
        .chars()
        .any(|c| c.is_alphanumeric() || (c as u32) > 0x7F);
    if !has_any_word_char {
        return true;
    }

    false
}

/// Pick a display-friendly title for a Codex session, falling back through candidates
/// whenever the preferred value is noisy.
///
/// Priority: `last_question` -> `first_question` -> `thread_name` -> session id prefix.
pub fn choose_codex_display_title(
    last_question: Option<&str>,
    first_question: Option<&str>,
    thread_name: Option<&str>,
    session_id: &str,
) -> String {
    for value in [last_question, first_question, thread_name]
        .into_iter()
        .flatten()
    {
        if !looks_noisy_title(value) {
            return value.to_string();
        }
    }

    let short_id: String = session_id.chars().take(8).collect();
    if short_id.is_empty() {
        "codex session".to_string()
    } else {
        short_id
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn descriptor_returns_codex() {
        let d = descriptor();
        assert_eq!(d.tool, "codex");
    }

    #[test]
    fn looks_noisy_flags_json_objects_and_arrays() {
        assert!(looks_noisy_title("{\"role\":\"user\",\"content\":\"hi\"}"));
        assert!(looks_noisy_title("[ {\"type\":\"text\"} ]"));
        assert!(looks_noisy_title("<tool_use>payload</tool_use>"));
    }

    #[test]
    fn looks_noisy_flags_markdown_fence() {
        assert!(looks_noisy_title("```json\n{\"a\":1}\n```"));
    }

    #[test]
    fn looks_noisy_flags_system_prefix() {
        assert!(looks_noisy_title("System: this is a hidden instruction"));
        assert!(looks_noisy_title("[SYSTEM] reminder"));
        assert!(looks_noisy_title("Instructions: do the thing"));
    }

    #[test]
    fn looks_noisy_flags_empty_and_symbol_only_values() {
        assert!(looks_noisy_title(""));
        assert!(looks_noisy_title("   "));
        assert!(looks_noisy_title("---/////"));
    }

    #[test]
    fn looks_noisy_flags_long_single_token_line() {
        let long = "a".repeat(260);
        assert!(looks_noisy_title(&long));
    }

    #[test]
    fn looks_noisy_accepts_normal_sentences() {
        assert!(!looks_noisy_title("Fix pagination off-by-one bug"));
        assert!(!looks_noisy_title("修复登录跳转问题"));
        assert!(!looks_noisy_title("Summarize the release notes"));
    }

    #[test]
    fn choose_display_title_prefers_last_question() {
        let out = choose_codex_display_title(
            Some("Real last question"),
            Some("First question"),
            Some("Thread one"),
            "abcdef1234567890",
        );
        assert_eq!(out, "Real last question");
    }

    #[test]
    fn choose_display_title_falls_back_past_noisy_values() {
        let out = choose_codex_display_title(
            Some("{\"payload\":\"noisy\"}"),
            Some("```json\n{\"a\":1}\n```"),
            Some("Clean thread name"),
            "abcdef1234567890",
        );
        assert_eq!(out, "Clean thread name");
    }

    #[test]
    fn choose_display_title_returns_short_session_id_when_all_noisy() {
        let out = choose_codex_display_title(
            Some("{\"a\":1}"),
            Some("[payload]"),
            Some("   "),
            "abcdef1234567890",
        );
        assert_eq!(out, "abcdef12");
    }

    fn apply_lines(state: &mut CodexSessionState, lines: &[&str]) {
        for line in lines {
            apply_codex_line(state, line);
        }
    }

    #[test]
    fn progress_kind_running_when_task_started_without_complete() {
        let mut state = CodexSessionState::default();
        apply_lines(
            &mut state,
            &[
                r#"{"type":"session_meta","timestamp":"2026-04-24T00:00:00Z","payload":{"id":"s1"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:01Z","payload":{"type":"user_message","message":"hi"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:02Z","payload":{"type":"task_started","turn_id":"t1"}}"#,
                r#"{"type":"response_item","timestamp":"2026-04-24T00:00:03Z","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"pwd\"}","call_id":"c1"}}"#,
            ],
        );
        assert_eq!(
            infer_codex_progress_kind(&state),
            CodexProgressKind::Running
        );
        assert!(codex_turn_open(&state));
        assert_eq!(
            state.progress_reason.as_deref(),
            Some("Running tool: exec_command")
        );
        assert_eq!(state.recent_tools, vec!["exec_command".to_string()]);
    }

    #[test]
    fn progress_kind_completed_after_task_complete() {
        let mut state = CodexSessionState::default();
        apply_lines(
            &mut state,
            &[
                r#"{"type":"session_meta","timestamp":"2026-04-24T00:00:00Z","payload":{"id":"s1"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:01Z","payload":{"type":"user_message","message":"hi"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:02Z","payload":{"type":"task_started","turn_id":"t1"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:03Z","payload":{"type":"task_complete","turn_id":"t1","last_agent_message":"Done"}}"#,
            ],
        );
        assert_eq!(
            infer_codex_progress_kind(&state),
            CodexProgressKind::Completed
        );
        assert!(!codex_turn_open(&state));
        // progress_reason must be a fixed status line — never the assistant's
        // message. Even the assistant's 80-char preview is off-limits.
        assert_eq!(state.progress_reason.as_deref(), Some("Task complete"));
    }

    #[test]
    fn progress_reason_does_not_leak_agent_message_content() {
        // Regression: earlier iterations echoed up to 80 chars of agent_message
        // / last_agent_message into last_tail, which violates the safety
        // boundary in §8 #4 of the plan.
        let mut state = CodexSessionState::default();
        apply_lines(
            &mut state,
            &[
                r#"{"type":"session_meta","timestamp":"2026-04-24T00:00:00Z","payload":{"id":"s1"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:01Z","payload":{"type":"user_message","message":"go"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:02Z","payload":{"type":"agent_message","message":"secret-api-key=abcdef sensitive-value"}}"#,
            ],
        );
        let reason = state.progress_reason.as_deref().unwrap_or("");
        assert!(
            !reason.contains("secret-api-key"),
            "progress_reason leaked agent content: {reason:?}"
        );
        assert_eq!(reason, "Assistant replied");
    }

    #[test]
    fn progress_kind_aborted_on_turn_aborted() {
        let mut state = CodexSessionState::default();
        apply_lines(
            &mut state,
            &[
                r#"{"type":"session_meta","timestamp":"2026-04-24T00:00:00Z","payload":{"id":"s1"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:01Z","payload":{"type":"user_message","message":"hi"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:02Z","payload":{"type":"task_started","turn_id":"t1"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:03Z","payload":{"type":"turn_aborted","reason":"interrupted"}}"#,
            ],
        );
        assert_eq!(
            infer_codex_progress_kind(&state),
            CodexProgressKind::Aborted
        );
        assert!(!codex_turn_open(&state));
        assert_eq!(
            state.progress_reason.as_deref(),
            Some("Turn aborted: interrupted")
        );
    }

    #[test]
    fn progress_kind_waiting_without_task_markers_after_user_message() {
        let mut state = CodexSessionState::default();
        apply_lines(
            &mut state,
            &[
                r#"{"type":"session_meta","timestamp":"2026-04-24T00:00:00Z","payload":{"id":"s1"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:01Z","payload":{"type":"user_message","message":"hi"}}"#,
            ],
        );
        assert!(codex_turn_open(&state));
        assert_eq!(
            infer_codex_progress_kind(&state),
            CodexProgressKind::Running
        );
    }

    #[test]
    fn event_msg_user_message_updates_first_and_last_question() {
        let mut state = CodexSessionState::default();
        apply_lines(
            &mut state,
            &[
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:00Z","payload":{"type":"user_message","message":"first question"}}"#,
                r#"{"type":"event_msg","timestamp":"2026-04-24T00:00:01Z","payload":{"type":"user_message","message":"second question"}}"#,
            ],
        );
        assert_eq!(state.message_count, 2);
        assert_eq!(state.first_question.as_deref(), Some("first question"));
        assert_eq!(state.last_question.as_deref(), Some("second question"));
    }

    #[test]
    fn push_recent_tool_is_capped_and_dedupes_consecutive() {
        let mut state = CodexSessionState::default();
        push_recent_tool(&mut state, "a");
        push_recent_tool(&mut state, "a");
        push_recent_tool(&mut state, "b");
        push_recent_tool(&mut state, "c");
        push_recent_tool(&mut state, "d");
        assert_eq!(state.recent_tools, vec!["b", "c", "d"]);
    }

    #[test]
    fn probe_runs_without_panic() {
        let snap = probe();
        assert!(!snap.probed_at.is_empty());
        assert!(!snap.command_probes.is_empty());
    }

    #[test]
    fn cached_thread_index_updates_only_new_lines() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let index_path = temp_dir.path().join("session_index.jsonl");
        std::fs::write(
            &index_path,
            "{\"id\":\"s1\",\"thread_name\":\"Thread One\"}\n",
        )
        .expect("initial index");

        let mut cache = CachedThreadIndex::default();
        let first = load_thread_index_with_cache(temp_dir.path(), Some(&mut cache));
        assert_eq!(first.get("s1").map(String::as_str), Some("Thread One"));

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&index_path)
            .expect("append index");
        writeln!(file, "{{\"id\":\"s2\",\"thread_name\":\"Thread Two\"}}").expect("append line");

        let second = load_thread_index_with_cache(temp_dir.path(), Some(&mut cache));
        assert_eq!(second.get("s1").map(String::as_str), Some("Thread One"));
        assert_eq!(second.get("s2").map(String::as_str), Some("Thread Two"));
    }

    #[test]
    fn cached_session_updates_only_with_new_jsonl_lines() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let transcript = temp_dir.path().join("session.jsonl");
        std::fs::write(
            &transcript,
            concat!(
                "{\"type\":\"session_meta\",\"timestamp\":\"2026-04-01T00:00:00Z\",\"payload\":{\"id\":\"s1\",\"cwd\":\"/tmp/project\",\"cli_version\":\"0.1.0\"}}\n",
                "{\"type\":\"user_msg\",\"timestamp\":\"2026-04-01T00:00:01Z\",\"payload\":{\"content\":\"hello\"}}\n",
                "{\"type\":\"event_msg\",\"timestamp\":\"2026-04-01T00:00:05Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":10,\"cached_input_tokens\":2,\"output_tokens\":5,\"total_tokens\":15}},\"rate_limits\":{\"primary\":{\"used_percent\":1.0,\"resets_at\":1},\"secondary\":{\"used_percent\":2.0,\"resets_at\":2},\"plan_type\":\"plus\"}}}\n"
            ),
        )
        .expect("initial transcript");

        let mut cached = CachedCodexSession::default();
        let mut thread_index = HashMap::new();
        thread_index.insert("s1".into(), "Thread One".into());

        let first =
            update_cached_codex_session(&mut cached, &transcript, &thread_index).expect("first");
        assert_eq!(first.message_count, 1);
        assert_eq!(first.total_tokens, 15);
        assert_eq!(first.first_question.as_deref(), Some("hello"));
        assert_eq!(first.active_elapsed_ms, 4_000);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&transcript)
            .expect("append transcript");
        writeln!(
            file,
            "{{\"type\":\"user_msg\",\"timestamp\":\"2026-04-01T00:00:10Z\",\"payload\":{{\"content\":\"follow up\"}}}}"
        )
        .expect("append user");
        writeln!(
            file,
            "{{\"type\":\"event_msg\",\"timestamp\":\"2026-04-01T00:00:14Z\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":12,\"cached_input_tokens\":3,\"output_tokens\":6,\"total_tokens\":18}}}},\"rate_limits\":{{\"primary\":{{\"used_percent\":3.0,\"resets_at\":3}},\"secondary\":{{\"used_percent\":4.0,\"resets_at\":4}},\"plan_type\":\"pro\"}}}}}}"
        )
        .expect("append assistant");

        let second =
            update_cached_codex_session(&mut cached, &transcript, &thread_index).expect("second");
        assert_eq!(second.message_count, 2);
        assert_eq!(second.total_tokens, 18);
        assert_eq!(second.last_question.as_deref(), Some("follow up"));
        assert_eq!(second.active_elapsed_ms, 8_000);
        assert_eq!(second.plan_type.as_deref(), Some("pro"));
    }
}
