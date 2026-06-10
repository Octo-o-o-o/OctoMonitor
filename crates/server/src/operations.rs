use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use octomonitor_core::{
    AuditLevel, CapabilityDescriptor, CapabilityFailureMode, RunRecord, ToolKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config::config_path,
    handlers::resume::build_resume_command,
    platform::{expand_home_path, home_relative_path},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDescriptor {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub blocked_reason: Option<String>,
    pub requires_confirmation: bool,
    pub mutates_state: bool,
    pub audit_level: AuditLevel,
    pub failure_mode: CapabilityFailureMode,
    pub capability: Option<CapabilityDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOperationsPayload {
    pub run_id: String,
    pub tool: ToolKind,
    pub operations: Vec<OperationDescriptor>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationApplyRequest {
    pub action: String,
    pub expected_last_activity_at: Option<String>,
    pub confirmed: Option<bool>,
    pub payload: Option<Value>,
    #[cfg(test)]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationApplyResult {
    pub ok: bool,
    pub run_id: String,
    pub tool: ToolKind,
    pub action: String,
    pub command: Option<String>,
    pub opened_path: Option<String>,
    pub audit_path: String,
    pub blocked_reason: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationAuditRecord {
    at: String,
    run_id: String,
    tool: ToolKind,
    action: String,
    status: String,
    reason: Option<String>,
    opened_path: Option<String>,
    command_returned: bool,
    payload_present: bool,
    expected_last_activity_at: Option<String>,
    actual_last_activity_at: String,
}

fn operation_label(id: &str) -> String {
    match id {
        "resume.copyCommand" => "Copy resume command",
        "open.workspace" => "Open workspace",
        "open.sessionDeeplink" => "Open session deep link",
        "codex.appServer" => "Codex app-server",
        "turn.interrupt" => "Interrupt turn",
        "approval.respond" => "Respond to approval",
        _ => id,
    }
    .into()
}

fn capability<'a>(run: &'a RunRecord, id: &str) -> Option<&'a CapabilityDescriptor> {
    run.capabilities
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|capability| capability.id == id)
}

fn operation_for_capability(run: &RunRecord, cap: &CapabilityDescriptor) -> OperationDescriptor {
    let mut available = true;
    let mut blocked_reason = None;
    let mut requires_confirmation = cap.requires_user_confirmation;
    let mut mutates_state = cap.mutates_state;

    match cap.id.as_str() {
        "resume.copyCommand" => {
            if build_resume_command(run).command.is_none() {
                available = false;
                blocked_reason = Some("Run is missing a resume id".into());
            }
        }
        "open.workspace" => {
            requires_confirmation = true;
            mutates_state = true;
            if run.workspace_path.trim().is_empty() {
                available = false;
                blocked_reason = Some("Run is missing a workspace path".into());
            }
        }
        "open.sessionDeeplink" => {
            available = false;
            blocked_reason =
                Some("Native deep links are handled by the desktop UI fallback path.".into());
        }
        "turn.interrupt" => {
            available = false;
            blocked_reason =
                Some("Interrupt requires an attested managed app-server turn target.".into());
        }
        "approval.respond" => {
            available = false;
            blocked_reason =
                Some("Approval response requires the exact native request payload.".into());
        }
        "codex.appServer" => {
            available = false;
            blocked_reason =
                Some("App-server probe is available; mutation execution is not wired.".into());
        }
        _ => {
            available = false;
            blocked_reason =
                Some("Operation is not implemented by the safe operation layer.".into());
        }
    }

    OperationDescriptor {
        id: cap.id.clone(),
        label: operation_label(&cap.id),
        available,
        blocked_reason,
        requires_confirmation,
        mutates_state,
        audit_level: cap.audit_level.clone(),
        failure_mode: cap.failure_mode.clone(),
        capability: Some(cap.clone()),
    }
}

pub fn list_run_operations(run: &RunRecord) -> RunOperationsPayload {
    let operations = run
        .capabilities
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|cap| operation_for_capability(run, cap))
        .collect();
    RunOperationsPayload {
        run_id: run.id.clone(),
        tool: run.tool,
        operations,
    }
}

fn operation_audit_path() -> PathBuf {
    config_path()
        .parent()
        .map(|path| path.join("operation-audit.jsonl"))
        .unwrap_or_else(|| home_relative_path(".octomonitor").join("operation-audit.jsonl"))
}

fn append_operation_audit(record: &OperationAuditRecord) -> Result<PathBuf> {
    let path = operation_audit_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create operation audit dir {}", parent.display()))?;
    }
    let mut line = serde_json::to_string(record).context("serialize operation audit")?;
    line.push('\n');
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open operation audit {}", path.display()))?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("append operation audit {}", path.display()))?;
    Ok(path)
}

fn audit_result(
    run: &RunRecord,
    request: &OperationApplyRequest,
    status: &str,
    reason: Option<String>,
    command_returned: bool,
    opened_path: Option<String>,
) -> Result<PathBuf> {
    append_operation_audit(&OperationAuditRecord {
        at: Utc::now().to_rfc3339(),
        run_id: run.id.clone(),
        tool: run.tool,
        action: request.action.clone(),
        status: status.into(),
        reason,
        opened_path,
        command_returned,
        payload_present: request.payload.is_some(),
        expected_last_activity_at: request.expected_last_activity_at.clone(),
        actual_last_activity_at: run.last_activity_at.clone(),
    })
}

fn blocked(
    run: &RunRecord,
    request: &OperationApplyRequest,
    reason: impl Into<String>,
) -> Result<OperationApplyResult> {
    let reason = reason.into();
    let audit_path = audit_result(run, request, "blocked", Some(reason.clone()), false, None)?;
    Ok(OperationApplyResult {
        ok: false,
        run_id: run.id.clone(),
        tool: run.tool,
        action: request.action.clone(),
        command: None,
        opened_path: None,
        audit_path: audit_path.display().to_string(),
        blocked_reason: Some(reason),
        message: "Operation blocked by safety policy".into(),
    })
}

pub fn apply_run_operation(
    run: &RunRecord,
    request: OperationApplyRequest,
) -> Result<OperationApplyResult> {
    if let Some(expected) = request.expected_last_activity_at.as_deref() {
        if expected != run.last_activity_at {
            return blocked(
                run,
                &request,
                "Run changed after the operation panel loaded",
            );
        }
    }

    let Some(capability) = capability(run, &request.action) else {
        return blocked(
            run,
            &request,
            "Run does not advertise this operation capability",
        );
    };
    let descriptor = operation_for_capability(run, capability);
    if !descriptor.available {
        return blocked(
            run,
            &request,
            descriptor
                .blocked_reason
                .unwrap_or_else(|| "Operation is unavailable".into()),
        );
    }

    match request.action.as_str() {
        "resume.copyCommand" => {
            let payload = build_resume_command(run);
            let Some(command) = payload.command else {
                return blocked(
                    run,
                    &request,
                    payload
                        .note
                        .unwrap_or_else(|| "Resume command unavailable".into()),
                );
            };
            let audit_path = audit_result(run, &request, "completed", None, true, None)?;
            Ok(OperationApplyResult {
                ok: true,
                run_id: run.id.clone(),
                tool: run.tool,
                action: request.action,
                command: Some(command),
                opened_path: None,
                audit_path: audit_path.display().to_string(),
                blocked_reason: None,
                message: "Resume command returned".into(),
            })
        }
        "open.workspace" => {
            if request.confirmed != Some(true) {
                return blocked(
                    run,
                    &request,
                    "Opening a local workspace requires confirmation",
                );
            }
            let path = expand_home_path(&run.workspace_path);
            if !path.exists() {
                return blocked(
                    run,
                    &request,
                    format!("Workspace path does not exist: {}", path.display()),
                );
            }
            if !path.is_dir() {
                return blocked(
                    run,
                    &request,
                    format!("Workspace path is not a directory: {}", path.display()),
                );
            }
            #[cfg(test)]
            let dry_run = request.dry_run.unwrap_or(false);
            #[cfg(not(test))]
            let dry_run = false;
            if !dry_run {
                if let Err(err) = open::that_detached(&path) {
                    return blocked(
                        run,
                        &request,
                        format!("Could not open workspace {}: {err}", path.display()),
                    );
                }
            }
            let opened_path = Some(path.display().to_string());
            let audit_path =
                audit_result(run, &request, "completed", None, false, opened_path.clone())?;
            Ok(OperationApplyResult {
                ok: true,
                run_id: run.id.clone(),
                tool: run.tool,
                action: request.action,
                command: None,
                opened_path,
                audit_path: audit_path.display().to_string(),
                blocked_reason: None,
                message: if dry_run {
                    "Workspace open dry run completed".into()
                } else {
                    "Workspace open requested".into()
                },
            })
        }
        "turn.interrupt" | "approval.respond" | "process.kill" => blocked(
            run,
            &request,
            "Mutating operation requires exact payload or owned-process attestation",
        ),
        _ => blocked(
            run,
            &request,
            "Operation is not implemented by the safe operation layer",
        ),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::test_support::{sample_run_record, ConfigDirGuard};
    use octomonitor_core::{CapabilitySource, SchemaConfidence};

    fn cap(id: &str) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: id.into(),
            source: CapabilitySource::OfficialCli,
            confidence: SchemaConfidence::High,
            mutates_state: false,
            requires_user_confirmation: false,
            requires_managed_process: false,
            can_expose_secrets: false,
            audit_level: AuditLevel::Metadata,
            failure_mode: CapabilityFailureMode::Safe,
        }
    }

    #[test]
    fn list_operations_marks_mutating_capabilities_blocked() {
        let mut run = sample_run_record();
        run.capabilities = Some(vec![cap("resume.copyCommand"), cap("turn.interrupt")]);
        run.thread_id = Some("thread-1".into());

        let payload = list_run_operations(&run);

        let interrupt = payload
            .operations
            .iter()
            .find(|operation| operation.id == "turn.interrupt")
            .expect("interrupt operation");
        assert!(!interrupt.available);
        assert!(interrupt.blocked_reason.is_some());
    }

    #[test]
    fn stale_operation_is_blocked_and_audited() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = ConfigDirGuard::set(temp.path());
        let mut run = sample_run_record();
        run.capabilities = Some(vec![cap("resume.copyCommand")]);
        run.thread_id = Some("thread-1".into());

        let result = apply_run_operation(
            &run,
            OperationApplyRequest {
                action: "resume.copyCommand".into(),
                expected_last_activity_at: Some("stale".into()),
                confirmed: None,
                payload: None,
                dry_run: None,
            },
        )
        .expect("operation result");

        assert!(!result.ok);
        assert!(result
            .blocked_reason
            .as_deref()
            .unwrap_or("")
            .contains("changed"));
        assert!(temp.path().join("operation-audit.jsonl").exists());
    }

    #[test]
    fn open_workspace_requires_confirmation() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = ConfigDirGuard::set(temp.path());
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).expect("workspace");
        let mut run = sample_run_record();
        run.capabilities = Some(vec![cap("open.workspace")]);
        run.workspace_path = workspace.display().to_string();

        let result = apply_run_operation(
            &run,
            OperationApplyRequest {
                action: "open.workspace".into(),
                expected_last_activity_at: Some(run.last_activity_at.clone()),
                confirmed: None,
                payload: None,
                dry_run: Some(true),
            },
        )
        .expect("operation result");

        assert!(!result.ok);
        assert!(result
            .blocked_reason
            .as_deref()
            .unwrap_or("")
            .contains("confirmation"));
    }

    #[test]
    fn open_workspace_missing_path_is_blocked_and_audited() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = ConfigDirGuard::set(temp.path());
        let mut run = sample_run_record();
        run.capabilities = Some(vec![cap("open.workspace")]);
        run.workspace_path = temp.path().join("missing").display().to_string();

        let result = apply_run_operation(
            &run,
            OperationApplyRequest {
                action: "open.workspace".into(),
                expected_last_activity_at: Some(run.last_activity_at.clone()),
                confirmed: Some(true),
                payload: None,
                dry_run: Some(true),
            },
        )
        .expect("operation result");

        assert!(!result.ok);
        assert!(result
            .blocked_reason
            .as_deref()
            .unwrap_or("")
            .contains("does not exist"));
        assert!(temp.path().join("operation-audit.jsonl").exists());
    }

    #[test]
    fn open_workspace_file_path_is_blocked_and_audited() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = ConfigDirGuard::set(temp.path());
        let file_path = temp.path().join("workspace.txt");
        fs::write(&file_path, "not a directory").expect("fixture file");
        let mut run = sample_run_record();
        run.capabilities = Some(vec![cap("open.workspace")]);
        run.workspace_path = file_path.display().to_string();

        let result = apply_run_operation(
            &run,
            OperationApplyRequest {
                action: "open.workspace".into(),
                expected_last_activity_at: Some(run.last_activity_at.clone()),
                confirmed: Some(true),
                payload: None,
                dry_run: Some(true),
            },
        )
        .expect("operation result");

        assert!(!result.ok);
        assert!(result
            .blocked_reason
            .as_deref()
            .unwrap_or("")
            .contains("not a directory"));
        assert!(temp.path().join("operation-audit.jsonl").exists());
    }

    #[test]
    fn open_workspace_dry_run_succeeds_with_confirmation() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = ConfigDirGuard::set(temp.path());
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).expect("workspace");
        let mut run = sample_run_record();
        run.capabilities = Some(vec![cap("open.workspace")]);
        run.workspace_path = workspace.display().to_string();

        let result = apply_run_operation(
            &run,
            OperationApplyRequest {
                action: "open.workspace".into(),
                expected_last_activity_at: Some(run.last_activity_at.clone()),
                confirmed: Some(true),
                payload: None,
                dry_run: Some(true),
            },
        )
        .expect("operation result");

        assert!(result.ok);
        assert_eq!(
            result.opened_path.as_deref(),
            Some(workspace.to_str().unwrap())
        );
    }
}
