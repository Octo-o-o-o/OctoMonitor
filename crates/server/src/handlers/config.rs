use axum::{extract::State, Json};
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
) -> Json<AppConfig> {
    let pricing = state.pricing.clone();
    let mut payload = state.bootstrap.write().await;
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
    save_config(&payload.config);
    let config = payload.config.clone();
    drop(payload);
    if history_changed {
        state.wake_probe();
    }
    state.signal_change();
    Json(config)
}
