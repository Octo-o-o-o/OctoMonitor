use axum::Json;
use octomonitor_installer::{detect_tools, doctor_report};

pub async fn installer_detect() -> Json<serde_json::Value> {
    Json(serde_json::json!({"capabilities": detect_tools()}))
}

pub async fn installer_doctor() -> Json<serde_json::Value> {
    Json(serde_json::json!({"checks": doctor_report()}))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{self, Body},
        http::{Request, StatusCode},
    };

    use crate::test_support::ServerTestHarness;

    async fn fetch_json(harness: &ServerTestHarness, uri: &str) -> serde_json::Value {
        let response = harness
            .request(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK, "GET {uri} expected 200");
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        serde_json::from_slice(&bytes).expect("body should be JSON")
    }

    #[tokio::test]
    async fn installer_detect_returns_capabilities_envelope() {
        let harness = ServerTestHarness::new();
        let payload = fetch_json(&harness, "/api/installer/detect").await;
        let capabilities = payload
            .get("capabilities")
            .and_then(|v| v.as_array())
            .expect("capabilities array present");
        // Four well-known tools are always reported (detection result varies
        // per host, but the envelope shape is fixed by the installer crate).
        assert_eq!(capabilities.len(), 4);
    }

    #[tokio::test]
    async fn installer_doctor_returns_checks_envelope() {
        let harness = ServerTestHarness::new();
        let payload = fetch_json(&harness, "/api/installer/doctor").await;
        assert!(
            payload.get("checks").and_then(|v| v.as_array()).is_some(),
            "doctor payload should expose `checks` array: {payload}"
        );
    }
}
