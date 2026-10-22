//! The window's layout, kept in the `layout` key of `desktop` so a window opened after quit
//! looks the way the last one did: its bounds, the open tools, and the thread each project had
//! open.

use std::collections::BTreeMap;

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
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            bounds: None,
            tools: vec![Tool::Editor, Tool::Terminal],
            open: BTreeMap::new(),
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
        Self {
            bounds,
            tools,
            open,
        }
    }

    pub fn to_value(&self) -> Value {
        let mut layout = json!({
            "tools": self.tools.iter().map(|t| t.key()).collect::<Vec<_>>(),
            "open": self.open,
        });
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
        };
        let desktop = json!({ "layout": layout.to_value() });
        assert_eq!(Layout::from_desktop(&desktop), layout);
        assert_eq!(Layout::from_desktop(&json!({})), Layout::default());
        let old = json!({ "layout": { "right": "review" } });
        assert_eq!(Layout::from_desktop(&old).tools, [Tool::Review]);
    }
}
