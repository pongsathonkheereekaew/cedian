//! What the open thread's next run runs on, in the composer's bottom row: a model chip and a
//! permission chip, each with a menu of what cedia lists (`cedia models`, `cedia permissions`).
//! A model that takes an effort shows it after its name, and its menu ends with the levels.
//! A pick holds for its thread until the window closes; a thread with none shows its writer's
//! model, the model's own effort and cedia's default mode, and cedia decides all three.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use cedian_protocol::{Effort, ModelEntry, ModelList, PermissionList, PermissionSetting};
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, deferred, div, prelude::FluentBuilder as _, px,
    svg,
};
use theme::ActiveTheme as _;

use super::RootView;
use crate::{
    on_click::OnClick,
    popup_menu::{self, Row},
    type_scale::TypeScale as _,
};

/// The run menus' width; a mode's description wraps inside it.
const MENU_WIDTH: f32 = 300.;

/// The models and modes cedia listed, and the person's picks.
#[derive(Default)]
pub(super) struct RunChoices {
    models: Option<ModelList>,
    permissions: Option<PermissionList>,
    /// Picks by thread id.
    picks: HashMap<String, Pick>,
    /// The pick for the thread the composer would start.
    new_thread: Pick,
    pub(super) menu: Option<Menu>,
    /// The menu a press outside it just closed: a press on its own chip closes it, and the
    /// click that follows must not open it again.
    closed: Option<(Menu, Instant)>,
}

#[derive(Clone, Default)]
pub(super) struct Pick {
    pub model: Option<String>,
    pub permission: Option<PermissionSetting>,
    /// Sent only while the model takes it: a pick made on another model waits.
    pub effort: Option<Effort>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Menu {
    Model,
    Permission,
}

/// An effort level's word in the chip and its menu.
fn effort_word(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "ต่ำ",
        Effort::Medium => "กลาง",
        Effort::High => "สูง",
        Effort::Xhigh => "สูงมาก",
        Effort::Max => "สูงสุด",
    }
}

impl RunChoices {
    pub(super) fn set(&mut self, models: Option<ModelList>, permissions: Option<PermissionList>) {
        self.models = models;
        self.permissions = permissions;
    }

    fn pick(&self, thread: Option<&str>) -> &Pick {
        thread
            .and_then(|t| self.picks.get(t))
            .unwrap_or(&self.new_thread)
    }

    pub(super) fn pick_mut(&mut self, thread: Option<&str>) -> &mut Pick {
        match thread {
            Some(t) => self.picks.entry(t.to_owned()).or_default(),
            None => &mut self.new_thread,
        }
    }

    /// The new thread the composer started now has an id: its pick goes with it.
    pub(super) fn started(&mut self, thread: &str) {
        let pick = std::mem::take(&mut self.new_thread);
        self.picks.insert(thread.to_owned(), pick);
    }

    /// A fork of `from` starts on `from`'s pick.
    pub(super) fn forked(&mut self, from: &str, thread: &str) {
        if let Some(pick) = self.picks.get(from).cloned() {
            self.picks.insert(thread.to_owned(), pick);
        }
    }

    /// What a run of `thread` sends: the picks, else the writer's model so a thread stays on
    /// the model it shows; the effort only when that model takes it. A plan run's go-ahead is
    /// the next run, so plan gives way to workspace once it is sent.
    pub(super) fn for_run(
        &mut self,
        thread: Option<&str>,
        writer: Option<&str>,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let model = self
            .pick(thread)
            .model
            .clone()
            .or(writer.map(str::to_owned));
        let effort = self
            .pick(thread)
            .effort
            .filter(|e| self.effort_levels(thread, writer).contains(e));
        let pick = self.pick_mut(thread);
        let permission = pick.permission;
        if permission == Some(PermissionSetting::Plan) {
            pick.permission = Some(PermissionSetting::Workspace);
        }
        (
            model,
            permission.map(|p| p.to_string()),
            effort.map(|e| e.to_string()),
        )
    }

    /// The model a run of `thread` takes: the pick, else the writer's, else cedia's default.
    fn model_name<'a>(&'a self, thread: Option<&str>, writer: Option<&'a str>) -> Option<&'a str> {
        self.pick(thread)
            .model
            .as_deref()
            .or(writer)
            .or(self.models.as_ref().map(|m| m.default.as_str()))
    }

    /// The effort levels the thread's model takes; none for a model cedia does not list.
    fn effort_levels(&self, thread: Option<&str>, writer: Option<&str>) -> &[Effort] {
        self.model_name(thread, writer)
            .and_then(|name| self.entry(name))
            .map_or(&[], |m| m.effort_levels.as_slice())
    }

    /// The effort shown: the pick while the model takes it, else the model's own.
    fn effort(&self, thread: Option<&str>, writer: Option<&str>) -> Option<Effort> {
        let levels = self.effort_levels(thread, writer);
        self.pick(thread)
            .effort
            .filter(|e| levels.contains(e))
            .or_else(|| {
                self.model_name(thread, writer)
                    .and_then(|name| self.entry(name))
                    .and_then(|m| m.default_effort)
            })
    }

    /// The effort's word after the model's name; none for a model that takes no effort.
    fn effort_label(&self, thread: Option<&str>, writer: Option<&str>) -> Option<SharedString> {
        if self.effort_levels(thread, writer).is_empty() {
            return None;
        }
        Some(
            self.effort(thread, writer)
                .map_or("อัตโนมัติ", effort_word)
                .into(),
        )
    }

    /// The context window of the listed model a ref or alias names.
    pub(super) fn context_window(&self, name: &str) -> Option<u64> {
        self.entry(name).map(|m| m.context_window as u64)
    }

    /// The listed model a ref or alias names.
    fn entry(&self, name: &str) -> Option<&ModelEntry> {
        self.models
            .as_ref()?
            .models
            .iter()
            .find(|m| m.ref_ == name || m.alias.as_deref() == Some(name))
    }

    /// The model chip's word: the pick, else the writer's, else cedia's default.
    fn model_label(&self, thread: Option<&str>, writer: Option<&str>) -> Option<SharedString> {
        let name = self.model_name(thread, writer)?;
        Some(
            self.entry(name)
                .map_or_else(|| name.to_owned(), |m| m.label.clone())
                .into(),
        )
    }

    fn permission(&self, thread: Option<&str>) -> Option<PermissionSetting> {
        self.pick(thread)
            .permission
            .or(self.permissions.as_ref().map(|p| p.default))
    }

    fn permission_label(&self, thread: Option<&str>) -> Option<SharedString> {
        let mode = self.permission(thread)?;
        let listed = self
            .permissions
            .as_ref()?
            .modes
            .iter()
            .find(|m| m.mode == mode);
        Some(
            listed
                .map_or_else(|| mode.to_string(), |m| m.label.clone())
                .into(),
        )
    }
}

impl RootView {
    /// The open thread's writer model, from its transcript.
    pub(super) fn writer_model(&self, cx: &gpui::App) -> Option<String> {
        self.project().current.as_ref()?;
        self.agent.read(cx).transcript().model().map(str::to_owned)
    }

    /// The model and permission chips; nothing for what cedia did not list.
    pub(super) fn render_run_choices(&self, cx: &mut Context<Self>) -> AnyElement {
        let thread = self.project().current.clone();
        let thread = thread.as_deref();
        let writer = self.writer_model(cx);
        let choices = &self.choices;
        let model = choices
            .models
            .is_some()
            .then(|| choices.model_label(thread, writer.as_deref()))
            .flatten()
            .map(|label| {
                let effort = choices.effort_label(thread, writer.as_deref());
                self.choice_chip("run-model", Menu::Model, label, effort, cx)
            });
        let permission = choices
            .permission_label(thread)
            .map(|label| self.choice_chip("run-permission", Menu::Permission, label, None, cx));
        // The chips shrink before anything else in the row is clipped.
        // A menu opens from the group's right edge, so a wide one stays over the block.
        let menu = choices.menu.map(|menu| self.choice_menu(menu, cx));
        div()
            .relative()
            .min_w_0()
            .flex_shrink(1.)
            .flex()
            .items_center()
            .gap_1p5()
            .children(model)
            .children(permission)
            .children(menu)
            .into_any_element()
    }

    fn choice_chip(
        &self,
        id: &'static str,
        menu: Menu,
        label: SharedString,
        after: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let colors = cx.theme().colors();
        let open = self.choices.menu == Some(menu);
        // A mode's label is a word or two and keeps its width; a model's name shrinks.
        div()
            .relative()
            .max_w(px(200.))
            .map(|chip| match menu {
                Menu::Model => chip.min_w(px(72.)),
                Menu::Permission => chip.flex_none(),
            })
            .child(
                div()
                    .id(id)
                    .debug_selector(|| id.into())
                    .h(px(24.))
                    .max_w_full()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(colors.border)
                    .text_meta()
                    .text_color(colors.text_muted)
                    .cursor_pointer()
                    .when(open, |b| b.bg(colors.element_selected))
                    .hover(|b| b.text_color(colors.text))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        let choices = &mut view.choices;
                        let just_closed = choices.closed.take().is_some_and(|(closed, at)| {
                            closed == menu && at.elapsed() < Duration::from_millis(500)
                        });
                        choices.menu = (choices.menu.is_none() && !just_closed).then_some(menu);
                        cx.notify();
                    }))
                    .child(crate::tooltip::cut("label", label))
                    // The effort keeps its word while the name is cut.
                    .children(after.map(|word| {
                        div()
                            .debug_selector(|| "run-effort".into())
                            .flex_none()
                            .child(format!("· {word}"))
                    }))
                    .child(
                        svg()
                            .path("icons/chevron_down.svg")
                            .size(px(10.))
                            .flex_none()
                            .text_color(colors.text_muted),
                    ),
            )
    }

    fn choice_menu(&self, menu: Menu, cx: &mut Context<Self>) -> AnyElement {
        let thread = self.project().current.clone();
        let writer = self.writer_model(cx);
        let choices = &self.choices;
        let mut rows = Vec::new();
        match menu {
            Menu::Model => {
                let shown = choices
                    .pick(thread.as_deref())
                    .model
                    .clone()
                    .or(writer.clone())
                    .or(choices.models.as_ref().map(|m| m.default.clone()));
                // Under a label per provider, in cedia's order.
                let models = choices.models.iter().flat_map(|m| &m.models);
                let mut provider = None;
                for entry in models {
                    if provider != Some(&entry.provider) {
                        provider = Some(&entry.provider);
                        if rows.is_empty() {
                            rows.push(Row::Label(entry.provider.clone().into()));
                        } else {
                            rows.extend([
                                Row::Separator,
                                Row::Label(entry.provider.clone().into()),
                            ]);
                        }
                    }
                    let chosen = shown
                        .as_deref()
                        .is_some_and(|s| s == entry.ref_ || entry.alias.as_deref() == Some(s));
                    let model = entry.ref_.clone();
                    let thread = thread.clone();
                    rows.push(Row::Choice {
                        chosen,
                        label: entry.label.clone().into(),
                        note: None,
                        detail: (!entry.available).then(|| {
                            entry
                                .reason
                                .clone()
                                .unwrap_or_else(|| "ใช้ไม่ได้".into())
                                .into()
                        }),
                        on_click: entry.available.then(|| {
                            let view = cx.entity().downgrade();
                            Box::new(
                                move |_: &gpui::ClickEvent,
                                      _: &mut gpui::Window,
                                      cx: &mut gpui::App| {
                                    let model = model.clone();
                                    let thread = thread.clone();
                                    if let Err(error) = view.update(cx, |view, cx| {
                                        view.choices.pick_mut(thread.as_deref()).model =
                                            Some(model);
                                        view.choices.menu = None;
                                        cx.notify();
                                    }) {
                                        log::warn!("model pick: {error}");
                                    }
                                },
                            ) as OnClick
                        }),
                    });
                }
                // A model that takes an effort ends its menu with the levels.
                let writer = writer.as_deref();
                let levels = choices.effort_levels(thread.as_deref(), writer);
                if !levels.is_empty() {
                    rows.extend([Row::Separator, Row::Label("การคิด".into())]);
                }
                let shown = choices.effort(thread.as_deref(), writer);
                let own = choices
                    .model_name(thread.as_deref(), writer)
                    .and_then(|name| choices.entry(name))
                    .and_then(|m| m.default_effort);
                for &level in levels {
                    let thread = thread.clone();
                    let view = cx.entity().downgrade();
                    rows.push(Row::Choice {
                        chosen: shown == Some(level),
                        label: effort_word(level).into(),
                        note: (own == Some(level)).then(|| "ค่าเริ่มต้น".into()),
                        detail: None,
                        on_click: Some(Box::new(move |_, _, cx| {
                            let thread = thread.clone();
                            if let Err(error) = view.update(cx, |view, cx| {
                                view.choices.pick_mut(thread.as_deref()).effort = Some(level);
                                view.choices.menu = None;
                                cx.notify();
                            }) {
                                log::warn!("effort pick: {error}");
                            }
                        })),
                    });
                }
            }
            Menu::Permission => {
                let shown = choices.permission(thread.as_deref());
                let modes = choices.permissions.iter().flat_map(|p| &p.modes);
                for entry in modes {
                    let mode = entry.mode;
                    let thread = thread.clone();
                    let view = cx.entity().downgrade();
                    rows.push(Row::Choice {
                        chosen: shown.as_ref() == Some(&entry.mode),
                        label: entry.label.clone().into(),
                        note: None,
                        detail: Some(entry.description.clone().into()),
                        on_click: Some(Box::new(move |_, _, cx| {
                            let mode = mode;
                            let thread = thread.clone();
                            if let Err(error) = view.update(cx, |view, cx| {
                                view.choices.pick_mut(thread.as_deref()).permission = Some(mode);
                                view.choices.menu = None;
                                cx.notify();
                            }) {
                                log::warn!("permission pick: {error}");
                            }
                        })),
                    });
                }
            }
        }
        // From the chips' right edge leftwards: they sit left of the block's middle.
        deferred(
            div().absolute().bottom(px(28.)).right_0().child(
                popup_menu::menu("run-choice-menu", rows, cx)
                    .debug_selector(|| "run-choice-menu".into())
                    .w(px(MENU_WIDTH))
                    .on_mouse_down_out(cx.listener(|view, _, _, cx| {
                        view.choices.closed =
                            view.choices.menu.take().map(|menu| (menu, Instant::now()));
                        cx.notify();
                    })),
            ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices() -> RunChoices {
        let models: ModelList = serde_json::from_str(
            r#"{"default":"faux/a","models":[{"ref":"faux/a","alias":"a","label":"Faux A","provider":"faux","available":true,"contextWindow":1,"effort":false},{"ref":"x/think","label":"Think","provider":"x","available":true,"contextWindow":1,"effort":true,"effortLevels":["low","medium","high"],"defaultEffort":"medium"}]}"#,
        )
        .expect("models");
        let permissions: PermissionList = serde_json::from_str(
            r#"{"default":"read-only","modes":[{"mode":"read-only","label":"Read only","description":"Reads"},{"mode":"plan","label":"Plan","description":"Plans"}]}"#,
        )
        .expect("permissions");
        let mut choices = RunChoices::default();
        choices.set(Some(models), Some(permissions));
        choices
    }

    #[test]
    fn a_thread_with_no_pick_shows_its_writer_then_the_default() {
        let choices = choices();
        assert_eq!(
            choices.model_label(Some("t-1"), Some("a")).as_deref(),
            Some("Faux A")
        );
        assert_eq!(choices.model_label(None, None).as_deref(), Some("Faux A"));
        assert_eq!(
            choices.model_label(Some("t-1"), Some("other/x")).as_deref(),
            Some("other/x")
        );
        assert_eq!(choices.permission_label(None).as_deref(), Some("Read only"));
    }

    #[test]
    fn a_run_sends_the_pick_or_the_writer_and_plan_gives_way_to_workspace() {
        let mut choices = choices();
        assert_eq!(
            choices.for_run(Some("t-1"), Some("a")),
            (Some("a".into()), None, None)
        );
        choices.pick_mut(None).permission = Some(PermissionSetting::Plan);
        choices.pick_mut(None).model = Some("faux/a".into());
        assert_eq!(
            choices.for_run(None, None),
            (Some("faux/a".into()), Some("plan".into()), None)
        );
        choices.started("t-2");
        assert_eq!(
            choices.for_run(Some("t-2"), None),
            (Some("faux/a".into()), Some("workspace".into()), None)
        );
        assert!(choices.new_thread.model.is_none());
    }

    #[test]
    fn an_effort_shows_and_goes_only_with_a_model_that_takes_it() {
        let mut choices = choices();
        // The faux model takes none: no chip, and a pick is not sent.
        assert_eq!(choices.effort_label(Some("t-1"), Some("a")), None);
        choices.pick_mut(Some("t-1")).effort = Some(Effort::High);
        assert_eq!(choices.for_run(Some("t-1"), Some("a")).2, None);
        // A model that takes it shows the pick, else its own.
        assert_eq!(
            choices
                .effort_label(Some("t-2"), Some("x/think"))
                .as_deref(),
            Some("กลาง")
        );
        assert_eq!(
            choices
                .effort_label(Some("t-1"), Some("x/think"))
                .as_deref(),
            Some("สูง")
        );
        assert_eq!(
            choices.for_run(Some("t-1"), Some("x/think")).2.as_deref(),
            Some("high")
        );
        // A level the model does not list falls back to its own and is not sent.
        choices.pick_mut(Some("t-1")).effort = Some(Effort::Max);
        assert_eq!(
            choices
                .effort_label(Some("t-1"), Some("x/think"))
                .as_deref(),
            Some("กลาง")
        );
        assert_eq!(choices.for_run(Some("t-1"), Some("x/think")).2, None);
    }
}
