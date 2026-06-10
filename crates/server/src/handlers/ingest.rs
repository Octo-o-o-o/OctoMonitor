use axum::{extract::State, Json};
use chrono::Utc;
use octomonitor_core::{
    AuditLevel, CapabilityDescriptor, CapabilityFailureMode, CapabilitySource, DataSourceHealth,
    DataSourceType, Freshness, LifecycleStatusSource, MoneyValue, RunRecord, RunState,
    SchemaConfidence, SessionLifecycle, SourceConfidence, SourceInfo, TokenUsage, ToolKind,
    UsageCostKind, UsageDataSource, UsageSemantics,
};
use serde::Deserialize;

use crate::perf;
use crate::platform::last_path_component;
use crate::probe::{elapsed_from_timestamps, shorten_path};
use crate::state::AppState;

fn default_quota() -> octomonitor_core::QuotaValue {
    statusline_quota(None, None, Vec::new())
}

fn statusline_quota(
    five_hour_used_pct: Option<f64>,
    seven_day_used_pct: Option<f64>,
    reset_at: Vec<String>,
) -> octomonitor_core::QuotaValue {
    let has_data =
        five_hour_used_pct.is_some() || seven_day_used_pct.is_some() || !reset_at.is_empty();
    octomonitor_core::QuotaValue {
        five_hour_used_pct,
        seven_day_used_pct,
        reset_at,
        confidence: if has_data {
            SourceConfidence::Official
        } else {
            SourceConfidence::Derived
        },
    }
}

fn epoch_seconds_to_rfc3339(value: Option<i64>) -> Option<String> {
    chrono::DateTime::from_timestamp(value?, 0).map(|dt| dt.to_rfc3339())
}

fn live_source() -> SourceInfo {
    SourceInfo {
        confidence: SourceConfidence::Live,
        freshness: Freshness::Hot,
        last_updated_at: Utc::now().to_rfc3339(),
    }
}

fn live_lifecycle(
    state: RunState,
    started_at: &str,
    last_activity_at: &str,
    source: LifecycleStatusSource,
) -> SessionLifecycle {
    SessionLifecycle {
        status: state,
        status_source: source,
        started_at: Some(started_at.to_string()),
        last_activity_at: Some(last_activity_at.to_string()),
        ended_at: None,
        error: None,
    }
}

fn usage_semantics(cost_kind: UsageCostKind, source: UsageDataSource) -> UsageSemantics {
    UsageSemantics {
        cost_kind,
        source,
        enters_usage_totals: true,
        note: None,
    }
}

fn data_source_health(
    id: &str,
    source_type: DataSourceType,
    path: Option<String>,
    last_seen_at: &str,
) -> Vec<DataSourceHealth> {
    vec![DataSourceHealth {
        id: id.into(),
        source_type,
        path,
        api_endpoint: None,
        last_seen_at: Some(last_seen_at.to_string()),
        schema_version: None,
        schema_confidence: SchemaConfidence::High,
        errors: Vec::new(),
    }]
}

fn safe_capability(
    id: &str,
    source: CapabilitySource,
    confidence: SchemaConfidence,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: id.into(),
        source,
        confidence,
        mutates_state: false,
        requires_user_confirmation: false,
        requires_managed_process: false,
        can_expose_secrets: false,
        audit_level: AuditLevel::Metadata,
        failure_mode: CapabilityFailureMode::Safe,
    }
}

fn resume_capabilities(has_resume_id: bool, include_deeplink: bool) -> Vec<CapabilityDescriptor> {
    let mut capabilities = Vec::new();
    if has_resume_id {
        capabilities.push(safe_capability(
            "resume.copyCommand",
            CapabilitySource::OfficialCli,
            SchemaConfidence::High,
        ));
        if include_deeplink {
            capabilities.push(safe_capability(
                "open.sessionDeeplink",
                CapabilitySource::Inferred,
                SchemaConfidence::Medium,
            ));
        }
    }
    capabilities
}

async fn source_ingest_enabled(state: &AppState, tool: ToolKind) -> bool {
    let payload = state.bootstrap.read().await;
    payload.config.source_enabled(tool)
}

fn disabled_source_response(tool: ToolKind) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": false,
        "ignored": true,
        "reason": "source disabled",
        "tool": tool,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatuslineIngest {
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub project_name: Option<String>,
    pub workspace_path: Option<String>,
    pub total_cost_usd: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub context_tokens: Option<u64>,
    pub five_hour_used_pct: Option<f64>,
    pub seven_day_used_pct: Option<f64>,
    pub five_hour_resets_at: Option<i64>,
    pub seven_day_resets_at: Option<i64>,
    pub pending_approval: Option<bool>,
    pub last_action: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeHookIngest {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub event: Option<String>,
    pub transcript_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookIngest {
    pub thread_id: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub event: Option<String>,
    pub waiting_on_approval: Option<bool>,
    pub total_tokens: Option<u64>,
}

pub async fn ingest_claude_statusline(
    State(state): State<AppState>,
    Json(input): Json<ClaudeStatuslineIngest>,
) -> Json<serde_json::Value> {
    if !source_ingest_enabled(&state, ToolKind::Claude).await {
        return disabled_source_response(ToolKind::Claude);
    }
    let session_id = input.session_id;
    let workspace_path = input.workspace_path.unwrap_or_else(|| "~/.claude".into());
    let quota = statusline_quota(
        input.five_hour_used_pct,
        input.seven_day_used_pct,
        [
            epoch_seconds_to_rfc3339(input.five_hour_resets_at),
            epoch_seconds_to_rfc3339(input.seven_day_resets_at),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );
    let started_at = Utc::now().to_rfc3339();
    let last_activity_at = started_at.clone();
    let run_state = if input.pending_approval.unwrap_or(false) {
        RunState::WaitingApproval
    } else {
        RunState::Active
    };
    let transcript_path = input.transcript_path;
    let has_resume_id = session_id.as_deref().is_some_and(|id| !id.is_empty());
    let run = RunRecord {
        id: format!(
            "ingest-claude-{}",
            session_id.as_deref().unwrap_or("unknown")
        ),
        tool: ToolKind::Claude,
        source_id: Some("claude:statusline".into()),
        source_mode: "claude_statusline".into(),
        project_name: input
            .project_name
            .unwrap_or_else(|| "Claude Session".into()),
        workspace_short: shorten_path(&workspace_path),
        model: input.model,
        provider: input.provider,
        agent_name: Some("live-ingest".into()),
        agent_display_name: None,
        account_alias: Some("local-ingest".into()),
        auth_mode: Some("claude.ai".into()),
        auth_verified: true,
        session_id,
        thread_id: None,
        session_key: None,
        transcript_path: transcript_path.clone(),
        started_at: started_at.clone(),
        last_activity_at: last_activity_at.clone(),
        elapsed_ms: 0,
        state: run_state.clone(),
        last_action: input.last_action,
        last_tail: Some("live statusline ingest".into()),
        pending_approval: input.pending_approval.unwrap_or(false),
        first_question: None,
        last_question: None,
        error_message: None,
        message_count: 0,
        tokens: TokenUsage {
            input: input.input_tokens.unwrap_or(0),
            output: input.output_tokens.unwrap_or(0),
            cache_read: 0,
            cache_write: 0,
            total: input.total_tokens.unwrap_or(0),
            context: input.context_tokens.unwrap_or(0),
        },
        cost: MoneyValue {
            usd: input.total_cost_usd,
            confidence: SourceConfidence::Live,
        },
        quota,
        source: live_source(),
        lifecycle: Some(live_lifecycle(
            run_state,
            &started_at,
            &last_activity_at,
            LifecycleStatusSource::Hook,
        )),
        usage_semantics: Some(usage_semantics(
            if input.total_cost_usd.is_some() {
                UsageCostKind::Exact
            } else {
                UsageCostKind::Partial
            },
            UsageDataSource::Statusline,
        )),
        data_sources: Some(data_source_health(
            "claude:statusline",
            DataSourceType::Hook,
            transcript_path,
            &last_activity_at,
        )),
        capabilities: Some(resume_capabilities(has_resume_id, false)),
        jump_targets: Some(Vec::new()),
        tool_specific: Some(serde_json::json!({})),
        vcs: crate::commits::discover_vcs_context(&workspace_path),
        workspace_path,
        origin_label: None,
        origin_provider: None,
    };
    perf::log_ingest_event("claude_statusline", &run.id);
    upsert_runtime_run(&state, run, "ingest_claude_statusline").await;
    Json(serde_json::json!({"ok": true}))
}

pub async fn ingest_claude_hook(
    State(state): State<AppState>,
    Json(input): Json<ClaudeHookIngest>,
) -> Json<serde_json::Value> {
    if !source_ingest_enabled(&state, ToolKind::Claude).await {
        return disabled_source_response(ToolKind::Claude);
    }
    let event = input.event.unwrap_or_else(|| "notification".into());
    let pending = event.contains("permission");
    let workspace_path = input.cwd.unwrap_or_else(|| "~/.claude".into());
    let started_at = Utc::now().to_rfc3339();
    let last_activity_at = started_at.clone();
    let run_state = if pending {
        RunState::WaitingApproval
    } else if event.contains("idle") {
        RunState::Idle
    } else {
        RunState::Active
    };
    let transcript_path = input.transcript_path;
    let has_resume_id = input.session_id.as_deref().is_some_and(|id| !id.is_empty());
    let run = RunRecord {
        id: format!(
            "ingest-claude-{}",
            input.session_id.as_deref().unwrap_or("hook")
        ),
        tool: ToolKind::Claude,
        source_id: Some("claude:hook".into()),
        source_mode: "claude_hook".into(),
        project_name: last_path_component(&workspace_path)
            .unwrap_or("Claude Session")
            .into(),
        workspace_short: shorten_path(&workspace_path),
        model: input.model,
        provider: Some("claude".into()),
        agent_name: Some("hook".into()),
        agent_display_name: None,
        account_alias: Some("local-ingest".into()),
        auth_mode: Some("claude.ai".into()),
        auth_verified: true,
        session_id: input.session_id,
        thread_id: None,
        session_key: None,
        transcript_path: transcript_path.clone(),
        started_at: started_at.clone(),
        last_activity_at: last_activity_at.clone(),
        elapsed_ms: 0,
        state: run_state.clone(),
        last_action: Some(format!("hook event: {event}")),
        last_tail: Some("live hook ingest".into()),
        pending_approval: pending,
        first_question: None,
        last_question: None,
        error_message: None,
        message_count: 0,
        tokens: TokenUsage::default(),
        cost: MoneyValue {
            usd: None,
            confidence: SourceConfidence::Derived,
        },
        quota: default_quota(),
        source: live_source(),
        lifecycle: Some(live_lifecycle(
            run_state,
            &started_at,
            &last_activity_at,
            LifecycleStatusSource::Hook,
        )),
        usage_semantics: Some(usage_semantics(
            UsageCostKind::NotAvailable,
            UsageDataSource::Unknown,
        )),
        data_sources: Some(data_source_health(
            "claude:hook",
            DataSourceType::Hook,
            transcript_path,
            &last_activity_at,
        )),
        capabilities: Some(resume_capabilities(has_resume_id, false)),
        jump_targets: Some(Vec::new()),
        tool_specific: Some(serde_json::json!({ "event": event })),
        vcs: crate::commits::discover_vcs_context(&workspace_path),
        workspace_path,
        origin_label: None,
        origin_provider: None,
    };
    let run_id = run.id.clone();
    perf::log_ingest_event("claude_hook", &run_id);
    upsert_runtime_run(&state, run, "ingest_claude_hook").await;
    Json(serde_json::json!({"ok": true}))
}

pub async fn ingest_codex_hook(
    State(state): State<AppState>,
    Json(input): Json<CodexHookIngest>,
) -> Json<serde_json::Value> {
    if !source_ingest_enabled(&state, ToolKind::Codex).await {
        return disabled_source_response(ToolKind::Codex);
    }
    let pending = input.waiting_on_approval.unwrap_or(false);
    let event = input.event.unwrap_or_else(|| "hook".into());
    let workspace_path = input
        .cwd
        .as_deref()
        .map(crate::probe::resolve_worktree_cwd)
        .unwrap_or_else(|| "~/.codex".into());
    let started_at = Utc::now().to_rfc3339();
    let last_activity_at = started_at.clone();
    let run_state = if pending {
        RunState::WaitingApproval
    } else if event.contains("stop") {
        RunState::Completed
    } else {
        RunState::Active
    };
    let has_resume_id = input.thread_id.as_deref().is_some_and(|id| !id.is_empty());
    let run = RunRecord {
        id: format!(
            "ingest-codex-{}",
            input.thread_id.as_deref().unwrap_or("thread")
        ),
        tool: ToolKind::Codex,
        source_id: Some("codex:hook".into()),
        source_mode: "codex_hook".into(),
        project_name: last_path_component(&workspace_path)
            .unwrap_or("Codex Thread")
            .into(),
        workspace_short: shorten_path(&workspace_path),
        model: input.model,
        provider: Some("openai".into()),
        agent_name: Some("hook".into()),
        agent_display_name: None,
        account_alias: Some("local-ingest".into()),
        auth_mode: Some("configured".into()),
        auth_verified: true,
        session_id: None,
        thread_id: input.thread_id,
        session_key: None,
        transcript_path: None,
        started_at: started_at.clone(),
        last_activity_at: last_activity_at.clone(),
        elapsed_ms: 0,
        state: run_state.clone(),
        last_action: Some(format!("codex hook: {event}")),
        last_tail: Some("live hook ingest".into()),
        pending_approval: pending,
        first_question: None,
        last_question: None,
        error_message: None,
        message_count: 0,
        tokens: TokenUsage {
            total: input.total_tokens.unwrap_or(0),
            ..TokenUsage::default()
        },
        cost: MoneyValue {
            usd: None,
            confidence: SourceConfidence::Estimated,
        },
        quota: default_quota(),
        source: live_source(),
        lifecycle: Some(live_lifecycle(
            run_state,
            &started_at,
            &last_activity_at,
            LifecycleStatusSource::Hook,
        )),
        usage_semantics: Some(usage_semantics(
            UsageCostKind::Partial,
            UsageDataSource::Transcript,
        )),
        data_sources: Some(data_source_health(
            "codex:hook",
            DataSourceType::Hook,
            None,
            &last_activity_at,
        )),
        capabilities: Some(resume_capabilities(has_resume_id, true)),
        jump_targets: Some(Vec::new()),
        tool_specific: Some(serde_json::json!({ "event": event })),
        vcs: crate::commits::discover_vcs_context(&workspace_path),
        workspace_path,
        origin_label: None,
        origin_provider: None,
    };
    let run_id = run.id.clone();
    perf::log_ingest_event("codex_hook", &run_id);
    upsert_runtime_run(&state, run, "ingest_codex_hook").await;
    Json(serde_json::json!({"ok": true}))
}

async fn upsert_runtime_run(state: &AppState, mut run: RunRecord, wake_reason: &'static str) {
    let mut payload = state.bootstrap.write().await;
    if let Some(existing) = payload.runs.iter_mut().find(|item| item.id == run.id) {
        run.started_at = existing.started_at.clone();
        run.elapsed_ms = elapsed_from_timestamps(&run.started_at, &run.last_activity_at);
        *existing = run;
    } else {
        payload.runs.push(run);
    }
    payload
        .runs
        .sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    payload.generated_at = Utc::now().to_rfc3339();
    state.bump_revision();
    drop(payload);
    state.signal_change();
    state.mark_derive_dirty();
    state.wake_probe_with_reason(wake_reason);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;
    use octomonitor_core::{QuotaValue, SourceInfo};

    use super::*;
    use crate::{pricing::PricingStore, probe::empty_bootstrap};

    fn test_state() -> AppState {
        AppState::new(empty_bootstrap(), PricingStore::new())
    }

    fn sample_run(id: &str, state: RunState) -> RunRecord {
        let now = Utc::now().to_rfc3339();
        let pending_approval = matches!(state, RunState::WaitingApproval);
        RunRecord {
            id: id.into(),
            tool: ToolKind::Codex,
            source_mode: "test_ingest".into(),
            project_name: "octomonitor".into(),
            workspace_path: "/tmp/octomonitor".into(),
            workspace_short: "~/octomonitor".into(),
            model: Some("gpt-5".into()),
            provider: Some("openai".into()),
            agent_name: Some("hook".into()),
            agent_display_name: None,
            account_alias: Some("local-ingest".into()),
            auth_mode: Some("configured".into()),
            auth_verified: true,
            session_id: None,
            thread_id: Some(id.into()),
            session_key: None,
            transcript_path: None,
            started_at: now.clone(),
            last_activity_at: now.clone(),
            elapsed_ms: 0,
            state,
            last_action: Some("test event".into()),
            last_tail: Some("tail".into()),
            pending_approval,
            first_question: Some("first".into()),
            last_question: Some("last".into()),
            error_message: None,
            message_count: 1,
            tokens: TokenUsage {
                input: 100,
                output: 50,
                cache_read: 0,
                cache_write: 0,
                total: 150,
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
            source_id: Some("test:ingest".into()),
            lifecycle: Some(SessionLifecycle::default()),
            usage_semantics: Some(UsageSemantics {
                cost_kind: UsageCostKind::Estimated,
                source: UsageDataSource::Computed,
                enters_usage_totals: true,
                note: None,
            }),
            data_sources: Some(Vec::new()),
            capabilities: Some(Vec::new()),
            jump_targets: Some(Vec::new()),
            tool_specific: Some(serde_json::json!({})),
            vcs: None,
            origin_label: None,
            origin_provider: None,
        }
    }

    #[tokio::test]
    async fn upsert_runtime_run_leaves_derived_sections_untouched_until_worker_runs() {
        let state = test_state();

        upsert_runtime_run(
            &state,
            sample_run("thread-1", RunState::Active),
            "test_ingest",
        )
        .await;

        let payload = state.bootstrap.read().await;
        assert_eq!(payload.runs.len(), 1);
        assert!(payload.usage_buckets.is_empty());
        assert!(payload.attentions.is_empty());
        assert_eq!(state.current_revision(), 1);
    }

    #[tokio::test]
    async fn derive_worker_rebuilds_sections_after_runtime_upsert() {
        let state = test_state();
        crate::probe::spawn_derive_refresh(state.clone());

        upsert_runtime_run(
            &state,
            sample_run("thread-2", RunState::Error),
            "test_ingest",
        )
        .await;

        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let payload = state.bootstrap.read().await;
                let has_usage = !payload.usage_buckets.is_empty();
                let has_attention = !payload.attentions.is_empty();
                drop(payload);
                if has_usage && has_attention {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("derive worker should rebuild derived sections");

        let payload = state.bootstrap.read().await;
        assert_eq!(payload.runs.len(), 1);
        assert_eq!(payload.usage_buckets.len(), 1);
        assert_eq!(payload.attentions.len(), 1);
        assert_eq!(payload.attentions[0].run_id.as_deref(), Some("thread-2"));
    }

    #[tokio::test]
    async fn claude_statusline_ingest_preserves_rate_limit_usage() {
        let state = test_state();

        let _ = ingest_claude_statusline(
            State(state.clone()),
            Json(ClaudeStatuslineIngest {
                session_id: Some("session-1".into()),
                transcript_path: None,
                model: Some("claude-sonnet-4-5".into()),
                provider: Some("claude.ai".into()),
                project_name: Some("OctoMonitor".into()),
                workspace_path: Some("/tmp/octomonitor".into()),
                total_cost_usd: Some(1.23),
                input_tokens: Some(100),
                output_tokens: Some(50),
                total_tokens: Some(150),
                context_tokens: Some(25),
                five_hour_used_pct: Some(23.5),
                seven_day_used_pct: Some(41.2),
                five_hour_resets_at: Some(1_738_425_600),
                seven_day_resets_at: Some(1_738_857_600),
                pending_approval: Some(false),
                last_action: Some("statusline update".into()),
            }),
        )
        .await;

        let payload = state.bootstrap.read().await;
        let run = payload
            .runs
            .iter()
            .find(|run| run.id == "ingest-claude-session-1")
            .expect("statusline run");

        assert_eq!(run.quota.five_hour_used_pct, Some(23.5));
        assert_eq!(run.quota.seven_day_used_pct, Some(41.2));
        assert_eq!(run.quota.confidence, SourceConfidence::Official);
        assert_eq!(run.quota.reset_at.len(), 2);
    }

    fn empty_claude_statusline() -> ClaudeStatuslineIngest {
        ClaudeStatuslineIngest {
            session_id: None,
            transcript_path: None,
            model: None,
            provider: None,
            project_name: None,
            workspace_path: None,
            total_cost_usd: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            context_tokens: None,
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            five_hour_resets_at: None,
            seven_day_resets_at: None,
            pending_approval: None,
            last_action: None,
        }
    }

    #[tokio::test]
    async fn claude_statusline_with_missing_session_id_routes_to_fallback_run() {
        // External hooks sometimes omit session_id; that should still produce
        // a deterministic run id ("ingest-claude-unknown") so the next call
        // upserts rather than littering the store with duplicates.
        let state = test_state();
        let _ =
            ingest_claude_statusline(State(state.clone()), Json(empty_claude_statusline())).await;
        let _ =
            ingest_claude_statusline(State(state.clone()), Json(empty_claude_statusline())).await;

        let payload = state.bootstrap.read().await;
        let fallback_runs: Vec<_> = payload
            .runs
            .iter()
            .filter(|run| run.id == "ingest-claude-unknown")
            .collect();
        assert_eq!(
            fallback_runs.len(),
            1,
            "two empty statusline ingests should upsert into a single run, got {} (runs: {:?})",
            fallback_runs.len(),
            payload.runs.iter().map(|r| &r.id).collect::<Vec<_>>(),
        );
    }

    #[tokio::test]
    async fn claude_statusline_with_missing_workspace_falls_back_without_panic() {
        // Hook should never crash on a minimally-populated payload; missing
        // workspace_path should yield the documented `~/.claude` fallback.
        let state = test_state();
        let mut input = empty_claude_statusline();
        input.session_id = Some("solo".into());
        let _ = ingest_claude_statusline(State(state.clone()), Json(input)).await;

        let payload = state.bootstrap.read().await;
        let run = payload
            .runs
            .iter()
            .find(|r| r.id == "ingest-claude-solo")
            .expect("solo run should be present");
        assert_eq!(run.workspace_path, "~/.claude");
    }

    #[tokio::test]
    async fn claude_statusline_pending_approval_lands_waiting_state() {
        let state = test_state();
        let mut input = empty_claude_statusline();
        input.session_id = Some("waiting".into());
        input.pending_approval = Some(true);
        let _ = ingest_claude_statusline(State(state.clone()), Json(input)).await;

        let payload = state.bootstrap.read().await;
        let run = payload
            .runs
            .iter()
            .find(|r| r.id == "ingest-claude-waiting")
            .expect("waiting run");
        assert_eq!(run.state, RunState::WaitingApproval);
        assert!(run.pending_approval);
    }

    #[tokio::test]
    async fn codex_hook_marks_completed_on_stop_event() {
        let state = test_state();
        let _ = ingest_codex_hook(
            State(state.clone()),
            Json(CodexHookIngest {
                thread_id: Some("t-stop".into()),
                cwd: Some("/tmp/octomonitor".into()),
                model: Some("gpt-5".into()),
                event: Some("stop".into()),
                waiting_on_approval: None,
                total_tokens: Some(200),
            }),
        )
        .await;

        let payload = state.bootstrap.read().await;
        let run = payload
            .runs
            .iter()
            .find(|r| r.id == "ingest-codex-t-stop")
            .expect("codex stop run");
        assert_eq!(run.state, RunState::Completed);
        assert_eq!(run.tokens.total, 200);
    }

    #[tokio::test]
    async fn disabled_sources_ignore_hook_ingest() {
        let state = test_state();
        state
            .bootstrap
            .write()
            .await
            .config
            .disabled_sources
            .push(ToolKind::Codex);

        let response = ingest_codex_hook(
            State(state.clone()),
            Json(CodexHookIngest {
                thread_id: Some("disabled".into()),
                cwd: Some("/tmp/octomonitor".into()),
                model: Some("gpt-5".into()),
                event: Some("start".into()),
                waiting_on_approval: None,
                total_tokens: Some(200),
            }),
        )
        .await;

        assert_eq!(
            response.0.get("ignored").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert!(
            state.bootstrap.read().await.runs.is_empty(),
            "disabled source hook must not create runtime runs"
        );
    }
}
