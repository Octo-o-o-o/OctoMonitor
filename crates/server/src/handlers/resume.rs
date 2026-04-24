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
    match run.tool {
        ToolKind::Codex => match run.thread_id.as_deref() {
            Some(thread_id) if !thread_id.is_empty() => ResumeCommandPayload {
                command: Some(format!("codex resume {thread_id}")),
                tool: ToolKind::Codex,
                note: None,
            },
            _ => ResumeCommandPayload {
                command: None,
                tool: ToolKind::Codex,
                note: Some("Codex session is missing a thread id".to_string()),
            },
        },
        ToolKind::Claude => ResumeCommandPayload {
            command: None,
            tool: ToolKind::Claude,
            note: Some("Resume command is not yet available for Claude".to_string()),
        },
        ToolKind::OpenClaw => ResumeCommandPayload {
            command: None,
            tool: ToolKind::OpenClaw,
            note: Some("Resume command is not yet available for OpenClaw".to_string()),
        },
        ToolKind::Hermes => ResumeCommandPayload {
            command: None,
            tool: ToolKind::Hermes,
            note: Some("Resume command is not yet available for Hermes".to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{build_resume_command, ResumeCommandPayload};
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
    fn claude_returns_unavailable() {
        let mut run = sample_run_record();
        run.tool = ToolKind::Claude;
        run.thread_id = Some("anything".into());
        let out = build_resume_command(&run);
        assert!(out.command.is_none());
        assert_eq!(out.tool, ToolKind::Claude);
        assert!(out.note.is_some());
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
