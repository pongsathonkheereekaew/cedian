//! Import from OMP (`cedian agent import`, Settings > Account): bring the
//! person's OMP skills (and, on request, settings and model roles) into
//! cedian's agent config dir (ADR-0078 decision 5). The source is only read. Only the files named here are
//! copied, an allow-list: auth (`agent.db`), `.env` and token files never
//! are. A dry run by default; an existing target is refused without
//! `--force`, before anything is written.

use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: cedian agent import [--settings] [--models] [--force] [--apply] [--from <omp agent dir>]";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Options {
    pub settings: bool,
    pub models: bool,
    pub force: bool,
    pub apply: bool,
    pub from: Option<PathBuf>,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = Self::default();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--settings" => opts.settings = true,
                "--models" => opts.models = true,
                "--force" => opts.force = true,
                "--apply" => opts.apply = true,
                "--from" => opts.from = Some(args.next().ok_or(USAGE)?.into()),
                other => return Err(format!("unknown flag {other:?}\n{USAGE}")),
            }
        }
        Ok(opts)
    }
}

/// One thing to copy, relative to both agent dirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub rel: PathBuf,
    pub exists: bool,
}

/// What an import from `source` into `dest` copies.
pub fn plan(source: &Path, dest: &Path, opts: &Options) -> Result<Vec<Item>, String> {
    let mut rels = Vec::new();
    let skills = source.join("skills");
    if let Ok(entries) = std::fs::read_dir(&skills) {
        let mut names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
        names.sort();
        rels.extend(names.into_iter().map(|n| Path::new("skills").join(n)));
    }
    for (wanted, file) in [(opts.settings, "config.yml"), (opts.models, "models.yml")] {
        if wanted {
            if !source.join(file).is_file() {
                return Err(format!("{} has no {file}", source.display()));
            }
            rels.push(file.into());
        }
    }
    Ok(rels
        .into_iter()
        .map(|rel| Item {
            exists: std::fs::symlink_metadata(dest.join(&rel)).is_ok(),
            rel,
        })
        .collect())
}

/// Run the import: print the plan, and with `--apply` copy it. Returns the
/// report lines.
pub fn run(source: &Path, dest: &Path, opts: &Options) -> Result<Vec<String>, String> {
    if !source.is_dir() {
        return Err(format!("no OMP agent dir at {}", source.display()));
    }
    if same_dir(source, dest) {
        return Err(format!("{} is already cedian's agent dir", dest.display()));
    }
    let items = plan(source, dest, opts)?;
    let taken: Vec<_> = items.iter().filter(|i| i.exists).collect();
    if !opts.force && !taken.is_empty() {
        let names: Vec<_> = taken.iter().map(|i| i.rel.display().to_string()).collect();
        return Err(format!(
            "already in {}: {}; nothing copied (--force replaces them)",
            dest.display(),
            names.join(", ")
        ));
    }
    let verb = if opts.apply { "copied" } else { "would copy" };
    let mut lines = vec![format!("{} -> {}", source.display(), dest.display())];
    for item in &items {
        if opts.apply {
            let target = dest.join(&item.rel);
            if item.exists {
                remove(&target)?;
            }
            copy(&source.join(&item.rel), &target)?;
        }
        let replaced = if item.exists { " (replaced)" } else { "" };
        lines.push(format!("{verb} {}{replaced}", item.rel.display()));
    }
    if items.is_empty() {
        lines.push("nothing to import".to_string());
    } else if !opts.apply {
        lines.push("dry run: pass --apply to copy".to_string());
    }
    Ok(lines)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn remove(path: &Path) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
    .map_err(|e| format!("remove {}: {e}", path.display()))
}

/// Copy `from` to `to`: directories recursively, symlinks as symlinks.
fn copy(from: &Path, to: &Path) -> Result<(), String> {
    let err = |e: std::io::Error| format!("copy {} -> {}: {e}", from.display(), to.display());
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    let meta = std::fs::symlink_metadata(from).map_err(err)?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(from).map_err(err)?;
        std::os::unix::fs::symlink(target, to).map_err(err)
    } else if meta.is_dir() {
        std::fs::create_dir_all(to).map_err(err)?;
        for entry in std::fs::read_dir(from).map_err(err)? {
            let entry = entry.map_err(err)?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ()).map_err(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("cedian-{tag}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            Self(std::fs::canonicalize(&dir).unwrap())
        }
    }

    impl std::ops::Deref for TestDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn omp_home(t: &TestDir) -> PathBuf {
        let agent = t.join("home/.omp/agent");
        std::fs::create_dir_all(agent.join("skills/alpha/refs")).unwrap();
        std::fs::write(agent.join("skills/alpha/SKILL.md"), "alpha").unwrap();
        std::fs::write(agent.join("skills/alpha/refs/r.md"), "r").unwrap();
        std::fs::write(agent.join("skills/beta.md"), "beta").unwrap();
        std::os::unix::fs::symlink("/elsewhere/gamma", agent.join("skills/gamma")).unwrap();
        std::fs::write(agent.join("config.yml"), "modelRoles: {}\n").unwrap();
        std::fs::write(agent.join("models.yml"), "providers: {}\n").unwrap();
        std::fs::write(agent.join("agent.db"), "TOKENS").unwrap();
        std::fs::write(agent.join(".env"), "OPENAI_API_KEY=x").unwrap();
        agent
    }

    fn opts(flags: &[&str]) -> Options {
        Options::parse(&flags.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    fn tree(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let p = e.path();
                let meta = std::fs::symlink_metadata(&p).unwrap();
                if meta.is_dir() {
                    stack.push(p.clone());
                }
                out.push(format!(
                    "{} {:?}",
                    p.strip_prefix(root).unwrap().display(),
                    meta.modified().ok()
                ));
            }
        }
        out.sort();
        out
    }

    #[test]
    fn dry_run_by_default_writes_nothing() {
        let t = TestDir::new("import-dry");
        let source = omp_home(&t);
        let dest = t.join("home/.config/cedian/agent");
        let lines = run(&source, &dest, &opts(&[])).unwrap();
        assert_eq!(
            lines[1..],
            [
                "would copy skills/alpha",
                "would copy skills/beta.md",
                "would copy skills/gamma",
                "dry run: pass --apply to copy"
            ]
        );
        assert!(!dest.exists());
    }

    #[test]
    fn apply_copies_skills_and_only_named_files_never_tokens() {
        let t = TestDir::new("import-apply");
        let source = omp_home(&t);
        let before = tree(&source);
        let dest = t.join("home/.config/cedian/agent");
        run(
            &source,
            &dest,
            &opts(&["--apply", "--settings", "--models"]),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("skills/alpha/refs/r.md")).unwrap(),
            "r"
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("skills/beta.md")).unwrap(),
            "beta"
        );
        assert_eq!(
            std::fs::read_link(dest.join("skills/gamma")).unwrap(),
            PathBuf::from("/elsewhere/gamma")
        );
        assert!(dest.join("config.yml").is_file());
        assert!(dest.join("models.yml").is_file());
        assert!(!dest.join("agent.db").exists());
        assert!(!dest.join(".env").exists());
        assert_eq!(tree(&source), before, "the source is only read");
    }

    #[test]
    fn settings_and_models_need_their_flags() {
        let t = TestDir::new("import-skills-only");
        let source = omp_home(&t);
        let dest = t.join("dest");
        run(&source, &dest, &opts(&["--apply"])).unwrap();
        assert!(!dest.join("config.yml").exists());
        assert!(!dest.join("models.yml").exists());
    }

    #[test]
    fn existing_target_refused_without_force_and_nothing_copied() {
        let t = TestDir::new("import-force");
        let source = omp_home(&t);
        let dest = t.join("dest");
        std::fs::create_dir_all(dest.join("skills")).unwrap();
        std::fs::write(dest.join("skills/beta.md"), "mine").unwrap();
        let err = run(&source, &dest, &opts(&["--apply"])).unwrap_err();
        assert!(err.contains("skills/beta.md"), "{err}");
        assert!(
            !dest.join("skills/alpha").exists(),
            "refused before any write"
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("skills/beta.md")).unwrap(),
            "mine"
        );

        let lines = run(&source, &dest, &opts(&["--apply", "--force"])).unwrap();
        assert!(lines.contains(&"copied skills/beta.md (replaced)".to_string()));
        assert_eq!(
            std::fs::read_to_string(dest.join("skills/beta.md")).unwrap(),
            "beta"
        );
    }

    #[test]
    fn missing_settings_file_is_an_error() {
        let t = TestDir::new("import-missing");
        let source = t.join("agent");
        std::fs::create_dir_all(&source).unwrap();
        assert!(run(&source, &t.join("dest"), &opts(&["--settings"])).is_err());
    }
}
