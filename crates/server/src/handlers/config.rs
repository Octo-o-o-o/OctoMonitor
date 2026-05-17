use axum::{extract::State, http::StatusCode, Json};
use octomonitor_core::AppConfig;

use crate::config::{save_config, ConfigPatch};
use crate::probe::rebuild_derived;
use crate::state::AppState;

pub async fn get_config(State(state): State<AppState>) -> Json<AppConfig> {
    Json(state.bootstrap.read().await.config.clone())
}

pub async fn patch_config(
    State(state): State<AppState>,
    Json(patch): Json<ConfigPatch>,
) -> Result<Json<AppConfig>, StatusCode> {
    let pricing = state.pricing.clone();
    let mut payload = state.bootstrap.write().await;
    let previous_config = payload.config.clone();
    let previous_history_days = payload.config.history_days;
    if let Some(v) = patch.companion_enabled {
        payload.config.companion_enabled = v;
    }
    if let Some(v) = patch.history_days {
        payload.config.history_days = v.clamp(1, 180);
    }
    let history_changed = payload.config.history_days != previous_history_days;
    if history_changed {
        rebuild_derived(&mut payload, &pricing);
    }
    if let Err(error) = save_config(&payload.config) {
        tracing::warn!("Failed to persist config patch: {error}");
        payload.config = previous_config;
        if history_changed {
            rebuild_derived(&mut payload, &pricing);
        }
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let config = payload.config.clone();
    state.bump_revision();
    drop(payload);
    if history_changed {
        state.wake_probe_with_reason("config_patch");
    }
    state.signal_change();
    Ok(Json(config))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{self, Body},
        http::{header, Method, Request, StatusCode},
    };
    use tempfile::tempdir;

    use crate::test_support::ServerTestHarness;

    fn patch_body(body_json: &str) -> Request<Body> {
        Request::builder()
            .method(Method::PATCH)
            .uri("/api/config")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body_json.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn patch_config_returns_500_and_reverts_when_save_fails() {
        let temp = tempdir().unwrap();
        let bad_parent = temp.path().join("not-a-directory");
        std::fs::write(&bad_parent, "occupied").unwrap();
        let harness = ServerTestHarness::with_config_dir(&bad_parent);
        let original = harness.state.bootstrap.read().await.config.clone();

        let response = harness
            .request(patch_body(r#"{"historyDays":90,"companionEnabled":true}"#))
            .await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let config = harness.state.bootstrap.read().await.config.clone();
        assert_eq!(config.history_days, original.history_days);
        assert_eq!(config.companion_enabled, original.companion_enabled);
    }

    #[tokio::test]
    async fn patch_config_clamps_history_days_to_180_and_persists() {
        // `historyDays` is typed `u8`, so the serde layer already rejects
        // anything above 255 (returns 422). The handler's own
        // `clamp(1, 180)` only kicks in for values in (180, 255]; pick 200.
        let harness = ServerTestHarness::new();
        let response = harness.request(patch_body(r#"{"historyDays":200}"#)).await;
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value.get("historyDays").and_then(|v| v.as_u64()), Some(180));

        let in_state = harness
            .state
            .bootstrap
            .read()
            .await
            .config
            .history_days;
        assert_eq!(in_state, 180);
    }

    #[tokio::test]
    async fn patch_config_clamps_history_days_below_one_to_one() {
        let harness = ServerTestHarness::new();
        let response = harness.request(patch_body(r#"{"historyDays":0}"#)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let in_state = harness
            .state
            .bootstrap
            .read()
            .await
            .config
            .history_days;
        assert_eq!(in_state, 1);
    }

    #[tokio::test]
    async fn patch_config_companion_enabled_toggle_leaves_history_days_untouched() {
        let harness = ServerTestHarness::new();
        let original_history = harness
            .state
            .bootstrap
            .read()
            .await
            .config
            .history_days;
        let original_revision = harness.state.current_revision();

        let response = harness
            .request(patch_body(r#"{"companionEnabled":true}"#))
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        let after = harness.state.bootstrap.read().await.config.clone();
        assert!(after.companion_enabled);
        assert_eq!(after.history_days, original_history);
        assert!(
            harness.state.current_revision() > original_revision,
            "config patch should bump snapshot revision so clients can detect change"
        );
    }
}
