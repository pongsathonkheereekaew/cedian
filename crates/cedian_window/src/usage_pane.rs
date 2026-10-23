//! The Usage block (Interface, Phase C: Usage): where the spend went, priced by cedia
//! (`cedia usage --json --since`, through the daemon). One row picks the range (7 วัน, 30 วัน,
//! เดือนนี้) and whether it is this project or every one; tiles give today, the range, the
//! average per day and the requests; the cost per day is one series of bars with a readout on
//! hover, or a table; then the cost by model, by project and the costliest threads.

use std::{collections::HashMap, path::PathBuf};

use cedian_protocol::UsageReport;
use chrono::{Datelike as _, Days, Local, NaiveDate};
use gpui::{
    AnyElement, Context, EventEmitter, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px, relative,
};
use theme::ActiveTheme as _;

use crate::{tokens, tooltip, type_scale::TypeScale as _};

/// The bars' height, and the widest a bar gets so a short range keeps thin marks.
const CHART_HEIGHT: f32 = 120.;
const BAR_MAX_WIDTH: f32 = 24.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Week,
    Days30,
    ThisMonth,
}

impl Range {
    const ALL: [Range; 3] = [Range::Week, Range::Days30, Range::ThisMonth];

    fn label(self) -> &'static str {
        match self {
            Range::Week => "7 วัน",
            Range::Days30 => "30 วัน",
            Range::ThisMonth => "เดือนนี้",
        }
    }

    /// The first day in the range, counting `today`.
    pub fn first_day(self, today: NaiveDate) -> NaiveDate {
        let back = |n: u64| today.checked_sub_days(Days::new(n)).unwrap_or(today);
        match self {
            Range::Week => back(6),
            Range::Days30 => back(29),
            Range::ThisMonth => today.with_day(1).unwrap_or(today),
        }
    }

    /// What `cedia usage --since` takes for it.
    pub fn since(self, today: NaiveDate) -> String {
        match self {
            Range::Week => "7d".into(),
            Range::Days30 => "30d".into(),
            Range::ThisMonth => self.first_day(today).format("%Y-%m-%d").to_string(),
        }
    }
}

/// The report asked for: every project (None) or one, and the range as cedia takes it.
type Asked = (Option<PathBuf>, String);

pub enum UsageEvent {
    /// The range or the project filter changed: ask again.
    Changed,
}

pub struct UsagePane {
    range: Range,
    all: bool,
    asked: Option<Asked>,
    report: Option<UsageReport>,
    /// The project names by cedia's project key, for the breakdown by project.
    names: HashMap<String, String>,
    hovered: Option<usize>,
    table: bool,
}

impl EventEmitter<UsageEvent> for UsagePane {}

impl UsagePane {
    pub fn new() -> Self {
        Self {
            range: Range::Week,
            all: false,
            asked: None,
            report: None,
            names: HashMap::new(),
            hovered: None,
            table: false,
        }
    }

    /// What to ask the daemon for, once, when the range, the filter or the project (`cwd`)
    /// changed since the last ask.
    pub fn take_want(&mut self, cwd: &std::path::Path, today: NaiveDate) -> Option<Asked> {
        let want = (
            (!self.all).then(|| cwd.to_path_buf()),
            self.range.since(today),
        );
        if self.asked.as_ref() == Some(&want) {
            return None;
        }
        self.asked = Some(want.clone());
        self.report = None;
        self.hovered = None;
        Some(want)
    }

    pub fn set_names(&mut self, names: HashMap<String, String>) {
        self.names = names;
    }

    /// A report arrived; one for an ask since replaced is dropped.
    pub fn set_report(
        &mut self,
        cwd: Option<PathBuf>,
        since: String,
        report: UsageReport,
        cx: &mut Context<Self>,
    ) {
        if self.asked.as_ref() == Some(&(cwd, since)) {
            self.report = Some(report);
            cx.notify();
        }
    }

    /// Asks again for the same range, for figures that moved since.
    fn reload(&mut self, cx: &mut Context<Self>) {
        self.asked = None;
        cx.emit(UsageEvent::Changed);
    }

    fn pick(&mut self, range: Range, cx: &mut Context<Self>) {
        if self.range != range {
            self.range = range;
            cx.emit(UsageEvent::Changed);
        }
    }

    fn set_all(&mut self, all: bool, cx: &mut Context<Self>) {
        if self.all != all {
            self.all = all;
            cx.emit(UsageEvent::Changed);
        }
    }

    #[cfg(test)]
    pub fn report(&self) -> Option<&UsageReport> {
        self.report.as_ref()
    }
}

/// Each day of the range with its cost, a day with no spend at zero.
pub fn days(report: &UsageReport, first: NaiveDate, today: NaiveDate) -> Vec<(NaiveDate, f64)> {
    let cost: HashMap<&str, f64> = report
        .days
        .iter()
        .map(|d| (d.day.as_str(), d.cost_usd))
        .collect();
    first
        .iter_days()
        .take_while(|day| *day <= today)
        .map(|day| {
            let key = day.format("%Y-%m-%d").to_string();
            (day, cost.get(key.as_str()).copied().unwrap_or(0.))
        })
        .collect()
}

/// Dollars as the window shows money: cents, or four places under a cent.
pub fn money(usd: f64) -> String {
    if usd > 0. && usd < 0.01 {
        format!("${usd:.4}")
    } else {
        format!("${usd:.2}")
    }
}

fn day_label(day: NaiveDate) -> String {
    day.format("%-d/%-m").to_string()
}

impl UsagePane {
    fn render_filters(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let chip = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .h(px(26.))
                .px_2()
                .flex()
                .items_center()
                .rounded(px(6.))
                .border_1()
                .cursor_pointer()
                .text_meta()
                .map(|c| {
                    if on {
                        c.bg(colors.element_selected)
                            .border_color(colors.border_selected)
                            .text_color(colors.text)
                    } else {
                        c.border_color(colors.border)
                            .text_color(colors.text_muted)
                            .hover(|c| c.bg(colors.element_hover))
                    }
                })
                .child(label)
        };
        let mut row = div().flex().flex_wrap().items_center().gap_1();
        for range in Range::ALL {
            let id = match range {
                Range::Week => "usage-7d",
                Range::Days30 => "usage-30d",
                Range::ThisMonth => "usage-month",
            };
            row = row.child(
                chip(id, range.label(), self.range == range)
                    .on_click(cx.listener(move |pane, _, _, cx| pane.pick(range, cx))),
            );
        }
        row.child(div().w(px(8.)))
            .child(
                chip("usage-here", "โปรเจกต์นี้", !self.all)
                    .on_click(cx.listener(|pane, _, _, cx| pane.set_all(false, cx))),
            )
            .child(
                chip("usage-all", "ทุกโปรเจกต์", self.all)
                    .on_click(cx.listener(|pane, _, _, cx| pane.set_all(true, cx))),
            )
            .child(div().flex_grow(1.))
            .child(
                chip("usage-table", "ตาราง", self.table)
                    .debug_selector(|| "usage-table-toggle".into())
                    .on_click(cx.listener(|pane, _, _, cx| {
                        pane.table = !pane.table;
                        cx.notify();
                    })),
            )
            .child(
                chip("usage-reload", "โหลดใหม่", false)
                    .on_click(cx.listener(|pane, _, _, cx| pane.reload(cx))),
            )
            .into_any_element()
    }

    fn render_chart(&self, days: &[(NaiveDate, f64)], cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let top = days.iter().map(|(_, c)| *c).fold(0., f64::max);
        let readout = match self.hovered.and_then(|n| days.get(n)) {
            Some((day, cost)) => format!("{} · {}", day.format("%Y-%m-%d"), money(*cost)),
            None => "ชี้ที่แท่งเพื่อดูยอดของวันนั้น".into(),
        };
        let mut bars = div()
            .h(px(CHART_HEIGHT))
            .flex()
            .items_end()
            .gap(px(2.))
            .border_b_1()
            .border_color(colors.border_variant);
        for (n, (_, cost)) in days.iter().enumerate() {
            let share = if top > 0. { (*cost / top) as f32 } else { 0. };
            let hovered = self.hovered == Some(n);
            let fill: Hsla = if hovered {
                colors.text
            } else {
                colors.text_muted.opacity(0.55)
            };
            bars = bars.child(
                // The whole column takes the hover, not just the bar.
                div()
                    .id(("usage-day", n))
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_end()
                    .justify_center()
                    .on_hover(cx.listener(move |pane, over: &bool, _, cx| {
                        let next = if *over {
                            Some(n)
                        } else if pane.hovered == Some(n) {
                            None
                        } else {
                            pane.hovered
                        };
                        if pane.hovered != next {
                            pane.hovered = next;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(BAR_MAX_WIDTH))
                            // A day with spend shows at least a sliver.
                            .h(relative(share.max(if *cost > 0. { 0.02 } else { 0. })))
                            .rounded_t(px(4.))
                            .bg(fill),
                    ),
            );
        }
        let first = days.first().map(|(d, _)| day_label(*d)).unwrap_or_default();
        let last = days.last().map(|(d, _)| day_label(*d)).unwrap_or_default();
        div()
            .debug_selector(|| "usage-chart".into())
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_meta()
                    .text_color(colors.text_muted)
                    .child("ค่าใช้จ่ายรายวัน")
                    .child(
                        div()
                            .font(tokens::numeric_font())
                            .text_color(if self.hovered.is_some() {
                                colors.text
                            } else {
                                colors.text_muted
                            })
                            .child(readout),
                    ),
            )
            .child(bars)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_label()
                    .font(tokens::numeric_font())
                    .text_color(colors.text_muted)
                    .child(first)
                    .child(format!("สูงสุด {}", money(top)))
                    .child(last),
            )
            .into_any_element()
    }

    fn render_table(&self, days: &[(NaiveDate, f64)], cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let mut table = div()
            .debug_selector(|| "usage-table".into())
            .flex()
            .flex_col()
            .text_meta();
        for (day, cost) in days.iter().rev() {
            table = table.child(
                div()
                    .flex()
                    .justify_between()
                    .py(px(3.))
                    .border_b_1()
                    .border_color(colors.border_variant)
                    .font(tokens::numeric_font())
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .child(day.format("%Y-%m-%d").to_string()),
                    )
                    .child(div().text_color(colors.text).child(money(*cost))),
            );
        }
        table.into_any_element()
    }
}

/// A breakdown: each row's name, cost and a thin bar of its share of the largest.
fn breakdown(
    title: &'static str,
    rows: Vec<(SharedString, f64, Option<String>)>,
    cx: &gpui::App,
) -> AnyElement {
    let colors = cx.theme().colors();
    let top = rows.iter().map(|(_, c, _)| *c).fold(0., f64::max);
    let mut list = div().flex().flex_col().gap(px(6.));
    for (n, (name, cost, note)) in rows.into_iter().enumerate() {
        let share = if top > 0. { (cost / top) as f32 } else { 0. };
        list = list.child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_meta()
                        .child(
                            tooltip::cut((title, n), name)
                                .flex_grow(1.)
                                .text_color(colors.text),
                        )
                        .when_some(note, |row, note| {
                            row.child(
                                div()
                                    .flex_none()
                                    .text_color(colors.text_muted)
                                    .font(tokens::numeric_font())
                                    .child(note),
                            )
                        })
                        .child(
                            div()
                                .flex_none()
                                .font(tokens::numeric_font())
                                .text_color(colors.text)
                                .child(money(cost)),
                        ),
                )
                .child(
                    div()
                        .h(px(3.))
                        .w_full()
                        .rounded(px(2.))
                        .bg(colors.border_variant)
                        .child(
                            div()
                                .h_full()
                                .w(relative(share))
                                .rounded(px(2.))
                                .bg(colors.text_muted.opacity(0.55)),
                        ),
                ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_label()
                .text_color(colors.text_muted)
                .child(title),
        )
        .child(list)
        .into_any_element()
}

fn tile(label: &'static str, value: String, cx: &gpui::App) -> AnyElement {
    let colors = cx.theme().colors();
    div()
        .flex_1()
        .min_w(px(110.))
        .p_3()
        .rounded(px(10.))
        .border_1()
        .border_color(colors.border_variant)
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_label()
                .text_color(colors.text_muted)
                .child(label),
        )
        .child(
            div()
                .text_title()
                .font(tokens::numeric_font())
                .text_color(colors.text)
                .child(value),
        )
        .into_any_element()
}

impl Render for UsagePane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let filters = self.render_filters(cx);
        let body = match &self.report {
            None => div()
                .py_6()
                .text_ui()
                .text_color(colors.text_muted)
                .child("กำลังอ่านค่าใช้จ่ายจาก cedia…")
                .into_any_element(),
            Some(report) => {
                let today = Local::now().date_naive();
                let days = days(report, self.range.first_day(today), today);
                let today_cost = days.last().map(|(_, c)| *c).unwrap_or(0.);
                let per_day = report.cost_usd / days.len().max(1) as f64;
                let models = report
                    .models
                    .iter()
                    .map(|m| {
                        (
                            m.model.clone().into(),
                            m.cost_usd,
                            Some(format!("{} คำขอ", m.requests as u64)),
                        )
                    })
                    .collect();
                let projects = report
                    .projects
                    .iter()
                    .map(|p| {
                        let name = self
                            .names
                            .get(&p.project)
                            .cloned()
                            .unwrap_or(p.project.clone());
                        (name.into(), p.cost_usd, None)
                    })
                    .collect();
                let threads = report
                    .threads
                    .iter()
                    .map(|t| {
                        let project = t
                            .project
                            .as_ref()
                            .filter(|_| self.all)
                            .map(|p| self.names.get(p).cloned().unwrap_or(p.clone()));
                        (t.title.clone().into(), t.cost_usd, project)
                    })
                    .collect();
                let chart = if self.table {
                    self.render_table(&days, cx)
                } else {
                    self.render_chart(&days, cx)
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(tile("วันนี้", money(today_cost), cx))
                            .child(tile("ช่วงนี้", money(report.cost_usd), cx))
                            .child(tile("เฉลี่ยต่อวัน", money(per_day), cx))
                            .child(tile("คำขอ", format!("{}", report.requests as u64), cx)),
                    )
                    .child(chart)
                    .when(report.unpriced > 0., |b| {
                        b.child(
                            div()
                                .text_meta()
                                .text_color(colors.text_muted)
                                .child(format!(
                                    "{} คำตอบมาจาก model ที่ cedia ไม่รู้ราคา จึงไม่ได้นับ",
                                    report.unpriced as u64
                                )),
                        )
                    })
                    .child(breakdown("ตาม model", models, cx))
                    .when(self.all, |b| b.child(breakdown("ตามโปรเจกต์", projects, cx)))
                    .child(breakdown("thread ที่แพงสุด", threads, cx))
                    .into_any_element()
            }
        };
        div()
            .id("usage")
            .debug_selector(|| "usage-pane".into())
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(filters)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
    }

    #[test]
    fn ranges_count_today_and_this_month_starts_on_the_first() {
        let today = date("2026-10-22");
        assert_eq!(Range::Week.first_day(today), date("2026-10-16"));
        assert_eq!(Range::Days30.first_day(today), date("2026-09-23"));
        assert_eq!(Range::ThisMonth.since(today), "2026-10-01");
        assert_eq!(Range::Week.since(today), "7d");
    }

    #[test]
    fn every_day_of_the_range_is_a_bar_and_a_quiet_day_is_zero() {
        let report: UsageReport = serde_json::from_str(
            r#"{"sessions":1,"requests":2,"costUsd":0.3,"keepaliveUsd":0,"compactUsd":0,
            "tokens":{"input":1,"cacheRead":0,"output":1},"sources":[],"largest":[],"misses":[],
            "overPriceLine":0,"maxEffort":0,"handoffs":0,"unpriced":0,
            "days":[{"day":"2026-10-20","costUsd":0.1},{"day":"2026-10-22","costUsd":0.2}],
            "models":[],"projects":[],"threads":[]}"#,
        )
        .expect("a report");
        let days = days(&report, date("2026-10-19"), date("2026-10-22"));
        let costs: Vec<f64> = days.iter().map(|(_, c)| *c).collect();
        assert_eq!(costs, [0., 0.1, 0., 0.2]);
    }

    #[test]
    fn money_keeps_cents_and_shows_less_than_one() {
        assert_eq!(money(1.234), "$1.23");
        assert_eq!(money(0.0042), "$0.0042");
        assert_eq!(money(0.), "$0.00");
    }
}
