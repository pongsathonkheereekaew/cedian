//! Review input persistence for the CLI harness (headless stopgap).
//!
//! The app shell (§2.5) will own a real mapping store with `snapshot_version`.
//! Until then: per-workdir baseline (path → version + text) saved beside the
//! OMP session dir, so `review`/`accept`/`reject` in later invocations compare
//! against the pre-prompt baseline, not against whatever the last command left.

use cedian_review::Baseline;
use cedian_workspace::{HostTools, Version, WorkspaceHost};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Review store dir: `<workdir>/.cedian/` (gitignored by convention).
fn store_dir(workdir: &Path) -> PathBuf {
    workdir.join(".cedian")
}

fn baseline_path(workdir: &Path) -> PathBuf {
    store_dir(workdir).join("baseline.json")
}

/// Save the pre-prompt baseline (versions are in-process only — persist the
/// TEXTS; versions re-anchor at load since text-vs-text is what review diffs).
pub fn save_review_inputs(workdir: &Path, texts: &HashMap<PathBuf, String>) -> Result<(), String> {
    std::fs::create_dir_all(store_dir(workdir)).map_err(|e| e.to_string())?;
    let flat: HashMap<String, String> = texts
        .iter()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.clone()))
        .collect();
    let raw = serde_json::to_string_pretty(&flat).map_err(|e| e.to_string())?;
    std::fs::write(baseline_path(workdir), raw).map_err(|e| e.to_string())
}

/// Load-or-rebuild review inputs: baseline from disk when present, else fresh
/// snapshot at current versions (first `review` after a prompt in the same
/// workdir gets the saved baseline via mtimes — see below).
pub fn load_review_inputs(
    workdir: &Path,
    keys: &[PathBuf],
    host: &HostTools,
) -> Result<(Baseline, HashMap<PathBuf, String>), String> {
    // Headless v1: fresh baseline at CURRENT versions would hide pending
    // edits, so instead compare against files on disk: buffers were synced to
    // disk after the turn, and `review` re-reads disk — pending edits are the
    // DIFF between disk-at-baseline-save and disk-now. We approximate by
    // storing the baseline TEXTS at save time.
    let path = baseline_path(workdir);
    let mut baseline = Baseline::new();
    let mut texts = HashMap::new();
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(saved) = serde_json::from_str::<HashMap<String, String>>(&raw) {
            for (key_str, text) in saved {
                let key = PathBuf::from(&key_str);
                // Version unknown across processes — snapshot at a sentinel and
                // let the tracker diff text-vs-text (versions only gate edits).
                baseline.snapshot(&key, Version(0));
                texts.insert(key, text);
            }
            // Refresh buffers from CURRENT disk so rebuild compares saved-vs-now.
            // (Buffers already hold disk text from load_workspace; texts hold baseline.)
            return Ok((baseline, texts));
        }
    }
    // No saved baseline: snapshot current as baseline (empty diff).
    for key in keys {
        if let Some(v) = host.buffer_version(key) {
            baseline.snapshot(key, v);
        }
        if let Some(t) = host.read_buffer(key) {
            texts.insert(key.clone(), t);
        }
    }
    Ok((baseline, texts))
}
