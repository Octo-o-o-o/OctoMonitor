use axum::{extract::State, Json};
use chrono::Utc;
use octomonitor_core::{
    Freshness, MoneyValue, RunRecord, RunState, SourceConfidence, SourceInfo, TokenUsage, ToolKind,
};
use serde::Deserialize;

use crate::perf;
use crate::platform::last_path_component;
use crate::probe::{elapsed_from_timestamps, shorten_path};
use crate::state::AppState;

fn default_quota() -> octomonitor_core::QuotaValue {
    octomonitor_core::QuotaValue {
        five_hour_used_pct: None,
        seven_day_used_pct: None,
        reset_at: vec![],
        confidence: SourceConfidence::Derived,
    }
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
    };
    perf::log_ingest_event("claude_statusline", &run.id);
    upsert_runtime_run(&state, run, "ingest_claude_statusline").await;
    Json(serde_json::json!({"ok": true}))
}

pub async fn ingest_claude_hook(
    State(state): State<AppState>,
    Json(input): Json<ClaudeHookIngest>,
) -> Json<serde_json::Value> {
    let event = input.event.unwrap_or_else(|| "notification".into());
    let pending = event.contains("permission");
    let workspace_path = input.cwd.unwrap_or_else(|| "~/.claude".into());
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
    let pending = input.waiting_on_approval.unwrap_or(false);
    let event = input.event.unwrap_or_else(|| "hook".into());
    let workspace_path = input
        .cwd
        .as_deref()
        .map(crate::probe::resolve_worktree_cwd)
        .unwrap_or_else(|| "~/.codex".into());
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
}
