use std::{
    path::Path,
    sync::{Mutex, MutexGuard, OnceLock},
};

use axum::{
    body::Body,
    http::{Request, Response},
};
use chrono::Utc;
use octomonitor_core::{
    Freshness, MoneyValue, QuotaValue, RunRecord, RunState, SourceConfidence, SourceInfo,
    TokenUsage, ToolKind,
};
use tempfile::TempDir;
use tower::util::ServiceExt;

use crate::{build_app, pricing::PricingStore, probe::empty_bootstrap, state::AppState};

pub(crate) fn sample_run_record() -> RunRecord {
    let now = Utc::now().to_rfc3339();
    RunRecord {
        id: "test-run".into(),
        tool: ToolKind::Codex,
        source_mode: "test_support".into(),
        project_name: "octomonitor".into(),
        workspace_path: "/tmp/octomonitor".into(),
        workspace_short: "~/octomonitor".into(),
        model: Some("gpt-5".into()),
        provider: Some("openai".into()),
        agent_name: None,
        agent_display_name: None,
        account_alias: None,
        auth_mode: None,
        auth_verified: false,
        session_id: None,
        thread_id: None,
        session_key: None,
        transcript_path: None,
        started_at: now.clone(),
        last_activity_at: now.clone(),
        elapsed_ms: 0,
        state: RunState::Active,
        last_action: None,
        last_tail: None,
        pending_approval: false,
        first_question: None,
        last_question: None,
        error_message: None,
        message_count: 0,
        tokens: TokenUsage {
            input: 0,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            total: 0,
            context: 0,
        },
        cost: MoneyValue {
            usd: None,
            confidence: SourceConfidence::Estimated,
        },
        quota: QuotaValue {
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            reset_at: vec![],
            confidence: SourceConfidence::Derived,
        },
        source: SourceInfo {
            confidence: SourceConfidence::Live,
            freshness: Freshness::Hot,
            last_updated_at: now,
        },
        vcs: None,
        origin_label: None,
        origin_provider: None,
    }
}

pub(crate) struct ServerTestHarness {
    _temp_dir: TempDir,
    _config_guard: ConfigDirGuard,
    pub state: AppState,
    app: axum::Router,
}

impl ServerTestHarness {
    /// Create a harness with `OCTOMONITOR_CONFIG_DIR` pointed at a private
    /// `TempDir`, so any `save_config` triggered by handlers stays sandboxed.
    pub(crate) fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let guard = ConfigDirGuard::set(temp_dir.path());
        Self::assemble(temp_dir, guard)
    }

    /// Use a caller-provided config dir (kept alive by the caller, e.g. a
    /// `TempDir` bound to the test scope). The harness still owns its own
    /// private `TempDir` as a placeholder so the field type stays uniform.
    /// Useful when a test wants `save_config` to fail by pointing at a path
    /// that is not a writable directory.
    pub(crate) fn with_config_dir(dir: &Path) -> Self {
        let placeholder = tempfile::tempdir().expect("temp dir");
        let guard = ConfigDirGuard::set(dir);
        Self::assemble(placeholder, guard)
    }

    fn assemble(temp_dir: TempDir, guard: ConfigDirGuard) -> Self {
        let pricing = PricingStore::new();
        let state = AppState::new(empty_bootstrap(), pricing);
        let app = build_app(state.clone());
        Self {
            _temp_dir: temp_dir,
            _config_guard: guard,
            state,
            app,
        }
    }

    pub(crate) async fn request(&self, request: Request<Body>) -> Response<Body> {
        self.app
            .clone()
            .oneshot(request)
            .await
            .expect("request should succeed")
    }
}

fn config_dir_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) struct ConfigDirGuard {
    _guard: MutexGuard<'static, ()>,
}

impl ConfigDirGuard {
    pub(crate) fn set(path: &Path) -> Self {
        // Tolerate `PoisonError`: a panic in another harness-using test should
        // not cascade-fail every subsequent test in the same process. The
        // lock's only payload is `()`, so recovering the inner guard is safe
        // — we still rebuild env state below.
        let guard = match config_dir_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        std::env::set_var("OCTOMONITOR_CONFIG_DIR", path);
        Self { _guard: guard }
    }
}

impl Drop for ConfigDirGuard {
    fn drop(&mut self) {
        std::env::remove_var("OCTOMONITOR_CONFIG_DIR");
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };

    use super::ServerTestHarness;

    #[tokio::test]
    async fn harness_can_serve_health_route() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            harness.state.bootstrap.read().await.config.listen_port,
            46321
        );
    }
}
