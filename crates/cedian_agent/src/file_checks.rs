//! Which of a run's checks still cover a file (Interface, Diff review: Evidence). A check
//! covers a file when it last ran after the file's last edit; an edit after it outdates it.

use cedian_protocol::EffectStatus;

use crate::{Block, ToolState, Transcript};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileChecks {
    /// Commands that ran after the file's last edit.
    pub covering: Vec<String>,
    /// Commands an edit of the file came after.
    pub outdated: Vec<String>,
}

impl Transcript {
    /// The latest receipt's checks, split by whether they ran after `path` was last edited.
    /// A check the transcript never shows running (the done gate ran it at the end) covers.
    pub fn file_checks(&self, path: &str) -> FileChecks {
        let blocks = self.blocks();
        let Some(checks) = blocks.iter().rev().find_map(|block| match block {
            Block::Result {
                evidence: Some(evidence),
                ..
            } => Some(&evidence.checks),
            _ => None,
        }) else {
            return FileChecks::default();
        };
        let completed = |state: ToolState| state == ToolState::Done(EffectStatus::Completed);
        let edited = blocks.iter().rposition(|block| {
            matches!(block, Block::Tool(call)
                if matches!(call.name.as_str(), "edit" | "write")
                    && completed(call.state)
                    && call
                        .input
                        .get("path")
                        .and_then(|p| p.as_str())
                        .is_some_and(|p| same_file(p, path)))
        });
        let mut result = FileChecks::default();
        for check in checks {
            let ran = blocks.iter().rposition(|block| {
                matches!(block, Block::Tool(call)
                    if call.name == "bash"
                        && call.input.get("command").and_then(|c| c.as_str())
                            == Some(check.command.as_str()))
            });
            match (edited, ran) {
                (Some(edit), Some(run)) if run < edit => {
                    result.outdated.push(check.command.clone())
                }
                _ => result.covering.push(check.command.clone()),
            }
        }
        result
    }
}

/// A tool's path (absolute, or relative to where the agent ran) names the review's
/// repository-relative path.
fn same_file(tool_path: &str, path: &str) -> bool {
    let tool_path = tool_path.strip_prefix("./").unwrap_or(tool_path);
    tool_path == path || tool_path.ends_with(&format!("/{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_match_relative_and_absolute() {
        assert!(same_file("f.txt", "f.txt"));
        assert!(same_file("./src/a.rs", "src/a.rs"));
        assert!(same_file("/repo/src/a.rs", "src/a.rs"));
        assert!(!same_file("/repo/xsrc/a.rs", "src/a.rs"));
    }
}
