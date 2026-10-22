use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use anyhow::Context as _;
use cedian_protocol::VersionInfo;

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

    /// One run: `cedia --print=<text> --output-format stream-json --cwd <cwd> [--thread <id>]`.
    /// `--print=` keeps a prompt that starts with `-` from reading as a flag.
    pub fn prompt(&self, cwd: &Path, text: &str, thread: Option<&str>) -> anyhow::Result<Child> {
        let mut command = self.command(cwd);
        command
            .arg(format!("--print={text}"))
            .args(["--output-format", "stream-json", "--cwd"])
            .arg(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(thread) = thread {
            command.args(["--thread", thread]);
        }
        command
            .spawn()
            .with_context(|| format!("starting {}", self.bin.display()))
    }

    /// `cedia threads send <id> -- <text>`: the thread sees it at its next turn.
    pub fn send(&self, cwd: &Path, thread: &str, text: &str) -> anyhow::Result<()> {
        let output = self
            .command(cwd)
            .args(["threads", "--cwd"])
            .arg(cwd)
            .args(["send", thread, "--", text])
            .output()
            .with_context(|| format!("running {}", self.bin.display()))?;
        anyhow::ensure!(
            output.status.success(),
            "cedia threads send exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(())
    }
}
