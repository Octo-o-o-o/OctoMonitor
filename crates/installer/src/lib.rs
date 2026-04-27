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

struct ToolSpec {
    name: &'static str,
    label: &'static str,
    mode: &'static str,
    capability: &'static str,
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "claude",
        label: "Claude CLI",
        mode: "statusline+hooks",
        capability: "hook/statusline monitoring path available",
    },
    ToolSpec {
        name: "codex",
        label: "Codex CLI",
        mode: "app-server+hooks",
        capability: "app-server/hook monitoring path available",
    },
    ToolSpec {
        name: "openclaw",
        label: "OpenClaw CLI",
        mode: "gateway+status",
        capability: "gateway/status monitoring path available",
    },
    ToolSpec {
        name: "hermes",
        label: "Hermes Agent CLI (experimental)",
        mode: "gateway+sessions",
        capability: "gateway/sessions monitoring path available",
    },
];

fn command_exists(name: &str) -> bool {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return is_executable(candidate);
    }

    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    #[cfg(windows)]
    let extra_exts: Vec<String> = if candidate.extension().is_none() {
        windows_path_exts()
    } else {
        Vec::new()
    };

    for dir in env::split_paths(&paths) {
        if is_executable(&dir.join(name)) {
            return true;
        }

        #[cfg(windows)]
        for ext in &extra_exts {
            if is_executable(&dir.join(format!("{name}{ext}"))) {
                return true;
            }
        }
    }

    false
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };

    #[cfg(unix)]
    {
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
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

pub fn detect_tools() -> Vec<ToolCapability> {
    TOOLS
        .iter()
        .map(|spec| {
            let detected = command_exists(spec.name);
            ToolCapability {
                tool: spec.name,
                detected,
                mode: spec.mode,
                notes: if detected {
                    format!("{} detected; {}", spec.label, spec.capability)
                } else {
                    format!("{} not found on PATH", spec.label)
                },
            }
        })
        .collect()
}

pub fn doctor_report() -> Vec<String> {
    let tools = detect_tools();
    let detected = tools.iter().filter(|tool| tool.detected).count();
    let total = tools.len();

    let mut checks = vec![
        "No database configured — expected for local-first mode".to_string(),
        "Companion access disabled by default until config changes".to_string(),
        "Environment & Doctor is read-only — no tool config files are modified automatically"
            .to_string(),
        format!("Detected {detected}/{total} monitored CLIs on current PATH"),
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
