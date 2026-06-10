use chrono::Utc;
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

pub use octomonitor_adapter_common::{
    path_has_sensitive_component, probe_file, resolve_env_or_home, run_command_probe,
    truncate_chars, AdapterDescriptor, CommandProbeResult, FileProbeResult,
};

pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor {
        tool: "p0-control-plane",
        preferred_mode: "passive-readonly",
        fallback_mode: "fixture-gated-detection",
        confidence: "fixture-gated",
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum P0Tool {
    CodeBuddy,
    Gemini,
    Pi,
    OpenCode,
    Copilot,
    OpenHands,
    ContinueCn,
    Qwen,
    Kimi,
    Goose,
    Cursor,
    Cline,
    Kiro,
    WorkBuddy,
    AmazonQ,
    Aider,
    Amp,
    Windsurf,
    Codebuff,
    Roo,
    Kilo,
}

impl P0Tool {
    pub fn id(self) -> &'static str {
        match self {
            Self::CodeBuddy => "codebuddy",
            Self::Gemini => "gemini",
            Self::Pi => "pi",
            Self::OpenCode => "openCode",
            Self::Copilot => "copilot",
            Self::OpenHands => "openHands",
            Self::ContinueCn => "continueCn",
            Self::Qwen => "qwen",
            Self::Kimi => "kimi",
            Self::Goose => "goose",
            Self::Cursor => "cursor",
            Self::Cline => "cline",
            Self::Kiro => "kiro",
            Self::WorkBuddy => "workBuddy",
            Self::AmazonQ => "amazonQ",
            Self::Aider => "aider",
            Self::Amp => "amp",
            Self::Windsurf => "windsurf",
            Self::Codebuff => "codebuff",
            Self::Roo => "roo",
            Self::Kilo => "kilo",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CodeBuddy => "CodeBuddy",
            Self::Gemini => "Gemini CLI",
            Self::Pi => "Pi Agent",
            Self::OpenCode => "opencode",
            Self::Copilot => "GitHub Copilot CLI",
            Self::OpenHands => "OpenHands",
            Self::ContinueCn => "Continue cn",
            Self::Qwen => "Qwen Code",
            Self::Kimi => "Kimi Code",
            Self::Goose => "Goose",
            Self::Cursor => "Cursor Agent",
            Self::Cline => "Cline",
            Self::Kiro => "Kiro",
            Self::WorkBuddy => "WorkBuddy",
            Self::AmazonQ => "Amazon Q",
            Self::Aider => "Aider",
            Self::Amp => "Amp",
            Self::Windsurf => "Windsurf",
            Self::Codebuff => "Codebuff",
            Self::Roo => "Roo Code",
            Self::Kilo => "Kilo Code",
        }
    }

    fn command(self) -> &'static str {
        match self {
            Self::CodeBuddy => "codebuddy",
            Self::Gemini => "gemini",
            Self::Pi => "pi",
            Self::OpenCode => "opencode",
            Self::Copilot => "copilot",
            Self::OpenHands => "openhands",
            Self::ContinueCn => "cn",
            Self::Qwen => "qwen",
            Self::Kimi => "kimi",
            Self::Goose => "goose",
            Self::Cursor => "agent",
            Self::Cline => "cline",
            Self::Kiro => "kiro-cli",
            Self::WorkBuddy => "workbuddy",
            Self::AmazonQ => "q",
            Self::Aider => "aider",
            Self::Amp => "amp",
            Self::Windsurf => "windsurf",
            Self::Codebuff => "codebuff",
            Self::Roo => "roo",
            Self::Kilo => "kilo",
        }
    }

    fn provider(self) -> &'static str {
        match self {
            Self::CodeBuddy => "codebuddy",
            Self::Gemini => "google",
            Self::Pi => "pi",
            Self::OpenCode => "opencode",
            Self::Copilot => "github-copilot",
            Self::OpenHands => "openhands",
            Self::ContinueCn => "continue",
            Self::Qwen => "qwen",
            Self::Kimi => "moonshot",
            Self::Goose => "goose",
            Self::Cursor => "cursor",
            Self::Cline => "cline",
            Self::Kiro => "kiro",
            Self::WorkBuddy => "workbuddy",
            Self::AmazonQ => "amazon-q",
            Self::Aider => "aider",
            Self::Amp => "amp",
            Self::Windsurf => "windsurf",
            Self::Codebuff => "codebuff",
            Self::Roo => "roo",
            Self::Kilo => "kilo",
        }
    }

    fn default_root(self) -> PathBuf {
        match self {
            Self::CodeBuddy => resolve_env_or_home(&["CODEBUDDY_CONFIG_DIR"], ".codebuddy"),
            Self::Gemini => resolve_env_or_home(&["GEMINI_HOME"], ".gemini"),
            Self::Pi => resolve_env_or_home(&["PI_CODING_AGENT_HOME"], ".pi/agent"),
            Self::OpenCode => {
                resolve_env_or_home(&["OPENCODE_CONFIG_DIR"], ".local/share/opencode")
            }
            Self::Copilot => resolve_env_or_home(&["COPILOT_HOME"], ".copilot"),
            Self::OpenHands => resolve_env_or_home(&["OPENHANDS_HOME"], ".openhands"),
            Self::ContinueCn => resolve_env_or_home(&["CONTINUE_GLOBAL_DIR"], ".continue"),
            Self::Qwen => resolve_env_or_home(&["QWEN_CONFIG_DIR"], ".qwen"),
            Self::Kimi => resolve_env_or_home(&["KIMI_CODE_HOME"], ".kimi-code"),
            Self::Goose => resolve_env_or_home(&["GOOSE_DATA_DIR"], ".local/share/goose"),
            Self::Cursor => resolve_env_or_home(&["CURSOR_AGENT_HOME"], ".cursor"),
            Self::Cline => resolve_env_or_home(&["CLINE_HOME", "CLINE_DATA_DIR"], ".cline"),
            Self::Kiro => resolve_env_or_home(&["KIRO_HOME"], ".kiro"),
            Self::WorkBuddy => resolve_env_or_home(&["WORKBUDDY_CONFIG_DIR"], ".workbuddy"),
            Self::AmazonQ => resolve_env_or_home(&["AMAZON_Q_HOME"], ".aws/amazonq"),
            Self::Aider => resolve_env_or_home(&["AIDER_HOME"], ".aider"),
            Self::Amp => resolve_env_or_home(&["AMP_HOME"], ".amp"),
            Self::Windsurf => resolve_env_or_home(&["WINDSURF_HOME"], ".windsurf"),
            Self::Codebuff => resolve_env_or_home(&["CODEBUFF_HOME"], ".codebuff"),
            Self::Roo => resolve_env_or_home(&["ROO_CODE_HOME"], ".roo"),
            Self::Kilo => resolve_env_or_home(&["KILO_CODE_HOME"], ".kilo"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum P0SourceType {
    Json,
    Jsonl,
    Sqlite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum P0SchemaConfidence {
    High,
    Medium,
    Low,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum P0CostKind {
    Exact,
    Partial,
    NotAvailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P0Session {
    pub tool: P0Tool,
    pub session_id: String,
    pub workspace_path: String,
    pub project_name: String,
    pub source_mode: String,
    pub source_id: String,
    pub source_path: Option<String>,
    pub source_type: P0SourceType,
    pub schema_version: Option<String>,
    pub schema_confidence: P0SchemaConfidence,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub started_at: String,
    pub last_activity_at: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub cost_kind: P0CostKind,
    pub enters_usage_totals: bool,
    pub message_count: u64,
    pub first_question: Option<String>,
    pub last_question: Option<String>,
    pub pending_approval: bool,
    pub support_level: String,
    pub resume_command: Option<String>,
    pub tool_specific: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P0ToolReport {
    pub tool: P0Tool,
    pub probed_at: String,
    pub root: Option<String>,
    pub root_exists: bool,
    pub cli_available: bool,
    pub cli_version: Option<String>,
    pub sessions: Vec<P0Session>,
    pub command_probes: Vec<CommandProbeResult>,
    pub file_probes: Vec<FileProbeResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P0Snapshot {
    pub probed_at: String,
    pub reports: Vec<P0ToolReport>,
}

impl P0Snapshot {
    pub fn empty_with_error(reason: String) -> Self {
        let probed_at = Utc::now().to_rfc3339();
        Self {
            probed_at: probed_at.clone(),
            reports: all_p0_tools()
                .into_iter()
                .map(|tool| P0ToolReport {
                    tool,
                    probed_at: probed_at.clone(),
                    root: None,
                    root_exists: false,
                    cli_available: false,
                    cli_version: None,
                    sessions: Vec::new(),
                    command_probes: vec![CommandProbeResult {
                        command: format!("{} passive probe", tool.id()),
                        success: false,
                        stdout_snippet: None,
                        error: Some(reason.clone()),
                    }],
                    file_probes: Vec::new(),
                })
                .collect(),
        }
    }
}

pub fn all_p0_tools() -> Vec<P0Tool> {
    vec![
        P0Tool::CodeBuddy,
        P0Tool::Gemini,
        P0Tool::Pi,
        P0Tool::OpenCode,
        P0Tool::Copilot,
        P0Tool::OpenHands,
        P0Tool::ContinueCn,
        P0Tool::Qwen,
        P0Tool::Kimi,
        P0Tool::Goose,
        P0Tool::Cursor,
        P0Tool::Cline,
        P0Tool::Kiro,
        P0Tool::WorkBuddy,
        P0Tool::AmazonQ,
        P0Tool::Aider,
        P0Tool::Amp,
        P0Tool::Windsurf,
        P0Tool::Codebuff,
        P0Tool::Roo,
        P0Tool::Kilo,
    ]
}

pub fn probe() -> P0Snapshot {
    let probed_at = Utc::now().to_rfc3339();
    let reports = all_p0_tools()
        .into_iter()
        .map(|tool| probe_tool(tool, &probed_at))
        .collect();
    P0Snapshot { probed_at, reports }
}

fn probe_tool(tool: P0Tool, probed_at: &str) -> P0ToolReport {
    let root = tool.default_root();
    let version_probe = run_command_probe(tool.command(), &["--version"]);
    let mut command_probes = vec![version_probe.clone()];
    let mut file_probes = vec![probe_file(&root)];
    let sessions = match tool {
        P0Tool::CodeBuddy => scan_codebuddy(&root),
        P0Tool::Gemini => scan_gemini(&root),
        P0Tool::Pi => scan_pi(&root),
        P0Tool::OpenCode => {
            let db_probe = run_command_probe("opencode", &["db", "path"]);
            let db_path = db_probe
                .success
                .then(|| {
                    db_probe
                        .stdout_snippet
                        .as_deref()
                        .map(str::trim)
                        .map(PathBuf::from)
                })
                .flatten()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| root.join("opencode.db"));
            command_probes.push(db_probe);
            file_probes.push(probe_file(&db_path));
            scan_opencode(&db_path)
        }
        P0Tool::Copilot => scan_copilot(&root),
        P0Tool::OpenHands => scan_openhands(&root),
        P0Tool::ContinueCn => scan_continue_cn(&root),
        P0Tool::Qwen => scan_qwen(&root),
        P0Tool::Kimi => scan_kimi(&root),
        P0Tool::Goose => {
            let db_path = root.join("sessions").join("sessions.db");
            file_probes.push(probe_file(&db_path));
            scan_goose(&db_path)
        }
        P0Tool::Cursor if cursor_private_store_opted_in() => scan_cursor(&root),
        P0Tool::Cursor => Vec::new(),
        P0Tool::Cline => {
            let db_path = root.join("sessions.db");
            file_probes.push(probe_file(&db_path));
            scan_cline(&db_path)
        }
        P0Tool::Kiro => scan_kiro(&root),
        P0Tool::WorkBuddy
        | P0Tool::AmazonQ
        | P0Tool::Aider
        | P0Tool::Amp
        | P0Tool::Windsurf
        | P0Tool::Codebuff
        | P0Tool::Roo
        | P0Tool::Kilo => Vec::new(),
    };

    P0ToolReport {
        tool,
        probed_at: probed_at.to_string(),
        root: Some(root.display().to_string()),
        root_exists: root.exists(),
        cli_available: version_probe.success,
        cli_version: version_probe.stdout_snippet.clone(),
        sessions,
        command_probes,
        file_probes,
    }
}

#[derive(Debug, Clone)]
struct SessionAcc {
    tool: P0Tool,
    session_id: String,
    workspace_path: Option<String>,
    project_name: Option<String>,
    source_mode: &'static str,
    source_id: &'static str,
    source_path: Option<String>,
    source_type: P0SourceType,
    schema_version: &'static str,
    schema_confidence: P0SchemaConfidence,
    model: Option<String>,
    provider: Option<String>,
    started_at: Option<String>,
    last_activity_at: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    total_tokens: u64,
    cost_usd: Option<f64>,
    cost_kind: P0CostKind,
    enters_usage_totals: bool,
    message_count: u64,
    first_question: Option<String>,
    last_question: Option<String>,
    pending_approval: bool,
    support_level: &'static str,
    resume_command: Option<String>,
    tool_specific: Value,
}

impl SessionAcc {
    fn new(
        tool: P0Tool,
        session_id: String,
        source_mode: &'static str,
        source_id: &'static str,
        source_type: P0SourceType,
        schema_version: &'static str,
        schema_confidence: P0SchemaConfidence,
        support_level: &'static str,
    ) -> Self {
        Self {
            tool,
            session_id,
            workspace_path: None,
            project_name: None,
            source_mode,
            source_id,
            source_path: None,
            source_type,
            schema_version,
            schema_confidence,
            model: None,
            provider: Some(tool.provider().into()),
            started_at: None,
            last_activity_at: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            total_tokens: 0,
            cost_usd: None,
            cost_kind: P0CostKind::Partial,
            enters_usage_totals: true,
            message_count: 0,
            first_question: None,
            last_question: None,
            pending_approval: false,
            support_level,
            resume_command: None,
            tool_specific: serde_json::json!({}),
        }
    }

    fn touch(&mut self, timestamp: Option<String>) {
        if let Some(timestamp) = timestamp {
            if self
                .started_at
                .as_ref()
                .is_none_or(|existing| &timestamp < existing)
            {
                self.started_at = Some(timestamp.clone());
            }
            if self
                .last_activity_at
                .as_ref()
                .is_none_or(|existing| &timestamp > existing)
            {
                self.last_activity_at = Some(timestamp);
            }
        }
    }

    fn apply_usage(&mut self, usage: Option<&Value>) {
        let Some(usage) = usage else {
            return;
        };
        self.input_tokens = self.input_tokens.saturating_add(u64_field(
            usage,
            &[
                "input_tokens",
                "inputTokens",
                "promptTokens",
                "prompt_tokens",
                "input",
            ],
        ));
        self.output_tokens = self.output_tokens.saturating_add(u64_field(
            usage,
            &[
                "output_tokens",
                "outputTokens",
                "completionTokens",
                "completion_tokens",
                "output",
            ],
        ));
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(u64_field(
            usage,
            &[
                "cache_read_tokens",
                "cacheReadTokens",
                "cache_read_input_tokens",
                "cacheRead",
            ],
        ));
        self.cache_write_tokens = self.cache_write_tokens.saturating_add(u64_field(
            usage,
            &[
                "cache_write_tokens",
                "cacheWriteTokens",
                "cache_creation_input_tokens",
                "cacheWrite",
            ],
        ));
        let total = u64_field(usage, &["total_tokens", "totalTokens", "total"]);
        self.total_tokens = self.total_tokens.saturating_add(total);
        if self.cost_usd.is_none() {
            self.cost_usd = f64_field(usage, &["cost_usd", "costUsd", "cost"]);
        }
        if self.cost_usd.is_some() {
            self.cost_kind = P0CostKind::Exact;
        }
    }

    fn finish(mut self) -> Option<P0Session> {
        if self.session_id.trim().is_empty() {
            return None;
        }
        if self.total_tokens == 0 {
            self.total_tokens = self
                .input_tokens
                .saturating_add(self.output_tokens)
                .saturating_add(self.cache_read_tokens)
                .saturating_add(self.cache_write_tokens);
        }
        if matches!(self.cost_kind, P0CostKind::NotAvailable) {
            self.enters_usage_totals = false;
        }
        let now = Utc::now().to_rfc3339();
        let workspace_path = self
            .workspace_path
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("~/.{}", self.tool.id()));
        let project_name = self
            .project_name
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| project_name_from_path(&workspace_path, self.tool.label()));
        Some(P0Session {
            tool: self.tool,
            session_id: self.session_id,
            workspace_path,
            project_name,
            source_mode: self.source_mode.into(),
            source_id: self.source_id.into(),
            source_path: self.source_path,
            source_type: self.source_type,
            schema_version: Some(self.schema_version.into()),
            schema_confidence: self.schema_confidence,
            model: self.model,
            provider: self.provider,
            started_at: self.started_at.unwrap_or_else(|| now.clone()),
            last_activity_at: self.last_activity_at.unwrap_or(now),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_read_tokens: self.cache_read_tokens,
            cache_write_tokens: self.cache_write_tokens,
            total_tokens: self.total_tokens,
            cost_usd: self.cost_usd,
            cost_kind: self.cost_kind,
            enters_usage_totals: self.enters_usage_totals,
            message_count: self.message_count,
            first_question: self.first_question,
            last_question: self.last_question,
            pending_approval: self.pending_approval,
            support_level: self.support_level.into(),
            resume_command: self.resume_command,
            tool_specific: self.tool_specific,
        })
    }
}

fn scan_codebuddy(root: &Path) -> Vec<P0Session> {
    let mut sessions = Vec::new();
    for path in collect_files(&root.join("projects"), "jsonl", 4) {
        sessions.extend(parse_claude_like_jsonl(
            &path,
            P0Tool::CodeBuddy,
            "codebuddy_transcript",
            "codebuddy:transcript",
            "claude-like-v1",
            "fixture-gated-monitored",
            |id| format!("codebuddy --resume {}", shell_quote(id)),
        ));
    }
    add_codebuddy_worker_liveness(root, &mut sessions);
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn add_codebuddy_worker_liveness(root: &Path, sessions: &mut [P0Session]) {
    let workers = collect_files(&root.join("sessions"), "json", 2);
    if workers.is_empty() {
        return;
    }
    for session in sessions {
        session.tool_specific["workerLivenessFiles"] = serde_json::json!(workers.len());
    }
}

fn parse_claude_like_jsonl<F>(
    path: &Path,
    tool: P0Tool,
    source_mode: &'static str,
    source_id: &'static str,
    schema_version: &'static str,
    support_level: &'static str,
    resume: F,
) -> Vec<P0Session>
where
    F: Fn(&str) -> String + Copy,
{
    let Some(lines) = read_jsonl_values(path) else {
        return Vec::new();
    };
    let mut by_id: HashMap<String, SessionAcc> = HashMap::new();
    for value in lines {
        let session_id = string_field(&value, &["sessionId", "session_id", "sessionID"])
            .or_else(|| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        let timestamp = string_field(&value, &["timestamp", "created_at", "updated_at"]);
        let acc = by_id.entry(session_id.clone()).or_insert_with(|| {
            let mut acc = SessionAcc::new(
                tool,
                session_id.clone(),
                source_mode,
                source_id,
                P0SourceType::Jsonl,
                schema_version,
                P0SchemaConfidence::High,
                support_level,
            );
            acc.source_path = Some(path.display().to_string());
            acc.resume_command = Some(resume(&session_id));
            acc
        });
        if let Some(cwd) = string_field(&value, &["cwd", "workspace", "workspacePath"]) {
            acc.workspace_path = Some(cwd);
        }
        acc.touch(timestamp);
        acc.message_count = acc.message_count.saturating_add(1);
        let role = value
            .get("message")
            .and_then(|message| string_field(message, &["role"]))
            .or_else(|| string_field(&value, &["role", "type"]));
        if role.as_deref() == Some("user")
            || value.get("type").and_then(Value::as_str) == Some("user")
        {
            if let Some(text) = value
                .get("message")
                .and_then(extract_text)
                .or_else(|| extract_text(&value))
            {
                if acc.first_question.is_none() {
                    acc.first_question = Some(truncate_chars(&text, 120));
                }
                acc.last_question = Some(truncate_chars(&text, 120));
            }
        }
        if let Some(message) = value.get("message") {
            if let Some(model) = string_field(message, &["model"]) {
                acc.model = Some(model);
            }
            acc.apply_usage(message.get("usage"));
        }
        acc.apply_usage(value.get("usage"));
    }
    by_id.into_values().filter_map(SessionAcc::finish).collect()
}

fn scan_gemini(root: &Path) -> Vec<P0Session> {
    let mut sessions = Vec::new();
    for path in collect_files(&root.join("tmp"), "jsonl", 6) {
        if path.components().any(|c| c.as_os_str() == "chats") {
            sessions.extend(parse_chat_recording_jsonl(
                &path,
                P0Tool::Gemini,
                "gemini_chat_recording",
                "gemini:chat-recording",
                "chat-recording-v1",
                "fixture-gated-monitored",
                |id| format!("gemini --resume {}", shell_quote(id)),
            ));
        }
    }
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn scan_qwen(root: &Path) -> Vec<P0Session> {
    let projects = root.join("projects");
    let mut sessions = Vec::new();
    for path in collect_files(&projects, "jsonl", 6) {
        if path.components().any(|c| c.as_os_str() == "chats") {
            sessions.extend(parse_chat_recording_jsonl(
                &path,
                P0Tool::Qwen,
                "qwen_chat_recording",
                "qwen:chat-recording",
                "chat-recording-v1",
                "fixture-gated-monitored",
                |id| format!("qwen --resume {}", shell_quote(id)),
            ));
        }
    }
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn parse_chat_recording_jsonl<F>(
    path: &Path,
    tool: P0Tool,
    source_mode: &'static str,
    source_id: &'static str,
    schema_version: &'static str,
    support_level: &'static str,
    resume: F,
) -> Vec<P0Session>
where
    F: Fn(&str) -> String + Copy,
{
    let Some(lines) = read_jsonl_values(path) else {
        return Vec::new();
    };
    let fallback_id = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{}-session", tool.id()));
    let mut current_id = fallback_id.clone();
    let mut by_id: HashMap<String, SessionAcc> = HashMap::new();
    for value in lines {
        let line_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        if let Some(id) = string_field(&value, &["session_id", "sessionId", "id"]) {
            current_id = id;
        }
        let acc = by_id.entry(current_id.clone()).or_insert_with(|| {
            let mut acc = SessionAcc::new(
                tool,
                current_id.clone(),
                source_mode,
                source_id,
                P0SourceType::Jsonl,
                schema_version,
                P0SchemaConfidence::High,
                support_level,
            );
            acc.source_path = Some(path.display().to_string());
            acc.resume_command = Some(resume(&current_id));
            acc
        });
        if line_type == "$rewindTo" {
            let keep = (
                acc.workspace_path.clone(),
                acc.project_name.clone(),
                acc.model.clone(),
                acc.source_path.clone(),
                acc.resume_command.clone(),
            );
            *acc = SessionAcc::new(
                tool,
                current_id.clone(),
                source_mode,
                source_id,
                P0SourceType::Jsonl,
                schema_version,
                P0SchemaConfidence::High,
                support_level,
            );
            acc.workspace_path = keep.0;
            acc.project_name = keep.1;
            acc.model = keep.2;
            acc.source_path = keep.3;
            acc.resume_command = keep.4;
            acc.tool_specific["rewound"] = serde_json::json!(true);
            continue;
        }
        if let Some(cwd) = string_field(&value, &["cwd", "workspace", "workspacePath"]) {
            acc.workspace_path = Some(cwd);
        }
        if let Some(model) = string_field(&value, &["model"]) {
            acc.model = Some(model);
        }
        if let Some(title) = string_field(&value, &["title", "name"]) {
            acc.project_name = Some(title);
        }
        if line_type == "$set" {
            continue;
        }
        let timestamp = string_field(&value, &["timestamp", "created_at", "updated_at"]);
        acc.touch(timestamp);
        acc.message_count = acc.message_count.saturating_add(1);
        if line_type == "user" {
            if let Some(text) = extract_text(&value) {
                if acc.first_question.is_none() {
                    acc.first_question = Some(truncate_chars(&text, 120));
                }
                acc.last_question = Some(truncate_chars(&text, 120));
            }
        }
        acc.apply_usage(value.get("usage"));
    }
    by_id.into_values().filter_map(SessionAcc::finish).collect()
}

fn scan_pi(root: &Path) -> Vec<P0Session> {
    let session_dir = std::env::var_os("PI_CODING_AGENT_SESSION_DIR")
        .map(PathBuf::from)
        .or_else(|| pi_settings_session_dir(root))
        .unwrap_or_else(|| root.join("sessions"));
    let mut sessions = Vec::new();
    for path in collect_files(&session_dir, "jsonl", 5) {
        sessions.extend(parse_pi_jsonl(&path));
    }
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn pi_settings_session_dir(root: &Path) -> Option<PathBuf> {
    let settings = fs::read_to_string(root.join("settings.json")).ok()?;
    let value: Value = serde_json::from_str(&settings).ok()?;
    string_field(&value, &["sessionDir"]).map(PathBuf::from)
}

fn parse_pi_jsonl(path: &Path) -> Vec<P0Session> {
    let Some(lines) = read_jsonl_values(path) else {
        return Vec::new();
    };
    let fallback_id = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "pi-session".into());
    let mut acc = SessionAcc::new(
        P0Tool::Pi,
        fallback_id,
        "pi_session_jsonl",
        "pi:session-jsonl",
        P0SourceType::Jsonl,
        "session-jsonl-v1",
        P0SchemaConfidence::High,
        "fixture-gated-monitored",
    );
    acc.source_path = Some(path.display().to_string());
    let mut branch_nodes = 0u64;
    for value in lines {
        let line_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        if line_type == "session" {
            if let Some(id) = string_field(&value, &["id", "session_id", "sessionId"]) {
                acc.session_id = id;
            }
            if let Some(cwd) = string_field(&value, &["cwd", "workspace"]) {
                acc.workspace_path = Some(cwd);
            }
            acc.provider = string_field(&value, &["provider"]).or(acc.provider);
            acc.model = string_field(&value, &["model"]).or(acc.model);
        }
        if value.get("parentId").is_some() || value.get("parent_id").is_some() {
            branch_nodes = branch_nodes.saturating_add(1);
        }
        if string_field(&value, &["role"]).as_deref() == Some("user") {
            if let Some(text) = extract_text(&value) {
                if acc.first_question.is_none() {
                    acc.first_question = Some(truncate_chars(&text, 120));
                }
                acc.last_question = Some(truncate_chars(&text, 120));
            }
        }
        if let Some(model) = string_field(&value, &["model"]) {
            acc.model = Some(model);
        }
        acc.touch(string_field(
            &value,
            &["timestamp", "created_at", "updated_at"],
        ));
        acc.apply_usage(value.get("usage"));
        acc.message_count = acc.message_count.saturating_add(1);
    }
    acc.resume_command = Some(format!("pi --session {}", shell_quote(&acc.session_id)));
    acc.tool_specific["branchNodes"] = serde_json::json!(branch_nodes);
    acc.finish().into_iter().collect()
}

fn scan_continue_cn(root: &Path) -> Vec<P0Session> {
    let mut sessions = Vec::new();
    for path in collect_files(&root.join("sessions"), "json", 2) {
        if let Some(session) = parse_continue_session_json(&path) {
            sessions.push(session);
        }
    }
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn parse_continue_session_json(path: &Path) -> Option<P0Session> {
    let value = read_json_file(path)?;
    let session_id = string_field(&value, &["sessionId", "session_id", "id"])?;
    let mut acc = SessionAcc::new(
        P0Tool::ContinueCn,
        session_id.clone(),
        "continue_session_json",
        "continue-cn:sessions",
        P0SourceType::Json,
        "continue-session-json-v1",
        P0SchemaConfidence::High,
        "monitored-lite",
    );
    acc.source_path = Some(path.display().to_string());
    acc.workspace_path = string_field(&value, &["workspaceDirectory", "workspace", "cwd"]);
    acc.project_name = string_field(&value, &["title", "name"]);
    acc.model = string_field(&value, &["model"]).or_else(|| Some("auto".into()));
    acc.resume_command = Some(format!("cn --resume {}", shell_quote(&session_id)));
    if let Some(history) = value.get("history").and_then(Value::as_array) {
        for item in history {
            acc.message_count = acc.message_count.saturating_add(1);
            if string_field(item, &["role"]).as_deref() == Some("user") {
                if let Some(text) = extract_text(item) {
                    if acc.first_question.is_none() {
                        acc.first_question = Some(truncate_chars(&text, 120));
                    }
                    acc.last_question = Some(truncate_chars(&text, 120));
                }
            }
            acc.apply_usage(item.get("usage"));
        }
    }
    acc.cost_kind = P0CostKind::Partial;
    acc.finish()
}

fn scan_kimi(root: &Path) -> Vec<P0Session> {
    let session_root = if root.join("session_index.jsonl").exists() {
        root.to_path_buf()
    } else {
        root.join("sessions")
    };
    parse_kimi_session_root(&session_root)
}

fn parse_kimi_session_root(root: &Path) -> Vec<P0Session> {
    if path_has_sensitive_component(root) {
        return Vec::new();
    }
    let mut by_id: HashMap<String, SessionAcc> = HashMap::new();
    for value in read_jsonl_values(&root.join("session_index.jsonl")).unwrap_or_default() {
        let Some(session_id) = string_field(&value, &["session_id", "sessionId", "id"]) else {
            continue;
        };
        let acc = by_id.entry(session_id.clone()).or_insert_with(|| {
            let mut acc = SessionAcc::new(
                P0Tool::Kimi,
                session_id.clone(),
                "kimi_session_index_wire",
                "kimi:sessions",
                P0SourceType::Jsonl,
                "sessions-index-wire-v1",
                P0SchemaConfidence::High,
                "fixture-gated-monitored",
            );
            acc.source_path = Some(root.display().to_string());
            acc.resume_command = Some(format!("kimi --session {}", shell_quote(&session_id)));
            acc
        });
        acc.project_name = string_field(&value, &["title", "name"]).or(acc.project_name.clone());
        acc.touch(string_field(
            &value,
            &["updated_at", "updatedAt", "timestamp"],
        ));
    }
    if let Some(state) = read_json_file(&root.join("state.json")) {
        if let Some(session_id) = string_field(&state, &["session_id", "sessionId", "id"]) {
            let acc = by_id.entry(session_id.clone()).or_insert_with(|| {
                SessionAcc::new(
                    P0Tool::Kimi,
                    session_id.clone(),
                    "kimi_session_index_wire",
                    "kimi:sessions",
                    P0SourceType::Jsonl,
                    "sessions-index-wire-v1",
                    P0SchemaConfidence::High,
                    "fixture-gated-monitored",
                )
            });
            acc.workspace_path = string_field(&state, &["cwd", "workspace"]);
            acc.model = string_field(&state, &["model"]);
            acc.touch(string_field(
                &state,
                &["last_activity_at", "updated_at", "timestamp"],
            ));
            acc.resume_command = Some(format!("kimi --session {}", shell_quote(&session_id)));
        }
    }
    for path in collect_files(&root.join("agents"), "jsonl", 4) {
        for value in read_jsonl_values(&path).unwrap_or_default() {
            let Some(session_id) = string_field(&value, &["session_id", "sessionId", "id"]) else {
                continue;
            };
            let acc = by_id.entry(session_id.clone()).or_insert_with(|| {
                SessionAcc::new(
                    P0Tool::Kimi,
                    session_id.clone(),
                    "kimi_session_index_wire",
                    "kimi:sessions",
                    P0SourceType::Jsonl,
                    "sessions-index-wire-v1",
                    P0SchemaConfidence::High,
                    "fixture-gated-monitored",
                )
            });
            acc.source_path = Some(root.display().to_string());
            acc.apply_usage(value.get("usage"));
            acc.touch(string_field(&value, &["timestamp", "updated_at"]));
            acc.message_count = acc.message_count.saturating_add(1);
            acc.resume_command = Some(format!("kimi --session {}", shell_quote(&session_id)));
        }
    }
    let mut sessions: Vec<_> = by_id.into_values().filter_map(SessionAcc::finish).collect();
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn scan_copilot(root: &Path) -> Vec<P0Session> {
    let mut sessions = Vec::new();
    for path in collect_files(&root.join("session-state"), "json", 3) {
        if let Some(session) = parse_copilot_state_json(&path) {
            sessions.push(session);
        }
    }
    let db = root.join("session-store.db");
    sessions.extend(parse_simple_session_db(
        &db,
        P0Tool::Copilot,
        "copilot_chronicle",
        "copilot:chronicle",
        "chronicle-state-v1",
        P0SchemaConfidence::Medium,
        "fixture-gated-monitored",
    ));
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn parse_copilot_state_json(path: &Path) -> Option<P0Session> {
    let value = read_json_file(path)?;
    let session_id = string_field(&value, &["sessionId", "session_id", "id"])?;
    let mut acc = SessionAcc::new(
        P0Tool::Copilot,
        session_id.clone(),
        "copilot_chronicle",
        "copilot:chronicle",
        P0SourceType::Json,
        "chronicle-state-v1",
        P0SchemaConfidence::High,
        "fixture-gated-monitored",
    );
    acc.source_path = Some(path.display().to_string());
    acc.workspace_path = string_field(&value, &["workspace", "workspacePath", "cwd"]);
    acc.project_name = string_field(&value, &["title", "name"]);
    acc.model = string_field(&value, &["model"]);
    acc.touch(string_field(
        &value,
        &["updatedAt", "updated_at", "timestamp"],
    ));
    acc.apply_usage(value.get("usage"));
    acc.resume_command = Some(format!(
        "copilot session resume {}",
        shell_quote(&session_id)
    ));
    acc.finish()
}

fn scan_openhands(root: &Path) -> Vec<P0Session> {
    let mut sessions = Vec::new();
    for path in collect_files(&root.join("conversations"), "json", 3) {
        if path
            .file_name()
            .is_some_and(|name| name == "conversation.json")
        {
            if let Some(session) = parse_openhands_conversation_json(&path) {
                sessions.push(session);
            }
        }
    }
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn parse_openhands_conversation_json(path: &Path) -> Option<P0Session> {
    let value = read_json_file(path)?;
    let session_id = string_field(&value, &["conversation_id", "sessionId", "id"])?;
    let mut acc = SessionAcc::new(
        P0Tool::OpenHands,
        session_id.clone(),
        "openhands_conversation",
        "openhands:conversations",
        P0SourceType::Json,
        "conversation-json-v1",
        P0SchemaConfidence::High,
        "fixture-gated-monitored",
    );
    acc.source_path = Some(path.display().to_string());
    acc.workspace_path = string_field(&value, &["workspace", "workspacePath", "cwd"]);
    acc.project_name = string_field(&value, &["title", "name"]);
    acc.model = string_field(&value, &["model"]);
    acc.touch(string_field(
        &value,
        &["updated_at", "updatedAt", "timestamp"],
    ));
    acc.apply_usage(value.get("statistics").or_else(|| value.get("usage")));
    acc.message_count = value
        .get("messages")
        .and_then(Value::as_array)
        .map(|messages| messages.len() as u64)
        .unwrap_or(0);
    acc.resume_command = Some(format!(
        "openhands --conversation-id {}",
        shell_quote(&session_id)
    ));
    acc.finish()
}

fn scan_opencode(db_path: &Path) -> Vec<P0Session> {
    parse_opencode_db(db_path)
}

fn parse_opencode_db(path: &Path) -> Vec<P0Session> {
    let Some(conn) = open_readonly_db(path) else {
        return Vec::new();
    };
    if !table_exists(&conn, "sessions") {
        return Vec::new();
    }
    let session_columns = table_columns(&conn, "sessions");
    let fields = [
        select_alias(&session_columns, &["id", "session_id"], "session_id"),
        select_alias(
            &session_columns,
            &["project", "workspace", "cwd"],
            "workspace",
        ),
        select_alias(&session_columns, &["title", "name"], "title"),
        select_alias(&session_columns, &["model"], "model"),
        select_alias(
            &session_columns,
            &["created_at", "started_at"],
            "started_at",
        ),
        select_alias(
            &session_columns,
            &["updated_at", "last_activity_at"],
            "updated_at",
        ),
    ];
    let sql = format!("SELECT {} FROM sessions", fields.join(", "));
    let usage = sqlite_usage_by_session(&conn, "messages");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        let session_id = row_string(row, "session_id").unwrap_or_default();
        let mut acc = SessionAcc::new(
            P0Tool::OpenCode,
            session_id.clone(),
            "opencode_sqlite",
            "opencode:sqlite",
            P0SourceType::Sqlite,
            "opencode-db-v1",
            P0SchemaConfidence::High,
            "fixture-gated-monitored",
        );
        acc.source_path = Some(path.display().to_string());
        acc.workspace_path = row_string(row, "workspace");
        acc.project_name = row_string(row, "title");
        acc.model = row_string(row, "model");
        acc.touch(row_string(row, "started_at"));
        acc.touch(row_string(row, "updated_at"));
        acc.resume_command = Some(format!("opencode session {}", shell_quote(&session_id)));
        if let Some(total) = usage.get(&session_id) {
            acc.input_tokens = total.0;
            acc.output_tokens = total.1;
            acc.cache_read_tokens = total.2;
            acc.cache_write_tokens = total.3;
            acc.total_tokens = total.0 + total.1 + total.2 + total.3;
            acc.cost_kind = P0CostKind::Exact;
        }
        Ok(acc.finish())
    }) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok).flatten().collect()
}

fn scan_goose(db_path: &Path) -> Vec<P0Session> {
    parse_goose_db(db_path)
}

fn parse_goose_db(path: &Path) -> Vec<P0Session> {
    parse_simple_session_db(
        path,
        P0Tool::Goose,
        "goose_sessions_db",
        "goose:sessions-db",
        "sessions-db-v1",
        P0SchemaConfidence::High,
        "fixture-gated-monitored",
    )
}

fn parse_simple_session_db(
    path: &Path,
    tool: P0Tool,
    source_mode: &'static str,
    source_id: &'static str,
    schema_version: &'static str,
    schema_confidence: P0SchemaConfidence,
    support_level: &'static str,
) -> Vec<P0Session> {
    let Some(conn) = open_readonly_db(path) else {
        return Vec::new();
    };
    if !table_exists(&conn, "sessions") {
        return Vec::new();
    }
    let columns = table_columns(&conn, "sessions");
    let fields = [
        select_alias(&columns, &["id", "session_id"], "session_id"),
        select_alias(&columns, &["workspace", "project", "cwd"], "workspace"),
        select_alias(&columns, &["title", "name"], "title"),
        select_alias(&columns, &["model"], "model"),
        select_alias(&columns, &["created_at", "started_at"], "started_at"),
        select_alias(&columns, &["updated_at", "last_activity_at"], "updated_at"),
    ];
    let sql = format!("SELECT {} FROM sessions", fields.join(", "));
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        let session_id = row_string(row, "session_id").unwrap_or_default();
        let mut acc = SessionAcc::new(
            tool,
            session_id.clone(),
            source_mode,
            source_id,
            P0SourceType::Sqlite,
            schema_version,
            schema_confidence,
            support_level,
        );
        acc.source_path = Some(path.display().to_string());
        acc.workspace_path = row_string(row, "workspace");
        acc.project_name = row_string(row, "title");
        acc.model = row_string(row, "model");
        acc.touch(row_string(row, "started_at"));
        acc.touch(row_string(row, "updated_at"));
        acc.cost_kind = P0CostKind::NotAvailable;
        acc.resume_command = match tool {
            P0Tool::Goose => Some(format!("goose session resume {}", shell_quote(&session_id))),
            P0Tool::Copilot => Some(format!(
                "copilot session resume {}",
                shell_quote(&session_id)
            )),
            _ => None,
        };
        Ok(acc.finish())
    }) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok).flatten().collect()
}

fn scan_cursor(root: &Path) -> Vec<P0Session> {
    let mut sessions = Vec::new();
    for path in collect_files(&root.join("chats"), "db", 4) {
        if path.file_name().is_some_and(|name| name == "store.db") {
            sessions.extend(parse_cursor_store_db(&path));
        }
    }
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn cursor_private_store_opted_in() -> bool {
    std::env::var("OCTOMONITOR_CURSOR_PRIVATE_STORE")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn scan_cline(db_path: &Path) -> Vec<P0Session> {
    parse_simple_session_db(
        db_path,
        P0Tool::Cline,
        "cline_metadata_sqlite",
        "cline:metadata-db",
        "metadata-only-v1",
        P0SchemaConfidence::Medium,
        "fixture-gated-metadata",
    )
}

fn scan_kiro(root: &Path) -> Vec<P0Session> {
    let candidates = [
        root.join("custom-storage.jsonl"),
        root.join("storage").join("custom-storage.jsonl"),
    ];
    let mut sessions = Vec::new();
    for path in candidates {
        sessions.extend(parse_kiro_custom_storage_jsonl(&path));
    }
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    sessions
}

fn parse_kiro_custom_storage_jsonl(path: &Path) -> Vec<P0Session> {
    let Some(lines) = read_jsonl_values(path) else {
        return Vec::new();
    };
    let mut by_id: HashMap<String, SessionAcc> = HashMap::new();
    for value in lines {
        let Some(session_id) = string_field(&value, &["sessionId", "session_id", "id"]) else {
            continue;
        };
        let acc = by_id.entry(session_id.clone()).or_insert_with(|| {
            let mut acc = SessionAcc::new(
                P0Tool::Kiro,
                session_id.clone(),
                "kiro_custom_storage",
                "kiro:custom-storage",
                P0SourceType::Jsonl,
                "custom-storage-v1",
                P0SchemaConfidence::Medium,
                "fixture-gated-cli",
            );
            acc.source_path = Some(path.display().to_string());
            acc.resume_command = Some(format!(
                "kiro-cli chat --resume-id {}",
                shell_quote(&session_id)
            ));
            acc
        });
        if let Some(workspace) = string_field(&value, &["workspace", "cwd", "workspacePath"]) {
            acc.workspace_path = Some(workspace);
        }
        acc.touch(string_field(
            &value,
            &["timestamp", "updated_at", "updatedAt"],
        ));
        acc.message_count = acc.message_count.saturating_add(1);
        if string_field(&value, &["role"]).as_deref() == Some("user") {
            if let Some(text) = extract_text(&value) {
                if acc.first_question.is_none() {
                    acc.first_question = Some(truncate_chars(&text, 120));
                }
                acc.last_question = Some(truncate_chars(&text, 120));
            }
        }
        acc.apply_usage(value.get("usage"));
    }
    by_id.into_values().filter_map(SessionAcc::finish).collect()
}

fn parse_cursor_store_db(path: &Path) -> Vec<P0Session> {
    let Some(conn) = open_readonly_db(path) else {
        return Vec::new();
    };
    let mut sessions = Vec::new();
    for table in ["blobs", "meta"] {
        if !table_exists(&conn, table) {
            continue;
        }
        let columns = table_columns(&conn, table);
        let key_col = ["key", "name", "id"]
            .into_iter()
            .find(|candidate| columns.contains(*candidate));
        let value_col = ["value", "blob", "data"]
            .into_iter()
            .find(|candidate| columns.contains(*candidate));
        let (Some(key_col), Some(value_col)) = (key_col, value_col) else {
            continue;
        };
        let sql = format!(
            "SELECT {} AS key, {} AS value FROM {}",
            quote_ident(key_col),
            quote_ident(value_col),
            quote_ident(table)
        );
        let Ok(mut stmt) = conn.prepare(&sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row_string(row, "key").unwrap_or_default(),
                row_string(row, "value").unwrap_or_default(),
            ))
        }) else {
            continue;
        };
        for row in rows.filter_map(Result::ok) {
            if let Some(session) = cursor_blob_to_session(path, &row.0, &row.1) {
                sessions.push(session);
            }
        }
    }
    dedupe_sessions(sessions)
}

fn cursor_blob_to_session(path: &Path, key: &str, raw: &str) -> Option<P0Session> {
    if !key.to_ascii_lowercase().contains("session") {
        return None;
    }
    let decoded = decode_hex(raw).and_then(|bytes| String::from_utf8(bytes).ok());
    let text = decoded.as_deref().unwrap_or(raw);
    let value: Value = serde_json::from_str(text).ok()?;
    let session_id = string_field(&value, &["id", "sessionId", "session_id"])?;
    let mut acc = SessionAcc::new(
        P0Tool::Cursor,
        session_id.clone(),
        "cursor_store_db",
        "cursor:store-db",
        P0SourceType::Sqlite,
        "cursor-store-db-metadata-v1",
        P0SchemaConfidence::Low,
        "experimental",
    );
    acc.source_path = Some(path.display().to_string());
    acc.workspace_path = string_field(&value, &["workspace", "workspacePath", "cwd"]);
    acc.model = string_field(&value, &["model"]);
    acc.touch(string_field(
        &value,
        &["updated_at", "updatedAt", "timestamp"],
    ));
    acc.cost_kind = P0CostKind::NotAvailable;
    acc.resume_command = Some(format!("agent --resume {}", shell_quote(&session_id)));
    acc.finish()
}

fn sqlite_usage_by_session(
    conn: &Connection,
    table: &str,
) -> HashMap<String, (u64, u64, u64, u64)> {
    if !table_exists(conn, table) {
        return HashMap::new();
    }
    let columns = table_columns(conn, table);
    if !columns.contains("session_id") {
        return HashMap::new();
    }
    let sql = format!(
        "SELECT session_id, SUM({}) AS input_tokens, SUM({}) AS output_tokens, \
         SUM({}) AS cache_read_tokens, SUM({}) AS cache_write_tokens FROM {} GROUP BY session_id",
        first_existing_column(&columns, &["input_tokens", "prompt_tokens"]).unwrap_or("0"),
        first_existing_column(&columns, &["output_tokens", "completion_tokens"]).unwrap_or("0"),
        first_existing_column(&columns, &["cache_read_tokens"]).unwrap_or("0"),
        first_existing_column(&columns, &["cache_write_tokens"]).unwrap_or("0"),
        quote_ident(table)
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return HashMap::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row_string(row, "session_id").unwrap_or_default(),
            (
                row_u64(row, "input_tokens"),
                row_u64(row, "output_tokens"),
                row_u64(row, "cache_read_tokens"),
                row_u64(row, "cache_write_tokens"),
            ),
        ))
    }) else {
        return HashMap::new();
    };
    rows.filter_map(Result::ok)
        .filter(|(session_id, _)| !session_id.is_empty())
        .collect()
}

fn open_readonly_db(path: &Path) -> Option<Connection> {
    if path_has_sensitive_component(path) || !path.exists() {
        return None;
    }
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1 LIMIT 1",
        [table],
        |_| Ok(()),
    )
    .is_ok()
}

fn table_columns(conn: &Connection, table: &str) -> HashSet<String> {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({})", quote_ident(table))) else {
        return HashSet::new();
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return HashSet::new();
    };
    rows.filter_map(Result::ok).collect()
}

fn first_existing_column<'a>(
    columns: &'a HashSet<String>,
    candidates: &[&'a str],
) -> Option<&'a str> {
    candidates
        .iter()
        .copied()
        .find(|candidate| columns.contains(*candidate))
}

fn select_alias(columns: &HashSet<String>, candidates: &[&str], alias: &str) -> String {
    candidates
        .iter()
        .find(|candidate| columns.contains(**candidate))
        .map(|column| format!("{} AS {}", quote_ident(column), quote_ident(alias)))
        .unwrap_or_else(|| format!("NULL AS {}", quote_ident(alias)))
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn row_string(row: &rusqlite::Row<'_>, name: &str) -> Option<String> {
    match row.get_ref(name).ok()? {
        ValueRef::Null => None,
        ValueRef::Text(bytes) => std::str::from_utf8(bytes)
            .ok()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from),
        ValueRef::Integer(value) => Some(value.to_string()),
        ValueRef::Real(value) => Some(value.to_string()),
        ValueRef::Blob(bytes) => std::str::from_utf8(bytes)
            .ok()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from),
    }
}

fn row_u64(row: &rusqlite::Row<'_>, name: &str) -> u64 {
    match row.get_ref(name).ok() {
        Some(ValueRef::Integer(value)) if value > 0 => value as u64,
        Some(ValueRef::Real(value)) if value > 0.0 => value as u64,
        Some(ValueRef::Text(bytes)) => std::str::from_utf8(bytes)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(0),
        _ => 0,
    }
}

fn collect_files(root: &Path, extension: &str, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_files_inner(root, extension, max_depth, &mut out);
    out
}

fn collect_files_inner(root: &Path, extension: &str, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 || path_has_sensitive_component(root) {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path_has_sensitive_component(&path) {
            continue;
        }
        if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            collect_files_inner(&path, extension, depth - 1, out);
        } else if path.extension().is_some_and(|ext| ext == extension) {
            out.push(path);
        }
    }
}

fn read_jsonl_values(path: &Path) -> Option<Vec<Value>> {
    if path_has_sensitive_component(path) {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect(),
    )
}

fn read_json_file(path: &Path) -> Option<Value> {
    if path_has_sensitive_component(path) {
        return None;
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|v| match v {
            Value::String(s) => {
                let trimmed = s.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
    })
}

fn u64_field(value: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|v| match v {
            Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
            Value::String(s) => s.parse::<u64>().ok(),
            _ => None,
        })
        .unwrap_or(0)
}

fn f64_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|v| match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        })
}

fn extract_text(value: &Value) -> Option<String> {
    if let Some(text) = string_field(value, &["content", "text", "summary"]) {
        return Some(text);
    }
    value
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                (string_field(item, &["type"]).as_deref() == Some("text"))
                    .then(|| string_field(item, &["text"]))
                    .flatten()
            })
        })
}

fn project_name_from_path(path: &str, fallback: &str) -> String {
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    let clean = value.trim().strip_prefix("0x").unwrap_or(value.trim());
    if !clean.len().is_multiple_of(2) {
        return None;
    }
    (0..clean.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&clean[idx..idx + 2], 16).ok())
        .collect()
}

fn dedupe_sessions(sessions: Vec<P0Session>) -> Vec<P0Session> {
    let mut seen = HashSet::new();
    sessions
        .into_iter()
        .filter(|session| seen.insert((session.tool, session.session_id.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/agents")
            .join(path)
    }

    #[test]
    fn codebuddy_fixture_parses_claude_like_usage() {
        let path = fixture("codebuddy/2.105.0/positive-claude-like-jsonl/session.jsonl");
        let sessions = parse_claude_like_jsonl(
            &path,
            P0Tool::CodeBuddy,
            "codebuddy_transcript",
            "codebuddy:transcript",
            "claude-like-v1",
            "fixture-gated-monitored",
            |id| format!("codebuddy --resume {id}"),
        );
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "cb-session-1");
        assert_eq!(sessions[0].input_tokens, 1200);
        assert_eq!(sessions[0].total_tokens, 1620);
    }

    #[test]
    fn gemini_fixture_honors_rewind_to_usage() {
        let path = fixture("gemini/source-2026-06-10/positive-chat-jsonl/chat.jsonl");
        let sessions = parse_chat_recording_jsonl(
            &path,
            P0Tool::Gemini,
            "gemini_chat_recording",
            "gemini:chat-recording",
            "chat-recording-v1",
            "fixture-gated-monitored",
            |id| format!("gemini --resume {id}"),
        );
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "gemini-session-1");
        assert_eq!(sessions[0].input_tokens, 1000);
        assert_eq!(sessions[0].output_tokens, 260);
        assert_eq!(sessions[0].total_tokens, 1300);
        assert_eq!(sessions[0].tool_specific["rewound"], true);
    }

    #[test]
    fn qwen_fixture_parses_chat_recording_usage() {
        let path = fixture("qwen/source-2026-06-10/positive-chat-jsonl/chat.jsonl");
        let sessions = parse_chat_recording_jsonl(
            &path,
            P0Tool::Qwen,
            "qwen_chat_recording",
            "qwen:chat-recording",
            "chat-recording-v1",
            "fixture-gated-monitored",
            |id| format!("qwen --resume {id}"),
        );
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "qwen-session-1");
        assert_eq!(sessions[0].total_tokens, 1010);
    }

    #[test]
    fn pi_fixture_parses_branch_usage_and_cost() {
        let path = fixture("pi/source-2026-06-10/positive-session-jsonl/session.jsonl");
        let sessions = parse_pi_jsonl(&path);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "pi-session-1");
        assert_eq!(sessions[0].cache_read_tokens, 40);
        assert_eq!(sessions[0].cost_usd, Some(0.03));
        assert_eq!(sessions[0].tool_specific["branchNodes"], 1);
    }

    #[test]
    fn continue_fixture_parses_monitored_lite_session() {
        let path = fixture("continue-cn/source-2026-06-10/positive-session-json/session.json");
        let session = parse_continue_session_json(&path).expect("session");
        assert_eq!(session.session_id, "continue-session-1");
        assert_eq!(session.total_tokens, 930);
        assert_eq!(session.support_level, "monitored-lite");
    }

    #[test]
    fn kimi_fixture_joins_index_state_and_wire() {
        let root = fixture("kimi/source-2026-06-10/positive-session-index-wire");
        let sessions = parse_kimi_session_root(&root);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "kimi-session-1");
        assert_eq!(
            sessions[0].workspace_path,
            "/Users/demo/workspace/octomonitor"
        );
        assert_eq!(sessions[0].total_tokens, 1160);
    }

    #[test]
    fn copilot_fixture_parses_chronicle_state() {
        let path = fixture("copilot/source-2026-06-10/positive-chronicle-state/session.json");
        let session = parse_copilot_state_json(&path).expect("session");
        assert_eq!(session.session_id, "copilot-session-1");
        assert_eq!(session.total_tokens, 780);
    }

    #[test]
    fn openhands_fixture_parses_metadata_without_body_text() {
        let path =
            fixture("openhands/source-2026-06-10/positive-conversation-json/conversation.json");
        let session = parse_openhands_conversation_json(&path).expect("session");
        assert_eq!(session.session_id, "openhands-session-1");
        assert_eq!(session.message_count, 2);
        assert_eq!(session.first_question, None);
        assert_eq!(session.cost_usd, Some(0.01));
    }

    #[test]
    fn opencode_sqlite_fixture_parses_usage() {
        let temp = tempdir().expect("temp dir");
        let db = temp.path().join("opencode.db");
        {
            let conn = Connection::open(&db).expect("db");
            conn.execute_batch(
                r#"
                CREATE TABLE sessions (
                  id TEXT PRIMARY KEY,
                  project TEXT,
                  model TEXT,
                  updated_at TEXT
                );
                CREATE TABLE messages (
                  id TEXT,
                  session_id TEXT,
                  input_tokens INTEGER,
                  output_tokens INTEGER
                );
                INSERT INTO sessions VALUES (
                  'opencode-session-1',
                  '/Users/demo/workspace/octomonitor',
                  'gpt-5-codex',
                  '2026-06-10T01:30:00Z'
                );
                INSERT INTO messages VALUES ('m1', 'opencode-session-1', 1400, 360);
                "#,
            )
            .expect("schema");
        }
        let sessions = parse_opencode_db(&db);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "opencode-session-1");
        assert_eq!(sessions[0].total_tokens, 1760);
    }

    #[test]
    fn goose_sqlite_fixture_parses_metadata_only() {
        let temp = tempdir().expect("temp dir");
        let db = temp.path().join("sessions.db");
        {
            let conn = Connection::open(&db).expect("db");
            conn.execute_batch(
                r#"
                CREATE TABLE sessions (
                  id TEXT PRIMARY KEY,
                  title TEXT,
                  workspace TEXT,
                  updated_at TEXT
                );
                INSERT INTO sessions VALUES (
                  'goose-session-1',
                  'Goose fixture',
                  '/Users/demo/workspace/octomonitor',
                  '2026-06-10T01:25:00Z'
                );
                "#,
            )
            .expect("schema");
        }
        let sessions = parse_goose_db(&db);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "goose-session-1");
        assert_eq!(sessions[0].cost_kind, P0CostKind::NotAvailable);
    }

    #[test]
    fn cline_sqlite_fixture_parses_metadata_without_resume() {
        let temp = tempdir().expect("temp dir");
        let db = temp.path().join("sessions.db");
        {
            let conn = Connection::open(&db).expect("db");
            conn.execute_batch(
                r#"
                CREATE TABLE sessions (
                  id TEXT PRIMARY KEY,
                  workspace TEXT NOT NULL,
                  title TEXT,
                  updated_at TEXT NOT NULL
                );
                INSERT INTO sessions VALUES (
                  'cline-session-1',
                  '/Users/demo/workspace/octomonitor',
                  'Metadata fixture',
                  '2026-06-10T01:05:00Z'
                );
                "#,
            )
            .expect("schema");
        }
        let sessions = scan_cline(&db);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "cline-session-1");
        assert_eq!(sessions[0].support_level, "fixture-gated-metadata");
        assert_eq!(sessions[0].resume_command, None);
    }

    #[test]
    fn kiro_fixture_parses_custom_storage_resume_id() {
        let path =
            fixture("kiro/source-2026-06-10/positive-custom-storage-json/custom-storage.jsonl");
        let sessions = parse_kiro_custom_storage_jsonl(&path);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "kiro-session-1");
        assert_eq!(
            sessions[0].resume_command.as_deref(),
            Some("kiro-cli chat --resume-id kiro-session-1")
        );
    }

    #[test]
    fn cursor_store_decodes_hex_json_metadata_without_usage() {
        let temp = tempdir().expect("temp dir");
        let db = temp.path().join("store.db");
        {
            let conn = Connection::open(&db).expect("db");
            conn.execute_batch(
                r#"
                CREATE TABLE blobs (key TEXT, value TEXT);
                INSERT INTO blobs VALUES (
                  'session',
                  '7b226964223a22637572736f722d73657373696f6e2d31222c22776f726b7370616365223a222f55736572732f64656d6f2f776f726b73706163652f6f63746f6d6f6e69746f72222c226d6f64656c223a22637572736f722d6167656e74227d'
                );
                "#,
            )
            .expect("schema");
        }
        let sessions = parse_cursor_store_db(&db);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "cursor-session-1");
        assert_eq!(sessions[0].cost_kind, P0CostKind::NotAvailable);
        assert!(!sessions[0].enters_usage_totals);
    }
}
