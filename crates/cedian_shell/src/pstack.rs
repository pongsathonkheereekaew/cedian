use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const SHEET_FILE: &str = "pstack-models.md";

pub fn sheet_path() -> Result<PathBuf, String> {
    cedian_omp::AgentDirs::from_env()
        .map(|dirs| dirs.config.join(SHEET_FILE))
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PstackRole {
    Feature,
    BugFix,
    PerfIssue,
    Hillclimb,
    Judgment,
    Strongest,
}

impl PstackRole {
    /// In the order the sheet and the page list them.
    pub const ALL: [Self; 6] = [
        Self::Feature,
        Self::BugFix,
        Self::PerfIssue,
        Self::Hillclimb,
        Self::Judgment,
        Self::Strongest,
    ];

    /// The role's name on the sheet, as pstack's skills look it up.
    pub fn sheet_label(self) -> &'static str {
        match self {
            Self::Feature => "feature, refactoring",
            Self::BugFix => "bug-fix",
            Self::PerfIssue => "perf-issue",
            Self::Hillclimb => "hillclimb",
            Self::Judgment => "judgment and prose",
            Self::Strongest => "strongest judgment",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Feature => "Feature and refactoring",
            Self::BugFix => "Bug fix",
            Self::PerfIssue => "Perf issue",
            Self::Hillclimb => "Hillclimb",
            Self::Judgment => "Judgment and prose",
            Self::Strongest => "Strongest judgment",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Feature => "Code delegates for new behavior and structure changes.",
            Self::BugFix => "Delegates that find and fix a bug.",
            Self::PerfIssue => "One-off slowness fixes against a baseline.",
            Self::Hillclimb => "Sustained improvement of one metric.",
            Self::Judgment => "Reviews, summaries, docs.",
            Self::Strongest => "The hardest design calls and the final verdicts.",
        }
    }

    pub fn folded_under_more_roles(self) -> bool {
        matches!(self, Self::PerfIssue | Self::Hillclimb)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Effort {
    pub const ALL: [Self; 5] = [Self::Low, Self::Medium, Self::High, Self::Xhigh, Self::Max];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Xhigh => "Extra high",
            Self::Max => "Max",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|effort| effort.as_str() == text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DefaultEffort {
    #[default]
    Session,
    Level(Effort),
}

impl DefaultEffort {
    fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Level(effort) => effort.as_str(),
        }
    }
}

/// A role's line: a model (`provider/model`) or an agent role (`@slow`),
/// with an optional effort. Only [`RoleModel::new`] builds one, so its
/// model never breaks the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleModel {
    model: String,
    pub effort: Option<Effort>,
}

impl RoleModel {
    pub fn new(model: &str, effort: Option<Effort>) -> Result<Self, String> {
        let plain = |text: &str| {
            !text.is_empty()
                && !text
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || c == '@')
        };
        let ok = match model.strip_prefix('@') {
            Some(role) => plain(role),
            None => plain(model),
        };
        if !ok {
            return Err(format!("{model:?} is not a model or an agent role"));
        }
        Ok(Self {
            model: model.to_string(),
            effort,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    fn line(&self) -> String {
        match self.effort {
            Some(effort) => format!("{} @{}", self.model, effort.as_str()),
            None => self.model.clone(),
        }
    }

    fn parse(value: &str) -> Option<Self> {
        let (model, effort) = match value.rsplit_once(" @") {
            Some((model, effort)) => (model, Some(Effort::parse(effort)?)),
            None => (value, None),
        };
        Self::new(model, effort).ok()
    }
}

/// The override sheet. A role with no line runs on the agent's default.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PstackModels {
    roles: std::collections::BTreeMap<PstackRole, RoleModel>,
    pub default_effort: DefaultEffort,
}

const SHEET_HEADER: &str = "# pstack model configuration

Per-role model overrides for pstack skills, written by cedian (Settings > Methodology); edits here are replaced on the next change there. A role line names a model (`provider/model`) or an agent role (`@slow`), optionally with a reasoning effort (`@high`); a role with no line uses the skill's default. `default effort` sets the level for a line without one; `session` keeps the session's level.
";

impl PstackModels {
    pub fn get(&self, role: PstackRole) -> Option<&RoleModel> {
        self.roles.get(&role)
    }

    /// `None` is "Agent default": the role's line goes.
    pub fn set(&mut self, role: PstackRole, model: Option<RoleModel>) {
        match model {
            Some(model) => self.roles.insert(role, model),
            None => self.roles.remove(&role),
        };
    }

    pub fn render(&self) -> String {
        let mut text = format!("{SHEET_HEADER}\n");
        for (role, model) in &self.roles {
            text.push_str(&format!("{}: {}\n", role.sheet_label(), model.line()));
        }
        text.push_str(&format!(
            "default effort: {}\n",
            self.default_effort.as_str()
        ));
        text
    }

    /// The sheet in `text`. Lines it does not know are left out, so a hand
    /// edit cannot make it fail; the next write replaces them.
    pub fn parse(text: &str) -> Self {
        let mut sheet = Self::default();
        for line in text.lines() {
            let Some((label, value)) = line.split_once(": ") else {
                continue;
            };
            let value = value.trim();
            if label == "default effort" {
                if let Some(effort) = Effort::parse(value) {
                    sheet.default_effort = DefaultEffort::Level(effort);
                }
                continue;
            }
            let role = PstackRole::ALL
                .into_iter()
                .find(|role| role.sheet_label() == label);
            if let (Some(role), Some(model)) = (role, RoleModel::parse(value)) {
                sheet.roles.insert(role, model);
            }
        }
        sheet
    }

    /// The sheet at `path`; defaults when there is none yet.
    pub fn read(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        }
    }

    /// Replace the sheet at `path` in one rename, so a skill never reads
    /// half of it.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        write_atomically(path, self.render().as_bytes())
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("{} has no folder", path.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)
        .map_err(|e| format!("cannot write in {}: {e}", dir.display()))?;
    std::io::Write::write_all(&mut temp, bytes)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    temp.persist(path)
        .map_err(|e| format!("cannot replace {}: {}", path.display(), e.error))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Switches {
    pub pstack: bool,
    pub mandate: bool,
}

impl Default for Switches {
    fn default() -> Self {
        Self {
            pstack: true,
            mandate: true,
        }
    }
}

/// Set `pstack` and `pstack_mandate` in the `cedian.toml` at `path`, keeping
/// every other key and comment. A file that does not parse, or would not
/// parse afterwards, is left as it is.
pub fn write_switches(path: &Path, switches: Switches) -> Result<(), String> {
    let current = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            format!("schema = {}\n", crate::SETTINGS_SCHEMA)
        }
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let mut document: toml_edit::DocumentMut = current
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    document["pstack"] = toml_edit::value(switches.pstack);
    document["pstack_mandate"] = toml_edit::value(switches.mandate);
    let text = document.to_string();
    crate::parse_settings(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    write_atomically(path, text.as_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PstackBundle {
    pub upstream: PathBuf,
    pub cedian: PathBuf,
}

impl PstackBundle {
    /// `Contents/Resources/pstack{,-cedian}` when `exe` is in a bundle's
    /// `Contents/MacOS`, else `vendor/pstack{,-cedian}` in the first of
    /// `exe`'s four nearest ancestors that has them, which finds the
    /// repository from `target/<profile>/cedian` and from a test's
    /// `target/<profile>/deps/<t>`.
    pub fn for_exe(exe: &Path) -> Option<Self> {
        let complete =
            |bundle: Self| (bundle.skills().is_dir() && bundle.cedian.is_dir()).then_some(bundle);
        let macos = exe.parent()?;
        if macos.file_name()? == "MacOS" && macos.parent()?.file_name()? == "Contents" {
            let resources = macos.parent()?.join("Resources");
            return complete(Self {
                upstream: resources.join("pstack"),
                cedian: resources.join("pstack-cedian"),
            });
        }
        exe.ancestors().skip(1).take(4).find_map(|dir| {
            complete(Self {
                upstream: dir.join("vendor/pstack"),
                cedian: dir.join("vendor/pstack-cedian"),
            })
        })
    }

    pub fn skills(&self) -> PathBuf {
        self.upstream.join("skills")
    }

    pub fn mandate(&self, sheet: &Path) -> Result<String, String> {
        let template = self.cedian.join("mandate.md");
        let text = std::fs::read_to_string(&template)
            .map_err(|e| format!("cannot read {}: {e}", template.display()))?;
        Ok(text
            .replace("{{pstack}}", &self.upstream.to_string_lossy())
            .replace("{{cedian}}", &self.cedian.to_string_lossy())
            .replace("{{models}}", &sheet.to_string_lossy()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PstackPin {
    pub version: String,
    pub commit: String,
    pub author: String,
    pub license: String,
}

pub fn pin() -> Result<PstackPin, String> {
    serde_json::from_str(include_str!("../../../vendor/pstack-cedian/pin.json"))
        .map_err(|e| format!("vendor/pstack-cedian/pin.json: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_settings;

    #[test]
    fn the_sheet_is_written_in_the_shape_the_skills_read() {
        let mut sheet = PstackModels::default();
        sheet.default_effort = DefaultEffort::Level(Effort::High);
        sheet.set(
            PstackRole::Feature,
            Some(RoleModel::new("anthropic/claude-opus-5", Some(Effort::Xhigh)).unwrap()),
        );
        sheet.set(
            PstackRole::Strongest,
            Some(RoleModel::new("@slow", None).unwrap()),
        );
        let text = sheet.render();
        let lines: Vec<&str> = text.lines().filter(|line| line.contains(": ")).collect();
        assert_eq!(
            lines,
            [
                "feature, refactoring: anthropic/claude-opus-5 @xhigh",
                "strongest judgment: @slow",
                "default effort: high",
            ]
        );
        assert_eq!(PstackModels::parse(&text), sheet, "round trip");
    }

    #[test]
    fn agent_default_and_same_as_session_write_no_role_line() {
        let text = PstackModels::default().render();
        assert!(text.lines().any(|line| line == "default effort: session"));
        for role in PstackRole::ALL {
            let label = format!("{}:", role.sheet_label());
            assert!(
                !text.lines().any(|line| line.starts_with(&label)),
                "{role:?}"
            );
        }
    }

    #[test]
    fn a_model_that_would_break_the_line_is_refused() {
        for bad in ["", "a b", "x\ny", "@", "@sl ow", "@a@b"] {
            assert!(RoleModel::new(bad, None).is_err(), "{bad:?}");
        }
        assert!(RoleModel::new("ollama/llama3:8b", None).is_ok());
        assert!(RoleModel::new("@review-alt", Some(Effort::Low)).is_ok());
    }

    #[test]
    fn the_sheet_is_replaced_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent/pstack-models.md");
        let mut sheet = PstackModels::default();
        sheet.write(&path).unwrap();
        sheet.default_effort = DefaultEffort::Level(Effort::Low);
        sheet.write(&path).unwrap();
        assert_eq!(PstackModels::read(&path).unwrap(), sheet);
        let left: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, ["pstack-models.md"], "no temp file left");
        assert_eq!(
            PstackModels::read(&dir.path().join("missing.md")).unwrap(),
            PstackModels::default()
        );
    }

    #[test]
    fn the_switches_are_written_into_cedian_toml_keeping_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cedian.toml");
        std::fs::write(
            &path,
            "schema = 1\n# mine\n[permissions]\nsafe = \"allow\"\nproject_write = \"allow\"\ndangerous = \"deny\"\n",
        )
        .unwrap();
        write_switches(
            &path,
            Switches {
                pstack: false,
                mandate: true,
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# mine"), "comments kept");
        let settings = parse_settings(&text).unwrap();
        assert!(!settings.pstack);
        assert!(settings.pstack_mandate);
        assert_eq!(settings.permissions.dangerous, crate::Verdict::Deny);

        let missing = dir.path().join("new/cedian.toml");
        write_switches(
            &missing,
            Switches {
                pstack: true,
                mandate: false,
            },
        )
        .unwrap();
        let settings = parse_settings(&std::fs::read_to_string(&missing).unwrap()).unwrap();
        assert!(settings.pstack && !settings.pstack_mandate);

        std::fs::write(&path, "schema = 1\nnot valid = = toml").unwrap();
        assert!(
            write_switches(&path, Switches::default()).is_err(),
            "a file that does not parse is never overwritten"
        );
    }

    #[test]
    fn the_mandate_names_the_bundle_and_the_sheet() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = PstackBundle {
            upstream: dir.path().join("pstack"),
            cedian: dir.path().join("pstack-cedian"),
        };
        std::fs::create_dir_all(&bundle.cedian).unwrap();
        std::fs::write(
            bundle.cedian.join("mandate.md"),
            "a {{pstack}} b {{cedian}} c {{models}}",
        )
        .unwrap();
        let mandate = bundle
            .mandate(Path::new("/c/agent/pstack-models.md"))
            .unwrap();
        assert_eq!(
            mandate,
            format!(
                "a {} b {} c /c/agent/pstack-models.md",
                bundle.upstream.display(),
                bundle.cedian.display()
            )
        );
    }

    #[test]
    fn the_bundle_is_found_in_the_app_and_in_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("cedian.app/Contents");
        std::fs::create_dir_all(app.join("MacOS")).unwrap();
        std::fs::create_dir_all(app.join("Resources/pstack/skills")).unwrap();
        std::fs::create_dir_all(app.join("Resources/pstack-cedian")).unwrap();
        let found = PstackBundle::for_exe(&app.join("MacOS/cedian")).unwrap();
        assert_eq!(found.skills(), app.join("Resources/pstack/skills"));
        assert_eq!(found.cedian, app.join("Resources/pstack-cedian"));

        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join("vendor/pstack/skills")).unwrap();
        std::fs::create_dir_all(repo.join("vendor/pstack-cedian")).unwrap();
        let found = PstackBundle::for_exe(&repo.join("target/debug/deps/t-123")).unwrap();
        assert_eq!(found.upstream, repo.join("vendor/pstack"));
        assert!(PstackBundle::for_exe(&dir.path().join("elsewhere/bin/cedian")).is_none());
    }

    #[test]
    fn the_pin_is_the_vendored_version() {
        let pin = pin().unwrap();
        assert_eq!(pin.version, "0.15.24");
        assert!(pin.commit.starts_with("1e5c90a"));
        let commit = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pstack/COMMIT"),
        )
        .unwrap();
        assert_eq!(
            commit.split_whitespace().next(),
            Some(pin.commit.as_str()),
            "pin.json and the vendored COMMIT agree"
        );
    }
}
