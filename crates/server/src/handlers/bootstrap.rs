use axum::{extract::State, Json};
use octomonitor_core::BootstrapPayload;
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: &'static str,
    pub uptime_hint: &'static str,
}

pub async fn get_bootstrap(State(state): State<AppState>) -> Json<BootstrapPayload> {
    Json(state.bootstrap.read().await.clone())
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        uptime_hint: "local-first probe+ingest server",
    })
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{self, Body},
        http::{Request, StatusCode},
    };

    use crate::test_support::ServerTestHarness;

    #[tokio::test]
    async fn get_bootstrap_returns_snapshot_with_expected_shape() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/bootstrap")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let payload: serde_json::Value =
            serde_json::from_slice(&bytes).expect("bootstrap json should parse");
        // Smoke: route is wired up and the JSON shape carries the headline
        // collections the web client relies on.
        for key in [
            "generatedAt",
            "runs",
            "attentions",
            "usageBuckets",
            "commits",
            "config",
        ] {
            assert!(
                payload.get(key).is_some(),
                "bootstrap payload missing `{key}`: {payload}"
            );
        }
        assert_eq!(
            payload
                .get("config")
                .and_then(|c| c.get("listenPort"))
                .and_then(|p| p.as_u64()),
            Some(46321)
        );
    }
}
