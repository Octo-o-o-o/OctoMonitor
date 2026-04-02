use std::sync::Arc;
use tokio::sync::{broadcast, Notify, RwLock};

use octomonitor_companion::{PairingRecord, ViewerSession};
use octomonitor_core::BootstrapPayload;

use crate::pricing::PricingStore;

#[derive(Clone)]
pub struct AppState {
    pub bootstrap: Arc<RwLock<BootstrapPayload>>,
    pub pairings: Arc<RwLock<Vec<PairingRecord>>>,
    pub viewer_sessions: Arc<RwLock<Vec<ViewerSession>>>,
    pub notify: broadcast::Sender<()>,
    /// Wakes the probe loop when live ingest receives new activity.
    pub probe_wake: Arc<Notify>,
    /// LiteLLM-sourced model pricing with file cache.
    pub pricing: PricingStore,
}

impl AppState {
    pub fn new(initial: BootstrapPayload, pricing: PricingStore) -> Self {
        let (notify, _) = broadcast::channel(16);
        Self {
            bootstrap: Arc::new(RwLock::new(initial)),
            pairings: Arc::new(RwLock::new(Vec::new())),
            viewer_sessions: Arc::new(RwLock::new(Vec::new())),
            notify,
            probe_wake: Arc::new(Notify::new()),
            pricing,
        }
    }

    /// Signal all WS listeners that state has changed.
    pub fn signal_change(&self) {
        let _ = self.notify.send(());
    }

    /// Wake the probe loop so it runs a fresh scan soon.
    pub fn wake_probe(&self) {
        self.probe_wake.notify_one();
    }

    /// Revoke every pending remote pairing and active viewer session.
    pub async fn clear_remote_access_state(&self) {
        self.pairings.write().await.clear();
        self.viewer_sessions.write().await.clear();
    }

    /// Revoke a single remote viewer device by id.
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
    use crate::{pricing::PricingStore, probe::build_bootstrap};

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
        let state = AppState::new(build_bootstrap(&pricing), pricing);
        state.pairings.write().await.push(request_pairing(None));
        state.viewer_sessions.write().await.push(sample_session());

        state.clear_remote_access_state().await;

        assert!(state.pairings.read().await.is_empty());
        assert!(state.viewer_sessions.read().await.is_empty());
    }

    #[tokio::test]
    async fn revoke_remote_device_removes_matching_session_only() {
        let pricing = PricingStore::new();
        let state = AppState::new(build_bootstrap(&pricing), pricing);
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
