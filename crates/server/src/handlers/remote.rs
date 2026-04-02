use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use octomonitor_companion::request_pairing;
use octomonitor_core::{RemoteAccessState, RemoteDevice, RemotePairingCode};
use serde::Deserialize;

use crate::{config::save_config, remote_access::build_remote_access_state, state::AppState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAccessPatch {
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePairingRequest {
    pub label: Option<String>,
}

pub async fn get_remote_access(State(state): State<AppState>) -> Json<RemoteAccessState> {
    Json(build_remote_access_state(&state).await)
}

pub async fn patch_remote_access(
    State(state): State<AppState>,
    Json(patch): Json<RemoteAccessPatch>,
) -> Json<RemoteAccessState> {
    if let Some(enabled) = patch.enabled {
        let config = {
            let mut payload = state.bootstrap.write().await;
            payload.config.companion_enabled = enabled;
            payload.config.clone()
        };
        save_config(&config);

        if !enabled {
            state.clear_remote_access_state().await;
        }
    }

    state.signal_change();
    Json(build_remote_access_state(&state).await)
}

pub async fn list_remote_devices(State(state): State<AppState>) -> Json<Vec<RemoteDevice>> {
    Json(build_remote_access_state(&state).await.devices)
}

pub async fn create_remote_pairing(
    State(state): State<AppState>,
    Json(input): Json<RemotePairingRequest>,
) -> Result<Json<RemotePairingCode>, StatusCode> {
    if !state.bootstrap.read().await.config.companion_enabled {
        return Err(StatusCode::CONFLICT);
    }

    let record = request_pairing(input.label.as_deref());
    let pairing = RemotePairingCode {
        id: record.id.clone(),
        code: record.code.clone(),
        label: record.label.clone(),
        expires_at: record.expires_at.clone(),
    };
    state.pairings.write().await.push(record);
    state.signal_change();

    Ok(Json(pairing))
}

pub async fn revoke_remote_device(
    Path(device_id): Path<String>,
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let revoked = state.revoke_remote_device(&device_id).await;
    if revoked {
        state.signal_change();
    }
    Json(serde_json::json!({ "revoked": device_id }))
}
