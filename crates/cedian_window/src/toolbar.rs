//! The toolbar under the project tabs (Interface, Layout: toolbar, 44 px): a breadcrumb of the
//! project folder and the open thread, the thread's worktree branch, เปิดใน, and Thread ใหม่.
//! Search moved to the top of the sidebar in phase C (`thread_search`); Thread ใหม่ stays
//! until the project chat can open threads.

use gpui::{
    Context, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, deferred, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use std::path::PathBuf;

use crate::type_scale::TypeScale as _;
use crate::{open_in::OpenIn, tokens};

pub const TOOLBAR_HEIGHT: f32 = 44.;

pub enum ToolbarEvent {
    NewThread,
}

pub struct Toolbar {
    project: SharedString,
    /// The open thread's title, the breadcrumb's second part.
    thread: SharedString,
    /// The open thread's worktree branch, when it has one.
    branch: Option<String>,
    /// The directory เปิดใน opens: the open thread's worktree, else the project.
    dir: PathBuf,
    apps: Vec<OpenIn>,
    menu_open: bool,
}

impl EventEmitter<ToolbarEvent> for Toolbar {}

impl Toolbar {
    pub fn new(project: SharedString) -> Self {
        Self {
            project,
            thread: SharedString::default(),
            branch: None,
            dir: PathBuf::new(),
            apps: crate::open_in::available(),
            menu_open: false,
        }
    }

    pub fn set(
        &mut self,
        project: SharedString,
        thread: SharedString,
        branch: Option<String>,
        dir: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if project != self.project
            || thread != self.thread
            || branch != self.branch
            || dir != self.dir
        {
            self.project = project;
            self.thread = thread;
            self.branch = branch;
            self.dir = dir;
            cx.notify();
        }
    }

    /// The เปิดใน button and, while it is open, its menu of apps.
    fn open_in(&self, cx: &mut Context<Self>) -> gpui::Div {
        let colors = cx.theme().colors();
        let mut menu = div()
            .absolute()
            .top(px(28.))
            .left_0()
            .min_w(px(180.))
            .flex()
            .flex_col()
            .p_1()
            .rounded(px(8.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .text_ui()
            .text_color(colors.text);
        for (n, app) in self.apps.iter().enumerate() {
            menu = menu.child(
                div()
                    .id(("open-in", n))
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |toolbar, _, _, cx| {
                        toolbar.menu_open = false;
                        if let Some(app) = toolbar.apps.get(n) {
                            app.open(&toolbar.dir, cx);
                        }
                        cx.notify();
                    }))
                    .child(app.label),
            );
        }
        div()
            .relative()
            .child(
                div()
                    .id("open-in")
                    .h(px(22.))
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
                    .hover(|b| b.text_color(colors.text))
                    .on_click(cx.listener(|toolbar, _, _, cx| {
                        toolbar.menu_open = !toolbar.menu_open;
                        cx.notify();
                    }))
                    .child("เปิดใน")
                    .child(
                        svg()
                            .path("icons/chevron_down.svg")
                            .size(px(10.))
                            .text_color(colors.text_muted),
                    ),
            )
            .when(self.menu_open, |b| {
                b.child(
                    deferred(menu.on_mouse_down_out(cx.listener(|toolbar, _, _, cx| {
                        toolbar.menu_open = false;
                        cx.notify();
                    })))
                    .with_priority(1),
                )
            })
    }
}

impl Render for Toolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open_in = self.open_in(cx);
        let colors = cx.theme().colors();
        div()
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        svg()
                            .path("icons/folder.svg")
                            .size(px(14.))
                            .text_color(colors.text_muted),
                    )
                    .child(
                        div()
                            .text_ui()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.project.clone()),
                    )
                    .child(div().text_ui().text_color(colors.text_muted).child("›"))
                    .child(
                        div()
                            .max_w(px(360.))
                            .truncate()
                            .text_ui()
                            .child(self.thread.clone()),
                    )
                    .child(
                        div()
                            .h(px(22.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_1()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(colors.border)
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(
                                svg()
                                    .path("icons/git_branch.svg")
                                    .size(px(12.))
                                    .text_color(colors.text_muted),
                            )
                            .child(
                                div().font(tokens::code_font()).child(
                                    self.branch
                                        .clone()
                                        .unwrap_or_else(|| "main checkout".into()),
                                ),
                            ),
                    )
                    .child(open_in),
            )
            .child(div().flex_grow(1.))
            .child(
                div()
                    .id("new-thread")
                    .flex_none()
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .rounded(px(7.))
                    .bg(colors.text_accent)
                    .text_color(colors.background)
                    .text_ui()
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ToolbarEvent::NewThread)))
                    .child(
                        svg()
                            .path("icons/plus.svg")
                            .size(px(12.))
                            .text_color(colors.background),
                    )
                    .child("Thread ใหม่"),
            )
    }
}
