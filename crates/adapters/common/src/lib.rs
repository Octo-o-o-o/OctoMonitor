use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDescriptor {
    pub tool: &'static str,
    pub preferred_mode: &'static str,
    pub fallback_mode: &'static str,
    pub confidence: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandProbeResult {
    pub command: String,
    pub success: bool,
    pub stdout_snippet: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProbeResult {
    pub path: String,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct JsonlCursor {
    pub offset: u64,
}

#[derive(Debug, Clone)]
pub struct JsonlDelta {
    pub lines: Vec<String>,
    pub reset: bool,
}

pub fn run_command_probe(cmd: &str, args: &[&str]) -> CommandProbeResult {
    let command = format!("{} {}", cmd, args.join(" "));
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            let success = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            CommandProbeResult {
                command,
                success,
                stdout_snippet: snippet(&stdout),
                error: if success { None } else { snippet(&stderr) },
            }
        }
        Err(e) => CommandProbeResult {
            command,
            success: false,
            stdout_snippet: None,
            error: Some(e.to_string()),
        },
    }
}

fn snippet(text: &str) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text.chars().take(200).collect())
    }
}

pub fn probe_file(path: &Path) -> FileProbeResult {
    let meta = fs::metadata(path).ok();
    FileProbeResult {
        path: path.display().to_string(),
        exists: meta.is_some(),
        size_bytes: meta.as_ref().map(fs::Metadata::len),
        modified_at: meta
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339()),
    }
}

pub fn read_jsonl_delta(path: &Path, cursor: &mut JsonlCursor) -> io::Result<JsonlDelta> {
    let mut file = fs::File::open(path)?;
    let reset = file.metadata()?.len() < cursor.offset;
    if reset {
        cursor.offset = 0;
    }

    file.seek(SeekFrom::Start(cursor.offset))?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let end = line.trim_end_matches(['\n', '\r']).len();
        line.truncate(end);
        lines.push(std::mem::take(&mut line));
    }
    cursor.offset = reader.stream_position()?;

    Ok(JsonlDelta { lines, reset })
}

/// Mask a sensitive value, showing only the first and last 4 characters.
/// Values shorter than `min_visible` are fully masked.
pub fn mask_value(value: &str, min_visible: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= min_visible {
        return "****".to_string();
    }
    let prefix: String = chars[..4].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{prefix}…{suffix}")
}

/// Resolve a home-relative directory, falling back to HOME / USERPROFILE or "."
pub fn resolve_home_dir(relative: &str) -> PathBuf {
    home_dir()
        .map(|home| home.join(relative))
        .unwrap_or_else(|| PathBuf::from(".").join(relative))
}

fn home_drive_path() -> Option<OsString> {
    let mut combined = env::var_os("HOMEDRIVE")?;
    combined.push(env::var_os("HOMEPATH")?);
    Some(combined)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(home_drive_path)
        .map(PathBuf::from)
}

/// Resolve a directory by checking environment variables in order, falling back
/// to the given home-relative path. Empty env values are ignored.
pub fn resolve_env_or_home(env_vars: &[&str], home_relative: &str) -> PathBuf {
    env_vars
        .iter()
        .filter_map(|name| env::var(name).ok())
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_home_dir(home_relative))
}

/// Capitalize the first character of a string.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Truncate a string to at most `max_chars` characters (counting chars, not bytes).
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Metadata signature used to invalidate per-file caches.
pub fn file_signature(path: &Path) -> Option<(u64, Option<SystemTime>)> {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

/// Scan `dir` for direct file entries and return the most recently modified
/// file name. Directories and unreadable entries are ignored. If `extension`
/// is `Some`, only files with that extension are considered.
pub fn latest_file_name(dir: &Path, extension: Option<&str>) -> Option<String> {
    let mut newest: Option<(SystemTime, String)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        if let Some(ext) = extension {
            if entry.path().extension().is_none_or(|e| e != ext) {
                continue;
            }
        }
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
            newest = Some((modified, name));
        }
    }
    newest.map(|(_, name)| name)
}

/// Format a standard 5-field cron expression (`min hour dom mon dow`) as a
/// human-readable schedule. Falls back to `"<expr> (<tz>)"` when the
/// expression uses wildcards we don't render specially.
pub fn format_cron_expr(expr: &str, tz: &str) -> String {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() < 5 {
        return format!("{expr} ({tz})");
    }
    let (min, hour, _dom, _mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);

    if hour == "*" || min == "*" {
        return format!("{expr} ({tz})");
    }
    let time_str = format!("{hour:0>2}:{min:0>2}");

    if dow == "*" {
        return format!("Daily {time_str}");
    }

    let dow_str = dow
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalize_handles_empty_and_unicode() {
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("telegram"), "Telegram");
        assert_eq!(capitalize("é"), "É");
    }

    #[test]
    fn truncate_chars_counts_chars_not_bytes() {
        assert_eq!(truncate_chars("héllo", 3), "hél");
        assert_eq!(truncate_chars("abc", 10), "abc");
    }

    #[test]
    fn format_cron_expr_formats_common_patterns() {
        assert_eq!(format_cron_expr("0 9 * * *", "UTC"), "Daily 09:00");
        assert_eq!(format_cron_expr("30 17 * * 1,3,5", "UTC"), "Mon,Wed,Fri 17:30");
        assert_eq!(format_cron_expr("* * * * *", "UTC"), "* * * * * (UTC)");
        assert_eq!(format_cron_expr("bad", "UTC"), "bad (UTC)");
    }

    #[test]
    fn latest_file_name_skips_dirs_and_wrong_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("a.jsonl"), "a").expect("a");
        std::fs::create_dir(dir.path().join("sub")).expect("sub");
        std::fs::write(dir.path().join("b.txt"), "b").expect("b");

        let latest = latest_file_name(dir.path(), Some("jsonl"));
        assert_eq!(latest.as_deref(), Some("a.jsonl"));
    }

    #[test]
    fn read_jsonl_delta_reads_only_new_lines_and_resets_on_truncate() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("session.jsonl");
        std::fs::write(&path, "{\"a\":1}\n{\"a\":2}\n").expect("initial file");

        let mut cursor = JsonlCursor::default();
        let first = read_jsonl_delta(&path, &mut cursor).expect("first read");
        assert!(!first.reset);
        assert_eq!(first.lines.len(), 2);

        std::fs::write(&path, "{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n").expect("append file");
        let second = read_jsonl_delta(&path, &mut cursor).expect("second read");
        assert!(!second.reset);
        assert_eq!(second.lines, vec![r#"{"a":3}"#]);

        std::fs::write(&path, "{\"b\":1}\n").expect("truncate file");
        let third = read_jsonl_delta(&path, &mut cursor).expect("third read");
        assert!(third.reset);
        assert_eq!(third.lines, vec![r#"{"b":1}"#]);
    }
}
