use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use octomonitor_core::{RunRecord, ToolKind};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeCommandPayload {
    pub command: Option<String>,
    pub tool: ToolKind,
    pub note: Option<String>,
}

pub async fn get_run_resume_command(
    AxumPath(run_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ResumeCommandPayload>, StatusCode> {
    let run = state
        .bootstrap
        .read()
        .await
        .runs
        .iter()
        .find(|item| item.id == run_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(build_resume_command(&run)))
}

fn build_resume_command(run: &RunRecord) -> ResumeCommandPayload {
    let tool = run.tool.clone();
    let note = |msg: &str| ResumeCommandPayload {
        command: None,
        tool: tool.clone(),
        note: Some(msg.to_string()),
    };

    match run.tool {
        ToolKind::Codex => match run.thread_id.as_deref() {
            Some(thread_id) if !thread_id.is_empty() => ResumeCommandPayload {
                command: Some(format!("codex resume {}", shell_quote(thread_id))),
                tool,
                note: None,
            },
            _ => note("Codex session is missing a thread id"),
        },
        ToolKind::Claude => match run.session_id.as_deref() {
            Some(session_id) if !session_id.is_empty() => ResumeCommandPayload {
                command: Some(format!("claude --resume {}", shell_quote(session_id))),
                tool,
                note: None,
            },
            _ => note("Claude session is missing a session id"),
        },
        ToolKind::OpenClaw => note("Resume command is not yet available for OpenClaw"),
        ToolKind::Hermes => match run.session_id.as_deref() {
            Some(session_id) if !session_id.is_empty() => {
                let profile = run.agent_name.as_deref().filter(|value| {
                    !value.is_empty() && *value != "default" && *value != "local-probe"
                });
                let command = match profile {
                    Some(profile) => format!(
                        "hermes -p {} --resume {}",
                        shell_quote(profile),
                        shell_quote(session_id)
                    ),
                    None => format!("hermes --resume {}", shell_quote(session_id)),
                };
                ResumeCommandPayload {
                    command: Some(command),
                    tool,
                    note: None,
                }
            }
            _ => note("Hermes session is missing a session id"),
        },
    }
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

#[cfg(test)]
mod tests {
    use super::{build_resume_command, shell_quote, ResumeCommandPayload};
    use crate::test_support::{sample_run_record, ServerTestHarness};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use octomonitor_core::ToolKind;

    fn codex_run_with_thread(thread_id: Option<&str>) -> octomonitor_core::RunRecord {
        let mut run = sample_run_record();
        run.tool = ToolKind::Codex;
        run.thread_id = thread_id.map(String::from);
        run
    }

    #[test]
    fn codex_with_thread_id_returns_resume_command() {
        let run = codex_run_with_thread(Some("12345678-abcd"));
        let out = build_resume_command(&run);
        assert_eq!(
            out,
            ResumeCommandPayload {
                command: Some("codex resume 12345678-abcd".to_string()),
                tool: ToolKind::Codex,
                note: None,
            }
        );
    }

    #[test]
    fn codex_without_thread_id_returns_null_with_note() {
        let run = codex_run_with_thread(None);
        let out = build_resume_command(&run);
        assert!(out.command.is_none());
        assert_eq!(out.tool, ToolKind::Codex);
        assert!(out.note.is_some());
    }

    #[test]
    fn codex_with_empty_thread_id_returns_null() {
        let run = codex_run_with_thread(Some(""));
        let out = build_resume_command(&run);
        assert!(out.command.is_none());
        assert!(out.note.is_some());
    }

    #[test]
    fn claude_with_session_id_returns_resume_command() {
        let mut run = sample_run_record();
        run.tool = ToolKind::Claude;
        run.session_id = Some("claude-session-1".into());
        let out = build_resume_command(&run);
        assert_eq!(
            out,
            ResumeCommandPayload {
                command: Some("claude --resume claude-session-1".to_string()),
                tool: ToolKind::Claude,
                note: None,
            }
        );
    }

    #[test]
    fn claude_without_session_id_returns_unavailable() {
        let mut run = sample_run_record();
        run.tool = ToolKind::Claude;
        run.session_id = None;
        let out = build_resume_command(&run);
        assert!(out.command.is_none());
        assert_eq!(out.tool, ToolKind::Claude);
        assert!(out.note.is_some());
    }

    #[test]
    fn hermes_with_profile_returns_resume_command() {
        let mut run = sample_run_record();
        run.tool = ToolKind::Hermes;
        run.session_id = Some("hermes-session-1".into());
        run.agent_name = Some("research".into());
        let out = build_resume_command(&run);
        assert_eq!(
            out,
            ResumeCommandPayload {
                command: Some("hermes -p research --resume hermes-session-1".to_string()),
                tool: ToolKind::Hermes,
                note: None,
            }
        );
    }

    #[test]
    fn shell_quote_handles_spaces_and_quotes() {
        assert_eq!(shell_quote("abc-123"), "abc-123");
        assert_eq!(shell_quote("a b'c"), "'a b'\\''c'");
    }

    #[test]
    fn openclaw_returns_unavailable() {
        let mut run = sample_run_record();
        run.tool = ToolKind::OpenClaw;
        let out = build_resume_command(&run);
        assert!(out.command.is_none());
        assert_eq!(out.tool, ToolKind::OpenClaw);
        assert!(out.note.is_some());
    }

    #[tokio::test]
    async fn resume_command_returns_404_for_missing_run() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/runs/does-not-exist/resume-command")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn resume_command_returns_200_for_existing_codex_run() {
        let harness = ServerTestHarness::new();
        {
            let mut bootstrap = harness.state.bootstrap.write().await;
            let mut run = sample_run_record();
            run.id = "run-1".into();
            run.tool = ToolKind::Codex;
            run.thread_id = Some("uuid-123".into());
            bootstrap.runs.push(run);
        }
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/runs/run-1/resume-command")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
