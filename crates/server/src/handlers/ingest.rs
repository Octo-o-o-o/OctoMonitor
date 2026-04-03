use axum::{extract::State, Json};
use chrono::Utc;
use octomonitor_core::{
    Freshness, MoneyValue, RunRecord, RunState, SourceConfidence, SourceInfo, TokenUsage, ToolKind,
};
use serde::Deserialize;

use crate::platform::last_path_component;
use crate::probe::{elapsed_from_timestamps, rebuild_derived, shorten_path};
use crate::state::AppState;

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
    let run = RunRecord {
        id: format!(
            "ingest-claude-{}",
            input.session_id.clone().unwrap_or_else(|| "unknown".into())
        ),
        tool: ToolKind::Claude,
        source_mode: "claude_statusline".into(),
        project_name: input
            .project_name
            .unwrap_or_else(|| "Claude Session".into()),
        workspace_path: input
            .workspace_path
            .clone()
            .unwrap_or_else(|| "~/.claude".into()),
        workspace_short: shorten_path(input.workspace_path.as_deref().unwrap_or("~/.claude")),
        model: input.model,
        provider: input.provider,
        agent_name: Some("live-ingest".into()),
        agent_display_name: None,
        account_alias: Some("local-ingest".into()),
        auth_mode: Some("claude.ai".into()),
        auth_verified: true,
        session_id: input.session_id.clone(),
        thread_id: None,
        session_key: None,
        transcript_path: input.transcript_path,
        started_at: Utc::now().to_rfc3339(),
        last_activity_at: Utc::now().to_rfc3339(),
        elapsed_ms: 0,
        state: if input.pending_approval.unwrap_or(false) {
            RunState::WaitingApproval
        } else {
            RunState::Active
        },
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
        quota: octomonitor_core::QuotaValue {
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            reset_at: vec![],
            confidence: SourceConfidence::Derived,
        },
        source: SourceInfo {
            confidence: SourceConfidence::Live,
            freshness: Freshness::Hot,
            last_updated_at: Utc::now().to_rfc3339(),
        },
        vcs: input
            .workspace_path
            .as_deref()
            .and_then(crate::commits::discover_vcs_context),
        origin_label: None,
        origin_provider: None,
    };
    upsert_runtime_run(&state, run).await;
    Json(serde_json::json!({"ok": true}))
}

pub async fn ingest_claude_hook(
    State(state): State<AppState>,
    Json(input): Json<ClaudeHookIngest>,
) -> Json<serde_json::Value> {
    let event = input.event.unwrap_or_else(|| "notification".into());
    let pending = event.contains("permission");
    let run = RunRecord {
        id: format!(
            "ingest-claude-{}",
            input.session_id.clone().unwrap_or_else(|| "hook".into())
        ),
        tool: ToolKind::Claude,
        source_mode: "claude_hook".into(),
        project_name: input
            .cwd
            .as_ref()
            .and_then(|cwd| last_path_component(cwd))
            .unwrap_or("Claude Session")
            .into(),
        workspace_path: input.cwd.clone().unwrap_or_else(|| "~/.claude".into()),
        workspace_short: shorten_path(input.cwd.as_deref().unwrap_or("~/.claude")),
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
        transcript_path: input.transcript_path,
        started_at: Utc::now().to_rfc3339(),
        last_activity_at: Utc::now().to_rfc3339(),
        elapsed_ms: 0,
        state: if pending {
            RunState::WaitingApproval
        } else if event.contains("idle") {
            RunState::Idle
        } else {
            RunState::Active
        },
        last_action: Some(format!("hook event: {event}")),
        last_tail: Some("live hook ingest".into()),
        pending_approval: pending,
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
            confidence: SourceConfidence::Derived,
        },
        quota: octomonitor_core::QuotaValue {
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            reset_at: vec![],
            confidence: SourceConfidence::Derived,
        },
        source: SourceInfo {
            confidence: SourceConfidence::Live,
            freshness: Freshness::Hot,
            last_updated_at: Utc::now().to_rfc3339(),
        },
        vcs: input
            .cwd
            .as_deref()
            .and_then(crate::commits::discover_vcs_context),
        origin_label: None,
        origin_provider: None,
    };
    upsert_runtime_run(&state, run).await;
    Json(serde_json::json!({"ok": true}))
}

pub async fn ingest_codex_hook(
    State(state): State<AppState>,
    Json(input): Json<CodexHookIngest>,
) -> Json<serde_json::Value> {
    let pending = input.waiting_on_approval.unwrap_or(false);
    let event = input.event.unwrap_or_else(|| "hook".into());
    let resolved_cwd = input.cwd.as_deref().map(crate::probe::resolve_worktree_cwd);
    let workspace_path = resolved_cwd.clone().unwrap_or_else(|| "~/.codex".into());
    let run = RunRecord {
        id: format!(
            "ingest-codex-{}",
            input.thread_id.clone().unwrap_or_else(|| "thread".into())
        ),
        tool: ToolKind::Codex,
        source_mode: "codex_hook".into(),
        project_name: resolved_cwd
            .as_deref()
            .and_then(last_path_component)
            .unwrap_or("Codex Thread")
            .into(),
        workspace_path: workspace_path.clone(),
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
        started_at: Utc::now().to_rfc3339(),
        last_activity_at: Utc::now().to_rfc3339(),
        elapsed_ms: 0,
        state: if pending {
            RunState::WaitingApproval
        } else if event.contains("stop") {
            RunState::Completed
        } else {
            RunState::Active
        },
        last_action: Some(format!("codex hook: {event}")),
        last_tail: Some("live hook ingest".into()),
        pending_approval: pending,
        first_question: None,
        last_question: None,
        error_message: None,
        message_count: 0,
        tokens: TokenUsage {
            input: 0,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            total: input.total_tokens.unwrap_or(0),
            context: 0,
        },
        cost: MoneyValue {
            usd: None,
            confidence: SourceConfidence::Estimated,
        },
        quota: octomonitor_core::QuotaValue {
            five_hour_used_pct: None,
            seven_day_used_pct: None,
            reset_at: vec![],
            confidence: SourceConfidence::Derived,
        },
        source: SourceInfo {
            confidence: SourceConfidence::Live,
            freshness: Freshness::Hot,
            last_updated_at: Utc::now().to_rfc3339(),
        },
        vcs: input
            .cwd
            .as_deref()
            .and_then(crate::commits::discover_vcs_context),
        origin_label: None,
        origin_provider: None,
    };
    upsert_runtime_run(&state, run).await;
    Json(serde_json::json!({"ok": true}))
}

async fn upsert_runtime_run(state: &AppState, mut run: RunRecord) {
    let mut payload = state.bootstrap.write().await;
    if let Some(existing) = payload.runs.iter_mut().find(|item| item.id == run.id) {
        // Preserve the original start time across updates
        run.started_at = existing.started_at.clone();
        run.elapsed_ms = elapsed_from_timestamps(&run.started_at, &run.last_activity_at);
        *existing = run;
    } else {
        payload.runs.push(run);
    }
    rebuild_derived(&mut payload, &state.pricing);
    payload.generated_at = Utc::now().to_rfc3339();
    drop(payload);
    state.signal_change();
    // Wake the probe loop so it picks up fresh adapter data soon
    // (e.g. updated quota after a new session starts).
    state.wake_probe();
}
