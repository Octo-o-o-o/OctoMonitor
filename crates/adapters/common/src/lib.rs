use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
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
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            CommandProbeResult {
                command: format!("{} {}", cmd, args.join(" ")),
                success: output.status.success(),
                stdout_snippet: if stdout.is_empty() {
                    None
                } else {
                    Some(stdout.chars().take(200).collect())
                },
                error: if stderr.is_empty() || output.status.success() {
                    None
                } else {
                    Some(stderr.chars().take(200).collect())
                },
            }
        }
        Err(e) => CommandProbeResult {
            command: format!("{} {}", cmd, args.join(" ")),
            success: false,
            stdout_snippet: None,
            error: Some(e.to_string()),
        },
    }
}

pub fn probe_file(path: &Path) -> FileProbeResult {
    let meta = fs::metadata(path).ok();
    FileProbeResult {
        path: path.display().to_string(),
        exists: meta.is_some(),
        size_bytes: meta.as_ref().map(|m| m.len()),
        modified_at: meta
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339()),
    }
}

pub fn read_jsonl_delta(path: &Path, cursor: &mut JsonlCursor) -> io::Result<JsonlDelta> {
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let mut reset = false;
    if metadata.len() < cursor.offset {
        cursor.offset = 0;
        reset = true;
    }

    file.seek(SeekFrom::Start(cursor.offset))?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        lines.push(line);
    }
    cursor.offset = reader.stream_position()?;

    Ok(JsonlDelta { lines, reset })
}

/// Mask a sensitive value, showing only the first and last 4 characters.
/// Values shorter than `min_visible` are fully masked.
pub fn mask_value(value: &str, min_visible: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= min_visible {
        "****".to_string()
    } else {
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[chars.len() - 4..].iter().collect();
        format!("{prefix}…{suffix}")
    }
}

/// Resolve a home-relative directory, falling back to HOME / USERPROFILE or "."
pub fn resolve_home_dir(relative: &str) -> PathBuf {
    home_dir()
        .map(|home| home.join(relative))
        .unwrap_or_else(|| PathBuf::from(".").join(relative))
}

fn home_drive_path() -> Option<OsString> {
    let (Some(drive), Some(path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) else {
        return None;
    };
    let mut combined = drive;
    combined.push(path);
    Some(combined)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(home_drive_path)
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

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
