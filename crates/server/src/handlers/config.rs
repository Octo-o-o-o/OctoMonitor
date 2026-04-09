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
        body::Body,
        http::{header, Method, Request, StatusCode},
    };
    use tempfile::tempdir;

    use crate::test_support::{ConfigDirGuard, ServerTestHarness};

    #[tokio::test]
    async fn patch_config_returns_500_and_reverts_when_save_fails() {
        let harness = ServerTestHarness::new();
        let original = harness.state.bootstrap.read().await.config.clone();
        let temp = tempdir().unwrap();
        let bad_parent = temp.path().join("not-a-directory");
        std::fs::write(&bad_parent, "occupied").unwrap();
        let _guard = ConfigDirGuard::set(&bad_parent);

        let response = harness
            .request(
                Request::builder()
                    .method(Method::PATCH)
                    .uri("/api/config")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"historyDays":90,"companionEnabled":true}"#))
                    .unwrap(),
            )
            .await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let config = harness.state.bootstrap.read().await.config.clone();
        assert_eq!(config.history_days, original.history_days);
        assert_eq!(config.companion_enabled, original.companion_enabled);
    }
}
