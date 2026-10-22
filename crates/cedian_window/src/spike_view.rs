//! Phase A spike: a Zed editor above a shell from Zed's terminal crate, drawn as plain text.

use std::time::Duration;

use collections::HashMap;
use editor::Editor;
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render, Styled as _, Window, div,
};
use terminal::{Terminal, TerminalBuilder, TerminalMode};
use theme::ActiveTheme as _;
use util::paths::PathStyle;

pub struct SpikeView {
    editor: Entity<Editor>,
    terminal: Option<Entity<Terminal>>,
    terminal_error: Option<String>,
    terminal_focus: FocusHandle,
}

impl SpikeView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = Editor::multi_line(window, cx);
            editor.set_text("cedian spike: Zed's editor in a GPUI window.\n", window, cx);
            editor
        });
        window.focus(&editor.focus_handle(cx), cx);
        let window_id = window.window_handle().window_id().as_u64();
        let spawn = TerminalBuilder::new(
            None,
            TerminalMode::interactive(),
            task::Shell::System,
            HashMap::default(),
            terminal::terminal_settings::CursorShape::default(),
            terminal::terminal_settings::AlternateScroll::On,
            None,
            Vec::new(),
            Duration::ZERO,
            false,
            window_id,
            cx,
            Vec::new(),
            PathStyle::local(),
        );
        cx.spawn_in(window, async move |this, cx| {
            let built = spawn.await;
            this.update(cx, |this, cx| {
                match built {
                    Ok(builder) => {
                        let terminal = cx.new(|cx| builder.subscribe(cx));
                        cx.observe(&terminal, |_, _, cx| cx.notify()).detach();
                        cx.subscribe(&terminal, |_, _, _: &terminal::Event, cx| cx.notify())
                            .detach();
                        this.terminal = Some(terminal);
                    }
                    Err(error) => this.terminal_error = Some(format!("{error:#}")),
                }
                cx.notify();
            })
        })
        .detach();
        Self {
            editor,
            terminal: None,
            terminal_error: None,
            terminal_focus: cx.focus_handle(),
        }
    }

    /// Sends a key to the shell: control and special keys through Zed's escape mapping,
    /// anything else as the text it types.
    fn terminal_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(terminal) = &self.terminal else {
            return;
        };
        terminal.update(cx, |terminal, _| {
            if !terminal.try_keystroke(&event.keystroke, false)
                && let Some(text) = &event.keystroke.key_char
            {
                terminal.input(text.clone().into_bytes());
            }
        });
        cx.stop_propagation();
    }

    fn terminal_lines(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<String> {
        let Some(terminal) = &self.terminal else {
            return vec![
                self.terminal_error
                    .clone()
                    .unwrap_or_else(|| "starting shell…".into()),
            ];
        };
        terminal.update(cx, |terminal, cx| terminal.sync(window, cx));
        let content = terminal.read(cx).last_content();
        let mut lines = vec![String::new(); content.screen_lines.max(1)];
        for cell in &content.cells {
            let row = i64::from(cell.point.line) + content.display_offset as i64;
            if let Some(line) = usize::try_from(row).ok().and_then(|row| lines.get_mut(row)) {
                line.push(cell.character());
            }
        }
        lines
            .iter()
            .map(|line| line.trim_end().to_owned())
            .collect()
    }
}

impl Render for SpikeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let (background, text, border) = (colors.background, colors.text, colors.border);
        let lines = self.terminal_lines(window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(background)
            .text_color(text)
            .child(
                div()
                    .h_1_2()
                    .p_2()
                    .border_b_1()
                    .border_color(border)
                    .child(self.editor.clone()),
            )
            .child(
                div()
                    .id("terminal")
                    .track_focus(&self.terminal_focus)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| window.focus(&this.terminal_focus, cx)),
                    )
                    .on_key_down(cx.listener(Self::terminal_key))
                    .h_1_2()
                    .p_2()
                    .font_family("Lilex")
                    .flex()
                    .flex_col()
                    .children(lines.into_iter().map(|line| div().child(line))),
            )
    }
}
