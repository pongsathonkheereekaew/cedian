use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, bail};

/// cedia's shadow git for one working directory, read only: cedian never adds to it.
pub struct Shadow {
    git_dir: PathBuf,
}

impl Shadow {
    /// `~/.cedia/snapshots/<slug of the working directory>`, as cedia's headless client opens it.
    pub fn new(git_dir: PathBuf) -> Self {
        Self { git_dir }
    }

    pub fn exists(&self) -> bool {
        self.git_dir.join("HEAD").exists()
    }

    /// The unified diff between two trees, three lines of context.
    pub fn patch(&self, from: &str, to: &str) -> anyhow::Result<String> {
        if from == to {
            return Ok(String::new());
        }
        self.git(&[
            "diff-tree",
            "-r",
            "-p",
            "-U3",
            "--no-color",
            "--no-renames",
            "--no-ext-diff",
            from,
            to,
        ])
    }

    fn git(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "-c", "gc.auto=0"])
            .args(args)
            .env_clear()
            .env(
                "PATH",
                std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            )
            .env("GIT_DIR", &self.git_dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .current_dir(existing_parent(&self.git_dir))
            .output()
            .with_context(|| format!("running git in {}", self.git_dir.display()))?;
        if !output.status.success() {
            bail!(
                "git {}: {}",
                args[0],
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn existing_parent(path: &Path) -> &Path {
    let mut dir = path;
    while !dir.is_dir() {
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    dir
}
