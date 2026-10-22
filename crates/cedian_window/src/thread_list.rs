//! The sidebar's thread list: threads in groups by what they need from the person, each row a
//! state dot, the title and a line with the state and when (Interface, Layout and Thread
//! states as drawn).

use cedian_wire::{Status, ThreadInfo};
use chrono::{DateTime, Utc};
use gpui::{App, Div, Hsla, ParentElement as _, Styled as _, div, px};
use theme::ActiveTheme as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Working,
    NeedsYou,
    Finished,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Working, Group::NeedsYou, Group::Finished];

    pub fn of(info: &ThreadInfo) -> Self {
        match info.status {
            Status::Working => Group::Working,
            Status::Asking | Status::Failed | Status::Interrupted => Group::NeedsYou,
            Status::Done if info.needs_you => Group::NeedsYou,
            Status::Done | Status::Stopped => Group::Finished,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Group::Working => "กำลังทำ",
            Group::NeedsYou => "ต้องดู",
            Group::Finished => "เสร็จแล้ว",
        }
    }
}

/// The threads the badges count: those that changed since the person last opened them.
pub fn badge_count(threads: &[ThreadInfo]) -> usize {
    threads.iter().filter(|t| t.needs_you).count()
}

pub fn state_word(status: Status) -> &'static str {
    match status {
        Status::Working => "กำลังทำ",
        Status::Done => "เสร็จ",
        Status::Failed => "ล้มเหลว",
        Status::Stopped => "หยุดแล้ว",
        Status::Interrupted => "ขาดตอน",
        Status::Asking => "ถามคุณ",
    }
}

/// How long ago `at` (RFC 3339) was, in short Thai; empty when it does not parse.
pub fn since(at: &str, now: DateTime<Utc>) -> String {
    let Ok(at) = DateTime::parse_from_rfc3339(at) else {
        return String::new();
    };
    let minutes = (now - at.with_timezone(&Utc)).num_minutes().max(0);
    match minutes {
        0 => "เมื่อสักครู่".into(),
        1..=59 => format!("{minutes} นาที"),
        60..=1439 => format!("{} ชม.", minutes / 60),
        _ => format!("{} วัน", minutes / 1440),
    }
}

/// The state's dot, which differs by shape as well as color: solid while working, failed or
/// stopped, hollow when done or interrupted, a ring when it asks.
pub fn dot(status: Status, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let state = cx.theme().status();
    let (color, solid): (Hsla, bool) = match status {
        Status::Working => (state.info, true),
        Status::Done => (colors.text_placeholder, false),
        Status::Failed => (state.error, true),
        Status::Stopped => (colors.text_muted, true),
        Status::Interrupted => (state.warning, false),
        Status::Asking => (colors.text_accent, false),
    };
    let base = div().flex_none().size(px(8.)).rounded_full();
    match (solid, status) {
        (true, _) => base.bg(color),
        (false, Status::Asking) => base.border_2().border_color(color),
        (false, _) => base.border_1().border_color(color),
    }
}

/// A count in a pill: `failed` when any counted thread failed, else the accent.
pub fn badge(count: usize, failed: bool, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let fill = if failed {
        cx.theme().status().error
    } else {
        colors.text_accent
    };
    div()
        .flex_none()
        .px(px(5.))
        .min_w(px(16.))
        .h(px(16.))
        .rounded(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .bg(fill)
        .text_color(colors.background)
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(count.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(status: Status, needs_you: bool) -> ThreadInfo {
        ThreadInfo {
            thread: serde_json::from_value(serde_json::json!({
                "id": "t-1", "title": "x", "task": "x", "state": "done", "result": null,
                "parent": null, "archived": false, "sessions": [], "messages": [],
                "pending": [], "updated_at": "2026-10-22T03:00:00.000Z"
            }))
            .expect("summary"),
            status,
            cwd: None,
            needs_you,
        }
    }

    #[test]
    fn threads_fall_into_the_groups_of_the_spec() {
        assert_eq!(Group::of(&info(Status::Working, false)), Group::Working);
        assert_eq!(Group::of(&info(Status::Asking, false)), Group::NeedsYou);
        assert_eq!(Group::of(&info(Status::Failed, false)), Group::NeedsYou);
        assert_eq!(
            Group::of(&info(Status::Interrupted, false)),
            Group::NeedsYou
        );
        assert_eq!(Group::of(&info(Status::Done, true)), Group::NeedsYou);
        assert_eq!(Group::of(&info(Status::Done, false)), Group::Finished);
        assert_eq!(Group::of(&info(Status::Stopped, false)), Group::Finished);
    }

    #[test]
    fn badges_count_only_what_changed_since_it_was_seen() {
        let threads = [
            info(Status::Done, true),
            info(Status::Failed, false),
            info(Status::Asking, true),
        ];
        assert_eq!(badge_count(&threads), 2);
    }

    #[test]
    fn since_reads_in_thai() {
        let now = DateTime::parse_from_rfc3339("2026-10-22T05:00:00.000Z")
            .expect("now")
            .with_timezone(&Utc);
        assert_eq!(since("2026-10-22T04:59:30.000Z", now), "เมื่อสักครู่");
        assert_eq!(since("2026-10-22T04:45:00.000Z", now), "15 นาที");
        assert_eq!(since("2026-10-22T02:00:00.000Z", now), "3 ชม.");
        assert_eq!(since("2026-10-20T05:00:00.000Z", now), "2 วัน");
        assert_eq!(since("garbage", now), "");
    }
}
