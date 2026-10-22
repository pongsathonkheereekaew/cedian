//! A thread's review: its hunks from cedia's shadow git, and the decisions made on them, kept
//! in the daemon's state directory.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use cedian_index::{ThreadSummary, project_slug, session_cwd, session_path, session_trees};
use cedian_review::{Decisions, Hunk, HunkState, Review, Shadow};
use cedian_wire::Decision;

/// Where a thread's changes are: the directory its latest session ran in, and the first and
/// last trees its sessions there logged.
pub struct Changes {
    pub work_tree: PathBuf,
    trees: Option<(String, String)>,
}

impl Changes {
    pub fn of(home: &Path, slug: &str, thread: &ThreadSummary) -> anyhow::Result<Self> {
        let paths: Vec<PathBuf> = thread
            .sessions
            .iter()
            .map(|s| session_path(home, slug, &s.session_id, s.path.as_deref()))
            .collect();
        let work_tree = paths
            .last()
            .and_then(|path| session_cwd(path))
            .with_context(|| format!("{} has no session yet", thread.id))?;
        let mut trees = Vec::new();
        for path in &paths {
            if session_cwd(path).as_deref() == Some(work_tree.as_path()) {
                trees.extend(session_trees(path)?);
            }
        }
        let trees = match (trees.first(), trees.last()) {
            (Some(first), Some(last)) => Some((first.clone(), last.clone())),
            _ => None,
        };
        Ok(Self { work_tree, trees })
    }

    pub fn review(&self, home: &Path, decisions: &Decisions) -> anyhow::Result<Review> {
        let Some((base, head)) = &self.trees else {
            return Ok(Review::default());
        };
        // cedia files a directory's snapshots under its own slug, not the project's.
        let shadow = Shadow::new(home.join("snapshots").join(project_slug(&self.work_tree)));
        if !shadow.exists() {
            bail!("no snapshots for {}", self.work_tree.display());
        }
        cedian_review::review(&shadow, base, head, &self.work_tree, decisions)
    }
}

/// Each thread's decisions, one file per thread under `<state dir>/review`.
pub struct DecisionStore {
    dir: PathBuf,
}

impl DecisionStore {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            dir: state_dir.join("review"),
        }
    }

    pub fn load(&self, thread: &str) -> Decisions {
        let path = self.path(thread);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                log::warn!("{}: {error}", path.display());
                Decisions::new()
            }),
            Err(_) => Decisions::new(),
        }
    }

    pub fn save(&self, thread: &str, decisions: &Decisions) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating {}", self.dir.display()))?;
        let path = self.path(thread);
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(decisions)?)
            .with_context(|| format!("writing {}", temp.display()))?;
        std::fs::rename(&temp, &path).with_context(|| format!("replacing {}", path.display()))
    }

    fn path(&self, thread: &str) -> PathBuf {
        let name: String = thread
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.dir.join(format!("{name}.json"))
    }
}

/// Applies one decision. A reject writes the worktree and returns the message for the thread.
pub fn decide(
    review: &Review,
    work_tree: &Path,
    decisions: &mut Decisions,
    id: &str,
    decision: &Decision,
) -> anyhow::Result<Option<String>> {
    let hunk = review
        .files
        .iter()
        .flat_map(|file| &file.hunks)
        .find(|hunk| hunk.id == id)
        .with_context(|| "that hunk is no longer in the review")?;
    match decision {
        Decision::Accept => {
            if hunk.state != HunkState::Pending {
                bail!("only a pending hunk can be accepted");
            }
            decisions.insert(id.to_owned(), HunkState::Accepted);
            Ok(None)
        }
        Decision::Undo => {
            if hunk.state != HunkState::Accepted {
                bail!("only an accepted hunk can be undone");
            }
            decisions.remove(id);
            Ok(None)
        }
        Decision::Reject { reason } => {
            if !matches!(hunk.state, HunkState::Pending | HunkState::Accepted) {
                bail!("a stale or rejected hunk cannot be rejected");
            }
            cedian_review::reject(work_tree, hunk)?;
            decisions.insert(id.to_owned(), HunkState::Rejected);
            Ok(Some(rejection(hunk, reason)))
        }
    }
}

fn rejection(hunk: &Hunk, reason: &str) -> String {
    let reason = reason.trim();
    let why = if reason.is_empty() {
        String::new()
    } else {
        format!(" Why: {reason}")
    };
    format!(
        "I rejected your change to {} and put the old lines back in the file.{why}\n\n```diff\n{}\n```",
        hunk.path,
        hunk.diff_text()
    )
}
