use axum::Json;
use octomonitor_installer::{
    apply_install, detect_tools, doctor_report, install_plan, rollback_install,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequest {
    pub tool: String,
}

pub async fn installer_detect() -> Json<serde_json::Value> {
    Json(serde_json::json!({"capabilities": detect_tools()}))
}

pub async fn installer_doctor() -> Json<serde_json::Value> {
    Json(serde_json::json!({"checks": doctor_report()}))
}

pub async fn installer_install_plan(Json(input): Json<ToolRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"plan": install_plan(&input.tool)}))
}

pub async fn installer_install(Json(input): Json<ToolRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"result": apply_install(&input.tool)}))
}

pub async fn installer_rollback(Json(input): Json<ToolRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"result": rollback_install(&input.tool)}))
}
