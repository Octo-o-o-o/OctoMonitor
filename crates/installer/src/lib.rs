use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SupportLevel {
    Monitored,
    Experimental,
    Candidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapability {
    pub tool: &'static str,
    pub detected: bool,
    pub detected_command: Option<&'static str>,
    pub mode: &'static str,
    pub support_level: SupportLevel,
    pub notes: String,
}

struct ToolSpec {
    name: &'static str,
    label: &'static str,
    commands: &'static [&'static str],
    mode: &'static str,
    capability: &'static str,
    support_level: SupportLevel,
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "claude",
        label: "Claude CLI",
        commands: &["claude"],
        mode: "statusline+hooks",
        capability: "hook/statusline monitoring path available",
        support_level: SupportLevel::Monitored,
    },
    ToolSpec {
        name: "codex",
        label: "Codex CLI",
        commands: &["codex"],
        mode: "app-server+hooks",
        capability: "app-server/hook monitoring path available",
        support_level: SupportLevel::Monitored,
    },
    ToolSpec {
        name: "openclaw",
        label: "OpenClaw CLI",
        commands: &["openclaw"],
        mode: "gateway+status",
        capability: "gateway/status monitoring path available",
        support_level: SupportLevel::Monitored,
    },
    ToolSpec {
        name: "hermes",
        label: "Hermes Agent CLI (experimental)",
        commands: &["hermes"],
        mode: "gateway+sessions",
        capability: "gateway/sessions monitoring path available",
        support_level: SupportLevel::Experimental,
    },
    ToolSpec {
        name: "gemini",
        label: "Gemini CLI",
        commands: &["gemini"],
        mode: "fixture-gated passive JSONL + hooks-ready",
        capability:
            "fixture-gated passive scan can count sessions/usage; hooks are opt-in for live metadata",
        support_level: SupportLevel::Experimental,
    },
    ToolSpec {
        name: "cursor",
        label: "Cursor Agent CLI",
        commands: &["agent"],
        mode: "experimental opt-in private store",
        capability:
            "private store parser is disabled unless OCTOMONITOR_CURSOR_PRIVATE_STORE is set; usage is N/A",
        support_level: SupportLevel::Experimental,
    },
    ToolSpec {
        name: "workbuddy",
        label: "WorkBuddy / CodeBuddy CLI",
        commands: &["workbuddy", "codebuddy"],
        mode: "codebuddy fixture-gated; workbuddy detection-only",
        capability:
            "CodeBuddy passive scan is fixture-gated; WorkBuddy remains detection-only until a real layout is verified",
        support_level: SupportLevel::Candidate,
    },
    ToolSpec {
        name: "pi",
        label: "Pi Coding Agent CLI",
        commands: &["pi"],
        mode: "fixture-gated passive JSONL",
        capability: "fixture-gated passive scan can count sessions and usage; RPC operations remain out of scope",
        support_level: SupportLevel::Experimental,
    },
];

fn detected_command(commands: &[&'static str]) -> Option<&'static str> {
    commands
        .iter()
        .copied()
        .find(|command| command_exists(command))
}

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
            let detected_command = detected_command(spec.commands);
            let detected = detected_command.is_some();
            ToolCapability {
                tool: spec.name,
                detected,
                detected_command,
                mode: spec.mode,
                support_level: spec.support_level.clone(),
                notes: if detected {
                    format!(
                        "{} detected via {}; {}",
                        spec.label,
                        detected_command.unwrap_or(spec.name),
                        spec.capability
                    )
                } else {
                    format!(
                        "{} not found on PATH (expected: {})",
                        spec.label,
                        spec.commands.join(", ")
                    )
                },
            }
        })
        .collect()
}

pub fn doctor_report() -> Vec<String> {
    let tools = detect_tools();
    let monitored_total = tools
        .iter()
        .filter(|tool| {
            matches!(
                tool.support_level,
                SupportLevel::Monitored | SupportLevel::Experimental
            )
        })
        .count();
    let monitored_detected = tools
        .iter()
        .filter(|tool| {
            tool.detected
                && matches!(
                    tool.support_level,
                    SupportLevel::Monitored | SupportLevel::Experimental
                )
        })
        .count();
    let candidate_total = tools
        .iter()
        .filter(|tool| matches!(tool.support_level, SupportLevel::Candidate))
        .count();
    let candidate_detected = tools
        .iter()
        .filter(|tool| tool.detected && matches!(tool.support_level, SupportLevel::Candidate))
        .count();

    let mut checks = vec![
        "No database configured — expected for local-first mode".to_string(),
        "Companion access disabled by default until config changes".to_string(),
        "Environment & Doctor is read-only — no tool config files are modified automatically"
            .to_string(),
        format!("Detected {monitored_detected}/{monitored_total} monitored CLIs on current PATH"),
        format!("Detected {candidate_detected}/{candidate_total} candidate CLIs on current PATH"),
    ];
    checks.extend(tools.into_iter().map(|tool| {
        let level = match tool.support_level {
            SupportLevel::Monitored => "monitored",
            SupportLevel::Experimental => "experimental",
            SupportLevel::Candidate => "candidate",
        };
        if tool.detected {
            format!("{}: detected ({}, {level})", tool.tool, tool.mode)
        } else {
            format!("{}: missing from PATH ({level})", tool.tool)
        }
    }));
    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tools_returns_known_tools() {
        let tools = detect_tools();
        assert_eq!(tools.len(), 8);
        assert!(tools.iter().any(|tool| tool.tool == "gemini"));
        assert!(tools.iter().any(|tool| tool.tool == "cursor"));
        assert!(tools.iter().any(|tool| tool.tool == "workbuddy"));
        assert!(tools.iter().any(|tool| tool.tool == "pi"));
    }

    #[test]
    fn doctor_report_is_nonempty() {
        let checks = doctor_report();
        assert!(!checks.is_empty());
        assert!(checks.iter().any(|line| line.contains("read-only")));
    }
}
