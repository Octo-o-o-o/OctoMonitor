use std::collections::HashMap;
use std::net::UdpSocket;

use chrono::Utc;
use octomonitor_claude_adapter as claude_adapter;
use octomonitor_codex_adapter as codex_adapter;
use octomonitor_core::{
    AdapterHealth, AppConfig, AttentionItem, BootstrapPayload, CommitHistoryPayload, Freshness,
    HistoryRange, IdentityState, MoneyValue, PendingCron, QuotaValue, RunRecord, RunState,
    SourceConfidence, SourceInfo, TokenUsage, ToolKind, UsageBucket, UsageHistoryPayload,
};
use octomonitor_openclaw_adapter as openclaw_adapter;

use crate::commits::{build_commit_records, hydrate_run_vcs};
use crate::platform::last_path_component;
use crate::pricing::PricingStore;
use crate::state::AppState;

const PROBE_ACTIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(120);
const PROBE_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
const MAX_BOOTSTRAP_RUNS: usize = 1_500;
const MAX_BOOTSTRAP_COMMITS: usize = 2_000;
const MAX_HISTORY_USAGE_RUNS: usize = 20_000;
const MAX_HISTORY_COMMITS: usize = 5_000;
const MAX_HISTORY_LINKED_RUNS: usize = 20_000;

struct ProbeScanResult {
    generated_at: String,
    runs: Vec<RunRecord>,
    identities: Vec<IdentityState>,
    adapter_health: Vec<AdapterHealth>,
    pending_crons: Vec<PendingCron>,
}

fn has_active_runs(payload: &BootstrapPayload) -> bool {
    payload.runs.iter().any(|r| {
        matches!(
            r.state,
            RunState::Active | RunState::Idle | RunState::WaitingApproval
        )
    })
}

fn default_app_config() -> AppConfig {
    AppConfig {
        listen_host: "127.0.0.1".into(),
        listen_port: 46321,
        history_days: 30,
        companion_enabled: false,
        local_ip: None,
    }
}

pub fn empty_bootstrap() -> BootstrapPayload {
    let mut payload = BootstrapPayload {
        generated_at: String::new(),
        runs: Vec::new(),
        attentions: Vec::new(),
        usage_buckets: Vec::new(),
        commits: Vec::new(),
        identities: Vec::new(),
        adapter_health: Vec::new(),
        recent_completions: Vec::new(),
        pending_crons: Vec::new(),
        config: default_app_config(),
    };
    payload.config.local_ip = detect_local_ip();
    payload
}

async fn refresh_bootstrap_once(state: &AppState) {
    let preserved = state.bootstrap.read().await.clone();
    let pricing = state.pricing.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut fresh = build_bootstrap(&pricing);
        merge_runtime_state(&mut fresh, &preserved, &pricing);
        fresh
    })
    .await;

    match result {
        Ok(refreshed) => {
            *state.bootstrap.write().await = refreshed;
            state.signal_change();
        }
        Err(e) => {
            tracing::error!("Probe thread panicked: {e}; will retry next cycle");
        }
    }
}

pub fn spawn_probe_refresh(state: AppState) {
    tokio::spawn(async move {
        refresh_bootstrap_once(&state).await;
        loop {
            let active = has_active_runs(&*state.bootstrap.read().await);
            if active {
                tokio::time::sleep(PROBE_ACTIVE_INTERVAL).await;
            } else {
                tokio::select! {
                    _ = state.probe_wake.notified() => {}
                    _ = tokio::time::sleep(PROBE_IDLE_TIMEOUT) => {}
                }
            }

            refresh_bootstrap_once(&state).await;
        }
    });
}

const RUN_RETENTION: chrono::Duration = chrono::Duration::days(3650);

fn merge_runtime_state(
    target: &mut BootstrapPayload,
    previous: &BootstrapPayload,
    pricing: &PricingStore,
) {
    target.config = previous.config.clone();

    let mut run_map: HashMap<String, RunRecord> = target
        .runs
        .iter()
        .cloned()
        .map(|run| (run.id.clone(), run))
        .collect();

    let eviction_cutoff = (Utc::now() - RUN_RETENTION).to_rfc3339();
    for run in &previous.runs {
        if run_map.contains_key(&run.id) {
            continue;
        }

        if run.id.starts_with("ingest-")
            && target
                .runs
                .iter()
                .any(|fresh| same_underlying_run(fresh, run))
        {
            continue;
        }

        let dominated = run.id.starts_with("ingest-")
            || matches!(run.state, RunState::Completed | RunState::Error);
        if dominated && run.last_activity_at > eviction_cutoff {
            run_map.insert(run.id.clone(), run.clone());
        }
    }

    target.runs = run_map.into_values().collect();
    target
        .runs
        .sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));

    let mut completions = previous.recent_completions.clone();
    for completion in &target.recent_completions {
        if !completions.iter().any(|item| item.id == completion.id) {
            completions.push(completion.clone());
        }
    }
    completions.sort_by(|a, b| b.finished_at.cmp(&a.finished_at));
    completions.truncate(12);
    target.recent_completions = completions;

    rebuild_derived(target, pricing);
}

fn same_underlying_run(a: &RunRecord, b: &RunRecord) -> bool {
    if a.tool != b.tool {
        return false;
    }

    fn both_match(left: &Option<String>, right: &Option<String>) -> bool {
        matches!((left, right), (Some(l), Some(r)) if l == r)
    }

    both_match(&a.session_id, &b.session_id)
        || both_match(&a.thread_id, &b.thread_id)
        || both_match(&a.session_key, &b.session_key)
}

fn history_cutoff(config: &AppConfig) -> chrono::DateTime<Utc> {
    Utc::now() - chrono::Duration::days(i64::from(config.history_days.max(1)))
}

fn parse_rfc3339_utc(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn is_pinned_run(run: &RunRecord) -> bool {
    matches!(
        run.state,
        RunState::Active
            | RunState::Idle
            | RunState::WaitingApproval
            | RunState::Error
            | RunState::GatewayOffline
            | RunState::LimitExceeded
            | RunState::ContextExceeded
            | RunState::Stale
    )
}

fn apply_history_window(payload: &mut BootstrapPayload) {
    let cutoff = history_cutoff(&payload.config);
    let mut pinned = Vec::new();
    let mut recent = Vec::new();

    for run in std::mem::take(&mut payload.runs) {
        let is_recent = parse_rfc3339_utc(&run.last_activity_at)
            .map(|activity| activity >= cutoff)
            .unwrap_or(false);

        if is_pinned_run(&run) {
            pinned.push(run);
        } else if is_recent {
            recent.push(run);
        }
    }

    pinned.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    recent.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));

    let mut retained = pinned;
    let remaining = MAX_BOOTSTRAP_RUNS.saturating_sub(retained.len());
    retained.extend(recent.into_iter().take(remaining));
    retained.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    payload.runs = retained;

    payload.recent_completions.retain(|completion| {
        parse_rfc3339_utc(&completion.finished_at)
            .map(|finished| finished >= cutoff)
            .unwrap_or(false)
    });
    payload
        .recent_completions
        .sort_by(|a, b| b.finished_at.cmp(&a.finished_at));
    payload.recent_completions.truncate(12);
}

pub fn build_bootstrap(pricing: &PricingStore) -> BootstrapPayload {
    let mut payload = empty_bootstrap();
    let scanned = collect_probe_scan(true);
    payload.runs = scanned.runs;
    payload.identities = scanned.identities;
    payload.adapter_health = scanned.adapter_health;
    payload.pending_crons = scanned.pending_crons;
    rebuild_derived(&mut payload, pricing);
    payload.config.local_ip = detect_local_ip();
    payload.generated_at = scanned.generated_at;
    payload
}

fn scan_adapters() -> (
    claude_adapter::ClaudeSnapshot,
    codex_adapter::CodexSnapshot,
    openclaw_adapter::OpenClawSnapshot,
) {
    let (claude_probe, codex_probe, openclaw_probe) = std::thread::scope(|s| {
        let h1 = s.spawn(claude_adapter::probe);
        let h2 = s.spawn(codex_adapter::probe);
        let h3 = s.spawn(openclaw_adapter::probe);
        (
            h1.join().unwrap_or_else(|e| {
                tracing::error!("claude probe panicked: {e:?}");
                claude_adapter::probe()
            }),
            h2.join().unwrap_or_else(|e| {
                tracing::error!("codex probe panicked: {e:?}");
                codex_adapter::probe()
            }),
            h3.join().unwrap_or_else(|e| {
                tracing::error!("openclaw probe panicked: {e:?}");
                openclaw_adapter::probe()
            }),
        )
    });
    (claude_probe, codex_probe, openclaw_probe)
}

fn make_identity(
    tool: ToolKind,
    auth_mode: &str,
    provider: &str,
    verified: bool,
    configured: bool,
    source: SourceConfidence,
) -> IdentityState {
    IdentityState {
        tool,
        auth_mode: Some(auth_mode.into()),
        provider: Some(provider.into()),
        account_alias: Some("local-probe".into()),
        fingerprint: None,
        auth_age: None,
        verified,
        configured,
        source,
    }
}

fn make_adapter_health(
    tool: ToolKind,
    mode: &str,
    online: bool,
    now: &str,
    first_error: Option<String>,
) -> AdapterHealth {
    AdapterHealth {
        tool,
        mode: mode.into(),
        online,
        last_success_at: Some(now.into()),
        last_error_at: None,
        last_error: first_error,
        freshness: Freshness::Hot,
    }
}

fn collect_probe_scan(include_placeholder_runs: bool) -> ProbeScanResult {
    let now = Utc::now().to_rfc3339();
    let (claude_probe, codex_probe, openclaw_probe) = scan_adapters();
    let mut runs: Vec<RunRecord> = Vec::new();

    if claude_probe.sessions.is_empty() {
        if include_placeholder_runs {
            runs.push(build_probe_run_from_claude(&claude_probe));
        }
    } else {
        runs.extend(claude_probe.sessions.iter().map(|s| build_run_from_claude_session(s, &claude_probe)));
    }

    if codex_probe.sessions.is_empty() {
        if include_placeholder_runs {
            runs.push(build_probe_run_from_codex(&codex_probe));
        }
    } else {
        runs.extend(codex_probe.sessions.iter().map(|s| build_run_from_codex_session(s, &codex_probe)));
    }

    if openclaw_probe.sessions.is_empty() {
        if include_placeholder_runs {
            runs.push(build_probe_run_from_openclaw(&openclaw_probe));
        }
    } else {
        runs.extend(openclaw_probe.sessions.iter().map(|s| build_run_from_openclaw_session(s, &openclaw_probe)));
    }

    dedupe_runs(&mut runs);

    let claude_auth = if claude_probe.cli_available {
        "claude.ai/configured"
    } else {
        "unavailable"
    };
    let codex_auth = if codex_probe.cli_available {
        "configured"
    } else {
        "unavailable"
    };
    let openclaw_auth = if openclaw_probe.gateway_status_ok {
        "gateway"
    } else {
        "sessions-scan"
    };

    let identities = vec![
        make_identity(
            ToolKind::Claude,
            claude_auth,
            "claude",
            claude_probe.cli_available,
            claude_probe.config_exists,
            SourceConfidence::Live,
        ),
        make_identity(
            ToolKind::Codex,
            codex_auth,
            "openai",
            codex_probe.cli_available,
            codex_probe.config_exists,
            SourceConfidence::Live,
        ),
        make_identity(
            ToolKind::OpenClaw,
            openclaw_auth,
            "openclaw",
            openclaw_probe.cli_available,
            openclaw_probe.sessions_dir_exists || openclaw_probe.state_file_exists,
            SourceConfidence::Official,
        ),
    ];

    let openclaw_mode = if openclaw_probe.gateway_status_ok {
        "gateway+status+probe"
    } else {
        "sessions-scan+probe"
    };

    let claude_error = claude_probe.command_probes.iter().find(|p| !p.success).and_then(|p| p.error.clone());
    let codex_error = codex_probe.command_probes.iter().find(|p| !p.success).and_then(|p| p.error.clone());
    let openclaw_error = openclaw_probe.command_probes.iter().find(|p| !p.success).and_then(|p| p.error.clone());

    let adapter_health = vec![
        make_adapter_health(
            ToolKind::Claude,
            "hook+statusline+probe",
            claude_probe.cli_available,
            &now,
            claude_error,
        ),
        make_adapter_health(
            ToolKind::Codex,
            "app-server+hook+probe",
            codex_probe.cli_available,
            &now,
            codex_error,
        ),
        make_adapter_health(
            ToolKind::OpenClaw,
            openclaw_mode,
            openclaw_probe.cli_available,
            &now,
            openclaw_error,
        ),
    ];

    let pending_crons = openclaw_probe
        .cron_jobs
        .iter()
        .filter(|j| j.enabled)
        .map(|j| PendingCron {
            id: j.id.clone(),
            name: j.name.clone(),
            agent_id: j.agent_id.clone(),
            schedule_expr: j.schedule_expr.clone(),
            schedule_tz: j.schedule_tz.clone(),
            schedule_human: j.schedule_human.clone(),
        })
        .collect();

    ProbeScanResult {
        generated_at: now,
        runs,
        identities,
        adapter_health,
        pending_crons,
    }
}

fn detect_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

fn build_run_from_claude_session(
    session: &claude_adapter::ClaudeSession,
    probe: &claude_adapter::ClaudeSnapshot,
) -> RunRecord {
    let five_h = probe.quota.as_ref().and_then(|q| q.five_hour_used_pct);
    let seven_d = probe.quota.as_ref().and_then(|q| q.seven_day_used_pct);

    RunRecord {
        id: format!("claude-session-{}", session.session_id),
        tool: ToolKind::Claude,
        source_mode: "claude_transcript".into(),
        project_name: session.project_name.clone(),
        workspace_path: session.project_path.clone(),
        workspace_short: shorten_path(&session.project_path),
        model: session.model.clone(),
        provider: Some("claude".into()),
        agent_name: None,
        agent_display_name: None,
        account_alias: Some("local-probe".into()),
        auth_mode: Some(
            if probe.cli_available {
                "configured"
            } else {
                "missing-cli"
            }
            .into(),
        ),
        auth_verified: probe.cli_available,
        session_id: Some(session.session_id.clone()),
        thread_id: None,
        session_key: None,
        transcript_path: Some(session.transcript_path.clone()),
        started_at: session.started_at.clone(),
        last_activity_at: session.last_activity_at.clone(),
        elapsed_ms: if session.active_elapsed_ms > 0 {
            session.active_elapsed_ms
        } else {
            elapsed_from_timestamps(&session.started_at, &session.last_activity_at)
        },
        state: classify_claude_session_state(session),
        last_action: session
            .last_question
            .clone()
            .or_else(|| session.first_question.clone()),
        last_tail: None,
        pending_approval: false,
        first_question: session.first_question.clone(),
        last_question: session.last_question.clone(),
        error_message: None,
        message_count: session.message_count,
        tokens: TokenUsage {
            input: session.input_tokens,
            output: session.output_tokens,
            cache_read: session.cache_read_tokens,
            cache_write: session.cache_write_tokens,
            total: session.total_tokens,
            context: 0,
        },
        cost: MoneyValue {
            usd: session.cost_usd,
            confidence: if session.cost_usd.is_some() {
                SourceConfidence::Live
            } else {
                SourceConfidence::Derived
            },
        },
        quota: QuotaValue {
            five_hour_used_pct: five_h,
            seven_day_used_pct: seven_d,
            reset_at: vec![],
            confidence: if five_h.is_some() || seven_d.is_some() {
                SourceConfidence::Official
            } else {
                SourceConfidence::Derived
            },
        },
        source: SourceInfo {
            confidence: SourceConfidence::Live,
            freshness: Freshness::Hot,
            last_updated_at: probe.probed_at.clone(),
        },
        vcs: crate::commits::discover_vcs_context(&session.project_path),
        origin_label: None,
        origin_provider: None,
    }
}

fn classify_claude_session_state(session: &claude_adapter::ClaudeSession) -> RunState {
    if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&session.last_activity_at) {
        let age = Utc::now().signed_duration_since(last);
        if age.num_seconds() < 60 {
            return RunState::Active;
        } else if age.num_minutes() < 5 {
            return RunState::Idle;
        }
    }
    RunState::Completed
}

fn build_run_from_codex_session(
    session: &codex_adapter::CodexSession,
    probe: &codex_adapter::CodexSnapshot,
) -> RunRecord {
    let resolved_cwd = session.cwd.as_deref().map(resolve_worktree_cwd);

    let project_name = resolved_cwd
        .as_deref()
        .and_then(last_path_component)
        .unwrap_or("Codex")
        .to_string();

    let workspace_path = resolved_cwd.unwrap_or_else(|| "~/.codex".into());

    let five_h = probe.sessions.iter().find_map(|s| s.five_hour_used_pct);
    let seven_d = probe.sessions.iter().find_map(|s| s.seven_day_used_pct);

    RunRecord {
        id: format!("codex-session-{}", session.session_id),
        tool: ToolKind::Codex,
        source_mode: "codex_session_scan".into(),
        project_name: session.thread_name.clone().unwrap_or(project_name),
        workspace_path: workspace_path.clone(),
        workspace_short: shorten_path(&workspace_path),
        model: session.model.clone(),
        provider: Some("openai".into()),
        agent_name: None,
        agent_display_name: None,
        account_alias: Some("local-probe".into()),
        auth_mode: Some(
            if probe.cli_available {
                "configured"
            } else {
                "missing-cli"
            }
            .into(),
        ),
        auth_verified: probe.cli_available,
        session_id: None,
        thread_id: Some(session.session_id.clone()),
        session_key: None,
        transcript_path: Some(session.transcript_path.clone()),
        started_at: session.started_at.clone(),
        last_activity_at: session.last_activity_at.clone(),
        elapsed_ms: if session.active_elapsed_ms > 0 {
            session.active_elapsed_ms
        } else {
            elapsed_from_timestamps(&session.started_at, &session.last_activity_at)
        },
        state: classify_codex_session_state(session),
        last_action: session
            .last_question
            .clone()
            .or_else(|| session.first_question.clone())
            .or_else(|| session.thread_name.clone()),
        last_tail: None,
        pending_approval: false,
        first_question: session
            .first_question
            .clone()
            .or_else(|| session.thread_name.clone()),
        last_question: session
            .last_question
            .clone()
            .or_else(|| session.first_question.clone()),
        error_message: None,
        message_count: session.message_count,
        tokens: TokenUsage {
            input: session.input_tokens,
            output: session.output_tokens,
            cache_read: session.cached_input_tokens,
            cache_write: 0,
            total: session.total_tokens,
            context: 0,
        },
        cost: MoneyValue {
            usd: None,
            confidence: SourceConfidence::Estimated,
        },
        quota: QuotaValue {
            five_hour_used_pct: five_h,
            seven_day_used_pct: seven_d,
            reset_at: vec![],
            confidence: if five_h.is_some() || seven_d.is_some() {
                SourceConfidence::Live
            } else {
                SourceConfidence::Derived
            },
        },
        source: SourceInfo {
            confidence: SourceConfidence::Live,
            freshness: Freshness::Hot,
            last_updated_at: probe.probed_at.clone(),
        },
        vcs: session
            .cwd
            .as_deref()
            .and_then(crate::commits::discover_vcs_context),
        origin_label: None,
        origin_provider: None,
    }
}

fn classify_codex_session_state(session: &codex_adapter::CodexSession) -> RunState {
    if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&session.last_activity_at) {
        let age = Utc::now().signed_duration_since(last);
        if age.num_minutes() < 2 {
            return RunState::Active;
        } else if age.num_minutes() < 10 {
            return RunState::Idle;
        }
    }
    RunState::Completed
}

fn classify_openclaw_session_state(session: &openclaw_adapter::OpenClawSession) -> RunState {
    match session.status.trim() {
        "waiting" | "pending" => RunState::WaitingApproval,
        "done" | "completed" => RunState::Completed,
        "error" | "failed" => RunState::Error,
        "running" | "streaming" => {
            if let Some(last) = session
                .updated_at
                .and_then(chrono::DateTime::from_timestamp_millis)
            {
                let age = Utc::now().signed_duration_since(last);
                if age.num_minutes() < 2 {
                    return RunState::Active;
                } else if age.num_minutes() < 10 {
                    return RunState::Idle;
                }
            }
            RunState::Completed
        }
        _ => RunState::Completed,
    }
}

fn build_run_from_openclaw_session(
    session: &openclaw_adapter::OpenClawSession,
    probe: &openclaw_adapter::OpenClawSnapshot,
) -> RunRecord {
    let workspace = session
        .workspace_dir
        .clone()
        .unwrap_or_else(|| "~/.openclaw".into());

    let state = classify_openclaw_session_state(session);

    let started_at = session
        .started_at
        .map(|ts| {
            chrono::DateTime::from_timestamp_millis(ts)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let last_activity_at = session
        .updated_at
        .map(|ts| {
            chrono::DateTime::from_timestamp_millis(ts)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let elapsed = match (session.started_at, session.updated_at) {
        (Some(start), Some(end)) => end.saturating_sub(start),
        _ => 0,
    };

    RunRecord {
        id: format!("openclaw-{}-{}", session.agent_name, session.session_id),
        tool: ToolKind::OpenClaw,
        source_mode: if probe.gateway_status_ok {
            "openclaw_gateway".into()
        } else {
            "openclaw_sessions".into()
        },
        project_name: session
            .label
            .clone()
            .or_else(|| session.origin_label.clone())
            .unwrap_or_else(|| session.agent_name.clone()),
        workspace_path: workspace.clone(),
        workspace_short: shorten_path(&workspace),
        model: session.model.clone(),
        provider: session.model_provider.clone(),
        agent_name: Some(session.agent_name.clone()),
        agent_display_name: session.agent_display_name.clone(),
        account_alias: Some("local-probe".into()),
        auth_mode: Some(
            if probe.gateway_status_ok {
                "gateway"
            } else {
                "sessions-scan"
            }
            .into(),
        ),
        auth_verified: probe.cli_available,
        session_id: None,
        thread_id: None,
        session_key: Some(session.session_key.clone()),
        transcript_path: session.transcript_path.clone(),
        started_at,
        last_activity_at,
        elapsed_ms: elapsed,
        state: state.clone(),
        last_action: session
            .last_question
            .clone()
            .or_else(|| session.first_question.clone())
            .or_else(|| session.label.clone())
            .or_else(|| session.origin_label.clone()),
        last_tail: session.model.clone(),
        pending_approval: state == RunState::WaitingApproval,
        first_question: session.first_question.clone(),
        last_question: session.last_question.clone(),
        error_message: session.error_message.clone(),
        message_count: session.message_count,
        tokens: TokenUsage {
            input: session.input_tokens,
            output: session.output_tokens,
            cache_read: session.cache_read,
            cache_write: session.cache_write,
            total: session.total_tokens,
            context: session.context_tokens.unwrap_or(0),
        },
        cost: MoneyValue {
            usd: session.cost_usd,
            confidence: if session.cost_usd.is_some() {
                SourceConfidence::Derived
            } else {
                SourceConfidence::Estimated
            },
        },
        quota: QuotaValue {
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            reset_at: vec![],
            confidence: SourceConfidence::Derived,
        },
        source: SourceInfo {
            confidence: SourceConfidence::Official,
            freshness: Freshness::Hot,
            last_updated_at: probe.probed_at.clone(),
        },
        vcs: crate::commits::discover_vcs_context(&workspace),
        origin_label: session.origin_label.clone(),
        origin_provider: session.origin_provider.clone(),
    }
}

/// Fields that vary between the three probe-only placeholder runs.
struct ProbeRunParams {
    id: &'static str,
    tool: ToolKind,
    source_mode: String,
    project_name: &'static str,
    workspace_path: String,
    workspace_short: &'static str,
    provider: &'static str,
    auth_mode: &'static str,
    auth_verified: bool,
    state: RunState,
    last_action: &'static str,
    last_tail: Option<String>,
    probed_at: String,
    quota: QuotaValue,
    cost_confidence: SourceConfidence,
    source_confidence: SourceConfidence,
}

fn build_probe_placeholder_run(params: ProbeRunParams) -> RunRecord {
    let now = Utc::now();
    RunRecord {
        id: params.id.into(),
        tool: params.tool,
        source_mode: params.source_mode,
        project_name: params.project_name.into(),
        workspace_path: params.workspace_path,
        workspace_short: params.workspace_short.into(),
        model: None,
        provider: Some(params.provider.into()),
        agent_name: Some("local-probe".into()),
        agent_display_name: None,
        account_alias: Some("local-probe".into()),
        auth_mode: Some(params.auth_mode.into()),
        auth_verified: params.auth_verified,
        session_id: None,
        thread_id: None,
        session_key: None,
        transcript_path: None,
        started_at: now.to_rfc3339(),
        last_activity_at: params.probed_at.clone(),
        elapsed_ms: 0,
        state: params.state,
        last_action: Some(params.last_action.into()),
        last_tail: params.last_tail,
        pending_approval: false,
        first_question: None,
        last_question: None,
        error_message: None,
        message_count: 0,
        tokens: TokenUsage::default(),
        cost: MoneyValue {
            usd: None,
            confidence: params.cost_confidence,
        },
        quota: params.quota,
        source: SourceInfo {
            confidence: params.source_confidence,
            freshness: Freshness::Hot,
            last_updated_at: params.probed_at,
        },
        vcs: None,
        origin_label: None,
        origin_provider: None,
    }
}

fn build_probe_run_from_claude(probe: &claude_adapter::ClaudeSnapshot) -> RunRecord {
    let five_h = probe.quota.as_ref().and_then(|q| q.five_hour_used_pct);
    let seven_d = probe.quota.as_ref().and_then(|q| q.seven_day_used_pct);
    build_probe_placeholder_run(ProbeRunParams {
        id: "claude-probe-run",
        tool: ToolKind::Claude,
        source_mode: "claude_probe".into(),
        project_name: "Claude Code",
        workspace_path: probe
            .config_dir
            .clone()
            .unwrap_or_else(|| "~/.claude".into()),
        workspace_short: "~/.claude",
        provider: "claude",
        auth_mode: if probe.cli_available {
            "configured"
        } else {
            "missing-cli"
        },
        auth_verified: probe.cli_available,
        state: if probe.cli_available {
            RunState::Idle
        } else {
            RunState::Error
        },
        last_action: "Probed local Claude CLI + config",
        last_tail: probe.active_session_hint.clone(),
        probed_at: probe.probed_at.clone(),
        quota: QuotaValue {
            five_hour_used_pct: five_h,
            seven_day_used_pct: seven_d,
            reset_at: vec![],
            confidence: SourceConfidence::Derived,
        },
        cost_confidence: SourceConfidence::Derived,
        source_confidence: SourceConfidence::Live,
    })
}

fn build_probe_run_from_codex(probe: &codex_adapter::CodexSnapshot) -> RunRecord {
    build_probe_placeholder_run(ProbeRunParams {
        id: "codex-probe-run",
        tool: ToolKind::Codex,
        source_mode: "codex_local_state".into(),
        project_name: "Codex",
        workspace_path: probe
            .config_dir
            .clone()
            .unwrap_or_else(|| "~/.codex".into()),
        workspace_short: "~/.codex",
        provider: "openai",
        auth_mode: if probe.cli_available {
            "configured"
        } else {
            "missing-cli"
        },
        auth_verified: probe.cli_available,
        state: if probe.history_exists {
            RunState::Idle
        } else {
            RunState::Stale
        },
        last_action: "Scanned local Codex state",
        last_tail: probe.recent_history_hint.clone(),
        probed_at: probe.probed_at.clone(),
        quota: QuotaValue {
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            reset_at: vec![],
            confidence: SourceConfidence::Derived,
        },
        cost_confidence: SourceConfidence::Estimated,
        source_confidence: SourceConfidence::Live,
    })
}

fn build_probe_run_from_openclaw(probe: &openclaw_adapter::OpenClawSnapshot) -> RunRecord {
    build_probe_placeholder_run(ProbeRunParams {
        id: "openclaw-probe-run",
        tool: ToolKind::OpenClaw,
        source_mode: if probe.gateway_status_ok {
            "openclaw_gateway".into()
        } else {
            "openclaw_sessions".into()
        },
        project_name: "OpenClaw",
        workspace_path: probe
            .workspace_dir
            .clone()
            .unwrap_or_else(|| "~/.openclaw".into()),
        workspace_short: "~/.openclaw",
        provider: "openclaw",
        auth_mode: if probe.gateway_status_ok {
            "gateway"
        } else {
            "sessions-scan"
        },
        auth_verified: probe.cli_available,
        state: if probe.gateway_status_ok || probe.sessions_dir_exists {
            RunState::Idle
        } else {
            RunState::GatewayOffline
        },
        last_action: "Probed Gateway/CLI/session store",
        last_tail: probe.recent_session_hint.clone(),
        probed_at: probe.probed_at.clone(),
        quota: QuotaValue {
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            reset_at: vec![],
            confidence: SourceConfidence::Official,
        },
        cost_confidence: SourceConfidence::Official,
        source_confidence: SourceConfidence::Official,
    })
}

pub fn elapsed_from_timestamps(start: &str, end: &str) -> i64 {
    let start_dt = chrono::DateTime::parse_from_rfc3339(start).ok();
    let end_dt = chrono::DateTime::parse_from_rfc3339(end).ok();
    match (start_dt, end_dt) {
        (Some(s), Some(e)) => (e - s).num_milliseconds().max(0),
        _ => 0,
    }
}

fn normalized_total_tokens(
    tool: &ToolKind,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    total: u64,
) -> u64 {
    if total > 0 {
        return total;
    }

    match tool {
        // Codex cached input is a subset of input_tokens; ccusage falls back
        // to input + output when total_tokens is missing.
        ToolKind::Codex => input.saturating_add(output),
        ToolKind::Claude | ToolKind::OpenClaw => input
            .saturating_add(output)
            .saturating_add(cache_read)
            .saturating_add(cache_write),
    }
}

fn normalize_run_token_totals(run: &mut RunRecord) {
    run.tokens.total = normalized_total_tokens(
        &run.tool,
        run.tokens.input,
        run.tokens.output,
        run.tokens.cache_read,
        run.tokens.cache_write,
        run.tokens.total,
    );
}

fn dedupe_runs(runs: &mut Vec<RunRecord>) {
    let mut seen = std::collections::HashSet::new();
    runs.retain(|run| seen.insert(run.id.clone()));
}

fn build_usage_buckets(runs: &[RunRecord], pricing: &PricingStore) -> Vec<UsageBucket> {
    runs.iter()
        .map(|run| {
            let cost = pricing.estimate_run_cost(run);
            UsageBucket {
                scope: serde_json::json!({
                    "runId": run.id,
                    "tool": tool_key(&run.tool),
                    "project": run.project_name,
                }),
                window: "session".into(),
                start: run.started_at.clone(),
                end: run.last_activity_at.clone(),
                input_tokens: run.tokens.input,
                output_tokens: run.tokens.output,
                cache_read_tokens: run.tokens.cache_read,
                cache_write_tokens: run.tokens.cache_write,
                total_tokens: run.tokens.total,
                cost_usd: cost,
                confidence: if run.cost.usd.is_some() {
                    run.cost.confidence.clone()
                } else {
                    SourceConfidence::Estimated
                },
            }
        })
        .collect()
}

fn run_overlaps_range(
    run: &RunRecord,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
) -> bool {
    let Some(started_at) = parse_rfc3339_utc(&run.started_at) else {
        return false;
    };
    let Some(last_activity_at) = parse_rfc3339_utc(&run.last_activity_at) else {
        return false;
    };
    let range_end = std::cmp::max(last_activity_at, started_at);
    range_end >= from && started_at <= to
}

fn history_range(from: chrono::DateTime<Utc>, to: chrono::DateTime<Utc>) -> HistoryRange {
    HistoryRange {
        from: from.to_rfc3339(),
        to: to.to_rfc3339(),
    }
}

pub fn build_usage_history(
    pricing: &PricingStore,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
) -> UsageHistoryPayload {
    let mut runs = collect_probe_scan(false).runs;
    for run in &mut runs {
        normalize_run_token_totals(run);
    }
    runs.retain(|run| run_overlaps_range(run, from, to));
    runs.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    let truncated = runs.len() > MAX_HISTORY_USAGE_RUNS;
    runs.truncate(MAX_HISTORY_USAGE_RUNS);
    let usage_buckets = build_usage_buckets(&runs, pricing);

    UsageHistoryPayload {
        generated_at: Utc::now().to_rfc3339(),
        range: history_range(from, to),
        truncated,
        runs,
        usage_buckets,
    }
}

pub fn build_commit_history(
    pricing: &PricingStore,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
) -> CommitHistoryPayload {
    let mut runs = collect_probe_scan(false).runs;
    for run in &mut runs {
        normalize_run_token_totals(run);
    }
    hydrate_run_vcs(&mut runs);

    let mut commits = build_commit_records(&runs, pricing, from);
    commits.retain(|commit| {
        parse_rfc3339_utc(&commit.committed_at)
            .map(|committed_at| committed_at >= from && committed_at <= to)
            .unwrap_or(false)
    });
    commits.sort_by(|a, b| b.committed_at.cmp(&a.committed_at));

    let mut truncated = commits.len() > MAX_HISTORY_COMMITS;
    commits.truncate(MAX_HISTORY_COMMITS);

    let linked_run_ids = commits
        .iter()
        .flat_map(|commit| commit.links.iter().map(|link| link.run_id.clone()))
        .collect::<std::collections::HashSet<_>>();
    let mut linked_runs = runs
        .into_iter()
        .filter(|run| linked_run_ids.contains(&run.id))
        .collect::<Vec<_>>();
    linked_runs.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    if linked_runs.len() > MAX_HISTORY_LINKED_RUNS {
        truncated = true;
        linked_runs.truncate(MAX_HISTORY_LINKED_RUNS);
    }

    CommitHistoryPayload {
        generated_at: Utc::now().to_rfc3339(),
        range: history_range(from, to),
        truncated,
        runs: linked_runs,
        commits,
    }
}

pub fn rebuild_derived(payload: &mut BootstrapPayload, pricing: &PricingStore) {
    apply_history_window(payload);
    for run in &mut payload.runs {
        normalize_run_token_totals(run);
    }
    hydrate_run_vcs(&mut payload.runs);
    payload.usage_buckets = build_usage_buckets(&payload.runs, pricing);
    payload.commits = build_commit_records(&payload.runs, pricing, history_cutoff(&payload.config));
    payload.commits.truncate(MAX_BOOTSTRAP_COMMITS);
    payload.attentions = payload
        .runs
        .iter()
        .filter_map(build_attention_from_run)
        .collect();
}

pub fn build_attention_from_run(run: &RunRecord) -> Option<AttentionItem> {
    let kind = match run.state {
        RunState::WaitingApproval => Some(("permission", "warn", "Approval required")),
        RunState::Error | RunState::GatewayOffline => {
            Some(("error", "critical", "Source needs attention"))
        }
        RunState::Stale => Some(("source", "warn", "Source data stale")),
        _ => None,
    }?;
    Some(AttentionItem {
        id: format!("attention-{}", run.id),
        tool: run.tool.clone(),
        run_id: Some(run.id.clone()),
        severity: kind.1.into(),
        kind: kind.0.into(),
        title: kind.2.into(),
        detail: run.last_tail.clone().or_else(|| run.last_action.clone()),
        since: run.last_activity_at.clone(),
    })
}

pub fn tool_key(tool: &ToolKind) -> &'static str {
    match tool {
        ToolKind::Claude => "claude",
        ToolKind::Codex => "codex",
        ToolKind::OpenClaw => "openClaw",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use octomonitor_core::CompletionRecord;

    fn pricing_store() -> PricingStore {
        PricingStore::new()
    }

    fn run(
        id: &str,
        tool: ToolKind,
        state: RunState,
        started_at: &str,
        last_activity_at: &str,
    ) -> RunRecord {
        RunRecord {
            id: id.into(),
            tool,
            source_mode: "test".into(),
            project_name: format!("project-{id}"),
            workspace_path: format!("/tmp/{id}"),
            workspace_short: format!("~/{id}"),
            model: Some("gpt-5".into()),
            provider: Some("openai".into()),
            agent_name: None,
            agent_display_name: None,
            account_alias: None,
            auth_mode: None,
            auth_verified: true,
            session_id: None,
            thread_id: None,
            session_key: None,
            transcript_path: None,
            started_at: started_at.into(),
            last_activity_at: last_activity_at.into(),
            elapsed_ms: 60_000,
            state,
            last_action: None,
            last_tail: None,
            pending_approval: false,
            first_question: None,
            last_question: None,
            error_message: None,
            message_count: 1,
            tokens: TokenUsage {
                input: 1_000,
                output: 200,
                cache_read: 50,
                cache_write: 0,
                total: 1_250,
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
                last_updated_at: last_activity_at.into(),
            },
            vcs: None,
            origin_label: None,
            origin_provider: None,
        }
    }

    fn payload_with_runs(runs: Vec<RunRecord>, history_days: u8) -> BootstrapPayload {
        BootstrapPayload {
            generated_at: String::new(),
            runs,
            attentions: Vec::new(),
            usage_buckets: Vec::new(),
            commits: Vec::new(),
            identities: Vec::new(),
            adapter_health: Vec::new(),
            recent_completions: Vec::new(),
            pending_crons: Vec::new(),
            config: AppConfig {
                listen_host: "127.0.0.1".into(),
                listen_port: 46321,
                history_days,
                companion_enabled: false,
                local_ip: None,
            },
        }
    }

    #[test]
    fn merge_runtime_state_rebuilds_usage_buckets_for_preserved_runs() {
        let pricing = pricing_store();
        let now = Utc::now();
        let fresh_run = run(
            "fresh",
            ToolKind::Codex,
            RunState::Active,
            &(now - chrono::Duration::minutes(10)).to_rfc3339(),
            &now.to_rfc3339(),
        );
        let preserved_run = run(
            "preserved",
            ToolKind::Claude,
            RunState::Completed,
            &(now - chrono::Duration::hours(2)).to_rfc3339(),
            &(now - chrono::Duration::minutes(90)).to_rfc3339(),
        );

        let mut target = BootstrapPayload {
            generated_at: String::new(),
            runs: vec![fresh_run],
            attentions: Vec::new(),
            usage_buckets: Vec::new(),
            commits: Vec::new(),
            identities: Vec::new(),
            adapter_health: Vec::new(),
            recent_completions: Vec::new(),
            pending_crons: Vec::new(),
            config: AppConfig {
                listen_host: "127.0.0.1".into(),
                listen_port: 46321,
                history_days: 7,
                companion_enabled: false,
                local_ip: None,
            },
        };
        let previous = BootstrapPayload {
            generated_at: String::new(),
            runs: vec![preserved_run],
            attentions: Vec::new(),
            usage_buckets: Vec::new(),
            commits: Vec::new(),
            identities: Vec::new(),
            adapter_health: Vec::new(),
            recent_completions: Vec::new(),
            pending_crons: Vec::new(),
            config: target.config.clone(),
        };

        merge_runtime_state(&mut target, &previous, &pricing);

        assert_eq!(target.runs.len(), 2);
        assert_eq!(target.usage_buckets.len(), 2);
        assert_eq!(
            target
                .usage_buckets
                .iter()
                .filter_map(|bucket| bucket.scope.get("runId").and_then(|value| value.as_str()))
                .collect::<Vec<_>>(),
            vec!["fresh", "preserved"]
        );
    }

    #[test]
    fn merge_runtime_state_drops_ingest_runs_when_probe_has_same_session() {
        let pricing = pricing_store();
        let now = Utc::now();
        let mut fresh_run = run(
            "claude-session-session-1",
            ToolKind::Claude,
            RunState::Active,
            &(now - chrono::Duration::minutes(12)).to_rfc3339(),
            &(now - chrono::Duration::minutes(2)).to_rfc3339(),
        );
        fresh_run.session_id = Some("session-1".into());

        let mut ingest_run = run(
            "ingest-claude-session-1",
            ToolKind::Claude,
            RunState::Active,
            &(now - chrono::Duration::minutes(11)).to_rfc3339(),
            &(now - chrono::Duration::minutes(1)).to_rfc3339(),
        );
        ingest_run.session_id = Some("session-1".into());

        let mut target = BootstrapPayload {
            generated_at: String::new(),
            runs: vec![fresh_run],
            attentions: Vec::new(),
            usage_buckets: Vec::new(),
            commits: Vec::new(),
            identities: Vec::new(),
            adapter_health: Vec::new(),
            recent_completions: Vec::new(),
            pending_crons: Vec::new(),
            config: AppConfig {
                listen_host: "127.0.0.1".into(),
                listen_port: 46321,
                history_days: 7,
                companion_enabled: false,
                local_ip: None,
            },
        };
        let previous = BootstrapPayload {
            generated_at: String::new(),
            runs: vec![ingest_run],
            attentions: Vec::new(),
            usage_buckets: Vec::new(),
            commits: Vec::new(),
            identities: Vec::new(),
            adapter_health: Vec::new(),
            recent_completions: Vec::new(),
            pending_crons: Vec::new(),
            config: target.config.clone(),
        };

        merge_runtime_state(&mut target, &previous, &pricing);

        assert_eq!(target.runs.len(), 1);
        assert_eq!(target.runs[0].id, "claude-session-session-1");
        assert_eq!(target.usage_buckets.len(), 1);
    }

    #[test]
    fn rebuild_derived_respects_history_window_but_keeps_pinned_runs() {
        let pricing = pricing_store();
        let now = Utc::now();
        let recent_completed = run(
            "recent-completed",
            ToolKind::Codex,
            RunState::Completed,
            &(now - chrono::Duration::days(2)).to_rfc3339(),
            &(now - chrono::Duration::days(1)).to_rfc3339(),
        );
        let old_completed = run(
            "old-completed",
            ToolKind::Claude,
            RunState::Completed,
            &(now - chrono::Duration::days(30)).to_rfc3339(),
            &(now - chrono::Duration::days(20)).to_rfc3339(),
        );
        let old_error = run(
            "old-error",
            ToolKind::Claude,
            RunState::Error,
            &(now - chrono::Duration::days(30)).to_rfc3339(),
            &(now - chrono::Duration::days(20)).to_rfc3339(),
        );

        let mut payload = payload_with_runs(vec![old_completed, old_error, recent_completed], 7);

        rebuild_derived(&mut payload, &pricing);

        let retained_ids = payload
            .runs
            .iter()
            .map(|run| run.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(retained_ids, vec!["recent-completed", "old-error"]);
        assert_eq!(payload.usage_buckets.len(), 2);
        assert_eq!(payload.attentions.len(), 1);
        assert_eq!(payload.attentions[0].run_id.as_deref(), Some("old-error"));
    }

    #[test]
    fn rebuild_derived_caps_history_and_recent_completions() {
        let pricing = pricing_store();
        let now = Utc::now();
        let mut runs = Vec::with_capacity(MAX_BOOTSTRAP_RUNS + 32);
        let pinned = run(
            "pinned-error",
            ToolKind::Claude,
            RunState::Error,
            &(now - chrono::Duration::days(90)).to_rfc3339(),
            &(now - chrono::Duration::days(90)).to_rfc3339(),
        );
        runs.push(pinned);

        for index in 0..(MAX_BOOTSTRAP_RUNS + 32) {
            let activity_at = now - chrono::Duration::minutes(index as i64);
            runs.push(run(
                &format!("recent-{index:04}"),
                ToolKind::Codex,
                RunState::Completed,
                &(activity_at - chrono::Duration::minutes(5)).to_rfc3339(),
                &activity_at.to_rfc3339(),
            ));
        }

        let mut payload = payload_with_runs(runs, 30);
        payload.recent_completions = (0..20)
            .map(|index| CompletionRecord {
                id: format!("completion-{index:02}"),
                tool: ToolKind::Codex,
                project_name: "OctoMonitor".into(),
                title: format!("Completion {index}"),
                finished_at: (now - chrono::Duration::hours(index as i64)).to_rfc3339(),
                duration_ms: 60_000,
                total_tokens: Some(120),
                cost_usd: Some(0.01),
                state: "completed".into(),
                summary: None,
            })
            .collect();
        payload.recent_completions.push(CompletionRecord {
            id: "completion-old".into(),
            tool: ToolKind::Claude,
            project_name: "OctoMonitor".into(),
            title: "Old completion".into(),
            finished_at: (now - chrono::Duration::days(60)).to_rfc3339(),
            duration_ms: 60_000,
            total_tokens: Some(120),
            cost_usd: Some(0.01),
            state: "completed".into(),
            summary: None,
        });

        rebuild_derived(&mut payload, &pricing);

        assert_eq!(payload.runs.len(), MAX_BOOTSTRAP_RUNS);
        assert_eq!(payload.usage_buckets.len(), MAX_BOOTSTRAP_RUNS);
        assert!(payload.runs.iter().any(|run| run.id == "pinned-error"));
        assert_eq!(payload.recent_completions.len(), 12);
        assert!(!payload
            .recent_completions
            .iter()
            .any(|item| item.id == "completion-old"));
    }

    #[test]
    fn rebuild_derived_normalizes_missing_total_tokens_by_tool() {
        let pricing = pricing_store();
        let mut codex_run = run(
            "codex-missing-total",
            ToolKind::Codex,
            RunState::Completed,
            "2026-04-01T08:00:00Z",
            "2026-04-01T08:10:00Z",
        );
        codex_run.tokens.input = 1_000;
        codex_run.tokens.output = 200;
        codex_run.tokens.cache_read = 300;
        codex_run.tokens.total = 0;

        let mut claude_run = run(
            "claude-missing-total",
            ToolKind::Claude,
            RunState::Completed,
            "2026-04-01T06:00:00Z",
            "2026-04-01T06:30:00Z",
        );
        claude_run.tokens.input = 800;
        claude_run.tokens.output = 150;
        claude_run.tokens.cache_read = 75;
        claude_run.tokens.cache_write = 25;
        claude_run.tokens.total = 0;

        let mut payload = BootstrapPayload {
            generated_at: String::new(),
            runs: vec![codex_run, claude_run],
            attentions: Vec::new(),
            usage_buckets: Vec::new(),
            commits: Vec::new(),
            identities: Vec::new(),
            adapter_health: Vec::new(),
            recent_completions: Vec::new(),
            pending_crons: Vec::new(),
            config: AppConfig {
                listen_host: "127.0.0.1".into(),
                listen_port: 46321,
                history_days: 7,
                companion_enabled: false,
                local_ip: None,
            },
        };

        rebuild_derived(&mut payload, &pricing);

        let codex_run = payload
            .runs
            .iter()
            .find(|run| run.id == "codex-missing-total")
            .expect("codex run should remain present");
        assert_eq!(codex_run.tokens.total, 1_200);

        let claude_run = payload
            .runs
            .iter()
            .find(|run| run.id == "claude-missing-total")
            .expect("claude run should remain present");
        assert_eq!(claude_run.tokens.total, 1_050);

        let bucket_totals = payload
            .usage_buckets
            .iter()
            .filter_map(|bucket| {
                bucket
                    .scope
                    .get("runId")
                    .and_then(|value| value.as_str())
                    .map(|run_id| (run_id.to_string(), bucket.total_tokens))
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(bucket_totals.get("codex-missing-total"), Some(&1_200));
        assert_eq!(bucket_totals.get("claude-missing-total"), Some(&1_050));
    }

    #[test]
    fn codex_run_ids_keep_full_session_id() {
        let session = codex_adapter::CodexSession {
            session_id: "12345678-abcdef-session".into(),
            thread_name: Some("Thread".into()),
            cwd: Some("/tmp/codex".into()),
            model: Some("gpt-5-codex".into()),
            cli_version: None,
            transcript_path: "/tmp/codex/session.jsonl".into(),
            started_at: "2026-04-01T08:00:00Z".into(),
            last_activity_at: "2026-04-01T08:10:00Z".into(),
            input_tokens: 100,
            cached_input_tokens: 20,
            output_tokens: 50,
            total_tokens: 150,
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            five_hour_resets_at: None,
            seven_day_resets_at: None,
            plan_type: None,
            first_question: None,
            last_question: None,
            message_count: 1,
            active_elapsed_ms: 10_000,
        };
        let probe = codex_adapter::CodexSnapshot {
            probed_at: "2026-04-01T08:10:00Z".into(),
            cli_available: true,
            cli_version: None,
            config_dir: Some("/tmp/.codex".into()),
            config_exists: true,
            history_exists: true,
            recent_history_hint: None,
            sessions: vec![session.clone()],
            command_probes: Vec::new(),
            file_probes: Vec::new(),
        };

        let run = build_run_from_codex_session(&session, &probe);

        assert_eq!(run.id, "codex-session-12345678-abcdef-session");
    }

    #[test]
    fn openclaw_run_ids_keep_full_session_id() {
        let updated_at = Utc::now();
        let mut session = openclaw_session("running", updated_at);
        session.session_id = "12345678-openclaw-session".into();
        session.agent_name = "ops".into();

        let probe = openclaw_adapter::OpenClawSnapshot {
            probed_at: updated_at.to_rfc3339(),
            cli_available: true,
            gateway_status_ok: true,
            cli_version: None,
            workspace_dir: Some("/tmp/.openclaw".into()),
            sessions_dir_exists: true,
            state_file_exists: true,
            recent_session_hint: None,
            sessions: vec![session.clone()],
            cron_jobs: Vec::new(),
            command_probes: Vec::new(),
            file_probes: Vec::new(),
        };

        let run = build_run_from_openclaw_session(&session, &probe);

        assert_eq!(run.id, "openclaw-ops-12345678-openclaw-session");
    }

    fn openclaw_session(
        status: &str,
        updated_at: chrono::DateTime<Utc>,
    ) -> openclaw_adapter::OpenClawSession {
        openclaw_adapter::OpenClawSession {
            session_id: "session-1".into(),
            session_key: "agent:ops:cron:session-1".into(),
            agent_name: "ops".into(),
            label: Some("Cron: sample".into()),
            status: status.into(),
            model: Some("gpt-5.4".into()),
            model_provider: Some("openai-codex".into()),
            transcript_path: None,
            workspace_dir: Some("/tmp/openclaw".into()),
            started_at: Some((updated_at - chrono::Duration::minutes(1)).timestamp_millis()),
            updated_at: Some(updated_at.timestamp_millis()),
            context_tokens: None,
            first_question: None,
            last_question: None,
            message_count: 1,
            error_message: None,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            cache_read: 0,
            cache_write: 0,
            cost_usd: None,
            origin_label: None,
            origin_provider: None,
            agent_display_name: None,
        }
    }

    #[test]
    fn classify_openclaw_running_sessions_by_recent_activity() {
        let active = openclaw_session("running", Utc::now() - chrono::Duration::minutes(1));
        let idle = openclaw_session("running", Utc::now() - chrono::Duration::minutes(5));
        let stale = openclaw_session("running", Utc::now() - chrono::Duration::hours(6));

        assert_eq!(classify_openclaw_session_state(&active), RunState::Active);
        assert_eq!(classify_openclaw_session_state(&idle), RunState::Idle);
        assert_eq!(classify_openclaw_session_state(&stale), RunState::Completed);
    }

    #[test]
    fn classify_openclaw_terminal_states_without_recency_override() {
        let waiting = openclaw_session("waiting", Utc::now() - chrono::Duration::days(2));
        let failed = openclaw_session("failed", Utc::now() - chrono::Duration::days(2));
        let done = openclaw_session("done", Utc::now() - chrono::Duration::days(2));

        assert_eq!(
            classify_openclaw_session_state(&waiting),
            RunState::WaitingApproval
        );
        assert_eq!(classify_openclaw_session_state(&failed), RunState::Error);
        assert_eq!(classify_openclaw_session_state(&done), RunState::Completed);
    }
}

/// Resolve a Codex worktree cwd back to its canonical project root.
///
/// Codex creates git worktrees at `~/.codex/worktrees/{hash}/{name}`.
/// The `.git` file inside such a worktree contains a `gitdir:` line
/// pointing back to the main repo's `.git/worktrees/…` directory.
/// We walk up from that gitdir to find the real project root.
///
/// Falls back to the original path on any failure.
pub fn resolve_worktree_cwd(cwd: &str) -> String {
    // Fast path: only attempt resolution for paths that look like Codex worktrees
    if !cwd.contains("/.codex/worktrees/") && !cwd.contains("\\.codex\\worktrees\\") {
        return cwd.to_string();
    }

    let dot_git = std::path::Path::new(cwd).join(".git");
    // In a worktree, .git is a *file* (not a directory) containing "gitdir: …"
    if dot_git.is_file() {
        if let Ok(content) = std::fs::read_to_string(&dot_git) {
            if let Some(gitdir) = content.trim().strip_prefix("gitdir: ") {
                let gitdir_path = std::path::Path::new(gitdir);
                // gitdir points to e.g. /real/project/.git/worktrees/name
                // Walk up to find the parent of `.git`
                for ancestor in gitdir_path.ancestors() {
                    if ancestor.file_name().is_some_and(|n| n == ".git") {
                        if let Some(project_root) = ancestor.parent() {
                            return project_root.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }
    }
    cwd.to_string()
}

pub use crate::platform::shorten_path;
