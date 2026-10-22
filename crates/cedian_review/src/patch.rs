use crate::hunk::Hunk;

/// One file's part of a patch. `path` is the new path, or the old one for a deleted file.
#[derive(Clone, Debug, PartialEq)]
pub struct FileDiff {
    pub path: String,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

/// Reads `git diff-tree -p` output. A `\ No newline at end of file` marker takes the newline
/// off the line before it, so each side's lines are the file's own lines.
pub fn parse_patch(text: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut old_path: Option<String> = None;
    // Split on `\n` only: a `\r` is part of the line, as it is of the file.
    for line in text.split_terminator('\n') {
        if line.starts_with("diff --git ") {
            files.push(FileDiff {
                path: String::new(),
                binary: false,
                hunks: Vec::new(),
            });
            old_path = None;
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if file.hunks.is_empty() {
            if let Some(path) = line.strip_prefix("--- ") {
                old_path = side_path(path);
                continue;
            }
            if let Some(path) = line.strip_prefix("+++ ") {
                file.path = side_path(path).or(old_path.take()).unwrap_or_default();
                continue;
            }
            if line.starts_with("Binary files ") {
                file.binary = true;
                file.path = binary_path(line).unwrap_or_default();
                continue;
            }
        }
        if let Some(header) = line.strip_prefix("@@ ") {
            if let Some(hunk) = Hunk::start(&file.path, header) {
                file.hunks.push(hunk);
            }
            continue;
        }
        let Some(hunk) = file.hunks.last_mut() else {
            continue;
        };
        if line.starts_with('\\') {
            hunk.no_newline();
        } else if let Some(rest) = line.strip_prefix(' ') {
            hunk.push(' ', rest);
        } else if let Some(rest) = line.strip_prefix('-') {
            hunk.push('-', rest);
        } else if let Some(rest) = line.strip_prefix('+') {
            hunk.push('+', rest);
        } else if line.is_empty() {
            hunk.push(' ', "");
        }
    }
    for file in &mut files {
        for hunk in &mut file.hunks {
            hunk.finish();
        }
    }
    files.retain(|file| !file.path.is_empty());
    files
}

fn side_path(path: &str) -> Option<String> {
    let path = path.trim_end_matches('\t');
    if path == "/dev/null" {
        return None;
    }
    let path = path
        .strip_prefix("a/")
        .or(path.strip_prefix("b/"))
        .unwrap_or(path);
    Some(path.to_owned())
}

fn binary_path(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("Binary files ")?
        .strip_suffix(" differ")?;
    let (old, new) = rest.split_once(" and ")?;
    side_path(new).or(side_path(old))
}
