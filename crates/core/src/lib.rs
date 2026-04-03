use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolKind {
    Claude,
    Codex,
    OpenClaw,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RunState {
    Active,
    WaitingApproval,
    Idle,
    Completed,
    Error,
    Stale,
    GatewayOffline,
    LimitExceeded,
    ContextExceeded,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceConfidence {
    Official,
    Live,
    Derived,
    Estimated,
    Heuristic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Freshness {
    Hot,
    Warm,
    Stale,
    Cold,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TokenUsage {
    #[ts(type = "number")]
    pub input: u64,
    #[ts(type = "number")]
    pub output: u64,
    #[ts(type = "number")]
    pub cache_read: u64,
    #[ts(type = "number")]
    pub cache_write: u64,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub context: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MoneyValue {
    pub usd: Option<f64>,
    pub confidence: SourceConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuotaValue {
    pub five_hour_used_pct: Option<f64>,
    pub seven_day_used_pct: Option<f64>,
    pub reset_at: Vec<String>,
    pub confidence: SourceConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceInfo {
    pub confidence: SourceConfidence,
    pub freshness: Freshness,
    pub last_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VcsContext {
    pub repo_id: String,
    pub repo_name: String,
    pub repo_root: String,
    pub worktree_id: Option<String>,
    pub worktree_name: Option<String>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub confidence: SourceConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunRecord {
    pub id: String,
    pub tool: ToolKind,
    pub source_mode: String,
    pub project_name: String,
    pub workspace_path: String,
    pub workspace_short: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_name: Option<String>,
    pub agent_display_name: Option<String>,
    pub account_alias: Option<String>,
    pub auth_mode: Option<String>,
    pub auth_verified: bool,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub session_key: Option<String>,
    pub transcript_path: Option<String>,
    pub started_at: String,
    pub last_activity_at: String,
    #[ts(type = "number")]
    pub elapsed_ms: i64,
    pub state: RunState,
    pub last_action: Option<String>,
    pub last_tail: Option<String>,
    pub pending_approval: bool,
    pub first_question: Option<String>,
    pub last_question: Option<String>,
    pub error_message: Option<String>,
    #[ts(type = "number")]
    pub message_count: u64,
    pub tokens: TokenUsage,
    pub cost: MoneyValue,
    pub quota: QuotaValue,
    pub source: SourceInfo,
    pub vcs: Option<VcsContext>,
    /// Human-readable session origin, e.g. "Telegram: Yixiao Wang", "Cron: AI Daily Brief"
    pub origin_label: Option<String>,
    /// Source provider type, e.g. "telegram", "cron", "heartbeat"
    pub origin_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PendingCron {
    pub id: String,
    pub name: String,
    pub agent_id: Option<String>,
    pub schedule_expr: String,
    pub schedule_tz: String,
    pub schedule_human: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AttentionItem {
    pub id: String,
    pub tool: ToolKind,
    pub run_id: Option<String>,
    pub severity: String,
    pub kind: String,
    pub title: String,
    pub detail: Option<String>,
    pub since: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageBucket {
    #[ts(type = "Record<string, string>")]
    pub scope: serde_json::Value,
    pub window: String,
    pub start: String,
    pub end: String,
    #[ts(type = "number")]
    pub input_tokens: u64,
    #[ts(type = "number")]
    pub output_tokens: u64,
    #[ts(type = "number")]
    pub cache_read_tokens: u64,
    #[ts(type = "number")]
    pub cache_write_tokens: u64,
    #[ts(type = "number")]
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub confidence: SourceConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IdentityState {
    pub tool: ToolKind,
    pub auth_mode: Option<String>,
    pub provider: Option<String>,
    pub account_alias: Option<String>,
    pub fingerprint: Option<String>,
    pub auth_age: Option<String>,
    pub verified: bool,
    pub configured: bool,
    pub source: SourceConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdapterHealth {
    pub tool: ToolKind,
    pub mode: String,
    pub online: bool,
    pub last_success_at: Option<String>,
    pub last_error_at: Option<String>,
    pub last_error: Option<String>,
    pub freshness: Freshness,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompletionRecord {
    pub id: String,
    pub tool: ToolKind,
    pub project_name: String,
    pub title: String,
    pub finished_at: String,
    #[ts(type = "number")]
    pub duration_ms: i64,
    #[ts(type = "number | undefined")]
    pub total_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub state: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CommitAttributionMethod {
    ReadOnlyHeuristic,
    HookExact,
    HookRewrite,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitSourceStat {
    pub tool: ToolKind,
    #[ts(type = "number")]
    pub run_count: u64,
    #[ts(type = "number")]
    pub attributed_tokens: u64,
    pub attributed_cost_usd: Option<f64>,
    pub confidence: SourceConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitAttributionLink {
    pub run_id: String,
    pub tool: ToolKind,
    pub source_mode: String,
    pub project_name: String,
    pub session_label: String,
    #[ts(type = "number")]
    pub score: f64,
    #[ts(type = "number")]
    pub allocated_tokens: u64,
    pub allocated_cost_usd: Option<f64>,
    pub confidence: SourceConfidence,
    pub method: CommitAttributionMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitRecord {
    pub id: String,
    pub repo_id: String,
    pub repo_name: String,
    pub repo_root: String,
    pub worktree_id: Option<String>,
    pub worktree_name: Option<String>,
    pub sha: String,
    pub short_sha: String,
    pub author_name: String,
    pub committed_at: String,
    pub summary: String,
    #[ts(type = "number")]
    pub files_changed: u64,
    #[ts(type = "number")]
    pub insertions: u64,
    #[ts(type = "number")]
    pub deletions: u64,
    #[ts(type = "number")]
    pub attributed_tokens: u64,
    pub attributed_cost_usd: Option<f64>,
    #[ts(type = "number")]
    pub run_count: u64,
    #[ts(type = "number")]
    pub source_count: u64,
    pub confidence: SourceConfidence,
    pub method: CommitAttributionMethod,
    pub sources: Vec<CommitSourceStat>,
    pub links: Vec<CommitAttributionLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppConfig {
    pub listen_host: String,
    pub listen_port: u16,
    pub history_days: u8,
    pub companion_enabled: bool,
    pub local_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RemoteAccessMode {
    Off,
    LanViewer,
    PrivateNetwork,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdvertisedAddress {
    pub kind: String,
    pub label: String,
    pub host: String,
    pub url: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteDevice {
    pub id: String,
    pub label: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemotePairingCode {
    pub id: String,
    pub code: String,
    pub label: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteAccessState {
    pub enabled: bool,
    pub mode: RemoteAccessMode,
    pub listener_host: String,
    pub listener_port: u16,
    pub addresses: Vec<AdvertisedAddress>,
    pub devices: Vec<RemoteDevice>,
    pub pending_pairings: Vec<RemotePairingCode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BootstrapPayload {
    pub generated_at: String,
    pub runs: Vec<RunRecord>,
    pub attentions: Vec<AttentionItem>,
    pub usage_buckets: Vec<UsageBucket>,
    pub commits: Vec<CommitRecord>,
    pub identities: Vec<IdentityState>,
    pub adapter_health: Vec<AdapterHealth>,
    pub recent_completions: Vec<CompletionRecord>,
    pub pending_crons: Vec<PendingCron>,
    pub config: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryRange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageHistoryPayload {
    pub generated_at: String,
    pub range: HistoryRange,
    pub truncated: bool,
    pub runs: Vec<RunRecord>,
    pub usage_buckets: Vec<UsageBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitHistoryPayload {
    pub generated_at: String,
    pub range: HistoryRange,
    pub truncated: bool,
    pub runs: Vec<RunRecord>,
    pub commits: Vec<CommitRecord>,
}

pub fn classify_freshness(seconds: i64) -> Freshness {
    match seconds {
        0..=3 => Freshness::Hot,
        4..=15 => Freshness::Warm,
        16..=60 => Freshness::Stale,
        _ => Freshness::Cold,
    }
}

#[cfg(test)]
pub fn demo_bootstrap() -> BootstrapPayload {
    use chrono::{Duration, Utc};
    let now = Utc::now();
    let started = now - Duration::minutes(37);
    let last = now - Duration::seconds(4);
    let now_s = now.to_rfc3339();
    BootstrapPayload {
        generated_at: now_s.clone(),
        runs: vec![
            RunRecord {
                id: "claude-run-1".into(),
                tool: ToolKind::Claude,
                source_mode: "claude_statusline".into(),
                project_name: "OctoMonitor".into(),
                workspace_path: "~/workspace/octomonitor".into(),
                workspace_short: "~/WS/OctoMonitor".into(),
                model: Some("claude-sonnet-4.5".into()),
                provider: Some("claude.ai".into()),
                agent_name: Some("Athena".into()),
                agent_display_name: None,
                account_alias: Some("work-claude".into()),
                auth_mode: Some("claude.ai".into()),
                auth_verified: true,
                session_id: Some("sess_claude_1".into()),
                thread_id: None,
                session_key: None,
                transcript_path: Some("~/.claude/transcripts/octomonitor.jsonl".into()),
                started_at: started.to_rfc3339(),
                last_activity_at: last.to_rfc3339(),
                elapsed_ms: (now - started).num_milliseconds(),
                state: RunState::Active,
                last_action: Some("Editing Axum server routes".into()),
                last_tail: Some("Added config and pairing endpoints".into()),
                pending_approval: false,
                first_question: Some("Help me edit the Axum server routes".into()),
                last_question: Some("Add config and pairing endpoints".into()),
                error_message: None,
                message_count: 15,
                tokens: TokenUsage {
                    input: 182_000,
                    output: 24_100,
                    cache_read: 12_000,
                    cache_write: 1_500,
                    total: 219_600,
                    context: 81_000,
                },
                cost: MoneyValue {
                    usd: Some(12.42),
                    confidence: SourceConfidence::Live,
                },
                quota: QuotaValue {
                    five_hour_used_pct: Some(64.0),
                    seven_day_used_pct: Some(28.0),
                    reset_at: vec![(now + Duration::hours(2)).to_rfc3339()],
                    confidence: SourceConfidence::Official,
                },
                source: SourceInfo {
                    confidence: SourceConfidence::Live,
                    freshness: classify_freshness((now - last).num_seconds()),
                    last_updated_at: now_s.clone(),
                },
                vcs: Some(VcsContext {
                    repo_id: "repo-octomonitor".into(),
                    repo_name: "OctoMonitor".into(),
                    repo_root: "/Users/demo/workspace/octomonitor".into(),
                    worktree_id: Some("wt-octomonitor-main".into()),
                    worktree_name: Some("octomonitor".into()),
                    worktree_path: Some("/Users/demo/workspace/octomonitor".into()),
                    branch: Some("main".into()),
                    confidence: SourceConfidence::Derived,
                }),
                origin_label: None,
                origin_provider: None,
            },
            RunRecord {
                id: "codex-run-1".into(),
                tool: ToolKind::Codex,
                source_mode: "codex_app_server".into(),
                project_name: "Readless".into(),
                workspace_path: "~/workspace/readless".into(),
                workspace_short: "~/WS/readless".into(),
                model: Some("gpt-5-codex".into()),
                provider: Some("openai".into()),
                agent_name: Some("Codex".into()),
                agent_display_name: None,
                account_alias: Some("openai-main".into()),
                auth_mode: Some("chatgpt".into()),
                auth_verified: true,
                session_id: None,
                thread_id: Some("thread_123".into()),
                session_key: None,
                transcript_path: Some("~/.codex/history.jsonl".into()),
                started_at: (now - Duration::minutes(12)).to_rfc3339(),
                last_activity_at: (now - Duration::seconds(18)).to_rfc3339(),
                elapsed_ms: Duration::minutes(12).num_milliseconds(),
                state: RunState::WaitingApproval,
                last_action: Some("Waiting on Bash approval".into()),
                last_tail: Some("wants to run pnpm build".into()),
                pending_approval: true,
                first_question: Some("Build the readless frontend".into()),
                last_question: Some("Run pnpm build for production".into()),
                error_message: None,
                message_count: 8,
                tokens: TokenUsage {
                    input: 48_000,
                    output: 9_500,
                    cache_read: 0,
                    cache_write: 0,
                    total: 57_500,
                    context: 20_000,
                },
                cost: MoneyValue {
                    usd: None,
                    confidence: SourceConfidence::Estimated,
                },
                quota: QuotaValue {
                    five_hour_used_pct: None,
                    seven_day_used_pct: Some(41.0),
                    reset_at: vec![],
                    confidence: SourceConfidence::Derived,
                },
                source: SourceInfo {
                    confidence: SourceConfidence::Live,
                    freshness: classify_freshness(18),
                    last_updated_at: now_s.clone(),
                },
                vcs: Some(VcsContext {
                    repo_id: "repo-readless".into(),
                    repo_name: "Readless".into(),
                    repo_root: "/Users/demo/workspace/readless".into(),
                    worktree_id: Some("wt-readless-feature".into()),
                    worktree_name: Some("feature-build".into()),
                    worktree_path: Some("/Users/demo/workspace/readless-feature-build".into()),
                    branch: Some("feature/build".into()),
                    confidence: SourceConfidence::Derived,
                }),
                origin_label: None,
                origin_provider: None,
            },
            RunRecord {
                id: "openclaw-run-1".into(),
                tool: ToolKind::OpenClaw,
                source_mode: "openclaw_gateway".into(),
                project_name: "Alice".into(),
                workspace_path: "~/.openclaw/workspace".into(),
                workspace_short: "~/.openclaw/workspace".into(),
                model: Some("gpt-5.4".into()),
                provider: Some("openai-codex".into()),
                agent_name: Some("Iris".into()),
                agent_display_name: Some("Iris".into()),
                account_alias: Some("codex-oauth".into()),
                auth_mode: Some("oauth".into()),
                auth_verified: true,
                session_id: None,
                thread_id: None,
                session_key: Some("session-dev-42".into()),
                transcript_path: Some(
                    "~/.openclaw/agents/dev/sessions/session-dev-42.jsonl".into(),
                ),
                started_at: (now - Duration::minutes(52)).to_rfc3339(),
                last_activity_at: (now - Duration::seconds(2)).to_rfc3339(),
                elapsed_ms: Duration::minutes(52).num_milliseconds(),
                state: RunState::Active,
                last_action: Some("Gateway stream healthy".into()),
                last_tail: Some("3 background processes active".into()),
                pending_approval: false,
                first_question: None,
                last_question: None,
                error_message: None,
                message_count: 0,
                tokens: TokenUsage {
                    input: 96_000,
                    output: 13_800,
                    cache_read: 8_600,
                    cache_write: 2_400,
                    total: 120_800,
                    context: 64_000,
                },
                cost: MoneyValue {
                    usd: Some(4.83),
                    confidence: SourceConfidence::Derived,
                },
                quota: QuotaValue {
                    five_hour_used_pct: None,
                    seven_day_used_pct: Some(19.0),
                    reset_at: vec![(now + Duration::hours(7)).to_rfc3339()],
                    confidence: SourceConfidence::Official,
                },
                source: SourceInfo {
                    confidence: SourceConfidence::Official,
                    freshness: classify_freshness(2),
                    last_updated_at: now_s.clone(),
                },
                vcs: Some(VcsContext {
                    repo_id: "repo-openclaw-workspace".into(),
                    repo_name: "workspace".into(),
                    repo_root: "/Users/demo/.openclaw/workspace".into(),
                    worktree_id: Some("wt-openclaw-main".into()),
                    worktree_name: Some("workspace".into()),
                    worktree_path: Some("/Users/demo/.openclaw/workspace".into()),
                    branch: Some("main".into()),
                    confidence: SourceConfidence::Derived,
                }),
                origin_label: Some("Telegram: Yixiao".into()),
                origin_provider: Some("telegram".into()),
            },
        ],
        attentions: vec![
            AttentionItem {
                id: "attention-1".into(),
                tool: ToolKind::Codex,
                run_id: Some("codex-run-1".into()),
                severity: "warn".into(),
                kind: "permission".into(),
                title: "Codex waiting on approval".into(),
                detail: Some("Approve workspace build command".into()),
                since: (now - Duration::seconds(18)).to_rfc3339(),
            },
            AttentionItem {
                id: "attention-2".into(),
                tool: ToolKind::Claude,
                run_id: Some("claude-run-1".into()),
                severity: "info".into(),
                kind: "source".into(),
                title: "Claude quota window warming".into(),
                detail: Some("5h window at 64%".into()),
                since: now.to_rfc3339(),
            },
        ],
        usage_buckets: vec![
            UsageBucket {
                scope: serde_json::json!({"runId":"claude-run-1","tool":"claude"}),
                window: "today".into(),
                start: (now - Duration::hours(12)).to_rfc3339(),
                end: now.to_rfc3339(),
                input_tokens: 182_000,
                output_tokens: 24_100,
                cache_read_tokens: 12_000,
                cache_write_tokens: 1_500,
                total_tokens: 219_600,
                cost_usd: Some(12.42),
                confidence: SourceConfidence::Live,
            },
            UsageBucket {
                scope: serde_json::json!({"runId":"codex-run-1","tool":"codex"}),
                window: "today".into(),
                start: (now - Duration::hours(12)).to_rfc3339(),
                end: now.to_rfc3339(),
                input_tokens: 48_000,
                output_tokens: 9_500,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: 57_500,
                cost_usd: None,
                confidence: SourceConfidence::Estimated,
            },
            UsageBucket {
                scope: serde_json::json!({"runId":"openclaw-run-1","tool":"openClaw"}),
                window: "today".into(),
                start: (now - Duration::hours(12)).to_rfc3339(),
                end: now.to_rfc3339(),
                input_tokens: 96_000,
                output_tokens: 13_800,
                cache_read_tokens: 8_600,
                cache_write_tokens: 2_400,
                total_tokens: 120_800,
                cost_usd: Some(4.83),
                confidence: SourceConfidence::Official,
            },
        ],
        commits: vec![
            CommitRecord {
                id: "repo-octomonitor:4f3c2a1b7e4c9f9d0b2a4f7f2ce8130b4d11aa10".into(),
                repo_id: "repo-octomonitor".into(),
                repo_name: "OctoMonitor".into(),
                repo_root: "/Users/demo/workspace/octomonitor".into(),
                worktree_id: Some("wt-octomonitor-main".into()),
                worktree_name: Some("octomonitor".into()),
                sha: "4f3c2a1b7e4c9f9d0b2a4f7f2ce8130b4d11aa10".into(),
                short_sha: "4f3c2a1".into(),
                author_name: "Yixiao Wang".into(),
                committed_at: (now - Duration::minutes(3)).to_rfc3339(),
                summary: "Add commit attribution and worktree-aware VCS context".into(),
                files_changed: 6,
                insertions: 214,
                deletions: 29,
                attributed_tokens: 188_400,
                attributed_cost_usd: Some(10.97),
                run_count: 2,
                source_count: 2,
                confidence: SourceConfidence::Heuristic,
                method: CommitAttributionMethod::ReadOnlyHeuristic,
                sources: vec![
                    CommitSourceStat {
                        tool: ToolKind::Claude,
                        run_count: 1,
                        attributed_tokens: 151_200,
                        attributed_cost_usd: Some(9.41),
                        confidence: SourceConfidence::Heuristic,
                    },
                    CommitSourceStat {
                        tool: ToolKind::Codex,
                        run_count: 1,
                        attributed_tokens: 37_200,
                        attributed_cost_usd: Some(1.56),
                        confidence: SourceConfidence::Heuristic,
                    },
                ],
                links: vec![
                    CommitAttributionLink {
                        run_id: "claude-run-1".into(),
                        tool: ToolKind::Claude,
                        source_mode: "claude_statusline".into(),
                        project_name: "OctoMonitor".into(),
                        session_label: "OctoMonitor".into(),
                        score: 0.78,
                        allocated_tokens: 151_200,
                        allocated_cost_usd: Some(9.41),
                        confidence: SourceConfidence::Heuristic,
                        method: CommitAttributionMethod::ReadOnlyHeuristic,
                    },
                    CommitAttributionLink {
                        run_id: "codex-run-1".into(),
                        tool: ToolKind::Codex,
                        source_mode: "codex_app_server".into(),
                        project_name: "Readless".into(),
                        session_label: "Readless".into(),
                        score: 0.22,
                        allocated_tokens: 37_200,
                        allocated_cost_usd: Some(1.56),
                        confidence: SourceConfidence::Heuristic,
                        method: CommitAttributionMethod::ReadOnlyHeuristic,
                    },
                ],
            },
            CommitRecord {
                id: "repo-readless:8a1d9c365bf5f757d967e3b8a751687a1f69bb7d".into(),
                repo_id: "repo-readless".into(),
                repo_name: "Readless".into(),
                repo_root: "/Users/demo/workspace/readless".into(),
                worktree_id: Some("wt-readless-feature".into()),
                worktree_name: Some("feature-build".into()),
                sha: "8a1d9c365bf5f757d967e3b8a751687a1f69bb7d".into(),
                short_sha: "8a1d9c3".into(),
                author_name: "Yixiao Wang".into(),
                committed_at: (now - Duration::minutes(16)).to_rfc3339(),
                summary: "Prepare production build script and approval flow".into(),
                files_changed: 3,
                insertions: 48,
                deletions: 11,
                attributed_tokens: 20_300,
                attributed_cost_usd: Some(0.84),
                run_count: 1,
                source_count: 1,
                confidence: SourceConfidence::Heuristic,
                method: CommitAttributionMethod::ReadOnlyHeuristic,
                sources: vec![CommitSourceStat {
                    tool: ToolKind::Codex,
                    run_count: 1,
                    attributed_tokens: 20_300,
                    attributed_cost_usd: Some(0.84),
                    confidence: SourceConfidence::Heuristic,
                }],
                links: vec![CommitAttributionLink {
                    run_id: "codex-run-1".into(),
                    tool: ToolKind::Codex,
                    source_mode: "codex_app_server".into(),
                    project_name: "Readless".into(),
                    session_label: "Readless".into(),
                    score: 0.91,
                    allocated_tokens: 20_300,
                    allocated_cost_usd: Some(0.84),
                    confidence: SourceConfidence::Heuristic,
                    method: CommitAttributionMethod::ReadOnlyHeuristic,
                }],
            },
        ],
        identities: vec![
            IdentityState {
                tool: ToolKind::Claude,
                auth_mode: Some("claude.ai".into()),
                provider: Some("claude.ai".into()),
                account_alias: Some("work-claude".into()),
                fingerprint: None,
                auth_age: Some("2d".into()),
                verified: true,
                configured: true,
                source: SourceConfidence::Official,
            },
            IdentityState {
                tool: ToolKind::Codex,
                auth_mode: Some("chatgpt".into()),
                provider: Some("openai".into()),
                account_alias: Some("openai-main".into()),
                fingerprint: None,
                auth_age: Some("5d".into()),
                verified: true,
                configured: true,
                source: SourceConfidence::Live,
            },
            IdentityState {
                tool: ToolKind::OpenClaw,
                auth_mode: Some("oauth".into()),
                provider: Some("openai-codex".into()),
                account_alias: Some("codex-oauth".into()),
                fingerprint: None,
                auth_age: Some("4h".into()),
                verified: true,
                configured: true,
                source: SourceConfidence::Official,
            },
        ],
        adapter_health: vec![
            AdapterHealth {
                tool: ToolKind::Claude,
                mode: "hook+statusline".into(),
                online: true,
                last_success_at: Some(now_s.clone()),
                last_error_at: None,
                last_error: None,
                freshness: Freshness::Hot,
            },
            AdapterHealth {
                tool: ToolKind::Codex,
                mode: "app-server".into(),
                online: true,
                last_success_at: Some(now_s.clone()),
                last_error_at: Some((now - Duration::hours(1)).to_rfc3339()),
                last_error: Some("Earlier socket reconnect timeout recovered".into()),
                freshness: Freshness::Warm,
            },
            AdapterHealth {
                tool: ToolKind::OpenClaw,
                mode: "gateway+status".into(),
                online: true,
                last_success_at: Some(now_s.clone()),
                last_error_at: None,
                last_error: None,
                freshness: Freshness::Hot,
            },
        ],
        recent_completions: vec![
            CompletionRecord {
                id: "completion-1".into(),
                tool: ToolKind::Claude,
                project_name: "OctoMonitor".into(),
                title: "Wrote acceptance spec".into(),
                finished_at: (now - Duration::minutes(6)).to_rfc3339(),
                duration_ms: Duration::minutes(31).num_milliseconds(),
                total_tokens: Some(96_000),
                cost_usd: Some(3.15),
                state: "completed".into(),
                summary: Some("Phase 4 acceptance spec completed".into()),
            },
            CompletionRecord {
                id: "completion-2".into(),
                tool: ToolKind::OpenClaw,
                project_name: "Alice".into(),
                title: "Heartbeat cleanup".into(),
                finished_at: (now - Duration::minutes(20)).to_rfc3339(),
                duration_ms: Duration::minutes(7).num_milliseconds(),
                total_tokens: Some(16_000),
                cost_usd: Some(0.41),
                state: "completed".into(),
                summary: Some("Closed stale worker runs".into()),
            },
        ],
        pending_crons: vec![PendingCron {
            id: "demo-cron-1".into(),
            name: "AI Daily Brief".into(),
            agent_id: Some("briefing".into()),
            schedule_expr: "0 9 * * *".into(),
            schedule_tz: "Asia/Shanghai".into(),
            schedule_human: "Daily 09:00".into(),
        }],
        config: AppConfig {
            listen_host: "127.0.0.1".into(),
            listen_port: 46321,
            history_days: 7,
            companion_enabled: false,
            local_ip: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freshness_thresholds_work() {
        assert!(matches!(classify_freshness(1), Freshness::Hot));
        assert!(matches!(classify_freshness(10), Freshness::Warm));
        assert!(matches!(classify_freshness(40), Freshness::Stale));
        assert!(matches!(classify_freshness(90), Freshness::Cold));
    }
    #[test]
    fn demo_bootstrap_contains_three_tools() {
        let payload = demo_bootstrap();
        assert_eq!(payload.runs.len(), 3);
        assert_eq!(payload.identities.len(), 3);
    }
}
