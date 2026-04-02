use std::net::SocketAddr;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Json, Request, State,
    },
    http::{
        header::{COOKIE, SET_COOKIE},
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use octomonitor_companion::{
    claim_pairing, pairing_matches, session_is_expired, touch_viewer_session, PairingRecord,
};
use octomonitor_core::{
    BootstrapPayload, RemoteAccessMode, RemoteAccessState, RemoteDevice, RemotePairingCode,
};
use serde::Deserialize;
use tokio::sync::{broadcast::error::RecvError, oneshot};

use crate::{network::detect_advertised_addresses, state::AppState, static_files::static_handler};

pub const REMOTE_VIEWER_PORT: u16 = 46322;
const REMOTE_COOKIE_NAME: &str = "octomonitor_viewer_session";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePairClaim {
    pub code: String,
    pub label: Option<String>,
}

pub fn spawn_remote_server(state: AppState) {
    tokio::spawn(async move {
        let mut remote_listener = if state.bootstrap.read().await.config.companion_enabled {
            start_remote_listener(state.clone()).await
        } else {
            None
        };
        let mut rx = state.notify.subscribe();

        loop {
            match rx.recv().await {
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(_)) | Ok(_) => {}
            }
            while rx.try_recv().is_ok() {}

            let enabled = state.bootstrap.read().await.config.companion_enabled;
            if enabled && remote_listener.is_none() {
                remote_listener = start_remote_listener(state.clone()).await;
            } else if !enabled && remote_listener.is_some() {
                stop_remote_listener(&mut remote_listener).await;
            }
        }

        stop_remote_listener(&mut remote_listener).await;
    });
}

fn build_remote_router(state: AppState) -> Router {
    Router::new()
        .route("/api/bootstrap", get(get_remote_bootstrap))
        .route("/api/stream", get(remote_stream))
        .route("/api/pair/claim", post(claim_remote_pairing))
        .fallback(remote_static_handler)
        .with_state(state)
}

async fn start_remote_listener(
    state: AppState,
) -> Option<(oneshot::Sender<()>, tokio::task::JoinHandle<()>)> {
    let app = build_remote_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], REMOTE_VIEWER_PORT));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!("Failed to bind remote viewer listener on {addr}: {error}");
            return None;
        }
    };

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        tracing::info!("OctoMonitor remote viewer listening on http://{}", addr);
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(error) = server.await {
            tracing::error!("Remote viewer server exited: {error}");
        }
    });

    Some((shutdown_tx, handle))
}

async fn stop_remote_listener(
    listener: &mut Option<(oneshot::Sender<()>, tokio::task::JoinHandle<()>)>,
) {
    if let Some((shutdown_tx, handle)) = listener.take() {
        let _ = shutdown_tx.send(());
        let _ = handle.await;
    }
}

pub async fn build_remote_access_state(state: &AppState) -> RemoteAccessState {
    prune_expired_remote_state(state).await;

    let config = state.bootstrap.read().await.config.clone();
    let (devices, pending_pairings) = if config.companion_enabled {
        let devices = state
            .viewer_sessions
            .read()
            .await
            .iter()
            .map(|session| RemoteDevice {
                id: session.id.clone(),
                label: session.label.clone(),
                paired_at: session.paired_at.clone(),
                last_seen_at: session.last_seen_at.clone(),
                expires_at: session.expires_at.clone(),
            })
            .collect::<Vec<_>>();
        let pending_pairings = state
            .pairings
            .read()
            .await
            .iter()
            .filter(|record| record.claimed_at.is_none() && !pairing_is_expired(record))
            .map(|record| RemotePairingCode {
                id: record.id.clone(),
                code: record.code.clone(),
                label: record.label.clone(),
                expires_at: record.expires_at.clone(),
            })
            .collect::<Vec<_>>();
        (devices, pending_pairings)
    } else {
        (Vec::new(), Vec::new())
    };

    let addresses = if config.companion_enabled {
        detect_advertised_addresses(REMOTE_VIEWER_PORT)
    } else {
        Vec::new()
    };
    let mode = if !config.companion_enabled {
        RemoteAccessMode::Off
    } else if addresses
        .iter()
        .any(|address| matches!(address.kind.as_str(), "tailscale" | "private"))
    {
        RemoteAccessMode::PrivateNetwork
    } else {
        RemoteAccessMode::LanViewer
    };

    RemoteAccessState {
        enabled: config.companion_enabled,
        mode,
        listener_host: "0.0.0.0".into(),
        listener_port: REMOTE_VIEWER_PORT,
        addresses,
        devices,
        pending_pairings,
    }
}

pub fn redact_bootstrap(payload: &BootstrapPayload) -> BootstrapPayload {
    let mut redacted = payload.clone();

    for run in &mut redacted.runs {
        run.workspace_path.clear();
        run.account_alias = None;
        run.session_id = None;
        run.thread_id = None;
        run.session_key = None;
        run.transcript_path = None;
        run.last_action = None;
        run.last_tail = None;
        run.first_question = None;
        run.last_question = None;
        run.error_message = None;
        run.origin_label = None;
        if let Some(vcs) = &mut run.vcs {
            vcs.repo_root.clear();
            vcs.worktree_id = None;
            vcs.worktree_name = None;
            vcs.worktree_path = None;
            vcs.branch = None;
        }
    }

    for commit in &mut redacted.commits {
        commit.repo_root.clear();
        commit.worktree_id = None;
        commit.worktree_name = None;
        commit.links.clear();
    }

    for identity in &mut redacted.identities {
        identity.account_alias = None;
        identity.fingerprint = None;
    }

    redacted.config.listen_host = "remote-viewer".into();
    redacted.config.local_ip = None;

    redacted
}

pub async fn claim_remote_pairing(
    State(state): State<AppState>,
    Json(input): Json<RemotePairClaim>,
) -> Result<impl IntoResponse, StatusCode> {
    ensure_remote_enabled(&state).await?;
    prune_expired_remote_state(&state).await;

    let mut pairings = state.pairings.write().await;
    let Some(index) = pairings
        .iter()
        .position(|record| pairing_matches(record, &input.code))
    else {
        return Err(StatusCode::NOT_FOUND);
    };

    let record = pairings[index].clone();
    let (_, session) = claim_pairing(&record, input.label.as_deref()).ok_or(StatusCode::GONE)?;
    pairings.remove(index);
    drop(pairings);

    let cookie = build_viewer_cookie(&session.secret);
    state.viewer_sessions.write().await.push(session);
    state.signal_change();

    Ok((
        StatusCode::OK,
        [(SET_COOKIE, cookie)],
        Json(serde_json::json!({ "paired": true })),
    ))
}

pub async fn get_remote_bootstrap(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<BootstrapPayload>, StatusCode> {
    ensure_remote_enabled(&state).await?;
    authenticate_viewer(&headers, &state)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let payload = state.bootstrap.read().await.clone();
    Ok(Json(redact_bootstrap(&payload)))
}

pub async fn remote_stream(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    ensure_remote_enabled(&state).await?;
    let session_secret = authenticate_viewer(&headers, &state)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    Ok(ws.on_upgrade(move |socket| remote_stream_socket(socket, state, session_secret)))
}

pub async fn remote_static_handler(State(state): State<AppState>, req: Request) -> Response {
    if !state.bootstrap.read().await.config.companion_enabled {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    }

    static_handler(req).await
}

async fn remote_stream_socket(mut socket: WebSocket, state: AppState, session_secret: String) {
    if !viewer_session_is_active(&session_secret, &state).await {
        let _ = socket.send(Message::Close(None)).await;
        return;
    }

    {
        let payload = redact_bootstrap(&state.bootstrap.read().await.clone());
        let msg = serde_json::json!({"type": "snapshot.replace", "payload": payload});
        if socket
            .send(Message::Text(msg.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    let mut rx = state.notify.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => {}
                    Ok(_) => {}
                }
                while rx.try_recv().is_ok() {}

                if !viewer_session_is_active(&session_secret, &state).await {
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }

                let payload = redact_bootstrap(&state.bootstrap.read().await.clone());
                let msg = serde_json::json!({"type": "snapshot.replace", "payload": payload});
                if socket.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

async fn ensure_remote_enabled(state: &AppState) -> Result<(), StatusCode> {
    let enabled = state.bootstrap.read().await.config.companion_enabled;
    if enabled {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn authenticate_viewer(headers: &HeaderMap, state: &AppState) -> Option<String> {
    let secret = parse_cookie(headers, REMOTE_COOKIE_NAME)?;
    let mut sessions = state.viewer_sessions.write().await;
    sessions.retain(|session| !session_is_expired(session));

    let session = sessions
        .iter_mut()
        .find(|session| session.secret == secret)?;
    *session = touch_viewer_session(session);
    Some(secret)
}

async fn viewer_session_is_active(secret: &str, state: &AppState) -> bool {
    if !state.bootstrap.read().await.config.companion_enabled {
        return false;
    }

    state
        .viewer_sessions
        .read()
        .await
        .iter()
        .any(|session| session.secret == secret && !session_is_expired(session))
}

async fn prune_expired_remote_state(state: &AppState) {
    state
        .pairings
        .write()
        .await
        .retain(|record| record.claimed_at.is_none() && !pairing_is_expired(record));
    state
        .viewer_sessions
        .write()
        .await
        .retain(|session| !session_is_expired(session));
}

fn parse_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|entry| {
        let (cookie_name, value) = entry.trim().split_once('=')?;
        if cookie_name == name {
            Some(value.to_string())
        } else {
            None
        }
    })
}

fn build_viewer_cookie(secret: &str) -> String {
    format!(
        "{REMOTE_COOKIE_NAME}={secret}; HttpOnly; Path=/; SameSite=Lax; Max-Age={}",
        30 * 24 * 60 * 60
    )
}

fn pairing_is_expired(record: &PairingRecord) -> bool {
    DateTime::parse_from_rfc3339(&record.expires_at)
        .map(|value| value.with_timezone(&Utc) < Utc::now())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use octomonitor_companion::{request_pairing, ViewerSession};
    use octomonitor_core::BootstrapPayload;

    use super::*;
    use crate::pricing::PricingStore;
    use crate::probe::build_bootstrap;

    #[test]
    fn redaction_removes_local_paths_and_identities() {
        let pricing = PricingStore::new();
        let mut payload: BootstrapPayload = build_bootstrap(&pricing);
        if let Some(run) = payload.runs.first_mut() {
            run.last_action = Some("Approve dangerous command".into());
            run.last_tail = Some("rm -rf /tmp/worktree".into());
            run.first_question = Some("Find my API key".into());
            run.last_question = Some("Paste the secret".into());
            run.error_message = Some("Secret token leaked".into());
            run.origin_label = Some("Telegram: Alice".into());
        }
        let redacted = redact_bootstrap(&payload);

        assert!(redacted.runs.iter().all(|run| {
            run.workspace_path.is_empty()
                && run.transcript_path.is_none()
                && run.last_action.is_none()
                && run.last_tail.is_none()
                && run.first_question.is_none()
                && run.last_question.is_none()
                && run.error_message.is_none()
                && run.origin_label.is_none()
        }));
        assert!(redacted.commits.iter().all(|commit| {
            commit.repo_root.is_empty() && commit.worktree_id.is_none() && commit.links.is_empty()
        }));
        assert!(redacted
            .identities
            .iter()
            .all(|identity| identity.account_alias.is_none() && identity.fingerprint.is_none()));
    }

    #[test]
    fn cookie_parser_extracts_viewer_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            "foo=bar; octomonitor_viewer_session=secret"
                .parse()
                .unwrap(),
        );

        assert_eq!(
            parse_cookie(&headers, REMOTE_COOKIE_NAME).as_deref(),
            Some("secret")
        );
    }

    #[tokio::test]
    async fn remote_access_state_hides_devices_and_codes_when_disabled() {
        let pricing = PricingStore::new();
        let state = AppState::new(build_bootstrap(&pricing), pricing);
        state
            .pairings
            .write()
            .await
            .push(request_pairing(Some("Desk")));
        state.viewer_sessions.write().await.push(ViewerSession {
            id: "viewer-1".into(),
            secret: "secret-1".into(),
            label: "Desk".into(),
            paired_at: "2026-04-01T10:00:00Z".into(),
            last_seen_at: Some("2026-04-01T10:05:00Z".into()),
            expires_at: "2026-05-01T10:00:00Z".into(),
        });

        let access = build_remote_access_state(&state).await;

        assert!(!access.enabled);
        assert!(access.devices.is_empty());
        assert!(access.pending_pairings.is_empty());
    }

    #[tokio::test]
    async fn viewer_session_is_inactive_when_remote_is_disabled() {
        let pricing = PricingStore::new();
        let state = AppState::new(build_bootstrap(&pricing), pricing);
        state.viewer_sessions.write().await.push(ViewerSession {
            id: "viewer-1".into(),
            secret: "secret-1".into(),
            label: "Desk".into(),
            paired_at: "2026-04-01T10:00:00Z".into(),
            last_seen_at: Some("2026-04-01T10:05:00Z".into()),
            expires_at: "2026-05-01T10:00:00Z".into(),
        });

        assert!(!viewer_session_is_active("secret-1", &state).await);
    }
}
