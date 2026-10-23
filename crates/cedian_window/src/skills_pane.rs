//! The Skills block (Interface, Blocks): what `/` offers in this project, from `cedia commands
//! --json`: each skill with what it does and where it comes from (the person's, the
//! repository's). A click puts `/<name> ` in the composer, as picking it from the `/` list does;
//! cedia loads the skill when the run starts.

use cedian_protocol::{CommandEntry, CommandEntrySource};
use gpui::{
    Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;

use crate::{line_breaks::wrapped, type_scale::TypeScale as _};

#[derive(Default)]
pub struct SkillsPane {
    /// `None` until the daemon has answered, or when this cedia lists none.
    commands: Option<Vec<CommandEntry>>,
}

pub enum SkillsEvent {
    /// Put `/<name> ` in the composer.
    Use(String),
}

impl EventEmitter<SkillsEvent> for SkillsPane {}

impl SkillsPane {
    pub fn set(&mut self, commands: Vec<CommandEntry>, cx: &mut Context<Self>) {
        self.commands = Some(commands);
        cx.notify();
    }

    /// Another project: what was listed was the last one's.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.commands = None;
        cx.notify();
    }

    pub fn count(&self) -> usize {
        self.commands.as_ref().map_or(0, Vec::len)
    }
}

/// Where a command comes from, in a word.
pub fn source_word(source: CommandEntrySource) -> &'static str {
    match source {
        CommandEntrySource::User => "ของคุณ",
        CommandEntrySource::Repository => "ของ repo",
    }
}

impl Render for SkillsPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let pane = div()
            .id("skills")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .text_ui()
            .text_color(colors.text);
        let hint = div()
            .px_3()
            .pt_2()
            .pb_1()
            .text_meta()
            .text_color(colors.text_muted)
            .child(wrapped(
                "คลิกเพื่อใส่ /ชื่อ ในช่องพิมพ์ cedia โหลด skill นั้นตอน run เริ่ม หรือพิมพ์ / ในช่องพิมพ์ก็ได้",
            ));
        let commands = match &self.commands {
            Some(commands) if !commands.is_empty() => commands,
            shown => {
                let word = if shown.is_none() {
                    "กำลังอ่าน"
                } else {
                    "โปรเจกต์นี้ยังไม่มี skill: ใส่ไว้ที่ ~/.cedia/skills หรือ .agents/skills ใน repo"
                };
                return pane.child(
                    div()
                        .px_3()
                        .pt_2()
                        .text_color(colors.text_muted)
                        .child(wrapped(word)),
                );
            }
        };
        let mut pane = pane.child(hint);
        for (n, command) in commands.iter().enumerate() {
            let name = command.name.clone();
            let title = match &command.argument_hint {
                Some(hint) => format!("/{} {hint}", command.name),
                None => format!("/{}", command.name),
            };
            pane = pane.child(
                div()
                    .id(("skill", n))
                    .debug_selector(move || format!("skill-{n}"))
                    .flex_none()
                    .mx_3()
                    .my_1()
                    .px_2p5()
                    .py_1p5()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .min_w_0()
                    .overflow_hidden()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.border_variant)
                    .cursor_pointer()
                    .hover(|b| b.bg(colors.element_hover))
                    .on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(SkillsEvent::Use(name.clone()))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1p5()
                            .child(div().min_w_0().truncate().font_family("Lilex").child(title))
                            .child(div().flex_grow(1.))
                            .child(
                                div()
                                    .flex_none()
                                    .text_meta()
                                    .text_color(colors.text_muted)
                                    .child(source_word(command.source)),
                            ),
                    )
                    .child(
                        div()
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(wrapped(&command.description)),
                    ),
            );
        }
        pane.child(div().h(px(8.)).flex_none())
    }
}
