//! The agent pane (Interface, Agent pane): the thread's header and chips over its blocks,
//! drawn from cedia's stream as it arrives.

use std::{collections::HashSet, path::Path, rc::Rc};

use cedian_agent::{Block, ToolCall, Transcript};
use cedian_protocol::{Line, Rating};
use cedian_wire::Status;
use gpui::{
    AnyElement, App, Context, Div, EventEmitter, InteractiveElement as _, IntoElement,
    ListAlignment, ListState, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, actions, div, img, list,
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

#[derive(Clone)]
pub enum AgentEvent {
    Rate(Rating),
    ViewMode(ViewMode),
    /// Open the thread's review.
    Review,
    /// Edit the person's message with this text and send it again, in a fork.
    Edit(String),
    /// Take the thread back to just before the run this message of the person's started.
    Rewind(String),
    /// Put the files of the thread's last turn back (`false`), or forward again (`true`).
    Restore {
        redo: bool,
    },
}

/// What a message of the person's that started a run offers on hover, and the latest
/// receipt's undo.
#[derive(Clone, Default, PartialEq)]
pub struct RunOffers {
    /// The prompts of the thread's runs.
    pub prompts: Vec<String>,
    /// แก้ไข: a fork sends it again, edited.
    pub edit: bool,
    /// ย้อนมาตรงนี้: the thread goes back to just before it.
    pub rewind: bool,
    /// The latest receipt undoes its turn's files; `Some(true)` once undone, to redo.
    pub undone: Option<bool>,
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
    /// What the thread's runs offer: a message of the person's that started one offers แก้ไข
    /// and ย้อนมาตรงนี้.
    offers: RunOffers,
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
            offers: RunOffers::default(),
        }
    }

    #[cfg(test)]
    pub fn offers(&self) -> &RunOffers {
        &self.offers
    }

    pub fn set_offers(&mut self, offers: RunOffers, cx: &mut Context<Self>) {
        if offers != self.offers {
            self.offers = offers;
            // A message that offers แก้ไข is taller, and a receipt with undo.
            self.list.reset(self.transcript.blocks().len());
            cx.notify();
        }
    }

    /// Whether the message started one of the thread's runs, which a fork or a rewind can go
    /// back to.
    fn started_a_run(&self, text: &str) -> bool {
        self.offers
            .prompts
            .iter()
            .any(|p| !p.is_empty() && text.starts_with(p.as_str()))
    }

    pub fn with_view_mode(mut self, mode: ViewMode) -> Self {
        self.view_mode = mode;
        self
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
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
            Block::User { text, images } => {
                let group = SharedString::from(format!("user-{index}"));
                let started = self.started_a_run(text);
                // Shown on hover, their room kept so the list does not move.
                let offer = |id: &'static str, word: &'static str, event: AgentEvent| {
                    let emit = Self::emitter(pane);
                    let event = Rc::new(event);
                    div()
                        .id((id, index))
                        .debug_selector(move || format!("{id}-{index}"))
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
                        .on_click(move |_, _, cx| emit(event.as_ref().clone(), cx))
                        .child(word)
                };
                let edit = (started && self.offers.edit)
                    .then(|| offer("edit-message", "แก้ไข", AgentEvent::Edit(text.clone())));
                let rewind = (started && self.offers.rewind).then(|| {
                    offer(
                        "rewind-message",
                        "ย้อนมาตรงนี้",
                        AgentEvent::Rewind(text.clone()),
                    )
                });
                let offers = (edit.is_some() || rewind.is_some())
                    .then(|| div().flex().gap_1().children(rewind).children(edit));
                let shown = (!images.is_empty()).then(|| {
                    let blobs = cedian_index::cedia_home().join("blobs");
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap_2()
                        .max_w(relative(0.8))
                        .children(images.iter().map(|image| {
                            let (width, height) = thumbnail(image.width, image.height);
                            img(blobs.join(&image.sha256))
                                .w(px(width))
                                .h(px(height))
                                .rounded(px(12.))
                                .border_1()
                                .border_color(colors.border)
                        }))
                });
                div()
                    .group(group)
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap_1()
                    .children(shown)
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
                    .children(offers)
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
            Block::Thinking {
                text,
                streaming,
                ms,
            } => {
                // One quiet line that opens to the thinking, with how long it took once the
                // turn's assistant event lands.
                let label = match ms {
                    _ if *streaming => "กำลังคิด…".to_owned(),
                    Some(ms) => format!("คิด {}", tool_card::duration(*ms as i64)),
                    None => "ความคิด".to_owned(),
                };
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
                            .debug_selector(move || format!("thinking-{index}"))
                            .child(label)
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
                            undone: self.offers.undone,
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

    /// What the thread's runs cost so far, the one in flight included.
    pub fn cost(&self) -> Option<f64> {
        let live = self.transcript.run_cost();
        let mut total = (live > 0.).then_some(live);
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

/// How big an image the person sent is drawn over their message: its own size, made smaller
/// to fit 240 × 180 without changing its shape.
fn thumbnail(width: f64, height: f64) -> (f32, f32) {
    if width <= 0. || height <= 0. {
        return (180., 180.);
    }
    let scale = (240. / width).min(180. / height).min(1.);
    ((width * scale) as f32, (height * scale) as f32)
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
