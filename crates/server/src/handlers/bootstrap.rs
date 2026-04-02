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
