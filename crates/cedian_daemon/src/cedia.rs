use std::{
    collections::HashMap,
    os::unix::process::CommandExt as _,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use anyhow::Context as _;
use cedian_protocol::{
    Capability, CommandList, ModelList, PermissionList, Rating, Schedule, VersionInfo,
};

/// A run's flags beyond its text: cedia's `--profile`, `--model`, `--permission`, `--ask on`
/// and `--todo on`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flags<'a> {
    pub profile: Option<&'a str>,
    pub model: Option<&'a str>,
    pub permission: Option<&'a str>,
    /// The run may end on a question for the person, when this cedia can.
    pub ask: bool,
    /// The writer keeps a todo list, when this cedia can.
    pub todo: bool,
}

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
        known_capabilities(&output.stdout).map_err(|e| format!("cedia's version: {e}"))
    }

    /// `cedia models --json`: the models a run can take, and which one it takes by default.
    pub fn models(&self) -> Result<ModelList, String> {
        self.list(None, "models")
    }

    /// `cedia permissions --json`: the modes a run can be given.
    pub fn permissions(&self) -> Result<PermissionList, String> {
        self.list(None, "permissions")
    }

    /// `cedia commands --json --cwd <cwd>`: the project's `/` commands.
    pub fn commands(&self, cwd: &Path) -> Result<CommandList, String> {
        self.list(Some(cwd), "commands")
    }

    /// `cedia schedules --json --cwd <cwd>`: the project's scheduled runs.
    pub fn schedules(&self, cwd: &Path) -> Result<Vec<Schedule>, String> {
        self.list(Some(cwd), "schedules")
    }

    /// `cedia schedules <verb> [<id>] --json --cwd <cwd>`: `due` lists the runs due now,
    /// `fire <id>` marks one started, `pause <id>` and `resume <id>` turn one off and on.
    /// `fire` and `due` print the schedules they touch; the others print a line, not read.
    pub fn schedule(
        &self,
        cwd: &Path,
        verb: &str,
        id: Option<&str>,
    ) -> Result<Vec<Schedule>, String> {
        let mut run = self.command(cwd);
        run.args(["schedules", verb]).args(id).arg("--cwd").arg(cwd);
        if matches!(verb, "due" | "fire") {
            run.arg("--json");
        }
        let output = run
            .output()
            .map_err(|e| format!("running {}: {e}", self.bin.display()))?;
        if !output.status.success() {
            return Err(format!(
                "{} schedules {verb} exited with {}: {}",
                self.bin.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        if !matches!(verb, "due" | "fire") {
            return Ok(Vec::new());
        }
        serde_json::from_slice(&output.stdout).map_err(|e| format!("cedia schedules {verb}: {e}"))
    }

    /// `cedia <command> --json [--cwd <cwd>]`, read as `T`.
    fn list<T: serde::de::DeserializeOwned>(
        &self,
        cwd: Option<&Path>,
        command: &str,
    ) -> Result<T, String> {
        let mut run = self.command(cwd.unwrap_or(Path::new("/")));
        run.args([command, "--json"]);
        if let Some(cwd) = cwd {
            run.arg("--cwd").arg(cwd);
        }
        let output = run
            .output()
            .map_err(|e| format!("running {}: {e}", self.bin.display()))?;
        if !output.status.success() {
            return Err(format!(
                "{} {command} --json exited with {}: {}",
                self.bin.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        serde_json::from_slice(&output.stdout).map_err(|e| format!("cedia {command}: {e}"))
    }

    /// One run: `cedia --print=<text> --output-format stream-json --cwd <cwd> [--thread <id>]
    /// [--profile <name>] [--model <ref>] [--permission <mode>] [--ask on] [--todo on]`. `--print=`
    /// keeps a prompt that starts with `-` from reading as a flag.
    /// `ask` lets the run end on a question for the person, when this cedia can.
    pub fn prompt(
        &self,
        cwd: &Path,
        text: &str,
        thread: Option<&str>,
        flags: Flags,
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
        if let Some(profile) = flags.profile {
            command.args(["--profile", profile]);
        }
        if let Some(model) = flags.model {
            command.args(["--model", model]);
        }
        if let Some(permission) = flags.permission {
            command.args(["--permission", permission]);
        }
        if flags.ask {
            command.args(["--ask", "on"]);
        }
        // The writer's todo list is the thread's progress in the window.
        if flags.todo {
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

    /// `cedia threads fork <id> <run> --json`: a new thread whose conversation is `thread`'s
    /// just before `run`, for the person's message there sent again, edited. Its id.
    pub fn fork(&self, cwd: &Path, thread: &str, run: &str) -> anyhow::Result<String> {
        #[derive(serde::Deserialize)]
        struct Forked {
            thread: String,
        }
        let output = self.threads_output(cwd, &["fork", thread, run, "--json"])?;
        let forked: Forked =
            serde_json::from_slice(&output).context("reading cedia threads fork --json")?;
        Ok(forked.thread)
    }

    /// `cedia threads --cwd <cwd> <args>`, which writes one event to a thread's file.
    fn threads(&self, cwd: &Path, args: &[&str]) -> anyhow::Result<()> {
        self.threads_output(cwd, args).map(drop)
    }

    /// `cedia threads --cwd <cwd> <args>`, and what it printed.
    fn threads_output(&self, cwd: &Path, args: &[&str]) -> anyhow::Result<Vec<u8>> {
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
        Ok(output.stdout)
    }
}

/// cedia's version, without the capabilities this pin does not know: a cedia newer than the pin
/// still runs, with only what cedian can use.
fn known_capabilities(json: &[u8]) -> serde_json::Result<VersionInfo> {
    let mut version: serde_json::Value = serde_json::from_slice(json)?;
    if let Some(capabilities) = version
        .get_mut("capabilities")
        .and_then(serde_json::Value::as_array_mut)
    {
        capabilities.retain(|capability| {
            let known = serde_json::from_value::<Capability>(capability.clone()).is_ok();
            if !known {
                log::info!("cedia offers {capability}, which this pin does not know");
            }
            known
        });
    }
    serde_json::from_value(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capability_newer_than_the_pin_is_left_out() {
        let version = known_capabilities(
            br#"{"name":"cedia","version":"0.1.0","protocol":"x","capabilities":["ask","from-the-future"]}"#,
        )
        .expect("version");
        assert_eq!(version.capabilities, vec![Capability::Ask]);
    }
}
