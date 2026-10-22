//! What git ignores in the main worktree (dependencies, build output, `.env`), cloned into a
//! new thread's worktree so it starts ready to build and test. A clone shares the blocks until
//! one side writes, so it is instant and costs no space: APFS `clonefile` on macOS, a reflink
//! copy on Linux. Where the filesystem cannot clone, nothing is copied, since a full copy of a
//! `target/` or `node_modules/` could take minutes and gigabytes; the thread installs its own.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// What git lists as ignored in `root`: a directory ignored whole is one entry. cedian's own
/// `.cedian/` (the worktrees themselves) stays out.
pub fn ignored(root: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args([
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ])
        .current_dir(root)
        .output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            log::warn!(
                "listing ignored files in {}: {}",
                root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Vec::new();
        }
        Err(error) => {
            log::warn!("listing ignored files in {}: {error}", root.display());
            return Vec::new();
        }
    };
    paths(&String::from_utf8_lossy(&output.stdout))
}

fn paths(listing: &str) -> Vec<PathBuf> {
    listing
        .split('\0')
        .map(|entry| entry.trim_end_matches('/'))
        .filter(|entry| !entry.is_empty() && *entry != ".cedian" && !entry.starts_with(".cedian/"))
        .map(PathBuf::from)
        .collect()
}

/// Clones each ignored path of `root` into `dir`, and returns the ones cloned. A path that
/// cannot be cloned is skipped and logged; the worktree is usable either way.
pub fn clone_into(root: &Path, dir: &Path) -> Vec<PathBuf> {
    let mut cloned = Vec::new();
    for path in ignored(root) {
        let from = root.join(&path);
        let to = dir.join(&path);
        if to.symlink_metadata().is_ok() {
            continue;
        }
        if let Some(parent) = to.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            log::warn!("cloning {}: {error}", path.display());
            continue;
        }
        match clone(&from, &to) {
            Ok(()) => cloned.push(path),
            Err(error) => {
                log::info!("not cloning {}: {error:#}", path.display());
                // A failed clone can leave part of a directory behind.
                if to.is_dir() {
                    std::fs::remove_dir_all(&to).ok();
                } else {
                    std::fs::remove_file(&to).ok();
                }
            }
        }
    }
    cloned
}

#[cfg(target_os = "macos")]
fn clone(from: &Path, to: &Path) -> anyhow::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;
    let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
    // `CLONE_NOFOLLOW` from <sys/clonefile.h>: a link is cloned as a link.
    const CLONE_NOFOLLOW: u32 = 0x0001;
    // SAFETY: both are NUL-terminated paths that outlive the call.
    let result = unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), CLONE_NOFOLLOW) };
    anyhow::ensure!(result == 0, std::io::Error::last_os_error());
    Ok(())
}

#[cfg(target_os = "linux")]
fn clone(from: &Path, to: &Path) -> anyhow::Result<()> {
    // `always` refuses rather than falling back to a full copy.
    let output = Command::new("cp")
        .args(["-a", "--reflink=always", "--"])
        .arg(from)
        .arg(to)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn clone(_: &Path, _: &Path) -> anyhow::Result<()> {
    anyhow::bail!("no clone on this system")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listing_leaves_cedians_own_directory_out() {
        assert_eq!(
            paths(".cedian/\0node_modules/\0.env\0crates/a/target/\0"),
            [
                PathBuf::from("node_modules"),
                PathBuf::from(".env"),
                PathBuf::from("crates/a/target"),
            ]
        );
        assert!(paths("").is_empty());
    }

    #[test]
    fn ignored_paths_are_listed_from_git() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(root)
                .status()
                .expect("git");
            assert!(status.success());
        };
        git(&["init", "-q"]);
        std::fs::write(root.join(".gitignore"), "node_modules/\n.env\n").expect("write");
        std::fs::create_dir_all(root.join("node_modules/a")).expect("mkdir");
        std::fs::write(root.join("node_modules/a/index.js"), "x").expect("write");
        std::fs::write(root.join(".env"), "KEY=1").expect("write");
        std::fs::create_dir_all(root.join(".cedian/worktrees")).expect("mkdir");
        std::fs::write(root.join(".git/info/exclude"), "/.cedian/\n").expect("write");
        git(&["add", ".gitignore"]);
        git(&["commit", "-q", "-m", "init"]);
        let mut found = ignored(root);
        found.sort();
        assert_eq!(
            found,
            [PathBuf::from(".env"), PathBuf::from("node_modules")]
        );

        // Into an empty directory: each path is either cloned whole or not there at all.
        let into = tempfile::tempdir().expect("tempdir");
        let cloned = clone_into(root, into.path());
        // APFS always clones, so a skip there is a regression, not a filesystem's limit.
        if cfg!(target_os = "macos") {
            assert!(
                cloned.contains(&PathBuf::from("node_modules")),
                "{cloned:?}"
            );
        }
        for path in [".env", "node_modules"] {
            let there = into.path().join(path).exists();
            assert_eq!(there, cloned.contains(&PathBuf::from(path)), "{path}");
        }
        if cloned.contains(&PathBuf::from("node_modules")) {
            assert_eq!(
                std::fs::read_to_string(into.path().join("node_modules/a/index.js")).expect("read"),
                "x"
            );
        }
    }
}
