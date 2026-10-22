use serde::{Deserialize, Serialize};

/// Where a hunk stands in the review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HunkState {
    /// Waiting for a decision.
    Pending,
    Accepted,
    /// Put back to its old lines, and the thread told why.
    Rejected,
    /// Edited by hand after the agent wrote it, so it is not the agent's any more and is never
    /// rejected.
    Stale,
}

/// One hunk of a file's diff. `id` names it across reviews: the file and the hunk's text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hunk {
    pub id: String,
    pub path: String,
    /// The `@@ -a,b +c,d @@` line, without the `@@`s' trailing text.
    pub header: String,
    pub new_start: usize,
    /// The diff's lines with their ` `, `-` or `+`, newlines left off.
    pub lines: Vec<String>,
    pub state: HunkState,
    /// The old side's lines, each with its newline as the file had it.
    #[serde(skip)]
    pub(crate) old: Vec<String>,
    /// The new side's lines, each with its newline as the agent wrote it.
    #[serde(skip)]
    pub(crate) new: Vec<String>,
    /// Which sides the last line pushed belongs to, for a `\ No newline` marker.
    #[serde(skip)]
    last: (bool, bool),
}

impl Hunk {
    pub(crate) fn start(path: &str, header: &str) -> Option<Self> {
        let ranges = header.split(" @@").next()?;
        let new = ranges
            .split_whitespace()
            .find_map(|r| r.strip_prefix('+'))?;
        let new_start = new.split(',').next()?.parse().ok()?;
        Some(Self {
            id: String::new(),
            path: path.to_owned(),
            header: format!("@@ {ranges} @@"),
            new_start,
            lines: Vec::new(),
            state: HunkState::Pending,
            old: Vec::new(),
            new: Vec::new(),
            last: (false, false),
        })
    }

    pub(crate) fn push(&mut self, mark: char, text: &str) {
        self.lines.push(format!("{mark}{text}"));
        let line = format!("{text}\n");
        let (old, new) = (mark != '+', mark != '-');
        if old {
            self.old.push(line.clone());
        }
        if new {
            self.new.push(line);
        }
        self.last = (old, new);
    }

    pub(crate) fn no_newline(&mut self) {
        let (old, new) = self.last;
        if old && let Some(line) = self.old.last_mut() {
            line.pop();
        }
        if new && let Some(line) = self.new.last_mut() {
            line.pop();
        }
    }

    pub(crate) fn finish(&mut self) {
        let mut text = self.path.clone();
        for line in &self.lines {
            text.push('\n');
            text.push_str(line);
        }
        self.id = format!("{:016x}", fnv1a(text.as_bytes()));
    }

    /// The hunk as a unified diff, for the message that tells the thread.
    pub fn diff_text(&self) -> String {
        let mut text = self.header.clone();
        for line in &self.lines {
            text.push('\n');
            text.push_str(line);
        }
        text
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
