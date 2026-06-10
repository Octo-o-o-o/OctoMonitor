use octomonitor_core::{
    JumpReliability, JumpTarget, JumpTargetKind, RunRecord, TerminalProvider, TerminalTarget,
    ToolKind,
};

pub(crate) fn hydrate_jump_targets(run: &mut RunRecord) {
    run.jump_targets = Some(build_jump_targets(run));
}

pub(crate) fn build_jump_targets(run: &RunRecord) -> Vec<JumpTarget> {
    let mut targets = Vec::new();
    let cwd = nonempty(&run.workspace_path).map(str::to_string);
    let resume = resume_argv(run);

    if let Some(cwd) = cwd.as_deref() {
        targets.push(jump(
            JumpTargetKind::Workspace,
            "Open workspace",
            None,
            Some(cwd),
            None,
            None,
            JumpReliability::High,
            true,
        ));
        targets.push(jump(
            JumpTargetKind::NativeApp,
            "Open workspace in VS Code",
            Some(vec!["code".into(), cwd.into()]),
            Some(cwd),
            None,
            Some(terminal(TerminalProvider::VsCode)),
            JumpReliability::High,
            false,
        ));
        targets.push(jump(
            JumpTargetKind::NativeApp,
            "Open workspace in Cursor",
            Some(vec!["cursor".into(), cwd.into()]),
            Some(cwd),
            None,
            Some(terminal(TerminalProvider::Cursor)),
            JumpReliability::Medium,
            false,
        ));
    }

    if let Some(argv) = resume.as_ref() {
        targets.push(jump(
            JumpTargetKind::CopyCommand,
            "Copy resume command",
            Some(argv.clone()),
            cwd.as_deref(),
            None,
            None,
            JumpReliability::High,
            false,
        ));

        if let Some(cwd) = cwd.as_deref() {
            targets.extend(new_terminal_targets(cwd, argv));
        }
    }

    if run.tool == ToolKind::Codex {
        if let Some(thread_id) = nonempty(run.thread_id.as_deref().unwrap_or_default()) {
            targets.push(jump(
                JumpTargetKind::SessionDeeplink,
                "Open in Codex",
                None,
                cwd.as_deref(),
                Some(format!("codex://threads/{}", percent_encode(thread_id))),
                None,
                JumpReliability::Medium,
                false,
            ));
        }
    }

    if let Some(terminal) = managed_terminal_target(run) {
        targets.push(jump(
            JumpTargetKind::ManagedTerminalFocus,
            "Focus managed terminal",
            None,
            cwd.as_deref(),
            None,
            Some(terminal),
            JumpReliability::High,
            true,
        ));
    }

    targets
}

fn new_terminal_targets(cwd: &str, argv: &[String]) -> Vec<JumpTarget> {
    let resume_line = shell_join(argv);
    let command_line = format!("cd {} && {}", shell_quote(cwd), resume_line);
    vec![
        jump(
            JumpTargetKind::NewTerminalTab,
            "Warp new tab",
            Some(argv.to_vec()),
            Some(cwd),
            Some(format!(
                "warp://action/new_tab?path={}",
                percent_encode(cwd)
            )),
            Some(terminal(TerminalProvider::Warp)),
            JumpReliability::Medium,
            false,
        ),
        jump(
            JumpTargetKind::NewTerminalTab,
            "iTerm2 new window",
            Some(vec![
                "osascript".into(),
                "-e".into(),
                format!(
                    "tell application \"iTerm2\" to create window with default profile command \"{}\"",
                    applescript_escape(&command_line)
                ),
            ]),
            Some(cwd),
            None,
            Some(terminal(TerminalProvider::ITerm2)),
            JumpReliability::Medium,
            false,
        ),
        jump(
            JumpTargetKind::NewTerminalTab,
            "Ghostty new window",
            Some(vec![
                "ghostty".into(),
                "+new-window".into(),
                "--working-directory".into(),
                cwd.into(),
                "-e".into(),
                "/bin/sh".into(),
                "-lc".into(),
                resume_line.clone(),
            ]),
            Some(cwd),
            None,
            Some(terminal(TerminalProvider::Ghostty)),
            JumpReliability::Low,
            false,
        ),
        jump(
            JumpTargetKind::NewTerminalTab,
            "Terminal.app new window",
            Some(vec![
                "osascript".into(),
                "-e".into(),
                format!(
                    "tell application \"Terminal\" to do script \"{}\"",
                    applescript_escape(&command_line)
                ),
            ]),
            Some(cwd),
            None,
            Some(terminal(TerminalProvider::TerminalApp)),
            JumpReliability::Medium,
            false,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn jump(
    kind: JumpTargetKind,
    label: impl Into<String>,
    command: Option<Vec<String>>,
    cwd: Option<&str>,
    url: Option<String>,
    terminal: Option<TerminalTarget>,
    reliability: JumpReliability,
    requires_confirmation: bool,
) -> JumpTarget {
    JumpTarget {
        kind,
        label: label.into(),
        command,
        cwd: cwd.map(str::to_string),
        url,
        terminal,
        reliability,
        requires_confirmation,
    }
}

fn terminal(provider: TerminalProvider) -> TerminalTarget {
    TerminalTarget {
        provider,
        window_id: None,
        tab_id: None,
        pane_id: None,
        pid: None,
    }
}

fn nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn resume_argv(run: &RunRecord) -> Option<Vec<String>> {
    match run.tool {
        ToolKind::Codex => {
            let thread_id = nonempty(run.thread_id.as_deref()?)?;
            Some(vec!["codex".into(), "resume".into(), thread_id.into()])
        }
        ToolKind::Claude => {
            let session_id = nonempty(run.session_id.as_deref()?)?;
            Some(vec!["claude".into(), "--resume".into(), session_id.into()])
        }
        ToolKind::Hermes => {
            let session_id = nonempty(run.session_id.as_deref()?)?;
            let profile = run
                .agent_name
                .as_deref()
                .and_then(nonempty)
                .filter(|value| *value != "default" && *value != "local-probe");
            let mut argv = vec!["hermes".into()];
            if let Some(profile) = profile {
                argv.extend(["-p".into(), profile.into()]);
            }
            argv.extend(["--resume".into(), session_id.into()]);
            Some(argv)
        }
        ToolKind::CodeBuddy => resume_with_session(run, &["codebuddy", "--resume"]),
        ToolKind::Gemini => resume_with_session(run, &["gemini", "--resume"]),
        ToolKind::Pi => resume_with_session(run, &["pi", "--session"]),
        ToolKind::OpenCode => resume_with_session(run, &["opencode", "session"]),
        ToolKind::Copilot => resume_with_session(run, &["copilot", "session", "resume"]),
        ToolKind::OpenHands => resume_with_session(run, &["openhands", "--conversation-id"]),
        ToolKind::ContinueCn => resume_with_session(run, &["cn", "--resume"]),
        ToolKind::Qwen => resume_with_session(run, &["qwen", "--resume"]),
        ToolKind::Kimi => resume_with_session(run, &["kimi", "--session"]),
        ToolKind::Goose => resume_with_session(run, &["goose", "session", "resume"]),
        ToolKind::Cursor => resume_with_session(run, &["agent", "--resume"]),
        ToolKind::Kiro => resume_with_session(run, &["kiro-cli", "chat", "--resume-id"]),
        ToolKind::OpenClaw
        | ToolKind::Cline
        | ToolKind::WorkBuddy
        | ToolKind::AmazonQ
        | ToolKind::Aider
        | ToolKind::Amp
        | ToolKind::Windsurf
        | ToolKind::Codebuff
        | ToolKind::Roo
        | ToolKind::Kilo => None,
    }
}

fn resume_with_session(run: &RunRecord, prefix: &[&str]) -> Option<Vec<String>> {
    let session_id = nonempty(run.session_id.as_deref()?)?;
    let mut argv: Vec<String> = prefix.iter().map(|value| (*value).into()).collect();
    argv.push(session_id.into());
    Some(argv)
}

fn managed_terminal_target(run: &RunRecord) -> Option<TerminalTarget> {
    let value = run.tool_specific.as_ref()?.get("managedTerminal")?;
    let provider = match value.get("provider")?.as_str()? {
        "warp" => TerminalProvider::Warp,
        "iTerm2" | "iterm2" => TerminalProvider::ITerm2,
        "ghostty" => TerminalProvider::Ghostty,
        "terminalApp" | "terminal.app" => TerminalProvider::TerminalApp,
        "vsCode" | "vscode" => TerminalProvider::VsCode,
        "cursor" => TerminalProvider::Cursor,
        "tmux" => TerminalProvider::Tmux,
        _ => return None,
    };
    let pid = value
        .get("pid")
        .and_then(|pid| pid.as_u64())
        .and_then(|pid| u32::try_from(pid).ok());
    let target = TerminalTarget {
        provider,
        window_id: string_field(value, "windowId"),
        tab_id: string_field(value, "tabId"),
        pane_id: string_field(value, "paneId"),
        pid,
    };
    let has_attestation = target.window_id.is_some()
        || target.tab_id.is_some()
        || target.pane_id.is_some()
        || target.pid.is_some();
    has_attestation.then_some(target)
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|field| field.as_str())
        .and_then(nonempty)
        .map(str::to_string)
}

fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sample_run_record;

    #[test]
    fn codex_targets_include_copy_workspace_terminal_and_deeplink() {
        let mut run = sample_run_record();
        run.tool = ToolKind::Codex;
        run.thread_id = Some("thread with spaces".into());
        run.workspace_path = "/Users/me/Repo Name".into();

        let targets = build_jump_targets(&run);

        assert!(targets.iter().any(|target| {
            target.kind == JumpTargetKind::CopyCommand
                && target.command.as_ref().is_some_and(|command| {
                    command
                        == &vec![
                            "codex".to_string(),
                            "resume".to_string(),
                            "thread with spaces".to_string(),
                        ]
                })
        }));
        assert!(targets.iter().any(|target| {
            target.kind == JumpTargetKind::SessionDeeplink
                && target.url.as_deref() == Some("codex://threads/thread%20with%20spaces")
        }));
        assert!(targets.iter().any(|target| {
            target.kind == JumpTargetKind::NewTerminalTab
                && target
                    .terminal
                    .as_ref()
                    .is_some_and(|terminal| terminal.provider == TerminalProvider::Warp)
                && target.url.as_deref()
                    == Some("warp://action/new_tab?path=%2FUsers%2Fme%2FRepo%20Name")
        }));
        assert!(!targets
            .iter()
            .any(|target| target.kind == JumpTargetKind::ManagedTerminalFocus));
    }

    #[test]
    fn unmanaged_run_does_not_get_focus_target() {
        let mut run = sample_run_record();
        run.tool_specific = Some(serde_json::json!({
            "managedTerminal": {
                "provider": "tmux"
            }
        }));

        let targets = build_jump_targets(&run);

        assert!(!targets
            .iter()
            .any(|target| target.kind == JumpTargetKind::ManagedTerminalFocus));
    }

    #[test]
    fn managed_terminal_requires_attested_identity() {
        let mut run = sample_run_record();
        run.tool_specific = Some(serde_json::json!({
            "managedTerminal": {
                "provider": "tmux",
                "paneId": "%1",
                "pid": 1234
            }
        }));

        let targets = build_jump_targets(&run);

        let focus = targets
            .iter()
            .find(|target| target.kind == JumpTargetKind::ManagedTerminalFocus)
            .expect("managed focus target");
        assert_eq!(
            focus
                .terminal
                .as_ref()
                .and_then(|target| target.pane_id.as_deref()),
            Some("%1")
        );
        assert!(focus.requires_confirmation);
    }
}
