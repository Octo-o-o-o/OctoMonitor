use axum::Json;
use octomonitor_installer::{detect_tools, doctor_report};

pub async fn installer_detect() -> Json<serde_json::Value> {
    Json(serde_json::json!({"capabilities": detect_tools()}))
}

pub async fn installer_doctor() -> Json<serde_json::Value> {
    Json(serde_json::json!({"checks": doctor_report()}))
}
