use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use octomonitor_core::workflow::{
    summarize_run, WorkflowDef, WorkflowRun, WorkflowRunState, WorkflowRunSummary,
};

use crate::platform::home_dir;

pub struct WorkflowStore {
    base_dir: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct DefIndex {
    defs: Vec<DefIndexEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DefIndexEntry {
    id: String,
    name: String,
    step_count: usize,
    updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct RunIndex {
    runs: Vec<WorkflowRunSummary>,
}

impl WorkflowStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let base_dir = base_dir.into();
        fs::create_dir_all(base_dir.join("defs"))?;
        fs::create_dir_all(base_dir.join("runs"))?;
        Ok(Self { base_dir })
    }

    pub fn default_base_dir() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".octomonitor")
            .join("workflows")
    }

    fn defs_dir(&self) -> PathBuf {
        self.base_dir.join("defs")
    }

    fn runs_dir(&self) -> PathBuf {
        self.base_dir.join("runs")
    }

    fn def_index_path(&self) -> PathBuf {
        self.base_dir.join("index-defs.json")
    }

    fn run_index_path(&self) -> PathBuf {
        self.base_dir.join("index-runs.json")
    }

    fn def_path(&self, id: &str) -> PathBuf {
        self.defs_dir().join(format!("{}.json", id))
    }

    fn run_path(&self, id: &str) -> PathBuf {
        self.runs_dir().join(format!("{}.json", id))
    }

    // --- Definition CRUD ---

    pub fn list_defs(&self) -> Result<Vec<WorkflowDef>> {
        let index = self.load_def_index();
        Ok(index
            .defs
            .iter()
            .filter_map(|entry| self.load_def(&entry.id).ok())
            .collect())
    }

    pub fn load_def(&self, id: &str) -> Result<WorkflowDef> {
        let path = self.def_path(id);
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read workflow def {}", id))?;
        serde_json::from_str(&data).with_context(|| format!("Failed to parse workflow def {}", id))
    }

    pub fn save_def(&self, def: &WorkflowDef) -> Result<()> {
        atomic_write(&self.def_path(&def.id), def)?;
        self.update_def_index(def);
        Ok(())
    }

    pub fn delete_def(&self, id: &str) -> Result<()> {
        let path = self.def_path(id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        let mut index = self.load_def_index();
        index.defs.retain(|e| e.id != id);
        atomic_write(&self.def_index_path(), &index)?;
        Ok(())
    }

    // --- Run CRUD ---

    pub fn list_runs(
        &self,
        limit: Option<usize>,
        state_filter: Option<&WorkflowRunState>,
    ) -> Vec<WorkflowRunSummary> {
        let index = self.load_run_index();
        let mut runs = index.runs;
        if let Some(state) = state_filter {
            runs.retain(|r| &r.state == state);
        }
        if let Some(limit) = limit {
            runs.truncate(limit);
        }
        runs
    }

    pub fn load_run(&self, id: &str) -> Result<WorkflowRun> {
        let path = self.run_path(id);
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read workflow run {}", id))?;
        serde_json::from_str(&data).with_context(|| format!("Failed to parse workflow run {}", id))
    }

    pub fn save_run(&self, run: &WorkflowRun) -> Result<()> {
        atomic_write(&self.run_path(&run.id), run)?;
        self.update_run_index(run);
        Ok(())
    }

    // --- Index helpers ---

    fn load_def_index(&self) -> DefIndex {
        let path = self.def_index_path();
        if !path.exists() {
            return DefIndex::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    fn update_def_index(&self, def: &WorkflowDef) {
        let mut index = self.load_def_index();
        index.defs.retain(|e| e.id != def.id);
        index.defs.push(DefIndexEntry {
            id: def.id.clone(),
            name: def.name.clone(),
            step_count: def.steps.len(),
            updated_at: def.updated_at.clone(),
        });
        let _ = atomic_write(&self.def_index_path(), &index);
    }

    fn load_run_index(&self) -> RunIndex {
        let path = self.run_index_path();
        if !path.exists() {
            return RunIndex::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    fn update_run_index(&self, run: &WorkflowRun) {
        let mut index = self.load_run_index();
        index.runs.retain(|r| r.id != run.id);
        index.runs.insert(0, summarize_run(run));
        let _ = atomic_write(&self.run_index_path(), &index);
    }

    pub fn get_all_summaries(&self) -> Vec<WorkflowRunSummary> {
        self.load_run_index().runs
    }
}

fn atomic_write(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let data = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use octomonitor_core::workflow::*;
    use octomonitor_core::ToolKind;

    fn sample_def() -> WorkflowDef {
        WorkflowDef {
            id: "wf-test-1".into(),
            name: "Test Workflow".into(),
            description: Some("A test workflow".into()),
            default_working_dir: None,
            steps: vec![StepDef {
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
            }],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn sample_run(def: &WorkflowDef) -> WorkflowRun {
        WorkflowRun {
            id: "wr-test-1".into(),
            workflow_id: def.id.clone(),
            workflow_name: def.name.clone(),
            execution_mode: WorkflowExecutionMode::TrackingOnly,
            working_dir: "/tmp/test".into(),
            state: WorkflowRunState::Running,
            definition_snapshot: def.clone(),
            steps: vec![StepRun {
                step_id: "s1".into(),
                order: 0,
                label: "Write Plan".into(),
                tool: ToolKind::Claude,
                kind: WorkflowStepKind::Observe,
                state: StepRunState::WaitingLink,
                prompt_rendered: None,
                linked_runs: vec![],
                artifacts: vec![],
                completion_source: None,
                started_at: None,
                completed_at: None,
                error: None,
            }],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
        }
    }

    #[test]
    fn store_def_crud_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorkflowStore::new(dir.path().join("workflows")).unwrap();
        let def = sample_def();

        store.save_def(&def).unwrap();
        let loaded = store.load_def(&def.id).unwrap();
        assert_eq!(loaded.name, def.name);

        let defs = store.list_defs().unwrap();
        assert_eq!(defs.len(), 1);

        store.delete_def(&def.id).unwrap();
        let defs = store.list_defs().unwrap();
        assert_eq!(defs.len(), 0);
    }

    #[test]
    fn store_run_crud_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorkflowStore::new(dir.path().join("workflows")).unwrap();
        let def = sample_def();
        let run = sample_run(&def);

        store.save_run(&run).unwrap();
        let loaded = store.load_run(&run.id).unwrap();
        assert_eq!(loaded.workflow_name, run.workflow_name);

        let summaries = store.list_runs(None, None);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].progress_label, "0/1");
    }
}
