use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use octomonitor_core::{RunRecord, ToolKind};
use serde::Serialize;

use crate::platform::{expand_home_path, home_relative_path};
use crate::state::AppState;

const MAX_ENTRY_CHARS: usize = 1200;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InspectEntryKind {
    Input,
    Output,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InspectEntry {
    pub kind: InspectEntryKind,
    pub timestamp: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunInspectPayload {
    pub entries: Vec<InspectEntry>,
}

pub async fn get_run_inspect(
    AxumPath(run_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<RunInspectPayload>, StatusCode> {
    let run = state
        .bootstrap
        .read()
        .await
        .runs
        .iter()
        .find(|item| item.id == run_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(RunInspectPayload {
        entries: load_run_entries(&run),
    }))
}

fn load_run_entries(run: &RunRecord) -> Vec<InspectEntry> {
    let Some(path) = resolve_transcript_path(run) else {
        return Vec::new();
    };

    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);

    match run.tool {
        ToolKind::Claude => parse_claude_entries(reader),
        ToolKind::Codex => parse_codex_entries(reader),
        ToolKind::OpenClaw => parse_openclaw_entries(reader),
        ToolKind::Hermes => Vec::new(), // Hermes uses SQLite, no JSONL transcripts
    }
}

fn resolve_transcript_path(run: &RunRecord) -> Option<PathBuf> {
    run.transcript_path
        .as_deref()
        .map(expand_home_path)
        .filter(|path| path.exists())
        .or_else(|| match run.tool {
            ToolKind::Codex => run
                .thread_id
                .as_deref()
                .and_then(find_codex_transcript_path),
            _ => None,
        })
}

fn find_codex_transcript_path(thread_id: &str) -> Option<PathBuf> {
    let config_dir = std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_relative_path(".codex"));

    find_file_containing_id(&config_dir.join("sessions"), thread_id)
        .or_else(|| find_file_containing_id(&config_dir.join("archived_sessions"), thread_id))
}

fn find_file_containing_id(root: &Path, needle: &str) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_containing_id(&path, needle) {
                return Some(found);
            }
            continue;
        }

        if path.extension().is_some_and(|ext| ext == "jsonl")
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(needle))
        {
            return Some(path);
        }
    }

    None
}

fn parse_codex_entries<R: BufRead>(reader: R) -> Vec<InspectEntry> {
    let mut entries = Vec::new();
    let mut pending_output: Option<InspectEntry> = None;

    for line in reader.lines().map_while(Result::ok) {
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let timestamp = val
            .get("timestamp")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let kind = val
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        match kind {
            "event_msg" => {
                let Some(payload) = val.get("payload") else {
                    continue;
                };
                match payload
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                {
                    "user_message" => {
                        flush_pending_output(&mut entries, &mut pending_output);
                        if let Some(text) = payload.get("message").and_then(|value| value.as_str())
                        {
                            push_entry(
                                &mut entries,
                                InspectEntryKind::Input,
                                &timestamp,
                                summarize_user_text(text),
                            );
                        }
                    }
                    "agent_message" => {
                        if let Some(text) = payload.get("message").and_then(|value| value.as_str())
                        {
                            pending_output = build_entry(
                                InspectEntryKind::Output,
                                &timestamp,
                                summarize_output_text(text),
                            );
                        }
                    }
                    _ => {}
                }
            }
            "response_item" => {
                let Some(payload) = val.get("payload") else {
                    continue;
                };
                let payload_type = payload
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let role = payload
                    .get("role")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                if payload_type == "message" && role == "assistant" {
                    if let Some(text) = payload
                        .get("content")
                        .and_then(|c| extract_string_or_text_blocks(c, &["output_text"]))
                    {
                        pending_output = build_entry(
                            InspectEntryKind::Output,
                            &timestamp,
                            summarize_output_text(&text),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    flush_pending_output(&mut entries, &mut pending_output);
    entries
}

fn parse_claude_entries<R: BufRead>(reader: R) -> Vec<InspectEntry> {
    let mut entries = Vec::new();
    let mut pending_output: Option<InspectEntry> = None;

    for line in reader.lines().map_while(Result::ok) {
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let timestamp = val
            .get("timestamp")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let record_type = val
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        match record_type {
            "user" => {
                let Some(message) = val.get("message") else {
                    continue;
                };
                let Some(text) = extract_claude_user_text(message) else {
                    continue;
                };
                let summary = summarize_user_text(&text);
                if summary.is_empty() {
                    continue;
                }
                flush_pending_output(&mut entries, &mut pending_output);
                push_entry(&mut entries, InspectEntryKind::Input, &timestamp, summary);
            }
            "assistant" => {
                let Some(message) = val.get("message") else {
                    continue;
                };
                let Some(text) = extract_content_text(message) else {
                    continue;
                };
                pending_output = build_entry(
                    InspectEntryKind::Output,
                    &timestamp,
                    summarize_output_text(&text),
                );
            }
            _ => {}
        }
    }

    flush_pending_output(&mut entries, &mut pending_output);
    entries
}

fn parse_openclaw_entries<R: BufRead>(reader: R) -> Vec<InspectEntry> {
    let mut entries = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if val.get("type").and_then(|value| value.as_str()) != Some("message") {
            continue;
        }

        let Some(message) = val.get("message") else {
            continue;
        };
        let role = message
            .get("role")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let timestamp = val
            .get("timestamp")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();

        match role {
            "user" => {
                let Some(text) = extract_content_text(message) else {
                    continue;
                };
                let Some(text) = sanitize_openclaw_input_text(&text) else {
                    continue;
                };
                push_entry(&mut entries, InspectEntryKind::Input, &timestamp, text);
            }
            "assistant" => {
                let text = extract_content_text(message).or_else(|| {
                    message
                        .get("errorMessage")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                });
                if let Some(text) = text {
                    push_entry(&mut entries, InspectEntryKind::Output, &timestamp, text);
                }
            }
            _ => {}
        }
    }

    dedupe_consecutive(entries)
}

fn extract_claude_user_text(message: &serde_json::Value) -> Option<String> {
    let content = message.get("content")?;
    let text = extract_string_or_text_blocks(content, &["text"])?;
    if text.starts_with("<local-command-caveat>")
        || text.starts_with("<system-reminder>")
        || text.starts_with("<command-name>")
        || text.starts_with("<task-notification>")
    {
        return None;
    }
    Some(text)
}

fn extract_content_text(message: &serde_json::Value) -> Option<String> {
    extract_string_or_text_blocks(message.get("content")?, &["text"])
}

fn extract_string_or_text_blocks(
    content: &serde_json::Value,
    allowed_types: &[&str],
) -> Option<String> {
    if let Some(text) = content.as_str() {
        let normalized = normalize_text(text);
        return (!normalized.is_empty()).then_some(normalized);
    }

    let blocks = content.as_array()?;
    let joined = blocks
        .iter()
        .filter_map(|block| {
            let block_type = block.get("type").and_then(|value| value.as_str())?;
            if !allowed_types.contains(&block_type) {
                return None;
            }
            block.get("text").and_then(|value| value.as_str())
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let normalized = normalize_text(&joined);
    (!normalized.is_empty()).then_some(normalized)
}

fn sanitize_openclaw_input_text(text: &str) -> Option<String> {
    let normalized = normalize_text(text);
    if normalized.starts_with("[cron:")
        || normalized.starts_with("[[")
        || normalized.starts_with("Read HEARTBEAT")
        || normalized.starts_with("<system")
        || normalized.starts_with("<command")
        || normalized.contains("HEARTBEAT.md")
        || normalized.contains("# OpenClaw Worker Spawn Contract")
        || normalized.starts_with("A new session was started via")
        || normalized.starts_with("Conversation info (untrusted")
    {
        return None;
    }

    if normalized.starts_with('[') && normalized.contains("GMT") {
        if let Some((_, remainder)) = normalized.split_once("] ") {
            let remainder = normalize_text(remainder);
            if !remainder.is_empty() {
                return Some(remainder);
            }
        }
    }

    Some(normalized)
}

fn summarize_user_text(text: &str) -> String {
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return String::new();
    }

    let lines = meaningful_lines(&normalized);

    for line in &lines {
        if let Some(task) = strip_labeled_value(line, "Task:") {
            return truncate_text(task);
        }
    }

    for line in &lines {
        if line.starts_with("- room-summary ") {
            if let Some((_, summary)) = line.rsplit_once(':') {
                let summary = normalize_text(summary);
                if !summary.is_empty() {
                    return truncate_text(summary);
                }
            }
        }
    }

    for line in &lines {
        if should_skip_summary_line(line, true) {
            continue;
        }
        return truncate_text(strip_markdown_noise(line));
    }

    truncate_text(normalized)
}

fn summarize_output_text(text: &str) -> String {
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return String::new();
    }

    let lines = meaningful_lines(&normalized);

    // Find the first non-skippable line index
    let start = lines
        .iter()
        .position(|line| !should_skip_summary_line(line, false));

    match start {
        Some(idx) => {
            // Keep all lines from the first meaningful one onward
            let kept: String = lines[idx..]
                .iter()
                .map(|l| strip_markdown_noise(l))
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            truncate_text(kept)
        }
        None => truncate_text(normalized),
    }
}

fn meaningful_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn should_skip_summary_line(line: &str, is_user: bool) -> bool {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();

    if trimmed.starts_with('#')
        || trimmed.starts_with('<')
        || trimmed.starts_with("```")
        || trimmed.starts_with('|')
        || trimmed == "---"
    {
        return true;
    }

    if is_user {
        lower.starts_with("runtime instruction document:")
            || lower.starts_with("seat role:")
            || lower.starts_with("seat label:")
            || lower.starts_with("acceptance criteria:")
            || lower.starts_with("work inside ")
            || lower.starts_with("return only ")
            || lower.starts_with("- path:")
            || lower.starts_with("- strategy:")
            || lower == "goal"
    } else {
        let stripped = strip_markdown_noise(trimmed);
        stripped.eq_ignore_ascii_case("summary")
            || stripped.eq_ignore_ascii_case("final answer")
            || stripped.eq_ignore_ascii_case("output")
    }
}

fn strip_labeled_value(line: &str, label: &str) -> Option<String> {
    line.strip_prefix(label)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn strip_markdown_noise(line: &str) -> String {
    line.trim()
        .trim_start_matches(['-', '*', '#', ' '])
        .replace("**", "")
        .replace("__", "")
        .trim()
        .to_string()
}

fn build_entry(kind: InspectEntryKind, timestamp: &str, text: String) -> Option<InspectEntry> {
    let text = normalize_text(&text);
    if text.is_empty() {
        return None;
    }
    Some(InspectEntry {
        kind,
        timestamp: timestamp.to_string(),
        text: truncate_text(text),
    })
}

fn push_entry(
    entries: &mut Vec<InspectEntry>,
    kind: InspectEntryKind,
    timestamp: &str,
    text: String,
) {
    if let Some(entry) = build_entry(kind, timestamp, text) {
        entries.push(entry);
    }
}

fn flush_pending_output(
    entries: &mut Vec<InspectEntry>,
    pending_output: &mut Option<InspectEntry>,
) {
    if let Some(entry) = pending_output.take() {
        entries.push(entry);
    }
}

fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

fn truncate_text(text: impl Into<String>) -> String {
    let text = text.into();
    if text.chars().count() <= MAX_ENTRY_CHARS {
        return text;
    }
    let truncated: String = text.chars().take(MAX_ENTRY_CHARS).collect();
    format!("{truncated}…")
}

fn dedupe_consecutive(entries: Vec<InspectEntry>) -> Vec<InspectEntry> {
    let mut deduped = Vec::with_capacity(entries.len());
    for entry in entries {
        let is_duplicate = deduped.last().is_some_and(|previous: &InspectEntry| {
            previous.kind == entry.kind && previous.text == entry.text
        });
        if !is_duplicate {
            deduped.push(entry);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        parse_claude_entries, parse_codex_entries, parse_openclaw_entries, InspectEntryKind,
    };

    #[test]
    fn codex_entries_keep_turn_input_and_final_output() {
        let data = r#"{"timestamp":"2026-04-01T09:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"Please summarize the latest release notes."}}
{"timestamp":"2026-04-01T09:00:01Z","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"Thinking"}]}}
{"timestamp":"2026-04-01T09:00:02Z","type":"response_item","payload":{"type":"function_call","name":"shell_command","arguments":"echo hi"}}
{"timestamp":"2026-04-01T09:00:03Z","type":"event_msg","payload":{"type":"agent_message","message":"Release notes summary: fixed the reconnect bug and reduced polling."}}
"#;

        let entries = parse_codex_entries(Cursor::new(data));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, InspectEntryKind::Input);
        assert_eq!(
            entries[0].text,
            "Please summarize the latest release notes."
        );
        assert_eq!(entries[1].kind, InspectEntryKind::Output);
        assert_eq!(
            entries[1].text,
            "Release notes summary: fixed the reconnect bug and reduced polling."
        );
    }

    #[test]
    fn claude_entries_keep_task_summary_and_last_assistant_text() {
        let data = r####"{"type":"user","timestamp":"2026-04-01T09:00:00Z","message":{"role":"user","content":"Runtime instruction document\n\nTask: Fix pagination off-by-one bug\n\n## Workspace\n- path: /tmp/demo"}}
{"type":"assistant","timestamp":"2026-04-01T09:00:01Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Inspecting files"}]}}
{"type":"assistant","timestamp":"2026-04-01T09:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"Initial analysis: it is likely a ceil/floor mismatch."}]}}
{"type":"user","timestamp":"2026-04-01T09:00:03Z","message":{"role":"user","content":[{"type":"tool_result","content":"file contents"}]}}
{"type":"assistant","timestamp":"2026-04-01T09:00:04Z","message":{"role":"assistant","content":[{"type":"text","text":"### Summary\n\n**Root cause:** the code always adds one extra page."}]}}
"####;

        let entries = parse_claude_entries(Cursor::new(data));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, InspectEntryKind::Input);
        assert_eq!(entries[0].text, "Fix pagination off-by-one bug");
        assert_eq!(entries[1].kind, InspectEntryKind::Output);
        assert_eq!(
            entries[1].text,
            "Root cause: the code always adds one extra page."
        );
    }

    #[test]
    fn openclaw_entries_keep_text_messages_and_skip_system_noise() {
        let data = r#"{"type":"message","timestamp":"2026-04-01T09:00:00Z","message":{"role":"user","content":[{"type":"text","text":"[Fri 2026-04-01 17:00 GMT+8] Reply with exactly OK"}]}}
{"type":"message","timestamp":"2026-04-01T09:00:01Z","message":{"role":"user","content":[{"type":"text","text":"Summarize the failed deploy and propose next steps."}]}}
{"type":"message","timestamp":"2026-04-01T09:00:02Z","message":{"role":"assistant","content":[],"errorMessage":"529 overloaded"}}
{"type":"message","timestamp":"2026-04-01T09:00:03Z","message":{"role":"assistant","content":[],"errorMessage":"529 overloaded"}}
{"type":"message","timestamp":"2026-04-01T09:00:04Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"private"},{"type":"text","text":"Deploy failed after health checks. Retry after fixing the missing env var."}]}}
"#;

        let entries = parse_openclaw_entries(Cursor::new(data));
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].kind, InspectEntryKind::Input);
        assert_eq!(entries[0].text, "Reply with exactly OK");
        assert_eq!(entries[1].kind, InspectEntryKind::Input);
        assert_eq!(
            entries[1].text,
            "Summarize the failed deploy and propose next steps."
        );
        assert_eq!(entries[2].kind, InspectEntryKind::Output);
        assert_eq!(entries[2].text, "529 overloaded");
        assert_eq!(entries[3].kind, InspectEntryKind::Output);
        assert_eq!(
            entries[3].text,
            "Deploy failed after health checks. Retry after fixing the missing env var."
        );
    }
}
