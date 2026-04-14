use anyhow::{bail, Result};
use chrono::Utc;
use octomonitor_core::workflow::*;

use super::store::WorkflowStore;

pub struct WorkflowCoordinator {
    store: WorkflowStore,
}

impl WorkflowCoordinator {
    pub fn new(store: WorkflowStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &WorkflowStore {
        &self.store
    }

    pub fn create_run(
        &self,
        workflow_id: &str,
        working_dir: &str,
        mode: WorkflowExecutionMode,
    ) -> Result<WorkflowRun> {
        // Enforce: only one active launch-capable workflow run per workspace
        if mode != WorkflowExecutionMode::TrackingOnly {
            let active = self.store.list_runs(None, None);
            let has_active = active.iter().any(|s| {
                matches!(
                    s.state,
                    WorkflowRunState::Running
                        | WorkflowRunState::WaitingInput
                        | WorkflowRunState::WaitingApproval
                ) && self
                    .store
                    .load_run(&s.id)
                    .is_ok_and(|r| r.working_dir == working_dir)
            });
            if has_active {
                bail!("Another active workflow run already exists in this workspace");
            }
        }

        let def = self.store.load_def(workflow_id)?;
        let now = Utc::now().to_rfc3339();
        let run_id = format!("wr-{}", generate_id());

        let mut steps: Vec<StepRun> = def
            .steps
            .iter()
            .map(|s| StepRun {
                step_id: s.id.clone(),
                order: s.order,
                label: s.label.clone(),
                tool: s.tool.clone(),
                kind: s.kind.clone(),
                state: StepRunState::Pending,
                prompt_rendered: None,
                linked_runs: vec![],
                artifacts: vec![],
                completion_source: None,
                started_at: None,
                completed_at: None,
                error: None,
            })
            .collect();

        // Advance first step to initial state (respecting execution mode)
        if let Some(first) = steps.first_mut() {
            first.state = advance_step_state(&first.kind, &mode, first.order, &def);
            first.started_at = Some(now.clone());
        }

        let run_state = if steps.is_empty() {
            WorkflowRunState::Completed
        } else {
            derive_run_state(&steps)
        };

        let run = WorkflowRun {
            id: run_id,
            workflow_id: def.id.clone(),
            workflow_name: def.name.clone(),
            execution_mode: mode,
            working_dir: working_dir.to_string(),
            state: run_state,
            definition_snapshot: def,
            steps,
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
        };

        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn link_run(
        &self,
        run_id: &str,
        step_id: &str,
        monitor_run_id: &str,
        confidence: LinkConfidence,
        matched_by: &str,
    ) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        let step = find_step_mut(&mut run.steps, step_id)?;

        // Don't add duplicate links
        if step.linked_runs.iter().any(|l| l.run_id == monitor_run_id) {
            return Ok(run);
        }

        step.linked_runs.push(LinkedRunRef {
            run_id: monitor_run_id.to_string(),
            confidence,
            matched_by: matched_by.to_string(),
            linked_at: Utc::now().to_rfc3339(),
        });

        run.updated_at = Utc::now().to_rfc3339();
        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn unlink_run(
        &self,
        run_id: &str,
        step_id: &str,
        monitor_run_id: &str,
    ) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        let step = find_step_mut(&mut run.steps, step_id)?;
        step.linked_runs.retain(|l| l.run_id != monitor_run_id);
        run.updated_at = Utc::now().to_rfc3339();
        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn complete_step(
        &self,
        run_id: &str,
        step_id: &str,
        source: CompletionSource,
    ) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        let now = Utc::now().to_rfc3339();

        {
            let step = find_step_mut(&mut run.steps, step_id)?;

            // HeuristicSuggestion cannot complete a step
            if source == CompletionSource::HeuristicSuggestion {
                bail!("HeuristicSuggestion cannot be used to complete a step");
            }

            if step.state == StepRunState::Completed {
                return Ok(run);
            }

            // Validate CompletionPolicy: check required_artifacts are present
            let def_step = run
                .definition_snapshot
                .steps
                .iter()
                .find(|s| s.id == step.step_id);
            if let Some(def) = def_step {
                let artifact_paths: Vec<&str> = step
                    .artifacts
                    .iter()
                    .filter_map(|a| a.path.as_deref())
                    .collect();
                for required in &def.completion.required_artifacts {
                    if !artifact_paths.iter().any(|p| p.contains(required.as_str())) {
                        bail!(
                            "Required artifact '{}' not found in step artifacts",
                            required
                        );
                    }
                }
            }

            step.state = StepRunState::Completed;
            step.completion_source = Some(source);
            step.completed_at = Some(now.clone());
        }

        // Advance next pending step
        self.advance_next_step(&mut run, &now);

        run.state = derive_run_state(&run.steps);
        if run.state == WorkflowRunState::Completed {
            run.completed_at = Some(now.clone());
        }
        run.updated_at = now;

        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn fail_step(
        &self,
        run_id: &str,
        step_id: &str,
        error: Option<String>,
    ) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        let now = Utc::now().to_rfc3339();

        let step = find_step_mut(&mut run.steps, step_id)?;
        step.state = StepRunState::Failed;
        step.error = error;
        step.completed_at = Some(now.clone());

        run.state = WorkflowRunState::Failed;
        run.updated_at = now;
        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn skip_step(&self, run_id: &str, step_id: &str) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        let now = Utc::now().to_rfc3339();

        {
            let step = find_step_mut(&mut run.steps, step_id)?;
            step.state = StepRunState::Skipped;
            step.completed_at = Some(now.clone());
        }

        self.advance_next_step(&mut run, &now);
        run.state = derive_run_state(&run.steps);
        if run.state == WorkflowRunState::Completed {
            run.completed_at = Some(now.clone());
        }
        run.updated_at = now;

        self.store.save_run(&run)?;
        Ok(run)
    }

    /// Retry a failed step: reset it to its initial state.
    pub fn retry_step(&self, run_id: &str, step_id: &str) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        let now = Utc::now().to_rfc3339();

        let step = find_step_mut(&mut run.steps, step_id)?;
        if step.state != StepRunState::Failed {
            bail!("Only failed steps can be retried");
        }

        step.state = advance_step_state(
            &step.kind.clone(),
            &run.execution_mode,
            step.order,
            &run.definition_snapshot,
        );
        step.error = None;
        step.completion_source = None;
        step.completed_at = None;
        step.started_at = Some(now.clone());

        run.state = derive_run_state(&run.steps);
        run.updated_at = now;
        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn cancel_run(&self, run_id: &str) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        let now = Utc::now().to_rfc3339();

        for step in &mut run.steps {
            if !matches!(
                step.state,
                StepRunState::Completed | StepRunState::Skipped | StepRunState::Failed
            ) {
                step.state = StepRunState::Cancelled;
            }
        }

        run.state = WorkflowRunState::Cancelled;
        run.updated_at = now;
        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn change_mode(&self, run_id: &str, mode: WorkflowExecutionMode) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;
        run.execution_mode = mode;
        run.updated_at = Utc::now().to_rfc3339();
        self.store.save_run(&run)?;
        Ok(run)
    }

    pub fn get_summary_list(&self) -> Vec<WorkflowRunSummary> {
        self.store.get_all_summaries()
    }

    /// Return IDs of active runs for a given workflow definition.
    pub fn list_runs_for_def(&self, workflow_id: &str) -> Result<Vec<String>> {
        let summaries = self.store.list_runs(None, None);
        Ok(summaries
            .into_iter()
            .filter(|s| {
                s.workflow_id == workflow_id
                    && matches!(
                        s.state,
                        WorkflowRunState::Running
                            | WorkflowRunState::WaitingInput
                            | WorkflowRunState::WaitingApproval
                            | WorkflowRunState::Pending
                    )
            })
            .map(|s| s.id)
            .collect())
    }

    /// Get a preview of what would be launched for a step.
    pub fn get_launch_preview(
        &self,
        run_id: &str,
        step_id: &str,
        monitor_runs: &[octomonitor_core::RunRecord],
    ) -> Result<LaunchPreview> {
        let run = self.store.load_run(run_id)?;
        let step = run
            .steps
            .iter()
            .find(|s| s.step_id == step_id)
            .ok_or_else(|| anyhow::anyhow!("Step not found"))?;
        let def_step = run
            .definition_snapshot
            .steps
            .iter()
            .find(|s| s.id == step_id);

        let template = def_step
            .and_then(|d| d.prompt_template.as_deref())
            .unwrap_or("");

        let rendered =
            super::prompt::render_prompt(template, step, &run, &run.working_dir, monitor_runs);

        let launch = def_step.and_then(|d| d.launch.as_ref());

        Ok(LaunchPreview {
            rendered_prompt: rendered.clone(),
            input_artifacts: def_step.map(|d| d.inputs.clone()).unwrap_or_default(),
            model: launch.and_then(|l| l.model.clone()),
            allowed_tools: launch.map(|l| l.allowed_tools.clone()).unwrap_or_default(),
            estimated_prompt_chars: rendered.len(),
        })
    }

    /// Approve a launch step: transition from WaitingApproval → Running.
    /// The caller is responsible for actually spawning the tool launcher after this call.
    pub fn approve_step(&self, run_id: &str, step_id: &str) -> Result<WorkflowRun> {
        let mut run = self.store.load_run(run_id)?;

        if run.execution_mode == WorkflowExecutionMode::TrackingOnly {
            bail!("Cannot approve in TrackingOnly mode");
        }

        let now = Utc::now().to_rfc3339();
        let step = find_step_mut(&mut run.steps, step_id)?;

        if step.kind != WorkflowStepKind::Launch {
            bail!("Only launch steps can be approved");
        }
        if step.state != StepRunState::WaitingApproval && step.state != StepRunState::Ready {
            bail!(
                "Step is not in an approvable state (current: {:?})",
                step.state
            );
        }

        step.state = StepRunState::Running;
        step.started_at = Some(now.clone());

        run.state = derive_run_state(&run.steps);
        run.updated_at = now;
        self.store.save_run(&run)?;
        Ok(run)
    }

    // --- Private helpers ---

    fn advance_next_step(&self, run: &mut WorkflowRun, now: &str) {
        if let Some(next) = run
            .steps
            .iter_mut()
            .find(|s| s.state == StepRunState::Pending)
        {
            next.state = advance_step_state(
                &next.kind,
                &run.execution_mode,
                next.order,
                &run.definition_snapshot,
            );
            next.started_at = Some(now.to_string());
        }
    }
}

/// Determine the next state for a step based on its kind, the execution mode,
/// and whether approval is required.
fn advance_step_state(
    kind: &WorkflowStepKind,
    mode: &WorkflowExecutionMode,
    order: u32,
    def: &WorkflowDef,
) -> StepRunState {
    match kind {
        WorkflowStepKind::Observe => StepRunState::WaitingLink,
        WorkflowStepKind::Launch => {
            let step_def = def.steps.iter().find(|s| s.order == order);
            let approval_required = step_def.is_some_and(|s| s.approval_required);
            let auto_eligible = step_def.is_some_and(|s| s.auto_advance_eligible);

            match mode {
                WorkflowExecutionMode::TrackingOnly => StepRunState::Ready,
                WorkflowExecutionMode::Assisted => StepRunState::WaitingApproval,
                WorkflowExecutionMode::Auto => {
                    if approval_required || !auto_eligible {
                        StepRunState::WaitingApproval
                    } else {
                        StepRunState::Ready
                    }
                }
            }
        }
    }
}

fn derive_run_state(steps: &[StepRun]) -> WorkflowRunState {
    let all_done = steps
        .iter()
        .all(|s| matches!(s.state, StepRunState::Completed | StepRunState::Skipped));
    if all_done {
        return WorkflowRunState::Completed;
    }

    if steps.iter().any(|s| s.state == StepRunState::Failed) {
        return WorkflowRunState::Failed;
    }

    if steps
        .iter()
        .any(|s| s.state == StepRunState::WaitingApproval)
    {
        return WorkflowRunState::WaitingApproval;
    }

    if steps.iter().any(|s| s.state == StepRunState::WaitingLink) {
        return WorkflowRunState::WaitingInput;
    }

    WorkflowRunState::Running
}

fn find_step_mut<'a>(steps: &'a mut [StepRun], step_id: &str) -> Result<&'a mut StepRun> {
    steps
        .iter_mut()
        .find(|s| s.step_id == step_id)
        .ok_or_else(|| anyhow::anyhow!("Step {} not found", step_id))
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let random: u32 = (ts as u32).wrapping_mul(2654435761);
    format!("{:x}{:x}", ts % 0xFFFFFF, random % 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;
    use octomonitor_core::ToolKind;

    fn setup() -> (tempfile::TempDir, WorkflowCoordinator) {
        let dir = tempfile::tempdir().unwrap();
        let store = WorkflowStore::new(dir.path().join("workflows")).unwrap();
        let coordinator = WorkflowCoordinator::new(store);
        (dir, coordinator)
    }

    fn create_test_def(coordinator: &WorkflowCoordinator) -> WorkflowDef {
        let now = Utc::now().to_rfc3339();
        let def = WorkflowDef {
            id: "wf-test".into(),
            name: "Plan Design & Review".into(),
            description: None,
            default_working_dir: None,
            steps: vec![
                StepDef {
                    id: "s1".into(),
                    order: 0,
                    label: "Write Plan".into(),
                    tool: ToolKind::Claude,
                    kind: WorkflowStepKind::Observe,
                    prompt_template: None,
                    inputs: vec![],
                    outputs: vec![],
                    approval_required: false,
                    auto_advance_eligible: false,
                    completion: CompletionPolicy {
                        mode: CompletionMode::ManualLink,
                        required_artifacts: vec![],
                    },
                    launch: None,
                },
                StepDef {
                    id: "s2".into(),
                    order: 1,
                    label: "Review Plan".into(),
                    tool: ToolKind::Codex,
                    kind: WorkflowStepKind::Observe,
                    prompt_template: None,
                    inputs: vec![],
                    outputs: vec![],
                    approval_required: false,
                    auto_advance_eligible: false,
                    completion: CompletionPolicy {
                        mode: CompletionMode::ManualLink,
                        required_artifacts: vec![],
                    },
                    launch: None,
                },
                StepDef {
                    id: "s3".into(),
                    order: 2,
                    label: "Implement".into(),
                    tool: ToolKind::Claude,
                    kind: WorkflowStepKind::Launch,
                    prompt_template: Some("Implement based on {{file:docs/plan.md}}".into()),
                    inputs: vec![],
                    outputs: vec![],
                    approval_required: true,
                    auto_advance_eligible: true,
                    completion: CompletionPolicy {
                        mode: CompletionMode::LauncherExit,
                        required_artifacts: vec![],
                    },
                    launch: Some(LaunchSpec {
                        model: None,
                        timeout_secs: None,
                        allowed_tools: vec![],
                        args: vec![],
                    }),
                },
            ],
            created_at: now.clone(),
            updated_at: now,
        };
        coordinator.store().save_def(&def).unwrap();
        def
    }

    #[test]
    fn create_run_initializes_first_step() {
        let (_dir, coord) = setup();
        let def = create_test_def(&coord);

        let run = coord
            .create_run(&def.id, "/tmp/test", WorkflowExecutionMode::TrackingOnly)
            .unwrap();

        assert_eq!(run.steps.len(), 3);
        assert_eq!(run.steps[0].state, StepRunState::WaitingLink);
        assert_eq!(run.steps[1].state, StepRunState::Pending);
        assert_eq!(run.steps[2].state, StepRunState::Pending);
        assert_eq!(run.state, WorkflowRunState::WaitingInput);
    }

    #[test]
    fn link_and_complete_advances_pipeline() {
        let (_dir, coord) = setup();
        let def = create_test_def(&coord);

        let run = coord
            .create_run(&def.id, "/tmp/test", WorkflowExecutionMode::TrackingOnly)
            .unwrap();

        // Link a run to step 1
        let run = coord
            .link_run(
                &run.id,
                "s1",
                "ingest-claude-123",
                LinkConfidence::Explicit,
                "user-manual",
            )
            .unwrap();
        assert_eq!(run.steps[0].linked_runs.len(), 1);

        // Complete step 1
        let run = coord
            .complete_step(&run.id, "s1", CompletionSource::UserMarked)
            .unwrap();
        assert_eq!(run.steps[0].state, StepRunState::Completed);
        assert_eq!(run.steps[1].state, StepRunState::WaitingLink); // advanced
        assert_eq!(run.state, WorkflowRunState::WaitingInput);

        // Complete step 2
        let run = coord
            .complete_step(&run.id, "s2", CompletionSource::UserMarked)
            .unwrap();
        assert_eq!(run.steps[1].state, StepRunState::Completed);
        assert_eq!(run.steps[2].state, StepRunState::Ready); // Launch step -> Ready

        // Complete step 3
        let run = coord
            .complete_step(&run.id, "s3", CompletionSource::LauncherExit)
            .unwrap();
        assert_eq!(run.state, WorkflowRunState::Completed);
        assert!(run.completed_at.is_some());
    }

    #[test]
    fn skip_step_advances_pipeline() {
        let (_dir, coord) = setup();
        let def = create_test_def(&coord);

        let run = coord
            .create_run(&def.id, "/tmp/test", WorkflowExecutionMode::TrackingOnly)
            .unwrap();

        let run = coord.skip_step(&run.id, "s1").unwrap();
        assert_eq!(run.steps[0].state, StepRunState::Skipped);
        assert_eq!(run.steps[1].state, StepRunState::WaitingLink);
    }

    #[test]
    fn cancel_run_cancels_pending_steps() {
        let (_dir, coord) = setup();
        let def = create_test_def(&coord);

        let run = coord
            .create_run(&def.id, "/tmp/test", WorkflowExecutionMode::TrackingOnly)
            .unwrap();

        let run = coord.cancel_run(&run.id).unwrap();
        assert_eq!(run.state, WorkflowRunState::Cancelled);
        assert!(run.steps.iter().all(|s| s.state == StepRunState::Cancelled));
    }

    #[test]
    fn heuristic_cannot_complete_step() {
        let (_dir, coord) = setup();
        let def = create_test_def(&coord);

        let run = coord
            .create_run(&def.id, "/tmp/test", WorkflowExecutionMode::TrackingOnly)
            .unwrap();

        let result = coord.complete_step(&run.id, "s1", CompletionSource::HeuristicSuggestion);
        assert!(result.is_err());
    }

    #[test]
    fn summary_list_reflects_state() {
        let (_dir, coord) = setup();
        let def = create_test_def(&coord);

        coord
            .create_run(&def.id, "/tmp/test", WorkflowExecutionMode::Assisted)
            .unwrap();

        let summaries = coord.get_summary_list();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].workflow_name, "Plan Design & Review");
        assert_eq!(summaries[0].progress_label, "0/3");
        assert_eq!(summaries[0].waiting_count, 1);
    }
}
