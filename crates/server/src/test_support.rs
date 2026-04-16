use std::{
    path::Path,
    sync::{Mutex, MutexGuard, OnceLock},
};

use axum::{
    body::Body,
    http::{Request, Response},
};
use tempfile::TempDir;
use tower::util::ServiceExt;

use crate::{
    build_app,
    pricing::PricingStore,
    probe::empty_bootstrap,
    state::AppState,
};

pub(crate) struct ServerTestHarness {
    _temp_dir: TempDir,
    pub state: AppState,
    app: axum::Router,
}

impl ServerTestHarness {
    pub(crate) fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let pricing = PricingStore::new();
        let state = AppState::new(empty_bootstrap(), pricing);
        let app = build_app(state.clone());
        Self {
            _temp_dir: temp_dir,
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
        let guard = config_dir_lock().lock().expect("config dir lock");
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
