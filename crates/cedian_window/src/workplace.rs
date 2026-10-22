//! Where the open thread runs, at the left of the composer's bottom row (Interface, Polish):
//! its worktree branch (`main checkout` when it runs in the main worktree) and เปิดใน. Until
//! phase C's polish this was the toolbar row, with a breadcrumb and Thread ใหม่.

use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, deferred, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use std::path::PathBuf;

use crate::type_scale::TypeScale as _;
use crate::{open_in::OpenIn, tokens};

pub struct Workplace {
    /// The open thread's worktree branch, when it has one.
    branch: Option<String>,
    /// The directory เปิดใน opens: the open thread's worktree, else the project.
    dir: PathBuf,
    apps: Vec<OpenIn>,
    menu_open: bool,
}

impl Workplace {
    pub fn new() -> Self {
        Self {
            branch: None,
            dir: PathBuf::new(),
            apps: crate::open_in::available(),
            menu_open: false,
        }
    }

    pub fn set(&mut self, branch: Option<String>, dir: PathBuf, cx: &mut Context<Self>) {
        if branch != self.branch || dir != self.dir {
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
            .bottom(px(28.))
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
                    .on_click(cx.listener(move |workplace, _, _, cx| {
                        workplace.menu_open = false;
                        if let Some(app) = workplace.apps.get(n) {
                            app.open(&workplace.dir, cx);
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
                    .cursor_pointer()
                    .hover(|b| b.text_color(colors.text))
                    .on_click(cx.listener(|workplace, _, _, cx| {
                        workplace.menu_open = !workplace.menu_open;
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
                    deferred(menu.on_mouse_down_out(cx.listener(|workplace, _, _, cx| {
                        workplace.menu_open = false;
                        cx.notify();
                    })))
                    .with_priority(1),
                )
            })
    }
}

impl Render for Workplace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open_in = self.open_in(cx);
        let colors = cx.theme().colors();
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
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
            .child(open_in)
    }
}
