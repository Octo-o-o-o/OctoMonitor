use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ToolKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkflowStepKind {
    Observe,
    Launch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkflowExecutionMode {
    TrackingOnly,
    Assisted,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkflowRunState {
    Pending,
    Running,
    WaitingInput,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StepRunState {
    Pending,
    Ready,
    WaitingLink,
    WaitingApproval,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CompletionSource {
    LauncherExit,
    HookEvent,
    LinkedRun,
    UserMarked,
    HeuristicSuggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CompletionMode {
    ManualLink,
    LauncherExit,
    HookEvent,
    ManualComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LinkConfidence {
    Explicit,
    ContextFile,
    PromptMarker,
    WorkspaceAndArtifact,
    HeuristicCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkflowDef {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_working_dir: Option<String>,
    pub steps: Vec<StepDef>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StepDef {
    pub id: String,
    pub order: u32,
    pub label: String,
    pub tool: ToolKind,
    pub kind: WorkflowStepKind,
    pub prompt_template: Option<String>,
    pub inputs: Vec<ArtifactSpec>,
    pub outputs: Vec<ArtifactExpectation>,
    pub approval_required: bool,
    pub auto_advance_eligible: bool,
    pub completion: CompletionPolicy,
    pub launch: Option<LaunchSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtifactSpec {
    pub mode: String,
    pub value: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtifactExpectation {
    pub kind: String,
    pub value: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompletionPolicy {
    pub mode: CompletionMode,
    pub required_artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaunchSpec {
    pub model: Option<String>,
    #[ts(type = "number | undefined")]
    pub timeout_secs: Option<u64>,
    pub allowed_tools: Vec<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub execution_mode: WorkflowExecutionMode,
    pub working_dir: String,
    pub state: WorkflowRunState,
    pub definition_snapshot: WorkflowDef,
    pub steps: Vec<StepRun>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StepRun {
    pub step_id: String,
    pub order: u32,
    pub label: String,
    pub tool: ToolKind,
    pub kind: WorkflowStepKind,
    pub state: StepRunState,
    pub prompt_rendered: Option<String>,
    pub linked_runs: Vec<LinkedRunRef>,
    pub artifacts: Vec<ArtifactRef>,
    pub completion_source: Option<CompletionSource>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkedRunRef {
    pub run_id: String,
    pub confidence: LinkConfidence,
    pub matched_by: String,
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtifactRef {
    pub id: String,
    pub kind: String,
    pub path: Option<String>,
    pub preview: Option<String>,
    pub digest: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkflowRunSummary {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub state: WorkflowRunState,
    pub execution_mode: WorkflowExecutionMode,
    pub progress_label: String,
    pub current_step: Option<String>,
    #[ts(type = "number")]
    pub waiting_count: u32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaunchPreview {
    pub rendered_prompt: String,
    pub input_artifacts: Vec<ArtifactSpec>,
    pub model: Option<String>,
    pub allowed_tools: Vec<String>,
    #[ts(type = "number")]
    pub estimated_prompt_chars: usize,
}

pub fn summarize_run(run: &WorkflowRun) -> WorkflowRunSummary {
    let completed = run
        .steps
        .iter()
        .filter(|s| s.state == StepRunState::Completed || s.state == StepRunState::Skipped)
        .count();
    let total = run.steps.len();
    let current_step = run
        .steps
        .iter()
        .find(|s| {
            !matches!(
                s.state,
                StepRunState::Completed | StepRunState::Skipped | StepRunState::Pending
            )
        })
        .map(|s| s.label.clone());
    let waiting_count = run
        .steps
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                StepRunState::WaitingLink | StepRunState::WaitingApproval
            )
        })
        .count() as u32;

    WorkflowRunSummary {
        id: run.id.clone(),
        workflow_id: run.definition_snapshot.id.clone(),
        workflow_name: run.workflow_name.clone(),
        state: run.state.clone(),
        execution_mode: run.execution_mode.clone(),
        progress_label: format!("{}/{}", completed, total),
        current_step,
        waiting_count,
        updated_at: run.updated_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_types_serialize_camel_case() {
        let mode = WorkflowExecutionMode::TrackingOnly;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, r#""trackingOnly""#);

        let state = StepRunState::WaitingLink;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, r#""waitingLink""#);
    }

    #[test]
    fn summarize_run_computes_progress() {
        let run = WorkflowRun {
            id: "wr-test".into(),
            workflow_id: "wf-test".into(),
            workflow_name: "Test".into(),
            execution_mode: WorkflowExecutionMode::TrackingOnly,
            working_dir: "/tmp".into(),
            state: WorkflowRunState::Running,
            definition_snapshot: WorkflowDef {
                id: "wf-test".into(),
                name: "Test".into(),
                description: None,
                default_working_dir: None,
                steps: vec![],
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
            steps: vec![
                StepRun {
                    step_id: "s1".into(),
                    order: 0,
                    label: "Step 1".into(),
                    tool: ToolKind::Claude,
                    kind: WorkflowStepKind::Observe,
                    state: StepRunState::Completed,
                    prompt_rendered: None,
                    linked_runs: vec![],
                    artifacts: vec![],
                    completion_source: None,
                    started_at: None,
                    completed_at: None,
                    error: None,
                },
                StepRun {
                    step_id: "s2".into(),
                    order: 1,
                    label: "Step 2".into(),
                    tool: ToolKind::Codex,
                    kind: WorkflowStepKind::Launch,
                    state: StepRunState::WaitingApproval,
                    prompt_rendered: None,
                    linked_runs: vec![],
                    artifacts: vec![],
                    completion_source: None,
                    started_at: None,
                    completed_at: None,
                    error: None,
                },
                StepRun {
                    step_id: "s3".into(),
                    order: 2,
                    label: "Step 3".into(),
                    tool: ToolKind::Claude,
                    kind: WorkflowStepKind::Observe,
                    state: StepRunState::Pending,
                    prompt_rendered: None,
                    linked_runs: vec![],
                    artifacts: vec![],
                    completion_source: None,
                    started_at: None,
                    completed_at: None,
                    error: None,
                },
            ],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
        };

        let summary = summarize_run(&run);
        assert_eq!(summary.progress_label, "1/3");
        assert_eq!(summary.current_step, Some("Step 2".into()));
        assert_eq!(summary.waiting_count, 1);
    }
}
