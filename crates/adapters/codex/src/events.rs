//! Codex JSONL event parser and tail reader.
//!
//! The functions here are **pure** and do not touch OctoMonitor state. They take raw
//! JSONL lines or file paths and produce structured [`CodexEvent`] values suitable for
//! timeline rendering and progress inference. See
//! `docs/implementation-plan-monitoring-inspiration.md` §3 for the event taxonomy and
//! the sampling notes that back these field choices.

use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const DEFAULT_TAIL_BYTE_LIMIT: usize = 256 * 1024;
pub const DEFAULT_MAX_EVENTS: usize = 600;

const PREVIEW_MAX_CHARS: usize = 400;
const TEXT_MAX_CHARS: usize = 2000;
const COMMAND_MAX_CHARS: usize = 400;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexEventKind {
    UserMessage,
    AssistantMessage,
    Reasoning,
    ToolCall,
    ToolOutput,
    WebSearch,
    TokenCount,
    TaskStarted,
    TaskComplete,
    TaskAborted,
    TurnAborted,
    Context,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexEvent {
    pub kind: CodexEventKind,
    pub timestamp: String,
    pub turn_id: Option<String>,
    pub title: String,
    pub preview: String,
    pub text: Option<String>,
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub call_id: Option<String>,
}

/// Build a `CodexEvent` with all optional fields defaulted to `None`. Branches
/// override only the fields they care about — keeps `parse_*` arms readable.
fn mk_event(kind: CodexEventKind, timestamp: &str, title: &str, preview: String) -> CodexEvent {
    CodexEvent {
        kind,
        timestamp: timestamp.to_string(),
        turn_id: None,
        title: title.to_string(),
        preview,
        text: None,
        tool_name: None,
        command: None,
        exit_code: None,
        call_id: None,
    }
}

fn turn_id_of(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("turn_id")
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecOutput {
    pub exit_code: Option<i32>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailReadResult {
    pub events: Vec<CodexEvent>,
    pub cursor: u64,
    pub reset: bool,
}

/// Parse a single JSONL line into zero or more [`CodexEvent`]s.
/// Returns an empty vec for lines that are unparseable JSON or irrelevant
/// envelope types.
pub fn parse_session_event_line(line: &str) -> Vec<CodexEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return Vec::new();
    };

    let timestamp = val
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let outer_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let Some(payload) = val.get("payload") else {
        return Vec::new();
    };

    match outer_type {
        "event_msg" => parse_event_msg(&timestamp, payload),
        "response_item" => parse_response_item(&timestamp, payload),
        "turn_context" => vec![build_context_event(&timestamp, payload)],
        _ => Vec::new(),
    }
}

fn parse_event_msg(timestamp: &str, payload: &serde_json::Value) -> Vec<CodexEvent> {
    let subtype = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match subtype {
        "user_message" | "agent_message" => {
            let message = payload
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let (kind, title) = if subtype == "user_message" {
                (CodexEventKind::UserMessage, "User message")
            } else {
                (CodexEventKind::AssistantMessage, "Assistant message")
            };
            let mut event = mk_event(kind, timestamp, title, summarize_preview(message));
            event.text = Some(truncate_text(message));
            vec![event]
        }
        "token_count" => {
            let total = payload
                .pointer("/info/total_token_usage/total_tokens")
                .and_then(|v| v.as_u64());
            let preview = match total {
                Some(t) => format!("{t} tokens"),
                None => "token count".to_string(),
            };
            vec![mk_event(
                CodexEventKind::TokenCount,
                timestamp,
                "Token count",
                preview,
            )]
        }
        "task_started" => {
            let mut event = mk_event(
                CodexEventKind::TaskStarted,
                timestamp,
                "Task started",
                "Task started".to_string(),
            );
            event.turn_id = turn_id_of(payload);
            vec![event]
        }
        "task_complete" => {
            let preview = payload
                .get("last_agent_message")
                .and_then(|v| v.as_str())
                .map(summarize_preview)
                .unwrap_or_else(|| "Task complete".to_string());
            let mut event =
                mk_event(CodexEventKind::TaskComplete, timestamp, "Task complete", preview);
            event.turn_id = turn_id_of(payload);
            vec![event]
        }
        "task_aborted" | "turn_aborted" => {
            let reason = payload
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("aborted");
            let (kind, title, preview_prefix) = if subtype == "task_aborted" {
                (CodexEventKind::TaskAborted, "Task aborted", "Task aborted")
            } else {
                (CodexEventKind::TurnAborted, "Turn aborted", "Turn aborted")
            };
            let mut event = mk_event(kind, timestamp, title, format!("{preview_prefix}: {reason}"));
            event.turn_id = turn_id_of(payload);
            vec![event]
        }
        _ => Vec::new(),
    }
}

fn parse_response_item(timestamp: &str, payload: &serde_json::Value) -> Vec<CodexEvent> {
    let subtype = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match subtype {
        "message" => {
            let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                return Vec::new();
            }
            let text = extract_message_text(payload.get("content"));
            let mut event = mk_event(
                CodexEventKind::AssistantMessage,
                timestamp,
                "Assistant message",
                summarize_preview(&text),
            );
            event.text = Some(truncate_text(&text));
            vec![event]
        }
        "reasoning" => {
            let summary_text = payload
                .get("summary")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let (preview, text) = if summary_text.is_empty() {
                ("Reasoning (encrypted)".to_string(), None)
            } else {
                (summarize_preview(&summary_text), Some(truncate_text(&summary_text)))
            };
            let mut event = mk_event(CodexEventKind::Reasoning, timestamp, "Reasoning", preview);
            event.text = text;
            vec![event]
        }
        "function_call" | "custom_tool_call" => {
            let tool_name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from);
            let args_raw = payload
                .get("arguments")
                .or_else(|| payload.get("input"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let command = extract_command_from_arguments(args_raw);
            let preview = match (&tool_name, &command) {
                (Some(name), Some(cmd)) => format!("{name}: {}", truncate_chars_display(cmd, 120)),
                (Some(name), None) if args_raw.is_empty() => name.clone(),
                (Some(name), None) => format!("{name}: {}", summarize_preview(args_raw)),
                (None, Some(cmd)) => truncate_chars_display(cmd, 120),
                (None, None) => "tool call".to_string(),
            };
            let title = tool_name.clone().unwrap_or_else(|| "Tool call".to_string());
            let mut event = mk_event(CodexEventKind::ToolCall, timestamp, &title, preview);
            event.tool_name = tool_name;
            event.command = command;
            event.call_id = payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            vec![event]
        }
        "function_call_output" | "custom_tool_call_output" => {
            let raw = payload.get("output").and_then(|v| v.as_str()).unwrap_or("");
            let parsed = parse_exec_output(raw);
            let preview = parsed
                .body
                .as_deref()
                .map(summarize_preview)
                .unwrap_or_else(|| "tool output".to_string());
            let mut event =
                mk_event(CodexEventKind::ToolOutput, timestamp, "Tool output", preview);
            event.text = parsed.body.as_deref().map(truncate_text);
            event.exit_code = parsed.exit_code;
            event.call_id = payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            vec![event]
        }
        "web_search_call" => {
            let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let preview = if status.is_empty() {
                "Web search".to_string()
            } else {
                format!("Web search: {status}")
            };
            let mut event = mk_event(CodexEventKind::WebSearch, timestamp, "Web search", preview);
            event.tool_name = Some("web_search".to_string());
            vec![event]
        }
        _ => Vec::new(),
    }
}

fn build_context_event(timestamp: &str, payload: &serde_json::Value) -> CodexEvent {
    let model = payload
        .get("model")
        .or_else(|| payload.pointer("/info/model"))
        .and_then(|v| v.as_str());
    let preview = match model {
        Some(m) => format!("Model: {m}"),
        None => "Turn context".to_string(),
    };
    mk_event(CodexEventKind::Context, timestamp, "Turn context", preview)
}

/// Parse an `output` string from a `function_call_output` (text format) or
/// `custom_tool_call_output` (JSON format) into a structured [`ExecOutput`].
///
/// Never panics — if neither format matches, returns `raw` as the body with
/// `exit_code = None`.
pub fn parse_exec_output(raw: &str) -> ExecOutput {
    if raw.trim().is_empty() {
        return ExecOutput::default();
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        if value.is_object() {
            let body = value
                .get("output")
                .and_then(|v| v.as_str())
                .map(|s| strip_ansi(s.trim()));
            let exit_code = value
                .pointer("/metadata/exit_code")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            if body.is_some() || exit_code.is_some() {
                return ExecOutput { exit_code, body };
            }
        }
    }

    let mut exit_code: Option<i32> = None;
    let mut body_start: Option<usize> = None;

    for (idx, line) in raw.split_inclusive('\n').enumerate() {
        let stripped = line.trim_end_matches('\n').trim_end_matches('\r');
        if exit_code.is_none() {
            if let Some(rest) = stripped.strip_prefix("Process exited with code ") {
                if let Ok(code) = rest.trim().parse::<i32>() {
                    exit_code = Some(code);
                }
            }
        }
        if stripped.trim_end() == "Output:" {
            body_start = Some(idx + 1);
        }
    }

    let body = if let Some(start_line) = body_start {
        let body_text: String = raw
            .split_inclusive('\n')
            .skip(start_line)
            .collect::<String>();
        let trimmed = strip_ansi(body_text.trim());
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    } else if exit_code.is_some() {
        None
    } else {
        Some(strip_ansi(raw.trim()))
    };

    ExecOutput { exit_code, body }
}

/// Remove immediately adjacent duplicate events. Two events are considered
/// duplicates when their structural comparison is equal (same kind, same text
/// fields, same tool/call metadata).
pub fn dedupe_adjacent(events: Vec<CodexEvent>) -> Vec<CodexEvent> {
    let mut out: Vec<CodexEvent> = Vec::with_capacity(events.len());
    for event in events {
        if out.last().map(|prev| prev == &event).unwrap_or(false) {
            continue;
        }
        out.push(event);
    }
    out
}

/// Read the tail of a Codex JSONL session file and produce structured events.
///
/// - When `cursor` is `None`, reads the last `byte_limit` bytes and returns the
///   newest `max_events` events; `cursor` in the result points to end-of-file.
/// - When `cursor` is `Some(offset)` and `offset <= file_len`, reads from that
///   offset forward; returns the new events and the new end-of-file `cursor`.
/// - When `cursor > file_len` (file was truncated), returns `reset: true` and
///   falls back to a tail read.
/// - Half-written final lines (missing trailing `\n`) are ignored.
pub fn read_tail_events(
    path: &Path,
    cursor: Option<u64>,
    byte_limit: usize,
    max_events: usize,
) -> std::io::Result<TailReadResult> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();

    let (start_offset, reset) = match cursor {
        Some(c) if c <= file_len => (c, false),
        Some(_) => {
            let start = file_len.saturating_sub(byte_limit as u64);
            (start, true)
        }
        None => {
            let start = file_len.saturating_sub(byte_limit as u64);
            (start, false)
        }
    };

    file.seek(SeekFrom::Start(start_offset))?;
    let remaining = file_len.saturating_sub(start_offset);
    let mut buf = Vec::with_capacity(remaining as usize);
    file.take(remaining).read_to_end(&mut buf)?;

    let mut content = match String::from_utf8(buf) {
        Ok(s) => s,
        Err(err) => String::from_utf8_lossy(err.as_bytes()).to_string(),
    };

    if start_offset > 0 {
        if let Some(idx) = content.find('\n') {
            content = content[idx + 1..].to_string();
        } else {
            content.clear();
        }
    }

    let had_trailing_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();
    // Drop a half-written final line (no trailing `\n`) — we'll see it next read.
    let usable = if had_trailing_newline {
        lines.as_slice()
    } else {
        lines.split_last().map(|(_, rest)| rest).unwrap_or(&[])
    };
    let mut events: Vec<CodexEvent> = Vec::new();
    for line in usable {
        events.extend(parse_session_event_line(line));
    }

    if events.len() > max_events {
        let drop = events.len() - max_events;
        events.drain(..drop);
    }

    Ok(TailReadResult {
        events,
        cursor: file_len,
        reset,
    })
}

fn summarize_preview(value: &str) -> String {
    let cleaned = strip_ansi(value)
        .replace('\r', " ")
        .replace('\n', " ⏎ ");
    truncate_chars_display(&cleaned, PREVIEW_MAX_CHARS)
}

fn truncate_text(value: &str) -> String {
    let cleaned = strip_ansi(value);
    let total_chars = cleaned.chars().count();
    if total_chars <= TEXT_MAX_CHARS {
        return cleaned;
    }
    let prefix_chars = 1000;
    let suffix_chars = TEXT_MAX_CHARS - prefix_chars - 5;
    let prefix: String = cleaned.chars().take(prefix_chars).collect();
    let suffix: String = cleaned
        .chars()
        .skip(total_chars - suffix_chars)
        .collect();
    format!("{prefix}\n…\n{suffix}")
}

fn truncate_chars_display(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
}

fn strip_ansi(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() && !(bytes[i] as char).is_alphabetic() {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        let ch = bytes[i] as char;
        if ch.is_ascii_control() && ch != '\n' && ch != '\r' && ch != '\t' {
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn extract_command_from_arguments(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let obj = value.as_object()?;
    let cmd_val = obj
        .get("cmd")
        .or_else(|| obj.get("command"))
        .or_else(|| obj.get("shell"))?;
    let command = if let Some(s) = cmd_val.as_str() {
        s.to_string()
    } else if let Some(parts) = cmd_val.as_array() {
        parts
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        return None;
    };
    let cleaned = strip_ansi(command.trim());
    if cleaned.is_empty() {
        return None;
    }
    Some(truncate_chars_display(&cleaned, COMMAND_MAX_CHARS))
}

fn extract_message_text(content: Option<&serde_json::Value>) -> String {
    let Some(array) = content.and_then(|v| v.as_array()) else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    for item in array {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if matches!(item_type, "output_text" | "input_text" | "text") {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                parts.push(text.to_string());
            }
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_envelope(outer: &str, payload: serde_json::Value) -> String {
        let doc = serde_json::json!({
            "timestamp": "2026-04-24T08:00:00.000Z",
            "type": outer,
            "payload": payload,
        });
        serde_json::to_string(&doc).expect("json")
    }

    #[test]
    fn parse_user_message_line_yields_user_event() {
        let line = sample_envelope(
            "event_msg",
            serde_json::json!({
                "type": "user_message",
                "message": "Fix the login redirect bug",
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CodexEventKind::UserMessage);
        assert_eq!(events[0].preview, "Fix the login redirect bug");
        assert_eq!(
            events[0].text.as_deref(),
            Some("Fix the login redirect bug")
        );
    }

    #[test]
    fn parse_assistant_message_line_handles_event_msg_and_response_item() {
        let event_msg = sample_envelope(
            "event_msg",
            serde_json::json!({
                "type": "agent_message",
                "message": "Done — redirect is now using 302.",
            }),
        );
        let response_item = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "All set."}],
            }),
        );
        let a = parse_session_event_line(&event_msg);
        let b = parse_session_event_line(&response_item);
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);
        assert_eq!(a[0].kind, CodexEventKind::AssistantMessage);
        assert_eq!(b[0].kind, CodexEventKind::AssistantMessage);
        assert_eq!(b[0].preview, "All set.");
    }

    #[test]
    fn parse_response_item_message_skips_non_assistant_roles() {
        let line = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "ignore me"}],
            }),
        );
        assert!(parse_session_event_line(&line).is_empty());
    }

    #[test]
    fn parse_reasoning_line_with_summary_text() {
        let line = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "Checking login flow"}],
                "content": null,
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CodexEventKind::Reasoning);
        assert_eq!(events[0].preview, "Checking login flow");
    }

    #[test]
    fn parse_reasoning_line_with_empty_summary_marks_encrypted() {
        let line = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "reasoning",
                "summary": [],
                "content": null,
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].preview, "Reasoning (encrypted)");
        assert!(events[0].text.is_none());
    }

    #[test]
    fn parse_function_call_produces_tool_call_with_extracted_command() {
        let line = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "function_call",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"pwd\",\"yield_time_ms\":1000}",
                "call_id": "call_abc",
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CodexEventKind::ToolCall);
        assert_eq!(events[0].tool_name.as_deref(), Some("exec_command"));
        assert_eq!(events[0].command.as_deref(), Some("pwd"));
        assert_eq!(events[0].call_id.as_deref(), Some("call_abc"));
    }

    #[test]
    fn parse_function_call_output_text_format_extracts_exit_and_body() {
        let out_str = "Chunk ID: b6af7f\nWall time: 0.0000 seconds\nProcess exited with code 0\nOriginal token count: 12\nOutput:\n/Users/demo\n";
        let line = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "function_call_output",
                "call_id": "call_abc",
                "output": out_str,
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CodexEventKind::ToolOutput);
        assert_eq!(events[0].exit_code, Some(0));
        assert_eq!(events[0].text.as_deref(), Some("/Users/demo"));
        assert_eq!(events[0].call_id.as_deref(), Some("call_abc"));
    }

    #[test]
    fn parse_custom_tool_call_output_json_format_extracts_exit_and_body() {
        let out_str =
            "{\"output\":\"Success. Updated files:\\nM main.go\",\"metadata\":{\"exit_code\":0,\"duration_seconds\":0.0}}";
        let line = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "custom_tool_call_output",
                "call_id": "call_def",
                "output": out_str,
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CodexEventKind::ToolOutput);
        assert_eq!(events[0].exit_code, Some(0));
        assert!(events[0]
            .text
            .as_deref()
            .unwrap_or("")
            .contains("Success. Updated files"));
    }

    #[test]
    fn parse_exec_output_handles_raw_fallback() {
        let ex = parse_exec_output("just some raw output");
        assert_eq!(ex.exit_code, None);
        assert_eq!(ex.body.as_deref(), Some("just some raw output"));
    }

    #[test]
    fn parse_web_search_event() {
        let line = sample_envelope(
            "response_item",
            serde_json::json!({
                "type": "web_search_call",
                "status": "completed",
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CodexEventKind::WebSearch);
        assert_eq!(events[0].preview, "Web search: completed");
    }

    #[test]
    fn parse_task_markers() {
        let started = sample_envelope(
            "event_msg",
            serde_json::json!({
                "type": "task_started",
                "turn_id": "turn-1",
                "model_context_window": 100,
                "collaboration_mode_kind": "default",
            }),
        );
        let complete = sample_envelope(
            "event_msg",
            serde_json::json!({
                "type": "task_complete",
                "turn_id": "turn-1",
                "last_agent_message": "Here is the summary.",
            }),
        );
        let turn_aborted = sample_envelope(
            "event_msg",
            serde_json::json!({
                "type": "turn_aborted",
                "reason": "interrupted",
            }),
        );

        let s = parse_session_event_line(&started);
        let c = parse_session_event_line(&complete);
        let t = parse_session_event_line(&turn_aborted);
        assert_eq!(s[0].kind, CodexEventKind::TaskStarted);
        assert_eq!(s[0].turn_id.as_deref(), Some("turn-1"));
        assert_eq!(c[0].kind, CodexEventKind::TaskComplete);
        assert_eq!(c[0].preview, "Here is the summary.");
        assert_eq!(t[0].kind, CodexEventKind::TurnAborted);
        assert_eq!(t[0].preview, "Turn aborted: interrupted");
    }

    #[test]
    fn parse_token_count_reports_summary_only() {
        let line = sample_envelope(
            "event_msg",
            serde_json::json!({
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": 100,
                        "output_tokens": 50,
                        "total_tokens": 150,
                    }
                },
                "rate_limits": {"primary": {"used_percent": 42.0}},
            }),
        );
        let events = parse_session_event_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CodexEventKind::TokenCount);
        assert_eq!(events[0].preview, "150 tokens");
        assert!(events[0].text.is_none());
    }

    #[test]
    fn parse_invalid_json_returns_empty() {
        assert!(parse_session_event_line("{not json").is_empty());
        assert!(parse_session_event_line("").is_empty());
        assert!(parse_session_event_line("   ").is_empty());
    }

    #[test]
    fn dedupe_adjacent_collapses_identical_pairs() {
        let base = CodexEvent {
            kind: CodexEventKind::ToolOutput,
            timestamp: "t".into(),
            turn_id: None,
            title: "x".into(),
            preview: "same".into(),
            text: Some("same".into()),
            tool_name: None,
            command: None,
            exit_code: Some(0),
            call_id: Some("c1".into()),
        };
        let mut other = base.clone();
        other.preview = "different".into();
        let events = vec![base.clone(), base.clone(), other.clone(), base.clone()];
        let deduped = dedupe_adjacent(events);
        assert_eq!(deduped.len(), 3);
    }

    #[test]
    fn read_tail_events_respects_byte_limit_and_returns_cursor() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.jsonl");
        let mut file = File::create(&path).unwrap();
        for i in 0..5 {
            let line = sample_envelope(
                "event_msg",
                serde_json::json!({
                    "type": "user_message",
                    "message": format!("msg {i}"),
                }),
            );
            writeln!(file, "{line}").unwrap();
        }
        file.sync_all().unwrap();
        drop(file);

        let result =
            read_tail_events(&path, None, DEFAULT_TAIL_BYTE_LIMIT, DEFAULT_MAX_EVENTS).unwrap();
        assert_eq!(result.events.len(), 5);
        assert!(!result.reset);
        let final_cursor = result.cursor;

        let result2 = read_tail_events(
            &path,
            Some(final_cursor),
            DEFAULT_TAIL_BYTE_LIMIT,
            DEFAULT_MAX_EVENTS,
        )
        .unwrap();
        assert_eq!(result2.events.len(), 0);
        assert_eq!(result2.cursor, final_cursor);
    }

    #[test]
    fn read_tail_events_returns_reset_when_cursor_exceeds_file_size() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.jsonl");
        let mut file = File::create(&path).unwrap();
        let line = sample_envelope(
            "event_msg",
            serde_json::json!({"type": "user_message", "message": "hello"}),
        );
        writeln!(file, "{line}").unwrap();
        drop(file);

        let out = read_tail_events(&path, Some(10_000_000), DEFAULT_TAIL_BYTE_LIMIT, 100).unwrap();
        assert!(out.reset);
        assert_eq!(out.events.len(), 1);
    }

    #[test]
    fn read_tail_events_ignores_partial_last_line() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.jsonl");
        let mut file = File::create(&path).unwrap();
        let full = sample_envelope(
            "event_msg",
            serde_json::json!({"type": "user_message", "message": "complete"}),
        );
        writeln!(file, "{full}").unwrap();
        write!(
            file,
            "{}",
            sample_envelope(
                "event_msg",
                serde_json::json!({"type": "user_message", "message": "partial"})
            )
        )
        .unwrap();
        drop(file);

        let out = read_tail_events(&path, None, DEFAULT_TAIL_BYTE_LIMIT, 100).unwrap();
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].preview, "complete");
    }
}

#[cfg(test)]
mod real_session_smoke {
    use super::*;

    /// Smoke test against a real ~/.codex/sessions/*.jsonl when available.
    /// Skipped silently when the environment doesn't have codex data.
    #[test]
    fn read_tail_on_real_session_when_available() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let root = std::path::PathBuf::from(home).join(".codex").join("sessions");
        if !root.is_dir() {
            return;
        }
        let Some(sample) = walkdir_find_jsonl(&root) else {
            return;
        };
        let out =
            read_tail_events(&sample, None, DEFAULT_TAIL_BYTE_LIMIT, DEFAULT_MAX_EVENTS)
                .expect("tail read");
        assert!(out.cursor > 0, "cursor should move when file has content");
    }

    fn walkdir_find_jsonl(root: &std::path::Path) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(root).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = walkdir_find_jsonl(&path) {
                    return Some(found);
                }
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                return Some(path);
            }
        }
        None
    }
}
