use axum::{extract::State, Json};
use chrono::Utc;
use octomonitor_core::{
    Freshness, MoneyValue, RunRecord, RunState, SourceConfidence, SourceInfo, TokenUsage, ToolKind,
    WorkflowHint,
};
use serde::Deserialize;

use crate::platform::last_path_component;
use crate::probe::{elapsed_from_timestamps, rebuild_derived, shorten_path};
use crate::state::AppState;

fn default_quota() -> octomonitor_core::QuotaValue {
    octomonitor_core::QuotaValue {
        five_hour_used_pct: None,
        seven_day_used_pct: None,
        reset_at: vec![],
        confidence: SourceConfidence::Derived,
    }
}

fn build_workflow_hint(
    workflow_id: &Option<String>,
    step_id: &Option<String>,
    parent_step_id: &Option<String>,
    artifact_refs: &Option<Vec<String>>,
) -> Option<WorkflowHint> {
    if workflow_id.is_none() && step_id.is_none() {
        return None;
    }
    Some(WorkflowHint {
        workflow_id: workflow_id.clone(),
        step_id: step_id.clone(),
        parent_step_id: parent_step_id.clone(),
        artifact_refs: artifact_refs.clone().unwrap_or_default(),
        updated_at: Some(Utc::now().to_rfc3339()),
    })
}

fn live_source() -> SourceInfo {
    SourceInfo {
        confidence: SourceConfidence::Live,
        freshness: Freshness::Hot,
        last_updated_at: Utc::now().to_rfc3339(),
    }
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
    pub workflow_id: Option<String>,
    pub step_id: Option<String>,
    pub parent_step_id: Option<String>,
    pub artifact_refs: Option<Vec<String>>,
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
    pub workflow_id: Option<String>,
    pub step_id: Option<String>,
    pub parent_step_id: Option<String>,
    pub artifact_refs: Option<Vec<String>>,
}

pub async fn ingest_claude_statusline(
    State(state): State<AppState>,
    Json(input): Json<ClaudeStatuslineIngest>,
) -> Json<serde_json::Value> {
    let session_id = input.session_id;
    let workspace_path = input.workspace_path.unwrap_or_else(|| "~/.claude".into());
    let run = RunRecord {
        id: format!(
            "ingest-claude-{}",
            session_id.as_deref().unwrap_or("unknown")
        ),
        tool: ToolKind::Claude,
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
        quota: default_quota(),
        source: live_source(),
        vcs: crate::commits::discover_vcs_context(&workspace_path),
        workspace_path,
        origin_label: None,
        origin_provider: None,
        workflow_hint: None,
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
    let workspace_path = input.cwd.unwrap_or_else(|| "~/.claude".into());
    let wf_hint = build_workflow_hint(
        &input.workflow_id,
        &input.step_id,
        &input.parent_step_id,
        &input.artifact_refs,
    );
    let run = RunRecord {
        id: format!(
            "ingest-claude-{}",
            input.session_id.as_deref().unwrap_or("hook")
        ),
        tool: ToolKind::Claude,
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
        tokens: TokenUsage::default(),
        cost: MoneyValue {
            usd: None,
            confidence: SourceConfidence::Derived,
        },
        quota: default_quota(),
        source: live_source(),
        vcs: crate::commits::discover_vcs_context(&workspace_path),
        workspace_path,
        origin_label: None,
        origin_provider: None,
        workflow_hint: wf_hint.clone(),
    };
    let run_id = run.id.clone();
    upsert_runtime_run(&state, run).await;
    if let Some(hint) = wf_hint {
        notify_coordinator(&state, &run_id, &hint).await;
    }
    Json(serde_json::json!({"ok": true}))
}

pub async fn ingest_codex_hook(
    State(state): State<AppState>,
    Json(input): Json<CodexHookIngest>,
) -> Json<serde_json::Value> {
    let pending = input.waiting_on_approval.unwrap_or(false);
    let event = input.event.unwrap_or_else(|| "hook".into());
    let workspace_path = input
        .cwd
        .as_deref()
        .map(crate::probe::resolve_worktree_cwd)
        .unwrap_or_else(|| "~/.codex".into());
    let wf_hint = build_workflow_hint(
        &input.workflow_id,
        &input.step_id,
        &input.parent_step_id,
        &input.artifact_refs,
    );
    let run = RunRecord {
        id: format!(
            "ingest-codex-{}",
            input.thread_id.as_deref().unwrap_or("thread")
        ),
        tool: ToolKind::Codex,
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
            total: input.total_tokens.unwrap_or(0),
            ..TokenUsage::default()
        },
        cost: MoneyValue {
            usd: None,
            confidence: SourceConfidence::Estimated,
        },
        quota: default_quota(),
        source: live_source(),
        vcs: crate::commits::discover_vcs_context(&workspace_path),
        workspace_path,
        origin_label: None,
        origin_provider: None,
        workflow_hint: wf_hint.clone(),
    };
    let run_id = run.id.clone();
    upsert_runtime_run(&state, run).await;
    if let Some(hint) = wf_hint {
        notify_coordinator(&state, &run_id, &hint).await;
    }
    Json(serde_json::json!({"ok": true}))
}

async fn upsert_runtime_run(state: &AppState, mut run: RunRecord) {
    let mut payload = state.bootstrap.write().await;
    if let Some(existing) = payload.runs.iter_mut().find(|item| item.id == run.id) {
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
    state.wake_probe();
}

/// When an ingest event carries workflow hints, attempt to auto-link the run to the matching step.
async fn notify_coordinator(state: &AppState, run_id: &str, hint: &WorkflowHint) {
    let wf_id = match &hint.workflow_id {
        Some(w) => w.clone(),
        None => return,
    };

    // If we have both workflow_id and step_id, do a direct link (fast path)
    if let Some(step_id) = &hint.step_id {
        let coord = state.workflow_coordinator.lock().await;
        if let Ok(runs) = coord.list_runs_for_def(&wf_id) {
            for wr_id in runs {
                let _ = coord.link_run(
                    &wr_id,
                    step_id,
                    run_id,
                    octomonitor_core::workflow::LinkConfidence::Explicit,
                    "ingest-hint-explicit",
                );
            }
        }
        return;
    }

    // workflow_id present but no step_id — use resolve_strong / resolve_prompt_marker
    // to find the right step by scanning the run record
    let payload = state.bootstrap.read().await;
    let monitor_run = match payload.runs.iter().find(|r| r.id == run_id) {
        Some(r) => r,
        None => return,
    };

    let coord = state.workflow_coordinator.lock().await;
    let wr_ids = match coord.list_runs_for_def(&wf_id) {
        Ok(ids) => ids,
        Err(_) => return,
    };

    for wr_id in wr_ids {
        let wf_run = match coord.store().load_run(&wr_id) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for step in &wf_run.steps {
            // Try strong match (context-file with artifacts)
            if let Some(linked) =
                crate::workflows::link_resolver::resolve_strong(monitor_run, step, &wf_id)
            {
                let _ = coord.link_run(
                    &wr_id,
                    &step.step_id,
                    run_id,
                    linked.confidence,
                    &linked.matched_by,
                );
                return;
            }
            // Try prompt marker match
            if let Some(linked) =
                crate::workflows::link_resolver::resolve_prompt_marker(monitor_run, step, &wf_id)
            {
                let _ = coord.link_run(
                    &wr_id,
                    &step.step_id,
                    run_id,
                    linked.confidence,
                    &linked.matched_by,
                );
                return;
            }
        }
    }
}
