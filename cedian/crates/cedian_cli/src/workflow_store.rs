//! Workflow persistence for the CLI harness (headless stopgap).
//!
//! Same pattern as `session.rs`: per-workdir `workflow.json` beside the
//! baseline, so `workflow …` invocations share state. The app shell will own
//! a real store; until then JSON round-trip of `WorkflowState`.

use cedian_workflow::WorkflowState;
use std::path::{Path, PathBuf};

fn workflow_path(workdir: &Path) -> PathBuf {
    workdir.join(".cedian").join("workflow.json")
}

/// Load the workflow, or fail with usage hint when none is running.
pub fn load(workdir: &Path) -> Result<WorkflowState, String> {
    let raw = std::fs::read_to_string(workflow_path(workdir))
        .map_err(|_| "no workflow: run `cedian workflow run <kind> <title>` first".to_string())?;
    // Annotate: bare `from_str(..)?` as tail would infer `Result<_, _>` as T.
    let state: WorkflowState =
        serde_json::from_str(&raw).map_err(|e| format!("corrupt workflow.json: {e}"))?;
    Ok(state)
}

/// Save the workflow (creates `.cedian/` like the baseline store).
pub fn save(workdir: &Path, state: &WorkflowState) -> Result<(), String> {
    std::fs::create_dir_all(workdir.join(".cedian")).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(workflow_path(workdir), raw).map_err(|e| e.to_string())
}

/// Delete the workflow (fresh `run` overwrites anyway; explicit for tests).
#[allow(unused)]
pub fn clear(workdir: &Path) {
    let _ = std::fs::remove_file(workflow_path(workdir));
}
