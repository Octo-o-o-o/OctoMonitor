use octomonitor_core::AppConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    pub version: Option<u8>,
    pub companion_enabled: Option<bool>,
    pub history_days: Option<u8>,
}

pub fn config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home)
        .join(".octomonitor")
        .join("config.json")
}

pub fn load_config() -> Option<ConfigPatch> {
    let path = config_path();
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_config(config: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let patch = ConfigPatch {
        version: Some(2),
        companion_enabled: Some(config.companion_enabled),
        history_days: Some(config.history_days),
    };
    match serde_json::to_string_pretty(&patch) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                tracing::warn!("Failed to save config to {}: {e}", path.display());
            }
        }
        Err(e) => tracing::warn!("Failed to serialize config: {e}"),
    }
}
