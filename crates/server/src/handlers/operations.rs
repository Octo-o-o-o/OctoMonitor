use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    operations::{apply_run_operation, list_run_operations, OperationApplyRequest},
    state::AppState,
};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

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

async fn find_run(state: &AppState, run_id: &str) -> Option<octomonitor_core::RunRecord> {
    state
        .bootstrap
        .read()
        .await
        .runs
        .iter()
        .find(|run| run.id == run_id)
        .cloned()
}

pub async fn get_run_operations(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let run = find_run(&state, &run_id)
        .await
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "run not found"))?;
    Ok(Json(serde_json::json!(list_run_operations(&run))))
}

pub async fn apply_run_operation_handler(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(request): Json<OperationApplyRequest>,
) -> ApiResult<serde_json::Value> {
    let run = find_run(&state, &run_id)
        .await
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "run not found"))?;
    match apply_run_operation(&run, request) {
        Ok(result) if result.ok => Ok(Json(serde_json::json!(result))),
        Ok(result) => Err((StatusCode::CONFLICT, Json(serde_json::json!(result)))),
        Err(err) => Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use octomonitor_core::{
        AuditLevel, CapabilityDescriptor, CapabilityFailureMode, CapabilitySource, SchemaConfidence,
    };
    use serde_json::json;

    use crate::test_support::{sample_run_record, ServerTestHarness};

    fn cap(id: &str) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: id.into(),
            source: CapabilitySource::OfficialCli,
            confidence: SchemaConfidence::High,
            mutates_state: false,
            requires_user_confirmation: false,
            requires_managed_process: false,
            can_expose_secrets: false,
            audit_level: AuditLevel::Metadata,
            failure_mode: CapabilityFailureMode::Safe,
        }
    }

    async fn body_json(response: axum::http::Response<Body>) -> serde_json::Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        serde_json::from_slice(&body).expect("json")
    }

    #[tokio::test]
    async fn operations_list_returns_capability_rows() {
        let harness = ServerTestHarness::new();
        {
            let mut bootstrap = harness.state.bootstrap.write().await;
            let mut run = sample_run_record();
            run.id = "run-ops".into();
            run.capabilities = Some(vec![cap("resume.copyCommand"), cap("open.workspace")]);
            bootstrap.runs.push(run);
        }

        let response = harness
            .request(
                Request::builder()
                    .uri("/api/runs/run-ops/operations")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let payload = body_json(response).await;
        assert_eq!(payload["runId"], "run-ops");
        assert_eq!(payload["operations"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn stale_operation_request_returns_conflict() {
        let harness = ServerTestHarness::new();
        {
            let mut bootstrap = harness.state.bootstrap.write().await;
            let mut run = sample_run_record();
            run.id = "run-stale".into();
            run.thread_id = Some("thread-1".into());
            run.capabilities = Some(vec![cap("resume.copyCommand")]);
            bootstrap.runs.push(run);
        }

        let response = harness
            .request(
                Request::builder()
                    .method("POST")
                    .uri("/api/runs/run-stale/operations")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "action": "resume.copyCommand",
                            "expectedLastActivityAt": "stale"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let payload = body_json(response).await;
        assert_eq!(payload["ok"], false);
    }
}
