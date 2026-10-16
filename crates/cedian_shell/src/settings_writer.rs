//! Writes to the user's `cedian.toml` from the settings window: one typed
//! change at a time, through `toml_edit` so every other key and comment
//! stays. A file that does not parse, or would not parse afterwards, is
//! left as it is (ADR-0083 decision 3).

use crate::settings::{Policy, Verdict, parse_settings, project_key_problem};
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Safe,
    ProjectWrite,
    Dangerous,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Safe, Tier::ProjectWrite, Tier::Dangerous];

    fn key(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::ProjectWrite => "project_write",
            Self::Dangerous => "dangerous",
        }
    }
}

/// One change to a `[projects."<path>"]` entry. A value equal to the
/// default removes its key, and an entry left with nothing goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectChange {
    Policy(Policy),
    /// `None` inherits the user's `pstack`.
    Pstack(Option<bool>),
    ReadFolders(Vec<PathBuf>),
    Remove,
}

pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
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

/// Apply `change` to the document at `path` (a new file starts with the
/// schema line) and replace the file in one rename, only when the result
/// is valid settings.
pub(crate) fn edit(
    path: &Path,
    change: impl FnOnce(&mut DocumentMut) -> Result<(), String>,
) -> Result<(), String> {
    let current = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            format!("schema = {}\n", crate::SETTINGS_SCHEMA)
        }
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let mut document: DocumentMut = current
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    parse_settings(&current).map_err(|e| format!("{}: {e}", path.display()))?;
    change(&mut document)?;
    let text = document.to_string();
    parse_settings(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    write_atomically(path, text.as_bytes())
}

/// Set one approval tier. `[permissions]` takes all three, so the other two
/// are written with the values they have now.
pub fn write_tier(path: &Path, tier: Tier, verdict: Verdict) -> Result<(), String> {
    edit(path, |document| {
        let current = parse_settings(&document.to_string())
            .map_err(|e| e.to_string())?
            .permissions;
        let verdict_of = |each: Tier| match each {
            _ if each == tier => verdict,
            Tier::Safe => current.safe,
            Tier::ProjectWrite => current.project_write,
            Tier::Dangerous => current.dangerous,
        };
        for each in Tier::ALL {
            let text = match verdict_of(each) {
                Verdict::Allow => "allow",
                Verdict::Ask => "ask",
                Verdict::Deny => "deny",
            };
            let slot = &mut document["permissions"][each.key()];
            if slot.as_str() == Some(text) {
                continue;
            }
            let mut new = toml_edit::Value::from(text);
            if let Some(old) = slot.as_value() {
                *new.decor_mut() = old.decor().clone();
            }
            *slot = Item::Value(new);
        }
        Ok(())
    })
}

/// Where `path` resolves to, for a file that may not exist yet.
fn resolved(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        match (
            path.parent().and_then(|dir| dir.canonicalize().ok()),
            path.file_name(),
        ) {
            (Some(dir), Some(name)) => dir.join(name),
            _ => path.to_path_buf(),
        }
    })
}

/// Change the entry for the project at `key`. Every value but `Remove`
/// needs `key` absolute, existing and canonical, as the readers check, and a
/// settings file outside the project: a file the project carries never
/// holds its settings (ADR-0035, ADR-0077 decision 4). `Remove` takes any
/// key, so a broken entry can go.
pub fn write_project(path: &Path, key: &str, change: ProjectChange) -> Result<(), String> {
    if change != ProjectChange::Remove {
        if let Some(why) = project_key_problem(key) {
            return Err(format!("{key}: {why}"));
        }
        if resolved(path).starts_with(key) {
            return Err(format!(
                "{} is inside {key}, so it cannot hold that project's settings",
                path.display()
            ));
        }
    }
    if let ProjectChange::ReadFolders(folders) = &change {
        for folder in folders {
            if let Some(why) = project_key_problem(&folder.to_string_lossy()) {
                return Err(format!("{}: {why}", folder.display()));
            }
            // The project and its ancestors are already readable, and an
            // ancestor would widen the scope past the project (ADR-0084).
            if Path::new(key).starts_with(folder) {
                return Err(format!(
                    "{} contains the project, so it cannot be a read folder",
                    folder.display()
                ));
            }
        }
    }
    edit(path, |document| {
        if !document.contains_key("projects") {
            let mut projects = Table::new();
            projects.set_implicit(true);
            document.insert("projects", Item::Table(projects));
        }
        let projects = document["projects"]
            .as_table_mut()
            .ok_or("`projects` is not a table")?;
        if change == ProjectChange::Remove {
            projects.remove(key);
        } else {
            let entry = projects
                .entry(key)
                .or_insert_with(|| Item::Table(Table::new()))
                .as_table_mut()
                .ok_or_else(|| format!("[projects.{key:?}] is not a table"))?;
            match change {
                ProjectChange::Policy(Policy::Cedian) => {
                    entry.remove("policy");
                }
                ProjectChange::Policy(Policy::Omp) => {
                    entry.insert("policy", value("omp"));
                }
                ProjectChange::Pstack(None) => {
                    entry.remove("pstack");
                }
                ProjectChange::Pstack(Some(on)) => {
                    entry.insert("pstack", value(on));
                }
                ProjectChange::ReadFolders(folders) if folders.is_empty() => {
                    entry.remove("read_folders");
                }
                ProjectChange::ReadFolders(folders) => {
                    let list: Array = folders
                        .iter()
                        .map(|folder| folder.to_string_lossy().into_owned())
                        .collect();
                    entry.insert("read_folders", value(list));
                }
                ProjectChange::Remove => {}
            }
            if entry.is_empty() {
                projects.remove(key);
            }
        }
        if projects.is_empty() {
            document.remove("projects");
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{RunKind, load_user_file};
    use crate::test_dir::TestDir;

    fn project(dir: &Path, name: &str) -> String {
        let path = dir.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_tier_write_keeps_the_other_tiers_and_every_comment() {
        let dir = TestDir::new("writer-tier");
        let path = dir.join("cedian.toml");
        std::fs::write(
            &path,
            "# mine\nschema = 1\n[permissions]\nsafe = \"allow\"\nproject_write = \"deny\" # careful\ndangerous = \"ask\"\n",
        )
        .unwrap();
        write_tier(&path, Tier::Dangerous, Verdict::Deny).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("# mine\n") && text.contains("# careful"),
            "{text}"
        );
        let permissions = load_user_file(&path).unwrap().permissions;
        assert_eq!(
            (
                permissions.safe,
                permissions.project_write,
                permissions.dangerous
            ),
            (Verdict::Allow, Verdict::Deny, Verdict::Deny)
        );

        let fresh = dir.join("fresh/cedian.toml");
        write_tier(&fresh, Tier::Safe, Verdict::Ask).unwrap();
        let permissions = load_user_file(&fresh).unwrap().permissions;
        assert_eq!(
            (
                permissions.safe,
                permissions.project_write,
                permissions.dangerous
            ),
            (Verdict::Ask, Verdict::Allow, Verdict::Ask),
            "a new file starts from the defaults"
        );
    }

    #[test]
    fn project_values_round_trip_per_project_and_defaults_remove_the_entry() {
        let dir = TestDir::new("writer-project");
        let path = dir.join("config/cedian.toml");
        let one = project(&dir, "one");
        let two = project(&dir, "two");
        let outside = project(&dir, "docs");

        write_project(&path, &one, ProjectChange::Policy(Policy::Omp)).unwrap();
        write_project(&path, &one, ProjectChange::Pstack(Some(false))).unwrap();
        write_project(
            &path,
            &two,
            ProjectChange::ReadFolders(vec![PathBuf::from(&outside)]),
        )
        .unwrap();
        let settings = load_user_file(&path).unwrap();
        assert_eq!(
            settings
                .policy_for(Path::new(&one), RunKind::Interactive)
                .policy,
            Policy::Omp
        );
        assert!(!settings.pstack_for(Path::new(&one)).enabled);
        assert!(
            settings
                .read_folders_for(Path::new(&one))
                .folders
                .is_empty()
        );
        assert_eq!(
            settings
                .policy_for(Path::new(&two), RunKind::Interactive)
                .policy,
            Policy::Cedian,
            "one project's values never reach another"
        );
        assert!(settings.pstack_for(Path::new(&two)).enabled);
        assert_eq!(
            settings.read_folders_for(Path::new(&two)).folders,
            [PathBuf::from(&outside)]
        );

        write_project(&path, &one, ProjectChange::Policy(Policy::Cedian)).unwrap();
        write_project(&path, &one, ProjectChange::Pstack(None)).unwrap();
        write_project(&path, &two, ProjectChange::ReadFolders(Vec::new())).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "schema = 1\n",
            "default values leave no entry behind"
        );
    }

    #[test]
    fn remove_takes_a_broken_key_and_values_refuse_one() {
        let dir = TestDir::new("writer-remove");
        let path = dir.join("cedian.toml");
        std::fs::write(
            &path,
            "schema = 1\n[projects.\"relative/x\"]\npstack = false\n",
        )
        .unwrap();
        assert!(write_project(&path, "relative/x", ProjectChange::Pstack(Some(true))).is_err());
        write_project(&path, "relative/x", ProjectChange::Remove).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "schema = 1\n");

        let one = project(&dir, "one");
        let err = write_project(
            &path,
            &one,
            ProjectChange::ReadFolders(vec![PathBuf::from("relative/docs")]),
        )
        .unwrap_err();
        assert!(err.contains("not an absolute path"), "{err}");
    }

    #[test]
    fn a_read_folder_is_never_the_project_or_one_of_its_ancestors() {
        let dir = TestDir::new("read-folder-guard");
        let project = dir.join("parent/project");
        let sibling = dir.join("parent/sibling");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        let project = project.canonicalize().unwrap();
        let parent = project.parent().unwrap().to_path_buf();
        let sibling = sibling.canonicalize().unwrap();
        let path = dir.join("cedian.toml");
        std::fs::write(&path, "schema = 1\n").unwrap();
        let key = project.to_string_lossy().into_owned();
        for folder in [project, parent, PathBuf::from("/")] {
            let error = write_project(
                &path,
                &key,
                ProjectChange::ReadFolders(vec![folder.clone()]),
            )
            .unwrap_err();
            assert!(
                error.contains("contains the project"),
                "{folder:?}: {error}"
            );
        }
        write_project(
            &path,
            &key,
            ProjectChange::ReadFolders(vec![sibling.clone()]),
        )
        .unwrap();
        assert_eq!(
            load_user_file(&path).unwrap().project_read_folders(&key),
            [sibling]
        );
    }

    #[test]
    fn a_settings_file_inside_the_project_is_never_written_for_it() {
        let dir = TestDir::new("writer-inside");
        let repo = project(&dir, "repo");
        let path = Path::new(&repo).join("cedian.toml");
        let outside = project(&dir, "docs");
        for change in [
            ProjectChange::Policy(Policy::Omp),
            ProjectChange::Pstack(Some(false)),
            ProjectChange::ReadFolders(vec![PathBuf::from(&outside)]),
        ] {
            let err = write_project(&path, &repo, change).unwrap_err();
            assert!(err.contains("inside"), "{err}");
        }
        assert!(!path.exists());
    }

    #[test]
    fn an_unparseable_or_invalid_file_is_never_overwritten() {
        let dir = TestDir::new("writer-refuse");
        let one = project(&dir, "one");
        for text in [
            "schema = 1\nnot = valid = toml",
            "schema = 1\nmystery = true\n",
        ] {
            let path = dir.join("cedian.toml");
            std::fs::write(&path, text).unwrap();
            assert!(write_tier(&path, Tier::Safe, Verdict::Deny).is_err());
            assert!(write_project(&path, &one, ProjectChange::Policy(Policy::Omp)).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
    }
}
