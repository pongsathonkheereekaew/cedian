//! How cedian starts OMP for a person at the keyboard: the user's
//! `cedian.toml` mapped onto the spawn profile (ADR-0020, ADR-0035), and the
//! ADR-0041 pin of OMP's own `tools.approval` allows. The CLI and the app
//! both start OMP through here.

use crate::{Policy, Settings, Verdict};
use cedian_omp::{ApprovalMode, Approvals, SpawnPolicy, ToolPolicy};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Whether `cedian_worktree_request` is registered. It makes a tree and a
/// branch with no dialog of its own, so the two verdicts that do not refuse
/// project writes register it (ADR-0012 strict-wins). What answers it
/// differs: under `allow` the caller's gate always allows, under `ask` a
/// person answers, and under `deny` nobody could, so it is not registered.
pub fn registers_worktree_request(settings: &Settings) -> bool {
    matches!(
        settings.permissions.project_write,
        Verdict::Allow | Verdict::Ask
    )
}

/// Map `[permissions]` onto the OMP spawn policy (ADR-0020). Under the
/// default policy it only tightens: `dangerous = allow` still leaves the exec
/// floor at `prompt` (strict-wins, ADR-0012), and yolo is unrepresentable.
/// Under `policy = "omp"` (ADR-0035) OMP's own config decides approvals;
/// cedian's denies still apply. `host_tools` is the whole host-tool set the
/// caller registers (ADR-0004), so the overlay allows exactly those names.
pub fn spawn_policy(
    settings: &Settings,
    policy_source: Policy,
    host_tools: &[&str],
) -> SpawnPolicy {
    let mut policy = SpawnPolicy::default();
    if policy_source == Policy::Omp {
        policy.approvals = Approvals::Omp;
    }
    for tool in host_tools {
        policy.host_tools.insert((*tool).to_string());
    }
    if settings.permissions.project_write != Verdict::Allow && policy.approvals != Approvals::Omp {
        policy.approvals = Approvals::Cedian(ApprovalMode::AlwaysAsk);
    }
    let mut deny = |tools: &[&str]| {
        for tool in tools {
            policy
                .tool_policies
                .insert((*tool).to_string(), ToolPolicy::Deny);
        }
    };
    if settings.permissions.project_write == Verdict::Deny {
        deny(&["edit", "write", "ast_edit"]);
    }
    if settings.permissions.dangerous == Verdict::Deny {
        deny(cedian_omp::spawn_profile::EXEC_TOOLS);
    }
    // The Safe actions tier (ADR-0084 decision 6): `ask` makes every read
    // ask, inside the project too; `deny` refuses reads. Under the opt-in
    // the overlay keeps only the deny, as for the other tiers.
    let safe = match settings.permissions.safe {
        Verdict::Allow => None,
        Verdict::Ask => Some(ToolPolicy::Prompt),
        Verdict::Deny => Some(ToolPolicy::Deny),
    };
    if let Some(verdict) = safe {
        for tool in cedian_omp::READ_TOOLS {
            policy.tool_policies.insert((*tool).to_string(), verdict);
        }
    }
    policy
}

/// Where `workdir`'s agent reads without asking (ADR-0084): the workspace,
/// the `read_folders` of its key in the user's file, and the protected
/// names and dirs reads are refused, from the same roots writes are
/// (ADR-0012, ADR-0044, ADR-0078). The notes say which folders were
/// dropped and why.
pub fn read_scope(
    settings: &Settings,
    workdir: &Path,
    protected: &cedian_omp::ProtectedRoots,
) -> (cedian_omp::ReadScope, Vec<String>) {
    let folders = settings.read_folders_for(workdir);
    let scope = cedian_omp::ReadScope {
        roots: vec![protected.workspace.clone()],
        allow: folders.folders,
        protected_names: READ_PROTECTED_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        protected_paths: protected.session_dirs.clone(),
        outside_asks: true,
    };
    (scope, folders.notes)
}

/// Directory names the agent may not read under the project or an allowed
/// folder (ADR-0084 decision 3): git's store, cedian's old in-workspace
/// state and the agent's own project config.
pub const READ_PROTECTED_NAMES: &[&str] = &[".git", ".cedian", ".omp"];

/// What pstack gives `workdir`'s agent (ADR-0079): the bundled skills and,
/// unless the person turned it off, the mandate naming this machine's
/// bundle and override sheet. `None` when pstack is off for `workdir`.
/// Only for a person's session: a reviewer's profile never calls this.
pub fn pstack_profile(settings: &Settings, workdir: &Path) -> Result<PstackLaunch, String> {
    let sheet = crate::pstack::sheet_path()?;
    let agent_dir = cedian_omp::AgentDirs::from_env()
        .map_err(|error| error.to_string())?
        .config;
    pstack_profile_for(
        settings,
        workdir,
        std::env::current_exe().ok().as_deref(),
        &sheet,
        &agent_dir,
    )
}

pub fn pstack_profile_for(
    settings: &Settings,
    workdir: &Path,
    exe: Option<&Path>,
    sheet: &Path,
    agent_dir: &Path,
) -> Result<PstackLaunch, String> {
    let choice = settings.pstack_for(workdir);
    if !choice.enabled {
        return Ok(PstackLaunch {
            profile: None,
            notes: choice.notes,
        });
    }
    let bundle = exe
        .and_then(crate::pstack::PstackBundle::for_exe)
        .or_else(debug_build_source_tree_pstack)
        .ok_or_else(|| {
            "pstack is on but the app's Resources have no bundled pstack; turn it off in Settings > Methodology or reinstall the app".to_string()
        })?;
    let mut notes = choice.notes;
    let mandate = if settings.pstack_mandate {
        let mut mandate = bundle.mandate(sheet)?;
        // OMP reads APPEND_SYSTEM.md only when no --append-system-prompt is
        // passed, and the mandate is passed that way, so carry the person's
        // text along after it. OMP warns and goes on when it cannot read one,
        // and so does cedian: a repository's file never refuses the spawn.
        for path in append_system_files(workdir, agent_dir) {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    mandate.push_str("\n\n");
                    mandate.push_str(text.trim_end());
                }
                Err(error) => notes.push(format!("{} skipped: {error}", path.display())),
            }
        }
        Some(mandate)
    } else {
        None
    };
    Ok(PstackLaunch {
        profile: Some(cedian_omp::PstackProfile {
            skills_dir: bundle.skills(),
            mandate,
        }),
        notes,
    })
}

#[derive(Debug, PartialEq)]
pub struct PstackLaunch {
    pub profile: Option<cedian_omp::PstackProfile>,
    pub notes: Vec<String>,
}

/// The APPEND_SYSTEM.md files OMP would find: the project's (first of
/// `.omp`, `.claude`, `.codex`, `.gemini` under `workdir`), then the agent
/// dir's.
fn append_system_files(workdir: &Path, agent_dir: &Path) -> Vec<PathBuf> {
    let project = [".omp", ".claude", ".codex", ".gemini"]
        .iter()
        .map(|base| workdir.join(base).join("APPEND_SYSTEM.md"))
        .find(|path| path.is_file());
    let user = Some(agent_dir.join("APPEND_SYSTEM.md")).filter(|path| path.is_file());
    project.into_iter().chain(user).collect()
}

fn debug_build_source_tree_pstack() -> Option<crate::pstack::PstackBundle> {
    cfg!(debug_assertions)
        .then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/cedian"))
        .and_then(|exe| crate::pstack::PstackBundle::for_exe(&exe))
}

/// The pin file (`vendor/omp-revision.json`).
fn pin() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../vendor/omp-revision.json"))
        .expect("vendor/omp-revision.json is JSON")
}

/// The OMP version cedian is pinned to (`vendor/omp-revision.json`).
pub fn pinned_omp_version() -> String {
    pin()["spikeVerified"]["ompVersion"]
        .as_str()
        .expect("vendor/omp-revision.json names spikeVerified.ompVersion")
        .to_string()
}

/// The OMP fork commit the app's bundled build must come from
/// (`vendor/omp-revision.json`, the `vendor/cedian-agent` submodule).
pub fn pinned_omp_commit() -> String {
    pin()["commit"]
        .as_str()
        .expect("vendor/omp-revision.json names commit")
        .to_string()
}

/// The OMP binary a launch runs, and the warning to show when it is not
/// the pinned version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmpChoice {
    pub binary: PathBuf,
    pub warning: Option<String>,
}

/// The OMP binary (ADR-0075 decision 5, ADR-0077). A set
/// `CEDIAN_OMP_BINARY` runs as given, asked `--version` once only for the
/// warning. Otherwise the fork build the app ships, verified against its
/// stamp; else the launch is refused. The person's own `omp` (PATH,
/// `~/.local/bin`) is never run. Blocking: hashes the binary, so call it off
/// the UI thread.
pub fn omp_binary() -> Result<OmpChoice, String> {
    omp_binary_for(std::env::current_exe().ok().as_deref())
}

/// The resolution, from the executable a bundle lookup starts at
/// ([`omp_binary`] passes the running one; tests drive fake layouts, which
/// `current_exe` cannot be moved to).
pub fn omp_binary_for(exe: Option<&Path>) -> Result<OmpChoice, String> {
    let set = std::env::var_os("CEDIAN_OMP_BINARY");
    if let Some(binary) = binary_override(HONOR_BINARY_OVERRIDE, set) {
        let version = cedian_omp::omp_version(&binary, Duration::from_secs(3));
        let warning = off_pin(&pinned_omp_version(), &binary, version);
        return Ok(OmpChoice { binary, warning });
    }
    let binary = exe.and_then(bundled_omp).ok_or_else(|| {
        format!(
            "no bundled cedian agent next to {}: build it with script/cedian-omp-build (or, in a dev build, set CEDIAN_OMP_BINARY); cedian does not run the omp on PATH",
            exe.map_or("the app".into(), |exe| exe.display().to_string())
        )
    })?;
    verify_stamp(&binary)?;
    Ok(OmpChoice {
        binary,
        warning: None,
    })
}

/// Whether `CEDIAN_OMP_BINARY` is honored: in debug builds only (ADR-0078
/// decision 5). `debug_assertions` rather than the release channel, because
/// the channel is a runtime value a person can change with an env var or
/// file, while this is fixed when the binary is compiled: every `cargo test`
/// and dev build honors it, and the release build the bundle ships cannot.
const HONOR_BINARY_OVERRIDE: bool = cfg!(debug_assertions);

fn binary_override(honor: bool, set: Option<std::ffi::OsString>) -> Option<PathBuf> {
    set.filter(|_| honor).map(PathBuf::from)
}

/// `<binary>.stamp` beside the binary: `omp.stamp` in a dev build,
/// `cedian-agent.stamp` in the bundle.
fn stamp_path(binary: &Path) -> PathBuf {
    let mut name = binary.file_name().unwrap_or_default().to_os_string();
    name.push(".stamp");
    binary.with_file_name(name)
}

/// Where the app's agent sits: `Contents/Resources/cedian-agent` when `exe` is in a
/// bundle's `Contents/MacOS`, else `<dir>/omp/omp` for the first of `exe`'s
/// three nearest ancestors that has one, which finds `target/omp/omp` from
/// `target/<profile>/cedian` and from a test's `target/<profile>/deps/<t>`.
fn bundled_omp(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()? == "MacOS" && macos.parent()?.file_name()? == "Contents" {
        let sidecar = macos.parent()?.join("Resources/cedian-agent");
        return sidecar.is_file().then_some(sidecar);
    }
    exe.ancestors()
        .skip(1)
        .take(3)
        .map(|dir| dir.join("omp/omp"))
        .find(|omp| omp.is_file())
}

/// The build's `omp.stamp`, written by `script/cedian-omp-build`, must name
/// the pinned fork commit and the binary's sha256: `omp --version` reads the
/// same for every build of a version, so it cannot tell the fork's from
/// upstream's.
fn verify_stamp(binary: &Path) -> Result<(), String> {
    use sha2::{Digest as _, Sha256};
    let rebuild = "rebuild it with script/cedian-omp-build";
    let path = stamp_path(binary);
    let stamp: serde_json::Value = std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
        .map_err(|e| format!("{}: {e}; {rebuild}", path.display()))?;
    let pinned = pinned_omp_commit();
    let commit = stamp["commit"].as_str().unwrap_or("none");
    if commit != pinned {
        return Err(format!(
            "{} was built from cedian agent commit {commit}, but cedian is pinned to {pinned}; {rebuild}",
            binary.display()
        ));
    }
    let bytes = std::fs::read(binary).map_err(|e| format!("{}: {e}", binary.display()))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if stamp["sha256"].as_str() != Some(actual.as_str()) {
        return Err(format!(
            "{}'s sha256 {actual} is not the one its stamp records; {rebuild}",
            binary.display()
        ));
    }
    Ok(())
}

fn off_pin(
    pinned: &str,
    binary: &Path,
    version: Result<String, cedian_omp::OmpError>,
) -> Option<String> {
    let found = match version {
        Ok(version) if version == pinned => return None,
        Ok(version) => format!("cedian agent {version}"),
        Err(e) => format!("a cedian agent whose version cannot be read ({e})"),
    };
    Some(format!(
        "cedian is pinned to cedian agent {pinned} but found {found} at {}; it runs that one",
        binary.display()
    ))
}

/// The opt-in badge (ADR-0035 decision 4): OMP's effective approval mode
/// and `computer`, and whether the workspace's own `.omp/config.yml` set them
/// (OMP's answer in the workspace differs from its answer in an empty dir).
/// The CLI prints it; the panel shows it. Blocking: runs `omp config get`.
pub fn omp_policy_badge(binary: &Path, workdir: &Path) -> String {
    let empty = std::env::temp_dir().join(format!("cedian-omp-global-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&empty);
    let get = |cwd: &Path, key: &str| {
        cedian_omp::omp_config_get(binary, cwd, key, Duration::from_secs(10)).ok()
    };
    let describe = |key: &str, show: &dyn Fn(&serde_json::Value) -> String| {
        let here = get(workdir, key);
        let from = if here.is_some() && here != get(&empty, key) {
            " from the project's .omp/config.yml"
        } else {
            ""
        };
        match here {
            Some(v) => format!("{}{from}", show(&v)),
            None => "unknown, assume on".to_string(),
        }
    };
    let mode = describe("tools.approvalMode", &|v| {
        v.as_str().unwrap_or("unknown").to_string()
    });
    let computer = describe("computer.enabled", &|v| {
        if v.as_bool() == Some(true) {
            "on"
        } else {
            "off"
        }
        .to_string()
    });
    let _ = std::fs::remove_dir(&empty);
    format!(
        "◆ agent policy — approvals and computer from your agent config (approvalMode: {mode}; computer: {computer})"
    )
}

/// Tools OMP's merged config sets to `allow` in `workdir` (ADR-0041
/// decision 2). Fails closed: without the record the default profile cannot
/// pin them, so the spawn is refused.
pub fn config_allows(binary: &Path, workdir: &Path) -> Result<BTreeSet<String>, String> {
    let record =
        cedian_omp::omp_config_get(binary, workdir, "tools.approval", Duration::from_secs(10))
            .map_err(|e| {
                format!("cannot read cedian agent's tools.approval, so cannot pin it: {e}")
            })?;
    let record = record
        .as_object()
        .ok_or("cedian agent's tools.approval is not a record")?;
    Ok(record
        .iter()
        .filter(|(_, v)| v.as_str() == Some("allow"))
        .map(|(k, _)| k.clone())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0078 decision 5: a shipped (release) app ignores
    /// `CEDIAN_OMP_BINARY`; dev and test builds honor it.
    #[test]
    fn release_build_ignores_the_binary_override() {
        let set = Some(std::ffi::OsString::from("/tmp/fake-omp"));
        assert_eq!(binary_override(false, set.clone()), None);
        assert_eq!(
            binary_override(true, set),
            Some(PathBuf::from("/tmp/fake-omp"))
        );
        assert_eq!(binary_override(true, None), None);
        assert!(HONOR_BINARY_OVERRIDE, "test builds are debug builds");
    }

    /// ADR-0078 decision 4: the bundle carries the agent as
    /// `Contents/Resources/cedian-agent`, with `cedian-agent.stamp` beside it.
    #[test]
    fn bundled_agent_is_named_cedian_agent() {
        let dir = tempfile::tempdir().unwrap();
        let contents = dir.path().join("cedian.app/Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        std::fs::create_dir_all(contents.join("Resources")).unwrap();
        let exe = contents.join("MacOS/cedian");
        assert_eq!(bundled_omp(&exe), None);
        std::fs::write(contents.join("Resources/cedian-agent"), "").unwrap();
        assert_eq!(
            bundled_omp(&exe),
            Some(contents.join("Resources/cedian-agent"))
        );
        assert_eq!(
            stamp_path(&contents.join("Resources/cedian-agent")),
            contents.join("Resources/cedian-agent.stamp")
        );
        assert_eq!(
            stamp_path(Path::new("target/omp/omp")),
            PathBuf::from("target/omp/omp.stamp")
        );
    }

    #[test]
    fn opt_in_hands_approvals_to_omp_but_keeps_denies() {
        let mut settings = Settings::default();
        let default = spawn_policy(&settings, Policy::Cedian, &[]);
        assert_eq!(default.approvals, Approvals::Cedian(ApprovalMode::Write));
        assert_eq!(
            spawn_policy(&settings, Policy::Omp, &[]).approvals,
            Approvals::Omp
        );
        settings.permissions.project_write = Verdict::Deny;
        let opted = spawn_policy(&settings, Policy::Omp, &[]);
        assert_eq!(opted.approvals, Approvals::Omp);
        assert_eq!(opted.tool_policies.get("write"), Some(&ToolPolicy::Deny));
        assert_eq!(
            spawn_policy(&settings, Policy::Cedian, &[]).approvals,
            Approvals::Cedian(ApprovalMode::AlwaysAsk)
        );
    }

    #[test]
    fn the_spawn_gets_the_bundled_pstack_unless_it_is_off() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let exe = repo.join("target/debug/cedian");
        let sheet = Path::new("/c/agent/pstack-models.md");
        let workdir = std::env::temp_dir();
        let agent_dir = workdir.join("no-such-cedian-agent-dir");
        let mut settings = Settings::default();

        let on = pstack_profile_for(&settings, &workdir, Some(&exe), sheet, &agent_dir)
            .unwrap()
            .profile
            .expect("on by default");
        assert!(on.skills_dir.ends_with("vendor/pstack/skills"));
        let mandate = on.mandate.expect("mandate on by default");
        assert!(mandate.contains("skill://Poteto Mode"));
        assert!(mandate.contains("/c/agent/pstack-models.md"));
        assert!(!mandate.contains("{{"), "every placeholder filled");
        assert!(!mandate.contains("~/.omp"), "never the person's OMP");

        settings.pstack_mandate = false;
        let skills_only = pstack_profile_for(&settings, &workdir, Some(&exe), sheet, &agent_dir)
            .unwrap()
            .profile
            .unwrap();
        assert_eq!(skills_only.mandate, None);

        settings.pstack = false;
        assert_eq!(
            pstack_profile_for(&settings, &workdir, Some(&exe), sheet, &agent_dir)
                .unwrap()
                .profile,
            None,
            "off runs the plain agent"
        );

        settings.pstack = true;
        assert!(
            pstack_profile_for(
                &settings,
                &workdir,
                Some(Path::new("/nowhere/cedian")),
                sheet,
                &agent_dir
            )
            .is_ok_and(|launch| launch.profile.is_some()),
            "a debug build falls back to its source tree"
        );
    }

    #[test]
    fn the_mandate_carries_the_persons_append_system_text_after_it() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let exe = repo.join("target/debug/cedian");
        let sheet = Path::new("/c/agent/pstack-models.md");
        let workdir = tempfile::tempdir().unwrap();
        let agent_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(workdir.path().join(".claude")).unwrap();
        std::fs::write(
            workdir.path().join(".claude/APPEND_SYSTEM.md"),
            "Project sentence.\n",
        )
        .unwrap();
        std::fs::write(
            agent_dir.path().join("APPEND_SYSTEM.md"),
            "Agent dir sentence.\n",
        )
        .unwrap();

        let mandate = pstack_profile_for(
            &Settings::default(),
            workdir.path(),
            Some(&exe),
            sheet,
            agent_dir.path(),
        )
        .unwrap()
        .profile
        .unwrap()
        .mandate
        .unwrap();
        let mandate_at = mandate.find("skill://Poteto Mode").unwrap();
        let project_at = mandate.find("Project sentence.").unwrap();
        let agent_at = mandate.find("Agent dir sentence.").unwrap();
        assert!(mandate_at < project_at && project_at < agent_at);
    }

    #[test]
    fn an_unreadable_append_system_file_is_skipped_with_a_note() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let exe = repo.join("target/debug/cedian");
        let workdir = tempfile::tempdir().unwrap();
        let agent_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(workdir.path().join(".claude")).unwrap();
        std::fs::write(
            workdir.path().join(".claude/APPEND_SYSTEM.md"),
            [0xff, 0xfe, 0x00, 0x80],
        )
        .unwrap();
        std::fs::write(
            agent_dir.path().join("APPEND_SYSTEM.md"),
            "Agent dir sentence.\n",
        )
        .unwrap();

        let launch = pstack_profile_for(
            &Settings::default(),
            workdir.path(),
            Some(&exe),
            Path::new("/c/agent/pstack-models.md"),
            agent_dir.path(),
        )
        .expect("a repository's broken file never refuses the spawn");
        let mandate = launch.profile.unwrap().mandate.unwrap();
        assert!(mandate.contains("Agent dir sentence."));
        assert!(
            launch
                .notes
                .iter()
                .any(|note| note.contains(".claude/APPEND_SYSTEM.md") && note.contains("skipped")),
            "{:?}",
            launch.notes
        );
    }

    #[test]
    fn an_ignored_pstack_project_key_comes_back_as_a_note() {
        let settings = crate::settings::parse_settings(
            "schema = 1\n[projects.\"relative/ws\"]\npstack = false\n",
        )
        .unwrap();
        let workdir = std::env::temp_dir();
        let launch = pstack_profile_for(
            &settings,
            &workdir,
            None,
            Path::new("/c/agent/pstack-models.md"),
            &workdir.join("no-such-cedian-agent-dir"),
        )
        .unwrap();
        assert!(
            launch
                .notes
                .iter()
                .any(|note| note.contains("not an absolute path")),
            "{:?}",
            launch.notes
        );
    }

    #[test]
    fn the_safe_tier_is_written_for_every_read_tool() {
        let mut settings = Settings::default();
        assert!(
            !spawn_policy(&settings, Policy::Cedian, &[])
                .tool_policies
                .contains_key("read"),
            "allow writes nothing: the scope alone decides"
        );
        settings.permissions.safe = Verdict::Ask;
        let asks = spawn_policy(&settings, Policy::Cedian, &[]);
        for tool in cedian_omp::READ_TOOLS {
            assert_eq!(
                asks.tool_policies.get(*tool),
                Some(&ToolPolicy::Prompt),
                "{tool}"
            );
        }
        settings.permissions.safe = Verdict::Deny;
        let denies = spawn_policy(&settings, Policy::Omp, &[]);
        for tool in cedian_omp::READ_TOOLS {
            assert_eq!(
                denies.tool_policies.get(*tool),
                Some(&ToolPolicy::Deny),
                "{tool}"
            );
        }
        let overlay = denies.overlay().unwrap();
        assert_eq!(overlay["tools"]["approval"]["read"], "deny");
        let overlay = spawn_policy(
            &{
                let mut s = Settings::default();
                s.permissions.safe = Verdict::Ask;
                s
            },
            Policy::Omp,
            &[],
        )
        .overlay()
        .unwrap();
        assert!(
            overlay["tools"]["approval"].get("read").is_none(),
            "under the opt-in an ask is OMP's to decide"
        );
    }

    #[test]
    fn the_read_scope_is_the_workspace_the_user_folders_and_the_protected_roots() {
        let dir = crate::test_dir::TestDir::new("read-scope");
        let ws = dir.join("ws");
        let docs = dir.join("docs");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&docs).unwrap();
        let ws = ws.canonicalize().unwrap();
        let docs = docs.canonicalize().unwrap();
        let settings = crate::settings::parse_settings(&format!(
            "schema = 1\n[projects.{:?}]\nread_folders = [{:?}, \"relative/x\"]\n",
            ws.display().to_string(),
            docs.display().to_string()
        ))
        .unwrap();
        let protected = cedian_omp::ProtectedRoots {
            workspace: ws.clone(),
            session_dirs: vec![dir.join("state/omp"), dir.join("home/.omp")],
        };
        let (scope, notes) = read_scope(&settings, &ws, &protected);
        assert_eq!(scope.roots, [ws]);
        assert_eq!(scope.allow, [docs]);
        assert_eq!(scope.protected_names, [".git", ".cedian", ".omp"]);
        assert_eq!(scope.protected_paths, protected.session_dirs);
        assert!(scope.outside_asks);
        assert!(notes.iter().any(|n| n.contains("relative/x")), "{notes:?}");
    }

    #[test]
    fn host_tools_are_exactly_the_ones_named() {
        let policy = spawn_policy(
            &Settings::default(),
            Policy::Cedian,
            &["cedian_worktree_request"],
        );
        assert_eq!(
            policy
                .host_tools
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["cedian_worktree_request"]
        );
    }

    /// The tool is registered unless project writes are refused outright;
    /// who answers it is the caller's gate (ADR-0012).
    #[test]
    fn worktree_request_is_registered_under_allow_and_ask_but_not_deny() {
        let mut settings = Settings::default();
        for (verdict, registered) in [
            (Verdict::Allow, true),
            (Verdict::Ask, true),
            (Verdict::Deny, false),
        ] {
            settings.permissions.project_write = verdict;
            assert_eq!(
                registers_worktree_request(&settings),
                registered,
                "{verdict:?}"
            );
        }
    }
}
