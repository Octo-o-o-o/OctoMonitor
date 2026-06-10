use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use octomonitor_core::ToolKind;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{config::config_path, platform::home_relative_path};

const MANAGED_HOOK_NAME: &str = "octomonitor-live-state";
const MANAGED_HOOK_STATUS: &str = "OctoMonitor live state";
const MANAGED_HOOK_DESCRIPTION: &str = "Observe-only OctoMonitor live state ingest";
const EMPTY_JSON_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HookAction {
    Install,
    Uninstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookSchema {
    ClaudeCompatible,
    Codex,
    Gemini,
    Qwen,
}

#[derive(Debug, Clone, Copy)]
struct HookEvent {
    name: &'static str,
    matcher: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct HookToolSpec {
    tool: ToolKind,
    slug: &'static str,
    env_dir: Option<&'static str>,
    default_path: &'static str,
    file_name: &'static str,
    schema: Option<HookSchema>,
    events: &'static [HookEvent],
    unsupported_reason: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookManagerState {
    pub tool: ToolKind,
    pub supported: bool,
    pub installed: bool,
    pub target_path: Option<String>,
    pub target_exists: bool,
    pub writable: bool,
    pub parse_error: Option<String>,
    pub warnings: Vec<String>,
    pub unsupported_reason: Option<String>,
    pub last_audit_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookPlan {
    pub tool: ToolKind,
    pub action: HookAction,
    pub supported: bool,
    pub installed_before: bool,
    pub target_path: Option<String>,
    pub target_exists: bool,
    pub before_sha256: String,
    pub after_sha256: Option<String>,
    pub backup_required: bool,
    pub diff: String,
    pub warnings: Vec<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookApplyRequest {
    pub action: HookAction,
    pub expected_before_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookApplyResult {
    pub ok: bool,
    pub tool: ToolKind,
    pub action: HookAction,
    pub target_path: String,
    pub backup_path: Option<String>,
    pub audit_path: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub verified: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookAuditRecord {
    at: String,
    tool: ToolKind,
    action: HookAction,
    target_path: String,
    backup_path: Option<String>,
    before_sha256: String,
    after_sha256: String,
    verified: bool,
}

const CLAUDE_EVENTS: &[HookEvent] = &[
    HookEvent {
        name: "Notification",
        matcher: "",
    },
    HookEvent {
        name: "PermissionRequest",
        matcher: "",
    },
    HookEvent {
        name: "Stop",
        matcher: "",
    },
];

const CODEX_EVENTS: &[HookEvent] = &[
    HookEvent {
        name: "PermissionRequest",
        matcher: "",
    },
    HookEvent {
        name: "Stop",
        matcher: "",
    },
];

const GEMINI_EVENTS: &[HookEvent] = &[
    HookEvent {
        name: "Notification",
        matcher: "",
    },
    HookEvent {
        name: "AfterAgent",
        matcher: "",
    },
];

const QWEN_EVENTS: &[HookEvent] = &[
    HookEvent {
        name: "Notification",
        matcher: "",
    },
    HookEvent {
        name: "PermissionRequest",
        matcher: "",
    },
    HookEvent {
        name: "Stop",
        matcher: "",
    },
];

const EMPTY_EVENTS: &[HookEvent] = &[];

fn hook_tool_specs() -> Vec<HookToolSpec> {
    vec![
        HookToolSpec {
            tool: ToolKind::Claude,
            slug: "claude",
            env_dir: Some("CLAUDE_CONFIG_DIR"),
            default_path: ".claude/settings.json",
            file_name: "settings.json",
            schema: Some(HookSchema::ClaudeCompatible),
            events: CLAUDE_EVENTS,
            unsupported_reason: None,
        },
        HookToolSpec {
            tool: ToolKind::Codex,
            slug: "codex",
            env_dir: Some("CODEX_HOME"),
            default_path: ".codex/hooks.json",
            file_name: "hooks.json",
            schema: Some(HookSchema::Codex),
            events: CODEX_EVENTS,
            unsupported_reason: None,
        },
        HookToolSpec {
            tool: ToolKind::Gemini,
            slug: "gemini",
            env_dir: Some("GEMINI_HOME"),
            default_path: ".gemini/settings.json",
            file_name: "settings.json",
            schema: Some(HookSchema::Gemini),
            events: GEMINI_EVENTS,
            unsupported_reason: None,
        },
        HookToolSpec {
            tool: ToolKind::CodeBuddy,
            slug: "codeBuddy",
            env_dir: Some("CODEBUDDY_CONFIG_DIR"),
            default_path: ".codebuddy/settings.json",
            file_name: "settings.json",
            schema: Some(HookSchema::ClaudeCompatible),
            events: CLAUDE_EVENTS,
            unsupported_reason: None,
        },
        HookToolSpec {
            tool: ToolKind::Qwen,
            slug: "qwen",
            env_dir: Some("QWEN_CONFIG_DIR"),
            default_path: ".qwen/settings.json",
            file_name: "settings.json",
            schema: Some(HookSchema::Qwen),
            events: QWEN_EVENTS,
            unsupported_reason: None,
        },
        HookToolSpec {
            tool: ToolKind::Kiro,
            slug: "kiro",
            env_dir: None,
            default_path: "",
            file_name: "",
            schema: None,
            events: EMPTY_EVENTS,
            unsupported_reason: Some(
                "Kiro hooks live in a selected agent configuration; no safe global target is available yet.",
            ),
        },
        HookToolSpec {
            tool: ToolKind::Kimi,
            slug: "kimi",
            env_dir: None,
            default_path: "",
            file_name: "",
            schema: None,
            events: EMPTY_EVENTS,
            unsupported_reason: Some(
                "Kimi hook configuration is still detection-only until official fixture evidence lands.",
            ),
        },
        HookToolSpec {
            tool: ToolKind::Hermes,
            slug: "hermes",
            env_dir: None,
            default_path: "",
            file_name: "",
            schema: None,
            events: EMPTY_EVENTS,
            unsupported_reason: Some(
                "Hermes hook doctor/list support is not wired to a reversible config transaction yet.",
            ),
        },
        HookToolSpec {
            tool: ToolKind::OpenCode,
            slug: "openCode",
            env_dir: None,
            default_path: "",
            file_name: "",
            schema: None,
            events: EMPTY_EVENTS,
            unsupported_reason: Some(
                "opencode uses a plugin/server model; config mutation is blocked until plugin fixtures are locked.",
            ),
        },
        HookToolSpec {
            tool: ToolKind::Cline,
            slug: "cline",
            env_dir: None,
            default_path: "",
            file_name: "",
            schema: None,
            events: EMPTY_EVENTS,
            unsupported_reason: Some(
                "Cline managed hook directories remain fixture-gated and are not safe to write yet.",
            ),
        },
    ]
}

pub fn parse_tool_kind(raw: &str) -> Option<ToolKind> {
    serde_json::from_value(Value::String(raw.to_string())).ok()
}

fn spec_for_tool(tool: ToolKind) -> Option<HookToolSpec> {
    hook_tool_specs().into_iter().find(|spec| spec.tool == tool)
}

fn target_path(spec: HookToolSpec) -> Option<PathBuf> {
    if spec.schema.is_none() {
        return None;
    }
    if let Some(env_key) = spec.env_dir {
        if let Some(raw) = env::var_os(env_key) {
            return Some(PathBuf::from(raw).join(spec.file_name));
        }
    }
    Some(home_relative_path(spec.default_path))
}

fn octomonitor_data_dir() -> PathBuf {
    config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home_relative_path(".octomonitor"))
}

fn audit_path() -> PathBuf {
    octomonitor_data_dir().join("hook-audit.jsonl")
}

fn backup_dir(spec: HookToolSpec) -> PathBuf {
    octomonitor_data_dir().join("hook-backups").join(spec.slug)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn read_target(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

fn parse_config(text: Option<&str>) -> Result<Value> {
    match text {
        Some(raw) if !raw.trim().is_empty() => {
            let value: Value = serde_json::from_str(raw).context("parse hook config json")?;
            if value.is_object() {
                Ok(value)
            } else {
                Err(anyhow!("hook config root must be a JSON object"))
            }
        }
        _ => Ok(Value::Object(Map::new())),
    }
}

fn format_config(value: &Value) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map(|text| format!("{text}\n"))
        .context("format hook config json")
}

fn root_object(value: &mut Value) -> Result<&mut Map<String, Value>> {
    value
        .as_object_mut()
        .ok_or_else(|| anyhow!("hook config root must be a JSON object"))
}

fn hooks_object(value: &mut Value) -> Result<&mut Map<String, Value>> {
    let root = root_object(value)?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    hooks
        .as_object_mut()
        .ok_or_else(|| anyhow!("hooks field must be a JSON object"))
}

fn existing_hooks_object(value: &mut Value) -> Result<Option<&mut Map<String, Value>>> {
    let root = root_object(value)?;
    match root.get_mut("hooks") {
        Some(Value::Object(hooks)) => Ok(Some(hooks)),
        Some(_) => Err(anyhow!("hooks field must be a JSON object")),
        None => Ok(None),
    }
}

fn command_for_tool(spec: HookToolSpec) -> String {
    format!(
        "python3 -c 'import json,sys; d=json.load(sys.stdin); event=d.get(\"hook_event_name\") or d.get(\"event\") or d.get(\"name\") or \"hook\"; print(json.dumps({{\"marker\":\"{MANAGED_HOOK_NAME}\",\"event\":event,\"sessionId\":d.get(\"session_id\") or d.get(\"sessionId\") or d.get(\"conversation_id\") or d.get(\"thread_id\"),\"threadId\":d.get(\"thread_id\") or d.get(\"threadId\"),\"cwd\":d.get(\"cwd\") or d.get(\"workspace_path\"),\"transcriptPath\":d.get(\"transcript_path\"),\"waitingOnApproval\":\"permission\" in str(event).lower()}}))' | curl -fsS --max-time 1 -H 'Content-Type: application/json' -d @- http://127.0.0.1:46321/api/hooks/ingest/{}/hook >/dev/null 2>&1; printf '{{}}\\n'",
        spec.slug
    )
}

fn managed_hook_value(spec: HookToolSpec, command: &str) -> Value {
    match spec.schema.expect("supported schema") {
        HookSchema::ClaudeCompatible | HookSchema::Codex => json!({
            "type": "command",
            "command": command,
            "statusMessage": MANAGED_HOOK_STATUS,
            "timeout": 5
        }),
        HookSchema::Gemini => json!({
            "name": MANAGED_HOOK_NAME,
            "type": "command",
            "command": command,
            "timeout": 5000,
            "description": MANAGED_HOOK_DESCRIPTION
        }),
        HookSchema::Qwen => json!({
            "name": MANAGED_HOOK_NAME,
            "type": "command",
            "command": command,
            "timeout": 5000,
            "description": MANAGED_HOOK_DESCRIPTION,
            "statusMessage": MANAGED_HOOK_STATUS
        }),
    }
}

fn is_managed_hook(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.get("name").and_then(Value::as_str) == Some(MANAGED_HOOK_NAME)
        || object
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| command.contains(MANAGED_HOOK_NAME))
        || object.get("statusMessage").and_then(Value::as_str) == Some(MANAGED_HOOK_STATUS)
}

fn has_managed_hook(value: &Value) -> bool {
    let Some(hooks) = value.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    hooks.values().any(|groups| {
        groups.as_array().is_some_and(|groups| {
            groups.iter().any(|group| {
                group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|hooks| hooks.iter().any(is_managed_hook))
            })
        })
    })
}

fn matcher_matches(group: &Value, matcher: &str) -> bool {
    group.get("matcher").and_then(Value::as_str).unwrap_or("") == matcher
}

fn ensure_event_group<'a>(
    groups: &'a mut Vec<Value>,
    event: HookEvent,
) -> Result<&'a mut Map<String, Value>> {
    let existing_index = groups
        .iter()
        .position(|group| matcher_matches(group, event.matcher));
    let index = match existing_index {
        Some(index) => index,
        None => {
            groups.push(json!({
                "matcher": event.matcher,
                "hooks": []
            }));
            groups.len() - 1
        }
    };
    groups[index]
        .as_object_mut()
        .ok_or_else(|| anyhow!("hook matcher group must be a JSON object"))
}

fn install_managed_hook(
    mut value: Value,
    spec: HookToolSpec,
    command: &str,
) -> Result<(Value, bool)> {
    let mut changed = false;
    let hooks = hooks_object(&mut value)?;
    for event in spec.events {
        let groups_value = hooks
            .entry(event.name)
            .or_insert_with(|| Value::Array(Vec::new()));
        let groups = groups_value
            .as_array_mut()
            .ok_or_else(|| anyhow!("hook event {} must be a JSON array", event.name))?;
        let group = ensure_event_group(groups, *event)?;
        let hook_list = group
            .entry("hooks")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| anyhow!("hook handlers for {} must be a JSON array", event.name))?;
        if !hook_list.iter().any(is_managed_hook) {
            hook_list.push(managed_hook_value(spec, command));
            changed = true;
        }
    }
    Ok((value, changed))
}

fn uninstall_managed_hook(mut value: Value) -> Result<(Value, bool)> {
    let mut changed = false;
    let Some(hooks) = existing_hooks_object(&mut value)? else {
        return Ok((value, false));
    };
    let event_names = hooks.keys().cloned().collect::<Vec<_>>();
    for event_name in event_names {
        let Some(groups_value) = hooks.get_mut(&event_name) else {
            continue;
        };
        let groups = groups_value
            .as_array_mut()
            .ok_or_else(|| anyhow!("hook event {event_name} must be a JSON array"))?;
        let mut group_indexes_to_remove = Vec::new();
        for (group_index, group) in groups.iter_mut().enumerate() {
            let Some(group_object) = group.as_object_mut() else {
                return Err(anyhow!("hook matcher group must be a JSON object"));
            };
            let Some(hook_list_value) = group_object.get_mut("hooks") else {
                continue;
            };
            let hook_list = hook_list_value
                .as_array_mut()
                .ok_or_else(|| anyhow!("hook handlers for {event_name} must be a JSON array"))?;
            let before_len = hook_list.len();
            hook_list.retain(|hook| !is_managed_hook(hook));
            if hook_list.len() != before_len {
                changed = true;
                if hook_list.is_empty() {
                    group_indexes_to_remove.push(group_index);
                }
            }
        }
        for index in group_indexes_to_remove.into_iter().rev() {
            groups.remove(index);
        }
    }
    Ok((value, changed))
}

fn warnings_for_spec(spec: HookToolSpec, disabled_sources: &[ToolKind]) -> Vec<String> {
    let mut warnings = vec![
        "Preview only shows OctoMonitor-managed hook changes; existing hook commands are not echoed back.".into(),
        "Installed command hooks are observe-only metadata ingests and do not approve, deny, or mutate tool actions.".into(),
        "OctoMonitor uses command hooks by default; native HTTP hook handlers are not installed.".into(),
        "The managed hook command requires python3 and curl on PATH; failures are ignored so the agent flow is not blocked.".into(),
    ];
    if disabled_sources.contains(&spec.tool) {
        warnings.push(
            "This source is currently scan-disabled; hook events will be ignored until source collection is enabled.".into(),
        );
    }
    warnings
}

fn plan_diff(spec: HookToolSpec, action: HookAction, command: &str, changed: bool) -> String {
    if !changed {
        return match action {
            HookAction::Install => {
                "No managed hook changes: OctoMonitor hooks are already installed.".into()
            }
            HookAction::Uninstall => {
                "No managed hook changes: OctoMonitor hooks were not installed.".into()
            }
        };
    }

    let mut lines = vec![
        "--- current hook config (redacted)".to_string(),
        "+++ planned hook config (OctoMonitor managed block only)".to_string(),
    ];
    for event in spec.events {
        lines.push(format!(
            "@@ hooks.{} matcher={:?} @@",
            event.name, event.matcher
        ));
        match action {
            HookAction::Install => lines.push(format!("+ command {MANAGED_HOOK_NAME}: {command}")),
            HookAction::Uninstall => {
                lines.push(format!("- command {MANAGED_HOOK_NAME}: {command}"))
            }
        }
    }
    lines.join("\n")
}

fn compute_plan(spec: HookToolSpec, action: HookAction, disabled_sources: &[ToolKind]) -> HookPlan {
    let supported = spec.schema.is_some();
    let Some(path) = target_path(spec) else {
        return HookPlan {
            tool: spec.tool,
            action,
            supported,
            installed_before: false,
            target_path: None,
            target_exists: false,
            before_sha256: EMPTY_JSON_SHA256.into(),
            after_sha256: None,
            backup_required: false,
            diff: String::new(),
            warnings: Vec::new(),
            blocked_reason: spec.unsupported_reason.map(str::to_string),
        };
    };
    let target_exists = path.exists();
    let before_text = match read_target(&path) {
        Ok(text) => text,
        Err(err) => {
            return HookPlan {
                tool: spec.tool,
                action,
                supported,
                installed_before: false,
                target_path: Some(path.display().to_string()),
                target_exists,
                before_sha256: EMPTY_JSON_SHA256.into(),
                after_sha256: None,
                backup_required: false,
                diff: String::new(),
                warnings: warnings_for_spec(spec, disabled_sources),
                blocked_reason: Some(err.to_string()),
            };
        }
    };
    let before_sha256 = before_text
        .as_ref()
        .map(|text| sha256_hex(text.as_bytes()))
        .unwrap_or_else(|| EMPTY_JSON_SHA256.into());
    let before_value = match parse_config(before_text.as_deref()) {
        Ok(value) => value,
        Err(err) => {
            return HookPlan {
                tool: spec.tool,
                action,
                supported,
                installed_before: false,
                target_path: Some(path.display().to_string()),
                target_exists,
                before_sha256,
                after_sha256: None,
                backup_required: false,
                diff: String::new(),
                warnings: warnings_for_spec(spec, disabled_sources),
                blocked_reason: Some(format!("Cannot safely modify invalid hook JSON: {err}")),
            };
        }
    };
    let installed_before = has_managed_hook(&before_value);
    let command = command_for_tool(spec);
    let mutation = match action {
        HookAction::Install => install_managed_hook(before_value, spec, &command),
        HookAction::Uninstall => uninstall_managed_hook(before_value),
    };
    let (after_value, changed) = match mutation {
        Ok(result) => result,
        Err(err) => {
            return HookPlan {
                tool: spec.tool,
                action,
                supported,
                installed_before,
                target_path: Some(path.display().to_string()),
                target_exists,
                before_sha256,
                after_sha256: None,
                backup_required: false,
                diff: String::new(),
                warnings: warnings_for_spec(spec, disabled_sources),
                blocked_reason: Some(format!("Cannot safely modify hook JSON: {err}")),
            };
        }
    };
    let after_text = match format_config(&after_value) {
        Ok(text) => text,
        Err(err) => {
            return HookPlan {
                tool: spec.tool,
                action,
                supported,
                installed_before,
                target_path: Some(path.display().to_string()),
                target_exists,
                before_sha256,
                after_sha256: None,
                backup_required: false,
                diff: String::new(),
                warnings: warnings_for_spec(spec, disabled_sources),
                blocked_reason: Some(err.to_string()),
            };
        }
    };
    HookPlan {
        tool: spec.tool,
        action,
        supported,
        installed_before,
        target_path: Some(path.display().to_string()),
        target_exists,
        before_sha256: before_sha256.clone(),
        after_sha256: Some(if changed {
            sha256_hex(after_text.as_bytes())
        } else {
            before_sha256.clone()
        }),
        backup_required: target_exists && changed,
        diff: plan_diff(spec, action, &command, changed),
        warnings: warnings_for_spec(spec, disabled_sources),
        blocked_reason: None,
    }
}

pub fn build_hook_plan(
    tool: ToolKind,
    action: HookAction,
    disabled_sources: &[ToolKind],
) -> HookPlan {
    spec_for_tool(tool)
        .map(|spec| compute_plan(spec, action, disabled_sources))
        .unwrap_or_else(|| HookPlan {
            tool,
            action,
            supported: false,
            installed_before: false,
            target_path: None,
            target_exists: false,
            before_sha256: EMPTY_JSON_SHA256.into(),
            after_sha256: None,
            backup_required: false,
            diff: String::new(),
            warnings: Vec::new(),
            blocked_reason: Some(
                "Hook Manager does not have a verified adapter for this tool.".into(),
            ),
        })
}

fn latest_audit_for_tool(tool: ToolKind) -> Option<String> {
    let text = fs::read_to_string(audit_path()).ok()?;
    text.lines().rev().find_map(|line| {
        let value: Value = serde_json::from_str(line).ok()?;
        let record_tool: ToolKind = serde_json::from_value(value.get("tool")?.clone()).ok()?;
        if record_tool == tool {
            value.get("at")?.as_str().map(str::to_string)
        } else {
            None
        }
    })
}

pub fn list_hook_states(disabled_sources: &[ToolKind]) -> Vec<HookManagerState> {
    hook_tool_specs()
        .into_iter()
        .map(|spec| {
            let supported = spec.schema.is_some();
            let Some(path) = target_path(spec) else {
                return HookManagerState {
                    tool: spec.tool,
                    supported,
                    installed: false,
                    target_path: None,
                    target_exists: false,
                    writable: false,
                    parse_error: None,
                    warnings: Vec::new(),
                    unsupported_reason: spec.unsupported_reason.map(str::to_string),
                    last_audit_at: latest_audit_for_tool(spec.tool),
                };
            };
            let target_exists = path.exists();
            let writable = path
                .parent()
                .is_some_and(|parent| parent.exists() || parent.parent().is_some());
            let (installed, parse_error) =
                match read_target(&path).and_then(|text| parse_config(text.as_deref())) {
                    Ok(value) => (has_managed_hook(&value), None),
                    Err(err) => (false, Some(err.to_string())),
                };
            HookManagerState {
                tool: spec.tool,
                supported,
                installed,
                target_path: Some(path.display().to_string()),
                target_exists,
                writable,
                parse_error,
                warnings: warnings_for_spec(spec, disabled_sources),
                unsupported_reason: None,
                last_audit_at: latest_audit_for_tool(spec.tool),
            }
        })
        .collect()
}

fn write_backup(spec: HookToolSpec, before_text: &str, before_sha256: &str) -> Result<PathBuf> {
    let backup_dir = backup_dir(spec);
    fs::create_dir_all(&backup_dir)
        .with_context(|| format!("create backup dir {}", backup_dir.display()))?;
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let path = backup_dir.join(format!("{timestamp}-{before_sha256}.bak"));
    fs::write(&path, before_text).with_context(|| format!("write backup {}", path.display()))?;
    Ok(path)
}

fn atomic_write(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create hook config dir {}", parent.display()))?;
    }
    let temp_path = path.with_extension("octomonitor.tmp");
    fs::write(&temp_path, text)
        .with_context(|| format!("write temp hook config {}", temp_path.display()))?;
    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "atomic rename hook config {} -> {}",
            temp_path.display(),
            path.display()
        )
    })
}

fn append_audit(record: &HookAuditRecord) -> Result<PathBuf> {
    let path = audit_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create audit dir {}", parent.display()))?;
    }
    let mut line = serde_json::to_string(record).context("serialize hook audit")?;
    line.push('\n');
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open audit log {}", path.display()))?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("append audit log {}", path.display()))?;
    Ok(path)
}

pub fn apply_hook_transaction(
    tool: ToolKind,
    request: HookApplyRequest,
    disabled_sources: &[ToolKind],
) -> Result<HookApplyResult> {
    let spec = spec_for_tool(tool)
        .ok_or_else(|| anyhow!("Hook Manager does not have a verified adapter for this tool"))?;
    if let Some(reason) = spec.unsupported_reason {
        return Err(anyhow!(reason));
    }
    let path = target_path(spec).ok_or_else(|| anyhow!("missing hook target path"))?;
    let before_text = read_target(&path)?.unwrap_or_default();
    let before_sha256 = if before_text.is_empty() {
        EMPTY_JSON_SHA256.into()
    } else {
        sha256_hex(before_text.as_bytes())
    };
    if before_sha256 != request.expected_before_sha256 {
        return Err(anyhow!(
            "Hook config changed after preview; refresh the plan before applying"
        ));
    }
    let before_value = parse_config((!before_text.is_empty()).then_some(before_text.as_str()))?;
    let command = command_for_tool(spec);
    let (after_value, changed) = match request.action {
        HookAction::Install => install_managed_hook(before_value, spec, &command)?,
        HookAction::Uninstall => uninstall_managed_hook(before_value)?,
    };
    let after_text = format_config(&after_value)?;
    let backup_path = if path.exists() && changed {
        Some(write_backup(spec, &before_text, &before_sha256)?)
    } else {
        None
    };
    if changed {
        atomic_write(&path, &after_text)?;
    }
    let verify_text = read_target(&path)?.unwrap_or_default();
    let after_sha256 = if verify_text.is_empty() {
        EMPTY_JSON_SHA256.into()
    } else {
        sha256_hex(verify_text.as_bytes())
    };
    let verify_value = parse_config((!verify_text.is_empty()).then_some(verify_text.as_str()))?;
    let verified = match request.action {
        HookAction::Install => has_managed_hook(&verify_value),
        HookAction::Uninstall => !has_managed_hook(&verify_value),
    };
    if !verified {
        return Err(anyhow!("hook verification failed after write"));
    }
    let audit = HookAuditRecord {
        at: Utc::now().to_rfc3339(),
        tool,
        action: request.action,
        target_path: path.display().to_string(),
        backup_path: backup_path.as_ref().map(|path| path.display().to_string()),
        before_sha256,
        after_sha256,
        verified,
    };
    let audit_path = append_audit(&audit)?;
    Ok(HookApplyResult {
        ok: true,
        tool,
        action: request.action,
        target_path: path.display().to_string(),
        backup_path: backup_path.map(|path| path.display().to_string()),
        audit_path: audit_path.display().to_string(),
        before_sha256: audit.before_sha256,
        after_sha256: audit.after_sha256,
        verified,
        warnings: warnings_for_spec(spec, disabled_sources),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use tempfile::TempDir;

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn set_home(temp: &TempDir) -> MutexGuard<'static, ()> {
        let guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock");
        env::set_var("HOME", temp.path());
        env::set_var("OCTOMONITOR_CONFIG_DIR", temp.path().join(".octomonitor"));
        env::remove_var("CLAUDE_CONFIG_DIR");
        env::remove_var("CODEX_HOME");
        env::remove_var("GEMINI_HOME");
        env::remove_var("CODEBUDDY_CONFIG_DIR");
        env::remove_var("QWEN_CONFIG_DIR");
        guard
    }

    fn plan(tool: ToolKind, action: HookAction) -> HookPlan {
        build_hook_plan(tool, action, &[])
    }

    #[test]
    fn install_plan_preserves_existing_hooks_and_adds_managed_blocks() {
        let temp = TempDir::new().expect("temp dir");
        let _env = set_home(&temp);
        let settings = temp.path().join(".claude/settings.json");
        fs::create_dir_all(settings.parent().expect("parent")).expect("mkdir");
        fs::write(
            &settings,
            r#"{
  "hooks": {
    "Notification": [
      {
        "matcher": "",
        "hooks": [
          { "type": "command", "command": "echo user-hook" }
        ]
      }
    ]
  }
}"#,
        )
        .expect("write settings");

        let request = HookApplyRequest {
            action: HookAction::Install,
            expected_before_sha256: plan(ToolKind::Claude, HookAction::Install).before_sha256,
        };
        let result = apply_hook_transaction(ToolKind::Claude, request, &[]).expect("apply hook");

        assert!(result.verified);
        assert!(result.backup_path.is_some());
        let written: Value =
            serde_json::from_str(&fs::read_to_string(settings).expect("read settings"))
                .expect("json");
        let notification_hooks = written["hooks"]["Notification"][0]["hooks"]
            .as_array()
            .expect("notification hooks");
        assert!(
            notification_hooks
                .iter()
                .any(|hook| hook["command"] == "echo user-hook")
        );
        assert!(has_managed_hook(&written));
    }

    #[test]
    fn uninstall_removes_only_managed_hooks() {
        let temp = TempDir::new().expect("temp dir");
        let _env = set_home(&temp);
        let request = HookApplyRequest {
            action: HookAction::Install,
            expected_before_sha256: plan(ToolKind::Gemini, HookAction::Install).before_sha256,
        };
        apply_hook_transaction(ToolKind::Gemini, request, &[]).expect("install hook");
        let settings = temp.path().join(".gemini/settings.json");
        let mut written: Value =
            serde_json::from_str(&fs::read_to_string(&settings).expect("read settings"))
                .expect("json");
        written["hooks"]["Notification"][0]["hooks"]
            .as_array_mut()
            .expect("hooks")
            .push(json!({
                "name": "user-hook",
                "type": "command",
                "command": "echo still-here"
            }));
        fs::write(&settings, format_config(&written).expect("format")).expect("rewrite");

        let uninstall_plan = plan(ToolKind::Gemini, HookAction::Uninstall);
        let request = HookApplyRequest {
            action: HookAction::Uninstall,
            expected_before_sha256: uninstall_plan.before_sha256,
        };
        let result = apply_hook_transaction(ToolKind::Gemini, request, &[]).expect("uninstall");

        assert!(result.verified);
        let written: Value =
            serde_json::from_str(&fs::read_to_string(settings).expect("read settings"))
                .expect("json");
        assert!(!has_managed_hook(&written));
        assert!(
            written["hooks"]["Notification"][0]["hooks"]
                .as_array()
                .expect("hooks")
                .iter()
                .any(|hook| hook["name"] == "user-hook")
        );
    }

    #[test]
    fn invalid_json_blocks_plan_and_apply() {
        let temp = TempDir::new().expect("temp dir");
        let _env = set_home(&temp);
        let settings = temp.path().join(".qwen/settings.json");
        fs::create_dir_all(settings.parent().expect("parent")).expect("mkdir");
        fs::write(&settings, "{not-json").expect("write invalid");

        let hook_plan = plan(ToolKind::Qwen, HookAction::Install);

        assert!(hook_plan.blocked_reason.is_some());
        let request = HookApplyRequest {
            action: HookAction::Install,
            expected_before_sha256: hook_plan.before_sha256,
        };
        assert!(apply_hook_transaction(ToolKind::Qwen, request, &[]).is_err());
    }

    #[test]
    fn stale_expected_hash_rejects_apply() {
        let temp = TempDir::new().expect("temp dir");
        let _env = set_home(&temp);

        let request = HookApplyRequest {
            action: HookAction::Install,
            expected_before_sha256: "stale".into(),
        };

        let err = apply_hook_transaction(ToolKind::Codex, request, &[]).expect_err("stale hash");
        assert!(err.to_string().contains("changed after preview"));
    }

    #[test]
    fn noop_install_reports_actual_hash_and_no_backup() {
        let temp = TempDir::new().expect("temp dir");
        let _env = set_home(&temp);
        let first_plan = plan(ToolKind::Codex, HookAction::Install);
        apply_hook_transaction(
            ToolKind::Codex,
            HookApplyRequest {
                action: HookAction::Install,
                expected_before_sha256: first_plan.before_sha256,
            },
            &[],
        )
        .expect("first install");

        let second_plan = plan(ToolKind::Codex, HookAction::Install);
        assert_eq!(
            second_plan.after_sha256.as_deref(),
            Some(second_plan.before_sha256.as_str())
        );
        let result = apply_hook_transaction(
            ToolKind::Codex,
            HookApplyRequest {
                action: HookAction::Install,
                expected_before_sha256: second_plan.before_sha256,
            },
            &[],
        )
        .expect("noop install");

        assert_eq!(result.after_sha256, result.before_sha256);
        assert!(result.backup_path.is_none());
    }

    #[test]
    fn unsupported_tools_are_detection_only() {
        let temp = TempDir::new().expect("temp dir");
        let _env = set_home(&temp);

        let hook_plan = plan(ToolKind::Kiro, HookAction::Install);

        assert!(!hook_plan.supported);
        assert!(hook_plan.blocked_reason.is_some());
    }
}
