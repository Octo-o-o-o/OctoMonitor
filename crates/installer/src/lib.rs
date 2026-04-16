use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapability {
    pub tool: &'static str,
    pub detected: bool,
    pub mode: &'static str,
    pub notes: String,
}

fn command_exists(name: &str) -> bool {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return is_executable(candidate);
    }

    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    for dir in env::split_paths(&paths) {
        let direct = dir.join(name);
        if is_executable(&direct) {
            return true;
        }

        #[cfg(windows)]
        if candidate.extension().is_none() {
            for ext in windows_path_exts() {
                if is_executable(&dir.join(format!("{name}{ext}"))) {
                    return true;
                }
            }
        }
    }

    false
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(windows)]
fn windows_path_exts() -> Vec<String> {
    env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
        .split(';')
        .filter(|ext| !ext.is_empty())
        .map(str::to_string)
        .collect()
}

fn tool_mode(tool: &str) -> &'static str {
    match tool {
        "claude" => "statusline+hooks",
        "codex" => "app-server+hooks",
        "openclaw" => "gateway+status",
        "hermes" => "gateway+sessions",
        _ => "custom",
    }
}

pub fn detect_tools() -> Vec<ToolCapability> {
    [
        (
            "claude",
            "Claude CLI",
            "hook/statusline monitoring path available",
        ),
        (
            "codex",
            "Codex CLI",
            "app-server/hook monitoring path available",
        ),
        (
            "openclaw",
            "OpenClaw CLI",
            "gateway/status monitoring path available",
        ),
        (
            "hermes",
            "Hermes Agent CLI (experimental)",
            "gateway/sessions monitoring path available",
        ),
    ]
    .into_iter()
    .map(|(name, label, capability)| {
        let detected = command_exists(name);
        ToolCapability {
            tool: name,
            detected,
            mode: tool_mode(name),
            notes: if detected {
                format!("{label} detected; {capability}")
            } else {
                format!("{label} not found on PATH")
            },
        }
    })
    .collect()
}

pub fn doctor_report() -> Vec<String> {
    let tools = detect_tools();
    let detected = tools.iter().filter(|tool| tool.detected).count();
    let mut checks = vec![
        "No database configured — expected for local-first mode".to_string(),
        "Companion access disabled by default until config changes".to_string(),
        "Environment & Doctor is read-only — no tool config files are modified automatically"
            .to_string(),
        format!("Detected {detected}/4 monitored CLIs on current PATH"),
    ];
    checks.extend(tools.into_iter().map(|tool| {
        if tool.detected {
            format!("{}: detected ({})", tool.tool, tool.mode)
        } else {
            format!("{}: missing from PATH", tool.tool)
        }
    }));
    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tools_returns_four() {
        let tools = detect_tools();
        assert_eq!(tools.len(), 4);
    }

    #[test]
    fn doctor_report_is_nonempty() {
        let checks = doctor_report();
        assert!(!checks.is_empty());
        assert!(checks.iter().any(|line| line.contains("read-only")));
    }
}
