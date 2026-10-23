//! The Files tool (Interface, Layout: tool panel): the open thread's directory as a tree, its
//! worktree when it has one, else the project. In a git repository it lists what git would:
//! tracked files and untracked ones not ignored. A click on a folder opens or folds it; a click
//! on a file shows it in the Editor.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::type_scale::TypeScale as _;
use gpui::{
    Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Task, Window, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

/// More than this many files and the walk stops, so a home directory cannot hang the panel.
const MAX_FILES: usize = 50_000;

pub enum FileTreeEvent {
    /// Show this file (an absolute path) in the Editor.
    Open(PathBuf),
}

#[derive(Default)]
pub struct FileTree {
    root: PathBuf,
    /// Every file under the root, relative to it, sorted.
    files: Vec<PathBuf>,
    expanded: BTreeSet<PathBuf>,
    selected: Option<PathBuf>,
    loading: Option<Task<()>>,
}

impl EventEmitter<FileTreeEvent> for FileTree {}

/// One line of the tree as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Relative to the root.
    pub path: PathBuf,
    pub depth: usize,
    pub dir: bool,
}

impl FileTree {
    /// Shows `root`, listing it again when it is another directory.
    pub fn set_root(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if root != self.root {
            self.root = root;
            self.files.clear();
            self.expanded.clear();
            self.selected = None;
            self.refresh(cx);
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every file under the root, relative to it, sorted; empty until the first listing ends.
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Lists the root again, keeping what was open.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.root.as_os_str().is_empty() {
            return;
        }
        let root = self.root.clone();
        self.loading = Some(cx.spawn(async move |this, cx| {
            let listed = root.clone();
            let files = cx
                .background_executor()
                .spawn(async move { list(&listed) })
                .await;
            this.update(cx, |tree, cx| {
                if tree.root == root {
                    tree.files = files;
                    tree.loading = None;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn click(&mut self, row: &Row, cx: &mut Context<Self>) {
        if row.dir {
            if !self.expanded.remove(&row.path) {
                self.expanded.insert(row.path.clone());
            }
        } else {
            self.selected = Some(row.path.clone());
            cx.emit(FileTreeEvent::Open(self.root.join(&row.path)));
        }
        cx.notify();
    }
}

/// Folders in the home directory that macOS guards with a permission prompt (or that hold
/// guarded data, as Pictures holds the Photos library). A walk that reaches one asks the person
/// for access to a folder they never opened, so the walk passes them by; a project inside one,
/// opened on purpose, still lists.
const GUARDED_IN_HOME: [&str; 7] = [
    "Desktop",
    "Documents",
    "Downloads",
    "Library",
    "Movies",
    "Music",
    "Pictures",
];

/// The files under `root`, relative to it: git's list in a repository, else a walk that skips
/// dot directories and the guarded folders of home.
fn list(root: &Path) -> Vec<PathBuf> {
    let mut files = match git_files(root) {
        Some(files) => files,
        None => walk(root, std::env::home_dir().as_deref()),
    };
    files.sort();
    files.dedup();
    files.truncate(MAX_FILES);
    files
}

fn git_files(root: &Path) -> Option<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        output
            .stdout
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| PathBuf::from(String::from_utf8_lossy(p).into_owned()))
            // A tracked file deleted in the worktree is still in git's index.
            .filter(|p| root.join(p).exists())
            .collect(),
    )
}

fn walk(root: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let guarded: Vec<PathBuf> = home
        .map(|home| GUARDED_IN_HOME.iter().map(|name| home.join(name)).collect())
        .unwrap_or_default();
    let mut files = Vec::new();
    let mut dirs = vec![PathBuf::new()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(root.join(&dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let path = dir.join(&name);
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    if !guarded.contains(&root.join(&path)) {
                        dirs.push(path);
                    }
                }
                Ok(kind) if kind.is_file() => files.push(path),
                _ => {}
            }
            if files.len() >= MAX_FILES {
                return files;
            }
        }
    }
    files
}

/// The tree's lines: at each level folders first, then files, each by name; a folder's
/// contents only while it is in `expanded`.
pub fn rows(files: &[PathBuf], expanded: &BTreeSet<PathBuf>) -> Vec<Row> {
    #[derive(Default)]
    struct Node {
        dirs: BTreeMap<String, Node>,
        files: BTreeSet<String>,
    }
    let mut top = Node::default();
    for file in files {
        let parts: Vec<String> = file
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let Some((name, dirs)) = parts.split_last() else {
            continue;
        };
        let mut node = &mut top;
        for dir in dirs {
            node = node.dirs.entry(dir.clone()).or_default();
        }
        node.files.insert(name.clone());
    }
    fn draw(
        node: &Node,
        at: &Path,
        depth: usize,
        expanded: &BTreeSet<PathBuf>,
        out: &mut Vec<Row>,
    ) {
        for (name, child) in &node.dirs {
            let path = at.join(name);
            let open = expanded.contains(&path);
            out.push(Row {
                path: path.clone(),
                depth,
                dir: true,
            });
            if open {
                draw(child, &path, depth + 1, expanded, out);
            }
        }
        for name in &node.files {
            out.push(Row {
                path: at.join(name),
                depth,
                dir: false,
            });
        }
    }
    let mut out = Vec::new();
    draw(&top, Path::new(""), 0, expanded, &mut out);
    out
}

impl Render for FileTree {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let mut list = div()
            .id("file-tree")
            .size_full()
            .flex()
            .flex_col()
            .py_1()
            .overflow_y_scroll()
            .text_ui()
            .text_color(colors.text);
        if self.files.is_empty() {
            let word = if self.loading.is_some() {
                "กำลังอ่านไฟล์"
            } else {
                "ไม่มีไฟล์"
            };
            return list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(colors.text_muted)
                    .child(word),
            );
        }
        for (n, row) in rows(&self.files, &self.expanded).into_iter().enumerate() {
            let name: SharedString = row
                .path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                .into();
            let open = row.dir && self.expanded.contains(&row.path);
            let selected = self.selected.as_ref() == Some(&row.path);
            let icon = match (row.dir, open) {
                (true, true) => "icons/chevron_down.svg",
                (true, false) => "icons/chevron_right.svg",
                (false, _) => "icons/file.svg",
            };
            let clicked = row.clone();
            list = list.child(
                div()
                    .id(("file", n))
                    .h(px(24.))
                    .flex_none()
                    .pl(px(8. + 14. * row.depth as f32))
                    .pr_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .when(selected, |r| r.bg(colors.element_selected))
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |tree, _, _, cx| tree.click(&clicked, cx)))
                    .child(
                        svg()
                            .path(icon)
                            .size(px(12.))
                            .flex_none()
                            .text_color(colors.text_muted),
                    )
                    .child(crate::tooltip::cut("name", name)),
            );
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_passes_by_the_guarded_folders_of_home() {
        let home = tempfile::tempdir().expect("dir");
        for file in [
            "Downloads/a.zip",
            "Desktop/b.txt",
            "code/main.rs",
            "notes.md",
        ] {
            let path = home.path().join(file);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, "").expect("write");
        }
        let mut files = walk(home.path(), Some(home.path()));
        files.sort();
        assert_eq!(
            files,
            [PathBuf::from("code/main.rs"), PathBuf::from("notes.md")]
        );
        // A project inside one, opened on purpose, lists.
        let files = walk(&home.path().join("Downloads"), Some(home.path()));
        assert_eq!(files, [PathBuf::from("a.zip")]);
    }

    #[test]
    fn folders_come_first_and_open_only_when_expanded() {
        let files: Vec<PathBuf> = [
            "b.txt",
            "src/main.rs",
            "src/ui/view.rs",
            "a.txt",
            "docs/x.md",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let shown = |expanded: &[&str]| -> Vec<String> {
            let expanded = expanded.iter().map(PathBuf::from).collect();
            rows(&files, &expanded)
                .into_iter()
                .map(|r| {
                    format!(
                        "{}{}{}",
                        "  ".repeat(r.depth),
                        r.path.display(),
                        if r.dir { "/" } else { "" }
                    )
                })
                .collect()
        };
        assert_eq!(shown(&[]), ["docs/", "src/", "a.txt", "b.txt"]);
        assert_eq!(
            shown(&["src", "src/ui"]),
            [
                "docs/",
                "src/",
                "  src/ui/",
                "    src/ui/view.rs",
                "  src/main.rs",
                "a.txt",
                "b.txt"
            ]
        );
        // A folder inside a folded one stays hidden even when it is marked open.
        assert_eq!(shown(&["src/ui"]), ["docs/", "src/", "a.txt", "b.txt"]);
    }

    #[test]
    fn a_repository_lists_what_git_would() {
        let dir = std::env::temp_dir().join(format!("cedian-files-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).expect("dir");
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("git")
        };
        git(&["init", "-q"]);
        std::fs::write(dir.join(".gitignore"), "target/\n").expect("ignore");
        std::fs::create_dir_all(dir.join("target")).expect("target");
        std::fs::write(dir.join("target/big"), "x").expect("big");
        std::fs::write(dir.join("src/lib.rs"), "").expect("lib");
        std::fs::write(dir.join("gone.txt"), "").expect("gone");
        git(&["add", "-A"]);
        std::fs::remove_file(dir.join("gone.txt")).expect("rm");
        assert_eq!(
            list(&dir),
            [PathBuf::from(".gitignore"), PathBuf::from("src/lib.rs")]
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
