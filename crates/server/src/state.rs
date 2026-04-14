use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex as StdMutex,
};
use tokio::sync::{broadcast, Mutex, Notify, RwLock};

use octomonitor_companion::{PairingRecord, ViewerSession};
use octomonitor_core::BootstrapPayload;

use crate::perf;
use crate::pricing::PricingStore;
use crate::workflows::coordinator::WorkflowCoordinator;

#[derive(Clone)]
pub struct AppState {
    pub bootstrap: Arc<RwLock<BootstrapPayload>>,
    pub pairings: Arc<RwLock<Vec<PairingRecord>>>,
    pub viewer_sessions: Arc<RwLock<Vec<ViewerSession>>>,
    pub notify: broadcast::Sender<()>,
    pub probe_wake: Arc<Notify>,
    pub derive_wake: Arc<Notify>,
    pub derive_dirty: Arc<AtomicBool>,
    pub revision: Arc<AtomicU64>,
    pub claude_probe_cache: Arc<StdMutex<octomonitor_claude_adapter::ClaudeProbeCache>>,
    pub codex_probe_cache: Arc<StdMutex<octomonitor_codex_adapter::CodexProbeCache>>,
    pub openclaw_probe_cache: Arc<StdMutex<octomonitor_openclaw_adapter::OpenClawProbeCache>>,
    pub hermes_probe_cache: Arc<StdMutex<octomonitor_hermes_adapter::HermesProbeCache>>,
    pub pricing: PricingStore,
    pub workflow_coordinator: Arc<Mutex<WorkflowCoordinator>>,
}

impl AppState {
    pub fn new(
        initial: BootstrapPayload,
        pricing: PricingStore,
        workflow_coordinator: WorkflowCoordinator,
    ) -> Self {
        let (notify, _) = broadcast::channel(16);
        Self {
            bootstrap: Arc::new(RwLock::new(initial)),
            pairings: Arc::new(RwLock::new(Vec::new())),
            viewer_sessions: Arc::new(RwLock::new(Vec::new())),
            notify,
            probe_wake: Arc::new(Notify::new()),
            derive_wake: Arc::new(Notify::new()),
            derive_dirty: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
            claude_probe_cache: Arc::new(StdMutex::new(
                octomonitor_claude_adapter::ClaudeProbeCache::default(),
            )),
            codex_probe_cache: Arc::new(StdMutex::new(
                octomonitor_codex_adapter::CodexProbeCache::default(),
            )),
            openclaw_probe_cache: Arc::new(StdMutex::new(
                octomonitor_openclaw_adapter::OpenClawProbeCache::default(),
            )),
            hermes_probe_cache: Arc::new(StdMutex::new(
                octomonitor_hermes_adapter::HermesProbeCache::default(),
            )),
            pricing,
            workflow_coordinator: Arc::new(Mutex::new(workflow_coordinator)),
        }
    }

    pub fn signal_change(&self) {
        let _ = self.notify.send(());
    }

    pub fn wake_probe_with_reason(&self, reason: &'static str) {
        perf::log_probe_wake(reason);
        self.probe_wake.notify_one();
    }

    pub fn current_revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub fn bump_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub async fn snapshot_bootstrap(&self) -> (u64, BootstrapPayload) {
        let payload = self.bootstrap.read().await;
        let revision = self.current_revision();
        (revision, payload.clone())
    }

    pub fn mark_derive_dirty(&self) {
        self.derive_dirty.store(true, Ordering::SeqCst);
        self.derive_wake.notify_one();
    }

    pub fn take_derive_dirty(&self) -> bool {
        self.derive_dirty.swap(false, Ordering::SeqCst)
    }

    pub async fn clear_remote_access_state(&self) {
        self.pairings.write().await.clear();
        self.viewer_sessions.write().await.clear();
    }

    pub async fn revoke_remote_device(&self, device_id: &str) -> bool {
        let mut sessions = self.viewer_sessions.write().await;
        let original_len = sessions.len();
        sessions.retain(|session| session.id != device_id);
        sessions.len() != original_len
    }
}

#[cfg(test)]
mod tests {
    use octomonitor_companion::{request_pairing, ViewerSession};

    use super::*;
    use crate::{
        pricing::PricingStore,
        probe::build_bootstrap,
        workflows::{coordinator::WorkflowCoordinator, store::WorkflowStore},
    };

    fn test_state(pricing: &PricingStore) -> AppState {
        let dir = std::env::temp_dir().join(format!("octomonitor-test-{}", std::process::id()));
        let store = WorkflowStore::new(dir).unwrap();
        let coord = WorkflowCoordinator::new(store);
        AppState::new(build_bootstrap(pricing), pricing.clone(), coord)
    }

    fn sample_session() -> ViewerSession {
        ViewerSession {
            id: "viewer-1".into(),
            secret: "secret-1".into(),
            label: "Office iPad".into(),
            paired_at: "2026-04-01T10:00:00Z".into(),
            last_seen_at: Some("2026-04-01T10:05:00Z".into()),
            expires_at: "2026-05-01T10:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn clear_remote_access_state_revokes_pairings_and_sessions() {
        let pricing = PricingStore::new();
        let state = test_state(&pricing);
        state.pairings.write().await.push(request_pairing(None));
        state.viewer_sessions.write().await.push(sample_session());

        state.clear_remote_access_state().await;

        assert!(state.pairings.read().await.is_empty());
        assert!(state.viewer_sessions.read().await.is_empty());
    }

    #[tokio::test]
    async fn revoke_remote_device_removes_matching_session_only() {
        let pricing = PricingStore::new();
        let state = test_state(&pricing);
        state.viewer_sessions.write().await.extend([
            sample_session(),
            ViewerSession {
                id: "viewer-2".into(),
                secret: "secret-2".into(),
                label: "MacBook".into(),
                paired_at: "2026-04-01T11:00:00Z".into(),
                last_seen_at: None,
                expires_at: "2026-05-01T11:00:00Z".into(),
            },
        ]);

        assert!(state.revoke_remote_device("viewer-1").await);
        let sessions = state.viewer_sessions.read().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "viewer-2");
        drop(sessions);
        assert!(!state.revoke_remote_device("missing").await);
    }
}
