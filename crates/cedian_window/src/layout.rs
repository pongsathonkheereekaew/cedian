//! The window's layout, kept in the `layout` key of `desktop` so a window opened after quit
//! looks the way the last one did: its bounds, the open tools, the project tabs, and the thread
//! each project had open.

use std::{collections::BTreeMap, path::PathBuf};

use gpui::{App, Bounds, Pixels, WindowBounds, point, px, size};
use serde_json::{Value, json};

use crate::tool_rail::Tool;

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub bounds: Option<WindowBounds>,
    /// The tools open in the panel, in rail order.
    pub tools: Vec<Tool>,
    /// The open thread by project directory.
    pub open: BTreeMap<String, String>,
    /// Each tool's share of the panel's height, by key; a tool not named takes 1.
    pub tool_sizes: BTreeMap<String, f32>,
    /// The sidebar folded to its rail of dots (`⌘B`).
    pub sidebar_folded: bool,
    /// The threads pinned to the sidebar's ปักหมุด group, by project directory.
    pub pinned: BTreeMap<String, Vec<String>>,
    /// The project tabs' directories, in tab order.
    pub projects: Vec<String>,
    /// The tab in front.
    pub project: Option<String>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            bounds: None,
            tools: vec![Tool::Editor, Tool::Terminal],
            open: BTreeMap::new(),
            tool_sizes: BTreeMap::new(),
            sidebar_folded: false,
            pinned: BTreeMap::new(),
            projects: Vec::new(),
            project: None,
        }
    }
}

impl Layout {
    pub fn from_desktop(desktop: &Value) -> Self {
        let Some(layout) = desktop.get("layout") else {
            return Self::default();
        };
        let number = |key: &str| {
            layout
                .get(key)
                .and_then(Value::as_f64)
                .map(|n| px(n as f32))
        };
        let bounds = match (number("x"), number("y"), number("width"), number("height")) {
            (Some(x), Some(y), Some(w), Some(h)) if w > px(200.) && h > px(200.) => {
                let bounds = Bounds::new(point(x, y), size(w, h));
                Some(match layout.get("state").and_then(Value::as_str) {
                    Some("maximized") => WindowBounds::Maximized(bounds),
                    Some("fullscreen") => WindowBounds::Fullscreen(bounds),
                    _ => WindowBounds::Windowed(bounds),
                })
            }
            _ => None,
        };
        let open = layout
            .get("open")
            .and_then(Value::as_object)
            .map(|open| {
                open.iter()
                    .filter_map(|(cwd, id)| Some((cwd.clone(), id.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let tools = match layout.get("tools").and_then(Value::as_array) {
            Some(tools) => {
                let mut tools: Vec<Tool> = tools
                    .iter()
                    .filter_map(|t| Tool::from_key(t.as_str()?))
                    .collect();
                tools.sort();
                tools.dedup();
                tools
            }
            // Before the rail, the right column had two tabs.
            None => match layout.get("right").and_then(Value::as_str) {
                Some("review") => vec![Tool::Review],
                _ => Self::default().tools,
            },
        };
        let tool_sizes = layout
            .get("tool_sizes")
            .and_then(Value::as_object)
            .map(|sizes| {
                sizes
                    .iter()
                    .filter_map(|(key, n)| Some((key.clone(), n.as_f64()? as f32)))
                    .filter(|(_, n)| n.is_finite() && *n > 0.)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            bounds,
            tools,
            open,
            tool_sizes,
            sidebar_folded: layout.get("sidebar").and_then(Value::as_str) == Some("folded"),
            pinned: layout
                .get("pinned")
                .and_then(Value::as_object)
                .map(|pinned| {
                    pinned
                        .iter()
                        .filter_map(|(cwd, ids)| {
                            let ids: Vec<String> = ids
                                .as_array()?
                                .iter()
                                .filter_map(|id| Some(id.as_str()?.to_owned()))
                                .collect();
                            (!ids.is_empty()).then(|| (cwd.clone(), ids))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            projects: layout
                .get("projects")
                .and_then(Value::as_array)
                .map(|p| {
                    p.iter()
                        .filter_map(|cwd| Some(cwd.as_str()?.to_owned()))
                        .collect()
                })
                .unwrap_or_default(),
            project: layout
                .get("project")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }
    }

    /// The tabs to open and the one in front: the saved tabs whose directory is still there,
    /// with `asked` (the `--cwd`) in front, added when it is not one of them. With nothing
    /// asked and nothing saved, `here` alone.
    pub fn tabs(&self, asked: Option<PathBuf>, here: PathBuf) -> (Vec<PathBuf>, usize) {
        let mut tabs: Vec<PathBuf> = self
            .projects
            .iter()
            .map(PathBuf::from)
            .filter(|dir| dir.is_dir())
            .collect();
        tabs.dedup();
        let front = match asked {
            Some(dir) => match tabs.iter().position(|t| *t == dir) {
                Some(n) => n,
                None => {
                    tabs.push(dir);
                    tabs.len() - 1
                }
            },
            None if tabs.is_empty() => {
                tabs.push(here);
                0
            }
            None => self
                .project
                .as_ref()
                .and_then(|p| tabs.iter().position(|t| t.as_os_str() == p.as_str()))
                .unwrap_or(0),
        };
        (tabs, front)
    }

    /// Whether `thread` of the project at `cwd` is pinned.
    pub fn is_pinned(&self, cwd: &str, thread: &str) -> bool {
        self.pinned
            .get(cwd)
            .is_some_and(|ids| ids.iter().any(|id| id == thread))
    }

    /// Pins `thread` when it is not pinned, and unpins it when it is.
    pub fn toggle_pin(&mut self, cwd: &str, thread: &str) {
        let ids = self.pinned.entry(cwd.to_owned()).or_default();
        match ids.iter().position(|id| id == thread) {
            Some(n) => {
                ids.remove(n);
            }
            None => ids.push(thread.to_owned()),
        }
        if ids.is_empty() {
            self.pinned.remove(cwd);
        }
    }

    /// The share of the panel `tool` takes.
    pub fn tool_size(&self, tool: Tool) -> f32 {
        self.tool_sizes.get(tool.key()).copied().unwrap_or(1.)
    }

    pub fn to_value(&self) -> Value {
        let mut layout = json!({
            "tools": self.tools.iter().map(|t| t.key()).collect::<Vec<_>>(),
            "open": self.open,
            "sidebar": if self.sidebar_folded { "folded" } else { "open" },
            "projects": self.projects,
        });
        if !self.pinned.is_empty() {
            layout["pinned"] = json!(self.pinned);
        }
        if let Some(project) = &self.project {
            layout["project"] = project.as_str().into();
        }
        if !self.tool_sizes.is_empty() {
            // Two decimals keep the file readable; a drag never needs finer.
            let sizes: BTreeMap<&str, f64> = self
                .tool_sizes
                .iter()
                .map(|(k, n)| (k.as_str(), (f64::from(*n) * 100.).round() / 100.))
                .collect();
            layout["tool_sizes"] = json!(sizes);
        }
        if let Some(bounds) = self.bounds {
            let (state, b) = match bounds {
                WindowBounds::Windowed(b) => ("windowed", b),
                WindowBounds::Maximized(b) => ("maximized", b),
                WindowBounds::Fullscreen(b) => ("fullscreen", b),
            };
            let n = |p: Pixels| f64::from(f32::from(p)).round();
            layout["state"] = state.into();
            layout["x"] = n(b.origin.x).into();
            layout["y"] = n(b.origin.y).into();
            layout["width"] = n(b.size.width).into();
            layout["height"] = n(b.size.height).into();
        }
        layout
    }

    /// The saved bounds when some display still shows most of them.
    pub fn bounds_on_screen(&self, cx: &App) -> Option<WindowBounds> {
        let bounds = self.bounds?;
        let b = bounds.get_bounds();
        let visible = cx.displays().iter().any(|display| {
            let shared = display.bounds().intersect(&b);
            shared.size.width * 2. > b.size.width && shared.size.height * 2. > b.size.height
        });
        visible.then_some(bounds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_reads_back_what_it_wrote() {
        let layout = Layout {
            bounds: Some(WindowBounds::Maximized(Bounds::new(
                point(px(10.), px(20.)),
                size(px(1200.), px(700.)),
            ))),
            tools: vec![Tool::Review, Tool::Details],
            open: BTreeMap::from([("/repo".to_owned(), "t-1".to_owned())]),
            tool_sizes: BTreeMap::from([("review".to_owned(), 1.25)]),
            sidebar_folded: true,
            pinned: BTreeMap::from([("/repo".to_owned(), vec!["t-2".to_owned()])]),
            projects: vec!["/repo".to_owned(), "/other".to_owned()],
            project: Some("/other".to_owned()),
        };
        let desktop = json!({ "layout": layout.to_value() });
        assert_eq!(Layout::from_desktop(&desktop), layout);
        assert_eq!(Layout::from_desktop(&json!({})), Layout::default());
        let old = json!({ "layout": { "right": "review" } });
        assert_eq!(Layout::from_desktop(&old).tools, [Tool::Review]);
    }

    #[test]
    fn tabs_open_where_they_were_left() {
        let dir = std::env::temp_dir().join(format!("cedian-tabs-{}", std::process::id()));
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::fs::create_dir_all(&a).expect("a");
        std::fs::create_dir_all(&b).expect("b");
        let gone = dir.join("gone");
        let s = |p: &PathBuf| p.display().to_string();
        let layout = Layout {
            projects: vec![s(&a), s(&gone), s(&b)],
            project: Some(s(&b)),
            ..Layout::default()
        };
        let here = PathBuf::from("/here");
        assert_eq!(
            layout.tabs(None, here.clone()),
            (vec![a.clone(), b.clone()], 1)
        );
        assert_eq!(
            layout.tabs(Some(a.clone()), here.clone()),
            (vec![a.clone(), b.clone()], 0)
        );
        let c = dir.join("c");
        assert_eq!(
            layout.tabs(Some(c.clone()), here.clone()),
            (vec![a.clone(), b.clone(), c], 2)
        );
        assert_eq!(Layout::default().tabs(None, here.clone()), (vec![here], 0));
        std::fs::remove_dir_all(&dir).ok();
    }
}
