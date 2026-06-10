use std::net::SocketAddr;

use axum::{
    Router,
    extract::{
        Json, Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{
        HeaderMap, StatusCode,
        header::{COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use octomonitor_companion::{
    claim_pairing, pairing_is_expired, pairing_matches, session_is_expired, touch_viewer_session,
};
use octomonitor_core::{
    AppConfig, AttentionItem, BootstrapPayload, CommitRecord, CompletionRecord, DataSourceHealth,
    IdentityState, RemoteAccessMode, RemoteAccessState, RemoteDevice, RemotePairingCode, RunRecord,
    SessionLifecycle, VcsContext,
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

    let enabled = state.bootstrap.read().await.config.companion_enabled;

    if !enabled {
        return RemoteAccessState {
            enabled: false,
            mode: RemoteAccessMode::Off,
            listener_host: "0.0.0.0".into(),
            listener_port: REMOTE_VIEWER_PORT,
            addresses: Vec::new(),
            devices: Vec::new(),
            pending_pairings: Vec::new(),
        };
    }

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

    let addresses = detect_advertised_addresses(REMOTE_VIEWER_PORT);
    let mode = if addresses
        .iter()
        .any(|address| matches!(address.kind.as_str(), "tailscale" | "private"))
    {
        RemoteAccessMode::PrivateNetwork
    } else {
        RemoteAccessMode::LanViewer
    };

    RemoteAccessState {
        enabled: true,
        mode,
        listener_host: "0.0.0.0".into(),
        listener_port: REMOTE_VIEWER_PORT,
        addresses,
        devices,
        pending_pairings,
    }
}

pub fn redact_bootstrap(payload: &BootstrapPayload) -> BootstrapPayload {
    BootstrapPayload {
        generated_at: payload.generated_at.clone(),
        runs: payload.runs.iter().map(redact_run).collect(),
        attentions: payload.attentions.iter().map(redact_attention).collect(),
        usage_buckets: payload.usage_buckets.clone(),
        commits: payload.commits.iter().map(redact_commit).collect(),
        identities: payload.identities.iter().map(redact_identity).collect(),
        adapter_health: payload.adapter_health.clone(),
        recent_completions: payload
            .recent_completions
            .iter()
            .map(redact_completion)
            .collect(),
        pending_crons: payload.pending_crons.clone(),
        config: redact_config(&payload.config),
    }
}

fn redact_run(run: &RunRecord) -> RunRecord {
    RunRecord {
        id: run.id.clone(),
        tool: run.tool.clone(),
        source_id: run.source_id.clone(),
        source_mode: run.source_mode.clone(),
        project_name: run.project_name.clone(),
        workspace_path: String::new(),
        workspace_short: run.project_name.clone(),
        model: run.model.clone(),
        provider: run.provider.clone(),
        agent_name: run.agent_name.clone(),
        agent_display_name: run.agent_display_name.clone(),
        account_alias: None,
        auth_mode: run.auth_mode.clone(),
        auth_verified: run.auth_verified,
        session_id: None,
        thread_id: None,
        session_key: None,
        transcript_path: None,
        started_at: run.started_at.clone(),
        last_activity_at: run.last_activity_at.clone(),
        elapsed_ms: run.elapsed_ms,
        state: run.state.clone(),
        last_action: None,
        last_tail: None,
        pending_approval: run.pending_approval,
        first_question: None,
        last_question: None,
        error_message: None,
        message_count: run.message_count,
        tokens: run.tokens.clone(),
        cost: run.cost.clone(),
        quota: run.quota.clone(),
        source: run.source.clone(),
        lifecycle: run.lifecycle.as_ref().map(redact_lifecycle),
        usage_semantics: run.usage_semantics.clone(),
        data_sources: run
            .data_sources
            .as_ref()
            .map(|sources| sources.iter().map(redact_data_source).collect()),
        capabilities: Some(Vec::new()),
        jump_targets: Some(Vec::new()),
        tool_specific: Some(serde_json::json!({})),
        vcs: run.vcs.as_ref().map(redact_vcs_context),
        origin_label: None,
        origin_provider: run.origin_provider.clone(),
    }
}

fn redact_lifecycle(lifecycle: &SessionLifecycle) -> SessionLifecycle {
    SessionLifecycle {
        status: lifecycle.status.clone(),
        status_source: lifecycle.status_source.clone(),
        started_at: lifecycle.started_at.clone(),
        last_activity_at: lifecycle.last_activity_at.clone(),
        ended_at: lifecycle.ended_at.clone(),
        error: None,
    }
}

fn redact_data_source(source: &DataSourceHealth) -> DataSourceHealth {
    DataSourceHealth {
        id: source.id.clone(),
        source_type: source.source_type.clone(),
        path: None,
        api_endpoint: None,
        last_seen_at: source.last_seen_at.clone(),
        schema_version: source.schema_version.clone(),
        schema_confidence: source.schema_confidence.clone(),
        errors: Vec::new(),
    }
}

fn redact_vcs_context(vcs: &VcsContext) -> VcsContext {
    VcsContext {
        repo_id: vcs.repo_id.clone(),
        repo_name: String::new(),
        repo_root: String::new(),
        worktree_id: None,
        worktree_name: None,
        worktree_path: None,
        branch: None,
        confidence: vcs.confidence.clone(),
    }
}

fn redact_attention(attention: &AttentionItem) -> AttentionItem {
    AttentionItem {
        id: attention.id.clone(),
        tool: attention.tool.clone(),
        run_id: attention.run_id.clone(),
        severity: attention.severity.clone(),
        kind: attention.kind.clone(),
        title: attention.title.clone(),
        detail: None,
        since: attention.since.clone(),
    }
}

fn redact_commit(commit: &CommitRecord) -> CommitRecord {
    CommitRecord {
        id: commit.id.clone(),
        repo_id: commit.repo_id.clone(),
        repo_name: commit.repo_name.clone(),
        repo_root: String::new(),
        worktree_id: None,
        worktree_name: None,
        sha: commit.sha.clone(),
        short_sha: commit.short_sha.clone(),
        author_name: String::new(),
        committed_at: commit.committed_at.clone(),
        summary: "[redacted]".into(),
        files_changed: commit.files_changed,
        insertions: commit.insertions,
        deletions: commit.deletions,
        attributed_tokens: commit.attributed_tokens,
        attributed_cost_usd: commit.attributed_cost_usd,
        run_count: commit.run_count,
        source_count: commit.source_count,
        confidence: commit.confidence.clone(),
        method: commit.method.clone(),
        sources: commit.sources.clone(),
        links: Vec::new(),
    }
}

fn redact_identity(identity: &IdentityState) -> IdentityState {
    IdentityState {
        tool: identity.tool.clone(),
        auth_mode: identity.auth_mode.clone(),
        provider: identity.provider.clone(),
        account_alias: None,
        fingerprint: None,
        auth_age: identity.auth_age.clone(),
        verified: identity.verified,
        configured: identity.configured,
        source: identity.source.clone(),
    }
}

fn redact_completion(completion: &CompletionRecord) -> CompletionRecord {
    CompletionRecord {
        id: completion.id.clone(),
        tool: completion.tool.clone(),
        project_name: String::new(),
        title: "[redacted]".into(),
        finished_at: completion.finished_at.clone(),
        duration_ms: completion.duration_ms,
        total_tokens: completion.total_tokens,
        cost_usd: completion.cost_usd,
        state: completion.state.clone(),
        summary: None,
    }
}

fn redact_config(config: &AppConfig) -> AppConfig {
    AppConfig {
        listen_host: "remote-viewer".into(),
        listen_port: config.listen_port,
        history_days: config.history_days,
        companion_enabled: config.companion_enabled,
        local_ip: None,
    }
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

    async fn send_snapshot(socket: &mut WebSocket, state: &AppState) -> bool {
        let payload = redact_bootstrap(&state.bootstrap.read().await.clone());
        let msg = serde_json::json!({"type": "snapshot.replace", "payload": payload});
        socket
            .send(Message::Text(msg.to_string().into()))
            .await
            .is_ok()
    }

    if !send_snapshot(&mut socket, &state).await {
        return;
    }

    let mut rx = state.notify.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) | Ok(_) => {}
                }
                while rx.try_recv().is_ok() {}

                if !viewer_session_is_active(&session_secret, &state).await {
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }

                if !send_snapshot(&mut socket, &state).await {
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
    state
        .bootstrap
        .read()
        .await
        .config
        .companion_enabled
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
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
        (cookie_name == name).then(|| value.to_string())
    })
}

fn build_viewer_cookie(secret: &str) -> String {
    format!(
        "{REMOTE_COOKIE_NAME}={secret}; HttpOnly; Path=/; SameSite=Lax; Max-Age={}",
        30 * 24 * 60 * 60
    )
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use octomonitor_companion::{ViewerSession, request_pairing};
    use octomonitor_core::{
        AppConfig, AttentionItem, CommitAttributionLink, CommitAttributionMethod, CommitRecord,
        CommitSourceStat, CompletionRecord, Freshness, IdentityState, MoneyValue, QuotaValue,
        RunRecord, RunState, SourceConfidence, SourceInfo, TokenUsage, ToolKind, VcsContext,
    };

    use super::*;
    use crate::pricing::PricingStore;
    use crate::probe::empty_bootstrap;
    use tower::util::ServiceExt;

    fn test_state(pricing: &PricingStore) -> AppState {
        AppState::new(empty_bootstrap(), pricing.clone())
    }

    #[test]
    fn redaction_projects_remote_safe_payload() {
        let payload = sample_bootstrap();
        let redacted = redact_bootstrap(&payload);

        let run = &redacted.runs[0];
        assert_eq!(run.project_name, "OctoMonitor");
        assert_eq!(run.workspace_short, "OctoMonitor");
        assert!(run.workspace_path.is_empty());
        assert!(run.account_alias.is_none());
        assert!(run.session_id.is_none());
        assert!(run.thread_id.is_none());
        assert!(run.session_key.is_none());
        assert!(run.transcript_path.is_none());
        assert!(run.last_action.is_none());
        assert!(run.last_tail.is_none());
        assert!(run.first_question.is_none());
        assert!(run.last_question.is_none());
        assert!(run.error_message.is_none());
        assert!(run.origin_label.is_none());
        assert!(run.capabilities.as_ref().is_some_and(Vec::is_empty));
        assert!(run.jump_targets.as_ref().is_some_and(Vec::is_empty));
        assert_eq!(run.tool_specific, Some(serde_json::json!({})));
        assert!(
            run.lifecycle
                .as_ref()
                .is_some_and(|lifecycle| lifecycle.error.is_none())
        );
        assert!(
            run.data_sources
                .as_ref()
                .is_some_and(|sources| sources.iter().all(|source| {
                    source.path.is_none()
                        && source.api_endpoint.is_none()
                        && source.errors.is_empty()
                }))
        );

        let vcs = run.vcs.as_ref().expect("redacted vcs");
        assert!(vcs.repo_name.is_empty());
        assert!(vcs.repo_root.is_empty());
        assert!(vcs.worktree_id.is_none());
        assert!(vcs.worktree_name.is_none());
        assert!(vcs.worktree_path.is_none());
        assert!(vcs.branch.is_none());

        let attention = &redacted.attentions[0];
        assert!(attention.detail.is_none());

        let commit = &redacted.commits[0];
        assert_eq!(commit.repo_name, "OctoMonitor");
        assert!(commit.repo_root.is_empty());
        assert!(commit.worktree_id.is_none());
        assert!(commit.worktree_name.is_none());
        assert!(commit.author_name.is_empty());
        assert_eq!(commit.summary, "[redacted]");
        assert!(commit.links.is_empty());

        let identity = &redacted.identities[0];
        assert!(identity.account_alias.is_none());
        assert!(identity.fingerprint.is_none());

        let completion = &redacted.recent_completions[0];
        assert!(completion.project_name.is_empty());
        assert_eq!(completion.title, "[redacted]");
        assert!(completion.summary.is_none());

        assert_eq!(redacted.config.listen_host, "remote-viewer");
        assert!(redacted.config.local_ip.is_none());
    }

    #[tokio::test]
    async fn remote_router_does_not_expose_local_mutation_or_run_operation_routes() {
        let pricing = PricingStore::new();
        let state = test_state(&pricing);
        state.bootstrap.write().await.config.companion_enabled = true;
        let app = build_remote_router(state);

        for (method, uri) in [
            (Method::PATCH, "/api/config"),
            (Method::POST, "/api/ingest/codex/hook"),
            (Method::GET, "/api/runs/run-1/events"),
            (Method::GET, "/api/runs/run-1/resume-command"),
            (Method::POST, "/api/runs/run-1/turn/interrupt"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method.clone())
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {uri}");
        }
    }

    fn sample_bootstrap() -> BootstrapPayload {
        let mut payload = empty_bootstrap();
        payload.config = AppConfig {
            listen_host: "127.0.0.1".into(),
            listen_port: REMOTE_VIEWER_PORT,
            history_days: 30,
            companion_enabled: true,
            local_ip: Some("192.168.0.10".into()),
        };
        payload.runs.push(RunRecord {
            id: "run-1".into(),
            tool: ToolKind::Claude,
            source_mode: "hook".into(),
            project_name: "OctoMonitor".into(),
            workspace_path: "/Users/demo/WorkSpace/OctoMonitor".into(),
            workspace_short: "~/WorkSpace/OctoMonitor".into(),
            model: Some("claude-3-7-sonnet".into()),
            provider: Some("claude".into()),
            agent_name: Some("main".into()),
            agent_display_name: Some("Main Agent".into()),
            account_alias: Some("work".into()),
            auth_mode: Some("claude.ai".into()),
            auth_verified: true,
            session_id: Some("session-1".into()),
            thread_id: Some("thread-1".into()),
            session_key: Some("key-1".into()),
            transcript_path: Some("/Users/demo/.claude/transcript.jsonl".into()),
            started_at: "2026-04-09T08:00:00Z".into(),
            last_activity_at: "2026-04-09T08:05:00Z".into(),
            elapsed_ms: 300_000,
            state: RunState::WaitingApproval,
            last_action: Some("Approve dangerous command".into()),
            last_tail: Some("rm -rf /tmp/worktree".into()),
            pending_approval: true,
            first_question: Some("Find my API key".into()),
            last_question: Some("Paste the secret".into()),
            error_message: Some("Secret token leaked".into()),
            message_count: 12,
            tokens: TokenUsage {
                input: 1_000,
                output: 200,
                cache_read: 0,
                cache_write: 0,
                total: 1_200,
                context: 0,
            },
            cost: MoneyValue {
                usd: Some(1.25),
                confidence: SourceConfidence::Estimated,
            },
            quota: QuotaValue {
                five_hour_used_pct: Some(0.4),
                seven_day_used_pct: Some(0.6),
                reset_at: vec!["2026-04-09T12:00:00Z".into()],
                confidence: SourceConfidence::Derived,
            },
            source: SourceInfo {
                confidence: SourceConfidence::Live,
                freshness: Freshness::Hot,
                last_updated_at: "2026-04-09T08:05:00Z".into(),
            },
            source_id: Some("claude:hook".into()),
            lifecycle: Some(SessionLifecycle::default()),
            usage_semantics: Some(octomonitor_core::UsageSemantics {
                cost_kind: octomonitor_core::UsageCostKind::Estimated,
                source: octomonitor_core::UsageDataSource::Transcript,
                enters_usage_totals: true,
                note: None,
            }),
            data_sources: Some(vec![DataSourceHealth {
                id: "claude:hook".into(),
                source_type: octomonitor_core::DataSourceType::Hook,
                path: Some("/Users/demo/.claude/transcript.jsonl".into()),
                api_endpoint: None,
                last_seen_at: Some("2026-04-09T08:05:00Z".into()),
                schema_version: None,
                schema_confidence: octomonitor_core::SchemaConfidence::High,
                errors: Vec::new(),
            }]),
            capabilities: Some(vec![octomonitor_core::CapabilityDescriptor {
                id: "turn.interrupt".into(),
                source: octomonitor_core::CapabilitySource::Inferred,
                confidence: octomonitor_core::SchemaConfidence::Low,
                mutates_state: true,
                requires_user_confirmation: true,
                requires_managed_process: true,
                can_expose_secrets: true,
                audit_level: octomonitor_core::AuditLevel::Full,
                failure_mode: octomonitor_core::CapabilityFailureMode::MayLeaveProcessRunning,
            }]),
            jump_targets: Some(Vec::new()),
            tool_specific: Some(serde_json::json!({ "raw": "local-only" })),
            vcs: Some(VcsContext {
                repo_id: "repo-1".into(),
                repo_name: "OctoMonitor".into(),
                repo_root: "/Users/demo/WorkSpace/OctoMonitor".into(),
                worktree_id: Some("wt-1".into()),
                worktree_name: Some("feature/remote".into()),
                worktree_path: Some("/Users/demo/WorkSpace/OctoMonitor".into()),
                branch: Some("feature/remote".into()),
                confidence: SourceConfidence::Derived,
            }),
            origin_label: Some("Telegram: Alice".into()),
            origin_provider: Some("telegram".into()),
        });
        payload.attentions.push(AttentionItem {
            id: "attention-run-1".into(),
            tool: ToolKind::Claude,
            run_id: Some("run-1".into()),
            severity: "warn".into(),
            kind: "permission".into(),
            title: "Approval required".into(),
            detail: Some("rm -rf /tmp/worktree".into()),
            since: "2026-04-09T08:05:00Z".into(),
        });
        payload.commits.push(CommitRecord {
            id: "repo-1:abc123".into(),
            repo_id: "repo-1".into(),
            repo_name: "OctoMonitor".into(),
            repo_root: "/Users/demo/WorkSpace/OctoMonitor".into(),
            worktree_id: Some("wt-1".into()),
            worktree_name: Some("feature/remote".into()),
            sha: "abc123abc123abc123".into(),
            short_sha: "abc123".into(),
            author_name: "Demo User".into(),
            committed_at: "2026-04-09T08:02:00Z".into(),
            summary: "Handle remote redaction".into(),
            files_changed: 3,
            insertions: 42,
            deletions: 7,
            attributed_tokens: 500,
            attributed_cost_usd: Some(0.4),
            run_count: 1,
            source_count: 1,
            confidence: SourceConfidence::Heuristic,
            method: CommitAttributionMethod::ReadOnlyHeuristic,
            sources: vec![CommitSourceStat {
                tool: ToolKind::Claude,
                run_count: 1,
                attributed_tokens: 500,
                attributed_cost_usd: Some(0.4),
                confidence: SourceConfidence::Heuristic,
            }],
            links: vec![CommitAttributionLink {
                run_id: "run-1".into(),
                tool: ToolKind::Claude,
                source_mode: "hook".into(),
                project_name: "OctoMonitor".into(),
                session_label: "Find my API key".into(),
                score: 1.0,
                allocated_tokens: 500,
                allocated_cost_usd: Some(0.4),
                confidence: SourceConfidence::Heuristic,
                method: CommitAttributionMethod::ReadOnlyHeuristic,
            }],
        });
        payload.identities.push(IdentityState {
            tool: ToolKind::Claude,
            auth_mode: Some("claude.ai".into()),
            provider: Some("claude".into()),
            account_alias: Some("work".into()),
            fingerprint: Some("fingerprint".into()),
            auth_age: Some("1d".into()),
            verified: true,
            configured: true,
            source: SourceConfidence::Live,
        });
        payload.recent_completions.push(CompletionRecord {
            id: "completion-1".into(),
            tool: ToolKind::Claude,
            project_name: "OctoMonitor".into(),
            title: "Write remote redaction tests".into(),
            finished_at: "2026-04-09T08:03:00Z".into(),
            duration_ms: 60_000,
            total_tokens: Some(320),
            cost_usd: Some(0.12),
            state: "completed".into(),
            summary: Some("Covers recent completions redaction".into()),
        });
        payload
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
        let state = test_state(&pricing);
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
        let state = test_state(&pricing);
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
