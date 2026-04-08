use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use octomonitor_core::workflow::*;
use serde::Deserialize;

use crate::state::AppState;

// --- Definition CRUD ---

pub async fn list_defs(
    State(state): State<AppState>,
) -> Result<Json<Vec<WorkflowDef>>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    coord
        .store()
        .list_defs()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn create_def(
    State(state): State<AppState>,
    Json(mut def): Json<WorkflowDef>,
) -> Result<Json<WorkflowDef>, StatusCode> {
    if def.id.is_empty() {
        def.id = format!("wf-{}", generate_id());
    }
    let now = chrono::Utc::now().to_rfc3339();
    if def.created_at.is_empty() {
        def.created_at = now.clone();
    }
    def.updated_at = now;

    // Assign step IDs if missing
    for (i, step) in def.steps.iter_mut().enumerate() {
        if step.id.is_empty() {
            step.id = format!("step-{}", i);
        }
        step.order = i as u32;
    }

    let coord = state.workflow_coordinator.lock().await;
    coord
        .store()
        .save_def(&def)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(def))
}

pub async fn get_def(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowDef>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    coord
        .store()
        .load_def(&id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn update_def(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Json(mut def): Json<WorkflowDef>,
) -> Result<Json<WorkflowDef>, StatusCode> {
    def.id = id;
    def.updated_at = chrono::Utc::now().to_rfc3339();
    for (i, step) in def.steps.iter_mut().enumerate() {
        step.order = i as u32;
    }
    let coord = state.workflow_coordinator.lock().await;
    coord
        .store()
        .save_def(&def)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(def))
}

pub async fn delete_def(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    coord
        .store()
        .delete_def(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Run CRUD ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRunBody {
    pub working_dir: String,
    #[serde(default = "default_tracking_mode")]
    pub execution_mode: WorkflowExecutionMode,
}

fn default_tracking_mode() -> WorkflowExecutionMode {
    WorkflowExecutionMode::TrackingOnly
}

pub async fn create_run(
    AxumPath(workflow_id): AxumPath<String>,
    State(state): State<AppState>,
    Json(body): Json<CreateRunBody>,
) -> Result<(StatusCode, Json<WorkflowRun>), StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .create_run(&workflow_id, &body.working_dir, body.execution_mode)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();
    Ok((StatusCode::CREATED, Json(run)))
}

pub async fn list_runs(
    State(state): State<AppState>,
) -> Result<Json<Vec<WorkflowRunSummary>>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    Ok(Json(coord.get_summary_list()))
}

pub async fn get_run(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    coord
        .store()
        .load_run(&id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn cancel_run(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .cancel_run(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();
    Ok(Json(run))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeModeBody {
    pub mode: WorkflowExecutionMode,
}

pub async fn change_mode(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Json(body): Json<ChangeModeBody>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .change_mode(&id, body.mode)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();
    Ok(Json(run))
}

// --- Step Actions ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPath {
    pub id: String,
    pub step_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkBody {
    pub run_id: String,
    #[serde(default = "default_confidence")]
    pub confidence: LinkConfidence,
    #[serde(default = "default_matched_by")]
    pub matched_by: String,
}

fn default_confidence() -> LinkConfidence {
    LinkConfidence::Explicit
}

fn default_matched_by() -> String {
    "user-manual".into()
}

pub async fn link_run(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
    Json(body): Json<LinkBody>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .link_run(
            &path.id,
            &path.step_id,
            &body.run_id,
            body.confidence,
            &body.matched_by,
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();
    Ok(Json(run))
}

pub async fn unlink_run(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
    Json(body): Json<UnlinkBody>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .unlink_run(&path.id, &path.step_id, &body.run_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();
    Ok(Json(run))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkBody {
    pub run_id: String,
}

pub async fn complete_step(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .complete_step(&path.id, &path.step_id, CompletionSource::UserMarked)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();

    // Auto-launch: if the next step is Ready + Launch + Auto mode, approve & launch
    maybe_auto_launch(&run, &coord, &state);

    Ok(Json(run))
}

pub async fn fail_step(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .fail_step(&path.id, &path.step_id, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();
    Ok(Json(run))
}

pub async fn skip_step(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .skip_step(&path.id, &path.step_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();

    // Auto-launch: skipping may advance to a launchable step
    maybe_auto_launch(&run, &coord, &state);

    Ok(Json(run))
}

pub async fn retry_step(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .retry_step(&path.id, &path.step_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.signal_change();

    // Auto-launch: if retry puts the step into Ready + Launch + Auto mode
    maybe_auto_launch(&run, &coord, &state);

    Ok(Json(run))
}

pub async fn get_launch_preview(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
) -> Result<Json<LaunchPreview>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let payload = state.bootstrap.read().await;
    coord
        .get_launch_preview(&path.id, &path.step_id, &payload.runs)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn approve_step(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
) -> Result<Json<WorkflowRun>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .approve_step(&path.id, &path.step_id)
        .map_err(|e| {
            if e.to_string().contains("TrackingOnly") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;
    state.signal_change();

    // If approved, spawn the launcher in background
    let run_id = run.id.clone();
    let step_id = path.step_id.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        execute_launch_step(state_clone, &run_id, &step_id).await;
    });

    Ok(Json(run))
}

/// If any launch step is in Ready state and mode is Auto, approve and spawn the launcher.
fn maybe_auto_launch(
    run: &WorkflowRun,
    coord: &crate::workflows::coordinator::WorkflowCoordinator,
    state: &AppState,
) {
    if run.execution_mode != WorkflowExecutionMode::Auto {
        return;
    }
    for step in &run.steps {
        if step.kind == WorkflowStepKind::Launch && step.state == StepRunState::Ready {
            let run_id = run.id.clone();
            let step_id = step.step_id.clone();
            // Approve transitions Ready → Running
            if coord.approve_step(&run_id, &step_id).is_ok() {
                state.signal_change();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    execute_launch_step(state_clone, &run_id, &step_id).await;
                });
            }
            break;
        }
    }
}

async fn execute_launch_step(state: AppState, run_id: &str, step_id: &str) {
    use crate::workflows::launcher::{LaunchRequest, LauncherDispatcher};

    let (request, _) = {
        let coord = state.workflow_coordinator.lock().await;
        let payload = state.bootstrap.read().await;
        let preview = match coord.get_launch_preview(run_id, step_id, &payload.runs) {
            Ok(p) => p,
            Err(_) => return,
        };
        let run = match coord.store().load_run(run_id) {
            Ok(r) => r,
            Err(_) => return,
        };
        let step = match run.steps.iter().find(|s| s.step_id == step_id) {
            Some(s) => s,
            None => return,
        };
        let def_step = run.definition_snapshot.steps.iter().find(|d| d.id == step.step_id);
        let launch_spec = def_step.and_then(|d| d.launch.as_ref());
        let req = LaunchRequest {
            tool: step.tool.clone(),
            prompt: preview.rendered_prompt,
            working_dir: run.working_dir.clone(),
            model: preview.model,
            timeout_secs: launch_spec.and_then(|l| l.timeout_secs),
            allowed_tools: preview.allowed_tools,
            args: launch_spec.map(|l| l.args.clone()).unwrap_or_default(),
        };
        (req, ())
    };

    let mut dispatcher = LauncherDispatcher::new();
    dispatcher.detect_capabilities().await;

    match dispatcher.launch(request).await {
        Ok(result) => {
            let coord = state.workflow_coordinator.lock().await;
            let source = if result.exit_code == 0 {
                CompletionSource::LauncherExit
            } else {
                // Treat non-zero exit as failure
                let _ = coord.fail_step(
                    run_id,
                    step_id,
                    Some(format!("CLI exited with code {}", result.exit_code)),
                );
                state.signal_change();
                return;
            };
            if let Ok(run) = coord.complete_step(run_id, step_id, source) {
                state.signal_change();
                // Chain auto-launch for the next step in Auto mode
                maybe_auto_launch(&run, &coord, &state);
            }
        }
        Err(e) => {
            let coord = state.workflow_coordinator.lock().await;
            let _ = coord.fail_step(run_id, step_id, Some(e.to_string()));
            state.signal_change();
        }
    }
}

pub async fn get_candidates(
    AxumPath(path): AxumPath<StepPath>,
    State(state): State<AppState>,
) -> Result<Json<Vec<LinkedRunRef>>, StatusCode> {
    let coord = state.workflow_coordinator.lock().await;
    let run = coord
        .store()
        .load_run(&path.id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let step = run
        .steps
        .iter()
        .find(|s| s.step_id == path.step_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let payload = state.bootstrap.read().await;
    let candidates =
        crate::workflows::link_resolver::resolve_candidates(step, &run, &payload.runs);
    Ok(Json(candidates))
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
