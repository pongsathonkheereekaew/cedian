//! The Git tool (Interface, Layout: tool panel), read only: the open thread's directory's
//! branch and how far it is from its upstream, the files changed against HEAD, and the latest
//! commits. A click on a changed file shows its diff against HEAD in the Editor. Committing
//! and pull requests are cedia's (`/commit`, `/pr`), so the tool has no buttons for them.

use std::path::{Path, PathBuf};

use crate::type_scale::TypeScale as _;
use gpui::{
    Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Task, Window, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

/// Commits listed under the changes.
const COMMITS: usize = 10;

pub enum GitEvent {
    /// Show this text in the Editor under this name.
    Show { name: String, text: String },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitState {
    /// `None` outside a repository.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub changes: Vec<Change>,
    pub commits: Vec<Commit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// git's two status letters: the index's, then the worktree's (`??` untracked).
    pub code: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
    pub when: String,
}

#[derive(Default)]
pub struct GitPane {
    dir: PathBuf,
    state: GitState,
    loading: Option<Task<()>>,
}

impl EventEmitter<GitEvent> for GitPane {}

impl GitPane {
    pub fn set_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if dir != self.dir {
            self.dir = dir;
            self.state = GitState::default();
            self.refresh(cx);
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.dir.as_os_str().is_empty() {
            return;
        }
        let dir = self.dir.clone();
        self.loading = Some(cx.spawn(async move |this, cx| {
            let read = dir.clone();
            let state = cx
                .background_executor()
                .spawn(async move { read_state(&read) })
                .await;
            this.update(cx, |pane, cx| {
                if pane.dir == dir {
                    pane.state = state;
                    pane.loading = None;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn show_diff(&mut self, change: &Change, cx: &mut Context<Self>) {
        let dir = self.dir.clone();
        let change = change.clone();
        cx.spawn(async move |this, cx| {
            let path = change.path.clone();
            let text = cx
                .background_executor()
                .spawn(async move { diff(&dir, &change) })
                .await;
            this.update(cx, |_, cx| {
                cx.emit(GitEvent::Show {
                    name: format!("diff · {path}"),
                    text,
                })
            })
            .ok();
        })
        .detach();
    }
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read_state(dir: &Path) -> GitState {
    let Some(status) = git(
        dir,
        &[
            "status",
            "--porcelain=v1",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
    ) else {
        return GitState::default();
    };
    let mut state = parse_status(&status);
    if let Some(log) = git(
        dir,
        &["log", &format!("-n{COMMITS}"), "--format=%h%x1f%s%x1f%cr"],
    ) {
        state.commits = parse_log(&log);
    }
    state
}

/// `git status --porcelain=v1 --branch -z`: a `## ` branch entry, then one entry per file, a
/// rename or copy followed by the path it came from.
pub fn parse_status(text: &str) -> GitState {
    let mut state = GitState::default();
    let mut entries = text.split('\0').filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        if let Some(head) = entry.strip_prefix("## ") {
            parse_branch(head, &mut state);
            continue;
        }
        let (code, path) = entry.split_at(entry.len().min(3));
        let code = code.trim_end().to_owned();
        if code.starts_with('R') || code.starts_with('C') {
            entries.next();
        }
        state.changes.push(Change {
            code: format!("{code:>2}"),
            path: path.to_owned(),
        });
    }
    state
}

fn parse_branch(head: &str, state: &mut GitState) {
    let (names, counts) = match head.split_once(" [") {
        Some((names, counts)) => (names, counts.trim_end_matches(']')),
        None => (head, ""),
    };
    let (branch, upstream) = match names.split_once("...") {
        Some((branch, upstream)) => (branch, Some(upstream)),
        None => (names, None),
    };
    let branch = branch
        .strip_prefix("No commits yet on ")
        .unwrap_or(branch)
        .to_owned();
    state.branch = Some(if branch == "HEAD (no branch)" {
        "HEAD".to_owned()
    } else {
        branch
    });
    state.upstream = upstream.map(str::to_owned);
    for count in counts.split(", ") {
        if let Some(n) = count.strip_prefix("ahead ") {
            state.ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = count.strip_prefix("behind ") {
            state.behind = n.parse().unwrap_or(0);
        }
    }
}

pub fn parse_log(text: &str) -> Vec<Commit> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            Some(Commit {
                sha: parts.next()?.to_owned(),
                subject: parts.next()?.to_owned(),
                when: parts.next().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// The file's diff against HEAD; an untracked file against nothing.
fn diff(dir: &Path, change: &Change) -> String {
    let text = if change.code == "??" {
        // --no-index exits 1 when the files differ, which they always do here.
        std::process::Command::new("git")
            .args(["diff", "--no-index", "--", "/dev/null", &change.path])
            .current_dir(dir)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        git(dir, &["diff", "HEAD", "--", &change.path])
    };
    match text {
        Some(text) if !text.is_empty() => text,
        Some(_) => format!("{} has no changes against HEAD", change.path),
        None => format!("git could not diff {}", change.path),
    }
}

impl Render for GitPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let status = cx.theme().status().clone();
        let mut pane = div()
            .id("git")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .text_ui()
            .text_color(colors.text);
        let Some(branch) = self.state.branch.clone() else {
            let word = if self.loading.is_some() {
                "กำลังอ่าน git"
            } else {
                "ไม่ใช่ git repository"
            };
            return pane.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(colors.text_muted)
                    .child(word),
            );
        };
        let state = &self.state;
        let mut far = Vec::new();
        if state.ahead > 0 {
            far.push(format!("↑{}", state.ahead));
        }
        if state.behind > 0 {
            far.push(format!("↓{}", state.behind));
        }
        let heading = |text: String| {
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .text_label()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.text_muted)
                .child(text)
        };
        pane = pane.child(
            div()
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .gap_2()
                .border_b_1()
                .border_color(colors.border)
                .child(
                    svg()
                        .path("icons/git_branch.svg")
                        .size(px(14.))
                        .text_color(colors.text_muted),
                )
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(branch))
                .when_some(state.upstream.clone(), |row, upstream| {
                    row.child(
                        div()
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(upstream),
                    )
                })
                .when(!far.is_empty(), |row| {
                    row.child(
                        div()
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(far.join(" ")),
                    )
                }),
        );
        pane = pane.child(heading(if state.changes.is_empty() {
            "ไม่มีไฟล์ที่เปลี่ยน".to_owned()
        } else {
            format!("เปลี่ยน {} ไฟล์", state.changes.len())
        }));
        for (n, change) in state.changes.iter().enumerate() {
            let code_color = match change.code.trim() {
                "??" | "A" => status.created,
                "D" => status.deleted,
                _ => status.modified,
            };
            let clicked = change.clone();
            pane = pane.child(
                div()
                    .id(("change", n))
                    .h(px(24.))
                    .flex_none()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |pane, _, _, cx| pane.show_diff(&clicked, cx)))
                    .child(
                        div()
                            .w(px(20.))
                            .flex_none()
                            .font_family("Lilex")
                            .text_meta()
                            .text_color(code_color)
                            .child(SharedString::from(change.code.clone())),
                    )
                    .child(crate::tooltip::cut("path", change.path.clone())),
            );
        }
        if !state.commits.is_empty() {
            pane = pane.child(heading("commit ล่าสุด".to_owned()));
        }
        for commit in &state.commits {
            pane = pane.child(
                div()
                    .h(px(24.))
                    .flex_none()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_none()
                            .font_family("Lilex")
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(commit.sha.clone()),
                    )
                    .child(crate::tooltip::cut("subject", commit.subject.clone()).flex_grow(1.))
                    .child(
                        div()
                            .flex_none()
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(commit.when.clone()),
                    ),
            );
        }
        pane
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_the_branch_and_each_change() {
        let text = "## main...origin/main [ahead 2, behind 1]\0 M src/a.rs\0R  new.rs\0old.rs\0?? notes.md\0D  gone.txt\0";
        let state = parse_status(text);
        assert_eq!(state.branch.as_deref(), Some("main"));
        assert_eq!(state.upstream.as_deref(), Some("origin/main"));
        assert_eq!((state.ahead, state.behind), (2, 1));
        let changes: Vec<(&str, &str)> = state
            .changes
            .iter()
            .map(|c| (c.code.as_str(), c.path.as_str()))
            .collect();
        assert_eq!(
            changes,
            [
                (" M", "src/a.rs"),
                (" R", "new.rs"),
                ("??", "notes.md"),
                (" D", "gone.txt")
            ]
        );
        let fresh = parse_status("## No commits yet on main\0");
        assert_eq!(fresh.branch.as_deref(), Some("main"));
        assert_eq!(fresh.upstream, None);
    }

    #[test]
    fn a_repository_reads_back_its_changes_and_commits() {
        let dir = std::env::temp_dir().join(format!("cedian-git-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("git")
        };
        run(&["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.txt"), "one\n").expect("a");
        run(&["add", "a.txt"]);
        run(&["commit", "-q", "-m", "first"]);
        std::fs::write(dir.join("a.txt"), "two\n").expect("a");
        std::fs::write(dir.join("b.txt"), "new\n").expect("b");
        let state = read_state(&dir);
        assert_eq!(state.branch.as_deref(), Some("main"));
        assert_eq!(state.changes.len(), 2);
        assert_eq!(state.commits.len(), 1);
        assert_eq!(state.commits[0].subject, "first");
        let modified = diff(&dir, &state.changes[0]);
        assert!(
            modified.contains("-one") && modified.contains("+two"),
            "{modified}"
        );
        let added = diff(&dir, &state.changes[1]);
        assert!(added.contains("+new"), "{added}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
