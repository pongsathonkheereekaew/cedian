use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

use crate::{Hunk, HunkState, Shadow, parse_patch};

/// The decisions made so far, by hunk id: `Accepted` or `Rejected`.
pub type Decisions = BTreeMap<String, HunkState>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileReview {
    pub path: String,
    pub binary: bool,
    pub added: usize,
    pub removed: usize,
    pub hunks: Vec<Hunk>,
}

/// A thread's changes from its first snapshot to its last.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Review {
    /// The first snapshot's tree; none when the thread has not written anything.
    pub base: Option<String>,
    pub files: Vec<FileReview>,
}

/// The hunks from `base` to `head`, each marked with its decision, or `Stale` when the file in
/// `work_tree` no longer has the agent's lines.
pub fn review(
    shadow: &Shadow,
    base: &str,
    head: &str,
    work_tree: &Path,
    decisions: &Decisions,
) -> anyhow::Result<Review> {
    let patch = shadow.patch(base, head)?;
    let mut files = Vec::new();
    for diff in parse_patch(&patch) {
        let current = read_lines(&work_tree.join(&diff.path))?;
        let mut hunks = diff.hunks;
        let (mut added, mut removed) = (0, 0);
        for hunk in &mut hunks {
            added += hunk.lines.iter().filter(|l| l.starts_with('+')).count();
            removed += hunk.lines.iter().filter(|l| l.starts_with('-')).count();
            hunk.state = match decisions.get(&hunk.id) {
                Some(state) => *state,
                None if locate(current.as_deref(), hunk).is_some() => HunkState::Pending,
                None => HunkState::Stale,
            };
        }
        files.push(FileReview {
            path: diff.path,
            binary: diff.binary,
            added,
            removed,
            hunks,
        });
    }
    Ok(Review {
        base: Some(base.to_owned()),
        files,
    })
}

/// Puts the hunk's old lines back in `work_tree`. A hunk edited since is refused: it is no
/// longer the agent's.
pub fn reject(work_tree: &Path, hunk: &Hunk) -> anyhow::Result<()> {
    let path = work_tree.join(&hunk.path);
    let current = read_lines(&path)?;
    let Some(at) = locate(current.as_deref(), hunk) else {
        bail!("{} was edited after the agent wrote it", hunk.path);
    };
    let mut lines = current.unwrap_or_default();
    lines.splice(at..at + hunk.new.len(), hunk.old.iter().cloned());
    if lines.is_empty() && hunk.old.is_empty() {
        // The agent added the file: rejecting it removes it.
        return match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error).with_context(|| format!("removing {}", path.display()))
            }
            _ => Ok(()),
        };
    }
    write_atomic(&path, &lines.concat())
}

/// The file's lines, each with its newline, or none when it does not exist.
fn read_lines(path: &Path) -> anyhow::Result<Option<Vec<String>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(
            String::from_utf8_lossy(&bytes)
                .split_inclusive('\n')
                .map(str::to_owned)
                .collect(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Where the hunk's new lines are in the file now, nearest the line the diff names.
fn locate(current: Option<&[String]>, hunk: &Hunk) -> Option<usize> {
    let lines = current.unwrap_or_default();
    if hunk.new.is_empty() {
        // The agent deleted the file; it is still the agent's while the file stays gone.
        return lines.is_empty().then_some(0);
    }
    let want = hunk.new_start.saturating_sub(1);
    (0..=lines.len().checked_sub(hunk.new.len())?)
        .filter(|&at| lines[at..at + hunk.new.len()] == hunk.new[..])
        .min_by_key(|&at| at.abs_diff(want))
}

fn write_atomic(path: &Path, text: &str) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = dir.join(format!(".{name}.cedian-{}", std::process::id()));
    std::fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&temp, meta.permissions());
    }
    std::fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))
}
