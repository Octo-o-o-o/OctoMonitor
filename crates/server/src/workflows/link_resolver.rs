use octomonitor_core::workflow::*;
use octomonitor_core::{RunRecord, WorkflowHint};

/// Attempt strong (automatic) resolution from a workflow hint attached to a run.
/// Returns a `LinkedRunRef` if the hint matches the given step.
pub fn resolve_strong(run: &RunRecord, step: &StepRun, workflow_id: &str) -> Option<LinkedRunRef> {
    let hint = run.workflow_hint.as_ref()?;

    // Explicit match: workflowId + stepId both present and match
    if hint.workflow_id.as_deref() == Some(workflow_id)
        && hint.step_id.as_deref() == Some(&step.step_id)
    {
        return Some(LinkedRunRef {
            run_id: run.id.clone(),
            confidence: LinkConfidence::Explicit,
            matched_by: "context-file".into(),
            linked_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    if hint.workflow_id.as_deref() == Some(workflow_id)
        && hint.step_id.is_none()
        && !hint.artifact_refs.is_empty()
    {
        return Some(LinkedRunRef {
            run_id: run.id.clone(),
            confidence: LinkConfidence::ContextFile,
            matched_by: "context-file-artifact".into(),
            linked_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    None
}

/// Attempt resolution from prompt markers embedded in the run's first/last question.
/// Parses `[octomonitor wf=xxx step=yyy parent=zzz artifact=path]` markers.
pub fn resolve_prompt_marker(
    run: &RunRecord,
    step: &StepRun,
    workflow_id: &str,
) -> Option<LinkedRunRef> {
    let hint = parse_prompt_marker(run)?;

    if hint.workflow_id.as_deref() == Some(workflow_id)
        && hint.step_id.as_deref() == Some(&step.step_id)
    {
        return Some(LinkedRunRef {
            run_id: run.id.clone(),
            confidence: LinkConfidence::PromptMarker,
            matched_by: "prompt-marker".into(),
            linked_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    None
}

/// Find heuristic candidates: runs that match the step's tool and workspace,
/// within a recent time window. These are never auto-linked.
pub fn resolve_candidates(
    step: &StepRun,
    workflow_run: &WorkflowRun,
    monitor_runs: &[RunRecord],
) -> Vec<LinkedRunRef> {
    let already_linked: Vec<&str> = step.linked_runs.iter().map(|l| l.run_id.as_str()).collect();
    let working_dir = &workflow_run.working_dir;

    monitor_runs
        .iter()
        .filter(|r| {
            // Must match the step's tool
            r.tool == step.tool
            // Must not already be linked
            && !already_linked.contains(&r.id.as_str())
            // Same workspace
            && (r.workspace_path == *working_dir
                || working_dir == "."
                || r.workspace_path.ends_with(working_dir))
        })
        .take(10)
        .map(|r| LinkedRunRef {
            run_id: r.id.clone(),
            confidence: LinkConfidence::HeuristicCandidate,
            matched_by: format!("workspace+tool:{}", r.tool_label()),
            linked_at: chrono::Utc::now().to_rfc3339(),
        })
        .collect()
}

/// Parse `[octomonitor wf=xxx step=yyy parent=zzz artifact=path]` from run questions.
fn parse_prompt_marker(run: &RunRecord) -> Option<WorkflowHint> {
    let text = run
        .first_question
        .as_deref()
        .or(run.last_question.as_deref())?;

    // Find the marker pattern
    let start = text.find("[octomonitor ")?;
    let end = text[start..].find(']')? + start;
    let marker = &text[start + 13..end]; // skip "[octomonitor "

    let mut workflow_id = None;
    let mut step_id = None;
    let mut parent_step_id = None;
    let mut artifact_refs = Vec::new();

    for part in marker.split_whitespace() {
        if let Some(val) = part.strip_prefix("wf=") {
            workflow_id = Some(val.to_string());
        } else if let Some(val) = part.strip_prefix("step=") {
            step_id = Some(val.to_string());
        } else if let Some(val) = part.strip_prefix("parent=") {
            parent_step_id = Some(val.to_string());
        } else if let Some(val) = part.strip_prefix("artifact=") {
            artifact_refs.push(val.to_string());
        }
    }

    if workflow_id.is_some() || step_id.is_some() {
        Some(WorkflowHint {
            workflow_id,
            step_id,
            parent_step_id,
            artifact_refs,
            updated_at: None,
        })
    } else {
        None
    }
}

/// Helper trait to get a display label for the tool.
trait ToolLabel {
    fn tool_label(&self) -> &str;
}

impl ToolLabel for RunRecord {
    fn tool_label(&self) -> &str {
        match self.tool {
            octomonitor_core::ToolKind::Claude => "claude",
            octomonitor_core::ToolKind::Codex => "codex",
            octomonitor_core::ToolKind::OpenClaw => "openclaw",
            octomonitor_core::ToolKind::Hermes => "hermes",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use octomonitor_core::*;

    fn mock_run(id: &str, tool: ToolKind, workspace: &str) -> RunRecord {
        RunRecord {
            id: id.into(),
            tool,
            source_mode: "test".into(),
            project_name: "Test".into(),
            workspace_path: workspace.into(),
            workspace_short: workspace.into(),
            model: None,
            provider: None,
            agent_name: None,
            agent_display_name: None,
            account_alias: None,
            auth_mode: None,
            auth_verified: false,
            session_id: None,
            thread_id: None,
            session_key: None,
            transcript_path: None,
            started_at: "2026-04-01T10:00:00Z".into(),
            last_activity_at: "2026-04-01T10:05:00Z".into(),
            elapsed_ms: 300_000,
            state: RunState::Active,
            last_action: None,
            last_tail: None,
            pending_approval: false,
            first_question: None,
            last_question: None,
            error_message: None,
            message_count: 0,
            tokens: TokenUsage::default(),
            cost: MoneyValue {
                usd: None,
                confidence: SourceConfidence::Derived,
            },
            quota: QuotaValue {
                five_hour_used_pct: None,
                seven_day_used_pct: None,
                reset_at: vec![],
                confidence: SourceConfidence::Derived,
            },
            source: SourceInfo {
                confidence: SourceConfidence::Derived,
                freshness: Freshness::Warm,
                last_updated_at: "2026-04-01T10:05:00Z".into(),
            },
            vcs: None,
            origin_label: None,
            origin_provider: None,
            workflow_hint: None,
        }
    }

    fn mock_step(step_id: &str, tool: ToolKind) -> StepRun {
        StepRun {
            step_id: step_id.into(),
            order: 0,
            label: "Test Step".into(),
            tool,
            kind: WorkflowStepKind::Observe,
            state: StepRunState::WaitingLink,
            prompt_rendered: None,
            started_at: None,
            completed_at: None,
            error: None,
            linked_runs: vec![],
            artifacts: vec![],
            completion_source: None,
        }
    }

    #[test]
    fn resolve_strong_explicit_match() {
        let mut run = mock_run("r1", ToolKind::Claude, "/tmp/repo");
        run.workflow_hint = Some(WorkflowHint {
            workflow_id: Some("wf-1".into()),
            step_id: Some("s1".into()),
            parent_step_id: None,
            artifact_refs: vec![],
            updated_at: None,
        });
        let step = mock_step("s1", ToolKind::Claude);
        let result = resolve_strong(&run, &step, "wf-1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().confidence, LinkConfidence::Explicit);
    }

    #[test]
    fn resolve_strong_no_match_wrong_step() {
        let mut run = mock_run("r1", ToolKind::Claude, "/tmp/repo");
        run.workflow_hint = Some(WorkflowHint {
            workflow_id: Some("wf-1".into()),
            step_id: Some("s2".into()),
            parent_step_id: None,
            artifact_refs: vec![],
            updated_at: None,
        });
        let step = mock_step("s1", ToolKind::Claude);
        assert!(resolve_strong(&run, &step, "wf-1").is_none());
    }

    #[test]
    fn resolve_prompt_marker_parses() {
        let mut run = mock_run("r1", ToolKind::Claude, "/tmp/repo");
        run.first_question = Some(
            "Please implement the plan [octomonitor wf=wf-1 step=s1 parent=s0 artifact=docs/plan.md]"
                .into(),
        );
        let step = mock_step("s1", ToolKind::Claude);
        let result = resolve_prompt_marker(&run, &step, "wf-1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().confidence, LinkConfidence::PromptMarker);
    }

    #[test]
    fn resolve_candidates_filters_tool_and_workspace() {
        let runs = vec![
            mock_run("r1", ToolKind::Claude, "/tmp/repo"),
            mock_run("r2", ToolKind::Codex, "/tmp/repo"),
            mock_run("r3", ToolKind::Claude, "/other/repo"),
        ];
        let step = mock_step("s1", ToolKind::Claude);
        let wf_run = WorkflowRun {
            id: "wr-1".into(),
            workflow_id: "wf-1".into(),
            workflow_name: "Test".into(),
            state: WorkflowRunState::Running,
            execution_mode: WorkflowExecutionMode::TrackingOnly,
            working_dir: "/tmp/repo".into(),
            definition_snapshot: WorkflowDef {
                id: "wf-1".into(),
                name: "Test".into(),
                description: None,
                default_working_dir: None,
                steps: vec![],
                created_at: "".into(),
                updated_at: "".into(),
            },
            steps: vec![],
            created_at: "".into(),
            updated_at: "".into(),
            completed_at: None,
        };
        let candidates = resolve_candidates(&step, &wf_run, &runs);
        // Only r1 should match (claude + /tmp/repo)
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].run_id, "r1");
        assert_eq!(candidates[0].confidence, LinkConfidence::HeuristicCandidate);
    }
}
