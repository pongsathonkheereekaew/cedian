//! Reviewer findings + feedback payloads (plan §19).
//!
//! Reviewers are read-only with structured findings; completion gates require
//! evidence. Feedback carries exact review context (path, range, diff,
//! comment, task) so OMP receives precise pointers, not pasted text.

use serde::{Deserialize, Serialize};

/// One reviewer finding (read-only reviewer output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub path: String,
    /// 0-based start line.
    pub start_line: usize,
    /// Line count.
    pub line_count: usize,
    pub severity: FindingSeverity,
    pub message: String,
}

/// Finding severity (gates treat `Required` as blocking).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Info,
    Suggestion,
    Required,
}

/// Structured feedback to OMP (Ask/Fix/Explain + inline comments).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFeedback {
    pub path: String,
    /// 0-based `(start_line, line_count)`.
    pub range: (usize, usize),
    /// Hunk diff text the comment refers to.
    pub diff: String,
    pub comment: String,
    pub task_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_roundtrips() {
        let f = ReviewFeedback {
            path: "/a.rs".to_string(),
            range: (3, 2),
            diff: "-x\n+x\n".to_string(),
            comment: "why?".to_string(),
            task_id: "task-1".to_string(),
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: ReviewFeedback = serde_json::from_str(&json).unwrap();
        assert_eq!(back.range, (3, 2));
    }
}
