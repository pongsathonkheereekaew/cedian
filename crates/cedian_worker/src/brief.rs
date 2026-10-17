//! The worker brief (ADR-0033 decision 1): the checked shape a worktree
//! request or `cedian worker spawn` must carry before anything is created.
//! One validator, so the agent and headless paths cannot disagree.
//!
//! [`WorkerScope`] compiles the briefs' globs per worker tree and attributes
//! a write to its tree: the rule lives here, so the panel never re-implements it.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The required brief fields, in the order a refusal names them.
pub const REQUIRED_FIELDS: [&str; 5] =
    ["goal", "scope.write", "acceptance", "verify", "timebox_min"];

/// Where the worker may write, and where it must not (ADR-0033 decision 3).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scope {
    pub write: Vec<String>,
    pub deny: Vec<String>,
}

/// One turn's write scope, keyed by worker tree: each running worker's
/// compiled brief, rooted at its worktree. A write is attributed to the tree
/// it lands in and checked against that tree's brief alone. `src/**` covers
/// the subtree (`src/lib.rs`, `src/a/b.rs`); globs are tree-relative.
#[derive(Debug)]
pub struct WorkerScope {
    trees: Vec<TreeScope>,
}

#[derive(Debug)]
struct TreeScope {
    worker: String,
    root: PathBuf,
    write: GlobSet,
    deny: GlobSet,
}

/// Which running worker's tree a write landed in, and whether its brief
/// allows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution<'a> {
    pub worker: &'a str,
    /// Tree-relative, `/` separators.
    pub rel: String,
    pub in_scope: bool,
}

impl WorkerScope {
    /// Compile `briefs` — each `(worker id, worktree path relative to
    /// `repo`, brief)`. A glob that will not compile is an error naming it,
    /// never a silent non-match.
    pub fn new<'a>(
        repo: &Path,
        briefs: impl IntoIterator<Item = (&'a str, &'a str, &'a Brief)>,
    ) -> Result<Self, ScopeError> {
        let mut trees = Vec::new();
        for (worker, worktree, brief) in briefs {
            trees.push(TreeScope {
                worker: worker.to_string(),
                root: canonical_path(&repo.join(worktree)),
                write: compile(&brief.scope.write)?,
                deny: compile(&brief.scope.deny)?,
            });
        }
        Ok(Self { trees })
    }

    /// The running worker whose tree holds `abs`, the tree-relative path,
    /// and whether that worker's brief allows it. `None`: under no running
    /// worker's tree.
    pub fn attribute(&self, abs: &Path) -> Option<Attribution<'_>> {
        let abs = canonical_path(abs);
        let (tree, rel) = self
            .trees
            .iter()
            .filter_map(|tree| Some((tree, abs.strip_prefix(&tree.root).ok()?)))
            .max_by_key(|(tree, _)| tree.root.components().count())?;
        let rel = rel
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let in_scope = tree.write.is_match(&rel) && !tree.deny.is_match(&rel);
        Some(Attribution {
            worker: &tree.worker,
            rel,
            in_scope,
        })
    }

    /// Where `worker`'s tree is, when it is one the scope was built from.
    pub fn root(&self, worker: &str) -> Option<&Path> {
        self.trees
            .iter()
            .find(|tree| tree.worker == worker)
            .map(|tree| tree.root.as_path())
    }

    /// The running workers the scope was built from.
    pub fn workers(&self) -> impl Iterator<Item = &str> {
        self.trees.iter().map(|tree| tree.worker.as_str())
    }
}

/// `path` as the filesystem spells it — symlinks resolved, so `/var` and
/// `/private/var` meet — with the part that does not exist yet kept as
/// written, except `..` and `.` which are applied so a missing tail cannot
/// walk into another tree (`w1/missing/../../w2/x` attributes to `w2`).
pub fn canonical_path(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut missing = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(existing) {
            return missing.iter().rev().fold(real, |mut joined, part| {
                if *part == std::ffi::OsStr::new("..") {
                    joined.pop();
                } else if *part != std::ffi::OsStr::new(".") {
                    joined.push(part);
                }
                joined
            });
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// A glob in a brief will not compile; the error names the pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeError {
    /// The pattern that would not compile.
    pub pattern: String,
}

impl std::fmt::Display for ScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "brief scope: {:?} is not a valid glob (ADR-0033)",
            self.pattern
        )
    }
}

impl std::error::Error for ScopeError {}

/// Compile globs with `literal_separator`: `*` stays in one path segment,
/// `**` crosses them, so `src/**` is the whole subtree.
fn compile(patterns: &[String]) -> Result<GlobSet, ScopeError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|_| ScopeError {
                pattern: pattern.clone(),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|_| ScopeError {
        pattern: patterns.join(", "),
    })
}

/// A checked worker brief; [`Brief::to_json`] is the canonical stored form.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Brief {
    pub goal: String,
    pub scope: Scope,
    pub acceptance: Vec<String>,
    pub verify: Vec<String>,
    pub timebox_min: f64,
    pub context: Vec<String>,
    pub forbidden: Vec<String>,
}

/// Why a brief was refused: fields to name, or a shape that will not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BriefError {
    /// Every required field that is absent or empty, in ADR order.
    Missing(Vec<String>),
    /// Not JSON, not an object, or a field of the wrong type.
    Malformed(String),
}

impl std::fmt::Display for BriefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(fields) => write!(
                f,
                "no worktree: the brief is missing {} (ADR-0033)",
                fields.join(", ")
            ),
            Self::Malformed(why) => {
                write!(f, "no worktree: the brief is malformed: {why} (ADR-0033)")
            }
        }
    }
}

impl std::error::Error for BriefError {}

impl Brief {
    /// Parse and check a brief from JSON. A refusal names every required
    /// field that is missing or empty (ADR-0033 decision 1).
    pub fn from_json(raw: &str) -> Result<Self, BriefError> {
        let value: Value =
            serde_json::from_str(raw).map_err(|e| BriefError::Malformed(e.to_string()))?;
        let obj = value
            .as_object()
            .ok_or_else(|| BriefError::Malformed("not a JSON object".to_string()))?;

        let goal = req_text(obj.get("goal"), "goal")?;
        let (write, deny) = match obj.get("scope") {
            None => (None, Vec::new()),
            Some(Value::Object(scope)) => (
                req_list(scope.get("write"), "scope.write")?,
                opt_list(scope.get("deny"), "scope.deny")?,
            ),
            Some(_) => return Err(BriefError::Malformed("scope must be an object".to_string())),
        };
        let acceptance = req_list(obj.get("acceptance"), "acceptance")?;
        let verify = req_list(obj.get("verify"), "verify")?;
        let timebox_min = req_positive(obj.get("timebox_min"), "timebox_min")?;
        let context = opt_list(obj.get("context"), "context")?;
        let forbidden = opt_list(obj.get("forbidden"), "forbidden")?;

        let missing: Vec<String> = REQUIRED_FIELDS
            .iter()
            .filter(|field| !match **field {
                "goal" => goal.is_some(),
                "scope.write" => write.is_some(),
                "acceptance" => acceptance.is_some(),
                "verify" => verify.is_some(),
                "timebox_min" => timebox_min.is_some(),
                other => unreachable!("unknown required field {other}"),
            })
            .map(|field| (*field).to_string())
            .collect();
        if !missing.is_empty() {
            return Err(BriefError::Missing(missing));
        }
        Ok(Self {
            goal: goal.expect("present unless named missing"),
            scope: Scope {
                write: write.expect("present unless named missing"),
                deny,
            },
            acceptance: acceptance.expect("present unless named missing"),
            verify: verify.expect("present unless named missing"),
            timebox_min: timebox_min.expect("present unless named missing"),
            context,
            forbidden,
        })
    }

    /// Read and check a brief file (`cedian worker spawn --brief <file>`).
    pub fn from_file(path: &Path) -> Result<Self, BriefError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| BriefError::Malformed(format!("{}: {e}", path.display())))?;
        Self::from_json(&raw)
    }

    /// The canonical stored JSON for this brief.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("a brief has no unserializable field")
    }

    /// The highest revision of `id` under `<state>/briefs/`, or an error when
    /// none exists or the file will not parse as a brief.
    pub fn latest(state: &Path, id: &str) -> Result<Self, BriefError> {
        let dir = state.join("briefs");
        let prefix = format!("{id}.");
        let mut best: Option<(u32, PathBuf)> = None;
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| BriefError::Malformed(format!("briefs for {id}: {e}")))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| BriefError::Malformed(format!("briefs for {id}: {e}")))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(rest) = name
                .strip_prefix(&prefix)
                .and_then(|s| s.strip_suffix(".json"))
            else {
                continue;
            };
            let Ok(rev) = rest.parse::<u32>() else {
                continue;
            };
            if best.as_ref().is_none_or(|(r, _)| rev > *r) {
                best = Some((rev, entry.path()));
            }
        }
        let Some((_, path)) = best else {
            return Err(BriefError::Malformed(format!(
                "no stored brief for worker {id}"
            )));
        };
        Self::from_file(&path)
    }

    /// Write this revision to `<state>/briefs/<id>.<revision>.json`. `create_new`
    /// keeps a stored revision immutable — a change is a new revision beside it
    /// (ADR-0033 decision 2) — and returns the path so the caller can remove it
    /// when the spawn it was stored for fails.
    pub fn store(&self, state: &Path, id: &str, revision: u32) -> Result<PathBuf, String> {
        let dir = state.join("briefs");
        let path = dir.join(format!("{id}.{revision}.json"));
        let body = self.to_json();
        std::fs::create_dir_all(&dir)
            .and_then(|()| {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
            })
            .and_then(|mut file| {
                std::io::Write::write_all(&mut file, body.as_bytes()).inspect_err(|_| {
                    let _ = std::fs::remove_file(&path);
                })
            })
            .map_err(|e| format!("brief {}: {e}", path.display()))?;
        Ok(path)
    }
}

/// A required non-empty string, or `None` (→ named missing) when absent/blank.
fn req_text(v: Option<&Value>, field: &str) -> Result<Option<String>, BriefError> {
    match v {
        None => Ok(None),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(Some(s.clone())),
        Some(Value::String(_)) => Ok(None),
        Some(_) => Err(BriefError::Malformed(format!(
            "{field} must be a non-empty string"
        ))),
    }
}

/// A required array of non-empty strings, or `None` when absent/empty/blank.
fn req_list(v: Option<&Value>, field: &str) -> Result<Option<Vec<String>>, BriefError> {
    let Some(v) = v else { return Ok(None) };
    let Value::Array(items) = v else {
        return Err(BriefError::Malformed(format!(
            "{field} must be an array of strings"
        )));
    };
    let strings = string_items(items, field)?;
    Ok((!strings.is_empty() && strings.iter().all(|s| !s.trim().is_empty())).then_some(strings))
}

/// An optional array of strings; absent means empty.
fn opt_list(v: Option<&Value>, field: &str) -> Result<Vec<String>, BriefError> {
    match v {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => string_items(items, field),
        Some(_) => Err(BriefError::Malformed(format!(
            "{field} must be an array of strings"
        ))),
    }
}

fn string_items(items: &[Value], field: &str) -> Result<Vec<String>, BriefError> {
    items
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                BriefError::Malformed(format!("{field} must be an array of strings"))
            })
        })
        .collect()
}

/// A required positive number, or `None` (→ named missing) when absent/≤ 0.
fn req_positive(v: Option<&Value>, field: &str) -> Result<Option<f64>, BriefError> {
    match v {
        None => Ok(None),
        Some(Value::Number(n)) => Ok(n.as_f64().filter(|m| *m > 0.0)),
        Some(_) => Err(BriefError::Malformed(format!(
            "{field} must be a positive number"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(v: Value) -> Result<Brief, BriefError> {
        Brief::from_json(&v.to_string())
    }

    fn full() -> Value {
        serde_json::json!({
            "goal": "fix the casing", "scope": {"write": ["notes.txt"]},
            "acceptance": ["notes.txt keeps its casing"], "verify": ["cat notes.txt"],
            "timebox_min": 15,
        })
    }

    #[test]
    fn a_whole_brief_parses_with_defaults() {
        let brief = parse(full()).unwrap();
        assert_eq!(brief.goal, "fix the casing");
        assert_eq!(brief.scope.write, ["notes.txt"]);
        assert_eq!(brief.timebox_min, 15.0);
        assert!(brief.scope.deny.is_empty());
        assert!(brief.context.is_empty());
        assert!(brief.forbidden.is_empty());
    }

    #[test]
    fn canonical_path_normalizes_dotdot_in_a_missing_tail() {
        let dir = std::env::temp_dir().join(format!(
            "cedian-canonical-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("w1")).unwrap();
        std::fs::create_dir_all(dir.join("w2")).unwrap();
        let sneaky = dir.join("w1/missing/../../w2/x");
        assert_eq!(
            canonical_path(&sneaky),
            dir.join("w2/x"),
            "a missing tail must not keep attribution on w1"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn each_missing_field_is_named_alone_in_adr_order() {
        for field in REQUIRED_FIELDS {
            let mut v = full();
            let obj = v.as_object_mut().unwrap();
            match field {
                "goal" => obj.remove("goal"),
                "acceptance" => obj.remove("acceptance"),
                "verify" => obj.remove("verify"),
                "timebox_min" => obj.remove("timebox_min"),
                "scope.write" => obj.insert("scope".into(), serde_json::json!({})),
                other => panic!("unhandled field {other}"),
            };
            assert_eq!(
                parse(v).unwrap_err(),
                BriefError::Missing(vec![field.to_string()]),
                "{field}"
            );
        }
    }

    #[test]
    fn an_empty_brief_names_every_field_in_order() {
        assert_eq!(
            parse(serde_json::json!({})).unwrap_err(),
            BriefError::Missing(REQUIRED_FIELDS.iter().map(|f| f.to_string()).collect())
        );
    }

    #[test]
    fn a_wrong_type_is_malformed_and_an_empty_required_list_is_missing() {
        let mut v = full();
        v.as_object_mut()
            .unwrap()
            .insert("scope".into(), serde_json::json!({"write": "notes.txt"}));
        assert!(matches!(parse(v), Err(BriefError::Malformed(_))));

        let mut v = full();
        v.as_object_mut()
            .unwrap()
            .insert("timebox_min".into(), serde_json::json!("15"));
        assert!(matches!(parse(v), Err(BriefError::Malformed(_))));

        let mut v = full();
        v.as_object_mut()
            .unwrap()
            .insert("verify".into(), serde_json::json!([]));
        assert_eq!(
            parse(v).unwrap_err(),
            BriefError::Missing(vec!["verify".to_string()])
        );
    }

    #[test]
    fn optional_lists_parse_when_present() {
        let mut v = full();
        let obj = v.as_object_mut().unwrap();
        obj.insert("context".into(), serde_json::json!(["PR #12"]));
        obj.insert("forbidden".into(), serde_json::json!(["no new deps"]));
        obj.get_mut("scope")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("deny".into(), serde_json::json!(["vendor/**"]));
        let brief = parse(v).unwrap();
        assert_eq!(brief.context, ["PR #12"]);
        assert_eq!(brief.forbidden, ["no new deps"]);
        assert_eq!(brief.scope.deny, ["vendor/**"]);
    }

    fn scope_brief(write: &[&str], deny: &[&str]) -> Brief {
        Brief {
            goal: "g".to_string(),
            scope: Scope {
                write: write.iter().map(|s| (*s).to_string()).collect(),
                deny: deny.iter().map(|s| (*s).to_string()).collect(),
            },
            acceptance: vec!["a".to_string()],
            verify: vec!["v".to_string()],
            timebox_min: 1.0,
            context: Vec::new(),
            forbidden: Vec::new(),
        }
    }

    fn one_tree(brief: &Brief) -> WorkerScope {
        WorkerScope::new(Path::new("/ws"), [("w1", ".worktrees/w1", brief)]).unwrap()
    }

    fn in_scope(scope: &WorkerScope, abs: &str) -> Option<bool> {
        scope.attribute(Path::new(abs)).map(|a| a.in_scope)
    }

    /// `src/**` covers the subtree; a sibling tree is not in scope.
    #[test]
    fn a_subtree_glob_covers_its_files() {
        let scope = one_tree(&scope_brief(&["src/**"], &[]));
        let w1 = "/ws/.worktrees/w1";
        assert_eq!(in_scope(&scope, &format!("{w1}/src/lib.rs")), Some(true));
        assert_eq!(in_scope(&scope, &format!("{w1}/src/a/b.rs")), Some(true));
        assert_eq!(in_scope(&scope, &format!("{w1}/docs/x.md")), Some(false));
        assert_eq!(in_scope(&scope, &format!("{w1}/srcs/lib.rs")), Some(false));
    }

    #[test]
    fn a_deny_glob_wins_over_a_write_glob() {
        let scope = one_tree(&scope_brief(&["src/**"], &["src/secret/**"]));
        assert_eq!(in_scope(&scope, "/ws/.worktrees/w1/src/lib.rs"), Some(true));
        assert_eq!(
            in_scope(&scope, "/ws/.worktrees/w1/src/secret/key.rs"),
            Some(false),
            "the deny wins"
        );
    }

    /// Each write is attributed to the tree it lands in and checked against
    /// that tree's brief alone: another brief allowing the path does not help.
    #[test]
    fn a_write_is_attributed_to_its_own_tree_and_brief() {
        let a = scope_brief(&["src/**"], &[]);
        let b = scope_brief(&["docs/**"], &["docs/private.md"]);
        let scope = WorkerScope::new(
            Path::new("/ws"),
            [("wa", ".worktrees/wa", &a), ("wb", ".worktrees/wb", &b)],
        )
        .unwrap();
        let attribution = scope
            .attribute(Path::new("/ws/.worktrees/wa/src/lib.rs"))
            .unwrap();
        assert_eq!(
            (
                attribution.worker,
                attribution.rel.as_str(),
                attribution.in_scope
            ),
            ("wa", "src/lib.rs", true)
        );
        let crossed = scope
            .attribute(Path::new("/ws/.worktrees/wb/src/lib.rs"))
            .unwrap();
        assert_eq!((crossed.worker, crossed.in_scope), ("wb", false));
        assert_eq!(in_scope(&scope, "/ws/.worktrees/wb/docs/x.md"), Some(true));
        assert_eq!(
            in_scope(&scope, "/ws/.worktrees/wb/docs/private.md"),
            Some(false)
        );
        assert_eq!(in_scope(&scope, "/ws/src/lib.rs"), None, "under no tree");
        assert_eq!(in_scope(&scope, "/ws/.worktrees/wab/src/lib.rs"), None);
        assert_eq!(scope.workers().collect::<Vec<_>>(), vec!["wa", "wb"]);
    }

    /// A malformed glob is an error naming the pattern, never a non-match.
    #[test]
    fn a_malformed_glob_is_an_error_naming_it() {
        let brief = scope_brief(&["src/**", "src/["], &[]);
        let err =
            WorkerScope::new(Path::new("/ws"), [("w1", ".worktrees/w1", &brief)]).unwrap_err();
        assert_eq!(err.pattern, "src/[");
        assert!(err.to_string().contains("src/["), "{err}");
        let deny_only = scope_brief(&["src/**"], &["["]);
        assert_eq!(
            WorkerScope::new(Path::new("/ws"), [("w1", ".worktrees/w1", &deny_only)])
                .unwrap_err()
                .pattern,
            "["
        );
    }

    /// A tree reached through a symlink and a write named by its real path
    /// (or the reverse) are the same tree.
    #[test]
    fn a_symlinked_root_and_a_real_write_path_meet() {
        let dir = std::env::temp_dir().join(format!("cedian-scope-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let real = dir.join("real");
        std::fs::create_dir_all(real.join(".worktrees/w1/src")).unwrap();
        std::os::unix::fs::symlink(&real, dir.join("alias")).unwrap();
        let brief = scope_brief(&["src/**"], &[]);
        let scope =
            WorkerScope::new(&dir.join("alias"), [("w1", ".worktrees/w1", &brief)]).unwrap();
        let written = real.join(".worktrees/w1/src/new.rs");
        assert_eq!(
            scope.attribute(&written).map(|a| (a.worker, a.in_scope)),
            Some(("w1", true))
        );
        let through_alias = dir.join("alias/.worktrees/w1/docs/x.md");
        assert_eq!(
            scope.attribute(&through_alias).map(|a| a.in_scope),
            Some(false)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_brief_file_is_read() {
        let dir = std::env::temp_dir().join(format!("cedian-brief-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("b.json");
        std::fs::write(&path, full().to_string()).unwrap();
        assert_eq!(Brief::from_file(&path).unwrap().goal, "fix the casing");
        assert!(Brief::from_file(&dir.join("absent.json")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn latest_picks_the_highest_revision() {
        let dir = std::env::temp_dir().join(format!("cedian-brief-latest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut brief = Brief::from_json(&full().to_string()).unwrap();
        brief.store(&dir, "w1", 1).unwrap();
        brief.verify = vec!["v2".into()];
        brief.store(&dir, "w1", 2).unwrap();
        assert_eq!(
            Brief::latest(&dir, "w1").unwrap().verify,
            vec!["v2".to_string()]
        );
        assert!(Brief::latest(&dir, "missing").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_stored_revision_is_canonical_and_immutable() {
        let dir = std::env::temp_dir().join(format!("cedian-brief-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let brief = parse(full()).unwrap();
        let path = brief.store(&dir, "w1", 1).unwrap();
        let stored: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(stored, serde_json::to_value(&brief).unwrap());
        assert_eq!(stored.get("id"), None, "the brief carries no envelope");
        assert!(
            brief.store(&dir, "w1", 1).is_err(),
            "a revision is write-once"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
