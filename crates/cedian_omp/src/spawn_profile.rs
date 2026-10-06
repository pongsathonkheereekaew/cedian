//! `SpawnProfile`: the ONE builder for the `omp --mode rpc-ui` child (ADR-0020).
//!
//! Spawn and respawn both go through [`SpawnProfile::prepare`]. It produces the
//! exact argv, the scrubbed environment (ADR-0015 allow-list) and writes the
//! cedian `--config` overlay into the session dir (never the user's `.omp/`,
//! §77). Any failure fails closed: there is no bare-spawn fallback.
//!
//! Precedence (verified against pinned OMP `config/settings.ts`): global <
//! project < `--config` overlay < runtime overrides (`--approval-mode`). Scalars
//! and arrays in the overlay replace project values; `tools.approval` is a
//! deep-merged record, so every exec-tier tool cedian cares about is pinned by
//! name here — a project `tools.approval.bash: allow` must not survive.

use crate::OmpError;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// OMP `tools.approvalMode` values cedian may pass. `yolo` is not representable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalMode {
    /// Read + in-workspace write tiers run; exec tier prompts (ADR-0023 default).
    #[default]
    Write,
    /// Only read tier runs; write and exec prompt.
    AlwaysAsk,
}

impl ApprovalMode {
    /// Parse an OMP mode string. `yolo` (and anything else) is rejected.
    pub fn parse(s: &str) -> Result<Self, OmpError> {
        match s {
            "write" => Ok(Self::Write),
            "always-ask" => Ok(Self::AlwaysAsk),
            other => Err(OmpError::InvalidSpawnProfile(format!(
                "approval mode {other:?} not allowed (want write|always-ask)"
            ))),
        }
    }

    /// OMP wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::AlwaysAsk => "always-ask",
        }
    }
}

/// OMP `tools.approval.<tool>` / `bash.patterns[].approval` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    Allow,
    Prompt,
    Deny,
}

impl ToolPolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Prompt => "prompt",
            Self::Deny => "deny",
        }
    }
}

/// One ordered `bash.patterns` rule (OMP supports only `*` wildcards).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashRule {
    pub pattern: String,
    pub approval: ToolPolicy,
}

/// Pure exec-tier OMP tools pinned in every overlay. Arg-dependent tools
/// (`gh`, `debug`, `lsp`) are left to the mode: a user policy outranks the
/// mode, so pinning them would prompt on their read-tier calls too.
pub const EXEC_TOOLS: &[&str] = &["bash", "eval", "browser", "task", "vibe_spawn", "vibe_send"];

/// The policy half of the profile, mapped from cedian settings by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnPolicy {
    pub approval_mode: ApprovalMode,
    /// Per-tool overrides layered over the `EXEC_TOOLS` → `prompt` floor.
    pub tool_policies: BTreeMap<String, ToolPolicy>,
    pub bash_patterns: Vec<BashRule>,
    /// Host tools cedian serves over RPC. OMP gives them no tier (so `exec`,
    /// which `write` mode prompts for); cedian gates them itself at its own
    /// gate (ADR-0012), so the overlay allows exactly these names. A
    /// `tool_policies` entry still overrides (a `deny` stays a `deny`).
    pub host_tools: BTreeSet<String>,
}

impl Default for SpawnPolicy {
    fn default() -> Self {
        Self {
            approval_mode: ApprovalMode::Write,
            tool_policies: BTreeMap::new(),
            bash_patterns: Vec::new(),
            host_tools: BTreeSet::new(),
        }
    }
}

impl SpawnPolicy {
    /// The `tools.approval` record: exec floor, host-tool allows, then caller
    /// overrides.
    fn approval_record(&self) -> Result<Map<String, Value>, OmpError> {
        let mut record = Map::new();
        for tool in EXEC_TOOLS {
            record.insert((*tool).to_string(), json!(ToolPolicy::Prompt.as_str()));
        }
        for tool in &self.host_tools {
            check_tool_name(tool)?;
            if EXEC_TOOLS.contains(&tool.as_str()) {
                return Err(OmpError::InvalidSpawnProfile(format!(
                    "host tool {tool:?} shadows an OMP exec tool"
                )));
            }
            record.insert(tool.clone(), json!(ToolPolicy::Allow.as_str()));
        }
        for (tool, policy) in &self.tool_policies {
            // The eval gate: eval runs Python/JS outside `bash.patterns`, so it
            // is never auto-approved (ADR-0020 §3).
            if tool == "eval" && *policy == ToolPolicy::Allow {
                return Err(OmpError::InvalidSpawnProfile(
                    "tools.approval.eval: allow is forbidden (eval gate)".to_string(),
                ));
            }
            check_tool_name(tool)?;
            record.insert(tool.clone(), json!(policy.as_str()));
        }
        Ok(record)
    }

    /// The overlay document. JSON is valid YAML 1.2, which OMP's loader parses.
    pub fn overlay(&self) -> Result<Value, OmpError> {
        let patterns: Vec<Value> = self
            .bash_patterns
            .iter()
            .map(|rule| {
                if rule.pattern.trim().is_empty() || rule.pattern.chars().any(char::is_control) {
                    return Err(OmpError::InvalidSpawnProfile(format!(
                        "bad bash pattern: {:?}",
                        rule.pattern
                    )));
                }
                Ok(json!({"match": rule.pattern, "approval": rule.approval.as_str()}))
            })
            .collect::<Result<_, _>>()?;
        Ok(json!({
            // Until ADR-0008's atomic landing (driver + Seatbelt + bypass test).
            "computer": {"enabled": false},
            "tools": {
                "approvalMode": self.approval_mode.as_str(),
                "approval": Value::Object(self.approval_record()?),
            },
            "bash": {
                "patterns": patterns,
                "allowCompoundCommands": false,
            },
        }))
    }
}

fn check_tool_name(tool: &str) -> Result<(), OmpError> {
    if tool.is_empty() || !tool.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(OmpError::InvalidSpawnProfile(format!(
            "bad tool name in policy: {tool:?}"
        )));
    }
    Ok(())
}

/// Inputs for one OMP child. The dedupe key is `{binary_path, session_dir, cwd}`.
#[derive(Debug, Clone)]
pub struct SpawnProfile {
    /// Absolute path to the `omp` binary.
    pub binary_path: PathBuf,
    /// `--session-dir` for this workspace; the overlay is written inside it.
    pub session_dir: PathBuf,
    /// Workspace root the child runs in.
    pub cwd: PathBuf,
    pub policy: SpawnPolicy,
}

/// What [`SpawnProfile::prepare`] hands the process launcher.
#[derive(Debug, Clone)]
pub struct SpawnPlan {
    /// Exact argv, `argv[0]` = binary.
    pub argv: Vec<String>,
    /// The complete child environment (the launcher clears the rest).
    pub env: Vec<(String, String)>,
    /// Dedupe identity: `{binary_path, session_dir, cwd}`.
    pub dedupe_key: String,
    /// Where the overlay was written.
    pub overlay_path: PathBuf,
}

/// File name of the generated overlay inside the session dir.
pub const OVERLAY_FILE: &str = "cedian-overlay.yml";

const MAX_ARGS: usize = 64;
const MAX_ARG_BYTES: usize = 8 * 1024;

/// Environment variables the OMP child may inherit, by exact name. Allow-list,
/// never deny-list (ADR-0015/0020): no `SSH_AUTH_SOCK`, no cloud/forge
/// credentials. Provider auth comes from OMP's own auth store under `HOME`.
pub const ENV_ALLOW: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "TERM",
    "TZ",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
];

/// Filter `vars` down to [`ENV_ALLOW`], in allow-list order.
pub fn scrub_env(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let vars: BTreeMap<String, String> = vars.into_iter().collect();
    ENV_ALLOW
        .iter()
        .filter_map(|name| vars.get(*name).map(|v| ((*name).to_string(), v.clone())))
        .collect()
}

fn check_path(label: &str, path: &Path) -> Result<String, OmpError> {
    if !path.is_absolute() {
        return Err(OmpError::InvalidSpawnProfile(format!(
            "{label} must be absolute: {}",
            path.display()
        )));
    }
    let s = path.to_string_lossy();
    if s.len() > MAX_ARG_BYTES || s.chars().any(|c| c.is_control() || c == '\'') {
        return Err(OmpError::InvalidSpawnProfile(format!(
            "{label} rejected: control chars or too long"
        )));
    }
    Ok(s.into_owned())
}

impl SpawnProfile {
    /// Validate and build the argv + env without touching disk.
    pub fn plan(
        &self,
        parent_env: impl IntoIterator<Item = (String, String)>,
    ) -> Result<SpawnPlan, OmpError> {
        let binary = check_path("binary_path", &self.binary_path)?;
        let session_dir = check_path("session_dir", &self.session_dir)?;
        let cwd = check_path("cwd", &self.cwd)?;
        // Validate the overlay before anything is spawned (fail closed).
        self.policy.overlay()?;
        let overlay_path = self.session_dir.join(OVERLAY_FILE);
        let overlay = check_path("overlay", &overlay_path)?;
        let argv: Vec<String> = [
            binary.as_str(),
            "--mode",
            "rpc-ui",
            "--session-dir",
            session_dir.as_str(),
            "--cwd",
            cwd.as_str(),
            "--approval-mode",
            self.policy.approval_mode.as_str(),
            "--config",
            overlay.as_str(),
        ]
        .map(str::to_string)
        .into();
        if argv.len() > MAX_ARGS {
            return Err(OmpError::InvalidSpawnProfile(
                "argv exceeds MAX_ARGS".to_string(),
            ));
        }
        let dedupe_key = format!("{binary}:{session_dir}:{cwd}");
        Ok(SpawnPlan {
            argv,
            env: scrub_env(parent_env),
            dedupe_key,
            overlay_path,
        })
    }

    /// [`Self::plan`] against the current process env, then write the overlay.
    /// Any error means the runtime must not start.
    pub fn prepare(&self) -> Result<SpawnPlan, OmpError> {
        let plan = self.plan(std::env::vars())?;
        let body = serde_json::to_string_pretty(&self.policy.overlay()?)
            .map_err(|e| OmpError::InvalidSpawnProfile(format!("overlay encode: {e}")))?;
        std::fs::create_dir_all(&self.session_dir)
            .and_then(|()| std::fs::write(&plan.overlay_path, body))
            .map_err(|e| {
                OmpError::InvalidSpawnProfile(format!(
                    "overlay write {}: {e}",
                    plan.overlay_path.display()
                ))
            })?;
        Ok(plan)
    }
}

/// Resolve a bare binary name against `PATH` to an absolute path (dev lane).
pub fn resolve_on_path(name: &str, path_var: Option<&str>) -> Result<PathBuf, OmpError> {
    if Path::new(name).is_absolute() {
        return Ok(PathBuf::from(name));
    }
    if name.contains('/') {
        return Err(OmpError::InvalidSpawnProfile(format!(
            "binary must be a bare name or absolute: {name}"
        )));
    }
    path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| OmpError::Spawn(format!("{name} not found on PATH")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> SpawnProfile {
        SpawnProfile {
            binary_path: PathBuf::from("/Applications/cedian.app/Contents/Resources/omp"),
            session_dir: PathBuf::from("/tmp/t1"),
            cwd: PathBuf::from("/Users/u/work"),
            policy: SpawnPolicy::default(),
        }
    }

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn golden_argv() {
        let plan = profile().plan(Vec::new()).unwrap();
        assert_eq!(
            plan.argv,
            [
                "/Applications/cedian.app/Contents/Resources/omp",
                "--mode",
                "rpc-ui",
                "--session-dir",
                "/tmp/t1",
                "--cwd",
                "/Users/u/work",
                "--approval-mode",
                "write",
                "--config",
                "/tmp/t1/cedian-overlay.yml",
            ]
        );
        assert!(!plan
            .argv
            .iter()
            .any(|a| a == "yolo" || a == "--auto-approve"));
    }

    #[test]
    fn golden_overlay() {
        let mut policy = SpawnPolicy::default();
        policy.bash_patterns.push(BashRule {
            pattern: "cargo test*".to_string(),
            approval: ToolPolicy::Allow,
        });
        assert_eq!(
            policy.overlay().unwrap(),
            json!({
                "computer": {"enabled": false},
                "tools": {
                    "approvalMode": "write",
                    "approval": {
                        "bash": "prompt", "eval": "prompt", "browser": "prompt",
                        "task": "prompt", "vibe_spawn": "prompt", "vibe_send": "prompt",
                    },
                },
                "bash": {
                    "patterns": [{"match": "cargo test*", "approval": "allow"}],
                    "allowCompoundCommands": false,
                },
            })
        );
    }

    #[test]
    fn env_is_exact_allow_list() {
        let scrubbed = scrub_env(env(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/Users/u"),
            ("SSH_AUTH_SOCK", "/tmp/agent"),
            ("AWS_SECRET_ACCESS_KEY", "x"),
            ("GH_TOKEN", "x"),
            ("GITHUB_TOKEN", "x"),
            ("OPENAI_API_KEY", "x"),
        ]));
        assert_eq!(scrubbed, env(&[("PATH", "/usr/bin"), ("HOME", "/Users/u")]));
    }

    #[test]
    fn yolo_rejected() {
        assert!(ApprovalMode::parse("yolo").is_err());
        assert_eq!(ApprovalMode::parse("write").unwrap(), ApprovalMode::Write);
    }

    #[test]
    fn eval_allow_rejected() {
        let mut p = profile();
        p.policy
            .tool_policies
            .insert("eval".to_string(), ToolPolicy::Allow);
        assert!(p.plan(Vec::new()).is_err());
    }

    #[test]
    fn deny_overrides_exec_floor() {
        let mut policy = SpawnPolicy::default();
        policy
            .tool_policies
            .insert("eval".to_string(), ToolPolicy::Deny);
        assert_eq!(
            policy.overlay().unwrap()["tools"]["approval"]["eval"],
            "deny"
        );
    }

    #[test]
    fn host_tools_allowed_but_never_shadow_exec_tools() {
        let mut policy = SpawnPolicy::default();
        policy.host_tools.insert("cedian_apply_edit".to_string());
        assert_eq!(
            policy.overlay().unwrap()["tools"]["approval"]["cedian_apply_edit"],
            "allow"
        );
        policy.host_tools.insert("bash".to_string());
        assert!(policy.overlay().is_err());
    }

    #[test]
    fn relative_and_control_paths_rejected() {
        let mut p = profile();
        p.binary_path = PathBuf::from("omp");
        assert!(p.plan(Vec::new()).is_err());
        let mut p = profile();
        p.session_dir = PathBuf::from("/tmp/a\nb");
        assert!(p.plan(Vec::new()).is_err());
    }

    #[test]
    fn distinct_workspaces_distinct_keys() {
        let a = profile();
        let mut b = profile();
        b.cwd = PathBuf::from("/Users/u/other");
        assert_ne!(
            a.plan(Vec::new()).unwrap().dedupe_key,
            b.plan(Vec::new()).unwrap().dedupe_key
        );
    }

    #[test]
    fn missing_overlay_dir_fails_closed() {
        // A session dir under a regular file cannot hold the overlay.
        let file = std::env::temp_dir().join(format!("cedian-p1-file-{}", std::process::id()));
        std::fs::write(&file, "x").unwrap();
        let mut p = profile();
        p.session_dir = file.join("sessions");
        assert!(p.prepare().is_err());
        std::fs::remove_file(&file).unwrap();
    }

    #[test]
    fn prepare_writes_overlay_omp_can_read() {
        let dir = std::env::temp_dir().join(format!("cedian-p1-ok-{}", std::process::id()));
        let mut p = profile();
        p.session_dir = dir.clone();
        let plan = p.prepare().unwrap();
        let raw = std::fs::read_to_string(&plan.overlay_path).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["computer"]["enabled"], false);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_on_path_finds_absolute() {
        let dir = std::env::temp_dir().join(format!("cedian-p1-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("omp"), "").unwrap();
        let path_var = format!("relative/bin:{}", dir.display());
        assert_eq!(
            resolve_on_path("omp", Some(&path_var)).unwrap(),
            dir.join("omp")
        );
        assert!(resolve_on_path("./omp", Some(&path_var)).is_err());
        assert!(resolve_on_path("nope-omp", Some(&path_var)).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
