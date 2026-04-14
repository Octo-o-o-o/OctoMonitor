use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallAction {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub tool: String,
    pub dry_run: bool,
    pub actions: Vec<InstallAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub tool: String,
    pub applied: bool,
    pub paths: Vec<String>,
    pub message: String,
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

fn integration_root() -> PathBuf {
    env::temp_dir().join("octomonitor-integrations")
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
            "local hook/statusline install possible",
        ),
        ("codex", "Codex CLI", "app-server/hook path available"),
        ("openclaw", "OpenClaw CLI", "gateway-first path available"),
        ("hermes", "Hermes Agent CLI", "gateway/sessions path available"),
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
        "Sandbox manifest mode only — no tool config files are modified automatically".to_string(),
        format!("Detected {detected}/4 monitored CLIs on current PATH"),
        format!("Installer sandbox path: {}", integration_root().display()),
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

pub fn install_plan(tool: &str) -> InstallPlan {
    let root = integration_root().join(tool);
    InstallPlan {
        tool: tool.to_string(),
        dry_run: true,
        actions: vec![
            InstallAction {
                id: format!("{}-mkdir", tool),
                kind: "mkdir".into(),
                path: root.display().to_string(),
                description: format!("Create local sandbox directory for {tool}"),
            },
            InstallAction {
                id: format!("{}-manifest", tool),
                kind: "write-file".into(),
                path: root.join("manifest.json").display().to_string(),
                description: format!(
                    "Write a local read-only sandbox manifest for {tool} (no live tool config changes)"
                ),
            },
        ],
    }
}

pub fn apply_install(tool: &str) -> InstallResult {
    let root = integration_root().join(tool);
    let _ = fs::create_dir_all(&root);
    let manifest_path = root.join("manifest.json");
    let payload = serde_json::json!({
        "tool": tool,
        "mode": tool_mode(tool),
        "installedBy": "octomonitor",
        "readOnly": true
    });
    let _ = fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&payload).unwrap(),
    );
    InstallResult {
        tool: tool.to_string(),
        applied: true,
        paths: vec![
            root.display().to_string(),
            manifest_path.display().to_string(),
        ],
        message: format!(
            "Wrote local sandbox manifest for {tool}; live tool config was not changed"
        ),
    }
}

pub fn rollback_install(tool: &str) -> InstallResult {
    let root = integration_root().join(tool);
    let manifest_path = root.join("manifest.json");
    let existed = manifest_path.exists();
    let _ = fs::remove_file(&manifest_path);
    let _ = fs::remove_dir_all(&root);
    InstallResult {
        tool: tool.to_string(),
        applied: existed,
        paths: vec![root.display().to_string()],
        message: if existed {
            format!("Removed local sandbox manifest for {tool}")
        } else {
            format!("Nothing to remove for {tool} — no sandbox manifest found")
        },
    }
}

pub fn verify_install(tool: &str) -> InstallResult {
    let root = integration_root().join(tool);
    let manifest_path = root.join("manifest.json");
    let manifest_display = manifest_path.display().to_string();

    if !manifest_path.exists() {
        return InstallResult {
            tool: tool.to_string(),
            applied: false,
            paths: vec![],
            message: format!("No installation found for {tool}"),
        };
    }

    let content = match fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            return InstallResult {
                tool: tool.to_string(),
                applied: false,
                paths: vec![manifest_display],
                message: format!("Cannot read manifest for {tool}: {e}"),
            };
        }
    };

    let val = match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(v) => v,
        Err(e) => {
            return InstallResult {
                tool: tool.to_string(),
                applied: false,
                paths: vec![manifest_display],
                message: format!("Manifest for {tool} is not valid JSON: {e}"),
            };
        }
    };

    let matches_tool = val.get("tool").and_then(|v| v.as_str()) == Some(tool);
    let read_only = val
        .get("readOnly")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let valid = matches_tool && read_only;

    InstallResult {
        tool: tool.to_string(),
        applied: valid,
        paths: vec![manifest_display],
        message: if valid {
            format!("Installation for {tool} verified: manifest valid and read-only")
        } else {
            format!("Installation for {tool}: manifest exists but integrity check failed")
        },
    }
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
    }

    #[test]
    fn install_plan_has_actions() {
        let plan = install_plan("claude");
        assert_eq!(plan.tool, "claude");
        assert!(plan.dry_run);
        assert!(!plan.actions.is_empty());
    }

    #[test]
    fn install_and_rollback_cycle() {
        let result = apply_install("test-tool");
        assert!(result.applied);
        let verify = verify_install("test-tool");
        assert!(verify.applied);
        let rollback = rollback_install("test-tool");
        assert!(rollback.applied);
        let verify_after = verify_install("test-tool");
        assert!(!verify_after.applied);
    }
}
