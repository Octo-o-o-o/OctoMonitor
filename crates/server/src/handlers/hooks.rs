use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    hooks::{
        apply_hook_transaction, build_hook_plan, list_hook_states, parse_tool_kind, HookAction,
        HookApplyRequest,
    },
    state::AppState,
};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookPlanQuery {
    pub action: HookAction,
}

fn api_error(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "ok": false,
            "error": message.into(),
        })),
    )
}

async fn disabled_sources(state: &AppState) -> Vec<octomonitor_core::ToolKind> {
    state.bootstrap.read().await.config.disabled_sources.clone()
}

pub async fn list_hooks(State(state): State<AppState>) -> Json<serde_json::Value> {
    let disabled_sources = disabled_sources(&state).await;
    Json(serde_json::json!({
        "hooks": list_hook_states(&disabled_sources),
    }))
}

pub async fn hook_plan(
    State(state): State<AppState>,
    Path(tool): Path<String>,
    Query(query): Query<HookPlanQuery>,
) -> ApiResult<serde_json::Value> {
    let tool =
        parse_tool_kind(&tool).ok_or_else(|| api_error(StatusCode::NOT_FOUND, "unknown tool"))?;
    let disabled_sources = disabled_sources(&state).await;
    let plan = build_hook_plan(tool, query.action, &disabled_sources);
    Ok(Json(serde_json::json!(plan)))
}

pub async fn apply_hook(
    State(state): State<AppState>,
    Path(tool): Path<String>,
    Json(request): Json<HookApplyRequest>,
) -> ApiResult<serde_json::Value> {
    let tool =
        parse_tool_kind(&tool).ok_or_else(|| api_error(StatusCode::NOT_FOUND, "unknown tool"))?;
    let disabled_sources = disabled_sources(&state).await;
    match apply_hook_transaction(tool, request, &disabled_sources) {
        Ok(result) => Ok(Json(serde_json::json!(result))),
        Err(err) => Err(api_error(StatusCode::CONFLICT, err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };

    use crate::test_support::ServerTestHarness;

    async fn fetch_json(harness: &ServerTestHarness, uri: &str) -> serde_json::Value {
        let response = harness
            .request(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        serde_json::from_slice(&body).expect("json")
    }

    #[tokio::test]
    async fn hooks_list_returns_supported_and_detection_only_rows() {
        let harness = ServerTestHarness::new();
        let payload = fetch_json(&harness, "/api/hooks").await;
        let hooks = payload
            .get("hooks")
            .and_then(|value| value.as_array())
            .expect("hooks array");

        assert!(hooks
            .iter()
            .any(|row| row["tool"] == "claude" && row["supported"] == true));
        assert!(hooks
            .iter()
            .any(|row| row["tool"] == "kiro" && row["supported"] == false));
    }

    #[tokio::test]
    async fn hook_plan_returns_redacted_diff_and_expected_hash() {
        let harness = ServerTestHarness::new();
        let payload = fetch_json(&harness, "/api/hooks/codex/plan?action=install").await;

        assert_eq!(payload["tool"], "codex");
        assert!(payload["beforeSha256"].as_str().is_some());
        let diff = payload["diff"].as_str().expect("diff");
        assert!(diff.contains("OctoMonitor managed block only"));
        assert!(!diff.contains("current hook command"));
    }
}
