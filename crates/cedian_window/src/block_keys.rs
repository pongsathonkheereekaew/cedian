//! The keys for blocks and layouts (Interface, Blocks: Keyboard). On macOS `⌘⌥` with an arrow
//! moves focus, `⌃⌥⇧` with an arrow moves the focused block (`⌃⌥↑` alone is the overview),
//! `⌘⌃` with an arrow grows it, `⌃⌘=` evens out, `⌘Z` undoes a layout change.

use gpui::{KeyBinding, actions};

actions!(
    blocks,
    [
        FocusLeft,
        FocusRight,
        FocusUp,
        FocusDown,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        GrowLeft,
        GrowRight,
        GrowUp,
        GrowDown,
        EvenOut,
        UndoLayout,
        Zoom,
        Overview,
        Park,
        SplitRight,
        SplitDown,
        Float,
        CloseBlock,
        ToggleThreads,
        OpenChat,
        OpenAgent,
        OpenTerminal,
        OpenBrowser,
        OpenReview,
        OpenEditor,
        Escape,
    ]
);

/// `⌘⌥1`–`⌘⌥9`: the nth layout of the project.
#[derive(Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui::Action)]
#[action(namespace = blocks)]
pub struct SwitchLayout(pub usize);

/// The keys, by what they do. Linux keeps `ctrl-w`, `ctrl-d`, `ctrl-z` and `ctrl-t` for the
/// shell, so those take `ctrl-alt`.
#[cfg(target_os = "macos")]
mod keys {
    pub const FOCUS: &str = "cmd-alt";
    pub const MOVE: &str = "ctrl-alt-shift";
    pub const GROW: &str = "cmd-ctrl";
    pub const EVEN: &str = "ctrl-cmd-=";
    pub const UNDO: &str = "cmd-z";
    pub const ZOOM: &str = "cmd-shift-enter";
    pub const OVERVIEW: &str = "ctrl-alt-up";
    pub const PARK: &str = "ctrl-shift-down";
    pub const SPLIT_RIGHT: &str = "cmd-d";
    pub const SPLIT_DOWN: &str = "cmd-shift-d";
    pub const FLOAT: &str = "cmd-alt-p";
    pub const CLOSE: &str = "cmd-w";
    pub const THREADS: &str = "cmd-\\";
    pub const CHAT: &str = "cmd-0";
    pub const OPEN: &str = "cmd-shift";
    pub const LAYOUT: &str = "cmd-alt";
}
#[cfg(not(target_os = "macos"))]
mod keys {
    pub const FOCUS: &str = "ctrl-alt";
    pub const MOVE: &str = "ctrl-alt-shift";
    pub const GROW: &str = "super-ctrl";
    pub const EVEN: &str = "ctrl-alt-=";
    pub const UNDO: &str = "ctrl-alt-z";
    pub const ZOOM: &str = "ctrl-shift-enter";
    pub const OVERVIEW: &str = "super-alt-up";
    pub const PARK: &str = "super-shift-down";
    pub const SPLIT_RIGHT: &str = "ctrl-alt-d";
    pub const SPLIT_DOWN: &str = "ctrl-alt-shift-d";
    pub const FLOAT: &str = "ctrl-alt-p";
    pub const CLOSE: &str = "ctrl-alt-w";
    pub const THREADS: &str = "ctrl-\\";
    pub const CHAT: &str = "ctrl-0";
    pub const OPEN: &str = "ctrl-alt-shift";
    pub const LAYOUT: &str = "ctrl-alt";
}
pub use keys::*;

/// A key as the menus show it.
pub fn shown(key: &str) -> String {
    if cfg!(target_os = "macos") {
        key.split('-')
            .map(|part| match part {
                "cmd" => "⌘",
                "ctrl" => "⌃",
                "alt" => "⌥",
                "shift" => "⇧",
                "enter" => "↩",
                "up" => "↑",
                "down" => "↓",
                "left" => "←",
                "right" => "→",
                "\\" => "\\",
                "=" => "=",
                other => other,
            })
            .map(|part| part.to_uppercase())
            .collect()
    } else {
        key.split('-')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => String::new(),
                }
            })
            .collect::<Vec<String>>()
            .join("+")
    }
}

/// The key that opens a tool: `⌘⇧A` Agent, `T` Terminal, `B` Browser, `R` Review, `E` Editor.
pub fn open_key(letter: char) -> String {
    format!("{OPEN}-{letter}")
}

/// Every binding, for the window and for the composer, where Zed's keymap would take some.
pub fn bindings() -> Vec<KeyBinding> {
    let mut out = Vec::new();
    for context in [None, Some("Composer > Editor")] {
        let arrows = ["left", "right", "up", "down"];
        let bind = |key: String, action: Box<dyn gpui::Action>| bind(&key, action, context);
        let focus: [Box<dyn gpui::Action>; 4] = [
            Box::new(FocusLeft),
            Box::new(FocusRight),
            Box::new(FocusUp),
            Box::new(FocusDown),
        ];
        let moves: [Box<dyn gpui::Action>; 4] = [
            Box::new(MoveLeft),
            Box::new(MoveRight),
            Box::new(MoveUp),
            Box::new(MoveDown),
        ];
        let grows: [Box<dyn gpui::Action>; 4] = [
            Box::new(GrowLeft),
            Box::new(GrowRight),
            Box::new(GrowUp),
            Box::new(GrowDown),
        ];
        for (arrow, ((f, m), g)) in arrows.iter().zip(focus.into_iter().zip(moves).zip(grows)) {
            out.extend(bind(format!("{FOCUS}-{arrow}"), f));
            out.extend(bind(format!("{MOVE}-{arrow}"), m));
            out.extend(bind(format!("{GROW}-{arrow}"), g));
        }
        let rest: Vec<(String, Box<dyn gpui::Action>)> = vec![
            (EVEN.to_owned(), Box::new(EvenOut)),
            (ZOOM.to_owned(), Box::new(Zoom)),
            (OVERVIEW.to_owned(), Box::new(Overview)),
            (PARK.to_owned(), Box::new(Park)),
            (SPLIT_RIGHT.to_owned(), Box::new(SplitRight)),
            (SPLIT_DOWN.to_owned(), Box::new(SplitDown)),
            (FLOAT.to_owned(), Box::new(Float)),
            (CLOSE.to_owned(), Box::new(CloseBlock)),
            (THREADS.to_owned(), Box::new(ToggleThreads)),
            (CHAT.to_owned(), Box::new(OpenChat)),
            (open_key('a'), Box::new(OpenAgent)),
            (open_key('t'), Box::new(OpenTerminal)),
            (open_key('b'), Box::new(OpenBrowser)),
            (open_key('r'), Box::new(OpenReview)),
            (open_key('e'), Box::new(OpenEditor)),
        ];
        for (key, action) in rest {
            out.extend(bind(key, action));
        }
        for n in 1..=9 {
            out.extend(bind(format!("{LAYOUT}-{n}"), Box::new(SwitchLayout(n))));
        }
    }
    // The layout's undo is the window's; an editor keeps its own `⌘Z`.
    out.extend(bind(UNDO, Box::new(UndoLayout), None));
    out.extend(bind("escape", Box::new(Escape), None));
    out
}

fn bind(key: &str, action: Box<dyn gpui::Action>, context: Option<&str>) -> Option<KeyBinding> {
    let context = context
        .and_then(|c| gpui::KeyBindingContextPredicate::parse(c).ok())
        .map(std::rc::Rc::new);
    KeyBinding::load(
        key,
        action,
        context,
        false,
        None,
        &gpui::DummyKeyboardMapper,
    )
    .inspect_err(|e| log::warn!("binding {key}: {e}"))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binding_parses() {
        // 15 single keys, 12 arrows and 9 layouts in each of two contexts, then undo and
        // escape.
        assert_eq!(bindings().len(), 2 * (15 + 12 + 9) + 2);
    }
}
