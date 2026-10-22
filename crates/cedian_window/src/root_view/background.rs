//! The Background tasks block (Interface, Phase C): what runs across the project's threads
//! (each working thread and sub-thread with เปิด and หยุด, and the open thread's running bash
//! calls under it), the scheduled runs `cedia schedules` keeps with a switch each, and the MCP
//! servers in cedia's settings with the tools each gave the open thread's latest run.

use std::collections::HashMap;

use cedian_agent::{Block, ToolState, Transcript};
use cedian_protocol::Schedule;
use cedian_wire::{McpServer, McpTransport, Request, Status, ThreadInfo};
use chrono::{DateTime, Datelike as _, Local, Utc};
use gpui::{
    AnyElement, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
    px, svg,
};
use theme::ActiveTheme as _;

use super::RootView;
use crate::{project::is_chat, thread_list, tokens, tooltip, type_scale::TypeScale as _};

/// How far a sub-thread or a bash call sits in from what it belongs to.
const INDENT: f32 = 18.;
/// The widest the lists get.
const LIST_WIDTH: f32 = 720.;
/// The MCP tools a server's row names before `+n`.
const TOOLS_NAMED: usize = 4;

/// A row of the running list: a thread, or a bash call of one.
enum Running<'a> {
    Thread {
        info: &'a ThreadInfo,
        depth: usize,
    },
    Bash {
        command: String,
        started_at: Option<String>,
        depth: usize,
    },
}

impl RootView {
    /// The Background tasks block.
    pub(super) fn render_background(&self, cx: &mut Context<Self>) -> AnyElement {
        let running = self.render_running(cx);
        let schedules = self.render_schedules(cx);
        let mcp = self.render_mcp(cx);
        div()
            .id("background")
            .size_full()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                // A list reads as one column when the block is zoomed, as the conversation does.
                div()
                    .w_full()
                    .max_w(px(LIST_WIDTH))
                    .mx_auto()
                    .min_w_0()
                    .px_3()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(running)
                    .child(schedules)
                    .child(mcp),
            )
            .into_any_element()
    }

    /// The working threads, each sub-thread under its parent, and the open thread's bash
    /// calls under the thread that runs them (`bash`, by thread).
    fn running_rows<'a>(&'a self, bash: &HashMap<String, Vec<Call>>) -> Vec<Running<'a>> {
        let project = self.project();
        let live = |t: &&ThreadInfo| matches!(t.status, Status::Working | Status::Queued);
        let mut top: Vec<&ThreadInfo> = project.threads.iter().filter(live).collect();
        top.sort_by(|a, b| b.thread.updated_at.cmp(&a.thread.updated_at));
        let subs: Vec<&ThreadInfo> = project.subthreads.iter().filter(live).collect();
        let calls = |thread: &str, depth: usize| -> Vec<Running<'a>> {
            bash.get(thread)
                .into_iter()
                .flatten()
                .map(|(command, started_at)| Running::Bash {
                    command: command.clone(),
                    started_at: started_at.clone(),
                    depth,
                })
                .collect()
        };
        let mut rows = Vec::new();
        let mut shown = Vec::new();
        for info in &top {
            rows.push(Running::Thread { info, depth: 0 });
            rows.extend(calls(&info.thread.id, 1));
            for sub in subs
                .iter()
                .filter(|s| s.thread.parent.as_ref() == Some(&info.thread.id))
            {
                rows.push(Running::Thread {
                    info: sub,
                    depth: 1,
                });
                rows.extend(calls(&sub.thread.id, 2));
                shown.push(sub.thread.id.as_str());
            }
        }
        // A sub-thread still working after its parent's run ended stands on its own.
        for sub in subs
            .iter()
            .filter(|s| !shown.contains(&s.thread.id.as_str()))
        {
            rows.push(Running::Thread {
                info: sub,
                depth: 0,
            });
            rows.extend(calls(&sub.thread.id, 1));
        }
        rows
    }

    fn render_running(&self, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors().clone();
        // Only the open thread's lines are read, so only its calls (and its sub-threads') show.
        let bash: HashMap<String, Vec<Call>> = {
            let transcript = self.agent.read(cx).transcript();
            let mut bash = HashMap::new();
            if let Some(open) = self.project().current.clone() {
                bash.insert(open, bash_calls(transcript));
            }
            for (thread, sub) in transcript.subthreads() {
                bash.insert(thread.clone(), bash_calls(sub));
            }
            bash
        };
        let rows = self.running_rows(&bash);
        let threads = rows
            .iter()
            .filter(|r| matches!(r, Running::Thread { .. }))
            .count();
        let now = Utc::now();
        let mut list = div().flex().flex_col().gap_0p5();
        if rows.is_empty() {
            list = list.child(empty("ไม่มีงานที่กำลังทำ", cx));
        }
        for (n, row) in rows.iter().enumerate() {
            list = list.child(match row {
                Running::Thread { info, depth } => self.thread_row(info, *depth, now, cx),
                Running::Bash {
                    command,
                    started_at,
                    depth,
                } => div()
                    .h(px(28.))
                    .pl(px(8. + INDENT * *depth as f32))
                    .pr_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        svg()
                            .path("icons/terminal.svg")
                            .flex_none()
                            .size(px(12.))
                            .text_color(colors.text_muted),
                    )
                    .child(
                        tooltip::cut(SharedString::from(format!("bg-bash-{n}")), command.clone())
                            .flex_grow(1.)
                            .font(tokens::code_font())
                            .text_label()
                            .text_color(colors.text_muted),
                    )
                    .child(time(
                        started_at
                            .as_deref()
                            .map(|at| elapsed(at, now))
                            .unwrap_or_default(),
                        cx,
                    ))
                    .into_any_element(),
            });
        }
        section("กำลังทำงาน", Some(threads), cx).child(list)
    }

    fn thread_row(
        &self,
        info: &ThreadInfo,
        depth: usize,
        now: DateTime<Utc>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let id = info.thread.id.clone();
        let title = if is_chat(info) {
            SharedString::from("แชทโปรเจกต์")
        } else {
            info.thread.title.clone().into()
        };
        let opens = info.thread.parent.clone().unwrap_or_else(|| id.clone());
        let chat = is_chat(info);
        let working = info.status == Status::Working;
        div()
            .id(SharedString::from(format!("bg-thread-{id}")))
            .h(px(32.))
            .pl(px(8. + INDENT * depth as f32))
            .pr_1()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(8.))
            .hover(|row| row.bg(colors.element_hover))
            .child(thread_list::dot(info.status, cx))
            .child(
                tooltip::cut(SharedString::from(format!("bg-title-{id}")), title)
                    .debug_selector({
                        let id = id.clone();
                        move || format!("bg-title-{id}")
                    })
                    .flex_grow(1.)
                    .text_ui(),
            )
            .child(time(
                if working {
                    thread_list::since(&info.thread.updated_at, now)
                } else {
                    "รอเริ่ม".into()
                },
                cx,
            ))
            .child(
                button(format!("bg-open-{id}"), "เปิด", cx).on_click(cx.listener(
                    move |view, _, _, cx| {
                        if chat {
                            view.open_chat(cx);
                        } else {
                            // A sub-thread is read in its parent's agent pane.
                            view.select(opens.clone(), cx);
                            view.ensure_agent(cx);
                        }
                    },
                )),
            )
            .when(working, |row| {
                let thread = id.clone();
                row.child(
                    button(format!("bg-stop-{id}"), "หยุด", cx).on_click(cx.listener(
                        move |view, _, _, cx| {
                            view.request(Request::Stop {
                                thread: thread.clone(),
                            });
                            cx.notify();
                        },
                    )),
                )
            })
            .into_any_element()
    }

    fn render_schedules(&self, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors().clone();
        let project = self.project();
        let schedules = project
            .background
            .as_ref()
            .and_then(|b| b.schedules.as_ref());
        let mut list = div().flex().flex_col().gap_1();
        match (&project.background, schedules) {
            (None, _) => {}
            (Some(_), None) => list = list.child(empty("cedia รุ่นนี้ยังตั้งเวลาไม่ได้", cx)),
            (Some(_), Some(s)) if s.is_empty() => {
                list = list.child(empty(
                    "ยังไม่มีงานตามเวลา ขอในแชทโปรเจกต์ได้ เช่น ทุกเช้า 9 โมงสรุป PR ที่ค้าง",
                    cx,
                ))
            }
            (Some(_), Some(_)) => {}
        }
        let now = Local::now();
        for schedule in schedules.into_iter().flatten() {
            let thread = project
                .threads
                .iter()
                .chain(&project.subthreads)
                .chain(&project.archived)
                .find(|t| t.thread.id == schedule.thread);
            let thread = match thread {
                Some(t) if is_chat(t) => "แชทโปรเจกต์".to_owned(),
                Some(t) => t.thread.title.clone(),
                None => schedule.thread.clone(),
            };
            let meta = schedule_meta(schedule, &thread, now);
            let id = schedule.id.clone();
            let on = schedule.enabled;
            let cwd = project.cwd.clone();
            let prompt = schedule
                .prompt
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned();
            list = list.child(
                div()
                    .px_2()
                    .py_1p5()
                    .flex()
                    .flex_col()
                    .rounded(px(8.))
                    .hover(|row| row.bg(colors.element_hover))
                    .child(
                        div()
                            .h(px(21.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                svg()
                                    .path("icons/clock.svg")
                                    .flex_none()
                                    .size(px(13.))
                                    .text_color(colors.text_muted),
                            )
                            .child(
                                tooltip::cut(SharedString::from(format!("bg-prompt-{id}")), prompt)
                                    .debug_selector({
                                        let id = id.clone();
                                        move || format!("bg-prompt-{id}")
                                    })
                                    .flex_grow(1.)
                                    .text_ui()
                                    .when(!on, |t| t.text_color(colors.text_muted)),
                            )
                            .child(switch(format!("bg-switch-{id}"), on, cx).on_click(
                                cx.listener(move |view, _, _, cx| {
                                    view.set_schedule(&cwd, &id, !on, cx)
                                }),
                            )),
                    )
                    .child(
                        tooltip::cut(SharedString::from(format!("bg-when-{}", schedule.id)), meta)
                            .pl(px(21.))
                            .font(tokens::numeric_font())
                            .text_label()
                            .text_color(colors.text_placeholder),
                    ),
            );
        }
        section("ตามเวลา", schedules.map(Vec::len), cx).child(list)
    }

    /// Turns a schedule off or on; the switch moves now and the daemon's list confirms it.
    fn set_schedule(
        &mut self,
        cwd: &std::path::Path,
        id: &str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(schedule) = self
            .projects
            .iter_mut()
            .find(|p| p.cwd == cwd)
            .and_then(|p| p.background.as_mut())
            .and_then(|b| b.schedules.as_mut())
            .and_then(|s| s.iter_mut().find(|s| s.id == id))
        {
            schedule.enabled = enabled;
        }
        self.request(Request::Schedule {
            cwd: cwd.to_owned(),
            id: id.to_owned(),
            enabled,
        });
        cx.notify();
    }

    fn render_mcp(&self, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors().clone();
        let status = cx.theme().status().clone();
        let servers: &[McpServer] = self
            .project()
            .background
            .as_ref()
            .map_or(&[], |b| b.mcp.as_slice());
        let tools = self.agent.read(cx).transcript().tools().to_vec();
        // No session read yet: the open thread has not run, so nothing says what connected.
        let ran = !tools.is_empty();
        let mut list = div().flex().flex_col().gap_1();
        if self.project().background.is_some() && servers.is_empty() {
            list = list.child(empty(
                "ยังไม่มี MCP server ตั้งได้ที่ mcpServers ใน ~/.cedia/settings.json",
                cx,
            ));
        }
        for server in servers {
            let prefix = format!("mcp__{}__", server.name);
            let own: Vec<&str> = tools
                .iter()
                .filter_map(|t| t.strip_prefix(prefix.as_str()))
                .collect();
            let (state, color) = match (ran, own.len()) {
                (false, _) => ("ยังไม่ได้ใช้".to_owned(), colors.text_placeholder),
                (true, 0) => ("ไม่ได้ต่อในรอบล่าสุด".to_owned(), status.warning),
                (true, n) => (format!("ต่อแล้ว · {n} tools"), colors.text_muted),
            };
            let mut meta = vec![match &server.transport {
                McpTransport::Command { program } => program.clone(),
                McpTransport::Url { host } => host.clone(),
            }];
            if server.read_only {
                meta.push("อ่านอย่างเดียว".into());
            }
            if !own.is_empty() {
                let mut named = own
                    .iter()
                    .take(TOOLS_NAMED)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ");
                if own.len() > TOOLS_NAMED {
                    named.push_str(&format!(" +{}", own.len() - TOOLS_NAMED));
                }
                meta.push(named);
            }
            let name = server.name.clone();
            list = list.child(
                div()
                    .px_2()
                    .py_1p5()
                    .flex()
                    .flex_col()
                    .rounded(px(8.))
                    .hover(|row| row.bg(colors.element_hover))
                    .child(
                        div()
                            .h(px(21.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                svg()
                                    .path("icons/server.svg")
                                    .flex_none()
                                    .size(px(13.))
                                    .text_color(colors.text_muted),
                            )
                            .child(
                                tooltip::cut(
                                    SharedString::from(format!("bg-mcp-{name}")),
                                    name.clone(),
                                )
                                .debug_selector({
                                    let name = name.clone();
                                    move || format!("bg-mcp-{name}")
                                })
                                .flex_grow(1.)
                                .text_ui(),
                            )
                            .child(
                                div()
                                    .debug_selector({
                                        let name = name.clone();
                                        move || format!("bg-mcp-state-{name}")
                                    })
                                    .flex_none()
                                    .text_label()
                                    .text_color(color)
                                    .child(state),
                            ),
                    )
                    .child(
                        tooltip::cut(
                            SharedString::from(format!("bg-mcp-meta-{name}")),
                            meta.join(" · "),
                        )
                        .pl(px(21.))
                        .text_label()
                        .text_color(colors.text_placeholder),
                    ),
            );
        }
        section("MCP", Some(servers.len()), cx).child(list)
    }
}

/// A running bash call: its command's first line and when it started.
type Call = (String, Option<String>);

/// The running bash calls of one transcript.
fn bash_calls(transcript: &Transcript) -> Vec<Call> {
    transcript
        .blocks()
        .iter()
        .filter_map(|block| match block {
            Block::Tool(call) if call.name == "bash" && call.state == ToolState::Running => {
                let command = call
                    .input
                    .get("command")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default();
                Some((
                    command.lines().next().unwrap_or_default().to_owned(),
                    call.started_at.clone(),
                ))
            }
            _ => None,
        })
        .collect()
}

/// A section's title over its list, with its count.
fn section(title: &'static str, count: Option<usize>, cx: &mut Context<RootView>) -> Div {
    let colors = cx.theme().colors();
    div().flex().flex_col().gap_1().child(
        div()
            .h(px(20.))
            .px_2()
            .flex()
            .items_center()
            .gap_1p5()
            .text_label()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.text_muted)
            .child(title)
            .when_some(count.filter(|n| *n > 0), |row, n| {
                row.child(
                    div()
                        .font(tokens::numeric_font())
                        .text_color(colors.text_placeholder)
                        .child(n.to_string()),
                )
            }),
    )
}

/// What a section says when it has nothing; it may wrap.
fn empty(text: &'static str, cx: &mut Context<RootView>) -> AnyElement {
    div()
        .px_2()
        .text_meta()
        .text_color(cx.theme().colors().text_placeholder)
        .child(text)
        .into_any_element()
}

fn time(text: String, cx: &mut Context<RootView>) -> Div {
    div()
        .flex_none()
        .font(tokens::numeric_font())
        .text_label()
        .text_color(cx.theme().colors().text_placeholder)
        .child(text)
}

/// A small bordered button that never shrinks.
fn button(id: String, label: &'static str, cx: &mut Context<RootView>) -> gpui::Stateful<Div> {
    let colors = cx.theme().colors();
    div()
        .debug_selector({
            let id = id.clone();
            move || id
        })
        .id(SharedString::from(id))
        .flex_none()
        .h(px(24.))
        .px_2()
        .flex()
        .items_center()
        .rounded(px(6.))
        .border_1()
        .border_color(colors.border)
        .cursor_pointer()
        .text_label()
        .hover(|b| b.bg(colors.element_hover))
        .child(label)
}

/// A switch: filled with `text` when on (lime stays the person's), `border` when off.
fn switch(id: String, on: bool, cx: &mut Context<RootView>) -> gpui::Stateful<Div> {
    let colors = cx.theme().colors();
    div()
        .debug_selector({
            let id = id.clone();
            move || id
        })
        .id(SharedString::from(id))
        .flex_none()
        .w(px(28.))
        .h(px(16.))
        .p(px(2.))
        .flex()
        .items_center()
        .rounded_full()
        .cursor_pointer()
        .map(|s| {
            if on {
                s.justify_end().bg(colors.text)
            } else {
                s.justify_start().bg(colors.border)
            }
        })
        .child(div().size(px(12.)).rounded_full().bg(if on {
            colors.background
        } else {
            colors.text_muted
        }))
}

/// A schedule's second line: how often, when next, when last, and the thread it wakes.
fn schedule_meta(schedule: &Schedule, thread: &str, now: DateTime<Local>) -> String {
    let mut parts = vec![match schedule.every {
        Some(minutes) => every(minutes),
        None => "ครั้งเดียว".to_owned(),
    }];
    match (&schedule.next, schedule.enabled) {
        (_, false) => parts.push("ปิดอยู่".into()),
        (Some(next), true) => parts.push(format!("ถัดไป {}", when(next, now))),
        (None, true) => parts.push("รันแล้ว".into()),
    }
    if let Some(last) = &schedule.last {
        parts.push(format!(
            "ล่าสุด {}",
            thread_list::since(last, now.with_timezone(&Utc))
        ));
    }
    parts.push(thread.to_owned());
    parts.join(" · ")
}

/// cedia's minutes between runs, in words.
fn every(minutes: f64) -> String {
    let minutes = minutes.round() as i64;
    match minutes {
        m if m % 10_080 == 0 && m / 10_080 == 1 => "ทุกสัปดาห์".into(),
        m if m % 10_080 == 0 => format!("ทุก {} สัปดาห์", m / 10_080),
        1440 => "ทุกวัน".into(),
        m if m % 1440 == 0 => format!("ทุก {} วัน", m / 1440),
        60 => "ทุกชั่วโมง".into(),
        m if m % 60 == 0 => format!("ทุก {} ชม.", m / 60),
        m => format!("ทุก {m} นาที"),
    }
}

const MONTHS: [&str; 12] = [
    "ม.ค.",
    "ก.พ.",
    "มี.ค.",
    "เม.ย.",
    "พ.ค.",
    "มิ.ย.",
    "ก.ค.",
    "ส.ค.",
    "ก.ย.",
    "ต.ค.",
    "พ.ย.",
    "ธ.ค.",
];

/// A time to come, in the person's time zone: วันนี้ 14:30, พรุ่งนี้ 09:00 or 3 พ.ย. 09:00.
fn when(at: &str, now: DateTime<Local>) -> String {
    let Ok(at) = DateTime::parse_from_rfc3339(at) else {
        return String::new();
    };
    let at = at.with_timezone(&Local);
    if at <= now {
        return "ถึงเวลาแล้ว".into();
    }
    let clock = at.format("%H:%M");
    let days = (at.date_naive() - now.date_naive()).num_days();
    match days {
        0 => format!("วันนี้ {clock}"),
        1 => format!("พรุ่งนี้ {clock}"),
        _ => format!("{} {} {clock}", at.day(), MONTHS[at.month0() as usize]),
    }
}

/// How long a call has run: 12 วิ, 3 นาที, 1 ชม.
fn elapsed(from: &str, now: DateTime<Utc>) -> String {
    let Ok(from) = DateTime::parse_from_rfc3339(from) else {
        return String::new();
    };
    let seconds = (now - from.with_timezone(&Utc)).num_seconds().max(0);
    match seconds {
        0..=59 => format!("{seconds} วิ"),
        60..=3599 => format!("{} นาที", seconds / 60),
        _ => format!("{} ชม.", seconds / 3600),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;

    #[test]
    fn a_period_reads_in_the_largest_whole_unit() {
        assert_eq!(every(30.), "ทุก 30 นาที");
        assert_eq!(every(60.), "ทุกชั่วโมง");
        assert_eq!(every(120.), "ทุก 2 ชม.");
        assert_eq!(every(90.), "ทุก 90 นาที");
        assert_eq!(every(1440.), "ทุกวัน");
        assert_eq!(every(4320.), "ทุก 3 วัน");
        assert_eq!(every(10_080.), "ทุกสัปดาห์");
        assert_eq!(every(20_160.), "ทุก 2 สัปดาห์");
    }

    #[test]
    fn a_next_run_names_its_day() {
        let now = Local.with_ymd_and_hms(2026, 10, 22, 10, 0, 0).unwrap();
        let at = |d, h| {
            Local
                .with_ymd_and_hms(2026, 10, d, h, 30, 0)
                .unwrap()
                .to_rfc3339()
        };
        assert_eq!(when(&at(22, 14), now), "วันนี้ 14:30");
        assert_eq!(when(&at(23, 9), now), "พรุ่งนี้ 09:30");
        assert_eq!(when(&at(25, 9), now), "25 ต.ค. 09:30");
        assert_eq!(when(&at(22, 9), now), "ถึงเวลาแล้ว");
    }

    #[test]
    fn a_schedules_line_says_how_often_when_and_where() {
        let schedule: Schedule = serde_json::from_value(serde_json::json!({
            "id": "s-1", "prompt": "summarise", "thread": "t-1", "every": 1440,
            "enabled": true, "createdAt": "2026-10-20T00:00:00.000Z"
        }))
        .unwrap();
        let now = Local.with_ymd_and_hms(2026, 10, 22, 10, 0, 0).unwrap();
        assert_eq!(
            schedule_meta(&schedule, "แชทโปรเจกต์", now),
            "ทุกวัน · รันแล้ว · แชทโปรเจกต์"
        );
        let paused = Schedule {
            enabled: false,
            every: None,
            ..schedule
        };
        assert_eq!(schedule_meta(&paused, "fix", now), "ครั้งเดียว · ปิดอยู่ · fix");
    }

    /// A long title, prompt and server name at every size: the buttons, the switch and the
    /// state stay whole and inside the block, and the text gives way.
    #[test]
    fn nothing_in_the_block_leaks_at_any_size() {
        use cedian_layout::Kind;
        use cedian_wire::Notice;
        use gpui::{px, size};

        use super::super::{blocks::ToolTarget, window_tests as w};

        let long =
            "ทำให้ระบบ build ทั้งหมดผ่านบน macOS และ Linux พร้อมแก้ test ที่ล้มทุกตัวในทุก crate ของ workspace";
        let thread = |id: &str, parent: Option<&str>| {
            serde_json::from_value::<cedian_wire::ThreadInfo>(serde_json::json!({
                "id": id, "title": long, "task": long, "state": "working", "result": null,
                "parent": parent, "archived": false, "sessions": [], "messages": [],
                "pending": [], "updated_at": "2026-10-22T10:00:00.000Z",
                "status": "working", "cwd": null, "needs_you": false
            }))
            .expect("thread")
        };
        let mut app = w::app();
        let (view, cx) = w::window(&mut app);
        view.update_in(cx, |view, _, cx| {
            let cwd = view.project().cwd.clone();
            view.on_notice(
                Notice::Threads {
                    cwd: cwd.clone(),
                    threads: vec![thread("t-long", None), thread("t-sub", Some("t-long"))],
                },
                cx,
            );
            view.on_notice(
                Notice::Background {
                    cwd,
                    schedules: Some(vec![
                        serde_json::from_value(serde_json::json!({
                            "id": "s-1", "prompt": long, "thread": "t-long", "every": 1440,
                            "enabled": true, "next": "2026-10-23T02:00:00.000Z",
                            "last": "2026-10-22T02:00:00.000Z",
                            "createdAt": "2026-10-20T00:00:00.000Z"
                        }))
                        .expect("schedule"),
                    ]),
                    mcp: vec![McpServer {
                        name: "a-server-with-a-name-as-long-as-cedia-lets-it-be".into(),
                        transport: McpTransport::Url {
                            host: "mcp.a-very-long-host-name.example.com:8443".into(),
                        },
                        read_only: true,
                    }],
                },
                cx,
            );
            view.open_kind(Kind::Background, ToolTarget { against: None }, cx);
        });
        let server = "a-server-with-a-name-as-long-as-cedia-lets-it-be";
        // The block scrolls down, so a part below its fold is fine; across, nothing may leak.
        #[track_caller]
        fn across(
            part: &str,
            inner: gpui::Bounds<gpui::Pixels>,
            block: gpui::Bounds<gpui::Pixels>,
        ) {
            let slack = px(0.5);
            assert!(
                inner.left() >= block.left() - slack && inner.right() <= block.right() + slack,
                "{part} {inner:?} leaks out of the Background block {block:?}"
            );
        }
        for (width, height) in w::SIZES.into_iter().chain([(720., 600.)]) {
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();
            let block = w::bounds(cx, "background-block");
            for part in [
                "bg-title-t-long",
                "bg-open-t-long",
                "bg-stop-t-long",
                "bg-title-t-sub",
                "bg-stop-t-sub",
                "bg-prompt-s-1",
                "bg-switch-s-1",
            ] {
                let bounds = w::bounds(cx, part);
                across(part, bounds, block);
                assert!(bounds.size.height <= px(32.), "{part} wraps: {bounds:?}");
            }
            for part in [format!("bg-mcp-{server}"), format!("bg-mcp-state-{server}")] {
                let part: &'static str = Box::leak(part.into_boxed_str());
                across(part, w::bounds(cx, part), block);
            }
            let title = w::bounds(cx, "bg-title-t-long");
            w::apart("the title", title, "เปิด", w::bounds(cx, "bg-open-t-long"));
            let prompt = w::bounds(cx, "bg-prompt-s-1");
            w::apart(
                "the prompt",
                prompt,
                "its switch",
                w::bounds(cx, "bg-switch-s-1"),
            );
            let name = w::bounds(cx, Box::leak(format!("bg-mcp-{server}").into_boxed_str()));
            let state = w::bounds(
                cx,
                Box::leak(format!("bg-mcp-state-{server}").into_boxed_str()),
            );
            w::apart("the server's name", name, "its state", state);
        }
    }
}
