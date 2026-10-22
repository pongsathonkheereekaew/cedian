//! Thread details, a tool of the panel (Interface, Agent pane: Thread details): where the
//! thread runs, its sessions in order, what it cost by model, the context the latest turn
//! held, and the files its latest receipt says it changed.

use cedian_agent::{Block, Transcript};
use cedian_wire::ThreadInfo;
use gpui::{App, Div, FontWeight, ParentElement as _, Styled as _, div};
use theme::ActiveTheme as _;

use crate::tokens;
use crate::type_scale::TypeScale as _;

/// What the details show, read from the transcript.
#[derive(Debug, Default, PartialEq)]
pub struct Details {
    /// Each session's role and model, in order.
    pub sessions: Vec<(String, String)>,
    /// Finished runs' cost, by the model of the session they ended in.
    pub cost_by_model: Vec<(String, f64)>,
    pub context: Option<u64>,
    /// The writer model's context window, when cedia lists the model.
    pub context_window: Option<u64>,
    pub changed: Vec<String>,
}

impl Details {
    pub fn of(transcript: &Transcript) -> Self {
        let mut details = Details {
            context: transcript.context(),
            ..Details::default()
        };
        let mut model = String::new();
        for block in transcript.blocks() {
            match block {
                Block::Session { model: m, role } => {
                    model = m.clone();
                    details.sessions.push((role.clone(), m.clone()));
                }
                Block::Result {
                    cost_usd, evidence, ..
                } => {
                    match details.cost_by_model.iter_mut().find(|(m, _)| *m == model) {
                        Some((_, total)) => *total += cost_usd,
                        None => details.cost_by_model.push((model.clone(), *cost_usd)),
                    }
                    if let Some(evidence) = evidence {
                        details.changed = evidence.changed.clone();
                    }
                }
                _ => {}
            }
        }
        details
    }
}

pub fn render(info: Option<&ThreadInfo>, details: &Details, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let label = |text: &'static str| {
        div()
            .pt_2()
            .text_label()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(colors.text_muted)
            .child(text)
    };
    let mono = |text: String| div().font(tokens::code_font()).child(text);
    let mut column = div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .p_3()
        .text_meta()
        .bg(colors.panel_background)
        .text_color(colors.text);
    let Some(info) = info else {
        return column.child(
            div()
                .text_color(colors.text_muted)
                .child("ยังไม่ได้เปิด thread"),
        );
    };
    column = column
        .child(
            div()
                .text_ui()
                .font_weight(FontWeight::SEMIBOLD)
                .child(info.thread.title.clone()),
        )
        .child(
            div()
                .text_color(colors.text_muted)
                .font(tokens::code_font())
                .child(info.thread.id.clone()),
        )
        .child(label("สภาพแวดล้อม"));
    column = match &info.cwd {
        Some(cwd) => column.child(mono(cwd.display().to_string())),
        None => column.child(div().text_color(colors.text_muted).child("ยังไม่รู้")),
    };
    column = column.child(label("Sessions ตามลำดับ"));
    for (n, (role, model)) in details.sessions.iter().enumerate() {
        column = column.child(mono(format!("{}. {role} · {model}", n + 1)));
    }
    column = column.child(label("ค่าใช้จ่ายตาม model"));
    if details.cost_by_model.is_empty() {
        column = column.child(div().text_color(colors.text_muted).child("ยังไม่มีรอบที่จบ"));
    }
    for (model, cost) in &details.cost_by_model {
        column = column.child(mono(format!("${cost:.4} · {model}")));
    }
    if let Some(context) = details.context {
        column = column.child(label("Context ของ turn ล่าสุด")).child(mono(
            match details.context_window.filter(|w| *w > 0) {
                Some(window) => {
                    format!("{context} / {window} tokens ({}%)", context * 100 / window)
                }
                None => format!("{context} tokens"),
            },
        ));
    }
    if !details.changed.is_empty() {
        column = column.child(label("ไฟล์ที่เปลี่ยน"));
        for path in &details.changed {
            column = column.child(mono(path.clone()));
        }
    }
    column
}

#[cfg(test)]
mod tests {
    use cedian_protocol::{Line, fixtures_dir};

    use super::*;

    #[test]
    fn details_read_from_a_recorded_run() {
        let stream = std::fs::read_to_string(fixtures_dir().join("edit-and-check/stream.jsonl"))
            .expect("stream");
        let mut transcript = Transcript::default();
        for text in stream.lines() {
            transcript.apply(&Line::parse(text).expect("line"));
        }
        let details = Details::of(&transcript);
        assert_eq!(
            details.sessions,
            [("code".to_owned(), "faux/scripted".to_owned())]
        );
        assert_eq!(details.cost_by_model.len(), 1);
        assert_eq!(details.changed, ["f.txt"]);
    }
}
