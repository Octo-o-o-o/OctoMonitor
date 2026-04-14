use std::collections::HashMap;
use std::process::Stdio;

use anyhow::Result;
use octomonitor_core::ToolKind;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

// --- Types ---

#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub tool: ToolKind,
    pub prompt: String,
    pub working_dir: String,
    pub model: Option<String>,
    pub timeout_secs: Option<u64>,
    pub allowed_tools: Vec<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LauncherHandle {
    pub id: String,
    pub tool: ToolKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct LauncherResult {
    pub exit_code: i32,
    pub events: Vec<LauncherEvent>,
    pub stdout_text: String,
    pub changed_files: Vec<String>,
}

#[derive(Debug)]
pub enum LaunchError {
    CliNotAvailable(String),
    SpawnFailed(String),
    Timeout,
    Other(String),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::CliNotAvailable(msg) => write!(f, "CLI not available: {msg}"),
            LaunchError::SpawnFailed(msg) => write!(f, "Spawn failed: {msg}"),
            LaunchError::Timeout => write!(f, "Timeout"),
            LaunchError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for LaunchError {}

// --- Trait ---

pub trait ToolLauncher: Send + Sync {
    fn launch(
        &self,
        request: LaunchRequest,
    ) -> impl std::future::Future<Output = Result<LauncherResult, LaunchError>> + Send;
}

// --- Claude Launcher ---

pub struct ClaudeLauncher;

impl ToolLauncher for ClaudeLauncher {
    async fn launch(&self, request: LaunchRequest) -> Result<LauncherResult, LaunchError> {
        // Build command
        let mut cmd = Command::new("claude");
        cmd.arg("-p").arg(&request.prompt);
        cmd.arg("--output-format").arg("stream-json");

        if let Some(model) = &request.model {
            cmd.arg("--model").arg(model);
        }

        if !request.allowed_tools.is_empty() {
            cmd.arg("--allowedTools")
                .arg(request.allowed_tools.join(","));
        }

        for arg in &request.args {
            cmd.arg(arg);
        }

        cmd.current_dir(&request.working_dir);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        run_cli_command(cmd, request.timeout_secs, &request.working_dir).await
    }
}

// --- Codex Launcher ---

pub struct CodexLauncher;

impl ToolLauncher for CodexLauncher {
    async fn launch(&self, request: LaunchRequest) -> Result<LauncherResult, LaunchError> {
        let mut cmd = Command::new("codex");
        cmd.arg("exec").arg(&request.prompt);
        cmd.arg("--json");
        cmd.arg("--cd").arg(&request.working_dir);

        if let Some(model) = &request.model {
            cmd.arg("--model").arg(model);
        }

        for arg in &request.args {
            cmd.arg(arg);
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        run_cli_command(cmd, request.timeout_secs, &request.working_dir).await
    }
}

// --- Dispatcher ---

pub struct LauncherDispatcher {
    capabilities: HashMap<ToolKind, bool>,
}

impl LauncherDispatcher {
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
        }
    }

    /// Check which CLI tools are available on this system.
    pub async fn detect_capabilities(&mut self) {
        self.capabilities
            .insert(ToolKind::Claude, check_cli("claude", &["--version"]).await);
        self.capabilities
            .insert(ToolKind::Codex, check_cli("codex", &["--version"]).await);
    }

    pub fn is_available(&self, tool: &ToolKind) -> bool {
        self.capabilities.get(tool).copied().unwrap_or(false)
    }

    pub async fn launch(&self, request: LaunchRequest) -> Result<LauncherResult, LaunchError> {
        if !self.is_available(&request.tool) {
            return Err(LaunchError::CliNotAvailable(format!(
                "{:?} CLI not detected",
                request.tool
            )));
        }

        match request.tool {
            ToolKind::Claude => ClaudeLauncher.launch(request).await,
            ToolKind::Codex => CodexLauncher.launch(request).await,
            ToolKind::OpenClaw => Err(LaunchError::CliNotAvailable(
                "OpenClaw launcher not yet implemented".into(),
            )),
            ToolKind::Hermes => Err(LaunchError::CliNotAvailable(
                "Hermes launcher not yet implemented".into(),
            )),
        }
    }
}

// --- Helpers ---

async fn check_cli(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}

async fn run_cli_command(
    mut cmd: Command,
    timeout_secs: Option<u64>,
    working_dir: &str,
) -> Result<LauncherResult, LaunchError> {
    let mut child = cmd
        .spawn()
        .map_err(|e| LaunchError::SpawnFailed(e.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| LaunchError::SpawnFailed("No stdout".into()))?;

    let mut reader = BufReader::new(stdout).lines();
    let mut events = Vec::new();
    let mut stdout_text = String::new();

    let timeout_duration = tokio::time::Duration::from_secs(timeout_secs.unwrap_or(600));

    let result = tokio::time::timeout(timeout_duration, async {
        while let Ok(Some(line)) = reader.next_line().await {
            if let Ok(event) = serde_json::from_str::<LauncherEvent>(&line) {
                if event.event_type == "result" || event.event_type == "assistant" {
                    if let Some(msg) = &event.message {
                        stdout_text.push_str(msg);
                        stdout_text.push('\n');
                    }
                }
                events.push(event);
            }
        }
    })
    .await;

    if result.is_err() {
        let _ = child.kill().await;
        return Err(LaunchError::Timeout);
    }

    let status = child
        .wait()
        .await
        .map_err(|e| LaunchError::Other(e.to_string()))?;

    let changed_files = collect_changed_files(working_dir).await;

    Ok(LauncherResult {
        exit_code: status.code().unwrap_or(-1),
        events,
        stdout_text,
        changed_files,
    })
}

async fn collect_changed_files(working_dir: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dispatcher_detects_capabilities() {
        let mut dispatcher = LauncherDispatcher::new();
        dispatcher.detect_capabilities().await;
        // At minimum, the detection runs without panicking.
        // Claude may or may not be available on CI.
        let _ = dispatcher.is_available(&ToolKind::Claude);
    }

    #[tokio::test]
    async fn launch_echo_command() {
        // Test with a simple echo command to verify the spawn + capture flow.
        // We use `echo` directly via Command rather than the launcher trait
        // to avoid requiring actual CLI tools.
        let mut cmd = Command::new("echo");
        cmd.arg(r#"{"type":"result","message":"hello"}"#);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let result = run_cli_command(cmd, Some(5), "/tmp").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.events[0].event_type, "result");
        assert!(r.stdout_text.contains("hello"));
    }
}
