use std::{
    collections::HashMap,
    os::unix::process::CommandExt as _,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use anyhow::Context as _;
use cedian_protocol::{Rating, VersionInfo};

/// How the daemon starts cedia: which binary, with which environment.
#[derive(Clone, Debug)]
pub struct Cedia {
    pub bin: PathBuf,
    pub env: HashMap<String, String>,
}

impl Cedia {
    /// `$CEDIAN_CEDIA_BIN`, else a `cedia` next to the daemon (the bundled build), else `cedia`
    /// on the login shell's PATH.
    pub fn find(env: HashMap<String, String>) -> Self {
        let bundled = std::env::current_exe()
            .ok()
            .and_then(|exe| Some(exe.parent()?.join("cedia")))
            .filter(|path| path.is_file());
        let bin = env
            .get("CEDIAN_CEDIA_BIN")
            .map(PathBuf::from)
            .or(bundled)
            .unwrap_or_else(|| PathBuf::from("cedia"));
        Self { bin, env }
    }

    fn command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(&self.bin);
        command
            .env_clear()
            .envs(&self.env)
            .current_dir(cwd)
            .stdin(Stdio::null());
        command
    }

    /// `cedia --version --json`.
    pub fn version(&self) -> Result<VersionInfo, String> {
        let output = self
            .command(Path::new("/"))
            .args(["--version", "--json"])
            .output()
            .map_err(|e| format!("running {}: {e}", self.bin.display()))?;
        if !output.status.success() {
            return Err(format!(
                "{} --version --json exited with {}: {}",
                self.bin.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        serde_json::from_slice(&output.stdout).map_err(|e| format!("cedia's version: {e}"))
    }

    /// One run: `cedia --print=<text> --output-format stream-json --cwd <cwd> [--thread <id>]
    /// [--ask on]`. `--print=` keeps a prompt that starts with `-` from reading as a flag.
    /// `ask` lets the run end on a question for the person, when this cedia can.
    pub fn prompt(
        &self,
        cwd: &Path,
        text: &str,
        thread: Option<&str>,
        ask: bool,
        todo: bool,
    ) -> anyhow::Result<Child> {
        let mut command = self.command(cwd);
        command
            .arg(format!("--print={text}"))
            .args(["--output-format", "stream-json", "--cwd"])
            .arg(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Its own process group, so the run outlives a daemon that launchd restarts.
            .process_group(0);
        if let Some(thread) = thread {
            command.args(["--thread", thread]);
        }
        if ask {
            command.args(["--ask", "on"]);
        }
        // The writer's todo list is the thread's progress in the window.
        if todo {
            command.args(["--todo", "on"]);
        }
        command
            .spawn()
            .with_context(|| format!("starting {}", self.bin.display()))
    }

    /// `cedia threads send <id> -- <text>`: the thread sees it at its next turn.
    pub fn send(&self, cwd: &Path, thread: &str, text: &str) -> anyhow::Result<()> {
        self.threads(cwd, &["send", thread, "--", text])
    }

    /// `cedia threads unsend <id> <message>...`: the messages leave the thread's queue.
    pub fn unsend(&self, cwd: &Path, thread: &str, ids: &[String]) -> anyhow::Result<()> {
        let mut args = vec!["unsend", thread];
        args.extend(ids.iter().map(String::as_str));
        self.threads(cwd, &args)
    }

    /// `cedia threads archive|unarchive <id>`.
    pub fn archive(&self, cwd: &Path, thread: &str, archived: bool) -> anyhow::Result<()> {
        let verb = if archived { "archive" } else { "unarchive" };
        self.threads(cwd, &[verb, thread])
    }

    /// `cedia threads rate <id> up|down`.
    pub fn rate(&self, cwd: &Path, thread: &str, rating: Rating) -> anyhow::Result<()> {
        let word = match rating {
            Rating::Up => "up",
            Rating::Down => "down",
        };
        self.threads(cwd, &["rate", thread, word])
    }

    /// `cedia threads --cwd <cwd> <args>`, which writes one event to a thread's file.
    fn threads(&self, cwd: &Path, args: &[&str]) -> anyhow::Result<()> {
        let output = self
            .command(cwd)
            .args(["threads", "--cwd"])
            .arg(cwd)
            .args(args)
            .output()
            .with_context(|| format!("running {}", self.bin.display()))?;
        anyhow::ensure!(
            output.status.success(),
            "cedia threads {} exited with {}: {}",
            args.first().copied().unwrap_or_default(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(())
    }
}
