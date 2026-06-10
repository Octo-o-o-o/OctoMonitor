use axum::{extract::State, http::StatusCode, Json};
use octomonitor_core::{AppConfig, ToolKind};

use crate::config::{normalize_tool_list, save_config, ConfigPatch};
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
    let mut next_config = payload.config.clone();
    let previous_history_days = next_config.history_days;
    let previous_disabled_sources = next_config.disabled_sources.clone();
    if let Some(v) = patch.companion_enabled {
        next_config.companion_enabled = v;
    }
    if let Some(v) = patch.history_days {
        next_config.history_days = v.clamp(1, 180);
    }
    if let Some(v) = patch.disabled_sources {
        next_config.disabled_sources = normalize_tool_list(v);
    }
    if let Some(v) = patch.hidden_sources {
        next_config.hidden_sources = normalize_tool_list(v);
    }
    let history_changed = next_config.history_days != previous_history_days;
    let disabled_changed = next_config.disabled_sources != previous_disabled_sources;
    payload.config = next_config;
    if let Err(error) = save_config(&payload.config) {
        tracing::warn!("Failed to persist config patch: {error}");
        payload.config = previous_config;
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    if disabled_changed {
        let disabled = payload.config.disabled_sources.clone();
        payload.runs.retain(|run| !disabled.contains(&run.tool));
        payload
            .identities
            .retain(|identity| !disabled.contains(&identity.tool));
        payload
            .adapter_health
            .retain(|health| !disabled.contains(&health.tool));
        payload.pending_crons.retain(|cron| {
            if cron.id.starts_with("hermes-") {
                !disabled.contains(&ToolKind::Hermes)
            } else {
                !disabled.contains(&ToolKind::OpenClaw)
            }
        });
    }
    if history_changed || disabled_changed {
        rebuild_derived(&mut payload, &pricing);
    }
    let config = payload.config.clone();
    state.bump_revision();
    drop(payload);
    if history_changed || disabled_changed {
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

    use octomonitor_core::{
        Freshness, MoneyValue, QuotaValue, RunRecord, RunState, SourceConfidence, SourceInfo,
        TokenUsage, ToolKind,
    };

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

        let in_state = harness.state.bootstrap.read().await.config.history_days;
        assert_eq!(in_state, 180);
    }

    #[tokio::test]
    async fn patch_config_clamps_history_days_below_one_to_one() {
        let harness = ServerTestHarness::new();
        let response = harness.request(patch_body(r#"{"historyDays":0}"#)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let in_state = harness.state.bootstrap.read().await.config.history_days;
        assert_eq!(in_state, 1);
    }

    #[tokio::test]
    async fn patch_config_companion_enabled_toggle_leaves_history_days_untouched() {
        let harness = ServerTestHarness::new();
        let original_history = harness.state.bootstrap.read().await.config.history_days;
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

    fn sample_run(id: &str, tool: ToolKind) -> RunRecord {
        let now = chrono::Utc::now().to_rfc3339();
        RunRecord {
            id: id.into(),
            tool,
            source_mode: "test".into(),
            project_name: "OctoMonitor".into(),
            workspace_path: "/tmp/octomonitor".into(),
            workspace_short: "octomonitor".into(),
            model: None,
            provider: None,
            agent_name: None,
            agent_display_name: None,
            account_alias: None,
            auth_mode: None,
            auth_verified: false,
            session_id: Some(id.into()),
            thread_id: None,
            session_key: None,
            transcript_path: None,
            started_at: now.clone(),
            last_activity_at: now.clone(),
            elapsed_ms: 0,
            state: RunState::Completed,
            last_action: None,
            last_tail: None,
            pending_approval: false,
            first_question: None,
            last_question: None,
            error_message: None,
            message_count: 0,
            tokens: TokenUsage::default(),
            cost: MoneyValue {
                usd: None,
                confidence: SourceConfidence::Derived,
            },
            quota: QuotaValue {
                five_hour_used_pct: None,
                seven_day_used_pct: None,
                reset_at: Vec::new(),
                confidence: SourceConfidence::Derived,
            },
            source: SourceInfo {
                confidence: SourceConfidence::Derived,
                freshness: Freshness::Warm,
                last_updated_at: now,
            },
            source_id: None,
            lifecycle: None,
            usage_semantics: None,
            data_sources: None,
            capabilities: None,
            jump_targets: None,
            tool_specific: None,
            vcs: None,
            origin_label: None,
            origin_provider: None,
        }
    }

    #[tokio::test]
    async fn patch_config_disables_sources_and_prunes_runtime_state() {
        let harness = ServerTestHarness::new();
        {
            let mut payload = harness.state.bootstrap.write().await;
            payload.runs.push(sample_run("codex-run", ToolKind::Codex));
            payload
                .runs
                .push(sample_run("claude-run", ToolKind::Claude));
        }

        let response = harness
            .request(patch_body(
                r#"{"disabledSources":["codex"],"hiddenSources":["claude"]}"#,
            ))
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        let payload = harness.state.bootstrap.read().await;
        assert_eq!(payload.config.disabled_sources, vec![ToolKind::Codex]);
        assert_eq!(payload.config.hidden_sources, vec![ToolKind::Claude]);
        assert!(payload.runs.iter().all(|run| run.tool != ToolKind::Codex));
        assert!(payload.runs.iter().any(|run| run.tool == ToolKind::Claude));
    }

    #[tokio::test]
    async fn failed_source_control_patch_keeps_existing_runs() {
        let temp = tempdir().unwrap();
        let bad_parent = temp.path().join("not-a-directory");
        std::fs::write(&bad_parent, "occupied").unwrap();
        let harness = ServerTestHarness::with_config_dir(&bad_parent);
        {
            let mut payload = harness.state.bootstrap.write().await;
            payload.runs.push(sample_run("codex-run", ToolKind::Codex));
        }

        let response = harness
            .request(patch_body(r#"{"disabledSources":["codex"]}"#))
            .await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let payload = harness.state.bootstrap.read().await;
        assert!(payload.config.disabled_sources.is_empty());
        assert!(
            payload.runs.iter().any(|run| run.tool == ToolKind::Codex),
            "failed config persistence must not prune in-memory runs"
        );
    }
}
