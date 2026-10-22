//! Search the files' contents (`⌘⇧F`): a field over the window, as `⌘P`'s, that finds the
//! lines holding what is typed, ignoring case, in every file of the open thread's directory.
//! In a repository it asks `git grep` (tracked files and untracked ones not ignored, binary
//! files skipped); elsewhere it reads the files the Files block lists. Up and Down move, Enter
//! shows the file in the Editor at that line, Escape closes.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use editor::{Editor, EditorEvent};
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, StyledText, Task, Window, actions, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::{tokens, type_scale::TypeScale as _};

actions!(
    content_search,
    [SearchContents, Next, Previous, Pick, Close]
);

/// Lines found before the search stops; the rest are not worth a list.
const MAX_HITS: usize = 300;
/// Rows drawn; the list scrolls past them.
const WIDTH: f32 = 720.;
/// Fewer letters than this find too much to be useful.
const MIN_QUERY: usize = 2;
/// Typing faster than this starts one search, not one per key.
const SETTLE: Duration = Duration::from_millis(120);
/// A line longer than this is cut around its match.
const LINE_CHARS: usize = 160;

pub enum ContentEvent {
    /// Show this file (an absolute path) at this line, counted from 1.
    Open {
        path: PathBuf,
        line: u32,
    },
    Closed,
}

/// A line that holds the query.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Relative to the searched directory.
    pub path: String,
    /// Counted from 1.
    pub line: u32,
    pub text: String,
}

pub struct ContentSearch {
    input: Entity<Editor>,
    root: PathBuf,
    files: Arc<Vec<PathBuf>>,
    hits: Vec<Hit>,
    /// The query the hits are for.
    found_for: String,
    searching: bool,
    selected: usize,
    cancel: Arc<AtomicBool>,
    search: Option<Task<()>>,
}

impl EventEmitter<ContentEvent> for ContentSearch {}

impl Focusable for ContentSearch {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl ContentSearch {
    /// `files` are relative to `root`, read when `root` is not a repository.
    pub fn new(
        root: PathBuf,
        files: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("ค้นหาข้อความในไฟล์", window, cx);
            editor
        });
        cx.subscribe(&input, |search, _, event, cx| {
            if matches!(event, EditorEvent::Edited { .. }) {
                search.find(cx);
            }
        })
        .detach();
        Self {
            input,
            root,
            files: Arc::new(files),
            hits: Vec::new(),
            found_for: String::new(),
            searching: false,
            selected: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            search: None,
        }
    }

    fn query(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text(cx)
    }

    /// Searches for the query once typing settles; a newer query cancels the one before.
    fn find(&mut self, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        let query = self.query(cx);
        if query.trim().chars().count() < MIN_QUERY {
            self.hits.clear();
            self.found_for.clear();
            self.searching = false;
            self.search = None;
            cx.notify();
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        self.searching = true;
        let (root, files) = (self.root.clone(), self.files.clone());
        self.search = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let wanted = query.clone();
            let stop = cancel.clone();
            let hits = cx
                .background_executor()
                .spawn(async move { search(&root, &files, &wanted, &stop) })
                .await;
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            this.update(cx, |search, cx| {
                search.hits = hits;
                search.found_for = query;
                search.searching = false;
                search.selected = 0;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn next(&mut self, _: &Next, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected + 1 < self.hits.len() {
            self.selected += 1;
            cx.notify();
        }
    }

    fn previous(&mut self, _: &Previous, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = self.selected.saturating_sub(1);
        cx.notify();
    }

    fn pick(&mut self, _: &Pick, _: &mut Window, cx: &mut Context<Self>) {
        self.open(self.selected, cx);
    }

    fn open(&mut self, row: usize, cx: &mut Context<Self>) {
        if let Some(hit) = self.hits.get(row) {
            cx.emit(ContentEvent::Open {
                path: self.root.join(&hit.path),
                line: hit.line,
            });
        }
    }

    fn close(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(ContentEvent::Closed);
    }
}

/// The lines under `root` that hold `query`, ignoring case: `git grep` in a repository, else a
/// read of `files`. Stops at `MAX_HITS` or when `cancel` is set.
pub fn search(root: &Path, files: &[PathBuf], query: &str, cancel: &AtomicBool) -> Vec<Hit> {
    git_grep(root, query).unwrap_or_else(|| read_files(root, files, query, cancel))
}

fn git_grep(root: &Path, query: &str) -> Option<Vec<Hit>> {
    let output = Command::new("git")
        .args([
            "grep",
            "--untracked",
            "--no-color",
            "-I",
            "-i",
            "-n",
            "-F",
            "-z",
            "-e",
            query,
        ])
        .current_dir(root)
        .output()
        .ok()?;
    // 1 is "nothing found"; anything else is not a repository or a git that failed.
    match output.status.code() {
        Some(0) => {}
        Some(1) => return Some(Vec::new()),
        _ => return None,
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(
        text.lines()
            .filter_map(|line| {
                // With -z: path NUL line NUL text.
                let mut parts = line.splitn(3, '\0');
                let path = parts.next()?;
                let number = parts.next()?.parse().ok()?;
                let text = parts.next()?;
                Some(Hit {
                    path: path.to_owned(),
                    line: number,
                    text: text.to_owned(),
                })
            })
            .take(MAX_HITS)
            .collect(),
    )
}

fn read_files(root: &Path, files: &[PathBuf], query: &str, cancel: &AtomicBool) -> Vec<Hit> {
    let wanted = query.to_lowercase();
    let mut hits = Vec::new();
    for path in files {
        if cancel.load(Ordering::Relaxed) || hits.len() >= MAX_HITS {
            break;
        }
        let Ok(bytes) = std::fs::read(root.join(path)) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for (n, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&wanted) {
                hits.push(Hit {
                    path: path.to_string_lossy().into_owned(),
                    line: n as u32 + 1,
                    text: line.to_owned(),
                });
                if hits.len() >= MAX_HITS {
                    break;
                }
            }
        }
    }
    hits
}

/// The line with its matches marked.
fn shown(text: &str, query: &str, style: HighlightStyle) -> StyledText {
    let (line, marks) = cut(text, query);
    StyledText::new(line).with_highlights(marks.into_iter().map(|range| (range, style)))
}

/// The line, trimmed and cut around the first match when it is long, and where its matches
/// are in what is left.
fn cut(text: &str, query: &str) -> (String, Vec<std::ops::Range<usize>>) {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    let wanted = query.to_lowercase();
    // Lowercasing can change byte lengths; mark only when it did not.
    let same = lower.len() == trimmed.len();
    let first = if same { lower.find(&wanted) } else { None };
    let start = match first {
        Some(at) if trimmed[..at].chars().count() > LINE_CHARS / 3 => {
            let skip = trimmed[..at].chars().count() - LINE_CHARS / 3;
            trimmed.char_indices().nth(skip).map_or(0, |(byte, _)| byte)
        }
        _ => 0,
    };
    let end = trimmed[start..]
        .char_indices()
        .nth(LINE_CHARS)
        .map_or(trimmed.len(), |(byte, _)| start + byte);
    let lead = if start > 0 { "…" } else { "" };
    let line = format!("{lead}{}", &trimmed[start..end]);
    let mut marks = Vec::new();
    if same && !wanted.is_empty() {
        let window = &lower[start..end];
        let mut from = 0;
        while let Some(at) = window[from..].find(&wanted) {
            let begin = lead.len() + from + at;
            marks.push(begin..begin + wanted.len());
            from += at + wanted.len();
        }
    }
    (line, marks)
}

impl Render for ContentSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let marked = HighlightStyle {
            color: Some(colors.text_accent),
            font_weight: Some(gpui::FontWeight::SEMIBOLD),
            ..Default::default()
        };
        let query = self.found_for.clone();
        let status = if self.searching {
            "กำลังค้นหา…".to_owned()
        } else if query.is_empty() {
            format!("พิมพ์อย่างน้อย {MIN_QUERY} ตัวอักษร")
        } else if self.hits.len() >= MAX_HITS {
            format!("{MAX_HITS}+ บรรทัด")
        } else {
            format!("{} บรรทัด", self.hits.len())
        };
        let mut list = div()
            .id("content-hits")
            .flex()
            .flex_col()
            .gap_px()
            .p_1()
            .max_h(px(440.))
            .overflow_y_scroll();
        if !self.searching && !query.is_empty() && self.hits.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_2()
                    .text_meta()
                    .text_color(colors.text_muted)
                    .child("ไม่พบข้อความนี้"),
            );
        }
        for (row, hit) in self.hits.iter().enumerate() {
            list = list.child(
                div()
                    .id(("hit", row))
                    .px_2()
                    .py(px(4.))
                    .flex()
                    .flex_col()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(row == self.selected, |r| r.bg(colors.element_selected))
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |search, _, _, cx| search.open(row, cx)))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_label()
                            .text_color(colors.text_muted)
                            .child(format!("{}:{}", hit.path, hit.line)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_ui()
                            .font(tokens::code_font())
                            .child(shown(&hit.text, &query, marked)),
                    ),
            );
        }
        div()
            .key_context("ContentSearch")
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::pick))
            .on_action(cx.listener(Self::close))
            .debug_selector(|| "content-search".into())
            .w(px(WIDTH))
            .max_w_full()
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_lg()
            .overflow_hidden()
            .child(
                div()
                    .h(px(40.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(colors.border_variant)
                    .child(
                        svg()
                            .path("icons/magnifying_glass.svg")
                            .size(px(14.))
                            .text_color(colors.text_muted),
                    )
                    .child(div().flex_grow(1.).min_w_0().child(self.input.clone()))
                    .child(
                        div()
                            .text_label()
                            .text_color(colors.text_muted)
                            .child(status),
                    ),
            )
            .child(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_git_is_read_and_the_line_numbers_count_from_one() {
        let dir = std::env::temp_dir().join(format!("cedian-content-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("a.txt"), "first\nHello World\nhello again\n").expect("a");
        std::fs::write(dir.join("b.bin"), [0u8, 1, 2]).expect("b");
        let files = vec![PathBuf::from("a.txt"), PathBuf::from("b.bin")];
        let hits = read_files(&dir, &files, "HELLO", &AtomicBool::new(false));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            hits.iter().map(|h| h.line).collect::<Vec<_>>(),
            [2, 3],
            "{hits:?}"
        );
        assert_eq!(hits[0].text, "Hello World");
    }

    #[test]
    fn git_grep_finds_tracked_and_untracked_lines() {
        let dir = std::env::temp_dir().join(format!("cedian-grep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("git")
        };
        git(&["init", "-q"]);
        std::fs::write(dir.join("tracked.rs"), "fn main() {}\nlet needle = 1;\n").expect("t");
        git(&["add", "tracked.rs"]);
        std::fs::write(dir.join("new.rs"), "// a Needle here\n").expect("n");
        let mut hits = git_grep(&dir, "needle").expect("a repository");
        std::fs::remove_dir_all(&dir).ok();
        hits.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(
            hits.iter()
                .map(|h| (h.path.as_str(), h.line))
                .collect::<Vec<_>>(),
            [("new.rs", 1), ("tracked.rs", 2)]
        );
    }

    #[test]
    fn a_long_line_is_cut_around_its_match() {
        let long = format!("{}needle{}", "x".repeat(300), "y".repeat(300));
        let (line, marks) = cut(&long, "NEEDLE");
        assert!(line.starts_with('…'));
        assert_eq!(marks.len(), 1);
        assert_eq!(&line[marks[0].clone()], "needle");
        assert!(line.chars().count() <= LINE_CHARS + 1);
    }
}
