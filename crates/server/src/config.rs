use octomonitor_core::AppConfig;
use serde::{Deserialize, Serialize};

use crate::platform::home_relative_path;

const CONFIG_DIR_OVERRIDE_ENV: &str = "OCTOMONITOR_CONFIG_DIR";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    pub version: Option<u8>,
    pub companion_enabled: Option<bool>,
    pub history_days: Option<u8>,
}

pub fn config_path() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os(CONFIG_DIR_OVERRIDE_ENV) {
        return std::path::PathBuf::from(path).join("config.json");
    }
    home_relative_path(".octomonitor").join("config.json")
}

pub fn load_config() -> Option<ConfigPatch> {
    let path = config_path();
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_config(config: &AppConfig) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let patch = ConfigPatch {
        version: Some(2),
        companion_enabled: Some(config.companion_enabled),
        history_days: Some(config.history_days),
    };
    let json = serde_json::to_string_pretty(&patch).map_err(std::io::Error::other)?;
    std::fs::write(&path, json)
}
