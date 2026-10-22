//! The key a project's threads and sessions are filed under, computed the way cedia does
//! (`projectKey` in cedia's `src/core/session/log.ts`): every git worktree of a repository is
//! one project, keyed by the main worktree's root.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// The main worktree's root for a directory in git, else the directory itself.
pub fn project_root(cwd: &Path) -> PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(cwd)
        .output();
    let common = match output {
        Ok(output) if output.status.success() => {
            PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
        }
        _ => return cwd.to_path_buf(),
    };
    match common.parent() {
        Some(root) if common.file_name().is_some_and(|name| name == ".git") => root.to_path_buf(),
        _ => cwd.to_path_buf(),
    }
}

/// A path as a directory name: the leading `/` dropped and every run of other characters
/// than `A-Za-z0-9._-` turned into one `-`.
pub fn project_slug(root: &Path) -> String {
    let text = root.to_string_lossy();
    let text = text.strip_prefix('/').unwrap_or(&text);
    let mut slug = String::with_capacity(text.len());
    let mut in_run = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            slug.push(c);
            in_run = false;
        } else if !in_run {
            slug.push('-');
            in_run = true;
        }
    }
    if slug.is_empty() {
        "root".to_owned()
    } else {
        slug
    }
}

pub fn project_key(cwd: &Path) -> String {
    project_slug(&project_root(cwd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_match_cedia() {
        assert_eq!(project_slug(Path::new("/fixture/repo")), "fixture-repo");
        assert_eq!(
            project_slug(Path::new("/Users/pond/My Projects/a b")),
            "Users-pond-My-Projects-a-b"
        );
        assert_eq!(project_slug(Path::new("/tmp/งาน/x")), "tmp-x");
        assert_eq!(project_slug(Path::new("/")), "root");
    }
}
