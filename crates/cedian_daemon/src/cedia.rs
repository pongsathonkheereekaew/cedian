use std::{
    collections::HashMap,
    os::unix::process::CommandExt as _,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use anyhow::Context as _;
use cedian_protocol::{
    Capability, CommandList, McpCheck, McpList, ModelList, PermissionList, Rating, Restored,
    RewindResult, Schedule, UsageReport, VersionInfo,
};

/// A run's flags beyond its text: cedia's `--profile`, `--model`, `--permission`, `--effort`,
/// the helper roles (`--plan`, `--review`, `--advisor`, `--goal`), `--ask on`, `--todo on` and
/// `--image`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flags<'a> {
    pub profile: Option<&'a str>,
    pub model: Option<&'a str>,
    pub permission: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub helpers: Option<&'a cedian_wire::Helpers>,
    /// The run may end on a question for the person, when this cedia can.
    pub ask: bool,
    /// The writer keeps a todo list, when this cedia can.
    pub todo: bool,
    /// The project's page in the shared browser (`--browser`), when it runs and this cedia can
    /// drive it.
    pub browser: Option<&'a str>,
    /// Image files shown to the writer with the text (`--image`), when this cedia can.
    pub images: &'a [PathBuf],
    /// How long the writer's cached prompt lasts (`--cache-ttl`), when this cedia can set it.
    pub cache_ttl: Option<&'a str>,
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

    /// `cedia usage --json --since <since> (--cwd <cwd> | --all)`: the spend in the range, in
    /// one project or every one.
    pub fn usage(&self, cwd: Option<&Path>, since: &str) -> Result<UsageReport, String> {
        let range = ["--since", since];
        match cwd {
            Some(_) => self.list_with(cwd, "usage", &range),
            None => self.list_with(None, "usage", &[range[0], range[1], "--all"]),
        }
    }

    /// `cedia mcp list --json`: the person's MCP servers, without the values of their
    /// environments and headers.
    pub fn mcp_list(&self) -> Result<McpList, String> {
        self.list_with(None, "mcp", &["list"])
    }

    /// `cedia mcp check <name> --json`: starts the server, lists its tools and stops it. A
    /// server that did not start is an answer too (exit 1 with `ok: false`).
    pub fn mcp_check(&self, name: &str) -> Result<McpCheck, String> {
        let output = self
            .command(Path::new("/"))
            .args(["mcp", "check", "--json", "--", name])
            .output()
            .map_err(|e| format!("running {}: {e}", self.bin.display()))?;
        serde_json::from_slice(&output.stdout).map_err(|_| {
            format!(
                "cedia mcp check exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )
        })
    }

    /// `cedia mcp add <name> (--command <c> --arg=<a>... --env=<K=V>... | --url <u>
    /// --header=<K=V>...) [--read-only]`. `--arg=` keeps an argument that starts with `-` from
    /// reading as a flag.
    pub fn mcp_add(&self, name: &str, server: &cedian_wire::McpAdd) -> Result<(), String> {
        let mut run = self.command(Path::new("/"));
        run.args(["mcp", "add"]);
        let pair = match &server.target {
            cedian_wire::McpTarget::Command { command, args } => {
                run.arg(format!("--command={command}"));
                run.args(args.iter().map(|arg| format!("--arg={arg}")));
                "env"
            }
            cedian_wire::McpTarget::Url { url } => {
                run.arg(format!("--url={url}"));
                "header"
            }
        };
        run.args(
            server
                .pairs
                .iter()
                .map(|(key, value)| format!("--{pair}={key}={value}")),
        );
        if server.read_only {
            run.arg("--read-only");
        }
        self.mcp_change(run.arg("--").arg(name), "add")
    }

    /// `cedia mcp remove <name>`.
    pub fn mcp_remove(&self, name: &str) -> Result<(), String> {
        let mut run = self.command(Path::new("/"));
        self.mcp_change(run.args(["mcp", "remove", "--", name]), "remove")
    }

    /// Runs a change to `mcpServers`; cedia's refusal is its last line on stderr.
    fn mcp_change(&self, run: &mut Command, verb: &str) -> Result<(), String> {
        let output = run
            .output()
            .map_err(|e| format!("running {}: {e}", self.bin.display()))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let line = stderr.trim().lines().last().unwrap_or_default();
        Err(match line.strip_prefix("cedia: ") {
            Some(reason) => reason.to_owned(),
            None => format!("cedia mcp {verb} exited with {}: {line}", output.status),
        })
    }

    /// `cedia <command> --json [--cwd <cwd>]`, read as `T`.
    fn list<T: serde::de::DeserializeOwned>(
        &self,
        cwd: Option<&Path>,
        command: &str,
    ) -> Result<T, String> {
        self.list_with(cwd, command, &[])
    }

    /// `cedia <command> --json <args> [--cwd <cwd>]`, read as `T`.
    fn list_with<T: serde::de::DeserializeOwned>(
        &self,
        cwd: Option<&Path>,
        command: &str,
        args: &[&str],
    ) -> Result<T, String> {
        let mut run = self.command(cwd.unwrap_or(Path::new("/")));
        run.args([command, "--json"]).args(args);
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
    /// [--profile <name>] [--model <ref>] [--permission <mode>] [--effort <level>] [--ask on]
    /// [--todo on] [--browser <page>] [--cache-ttl <ttl>] [--image <file>]...`. `--print=`
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
        if let Some(effort) = flags.effort {
            command.args(["--effort", effort]);
        }
        if let Some(helpers) = flags.helpers {
            for (flag, value) in [
                ("--plan", &helpers.plan),
                ("--review", &helpers.review),
                ("--advisor", &helpers.advisor),
                ("--goal", &helpers.goal),
            ] {
                if let Some(value) = value {
                    command.arg(flag).arg(value);
                }
            }
        }
        if flags.ask {
            command.args(["--ask", "on"]);
        }
        // The writer's todo list is the thread's progress in the window.
        if flags.todo {
            command.args(["--todo", "on"]);
        }
        // The page the person sees in the Browser block (core/tools/browser.ts in cedia).
        if let Some(page) = flags.browser {
            command.args(["--browser", page]);
        }
        if let Some(ttl) = flags.cache_ttl {
            command.args(["--cache-ttl", ttl]);
        }
        for image in flags.images {
            command.arg("--image").arg(image);
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

    /// `cedia threads rename <id> <title>`.
    pub fn rename(&self, cwd: &Path, thread: &str, title: &str) -> anyhow::Result<()> {
        self.threads(cwd, &["rename", thread, "--", title])
    }

    /// `cedia threads delete <id>`: the thread, its sub-threads and their session logs.
    pub fn delete(&self, cwd: &Path, thread: &str) -> anyhow::Result<()> {
        self.threads(cwd, &["delete", thread])
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

    /// `cedia threads undo|redo <id> --json [--force]`: the files of the thread's last turn that
    /// changed files it moved, and the ones it left because they changed since.
    pub fn restore(
        &self,
        cwd: &Path,
        thread: &str,
        redo: bool,
        force: bool,
    ) -> anyhow::Result<Restored> {
        let mut args = vec![if redo { "redo" } else { "undo" }, thread, "--json"];
        if force {
            args.push("--force");
        }
        let output = self.threads_output(cwd, &args)?;
        serde_json::from_slice(&output).context("reading cedia threads undo --json")
    }

    /// `cedia threads rewind <id> <n> --json [--force]`: what it put back, or why it did not.
    /// cedia prints a refusal too, and exits 1.
    pub fn rewind(
        &self,
        cwd: &Path,
        thread: &str,
        n: u32,
        force: bool,
    ) -> anyhow::Result<RewindResult> {
        let n = n.to_string();
        let mut args = vec!["rewind", thread, n.as_str(), "--json"];
        if force {
            args.push("--force");
        }
        let output = self
            .command(cwd)
            .args(["threads", "--cwd"])
            .arg(cwd)
            .args(&args)
            .output()
            .with_context(|| format!("running {}", self.bin.display()))?;
        serde_json::from_slice(&output.stdout).with_context(|| {
            format!(
                "cedia threads rewind exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )
        })
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
