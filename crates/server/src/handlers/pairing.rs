use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use octomonitor_companion::{approve_pairing, request_pairing, PairingRecord};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    pub label: Option<String>,
}

pub async fn pair_request(
    State(state): State<AppState>,
    Json(input): Json<PairRequest>,
) -> Json<PairingRecord> {
    let record = request_pairing(input.label.as_deref());
    state.pairings.write().await.push(record.clone());
    Json(record)
}

pub async fn pair_approve(
    Path(token): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<PairingRecord>, StatusCode> {
    let mut pairings = state.pairings.write().await;
    if let Some(idx) = pairings.iter().position(|r| r.token == token) {
        let updated = approve_pairing(&pairings[idx]).ok_or(StatusCode::GONE)?;
        pairings[idx] = updated.clone();
        return Ok(Json(updated));
    }
    Err(StatusCode::NOT_FOUND)
}

pub async fn pair_revoke(
    Path(token): Path<String>,
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let mut pairings = state.pairings.write().await;
    pairings.retain(|p| p.token != token);
    Json(serde_json::json!({"revoked": token}))
}
