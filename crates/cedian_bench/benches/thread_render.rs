use cedian_agent::Thread;
use cedian_omp::{DeltaKind, RouterEvent};
use gpui::{
    AnyElement, BenchAppContext, Context, IntoElement, ListAlignment, ListState, Render,
    ScrollHandle, SharedString, Window, list, point, px,
};
use settings::SettingsStore;
use ui::prelude::*;

const PANEL_WIDTH: f32 = 420.;
const PANEL_HEIGHT: f32 = 900.;

/// The panel's thread as `CedianPanel::render` built it before virtualizing:
/// every history line, message and card is a child of one scrolling column,
/// and the event log is projected again on every frame.
struct ThreadBefore {
    thread: Thread,
    history: Vec<String>,
    scroll: ScrollHandle,
    frame: usize,
    stream: Vec<String>,
}

impl ThreadBefore {
    fn new(thread: Thread, history: Vec<String>) -> Self {
        Self {
            thread,
            history,
            scroll: ScrollHandle::new(),
            frame: 0,
            stream: Vec::new(),
        }
    }

    fn scroll_one_step(&mut self) {
        self.frame += 1;
        let offset = (self.frame % 40) as f32 * 24.;
        self.scroll.set_offset(point(px(0.), px(-offset)));
    }
}

impl Render for ThreadBefore {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let history: Vec<AnyElement> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, line)| {
                div()
                    .debug_selector(move || format!("cedian-history-{i}"))
                    .child(Label::new(line.clone()).color(Color::Muted))
                    .into_any_element()
            })
            .collect();
        let (messages, cards) = cedian_agent_ui::render_thread(self.thread.events());
        let mut rows: Vec<AnyElement> = messages
            .into_iter()
            .map(|m| Label::new(format!("{:?}: {}", m.role, m.text)).into_any_element())
            .collect();
        for card in cards.iter().cloned() {
            let selector = format!("cedian-tool-{}", card.call_id);
            rows.push(
                div()
                    .debug_selector(move || selector)
                    .child(
                        Label::new(format!("[{:?}] {}", card.status, card.display_line()))
                            .color(Color::Muted),
                    )
                    .into_any_element(),
            );
        }
        v_flex().w(px(PANEL_WIDTH)).h(px(PANEL_HEIGHT)).child(
            v_flex()
                .id("cedian-thread")
                .flex_1()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .gap_1()
                .children(history)
                .children(rows),
        )
    }
}

/// The panel's thread as `CedianPanel::render` builds it now: one virtualized
/// `list` over rows rebuilt each frame, only the visible rows laid out.
struct ThreadAfter {
    thread: Thread,
    history: Vec<String>,
    rows: Vec<Row>,
    state: ListState,
    sections: [usize; 3],
    frame: usize,
    stream: Vec<String>,
    /// Rebuild `rows` only when the thread changed, not on every frame.
    cache_rows: bool,
    rows_dirty: bool,
    renders: usize,
    rendered_rows: std::cell::Cell<usize>,
}

enum Row {
    History(usize),
    Message(SharedString),
    Card { call_id: String, line: SharedString },
}

impl ThreadAfter {
    fn new(thread: Thread, history: Vec<String>) -> Self {
        Self {
            thread,
            history,
            rows: Vec::new(),
            state: ListState::new(0, ListAlignment::Top, px(1024.)),
            sections: [0; 3],
            frame: 0,
            stream: Vec::new(),
            cache_rows: false,
            rows_dirty: true,
            renders: 0,
            rendered_rows: std::cell::Cell::new(0),
        }
    }

    fn cached(mut self) -> Self {
        self.cache_rows = true;
        self
    }

    fn scroll_one_step(&mut self) {
        self.frame += 1;
        let step = if self.frame % 80 < 40 { 24. } else { -24. };
        self.state.scroll_by(px(step));
    }

    fn sync_rows(&mut self) {
        let mut rows: Vec<Row> = (0..self.history.len()).map(Row::History).collect();
        let history_rows = rows.len();
        let (messages, cards) = cedian_agent_ui::render_thread(self.thread.events());
        rows.extend(
            messages
                .into_iter()
                .map(|m| Row::Message(format!("{:?}: {}", m.role, m.text).into())),
        );
        let message_rows = rows.len() - history_rows;
        let entries: Vec<Row> = cards
            .iter()
            .cloned()
            .map(|card| Row::Card {
                line: format!("[{:?}] {}", card.status, card.display_line()).into(),
                call_id: card.call_id,
            })
            .collect();
        let entry_rows = entries.len();
        rows.extend(entries);
        self.rows = rows;
        let sections = [history_rows, message_rows, entry_rows];
        for section in (0..sections.len()).rev() {
            let start: usize = self.sections[..section].iter().sum();
            let (old, new) = (self.sections[section], sections[section]);
            if new > old {
                self.state.splice(start + old..start + old, new - old);
            } else if new < old {
                self.state.splice(start + new..start + old, 0);
            }
        }
        self.sections = sections;
    }

    fn render_row(&self, index: usize) -> AnyElement {
        self.rendered_rows.set(self.rendered_rows.get() + 1);
        let row = match self.rows.get(index) {
            Some(Row::History(i)) => {
                let i = *i;
                div()
                    .debug_selector(move || format!("cedian-history-{i}"))
                    .child(
                        Label::new(self.history.get(i).cloned().unwrap_or_default())
                            .color(Color::Muted),
                    )
                    .into_any_element()
            }
            Some(Row::Message(text)) => Label::new(text.clone()).into_any_element(),
            Some(Row::Card { call_id, line }) => {
                let selector = format!("cedian-tool-{call_id}");
                div()
                    .debug_selector(move || selector)
                    .child(Label::new(line.clone()).color(Color::Muted))
                    .into_any_element()
            }
            None => div().into_any_element(),
        };
        div().pb_1().child(row).into_any_element()
    }
}

impl Render for ThreadAfter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        if !self.cache_rows || self.rows_dirty {
            self.sync_rows();
            self.rows_dirty = false;
        }
        v_flex().w(px(PANEL_WIDTH)).h(px(PANEL_HEIGHT)).child(
            list(
                self.state.clone(),
                cx.processor(|this, index, _window, _cx| this.render_row(index)),
            )
            .flex_1(),
        )
    }
}

/// Outside timing: the list drew rows, and fewer than the thread holds.
fn report_rows(view: &gpui::Entity<ThreadAfter>, cx: &mut BenchAppContext) {
    let (renders, rendered, total) = cx.read(|cx| {
        let view = view.read(cx);
        (view.renders, view.rendered_rows.get(), view.rows.len())
    });
    assert!(renders > 0 && rendered > 0, "the list drew no rows");
    eprintln!(
        "rows: {total} in the thread, {:.1} drawn per frame over {renders} frames",
        rendered as f64 / renders as f64
    );
}

fn init_context(cx: &mut BenchAppContext) {
    cx.update(|cx| {
        let store = SettingsStore::new(cx, &settings::default_settings());
        cx.set_global(store);
        assets::Assets.load_test_fonts(cx);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
    });
}

fn turn_counts() -> Vec<usize> {
    vec![25, 100, 400]
}

fn history_counts() -> Vec<usize> {
    vec![100, 500, 2000]
}

fn streaming_thread(turns: usize) -> (Thread, Vec<String>) {
    let mut thread = cedian_bench::thread(turns);
    thread.push_user("one more question");
    let mut deltas = Vec::new();
    for round in 0..64 {
        deltas.extend(cedian_bench::reply_deltas(turns + round));
    }
    deltas.reverse();
    (thread, deltas)
}

#[gpui::bench(inputs = turn_counts(), input_name = "turns", group = "Thread scroll before", sample_size = 10)]
fn thread_scroll_before(turns: &usize, cx: &mut BenchAppContext) {
    init_context(cx);
    let thread = cedian_bench::thread(*turns);
    let mut window = cx.add_empty_window();
    let view = window.update(|window, cx| {
        window.replace_root(cx, |_window, _cx| ThreadBefore::new(thread, Vec::new()))
    });
    cx.bench_renderer(view, |view, _window, cx| {
        view.scroll_one_step();
        cx.notify();
    });
}

#[gpui::bench(inputs = turn_counts(), input_name = "turns", group = "Thread stream before", sample_size = 10)]
fn thread_stream_before(turns: &usize, cx: &mut BenchAppContext) {
    init_context(cx);
    let (thread, stream) = streaming_thread(*turns);
    let mut window = cx.add_empty_window();
    let view = window.update(|window, cx| {
        window.replace_root(cx, |_window, _cx| {
            let mut view = ThreadBefore::new(thread, Vec::new());
            view.stream = stream;
            view
        })
    });
    cx.bench_renderer(view, |view, _window, cx| {
        let delta = view.stream.pop().unwrap_or_else(|| "more ".to_string());
        view.thread.apply(&RouterEvent::MessageDelta {
            message_id: "streaming".to_string(),
            kind: DeltaKind::Text,
            delta,
        });
        cx.notify();
    });
}

#[gpui::bench(inputs = history_counts(), input_name = "lines", group = "History scroll before", sample_size = 10)]
fn history_scroll_before(lines: &usize, cx: &mut BenchAppContext) {
    init_context(cx);
    let history = cedian_bench::history_lines(*lines);
    let mut window = cx.add_empty_window();
    let view = window.update(|window, cx| {
        window.replace_root(cx, |_window, _cx| ThreadBefore::new(Thread::new(), history))
    });
    cx.bench_renderer(view, |view, _window, cx| {
        view.scroll_one_step();
        cx.notify();
    });
}

fn scroll_after(view: ThreadAfter, cx: &mut BenchAppContext) {
    init_context(cx);
    let mut window = cx.add_empty_window();
    let view = window.update(|window, cx| window.replace_root(cx, |_window, _cx| view));
    cx.bench_renderer(view.clone(), |view, _window, cx| {
        view.scroll_one_step();
        cx.notify();
    });
    report_rows(&view, cx);
}

fn stream_after(turns: usize, cache_rows: bool, cx: &mut BenchAppContext) {
    init_context(cx);
    let (thread, stream) = streaming_thread(turns);
    let mut view = ThreadAfter::new(thread, Vec::new());
    view.stream = stream;
    view.cache_rows = cache_rows;
    let mut window = cx.add_empty_window();
    let view = window.update(|window, cx| window.replace_root(cx, |_window, _cx| view));
    cx.bench_renderer(view.clone(), |view, _window, cx| {
        let delta = view.stream.pop().unwrap_or_else(|| "more ".to_string());
        view.thread.apply(&RouterEvent::MessageDelta {
            message_id: "streaming".to_string(),
            kind: DeltaKind::Text,
            delta,
        });
        view.rows_dirty = true;
        cx.notify();
    });
    report_rows(&view, cx);
}

#[gpui::bench(inputs = turn_counts(), input_name = "turns", group = "Thread scroll after", sample_size = 10)]
fn thread_scroll_after(turns: &usize, cx: &mut BenchAppContext) {
    scroll_after(
        ThreadAfter::new(cedian_bench::thread(*turns), Vec::new()),
        cx,
    );
}

#[gpui::bench(inputs = turn_counts(), input_name = "turns", group = "Thread scroll cached", sample_size = 10)]
fn thread_scroll_cached(turns: &usize, cx: &mut BenchAppContext) {
    scroll_after(
        ThreadAfter::new(cedian_bench::thread(*turns), Vec::new()).cached(),
        cx,
    );
}

#[gpui::bench(inputs = turn_counts(), input_name = "turns", group = "Thread stream after", sample_size = 10)]
fn thread_stream_after(turns: &usize, cx: &mut BenchAppContext) {
    stream_after(*turns, false, cx);
}

#[gpui::bench(inputs = turn_counts(), input_name = "turns", group = "Thread stream cached", sample_size = 10)]
fn thread_stream_cached(turns: &usize, cx: &mut BenchAppContext) {
    stream_after(*turns, true, cx);
}

#[gpui::bench(inputs = history_counts(), input_name = "lines", group = "History scroll after", sample_size = 10)]
fn history_scroll_after(lines: &usize, cx: &mut BenchAppContext) {
    scroll_after(
        ThreadAfter::new(Thread::new(), cedian_bench::history_lines(*lines)),
        cx,
    );
}

#[gpui::bench(inputs = history_counts(), input_name = "lines", group = "History scroll cached", sample_size = 10)]
fn history_scroll_cached(lines: &usize, cx: &mut BenchAppContext) {
    scroll_after(
        ThreadAfter::new(Thread::new(), cedian_bench::history_lines(*lines)).cached(),
        cx,
    );
}

gpui::bench_group!(
    benches,
    thread_scroll_before,
    thread_scroll_after,
    thread_scroll_cached,
    thread_stream_before,
    thread_stream_after,
    thread_stream_cached,
    history_scroll_before,
    history_scroll_after,
    history_scroll_cached
);
gpui::bench_main!(benches);
