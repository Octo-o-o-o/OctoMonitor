use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub prompt: String,
    pub mode: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResponse {
    pub text: String,
}

/// Maximum time to wait for the CLI subprocess before killing it.
const GENERATE_TIMEOUT: Duration = Duration::from_secs(120);

pub async fn generate_daily_summary(
    Json(input): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, StatusCode> {
    let (cmd, args) = match input.mode.as_str() {
        "claude" => (
            "claude",
            vec![
                "-p".to_string(),
                input.prompt.clone(),
                "--output-format".to_string(),
                "text".to_string(),
                "--max-budget-usd".to_string(),
                "0.50".to_string(),
            ],
        ),
        "codex" => (
            "codex",
            vec![
                "exec".to_string(),
                input.prompt.clone(),
                "-a".to_string(),
                "never".to_string(),
            ],
        ),
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let result = tokio::time::timeout(
        GENERATE_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(cmd)
                .args(&args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
        }),
    )
    .await
    .map_err(|_| {
        tracing::warn!(
            "daily summary generation timed out after {:?}",
            GENERATE_TIMEOUT
        );
        StatusCode::GATEWAY_TIMEOUT
    })?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        tracing::warn!("daily summary generation failed ({}): {}", cmd, stderr);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let text = String::from_utf8_lossy(&result.stdout).trim().to_string();
    Ok(Json(GenerateResponse { text }))
}
