use octomonitor_core::workflow::{StepRun, WorkflowRun};
use octomonitor_core::RunRecord;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_FILE_CHARS: usize = 8000;
const MAX_PROMPT_CHARS: usize = 32000;

/// Render a prompt template by replacing `{{variable}}` placeholders.
pub fn render_prompt(
    template: &str,
    step: &StepRun,
    run: &WorkflowRun,
    working_dir: &str,
    monitor_runs: &[RunRecord],
) -> String {
    let mut result = template.to_string();

    // Simple variable replacements
    result = result.replace("{{workflow.name}}", &run.workflow_name);
    result = result.replace("{{step.label}}", &step.label);
    result = result.replace("{{working_dir}}", working_dir);

    // Previous step data
    let prev_step = previous_step(step, run);
    let prev_linked_run = prev_step.and_then(|ps| find_linked_run(ps, monitor_runs));

    result = result.replace(
        "{{previous.summary}}",
        &prev_linked_run
            .and_then(|r| r.last_question.as_deref())
            .unwrap_or("[no previous summary]"),
    );

    result = result.replace(
        "{{previous.artifacts}}",
        &prev_step
            .map(|ps| {
                ps.artifacts
                    .iter()
                    .filter_map(|a| a.path.as_deref())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
    );

    result = result.replace(
        "{{linked_run.last_question}}",
        &find_linked_run(step, monitor_runs)
            .and_then(|r| r.last_question.as_deref())
            .unwrap_or("[no linked run]"),
    );

    result = result.replace(
        "{{linked_run.summary}}",
        &find_linked_run(step, monitor_runs)
            .and_then(|r| r.first_question.as_deref())
            .unwrap_or("[no linked run]"),
    );

    // File inclusions: {{file:path/to/file.md}}
    result = replace_file_inclusions(&result, working_dir);

    // Truncate total prompt
    if result.len() > MAX_PROMPT_CHARS {
        result.truncate(MAX_PROMPT_CHARS);
        result.push_str("\n[prompt truncated]");
    }

    result
}

fn previous_step<'a>(current: &StepRun, run: &'a WorkflowRun) -> Option<&'a StepRun> {
    if current.order == 0 {
        return None;
    }
    run.steps.iter().find(|s| s.order == current.order - 1)
}

fn find_linked_run<'a>(step: &StepRun, monitor_runs: &'a [RunRecord]) -> Option<&'a RunRecord> {
    step.linked_runs
        .first()
        .and_then(|lr| monitor_runs.iter().find(|r| r.id == lr.run_id))
}

fn replace_file_inclusions(template: &str, working_dir: &str) -> String {
    let mut result = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find("{{file:") {
        result.push_str(&remaining[..start]);
        let after_tag = &remaining[start + 7..]; // skip "{{file:"
        if let Some(end) = after_tag.find("}}") {
            let file_path = &after_tag[..end];
            let content = read_included_file(working_dir, file_path);
            result.push_str(&content);
            remaining = &after_tag[end + 2..]; // skip "}}"
        } else {
            // Malformed tag, include as-is
            result.push_str("{{file:");
            remaining = after_tag;
        }
    }
    result.push_str(remaining);
    result
}

fn read_included_file(working_dir: &str, file_path: &str) -> String {
    let resolved = match resolve_included_file(working_dir, file_path) {
        Ok(path) => path,
        Err(FileIncludeError::NotFound) => return format!("[file not found: {file_path}]"),
        Err(FileIncludeError::AbsolutePath) => {
            return "[file access denied: absolute path not allowed]".into();
        }
        Err(FileIncludeError::ParentTraversal) => {
            return "[file access denied: parent traversal not allowed]".into();
        }
        Err(FileIncludeError::OutsideWorkingDir | FileIncludeError::WorkingDirUnavailable) => {
            return "[file access denied: outside working directory]".into();
        }
    };

    match fs::read_to_string(&resolved) {
        Ok(content) => {
            if content.len() > MAX_FILE_CHARS {
                format!(
                    "{}\n[file truncated at {} chars]",
                    &content[..MAX_FILE_CHARS],
                    MAX_FILE_CHARS
                )
            } else {
                content
            }
        }
        Err(_) => format!("[file not found: {file_path}]"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileIncludeError {
    NotFound,
    AbsolutePath,
    ParentTraversal,
    OutsideWorkingDir,
    WorkingDirUnavailable,
}

fn resolve_included_file(working_dir: &str, file_path: &str) -> Result<PathBuf, FileIncludeError> {
    let requested = Path::new(file_path);
    if requested.is_absolute() {
        return Err(FileIncludeError::AbsolutePath);
    }
    if requested
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(FileIncludeError::ParentTraversal);
    }

    let working_dir =
        fs::canonicalize(working_dir).map_err(|_| FileIncludeError::WorkingDirUnavailable)?;
    let resolved =
        fs::canonicalize(working_dir.join(requested)).map_err(|_| FileIncludeError::NotFound)?;
    if !resolved.starts_with(&working_dir) {
        return Err(FileIncludeError::OutsideWorkingDir);
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use octomonitor_core::workflow::*;
    use octomonitor_core::ToolKind;
    use tempfile::tempdir;

    fn mock_run() -> WorkflowRun {
        WorkflowRun {
            id: "wr-1".into(),
            workflow_id: "wf-1".into(),
            workflow_name: "Design Review".into(),
            state: WorkflowRunState::Running,
            execution_mode: WorkflowExecutionMode::Assisted,
            working_dir: "/tmp/test".into(),
            definition_snapshot: WorkflowDef {
                id: "wf-1".into(),
                name: "Design Review".into(),
                description: None,
                default_working_dir: None,
                steps: vec![],
                created_at: "".into(),
                updated_at: "".into(),
            },
            steps: vec![
                StepRun {
                    step_id: "s0".into(),
                    order: 0,
                    label: "Write Plan".into(),
                    tool: ToolKind::Claude,
                    kind: WorkflowStepKind::Observe,
                    state: StepRunState::Completed,
                    prompt_rendered: None,
                    started_at: None,
                    completed_at: None,
                    error: None,
                    linked_runs: vec![],
                    artifacts: vec![ArtifactRef {
                        id: "a1".into(),
                        kind: "file".into(),
                        path: Some("docs/plan.md".into()),
                        preview: None,
                        digest: None,
                        created_at: "".into(),
                    }],
                    completion_source: None,
                },
                StepRun {
                    step_id: "s1".into(),
                    order: 1,
                    label: "Implement".into(),
                    tool: ToolKind::Claude,
                    kind: WorkflowStepKind::Launch,
                    state: StepRunState::Ready,
                    prompt_rendered: None,
                    started_at: None,
                    completed_at: None,
                    error: None,
                    linked_runs: vec![],
                    artifacts: vec![],
                    completion_source: None,
                },
            ],
            created_at: "".into(),
            updated_at: "".into(),
            completed_at: None,
        }
    }

    #[test]
    fn renders_simple_variables() {
        let run = mock_run();
        let step = &run.steps[1];
        let result = render_prompt(
            "Workflow: {{workflow.name}}, Step: {{step.label}}",
            step,
            &run,
            "/tmp/test",
            &[],
        );
        assert_eq!(result, "Workflow: Design Review, Step: Implement");
    }

    #[test]
    fn renders_previous_artifacts() {
        let run = mock_run();
        let step = &run.steps[1];
        let result = render_prompt(
            "Previous artifacts: {{previous.artifacts}}",
            step,
            &run,
            "/tmp/test",
            &[],
        );
        assert_eq!(result, "Previous artifacts: docs/plan.md");
    }

    #[test]
    fn renders_missing_file_gracefully() {
        let run = mock_run();
        let step = &run.steps[1];
        let workdir = tempdir().unwrap();
        let result = render_prompt(
            "Content: {{file:nonexistent.md}}",
            step,
            &run,
            workdir.path().to_str().unwrap(),
            &[],
        );
        assert_eq!(result, "Content: [file not found: nonexistent.md]");
    }

    #[test]
    fn renders_existing_file_from_working_dir() {
        let run = mock_run();
        let step = &run.steps[1];
        let workdir = tempdir().unwrap();
        std::fs::write(workdir.path().join("notes.md"), "hello from workflow").unwrap();

        let result = render_prompt(
            "Content: {{file:notes.md}}",
            step,
            &run,
            workdir.path().to_str().unwrap(),
            &[],
        );

        assert_eq!(result, "Content: hello from workflow");
    }

    #[test]
    fn rejects_absolute_file_includes() {
        let run = mock_run();
        let step = &run.steps[1];
        let workdir = tempdir().unwrap();
        let result = render_prompt(
            "Content: {{file:/etc/passwd}}",
            step,
            &run,
            workdir.path().to_str().unwrap(),
            &[],
        );
        assert_eq!(
            result,
            "Content: [file access denied: absolute path not allowed]"
        );
    }

    #[test]
    fn rejects_parent_directory_escape() {
        let run = mock_run();
        let step = &run.steps[1];
        let workdir = tempdir().unwrap();
        std::fs::write(workdir.path().join("inside.md"), "safe").unwrap();
        let result = render_prompt(
            "Content: {{file:../outside.md}}",
            step,
            &run,
            workdir.path().to_str().unwrap(),
            &[],
        );
        assert_eq!(
            result,
            "Content: [file access denied: parent traversal not allowed]"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        let run = mock_run();
        let step = &run.steps[1];
        let workdir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        std::fs::write(outside.path().join("secret.md"), "secret").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.md"),
            workdir.path().join("linked.md"),
        )
        .unwrap();

        let result = render_prompt(
            "Content: {{file:linked.md}}",
            step,
            &run,
            workdir.path().to_str().unwrap(),
            &[],
        );

        assert_eq!(
            result,
            "Content: [file access denied: outside working directory]"
        );
    }

    #[test]
    fn truncates_long_prompt() {
        let run = mock_run();
        let step = &run.steps[1];
        let long_template = "x".repeat(MAX_PROMPT_CHARS + 1000);
        let result = render_prompt(&long_template, step, &run, "/tmp/test", &[]);
        assert!(result.len() <= MAX_PROMPT_CHARS + 20); // +20 for truncation message
        assert!(result.ends_with("[prompt truncated]"));
    }
}
