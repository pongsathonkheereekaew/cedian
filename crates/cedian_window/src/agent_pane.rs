//! The agent pane (Interface, Agent pane): the thread's header and chips over its blocks,
//! drawn from cedia's stream as it arrives.

use std::{collections::HashSet, path::Path, rc::Rc};

use cedian_agent::{Block, ToolCall, Transcript};
use cedian_protocol::{Line, Rating};
use cedian_wire::Status;
use gpui::{
    AnyElement, App, Context, Div, EventEmitter, InteractiveElement as _, IntoElement,
    ListAlignment, ListState, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, actions, div, list,
    prelude::FluentBuilder as _, px, relative,
};
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;
use crate::{
    density::Density,
    line_breaks::wrapped,
    receipt::{Actions, Receipt},
    tokens, tool_card,
    view_mode::ViewMode,
};

actions!(agent_pane, [StopRun]);

/// The widest an answer runs, for reading.
/// The conversation's column, centred in the sheet (Interface, Polish).
const COLUMN_WIDTH: f32 = 700.;

/// What the window tells the pane about the open thread, beyond its blocks.
#[derive(Clone, Debug, Default)]
pub struct Header {
    pub title: SharedString,
    pub status: Option<Status>,
    pub rating: Option<Rating>,
}

pub enum AgentEvent {
    Rate(Rating),
    ViewMode(ViewMode),
    /// Open the thread's review.
    Review,
    /// Edit the person's message with this text and send it again, in a fork.
    Edit(String),
}

impl EventEmitter<AgentEvent> for AgentPane {}

/// Sends an event from a control drawn outside the pane's own `Context`.
pub type Emit = Rc<dyn Fn(AgentEvent, &mut App)>;

pub struct AgentPane {
    header: Header,
    transcript: Transcript,
    list: ListState,
    view_mode: ViewMode,
    /// Runs of tool calls opened from their chip, by the index of their first block.
    unfolded: HashSet<usize>,
    density: Density,
    /// The prompts of the thread's runs, when it can be forked at one: a message of the
    /// person's that started a run offers แก้ไข.
    editable: Vec<String>,
}

impl AgentPane {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            header: Header {
                title: title.into(),
                ..Header::default()
            },
            transcript: Transcript::default(),
            list: ListState::new(0, ListAlignment::Bottom, px(512.)),
            view_mode: ViewMode::default(),
            unfolded: HashSet::new(),
            density: Density::default(),
            editable: Vec::new(),
        }
    }

    pub fn set_editable(&mut self, prompts: Vec<String>, cx: &mut Context<Self>) {
        if prompts != self.editable {
            self.editable = prompts;
            // A message that offers แก้ไข is taller.
            self.list.reset(self.transcript.blocks().len());
            cx.notify();
        }
    }

    /// Whether the message started one of the thread's runs, which a fork can go back to.
    fn editable(&self, text: &str) -> bool {
        self.editable
            .iter()
            .any(|p| !p.is_empty() && text.starts_with(p.as_str()))
    }

    pub fn with_view_mode(mut self, mode: ViewMode) -> Self {
        self.view_mode = mode;
        self
    }

    /// The latest receipt's checks, split by whether they ran after `path` was last edited.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub fn file_checks(&self, path: &str) -> cedian_agent::FileChecks {
        self.transcript.file_checks(path)
    }

    /// Empties the pane for another thread.
    pub fn reset(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.header = Header {
            title: title.into(),
            ..Header::default()
        };
        self.transcript = Transcript::default();
        self.unfolded.clear();
        self.list.reset(0);
        cx.notify();
    }

    pub fn set_header(&mut self, header: Header, cx: &mut Context<Self>) {
        let rated = header.rating != self.header.rating;
        self.header = header;
        if rated {
            // The latest receipt draws the rating.
            self.relayout(cx);
        }
        cx.notify();
    }

    /// Lays the blocks out again when the density changed, their heights with it.
    pub fn set_density(&mut self, density: Density, cx: &mut Context<Self>) {
        if density != self.density {
            self.density = density;
            self.relayout(cx);
        }
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if mode != self.view_mode {
            self.view_mode = mode;
            self.relayout(cx);
        }
    }

    fn relayout(&mut self, cx: &mut Context<Self>) {
        self.list.reset(self.transcript.blocks().len());
        cx.notify();
    }

    pub fn apply(&mut self, line: &Line, cx: &mut Context<Self>) {
        self.transcript.apply(line);
        let Some(from) = self.transcript.take_changed_from() else {
            return;
        };
        let old = self.list.item_count();
        // A run of tool calls draws as one chip at its first block, which changes with it,
        // and only the latest receipt carries the rating.
        let blocks = self.transcript.blocks();
        let from = match blocks.get(from) {
            Some(Block::Result { .. }) => blocks[..from]
                .iter()
                .rposition(|b| matches!(b, Block::Result { .. }))
                .unwrap_or(from),
            _ => self.run_start(from),
        }
        .min(old);
        self.list.splice(
            from..old,
            self.transcript.blocks().len().saturating_sub(from),
        );
        cx.notify();
    }

    /// The first block of the run of tool calls `index` is in, or `index`.
    fn run_start(&self, index: usize) -> usize {
        let blocks = self.transcript.blocks();
        let mut start = index;
        if matches!(blocks.get(index), Some(Block::Tool(_))) {
            while start > 0 && matches!(blocks.get(start - 1), Some(Block::Tool(_))) {
                start -= 1;
            }
        }
        start
    }

    fn toggle_run(&mut self, start: usize, cx: &mut Context<Self>) {
        if !self.unfolded.remove(&start) {
            self.unfolded.insert(start);
        }
        let end = self.run_end(start);
        self.list.splice(start..end, end - start);
        cx.notify();
    }

    /// Opens or folds one block, a thinking line.
    fn toggle_block(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.unfolded.remove(&index) {
            self.unfolded.insert(index);
        }
        self.list.splice(index..index + 1, 1);
        cx.notify();
    }

    fn run_end(&self, start: usize) -> usize {
        let blocks = self.transcript.blocks();
        let mut end = start;
        while matches!(blocks.get(end), Some(Block::Tool(_))) {
            end += 1;
        }
        end
    }

    fn emitter(pane: &WeakEntity<Self>) -> Emit {
        let pane = pane.clone();
        Rc::new(move |event, cx| {
            pane.update(cx, |_, cx| cx.emit(event)).ok();
        })
    }

    fn render_block(&self, index: usize, pane: &WeakEntity<Self>, cx: &App) -> AnyElement {
        let colors = cx.theme().colors();
        let tokens = tokens::Tokens::of(cx);
        let blocks = self.transcript.blocks();
        let Some(block) = blocks.get(index) else {
            return div().into_any_element();
        };
        let body = match block {
            Block::Session { model, role } => div()
                .flex()
                .items_center()
                .gap_3()
                .text_meta()
                .text_color(colors.text_muted)
                .child(div().flex_grow(1.).h(px(1.)).bg(colors.border))
                .child(format!("session {role} · {model}"))
                .child(div().flex_grow(1.).h(px(1.)).bg(colors.border)),
            Block::User { text } => {
                let group = SharedString::from(format!("user-{index}"));
                let edit = self.editable(text).then(|| {
                    let emit = Self::emitter(pane);
                    let text = text.clone();
                    // Shown on hover, its room kept so the list does not move.
                    div()
                        .id(("edit-message", index))
                        .h(px(22.))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .text_meta()
                        .text_color(colors.text_muted)
                        .cursor_pointer()
                        .opacity(0.)
                        .group_hover(group.clone(), |b| b.opacity(1.))
                        .hover(|b| b.bg(colors.element_hover).text_color(colors.text))
                        .on_click(move |_, _, cx| emit(AgentEvent::Edit(text.clone()), cx))
                        .child("แก้ไข")
                });
                div()
                    .group(group)
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap_1()
                    .child(
                        div()
                            .max_w(relative(0.8))
                            .px_4()
                            .py(px(10.))
                            .rounded(px(18.))
                            .bg(tokens.user_bubble)
                            .text_message()
                            .child(wrapped(text)),
                    )
                    .children(edit)
            }
            Block::Text { text, streaming } => div()
                .font(tokens::answer_font())
                .text_answer()
                // A caret while the text streams.
                .child(wrapped(&if *streaming {
                    format!("{text}▍")
                } else {
                    text.clone()
                })),
            Block::Thinking { .. } if !self.view_mode.shows_thinking() => div(),
            Block::Thinking { text, .. } => {
                // One quiet line that opens to the thinking. How long it took waits for cedia
                // to log when a thinking block starts and ends.
                let open = self.unfolded.contains(&index);
                let pane = pane.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .id(("thinking", index))
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_meta()
                            .text_color(colors.text_placeholder)
                            .cursor_pointer()
                            .hover(|line| line.text_color(colors.text_muted))
                            .on_click(move |_, _, cx| {
                                pane.update(cx, |pane, cx| pane.toggle_block(index, cx))
                                    .ok();
                            })
                            .child("ความคิด")
                            .child(if open { "⌄" } else { "›" }),
                    )
                    .when(open, |thinking| {
                        thinking.child(
                            div()
                                .pl_3()
                                .border_l_2()
                                .border_color(colors.border)
                                .text_ui()
                                .text_color(colors.text_muted)
                                .child(wrapped(text)),
                        )
                    })
            }
            Block::Tool(call) => return self.render_tool(index, call, pane, cx),
            Block::Result {
                subtype,
                text,
                cost_usd,
                turns,
                wall_ms,
                evidence,
            } => {
                // The answer above usually is the run's last words already.
                let said = index
                    .checked_sub(1)
                    .and_then(|i| blocks.get(i))
                    .is_some_and(|b| matches!(b, Block::Text { text: t, .. } if t == text));
                let latest = !blocks[index + 1..]
                    .iter()
                    .any(|b| matches!(b, Block::Result { .. }));
                div().child(
                    Receipt {
                        subtype: *subtype,
                        text: (!said && !text.is_empty()).then_some(text.as_str()),
                        cost_usd: *cost_usd,
                        turns: *turns,
                        wall_ms: *wall_ms,
                        evidence: evidence.as_ref(),
                        verified: block.verified(),
                        thread: self.transcript.thread(),
                        actions: latest.then(|| Actions {
                            rating: self.header.rating,
                            emit: Self::emitter(pane),
                        }),
                    }
                    .render(cx),
                )
            }
        };
        column(body.py(self.density.block_gap())).into_any_element()
    }

    fn render_tool(
        &self,
        index: usize,
        call: &ToolCall,
        pane: &WeakEntity<Self>,
        cx: &App,
    ) -> AnyElement {
        let start = self.run_start(index);
        let open = self.unfolded.contains(&start);
        if !self.view_mode.folds_tools() || open {
            let cards = div().py(px(3.)).flex().flex_col().gap_1();
            let cards = if open && index == start {
                cards.child(self.fold_chip(start, true, pane, cx))
            } else {
                cards
            };
            return column(cards.child(tool_card::card(call, cx))).into_any_element();
        }
        if index != start {
            return div().into_any_element();
        }
        column(div().py_1().child(self.fold_chip(start, false, pane, cx))).into_any_element()
    }

    fn fold_chip(
        &self,
        start: usize,
        open: bool,
        pane: &WeakEntity<Self>,
        cx: &App,
    ) -> gpui::Stateful<Div> {
        let calls: Vec<&ToolCall> = self.transcript.blocks()[start..self.run_end(start)]
            .iter()
            .filter_map(|b| match b {
                Block::Tool(call) => Some(call),
                _ => None,
            })
            .collect();
        let pane = pane.clone();
        div()
            .id(("tools", start))
            .w_full()
            .child(tool_card::chip(&calls, open, cx))
            .on_click(move |_, _, cx| {
                pane.update(cx, |pane, cx| pane.toggle_run(start, cx)).ok();
            })
    }

    /// What the thread's runs cost so far.
    pub fn cost(&self) -> Option<f64> {
        let mut total = None;
        for block in self.transcript.blocks() {
            if let Block::Result { cost_usd, .. } = block {
                *total.get_or_insert(0.) += cost_usd;
            }
        }
        total
    }

    /// The thread's title, state and cost, which the sheet's header shows.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The context the latest turn held and the view mode: the composer's bottom row
    /// (Interface, Polish). The session's model and role are on its divider.
    /// The composer row's context meter and view modes. `window`: the writer model's context
    /// window, when cedia lists the model (`cedia models`).
    pub fn chips(&self, pane: &WeakEntity<Self>, window: Option<u64>, cx: &App) -> Div {
        let colors = cx.theme().colors();
        let emit = Self::emitter(pane);
        let chip = || {
            div()
                .flex_none()
                .h(px(24.))
                .px_2()
                .flex()
                .items_center()
                .gap_1()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border)
                .text_meta()
                .text_color(colors.text_muted)
        };
        let mut chips = div()
            .debug_selector(|| "agent-chips".into())
            .flex()
            .items_center()
            .gap_1p5();
        if let Some(context) = self.transcript.context() {
            let meter = match window.filter(|w| *w > 0) {
                Some(window) => {
                    let used = (context as f32 / window as f32).clamp(0., 1.);
                    let status = cx.theme().status();
                    // Near the window a handoff or compaction comes soon.
                    let fill = if used >= 0.9 {
                        status.error
                    } else if used >= 0.75 {
                        status.warning
                    } else {
                        colors.text_muted
                    };
                    chip()
                        .font(tokens::numeric_font())
                        .child(
                            div()
                                .flex_none()
                                .w(px(METER_WIDTH))
                                .h(px(4.))
                                .rounded_full()
                                .bg(colors.element_selected)
                                .child(
                                    div()
                                        .h_full()
                                        .w(px((METER_WIDTH * used).max(2.)))
                                        .rounded_full()
                                        .bg(fill),
                                ),
                        )
                        .child(format!(
                            "{} / {}",
                            short_tokens(context),
                            short_tokens(window)
                        ))
                }
                None => chip()
                    .font(tokens::numeric_font())
                    .child(format!("{} context", short_tokens(context))),
            };
            chips = chips.child(meter.debug_selector(|| "context-chip".into()));
        }
        let mut modes = div()
            .debug_selector(|| "view-modes".into())
            .flex_none()
            .h(px(24.))
            .flex()
            .rounded(px(6.))
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .text_meta();
        for (mode, key, label) in ViewMode::ALL {
            let on = mode == self.view_mode;
            let emit = emit.clone();
            modes = modes.child(
                div()
                    .id(key)
                    .px_2()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .when(on, |m| {
                        m.bg(colors.element_selected).text_color(colors.text)
                    })
                    .when(!on, |m| {
                        m.text_color(colors.text_muted)
                            .hover(|m| m.bg(colors.element_hover))
                    })
                    .on_click(move |_, _, cx| emit(AgentEvent::ViewMode(mode), cx))
                    .child(label),
            );
        }
        chips.child(div().flex_grow(1.)).child(modes)
    }
}

#[cfg(target_os = "macos")]
pub const STOP_KEY: &str = "⌘.";
#[cfg(not(target_os = "macos"))]
pub const STOP_KEY: &str = "Ctrl+.";

/// The branch of a worktree the daemon made: `.cedian/worktrees/<name>` is on `cedian/<name>`.
pub(crate) fn branch_of(cwd: &Path) -> Option<String> {
    let name = cwd.file_name()?.to_str()?;
    let parent = cwd.parent()?;
    (parent.file_name()? == "worktrees" && parent.parent()?.file_name()? == ".cedian")
        .then(|| format!("cedian/{name}"))
}

/// A block in the conversation's column: at most `COLUMN_WIDTH` wide, centred in the sheet.
/// The list gives an item its full width, so the cap goes on a child.
fn column(body: Div) -> Div {
    div()
        .w_full()
        .px_6()
        .flex()
        .justify_center()
        .child(div().w_full().max_w(px(COLUMN_WIDTH)).child(body))
}

/// `76k`, `1.2M`: tokens at a glance.
/// The context meter's bar.
const METER_WIDTH: f32 = 28.;

fn short_tokens(n: u64) -> String {
    match n {
        ..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{}k", n / 1_000),
        _ if n.is_multiple_of(1_000_000) => format!("{}M", n / 1_000_000),
        _ => format!("{:.1}M", n as f64 / 1_000_000.),
    }
}

impl Render for AgentPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let pane = cx.entity().downgrade();
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(colors.text)
            .child(
                list(self.list.clone(), move |index, _window, cx| {
                    match pane.upgrade() {
                        Some(entity) => entity.read(cx).render_block(index, &pane, cx),
                        None => div().into_any_element(),
                    }
                })
                .flex_grow(1.)
                .py_2(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daemon_worktree_names_its_branch() {
        assert_eq!(
            branch_of(Path::new("/r/.cedian/worktrees/make-f-say-good")).as_deref(),
            Some("cedian/make-f-say-good")
        );
        assert_eq!(branch_of(Path::new("/r/src")), None);
    }

    #[test]
    fn tokens_read_short() {
        assert_eq!(short_tokens(812), "812");
        assert_eq!(short_tokens(76_400), "76k");
        assert_eq!(short_tokens(1_250_000), "1.2M");
        assert_eq!(short_tokens(1_000_000), "1M");
    }
}
