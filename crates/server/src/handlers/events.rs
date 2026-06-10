use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use octomonitor_codex_adapter::{
    self as codex_adapter, CodexEvent, DEFAULT_MAX_EVENTS, DEFAULT_TAIL_BYTE_LIMIT,
};
use octomonitor_core::{RunRecord, ToolKind};
use serde::{Deserialize, Serialize};

use crate::handlers::inspect::resolve_transcript_path;
use crate::state::AppState;

const DEFAULT_LIMIT: usize = 120;
const MAX_LIMIT: usize = 300;

#[derive(Debug, Clone, Deserialize)]
pub struct EventsQuery {
    #[serde(default)]
    pub cursor: Option<u64>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsPayload {
    pub tool: ToolKind,
    pub events: Vec<CodexEvent>,
    pub cursor: u64,
    pub reset: bool,
}

pub async fn get_run_events(
    AxumPath(run_id): AxumPath<String>,
    Query(params): Query<EventsQuery>,
    State(state): State<AppState>,
) -> Result<Json<EventsPayload>, StatusCode> {
    let run = state
        .bootstrap
        .read()
        .await
        .runs
        .iter()
        .find(|item| item.id == run_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    if run.tool != ToolKind::Codex {
        return Ok(Json(EventsPayload {
            tool: run.tool,
            events: Vec::new(),
            cursor: 0,
            reset: false,
        }));
    }

    Ok(Json(read_codex_events(&run, params)))
}

fn read_codex_events(run: &RunRecord, params: EventsQuery) -> EventsPayload {
    let limit = params
        .limit
        .map(|n| n.clamp(1, MAX_LIMIT))
        .unwrap_or(DEFAULT_LIMIT);

    let empty = || EventsPayload {
        tool: run.tool,
        events: Vec::new(),
        cursor: params.cursor.unwrap_or(0),
        reset: false,
    };

    let Some(path) = resolve_transcript_path(run) else {
        return empty();
    };

    match codex_adapter::read_tail_events(&path, params.cursor, DEFAULT_TAIL_BYTE_LIMIT, limit) {
        Ok(tail) => EventsPayload {
            tool: run.tool,
            events: codex_adapter::dedupe_adjacent(tail.events),
            cursor: tail.cursor,
            reset: tail.reset,
        },
        Err(_) => empty(),
    }
}

#[allow(dead_code)]
pub(crate) const TEST_DEFAULT_LIMIT: usize = DEFAULT_LIMIT;
#[allow(dead_code)]
pub(crate) const TEST_MAX_LIMIT: usize = MAX_LIMIT;
#[allow(dead_code)]
pub(crate) const TEST_DEFAULT_MAX_EVENTS: usize = DEFAULT_MAX_EVENTS;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{sample_run_record, ServerTestHarness};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };

    #[tokio::test]
    async fn events_returns_404_for_missing_run() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/runs/does-not-exist/events")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn events_returns_empty_for_non_codex_run() {
        let harness = ServerTestHarness::new();
        {
            let mut bootstrap = harness.state.bootstrap.write().await;
            let mut run = sample_run_record();
            run.id = "claude-run-1".into();
            run.tool = ToolKind::Claude;
            bootstrap.runs.push(run);
        }
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/runs/claude-run-1/events")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn events_returns_200_for_codex_run_without_transcript() {
        // Codex run with no transcript_path and no discoverable file —
        // should still return OK with empty events, not 500.
        let harness = ServerTestHarness::new();
        {
            let mut bootstrap = harness.state.bootstrap.write().await;
            let mut run = sample_run_record();
            run.id = "codex-run-missing-transcript".into();
            run.tool = ToolKind::Codex;
            run.transcript_path = None;
            run.thread_id = None;
            bootstrap.runs.push(run);
        }
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/runs/codex-run-missing-transcript/events")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn clamp_limits_large_requests() {
        let mut run = sample_run_record();
        run.tool = ToolKind::Codex;
        run.transcript_path = None;
        let q = EventsQuery {
            cursor: None,
            limit: Some(9999),
        };
        let payload = read_codex_events(&run, q);
        // No transcript ⇒ empty, but no panic. Clamp is exercised before file read.
        assert_eq!(payload.events.len(), 0);
        assert_eq!(payload.tool, ToolKind::Codex);
    }

    // These const assertions are checked at compile time; they would otherwise
    // trip clippy::assertions_on_constants in a runtime `#[test]`.
    const _: () = assert!(TEST_DEFAULT_LIMIT <= TEST_MAX_LIMIT);
    const _: () = assert!(TEST_MAX_LIMIT <= TEST_DEFAULT_MAX_EVENTS);
}
