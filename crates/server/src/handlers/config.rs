use axum::{extract::State, Json};
use octomonitor_core::AppConfig;

use crate::config::{save_config, ConfigPatch};
use crate::state::AppState;

pub async fn get_config(State(state): State<AppState>) -> Json<AppConfig> {
    Json(state.bootstrap.read().await.config.clone())
}

pub async fn patch_config(
    State(state): State<AppState>,
    Json(patch): Json<ConfigPatch>,
) -> Json<AppConfig> {
    let mut payload = state.bootstrap.write().await;
    if let Some(v) = patch.companion_enabled {
        payload.config.companion_enabled = v;
    }
    save_config(&payload.config);
    let config = payload.config.clone();
    drop(payload);
    state.signal_change();
    Json(config)
}
