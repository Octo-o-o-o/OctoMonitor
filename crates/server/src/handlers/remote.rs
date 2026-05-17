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
        body::{self, Body},
        http::{header, Method, Request, StatusCode},
    };
    use octomonitor_companion::{request_pairing, ViewerSession};
    use tempfile::tempdir;

    use crate::test_support::ServerTestHarness;

    fn post_json(uri: &str, body_json: &str) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body_json.to_string()))
            .unwrap()
    }

    async fn enable_companion(harness: &ServerTestHarness) {
        let mut payload = harness.state.bootstrap.write().await;
        payload.config.companion_enabled = true;
    }

    #[tokio::test]
    async fn patch_remote_access_returns_500_and_keeps_state_when_save_fails() {
        let temp = tempdir().unwrap();
        let bad_parent = temp.path().join("not-a-directory");
        std::fs::write(&bad_parent, "occupied").unwrap();
        let harness = ServerTestHarness::with_config_dir(&bad_parent);
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

    #[tokio::test]
    async fn create_remote_pairing_rejects_when_companion_disabled() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(post_json("/api/remote/pairings", r#"{"label":"laptop"}"#))
            .await;
        // Should refuse with CONFLICT instead of silently issuing a code that
        // would be unusable until the operator enables remote access.
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(
            harness.state.pairings.read().await.is_empty(),
            "no pairing should land in state when companion is off"
        );
    }

    #[tokio::test]
    async fn create_remote_pairing_returns_code_and_records_when_enabled() {
        let harness = ServerTestHarness::new();
        enable_companion(&harness).await;

        let response = harness
            .request(post_json("/api/remote/pairings", r#"{"label":"laptop"}"#))
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(value.get("code").and_then(|v| v.as_str()).is_some());
        assert_eq!(value.get("label").and_then(|v| v.as_str()), Some("laptop"));

        assert_eq!(harness.state.pairings.read().await.len(), 1);
    }

    #[tokio::test]
    async fn list_remote_devices_reflects_viewer_sessions() {
        let harness = ServerTestHarness::new();
        enable_companion(&harness).await;
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
                expires_at: "2099-05-01T10:00:00Z".into(),
            });

        let response = harness
            .request(
                Request::builder()
                    .uri("/api/remote/devices")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let devices = value.as_array().expect("devices is an array");
        assert_eq!(devices.len(), 1);
        assert_eq!(
            devices[0].get("label").and_then(|v| v.as_str()),
            Some("Desk")
        );
    }

    #[tokio::test]
    async fn revoke_remote_device_returns_ok_even_when_device_is_unknown() {
        // The web client always issues DELETE optimistically when a row goes
        // away on screen; the handler should respond consistently rather than
        // 404 for an already-gone id.
        let harness = ServerTestHarness::new();
        enable_companion(&harness).await;
        let response = harness
            .request(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/remote/devices/never-existed")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value.get("revoked").and_then(|v| v.as_str()),
            Some("never-existed")
        );
    }
}
