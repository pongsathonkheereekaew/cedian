//! A tool call as the agent pane draws it (Interface, Agent pane): a one-line card with a
//! 30 px header, opened only for a failure's output or an edit's diff, and the chip a run of
//! calls folds into in the two narrower view modes.

use cedian_agent::{DiffLine, ToolCall, ToolState};
use gpui::{
    App, Div, FontWeight, InteractiveElement as _, ParentElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::tokens;
use crate::type_scale::TypeScale as _;

/// Lines of a failure's output or an edit's diff shown before the rest is cut.
const BODY_LINES: usize = 12;

pub const HEADER_HEIGHT: f32 = 30.;

/// The card: icon, tool, argument, then what it came to.
pub fn card(call: &ToolCall, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let running = call.state == ToolState::Running;
    let failed = call.failed();
    let accent = if failed {
        status.error
    } else if running {
        status.info
    } else {
        colors.text_muted
    };
    let header = div()
        .h(px(HEADER_HEIGHT))
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .child(
            svg()
                .path(icon(&call.name))
                .size(px(14.))
                .flex_none()
                .text_color(accent),
        )
        .child(
            div()
                .flex_none()
                .font(tokens::code_font())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.text)
                .child(call.name.clone()),
        )
        .child(
            div()
                .flex_grow(1.)
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font(tokens::code_font())
                .text_color(colors.text_muted)
                .child(argument(call)),
        )
        .children(outcome(call).map(|word| {
            div()
                .flex_none()
                .font(tokens::numeric_font())
                .text_color(if failed {
                    status.error
                } else {
                    colors.text_muted
                })
                .child(word)
        }));
    let mut card = div()
        .flex()
        .flex_col()
        .rounded(px(12.))
        .border_1()
        .border_color(if failed {
            status.error.opacity(0.4)
        } else {
            colors.border
        })
        .bg(if running {
            status.info.opacity(0.1)
        } else if failed {
            status.error.opacity(0.08)
        } else {
            colors.editor_background
        })
        .text_meta()
        .overflow_hidden()
        .child(header);
    if let Some(permission) = call.permission.as_ref().filter(|p| !p.allowed) {
        let reason = permission.reason.clone().unwrap_or_default();
        card = card.child(
            div()
                .px_2()
                .pb_2()
                .text_color(status.error)
                .child(format!("ไม่อนุญาต ({}): {reason}", permission.rule)),
        );
    } else if failed {
        if let Some(output) = &call.output {
            card = card.child(body(output.lines().map(|line| (line.to_owned(), None)), cx));
        }
    } else if let Some(diff) = call.diff().filter(|d| !d.is_empty()) {
        card = card.child(body(
            diff.into_iter().map(|line| match line {
                DiffLine::Removed(text) => (format!("− {text}"), Some(false)),
                DiffLine::Added(text) => (format!("+ {text}"), Some(true)),
            }),
            cx,
        ));
    }
    card
}

/// A failure's output or an edit's diff, in code type, cut at `BODY_LINES`. A line carries
/// `Some(true)` when added and `Some(false)` when removed.
fn body(lines: impl Iterator<Item = (String, Option<bool>)>, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let tint = tokens::Tokens::of(cx);
    let mut lines = lines.peekable();
    let mut shown = Vec::new();
    while shown.len() < BODY_LINES {
        match lines.next() {
            Some(line) => shown.push(line),
            None => break,
        }
    }
    let more = lines.count();
    div()
        .border_t_1()
        .border_color(colors.border)
        .bg(tint.code)
        .py_1()
        .font(tokens::code_font())
        .text_code()
        .children(shown.into_iter().map(|(text, added)| {
            let row = div().px_2().whitespace_nowrap().overflow_hidden();
            match added {
                Some(true) => row
                    .bg(tokens::Tokens::of(cx).diff_add.opacity(0.1))
                    .text_color(colors.text)
                    .child(text),
                Some(false) => row
                    .bg(status.error.opacity(0.1))
                    .text_color(colors.text)
                    .child(text),
                None => row.text_color(colors.text_muted).child(text),
            }
        }))
        .when(more > 0, |d| {
            d.child(
                div()
                    .px_2()
                    .text_color(colors.text_muted)
                    .child(format!("… อีก {more} บรรทัด")),
            )
        })
}

/// A run of calls as one chip: `3 tools · read, bash exit 101, edit +6 −1`.
pub fn chip(calls: &[&ToolCall], open: bool, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let failed = calls.iter().any(|call| call.failed());
    let running = calls.iter().any(|call| call.state == ToolState::Running);
    div()
        .w_full()
        .h(px(38.))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .rounded(px(10.))
        .border_1()
        .border_color(if failed {
            status.error.opacity(0.4)
        } else {
            colors.border
        })
        .when(running, |chip| chip.bg(status.info.opacity(0.08)))
        .text_meta()
        .text_color(colors.text_muted)
        .cursor_pointer()
        .hover(|chip| chip.bg(colors.element_hover))
        .child(
            svg()
                .path(if open {
                    "icons/chevron_down.svg"
                } else {
                    "icons/chevron_right.svg"
                })
                .size(px(12.))
                .text_color(colors.text_muted),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(summary(calls)),
        )
}

pub fn summary(calls: &[&ToolCall]) -> String {
    let parts: Vec<String> = calls
        .iter()
        .map(|call| match chip_outcome(call) {
            Some(word) => format!("{} {word}", call.name),
            None => call.name.clone(),
        })
        .collect();
    let count = match calls.len() {
        1 => "1 tool".to_owned(),
        n => format!("{n} tools"),
    };
    format!("{count} · {}", parts.join(", "))
}

/// What a call came to on its card: `+n −n` for an edit, else its exit code and time.
pub fn outcome(call: &ToolCall) -> Option<String> {
    if let Some((added, removed)) = call.counts() {
        return Some(format!("+{added} −{removed}"));
    }
    let mut words = Vec::new();
    if let Some(code) = call.exit_code {
        words.push(format!("exit {code}"));
    }
    if let Some(ms) = call.ms {
        words.push(duration(ms));
    }
    match call.state {
        ToolState::Requested if call.permission.is_none() => words.push("รอ".to_owned()),
        ToolState::Running => words.push("กำลังทำ".to_owned()),
        _ => {}
    }
    (!words.is_empty()).then(|| words.join(" · "))
}

/// The short form in a chip: only what stands out.
fn chip_outcome(call: &ToolCall) -> Option<String> {
    if let Some((added, removed)) = call.counts() {
        return Some(format!("+{added} −{removed}"));
    }
    if call.permission.as_ref().is_some_and(|p| !p.allowed) {
        return Some("ไม่อนุญาต".to_owned());
    }
    match call.exit_code {
        Some(code) if code != 0. => Some(format!("exit {code}")),
        _ if call.failed() => Some("ล้มเหลว".to_owned()),
        _ => None,
    }
}

pub fn duration(ms: i64) -> String {
    match ms {
        ..1_000 => format!("{ms} ms"),
        1_000..60_000 => format!("{:.1} s", ms as f64 / 1000.),
        _ => format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000),
    }
}

/// The argument worth a glance: the command, the path, the pattern.
pub fn argument(call: &ToolCall) -> String {
    for key in [
        "command", "path", "pattern", "url", "task", "query", "thread", "question",
    ] {
        if let Some(value) = call.input.get(key).and_then(|v| v.as_str()) {
            return value.lines().next().unwrap_or_default().to_owned();
        }
    }
    serde_json::Value::Object(call.input.clone()).to_string()
}

fn icon(tool: &str) -> &'static str {
    match tool {
        "bash" => "icons/tool_terminal.svg",
        "edit" | "write" => "icons/tool_pencil.svg",
        "read" => "icons/file_text_outlined.svg",
        "spawn" | "message_thread" => "icons/split.svg",
        "recall" | "threads" | "read_thread" => "icons/tool_search.svg",
        "ask_user" => "icons/circle_help.svg",
        _ => "icons/tool_hammer.svg",
    }
}

#[cfg(test)]
mod tests {
    use cedian_protocol::EffectStatus;

    use super::*;

    fn call(name: &str, input: serde_json::Value, exit: Option<f64>) -> ToolCall {
        ToolCall {
            call_id: name.to_owned(),
            name: name.to_owned(),
            input: input.as_object().cloned().unwrap_or_default(),
            permission: None,
            state: ToolState::Done(if exit.is_some_and(|c| c != 0.) {
                EffectStatus::Failed
            } else {
                EffectStatus::Completed
            }),
            output: Some(String::new()),
            is_error: exit.is_some_and(|c| c != 0.),
            exit_code: exit,
            started_at: None,
            ms: None,
        }
    }

    #[test]
    fn a_run_folds_into_the_chip_the_spec_shows() {
        let read = call("read", serde_json::json!({"path": "f.txt"}), None);
        let bash = call(
            "bash",
            serde_json::json!({"command": "cargo t"}),
            Some(101.),
        );
        let edit = call(
            "edit",
            serde_json::json!({"path": "f.txt", "edits": [
                {"old_text": "a", "new_text": "b\nc\nd\ne\nf\ng"}
            ]}),
            None,
        );
        assert_eq!(
            summary(&[&read, &bash, &edit]),
            "3 tools · read, bash exit 101, edit +6 −1"
        );
    }

    #[test]
    fn durations_read_at_a_glance() {
        assert_eq!(duration(840), "840 ms");
        assert_eq!(duration(1_240), "1.2 s");
        assert_eq!(duration(63_000), "1m 3s");
    }
}
