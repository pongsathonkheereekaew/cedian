//! A login shell from Zed's terminal crate, drawn as plain text lines. Text comes in through
//! the platform input handler, so IME and non-Latin layouts type what the person typed;
//! control and special keys go through Zed's escape mapping.

use std::{ops::Range, path::PathBuf, time::Duration};

use collections::HashMap;
use gpui::{
    App, AppContext as _, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    Styled as _, UTF16Selection, Window, actions, canvas, div, prelude::FluentBuilder as _,
};
use settings::Settings as _;
use terminal::{MouseInputMode, Terminal, TerminalBounds, TerminalBuilder, TerminalMode};
use theme::ActiveTheme as _;
use util::paths::PathStyle;

use crate::tokens;

actions!(shell, [AttachSelection]);

pub enum ShellEvent {
    /// Text the person selected, to go into the composer.
    Attach(String),
}

/// One run of a screen line: its text and whether the selection covers it.
type Run = (String, bool);

pub struct ShellPane {
    terminal: Option<Entity<Terminal>>,
    error: Option<String>,
    focus: FocusHandle,
    /// Text an IME is composing, not yet sent to the shell.
    marked: Option<String>,
}

impl EventEmitter<ShellEvent> for ShellPane {}

impl ShellPane {
    /// A login shell, started in `cwd` when given.
    pub fn new(cwd: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let window_id = window.window_handle().window_id().as_u64();
        let spawn = TerminalBuilder::new(
            cwd,
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

    /// The text selected with the mouse, if any.
    pub fn selection(&self, cx: &App) -> Option<String> {
        let text = self
            .terminal
            .as_ref()?
            .read(cx)
            .last_content()
            .selection_text
            .clone()?;
        (!text.trim().is_empty()).then_some(text)
    }

    fn attach(&mut self, _: &AttachSelection, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selection(cx) {
            cx.emit(ShellEvent::Attach(text));
        }
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if let Some(terminal) = &self.terminal {
            terminal.update(cx, |terminal, cx| {
                terminal.mouse_down(event, MouseInputMode::ReportToTerminal, cx)
            });
        }
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(terminal) = &self.terminal else {
            return;
        };
        if event.pressed_button == Some(MouseButton::Left) {
            let region = terminal.read(cx).last_content().terminal_bounds.bounds;
            terminal.update(cx, |terminal, cx| {
                terminal.mouse_drag(event, region, MouseInputMode::ReportToTerminal, cx)
            });
            cx.notify();
        }
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(terminal) = &self.terminal {
            terminal.update(cx, |terminal, cx| {
                terminal.mouse_up(event, MouseInputMode::ReportToTerminal, cx)
            });
        }
        cx.notify();
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

    /// The screen as lines of runs, the selected cells apart from the rest.
    fn lines(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<Vec<Run>> {
        let Some(terminal) = &self.terminal else {
            let text = self
                .error
                .clone()
                .unwrap_or_else(|| "starting shell…".into());
            return vec![vec![(text, false)]];
        };
        terminal.update(cx, |terminal, cx| terminal.sync(window, cx));
        let content = terminal.read(cx).last_content();
        let mut lines: Vec<Vec<Run>> = vec![Vec::new(); content.screen_lines.max(1)];
        for cell in &content.cells {
            let row = i64::from(cell.point.line) + content.display_offset as i64;
            let Some(line) = usize::try_from(row).ok().and_then(|row| lines.get_mut(row)) else {
                continue;
            };
            let selected = content
                .selection
                .is_some_and(|selection| selected(&selection, cell.point));
            if line.last().is_none_or(|run| run.1 != selected) {
                line.push((String::new(), selected));
            }
            if let Some(run) = line.last_mut() {
                run.0.push(cell.character());
                // Combining marks, such as Thai vowels and tones, ride on the cell before them.
                run.0.extend(cell.zerowidth().unwrap_or_default());
            }
        }
        for line in &mut lines {
            if let Some(run) = line.last_mut()
                && !run.1
            {
                run.0.truncate(run.0.trim_end().len());
            }
        }
        if let Some(marked) = &self.marked
            && let Some(cursor) = usize::try_from(
                i64::from(content.cursor.point.line) + content.display_offset as i64,
            )
            .ok()
            && let Some(line) = lines.get_mut(cursor)
        {
            line.push((marked.clone(), false));
        }
        lines
    }
}

/// Whether the selection covers a cell, as alacritty decides it: a line selection runs across
/// the lines between its ends, a block one keeps its columns on every line.
fn selected(selection: &terminal::SelectionRange, point: terminal::Point) -> bool {
    let (start, end) = (selection.start, selection.end);
    start.line <= point.line
        && end.line >= point.line
        && (start.column <= point.column || (start.line != point.line && !selection.is_block))
        && (end.column >= point.column || (end.line != point.line && !selection.is_block))
}

impl Render for ShellPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lines = self.lines(window, cx);
        let focus = self.focus.clone();
        let pane = cx.entity();
        let terminal = self.terminal.clone();
        let font_size = theme_settings::ThemeSettings::get_global(cx).buffer_font_size(cx);
        let line_height = (font_size * 1.4).round();
        let font_id = window.text_system().resolve_font(&tokens::code_font());
        let cell_width = window
            .text_system()
            .advance(font_id, font_size, 'm')
            .map_or(font_size * 0.6, |size| size.width);
        let selection = cx.theme().colors().element_selection_background;
        div()
            .id("shell")
            .key_context("Shell")
            .size_full()
            .p_2()
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::attach))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_key_down(cx.listener(Self::key_down))
            .child(
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .font(tokens::code_font())
                    .text_size(font_size)
                    .line_height(line_height)
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                // The grid fills the pane, so the shell wraps where the pane does.
                                if let Some(terminal) = &terminal {
                                    terminal.update(cx, |terminal, _| {
                                        terminal.set_size(TerminalBounds::new(
                                            line_height,
                                            cell_width,
                                            bounds,
                                        ))
                                    });
                                }
                            },
                            move |bounds, _, window, cx| {
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(bounds, pane),
                                    cx,
                                )
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    .children(lines.into_iter().map(|line| {
                        div().h(line_height).flex().whitespace_nowrap().children(
                            line.into_iter().map(|(text, selected)| {
                                div().when(selected, |run| run.bg(selection)).child(text)
                            }),
                        )
                    })),
            )
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
