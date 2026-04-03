use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

// --- Shared probe types ---

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

// --- Shared probe functions ---

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
    let exists = path.exists();
    let meta = if exists {
        fs::metadata(path).ok()
    } else {
        None
    };
    FileProbeResult {
        path: path.display().to_string(),
        exists,
        size_bytes: meta.as_ref().map(|m| m.len()),
        modified_at: meta
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339()),
    }
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
