//! A login shell from Zed's terminal crate, drawn as plain text lines. Text comes in through
//! the platform input handler, so IME and non-Latin layouts type what the person typed;
//! control and special keys go through Zed's escape mapping.

use std::{ops::Range, time::Duration};

use collections::HashMap;
use gpui::{
    AppContext as _, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Pixels,
    Point, Render, Styled as _, UTF16Selection, Window, canvas, div,
};
use terminal::{Terminal, TerminalBuilder, TerminalMode};
use util::paths::PathStyle;

pub struct ShellPane {
    terminal: Option<Entity<Terminal>>,
    error: Option<String>,
    focus: FocusHandle,
    /// Text an IME is composing, not yet sent to the shell.
    marked: Option<String>,
}

impl ShellPane {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
                    Err(error) => this.error = Some(format!("{error:#}")),
                }
                cx.notify();
            })
        })
        .detach();
        Self {
            terminal: None,
            error: None,
            focus: cx.focus_handle(),
            marked: None,
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // Typed text arrives through the input handler instead.
        if event.prefer_character_input && event.keystroke.key_char.is_some() {
            return;
        }
        let Some(terminal) = &self.terminal else {
            return;
        };
        if terminal.update(cx, |terminal, _| {
            terminal.try_keystroke(&event.keystroke, false)
        }) {
            cx.stop_propagation();
        }
    }

    fn lines(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<String> {
        let Some(terminal) = &self.terminal else {
            return vec![
                self.error
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
                // Combining marks, such as Thai vowels and tones, ride on the cell before them.
                line.extend(cell.zerowidth().unwrap_or_default());
            }
        }
        let mut lines: Vec<String> = lines.iter().map(|l| l.trim_end().to_owned()).collect();
        if let Some(marked) = &self.marked
            && let Some(cursor) = usize::try_from(
                i64::from(content.cursor.point.line) + content.display_offset as i64,
            )
            .ok()
            && let Some(line) = lines.get_mut(cursor)
        {
            line.push_str(marked);
        }
        lines
    }
}

impl Render for ShellPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lines = self.lines(window, cx);
        let focus = self.focus.clone();
        let pane = cx.entity();
        div()
            .id("shell")
            .relative()
            .size_full()
            .p_2()
            .font_family("Lilex")
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| window.focus(&this.focus, cx)),
            )
            .on_key_down(cx.listener(Self::key_down))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, pane), cx)
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(lines.into_iter().map(|line| div().child(line)))
    }
}

impl EntityInputHandler for ShellPane {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|text| 0..text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = None;
        if let Some(terminal) = &self.terminal {
            let bytes = text.as_bytes().to_vec();
            terminal.update(cx, |terminal, _| terminal.input(bytes));
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        new_text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = (!new_text.is_empty()).then(|| new_text.to_owned());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
