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
        let open = kind.one_per_layout() && ws.find(kind).is_some();
        let note = if !built(kind) {
            Some("เร็วๆ นี้".to_owned())
        } else if open {
            Some("เปิดอยู่".to_owned())
        } else {
            key(kind)
        };
        Row::Item {
            icon: Some(icon(kind)),
            label: label.to_owned().into(),
            note: note.map(Into::into),
            on_click: built(kind).then(|| pick(kind)),
        }
    };
    vec![
        Row::Label("ของ thread ที่เปิดอยู่".into()),
        row(Kind::Agent, "Session"),
        row(Kind::Terminal, "Terminal"),
        row(Kind::Browser, "Browser"),
        row(Kind::Review, "Review"),
        row(Kind::Editor, "Editor"),
        row(Kind::Files, "Files"),
        row(Kind::Git, "Git"),
        row(Kind::Session, "รายละเอียด thread"),
        Row::Separator,
        Row::Label("ของ project".into()),
        row(Kind::Threads, "Threads"),
        row(Kind::Chat, "แชทโปรเจกต์"),
        row(Kind::Outputs, "Outputs"),
        row(Kind::Memory, "Memory"),
        row(Kind::Usage, "Usage"),
        row(Kind::Background, "Background tasks"),
    ]
}
