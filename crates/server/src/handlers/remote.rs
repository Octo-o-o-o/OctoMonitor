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
) -> Result<Json<RemoteAccessState>, StatusCode> {
    if let Some(enabled) = patch.enabled {
        let mut payload = state.bootstrap.write().await;
        let previous_enabled = payload.config.companion_enabled;
        payload.config.companion_enabled = enabled;
        if let Err(error) = save_config(&payload.config) {
            tracing::warn!("Failed to persist remote access patch: {error}");
            payload.config.companion_enabled = previous_enabled;
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        state.bump_revision();
        drop(payload);

        if !enabled {
            state.clear_remote_access_state().await;
        }
    }

    state.signal_change();
    Ok(Json(build_remote_access_state(&state).await))
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

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
    };
    use octomonitor_companion::{request_pairing, ViewerSession};
    use tempfile::tempdir;

    use crate::test_support::{ConfigDirGuard, ServerTestHarness};

    #[tokio::test]
    async fn patch_remote_access_returns_500_and_keeps_state_when_save_fails() {
        let harness = ServerTestHarness::new();
        {
            let mut payload = harness.state.bootstrap.write().await;
            payload.config.companion_enabled = true;
        }
        harness
            .state
            .pairings
            .write()
            .await
            .push(request_pairing(Some("Desk")));
        harness
            .state
            .viewer_sessions
            .write()
            .await
            .push(ViewerSession {
                id: "viewer-1".into(),
                secret: "secret-1".into(),
                label: "Desk".into(),
                paired_at: "2026-04-01T10:00:00Z".into(),
                last_seen_at: Some("2026-04-01T10:05:00Z".into()),
                expires_at: "2026-05-01T10:00:00Z".into(),
            });

        let temp = tempdir().unwrap();
        let bad_parent = temp.path().join("not-a-directory");
        std::fs::write(&bad_parent, "occupied").unwrap();
        let _guard = ConfigDirGuard::set(&bad_parent);

        let response = harness
            .request(
                Request::builder()
                    .method(Method::PATCH)
                    .uri("/api/remote/access")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"enabled":false}"#))
                    .unwrap(),
            )
            .await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            harness
                .state
                .bootstrap
                .read()
                .await
                .config
                .companion_enabled
        );
        assert_eq!(harness.state.pairings.read().await.len(), 1);
        assert_eq!(harness.state.viewer_sessions.read().await.len(), 1);
    }
}
