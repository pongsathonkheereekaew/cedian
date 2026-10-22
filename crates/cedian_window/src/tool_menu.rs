//! The + Tool menu in the title bar (Interface, Blocks): the tools to open as blocks, for the
//! open thread and for the project, each with its key. A tool a layout holds once and already
//! has says เปิดอยู่, and picking it focuses that block. It opens on hover after 120 ms and
//! closes 220 ms after the pointer leaves both the button and the menu.

use std::time::Duration;

use cedian_layout::{Kind, Workspace};

use crate::{block_header::icon, block_keys, on_click::OnClick, popup_menu::Row};

pub const OPEN_AFTER: Duration = Duration::from_millis(120);
pub const CLOSE_AFTER: Duration = Duration::from_millis(220);

/// Whether cedian draws this kind yet; the rest are in the spec and say so.
pub fn built(kind: Kind) -> bool {
    !matches!(
        kind,
        Kind::Browser | Kind::Outputs | Kind::Memory | Kind::Usage | Kind::Background
    )
}

/// The key that opens or focuses `kind`, when it has one.
pub fn key(kind: Kind) -> Option<String> {
    let letter = match kind {
        Kind::Agent => 'a',
        Kind::Terminal => 't',
        Kind::Browser => 'b',
        Kind::Review => 'r',
        Kind::Editor => 'e',
        Kind::Chat => return Some(block_keys::shown(block_keys::CHAT)),
        Kind::Threads => return Some(block_keys::shown(block_keys::THREADS)),
        _ => return None,
    };
    Some(block_keys::shown(&block_keys::open_key(letter)))
}

pub fn rows(ws: &Workspace, pick: impl Fn(Kind) -> OnClick) -> Vec<Row> {
    let row = |kind: Kind, label: &str| {
        let note = if built(kind) {
            note(ws, kind)
        } else {
            Some("เร็วๆ นี้".to_owned())
        };
        Row::Item {
            icon: Some(icon(kind)),
            label: label.to_owned().into(),
            note: note.map(Into::into),
            on_click: built(kind).then(|| pick(kind)),
        }
    };
    let mut rows = vec![Row::Label("ของ thread ที่เปิดอยู่".into())];
    rows.extend(THREAD_TOOLS.iter().map(|(kind, label)| row(*kind, label)));
    rows.extend([Row::Separator, Row::Label("ของ project".into())]);
    rows.extend(PROJECT_TOOLS.iter().map(|(kind, label)| row(*kind, label)));
    rows
}

/// The tools for the open thread, in the menu's order.
pub const THREAD_TOOLS: [(Kind, &str); 8] = [
    (Kind::Agent, "Session"),
    (Kind::Terminal, "Terminal"),
    (Kind::Browser, "Browser"),
    (Kind::Review, "Review"),
    (Kind::Editor, "Editor"),
    (Kind::Files, "Files"),
    (Kind::Git, "Git"),
    (Kind::Session, "รายละเอียด thread"),
];

/// The tools for the project.
pub const PROJECT_TOOLS: [(Kind, &str); 6] = [
    (Kind::Threads, "Threads"),
    (Kind::Chat, "แชทโปรเจกต์"),
    (Kind::Outputs, "Outputs"),
    (Kind::Memory, "Memory"),
    (Kind::Usage, "Usage"),
    (Kind::Background, "Background tasks"),
];

/// The note a tool's row carries: เปิดอยู่ for one the layout already shows, else its key.
pub fn note(ws: &Workspace, kind: Kind) -> Option<String> {
    if kind.one_per_layout() && ws.find(kind).is_some() {
        Some("เปิดอยู่".to_owned())
    } else {
        key(kind)
    }
}
