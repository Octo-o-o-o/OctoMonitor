use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use octomonitor_core::workflow::*;
use serde_json::Value;
use std::path::PathBuf;

const DEFAULT_BASE: &str = "http://127.0.0.1:46321";

#[derive(Parser)]
#[command(name = "octomonitor", about = "OctoMonitor CLI", version)]
struct Cli {
    /// Server base URL (default: http://127.0.0.1:46321)
    #[arg(long, env = "OCTOMONITOR_URL", default_value = DEFAULT_BASE)]
    server: String,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Workflow management
    #[command(alias = "wf")]
    Workflow {
        #[command(subcommand)]
        action: WorkflowAction,
    },
}

#[derive(Subcommand)]
enum WorkflowAction {
    /// Create a workflow definition from a JSON file
    Create {
        /// Path to a JSON file, or "-" to read from stdin
        #[arg(short, long)]
        file: PathBuf,

        /// Immediately start a run after creation
        #[arg(long)]
        run: bool,

        /// Working directory for the run (used with --run)
        #[arg(long)]
        dir: Option<String>,

        /// Execution mode: tracking, assisted, auto (used with --run)
        #[arg(long, default_value = "tracking")]
        mode: String,
    },

    /// List workflow definitions
    List,

    /// Show a workflow definition
    Show {
        /// Workflow definition ID
        id: String,
    },

    /// Delete a workflow definition
    Delete {
        /// Workflow definition ID
        id: String,
    },

    /// Start a new workflow run
    Run {
        /// Workflow definition ID
        id: String,

        /// Working directory for the run
        #[arg(short, long)]
        dir: String,

        /// Execution mode: tracking, assisted, auto
        #[arg(short, long, default_value = "tracking")]
        mode: String,
    },

    /// List workflow runs
    Runs {
        /// Filter by state (running, completed, failed, etc.)
        #[arg(long)]
        state: Option<String>,
    },

    /// Show a workflow run in detail
    Inspect {
        /// Workflow run ID
        id: String,
    },

    /// Operate on a step within a run
    Step {
        /// Workflow run ID
        run_id: String,

        /// Step ID (e.g. step-0)
        step_id: String,

        /// Action: approve, complete, skip, retry, fail
        action: String,
    },

    /// Link a monitor run to a step
    Link {
        /// Workflow run ID
        run_id: String,

        /// Step ID
        step_id: String,

        /// Monitor run ID to link
        monitor_run_id: String,
    },

    /// Get launch preview for a step
    Preview {
        /// Workflow run ID
        run_id: String,

        /// Step ID
        step_id: String,
    },

    /// Print a workflow JSON template to stdout
    Template,
}

fn parse_mode(s: &str) -> Result<&'static str> {
    match s {
        "tracking" | "trackingOnly" => Ok("trackingOnly"),
        "assisted" => Ok("assisted"),
        "auto" => Ok("auto"),
        other => bail!("Unknown mode '{}'. Use: tracking, assisted, auto", other),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let base = cli.server.trim_end_matches('/');
    let client = reqwest::Client::new();

    match cli.command {
        Cmd::Workflow { action } => match action {
            WorkflowAction::Template => {
                print_template();
                Ok(())
            }

            WorkflowAction::Create {
                file,
                run,
                dir,
                mode,
            } => {
                let json_str = read_input(&file)?;
                let def: Value = serde_json::from_str(&json_str)
                    .context("Invalid JSON in workflow definition file")?;

                let res = client
                    .post(format!("{base}/api/workflows"))
                    .json(&def)
                    .send()
                    .await
                    .context("Failed to connect to server")?;

                if !res.status().is_success() {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    bail!("Create failed ({}): {}", status, body);
                }

                let created: WorkflowDef = res.json().await?;
                println!("Created workflow: {} ({})", created.name, created.id);

                if run {
                    let working_dir = dir
                        .or(created.default_working_dir.clone())
                        .unwrap_or_else(|| ".".to_string());
                    let exec_mode = parse_mode(&mode)?;
                    start_run(&client, base, &created.id, &working_dir, exec_mode).await?;
                }
                Ok(())
            }

            WorkflowAction::List => {
                let res = client
                    .get(format!("{base}/api/workflows"))
                    .send()
                    .await
                    .context("Failed to connect to server")?;
                let defs: Vec<WorkflowDef> = res.json().await?;
                if defs.is_empty() {
                    println!("No workflow definitions found.");
                } else {
                    println!("{:<24} {:<30} {}", "ID", "NAME", "STEPS");
                    for d in &defs {
                        println!("{:<24} {:<30} {}", d.id, d.name, d.steps.len());
                    }
                }
                Ok(())
            }

            WorkflowAction::Show { id } => {
                let res = client
                    .get(format!("{base}/api/workflows/{id}"))
                    .send()
                    .await?;
                if !res.status().is_success() {
                    bail!("Workflow '{}' not found", id);
                }
                let def: Value = res.json().await?;
                println!("{}", serde_json::to_string_pretty(&def)?);
                Ok(())
            }

            WorkflowAction::Delete { id } => {
                let res = client
                    .delete(format!("{base}/api/workflows/{id}"))
                    .send()
                    .await?;
                if res.status().is_success() {
                    println!("Deleted workflow: {}", id);
                } else {
                    bail!("Delete failed ({})", res.status());
                }
                Ok(())
            }

            WorkflowAction::Run { id, dir, mode } => {
                let exec_mode = parse_mode(&mode)?;
                start_run(&client, base, &id, &dir, exec_mode).await
            }

            WorkflowAction::Runs { state } => {
                let res = client
                    .get(format!("{base}/api/workflow-runs"))
                    .send()
                    .await?;
                let runs: Vec<WorkflowRunSummary> = res.json().await?;
                let filtered: Vec<_> = if let Some(ref st) = state {
                    runs.into_iter()
                        .filter(|r| {
                            format!("{:?}", r.state)
                                .to_lowercase()
                                .contains(&st.to_lowercase())
                        })
                        .collect()
                } else {
                    runs
                };
                if filtered.is_empty() {
                    println!("No workflow runs found.");
                } else {
                    println!(
                        "{:<24} {:<24} {:<14} {:<10} {}",
                        "RUN ID", "WORKFLOW", "STATE", "MODE", "PROGRESS"
                    );
                    for r in &filtered {
                        println!(
                            "{:<24} {:<24} {:<14} {:<10} {}",
                            r.id,
                            truncate(&r.workflow_name, 22),
                            format!("{:?}", r.state).to_lowercase(),
                            format!("{:?}", r.execution_mode).to_lowercase(),
                            r.progress_label,
                        );
                    }
                }
                Ok(())
            }

            WorkflowAction::Inspect { id } => {
                let res = client
                    .get(format!("{base}/api/workflow-runs/{id}"))
                    .send()
                    .await?;
                if !res.status().is_success() {
                    bail!("Run '{}' not found", id);
                }
                let run: WorkflowRun = res.json().await?;
                println!("Run:      {} ({})", run.workflow_name, run.id);
                println!("State:    {:?}", run.state);
                println!("Mode:     {:?}", run.execution_mode);
                println!("Dir:      {}", run.working_dir);
                println!();
                println!(
                    "{:<8} {:<4} {:<20} {:<8} {:<8} {:<14} {}",
                    "STEP", "ORD", "LABEL", "TOOL", "KIND", "STATE", "LINKED"
                );
                for s in &run.steps {
                    println!(
                        "{:<8} {:<4} {:<20} {:<8} {:<8} {:<14} {}",
                        s.step_id,
                        s.order,
                        truncate(&s.label, 18),
                        format!("{:?}", s.tool).to_lowercase(),
                        format!("{:?}", s.kind).to_lowercase(),
                        format!("{:?}", s.state).to_lowercase(),
                        s.linked_runs.len(),
                    );
                }
                Ok(())
            }

            WorkflowAction::Step {
                run_id,
                step_id,
                action,
            } => {
                let valid = ["approve", "complete", "skip", "retry", "fail"];
                if !valid.contains(&action.as_str()) {
                    bail!(
                        "Unknown step action '{}'. Use: {}",
                        action,
                        valid.join(", ")
                    );
                }
                let res = client
                    .post(format!(
                        "{base}/api/workflow-runs/{run_id}/steps/{step_id}/{action}"
                    ))
                    .send()
                    .await?;
                if res.status().is_success() {
                    println!("Step {}: {} OK", step_id, action);
                } else {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    bail!("Step action failed ({}): {}", status, body);
                }
                Ok(())
            }

            WorkflowAction::Link {
                run_id,
                step_id,
                monitor_run_id,
            } => {
                let body = serde_json::json!({
                    "runId": monitor_run_id,
                    "confidence": "explicit",
                    "matchedBy": "cli-manual",
                });
                let res = client
                    .post(format!(
                        "{base}/api/workflow-runs/{run_id}/steps/{step_id}/link"
                    ))
                    .json(&body)
                    .send()
                    .await?;
                if res.status().is_success() {
                    println!("Linked {} to step {}", monitor_run_id, step_id);
                } else {
                    bail!("Link failed ({})", res.status());
                }
                Ok(())
            }

            WorkflowAction::Preview { run_id, step_id } => {
                let res = client
                    .get(format!(
                        "{base}/api/workflow-runs/{run_id}/steps/{step_id}/preview"
                    ))
                    .send()
                    .await?;
                if !res.status().is_success() {
                    bail!("Preview not available ({})", res.status());
                }
                let preview: Value = res.json().await?;
                println!("{}", serde_json::to_string_pretty(&preview)?);
                Ok(())
            }
        },
    }
}

async fn start_run(
    client: &reqwest::Client,
    base: &str,
    workflow_id: &str,
    working_dir: &str,
    exec_mode: &str,
) -> Result<()> {
    let body = serde_json::json!({
        "workingDir": working_dir,
        "executionMode": exec_mode,
    });
    let res = client
        .post(format!("{base}/api/workflows/{workflow_id}/runs"))
        .json(&body)
        .send()
        .await
        .context("Failed to connect to server")?;

    if !res.status().is_success() {
        let status = res.status();
        let body_text = res.text().await.unwrap_or_default();
        bail!("Create run failed ({}): {}", status, body_text);
    }

    let run: WorkflowRun = res.json().await?;
    println!("Started run: {} (mode: {:?})", run.id, run.execution_mode);
    println!("Steps:");
    for s in &run.steps {
        println!(
            "  {} [{}] {:?} {:?} → {:?}",
            s.step_id, s.label, s.tool, s.kind, s.state,
        );
    }
    Ok(())
}

fn read_input(path: &PathBuf) -> Result<String> {
    if path.as_os_str() == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

fn print_template() {
    let template = serde_json::json!({
        "name": "My Workflow",
        "description": "Optional description",
        "defaultWorkingDir": "/path/to/project",
        "steps": [
            {
                "label": "Step 1: Design",
                "tool": "claude",
                "kind": "observe",
                "completion": { "mode": "manualLink" },
                "_comment": "Observe step: you work in Claude Code, then link the run"
            },
            {
                "label": "Step 2: Review",
                "tool": "codex",
                "kind": "launch",
                "promptTemplate": "Review the design from {{previous.summary}}.\n\nProject: {{working_dir}}\nContext: {{file:docs/design.md}}",
                "approvalRequired": true,
                "completion": { "mode": "launcherExit" },
                "launch": {
                    "model": null,
                    "allowedTools": [],
                    "args": []
                },
                "_comment": "Launch step: OctoMonitor starts CLI with rendered prompt"
            },
            {
                "label": "Step 3: Implement",
                "tool": "claude",
                "kind": "observe",
                "completion": { "mode": "manualLink" }
            },
            {
                "label": "Step 4: Final Review",
                "tool": "codex",
                "kind": "launch",
                "promptTemplate": "Final review for {{workflow.name}}.\n\nPrevious output: {{previous.summary}}",
                "approvalRequired": false,
                "completion": { "mode": "launcherExit" },
                "launch": {}
            }
        ]
    });
    println!("{}", serde_json::to_string_pretty(&template).unwrap());
    eprintln!();
    eprintln!("# Template variables available in promptTemplate:");
    eprintln!("#   {{{{workflow.name}}}}          - Workflow name");
    eprintln!("#   {{{{step.label}}}}             - Current step label");
    eprintln!("#   {{{{working_dir}}}}            - Working directory path");
    eprintln!("#   {{{{previous.summary}}}}       - Previous step's linked run last question");
    eprintln!("#   {{{{previous.artifacts}}}}     - Previous step's artifact file paths");
    eprintln!("#   {{{{linked_run.last_question}}}} - Current step's linked run last question");
    eprintln!("#   {{{{linked_run.summary}}}}     - Current step's linked run first question");
    eprintln!("#   {{{{file:relative/path}}}}     - Inject file content (max 8000 chars)");
    eprintln!("#");
    eprintln!("# Tools: claude, codex, openClaw");
    eprintln!("# Kinds: observe, launch");
    eprintln!("# Completion modes: manualLink, manualComplete, launcherExit, hookEvent");
    eprintln!("# Execution modes: tracking (default), assisted, auto");
    eprintln!("#");
    eprintln!("# Usage:");
    eprintln!("#   octomonitor workflow create -f workflow.json");
    eprintln!(
        "#   octomonitor workflow create -f workflow.json --run --dir /project --mode assisted"
    );
    eprintln!("#   octomonitor workflow template > my-workflow.json  # then edit and create");
}
