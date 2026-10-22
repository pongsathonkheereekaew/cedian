//! The window's blocks (Interface, Blocks): the current layout's tree drawn as sheets with
//! their headers and dividers, what each kind of block shows, and what the person does to them
//! with the keys, the mouse and the menus. Every change goes through the project's
//! `Workspace`, which the daemon keeps in `desktop.layout`.

use cedian_layout::{
    BlockId, Dir, Kind, Node, Rect, Side, Workspace, fit, leaves, min_size, neighbour,
};
use gpui::{
    AnyElement, AppContext as _, Bounds, Context, DragMoveEvent, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, deferred, div,
    prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use super::{Connection, RootView};
use crate::{
    block_drag::{self, BlockDrag, SplitDrag},
    block_header::{self, HeaderActions, HeaderSpec, Tab},
    block_keys,
    on_click::OnClick,
    overview::{self, OverviewActions},
    popup_menu::{self, Row},
    project::{self, Project},
    tab_strip::STRIP_HEIGHT,
    thread_details::{self, Details},
    tool_menu,
    type_scale::TypeScale as _,
};

/// Blocks are 12 px sheets 6 px apart on `app` (Interface, Blocks).
pub(super) const BLOCK_RADIUS: f32 = 12.;
pub(super) const BLOCK_GAP: f32 = 6.;
/// Under this width a block compacts itself.
const NARROW: f32 = 360.;
const NARROW_LIST: f32 = 220.;
/// Under this width the composer's context and view chips give way to the run's model and
/// mode, so nothing in the row is clipped.
const CHIPS_FROM: f32 = 760.;
/// Under this width the thread list's foot leaves out cedia's version.
pub(super) const FOOT_FULL: f32 = 300.;
/// The most layout changes `⌘Z` goes back through.
const UNDO_DEPTH: usize = 50;
/// The floating block's card.
const PIP_WIDTH: f32 = 360.;
const PIP_HEIGHT: f32 = 260.;

/// What a + Tool pick or a split puts where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ToolTarget {
    pub against: Option<(BlockId, Side)>,
}

impl RootView {
    pub(super) fn ws(&self) -> &Workspace {
        &self.project().workspace
    }

    /// The area the tree fills: the window under the tab strip, less the outer gap.
    pub(super) fn area(&self) -> Rect {
        self.area
    }

    fn measure(&mut self, window: &Window) {
        let size = window.viewport_size();
        let w = f32::from(size.width) - 2. * BLOCK_GAP;
        let h = f32::from(size.height) - f32::from(STRIP_HEIGHT) - 2. * BLOCK_GAP;
        self.area = Rect::new(0., 0., w.max(1.), h.max(1.));
    }

    /// Applies a change to the front project's blocks, keeping the old ones for `⌘Z`, and
    /// saves the layouts.
    pub(super) fn change(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Workspace, Rect)) {
        let area = self.area();
        let before = self.ws().clone();
        let project = self.project_mut();
        f(&mut project.workspace, area);
        if project.workspace != before {
            project.undo.push(before);
            if project.undo.len() > UNDO_DEPTH {
                project.undo.remove(0);
            }
            self.remember_blocks(cx);
        }
        self.after_change(cx);
    }

    fn remember_blocks(&mut self, cx: &mut Context<Self>) {
        let cwd = self.project().cwd.display().to_string();
        let ws = self.ws().clone();
        self.layout.blocks.insert(cwd, ws);
        self.save_layout(cx);
    }

    /// Keeps focus and zoom on blocks that are still shown.
    fn after_change(&mut self, cx: &mut Context<Self>) {
        let shown = self.shown();
        if self.zoomed.is_some_and(|z| !shown.contains(&z)) {
            self.zoomed = None;
        }
        if self.focused.is_none_or(|f| !shown.contains(&f)) {
            self.refocus = true;
            self.focused = self
                .ws()
                .find(Kind::Agent)
                .filter(|a| shown.contains(a))
                .or(shown.first().copied());
        }
        self.sync_review_width(cx);
        self.open_chat_thread(cx);
        // A Review block that comes into view reads the hunks again.
        if self.showing(Kind::Review) {
            self.refresh_review();
        }
        cx.notify();
    }

    /// The current layout's tree as it is drawn in the area: fitted to the blocks' minimums,
    /// the focused block shown in its leaf.
    pub(super) fn drawn(&self) -> Option<Node> {
        let ws = self.ws();
        let area = self.area();
        ws.tree().map(|tree| {
            fit(
                tree,
                area.w,
                area.h,
                &|id| ws.kind(id),
                BLOCK_GAP,
                self.focused,
            )
        })
    }

    /// The blocks drawn now: each leaf's shown tab, and the floating one.
    pub(super) fn shown(&self) -> Vec<BlockId> {
        let ws = self.ws();
        self.drawn()
            .as_ref()
            .map(Node::shown)
            .unwrap_or_default()
            .into_iter()
            .chain(ws.floating)
            .collect()
    }

    /// Whether a block of `kind` is drawn now.
    pub(super) fn showing(&self, kind: Kind) -> bool {
        self.shown()
            .iter()
            .any(|id| self.ws().kind(*id) == Some(kind))
    }

    pub(super) fn focus_block(&mut self, id: BlockId, cx: &mut Context<Self>) {
        if self.focused != Some(id) {
            self.focused = Some(id);
            self.refocus = true;
            cx.notify();
        }
    }

    /// Gives the keys to the focused block's input, or to the window when it has none or it
    /// is not drawn.
    pub(super) fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let kind = self
            .focused
            .filter(|id| self.shown().contains(id) && self.zoomed.is_none_or(|z| z == *id))
            .and_then(|id| self.ws().kind(id));
        match kind {
            Some(Kind::Agent) => self.composer.update(cx, |c, cx| c.focus(window, cx)),
            Some(Kind::Chat) => self.chat_composer.update(cx, |c, cx| c.focus(window, cx)),
            Some(Kind::Review) => window.focus(&self.review.focus_handle(cx), cx),
            Some(Kind::Threads) | Some(Kind::Terminal) | Some(Kind::Editor) => {
                // A click puts the keys in the list's search, the shell or the editor.
                if !self.root_focus.contains_focused(window, cx) {
                    window.focus(&self.root_focus, cx);
                }
            }
            _ => window.focus(&self.root_focus, cx),
        }
    }

    /// Opens `kind` (or focuses the block of it the layout has), as + Tool and the keys do.
    /// The window draws one block per kind, so a kind already in the layout is shown there.
    pub(super) fn open_kind(&mut self, kind: Kind, target: ToolTarget, cx: &mut Context<Self>) {
        let existing = self.ws().find(kind).or_else(|| {
            let ws = self.ws();
            ws.floating
                .into_iter()
                .chain(ws.parked.iter().copied())
                .find(|id| ws.kind(*id) == Some(kind))
        });
        let mut opened = None;
        self.change(cx, |ws, area| {
            let id = match existing {
                Some(id) if ws.parked.contains(&id) => {
                    ws.unpark(id, target.against, area);
                    id
                }
                Some(id) => {
                    if let Some(tree) = ws.layouts.get_mut(ws.current).and_then(|l| l.tree.as_mut())
                    {
                        tree.activate(id);
                    }
                    id
                }
                None if kind == Kind::Threads && ws.hidden.is_some() => {
                    let id = ws.hidden.as_ref().map(|h| h.block);
                    ws.show_threads();
                    id.unwrap_or_default()
                }
                None => {
                    let id = ws.new_block(kind);
                    match target.against {
                        Some((other, side)) if ws.tree().is_some_and(|t| t.contains(other)) => {
                            if let Some(tree) =
                                ws.layouts.get_mut(ws.current).and_then(|l| l.tree.as_mut())
                            {
                                tree.insert(other, id, side);
                            }
                        }
                        _ => ws.place(id, area),
                    }
                    id
                }
            };
            opened = Some(id);
        });
        if let Some(id) = opened {
            self.focus_block(id, cx);
        }
        if kind == Kind::Terminal {
            cx.notify();
        }
    }

    pub(super) fn close_block(&mut self, id: BlockId, cx: &mut Context<Self>) {
        if self.ws().kind(id) == Some(Kind::Threads) {
            return self.toggle_threads(cx);
        }
        self.block_menu = None;
        self.change(cx, |ws, _| {
            ws.close(id);
        });
    }

    pub(super) fn park_block(&mut self, id: BlockId, cx: &mut Context<Self>) {
        self.block_menu = None;
        self.change(cx, |ws, _| ws.park(id));
    }

    pub(super) fn float_block(&mut self, id: BlockId, cx: &mut Context<Self>) {
        self.block_menu = None;
        self.change(cx, |ws, area| {
            if ws.floating == Some(id) {
                ws.land(area);
            } else {
                ws.float(id, area);
            }
        });
    }

    pub(super) fn toggle_zoom(&mut self, id: BlockId, cx: &mut Context<Self>) {
        self.block_menu = None;
        self.zoomed = if self.zoomed == Some(id) {
            None
        } else {
            Some(id)
        };
        self.focused = Some(id);
        self.refocus = true;
        self.sync_review_width(cx);
        cx.notify();
    }

    /// The review lays its files beside its hunks when its block is wide.
    fn sync_review_width(&mut self, cx: &mut Context<Self>) {
        let review = self.ws().find(Kind::Review);
        let width = review
            .and_then(|r| self.block_rect(r))
            .map_or(f32::MAX, |rect| rect.w);
        let wide = width >= 900.;
        let compact = width < 380.;
        self.review
            .update(cx, |pane, cx| pane.set_width(wide, compact, cx));
    }

    pub(super) fn block_rect(&self, id: BlockId) -> Option<Rect> {
        if self.zoomed == Some(id) {
            return Some(self.area());
        }
        let tree = self.drawn()?;
        leaves(&tree, self.area(), BLOCK_GAP)
            .into_iter()
            .find(|(b, _)| *b == id)
            .map(|(_, r)| r)
    }

    pub(super) fn toggle_threads(&mut self, cx: &mut Context<Self>) {
        let shown = self.ws().find(Kind::Threads).is_some();
        self.change(cx, |ws, area| {
            if shown {
                ws.hide_threads();
            } else if ws.hidden.is_some() {
                ws.show_threads();
            } else {
                ws.open(Kind::Threads, area, None);
            }
        });
    }

    pub(super) fn switch_layout(&mut self, n: usize, cx: &mut Context<Self>) {
        self.layout_menu = false;
        self.overview = false;
        self.zoomed = None;
        self.change(cx, |ws, _| {
            ws.switch(n);
        });
    }

    fn focus_toward(&mut self, side: Side, cx: &mut Context<Self>) {
        let (Some(from), Some(tree)) = (self.focused, self.drawn()) else {
            return;
        };
        if let Some(to) = neighbour(&tree, self.area(), from, side) {
            self.focus_block(to, cx);
        }
    }

    fn move_toward(&mut self, side: Side, cx: &mut Context<Self>) {
        let (Some(id), Some(tree)) = (self.focused, self.drawn()) else {
            return;
        };
        let Some(other) = neighbour(&tree, self.area(), id, side) else {
            return;
        };
        self.change(cx, |ws, _| {
            ws.move_block(id, other, side);
        });
    }

    fn grow_toward(&mut self, side: Side, cx: &mut Context<Self>) {
        let Some(id) = self.focused else {
            return;
        };
        self.change(cx, |ws, _| {
            if let Some(tree) = ws.layouts.get_mut(ws.current).and_then(|l| l.tree.as_mut()) {
                tree.grow(id, side, 0.1);
            }
        });
    }

    fn even_out(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |ws, _| {
            if let Some(tree) = ws.layouts.get_mut(ws.current).and_then(|l| l.tree.as_mut()) {
                tree.even();
            }
        });
    }

    fn undo_layout(&mut self, cx: &mut Context<Self>) {
        let Some(before) = self.project_mut().undo.pop() else {
            return;
        };
        self.project_mut().workspace = before;
        self.remember_blocks(cx);
        self.after_change(cx);
    }

    fn save_layout_as_new(&mut self, cx: &mut Context<Self>) {
        let name = format!("Layout {}", self.ws().layouts.len() + 1);
        self.change(cx, |ws, _| {
            let n = ws.save_as(&name);
            ws.switch(n);
        });
        self.overview = false;
    }

    /// Escape closes what floats over the blocks first: a menu, the overview, then zoom.
    fn escape(&mut self, cx: &mut Context<Self>) {
        if self.block_menu.take().is_some()
            || self.tool_menu.take().is_some()
            || std::mem::take(&mut self.layout_menu)
            || std::mem::take(&mut self.overview)
            || self.drop_target.take().is_some()
        {
            cx.notify();
            return;
        }
        if self.zoomed.take().is_some() {
            self.sync_review_width(cx);
            cx.notify();
        }
    }

    /// Opens the + Tool menu now, or after a moment when the pointer only passes over it.
    pub(super) fn show_tool_menu(&mut self, target: ToolTarget, cx: &mut Context<Self>) {
        self.tool_menu_closing = None;
        self.tool_menu = Some(target);
        cx.notify();
    }

    pub(super) fn hover_tool_menu(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.tool_menu_closing = None;
            if self.tool_menu.is_none() && self.tool_menu_opening.is_none() {
                self.tool_menu_opening = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(tool_menu::OPEN_AFTER).await;
                    this.update(cx, |view, cx| {
                        view.tool_menu_opening = None;
                        view.show_tool_menu(ToolTarget { against: None }, cx);
                    })
                    .ok();
                }));
            }
        } else {
            self.tool_menu_opening = None;
            if self.tool_menu.is_some() {
                self.tool_menu_closing = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(tool_menu::CLOSE_AFTER).await;
                    this.update(cx, |view, cx| {
                        view.tool_menu = None;
                        view.tool_menu_closing = None;
                        cx.notify();
                    })
                    .ok();
                }));
            }
        }
    }

    fn pick_tool(&mut self, kind: Kind, cx: &mut Context<Self>) {
        let target = self
            .tool_menu
            .take()
            .unwrap_or(ToolTarget { against: None });
        self.tool_menu_closing = None;
        self.open_kind(kind, target, cx);
    }

    /// A split from the menu or `⌘D`: the + Tool menu, to put the new block on that side.
    fn split(&mut self, side: Side, cx: &mut Context<Self>) {
        self.block_menu = None;
        let against = self.focused.map(|f| (f, side));
        self.show_tool_menu(ToolTarget { against }, cx);
    }

    /// The listeners for every block action, on the window's root.
    pub(super) fn block_actions(&self, root: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        use block_keys::*;
        root.on_action(cx.listener(|v, _: &FocusLeft, _, cx| v.focus_toward(Side::Left, cx)))
            .on_action(cx.listener(|v, _: &FocusRight, _, cx| v.focus_toward(Side::Right, cx)))
            .on_action(cx.listener(|v, _: &FocusUp, _, cx| v.focus_toward(Side::Up, cx)))
            .on_action(cx.listener(|v, _: &FocusDown, _, cx| v.focus_toward(Side::Down, cx)))
            .on_action(cx.listener(|v, _: &MoveLeft, _, cx| v.move_toward(Side::Left, cx)))
            .on_action(cx.listener(|v, _: &MoveRight, _, cx| v.move_toward(Side::Right, cx)))
            .on_action(cx.listener(|v, _: &MoveUp, _, cx| v.move_toward(Side::Up, cx)))
            .on_action(cx.listener(|v, _: &MoveDown, _, cx| v.move_toward(Side::Down, cx)))
            .on_action(cx.listener(|v, _: &GrowLeft, _, cx| v.grow_toward(Side::Left, cx)))
            .on_action(cx.listener(|v, _: &GrowRight, _, cx| v.grow_toward(Side::Right, cx)))
            .on_action(cx.listener(|v, _: &GrowUp, _, cx| v.grow_toward(Side::Up, cx)))
            .on_action(cx.listener(|v, _: &GrowDown, _, cx| v.grow_toward(Side::Down, cx)))
            .on_action(cx.listener(|v, _: &EvenOut, _, cx| v.even_out(cx)))
            .on_action(cx.listener(|v, _: &UndoLayout, _, cx| v.undo_layout(cx)))
            .on_action(cx.listener(|v, _: &Zoom, _, cx| {
                if let Some(id) = v.focused {
                    v.toggle_zoom(id, cx)
                }
            }))
            .on_action(cx.listener(|v, _: &Overview, _, cx| {
                v.overview = !v.overview;
                cx.notify();
            }))
            .on_action(cx.listener(|v, _: &Park, _, cx| {
                if let Some(id) = v.focused {
                    v.park_block(id, cx)
                }
            }))
            .on_action(cx.listener(|v, _: &SplitRight, _, cx| v.split(Side::Right, cx)))
            .on_action(cx.listener(|v, _: &SplitDown, _, cx| v.split(Side::Down, cx)))
            .on_action(cx.listener(|v, _: &Float, _, cx| {
                if let Some(id) = v.focused.or(v.ws().floating) {
                    v.float_block(id, cx)
                }
            }))
            .on_action(cx.listener(|v, _: &CloseBlock, _, cx| {
                if let Some(id) = v.focused {
                    v.close_block(id, cx)
                }
            }))
            .on_action(cx.listener(|v, _: &ToggleThreads, _, cx| v.toggle_threads(cx)))
            .on_action(cx.listener(|v, _: &OpenChat, _, cx| v.open_chat(cx)))
            .on_action(cx.listener(|v, _: &OpenAgent, _, cx| {
                v.open_kind(Kind::Agent, ToolTarget { against: None }, cx)
            }))
            .on_action(cx.listener(|v, _: &OpenTerminal, _, cx| {
                v.open_kind(Kind::Terminal, ToolTarget { against: None }, cx)
            }))
            .on_action(cx.listener(|v, _: &OpenReview, _, cx| {
                v.open_kind(Kind::Review, ToolTarget { against: None }, cx)
            }))
            .on_action(cx.listener(|v, _: &OpenEditor, _, cx| {
                v.open_kind(Kind::Editor, ToolTarget { against: None }, cx)
            }))
            .on_action(cx.listener(|_, _: &OpenBrowser, _, _| {}))
            .on_action(cx.listener(|v, _: &Escape, _, cx| v.escape(cx)))
            .on_action(cx.listener(|v, layout: &SwitchLayout, _, cx| {
                if layout.0 >= 1 {
                    v.switch_layout(layout.0 - 1, cx)
                }
            }))
    }

    /// The blocks: the zoomed one alone, or the tree, with the floating one over it.
    pub(super) fn render_blocks(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.measure(window);
        self.sync_review_width(cx);
        if self.showing(Kind::Terminal) {
            self.ensure_shell(window, cx);
        }
        let tree = self.drawn();
        // Dividers move the layout's own tree, whose paths a folded drawing does not share.
        self.folded = tree.as_ref() != self.ws().tree();
        let floating = self.ws().floating;
        let colors = cx.theme().colors().clone();
        let body = match (self.zoomed, tree) {
            (Some(id), _) => self.render_leaf(&[id], 0, &[], window, cx),
            (None, Some(tree)) => self.render_node(&tree, Vec::new(), window, cx),
            (None, None) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_ui()
                .text_color(colors.text_muted)
                .child("layout นี้ว่าง · เปิด block จาก + Tool")
                .into_any_element(),
        };
        div()
            .relative()
            .flex_grow(1.)
            .min_h_0()
            .w_full()
            .p(px(BLOCK_GAP))
            .flex()
            .overflow_hidden()
            .child(body)
            .when_some(floating.filter(|_| self.zoomed.is_none()), |area, id| {
                area.child(
                    div()
                        .absolute()
                        .right(px(18.))
                        .bottom(px(18.))
                        .w(px(PIP_WIDTH))
                        .h(px(PIP_HEIGHT))
                        .flex()
                        .shadow_lg()
                        .rounded(px(BLOCK_RADIUS))
                        .child(self.render_leaf(&[id], 0, &[], window, cx)),
                )
            })
            .into_any_element()
    }

    fn render_node(
        &mut self,
        node: &Node,
        path: Vec<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            Node::Leaf { tabs, active } => self.render_leaf(tabs, *active, &path, window, cx),
            Node::Split { dir, sizes, kids } => {
                let dir = *dir;
                let colors = cx.theme().colors().clone();
                let mut split = div()
                    .id(SharedString::from(format!("split-{path:?}")))
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .flex();
                if dir == Dir::Col {
                    split = split.flex_col();
                }
                for (n, (size, kid)) in sizes.iter().zip(kids).enumerate() {
                    if n > 0 {
                        let drag = SplitDrag {
                            path: path.clone(),
                            divider: n - 1,
                        };
                        let even = path.clone();
                        let divider = div()
                            .id(SharedString::from(format!("divider-{path:?}-{n}")))
                            .flex_none()
                            .map(|d| match dir {
                                Dir::Row => d.w(px(BLOCK_GAP)).h_full().cursor_col_resize(),
                                Dir::Col => d.h(px(BLOCK_GAP)).w_full().cursor_row_resize(),
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .group("divider")
                            .child(
                                div()
                                    .rounded(px(2.))
                                    .map(|g| match dir {
                                        Dir::Row => g.w(px(2.)).h(gpui::relative(0.4)),
                                        Dir::Col => g.h(px(2.)).w(gpui::relative(0.4)),
                                    })
                                    .group_hover("divider", |g| g.bg(colors.border_selected)),
                            )
                            .when(!self.folded, |d| {
                                d.on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                            })
                            .on_click(cx.listener(move |view, event: &gpui::ClickEvent, _, cx| {
                                if event.click_count() == 2 && !view.folded {
                                    let even = even.clone();
                                    view.change(cx, |ws, _| {
                                        if let Some(tree) = ws
                                            .layouts
                                            .get_mut(ws.current)
                                            .and_then(|l| l.tree.as_mut())
                                        {
                                            tree.even_at(&even);
                                        }
                                    });
                                }
                            }));
                        split = split.child(divider);
                    }
                    let mut kid_path = path.clone();
                    kid_path.push(n);
                    // The fit pass: a child never draws under its minimum, whatever its share.
                    let (min_w, min_h) = self.min_size_of(kid);
                    split = split.child(
                        div()
                            .flex_basis(px(0.))
                            .flex_grow(*size)
                            .flex_shrink(0.)
                            .min_w(px(min_w))
                            .min_h(px(min_h))
                            .flex()
                            .child(self.render_node(kid, kid_path, window, cx)),
                    );
                }
                let here = path.clone();
                split
                    .on_drag_move(cx.listener(
                        move |view, event: &DragMoveEvent<SplitDrag>, _, cx| {
                            let drag = event.drag(cx);
                            if drag.path != here {
                                return;
                            }
                            let divider = drag.divider;
                            let at = along(event.bounds, event.event.position, dir);
                            let length = f32::from(match dir {
                                Dir::Row => event.bounds.size.width,
                                Dir::Col => event.bounds.size.height,
                            })
                            .max(1.);
                            let path = here.clone();
                            view.resize_split(&path, divider, at, length, cx);
                        },
                    ))
                    .into_any_element()
            }
        }
    }

    /// A divider dragged to `at` of a split `length` px long; it stops where either block
    /// beside it would go under its minimum.
    fn resize_split(
        &mut self,
        path: &[usize],
        divider: usize,
        at: f32,
        length: f32,
        cx: &mut Context<Self>,
    ) {
        let min = match self.ws().tree().and_then(|t| at_path(t, path)) {
            Some(Node::Split { dir, kids, .. }) => {
                let along = |kid: Option<&Node>| {
                    kid.map_or(0., |k| {
                        let (w, h) = self.min_size_of(k);
                        (if *dir == Dir::Row { w } else { h }) + BLOCK_GAP
                    }) / length
                };
                (along(kids.get(divider)), along(kids.get(divider + 1)))
            }
            _ => return,
        };
        let project = self.project_mut();
        if let Some(tree) = project
            .workspace
            .layouts
            .get_mut(project.workspace.current)
            .and_then(|l| l.tree.as_mut())
        {
            tree.move_divider_within(path, divider, at, min);
        }
        self.remember_blocks(cx);
        cx.notify();
    }

    /// One leaf: its shown block as a sheet, its header carrying the leaf's tabs.
    fn render_leaf(
        &mut self,
        tabs: &[BlockId],
        active: usize,
        _path: &[usize],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(&id) = tabs.get(active).or(tabs.first()) else {
            return div().into_any_element();
        };
        let Some(kind) = self.ws().kind(id) else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors().clone();
        let focused = self.focused == Some(id);
        let width = self.block_rect(id).map_or(f32::MAX, |r| r.w);
        // The thread list keeps its rows' meta down to a narrower width than the rest.
        let narrow = width
            < if kind == Kind::Threads {
                NARROW_LIST
            } else {
                NARROW
            };
        let header_tabs = tabs
            .iter()
            .filter_map(|t| Some(self.tab_of(*t, self.ws().kind(*t)?, cx)))
            .collect();
        let spec = HeaderSpec {
            id,
            kind,
            tabs: header_tabs,
            active: id,
            meta: self.meta_of(kind, cx),
            focused,
            zoomed: self.zoomed == Some(id),
            narrow,
            // The icon and the buttons take about 120 px, a tab about 110.
            max_tabs: ((width - 120.) / 110.).floor().max(1.) as usize,
        };
        let actions = HeaderActions {
            tab: Box::new({
                let view = cx.entity().downgrade();
                move |tab| {
                    let view = view.clone();
                    Box::new(move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.change(cx, |ws, _| {
                                if let Some(tree) =
                                    ws.layouts.get_mut(ws.current).and_then(|l| l.tree.as_mut())
                                {
                                    tree.activate(tab);
                                }
                            });
                            view.focus_block(tab, cx);
                        })
                        .ok();
                    })
                }
            }),
            zoom: Box::new(cx.listener(move |view, _, _, cx| view.toggle_zoom(id, cx))),
            menu: Box::new(cx.listener(move |view, _, _, cx| {
                view.block_menu = if view.block_menu == Some(id) {
                    None
                } else {
                    Some(id)
                };
                view.focused = Some(id);
                cx.notify();
            })),
        };
        let label = self.tab_of(id, kind, cx).label;
        let header = block_header::header(spec, actions, cx).on_drag(
            BlockDrag {
                id,
                label,
                icon: block_header::icon(kind),
            },
            |drag, _, _, cx| cx.new(|_| drag.clone()),
        );
        let body = self.block_body(id, kind, narrow, window, cx);
        let drop = self
            .drop_target
            .filter(|(target, _)| *target == id)
            .map(|(_, side)| block_drag::drop_zone(side, kind.name(), cx));
        let menu = (self.block_menu == Some(id)).then(|| self.render_block_menu(id, kind, cx));
        div()
            .id(SharedString::from(format!("block-{id}")))
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .rounded(px(BLOCK_RADIUS))
            .border_1()
            .border_color(colors.border)
            .bg(colors.editor_background)
            .overflow_hidden()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |view, _, _, cx| {
                    // A click on a part of the block that takes no keys gives them back to
                    // its input, not to the window.
                    view.focused = Some(id);
                    view.refocus = true;
                    cx.notify();
                }),
            )
            .child(header)
            .child(div().flex_grow(1.).min_h_0().flex().flex_col().child(body))
            .when(focused, |b| {
                // A 2 px line along the focused block's top.
                b.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(2.))
                        .bg(colors.text_muted),
                )
            })
            .on_drag_move(
                cx.listener(move |view, event: &DragMoveEvent<BlockDrag>, _, cx| {
                    let dragged = event.drag(cx).id;
                    let over = event.bounds.contains(&event.event.position);
                    let target = (over && dragged != id)
                        .then(|| (id, block_drag::zone(event.bounds, event.event.position)));
                    if over && view.drop_target != target {
                        view.drop_target = target;
                        cx.notify();
                    }
                }),
            )
            .on_drop(cx.listener(move |view, drag: &BlockDrag, _, cx| {
                let side = view.drop_target.filter(|(t, _)| *t == id).map(|(_, s)| s);
                view.drop_target = None;
                if let Some(side) = side {
                    let moved = drag.id;
                    view.change(cx, |ws, area| {
                        if ws.floating == Some(moved) {
                            ws.floating = None;
                            if let Some(tree) =
                                ws.layouts.get_mut(ws.current).and_then(|l| l.tree.as_mut())
                            {
                                tree.insert(id, moved, side);
                            }
                        } else if ws.parked.contains(&moved) {
                            ws.unpark(moved, Some((id, side)), area);
                        } else {
                            ws.move_block(moved, id, side);
                        }
                    });
                    view.focus_block(moved, cx);
                }
                cx.notify();
            }))
            .children(drop)
            .children(menu)
            .into_any_element()
    }

    fn tab_of(&self, id: BlockId, kind: Kind, cx: &gpui::App) -> Tab {
        let project = self.project();
        match kind {
            Kind::Agent => {
                let header = self.agent.read(cx).header();
                Tab {
                    id,
                    label: header.title.clone(),
                    status: header.status,
                    calls: false,
                }
            }
            Kind::Chat => Tab {
                id,
                label: kind.name().into(),
                status: None,
                calls: project.chat_thread().is_some_and(|t| t.needs_you),
            },
            Kind::Editor => Tab {
                id,
                label: self
                    .editor_shows
                    .clone()
                    .unwrap_or_else(|| kind.name().into()),
                status: None,
                calls: false,
            },
            Kind::Terminal => Tab {
                id,
                label: project
                    .shell_dir()
                    .file_name()
                    .map(|n| SharedString::from(n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| kind.name().into()),
                status: None,
                calls: false,
            },
            _ => Tab {
                id,
                label: kind.name().into(),
                status: None,
                calls: false,
            },
        }
    }

    fn meta_of(&self, kind: Kind, cx: &Context<Self>) -> Option<SharedString> {
        match kind {
            Kind::Agent => self
                .agent
                .read(cx)
                .cost()
                .map(|cost| format!("${cost:.4}").into()),
            Kind::Chat => self
                .chat
                .read(cx)
                .cost()
                .map(|cost| format!("${cost:.4}").into()),
            Kind::Review => {
                (self.waiting_hunks > 0).then(|| format!("รอ {}", self.waiting_hunks).into())
            }
            Kind::Threads => {
                let working = self.project().working();
                (working > 0).then(|| format!("{working} กำลังทำ").into())
            }
            _ => None,
        }
    }

    fn render_block_menu(&self, id: BlockId, kind: Kind, cx: &mut Context<Self>) -> AnyElement {
        let key = |k: &str| Some(block_keys::shown(k));
        let mut rows = vec![
            Row::item(
                "แบ่งขวา",
                key(block_keys::SPLIT_RIGHT),
                Box::new(cx.listener(|view, _, _, cx| view.split(Side::Right, cx))),
            ),
            Row::item(
                "แบ่งล่าง",
                key(block_keys::SPLIT_DOWN),
                Box::new(cx.listener(|view, _, _, cx| view.split(Side::Down, cx))),
            ),
            Row::item(
                if self.ws().floating == Some(id) {
                    "กลับเข้า layout"
                } else {
                    "ลอยเป็น picture in picture"
                },
                key(block_keys::FLOAT),
                Box::new(cx.listener(move |view, _, _, cx| view.float_block(id, cx))),
            ),
            Row::item(
                if self.zoomed == Some(id) {
                    "ย่อกลับ"
                } else {
                    "ขยายเต็ม"
                },
                key(block_keys::ZOOM),
                Box::new(cx.listener(move |view, _, _, cx| view.toggle_zoom(id, cx))),
            ),
            Row::item(
                "พักไว้",
                key(block_keys::PARK),
                Box::new(cx.listener(move |view, _, _, cx| view.park_block(id, cx))),
            ),
        ];
        let others: Vec<(usize, String)> = self
            .ws()
            .layouts
            .iter()
            .enumerate()
            .filter(|(n, _)| *n != self.ws().current)
            .map(|(n, l)| (n, l.name.clone()))
            .collect();
        if !others.is_empty() {
            rows.push(Row::Separator);
            rows.push(Row::Label("ย้ายไป layout".into()));
            for (n, name) in others {
                rows.push(Row::item(
                    name,
                    None,
                    Box::new(cx.listener(move |view, _, _, cx| view.move_to_layout(id, n, cx))),
                ));
            }
        }
        rows.push(Row::Separator);
        rows.push(if kind.closable() {
            Row::item(
                "ปิด block",
                key(block_keys::CLOSE),
                Box::new(cx.listener(move |view, _, _, cx| view.close_block(id, cx))),
            )
        } else {
            Row::item(
                "ซ่อน",
                key(block_keys::THREADS),
                Box::new(cx.listener(|view, _, _, cx| {
                    view.block_menu = None;
                    view.toggle_threads(cx)
                })),
            )
        });
        // The menu opens on the side with room: from the block's left edge when the block
        // is near the window's left and narrower than the menu.
        let from_left = self.block_rect(id).is_some_and(|r| r.x + r.w < 260.);
        deferred(
            div()
                .absolute()
                .top(px(block_header::HEADER_HEIGHT))
                .map(|m| {
                    if from_left {
                        m.left(px(6.))
                    } else {
                        m.right(px(6.))
                    }
                })
                .child(
                    popup_menu::menu("block-menu", rows, cx).on_mouse_down_out(cx.listener(
                        |view, _, _, cx| {
                            view.block_menu = None;
                            cx.notify();
                        },
                    )),
                ),
        )
        .with_priority(2)
        .into_any_element()
    }

    /// Takes `id` out of this layout and puts it into layout `n` by the placing rule.
    fn move_to_layout(&mut self, id: BlockId, n: usize, cx: &mut Context<Self>) {
        self.block_menu = None;
        let Some(kind) = self.ws().kind(id) else {
            return;
        };
        self.change(cx, |ws, area| {
            if !kind.closable() {
                return;
            }
            ws.close(id);
            let from = ws.current;
            if ws.switch(n) {
                let new = ws.new_block(kind);
                ws.place(new, area);
                ws.switch(from);
            }
        });
    }

    /// What a block of `kind` shows.
    fn block_body(
        &mut self,
        id: BlockId,
        kind: Kind,
        narrow: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        match kind {
            Kind::Threads => self.render_threads(narrow, cx),
            Kind::Agent => {
                self.render_agent(self.block_rect(id).is_some_and(|r| r.w < CHIPS_FROM), cx)
            }
            Kind::Chat => self.render_chat(cx),
            Kind::Terminal => match self.project().shells.get(&self.project().shell_dir()) {
                Some(shell) => shell.clone().into_any_element(),
                None => div().into_any_element(),
            },
            Kind::Review => self.review.clone().into_any_element(),
            Kind::Editor => div()
                .key_context("ToolEditor")
                .on_action(cx.listener(Self::attach_editor_selection))
                .size_full()
                .p_2()
                .child(self.editor.clone())
                .into_any_element(),
            Kind::Files => self.files.clone().into_any_element(),
            Kind::Git => self.git.clone().into_any_element(),
            Kind::Session => {
                let details = Details::of(self.agent.read(cx).transcript());
                let info = self.project().info();
                div()
                    .id("details")
                    .size_full()
                    .overflow_y_scroll()
                    .child(thread_details::render(info, &details, cx))
                    .when(info.is_some(), |details| {
                        details.child(
                            div().px_3().pb_3().flex().child(
                                div()
                                    .id("export-thread")
                                    .h(px(26.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(colors.border)
                                    .cursor_pointer()
                                    .text_meta()
                                    .hover(|b| b.bg(colors.element_hover))
                                    .on_click(cx.listener(|view, _, _, cx| view.export_thread(cx)))
                                    .child("ส่งออกเป็น Markdown"),
                            ),
                        )
                    })
                    .into_any_element()
            }
            Kind::Browser | Kind::Outputs | Kind::Memory | Kind::Usage | Kind::Background => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_4()
                .text_ui()
                .text_color(colors.text_muted)
                .child(format!("{} มาใน phase C", kind.name()))
                .into_any_element(),
        }
    }

    /// The title bar's right side: the layout's name with its menu, and + Tool.
    pub(super) fn render_title_tools(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let ws = self.ws();
        let name = ws
            .layouts
            .get(ws.current)
            .map(|l| l.name.clone())
            .unwrap_or_default();
        let chip = |id: &'static str| {
            div()
                .id(id)
                .h(px(28.))
                .px_2p5()
                .flex()
                .items_center()
                .gap_1p5()
                .rounded(px(8.))
                .text_ui()
                .text_color(colors.text_muted)
                .cursor_pointer()
                .hover(|b| b.bg(colors.element_hover).text_color(colors.text))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let layout_menu = self.layout_menu.then(|| {
            let mut rows: Vec<Row> = ws
                .layouts
                .iter()
                .enumerate()
                .map(|(n, l)| {
                    Row::item(
                        l.name.clone(),
                        (n < 9).then(|| {
                            block_keys::shown(&format!("{}-{}", block_keys::LAYOUT, n + 1))
                        }),
                        Box::new(cx.listener(move |view, _, _, cx| view.switch_layout(n, cx))),
                    )
                })
                .collect();
            rows.push(Row::Separator);
            rows.push(Row::item(
                "ดูทุก layout",
                Some(block_keys::shown(block_keys::OVERVIEW)),
                Box::new(cx.listener(|view, _, _, cx| {
                    view.layout_menu = false;
                    view.overview = true;
                    cx.notify();
                })),
            ));
            rows.push(Row::item(
                "บันทึก layout นี้",
                None,
                Box::new(cx.listener(|view, _, _, cx| {
                    view.layout_menu = false;
                    view.save_layout_as_new(cx)
                })),
            ));
            deferred(div().absolute().top(px(32.)).right_0().child(
                popup_menu::menu("layout-menu", rows, cx).on_mouse_down_out(cx.listener(
                    |view, _, _, cx| {
                        view.layout_menu = false;
                        cx.notify();
                    },
                )),
            ))
            .with_priority(2)
        });
        let tool_menu =
            self.tool_menu.map(|_| {
                let rows = tool_menu::rows(ws, |kind| {
                    Box::new(cx.listener(move |view, _, _, cx| view.pick_tool(kind, cx)))
                });
                deferred(
                    div()
                        .id("tool-menu-area")
                        .absolute()
                        .top(px(32.))
                        .right_0()
                        .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
                            view.hover_tool_menu(*hovered, cx)
                        }))
                        .child(popup_menu::menu("tool-menu", rows, cx).on_mouse_down_out(
                            cx.listener(|view, _, _, cx| {
                                view.tool_menu = None;
                                cx.notify();
                            }),
                        )),
                )
                .with_priority(2)
            });
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .mb(px(4.))
            .child(
                div()
                    .relative()
                    .child(
                        chip("layout-chip")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.layout_menu = !view.layout_menu;
                                cx.notify();
                            }))
                            .child(
                                gpui::svg()
                                    .path("icons/blocks.svg")
                                    .size(px(14.))
                                    .text_color(colors.text_muted),
                            )
                            .child(name)
                            .child(
                                gpui::svg()
                                    .path("icons/chevron_down.svg")
                                    .size(px(10.))
                                    .text_color(colors.text_muted),
                            ),
                    )
                    .children(layout_menu),
            )
            .child(
                div()
                    .relative()
                    .child(
                        chip("tool-button")
                            .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
                                view.hover_tool_menu(*hovered, cx)
                            }))
                            .on_click(cx.listener(|view, _, _, cx| {
                                if view.tool_menu.is_some() {
                                    view.tool_menu = None;
                                    cx.notify();
                                } else {
                                    view.show_tool_menu(ToolTarget { against: None }, cx);
                                }
                            }))
                            .child(
                                gpui::svg()
                                    .path("icons/plus.svg")
                                    .size(px(14.))
                                    .text_color(colors.text_muted),
                            )
                            .when(!compact, |chip| chip.child("Tool")),
                    )
                    .children(tool_menu),
            )
            .into_any_element()
    }

    /// The button left of the project tabs that hides and shows Threads.
    pub(super) fn render_threads_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let shown = self.ws().find(Kind::Threads).is_some();
        div()
            .id("threads-button")
            .flex_none()
            .mb(px(4.))
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(7.))
            .cursor_pointer()
            .hover(|b| b.bg(colors.element_hover))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|view, _, _, cx| view.toggle_threads(cx)))
            .child(
                gpui::svg()
                    .path(if shown {
                        "icons/threads_sidebar_left_open.svg"
                    } else {
                        "icons/threads_sidebar_left_closed.svg"
                    })
                    .size(px(15.))
                    .text_color(colors.text_muted),
            )
            .into_any_element()
    }

    pub(super) fn render_overview(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.overview || matches!(self.connection, Connection::Replay) {
            return None;
        }
        Some(
            overview::overview(
                self.ws(),
                OverviewActions {
                    layout: Box::new({
                        let view = cx.entity().downgrade();
                        move |n| {
                            let view = view.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    view.update(cx, |view, cx| view.switch_layout(n, cx)).ok();
                                },
                            ) as OnClick
                        }
                    }),
                    unpark: Box::new({
                        let view = cx.entity().downgrade();
                        move |id| {
                            let view = view.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    view.update(cx, |view, cx| {
                                        view.overview = false;
                                        view.change(cx, |ws, area| ws.unpark(id, None, area));
                                        view.focus_block(id, cx);
                                    })
                                    .ok();
                                },
                            ) as OnClick
                        }
                    }),
                    save: Box::new(cx.listener(|view, _, _, cx| view.save_layout_as_new(cx))),
                    close: Box::new(cx.listener(|view, _, _, cx| {
                        view.overview = false;
                        cx.notify();
                    })),
                },
                cx,
            )
            .into_any_element(),
        )
    }

    /// The blocks of a project opened now, as `desktop.layout` kept them.
    pub(super) fn load_blocks(&self, project: &mut Project) {
        project.workspace = self.layout.workspace(&project.cwd.display().to_string());
        // A layout that lost its thread list gets it back on the left.
        let current = project.workspace.current;
        project.workspace.switch(current);
    }

    /// The project chat's block, opened or focused (`⌘0`).
    pub(super) fn open_chat(&mut self, cx: &mut Context<Self>) {
        self.open_kind(Kind::Chat, ToolTarget { against: None }, cx);
    }

    /// Makes sure a layout showing an agent block exists for an opened thread: one thread at a
    /// time, and every block follows it.
    pub(super) fn ensure_agent(&mut self, cx: &mut Context<Self>) {
        if self.ws().find(Kind::Agent).is_none() && !self.showing(Kind::Agent) {
            self.open_kind(Kind::Agent, ToolTarget { against: None }, cx);
        }
    }

    pub(super) fn chat_thread_id(&self) -> Option<String> {
        self.project().chat_thread().map(|t| t.thread.id.clone())
    }

    /// Reads the project chat's thread into the chat block while one is shown.
    pub(super) fn open_chat_thread(&mut self, cx: &mut Context<Self>) {
        // A chat just started may not be in the list yet.
        let want = self
            .showing(Kind::Chat)
            .then(|| self.chat_thread_id().or(self.project().chat_open.clone()))
            .flatten();
        if self.project().chat_open == want {
            return;
        }
        if let Some(old) = self.project_mut().chat_open.take()
            && Some(&old) != self.project().current.as_ref()
        {
            self.request(cedian_wire::Request::Close { thread: old });
        }
        self.chat
            .update(cx, |chat, cx| chat.reset("แชทโปรเจกต์", cx));
        if let Some(thread) = want.clone() {
            self.request(cedian_wire::Request::Open {
                cwd: self.project().cwd.clone(),
                thread,
            });
        }
        self.project_mut().chat_open = want;
    }
}

impl RootView {
    pub(super) fn min_size_of(&self, node: &Node) -> (f32, f32) {
        let ws = self.ws();
        min_size(node, &|id| ws.kind(id), BLOCK_GAP)
    }
}

fn at_path<'a>(node: &'a Node, path: &[usize]) -> Option<&'a Node> {
    match path.split_first() {
        None => Some(node),
        Some((first, rest)) => match node {
            Node::Split { kids, .. } => at_path(kids.get(*first)?, rest),
            Node::Leaf { .. } => None,
        },
    }
}

/// The pointer's place along a split, from 0 at its start to 1 at its end.
fn along(bounds: Bounds<Pixels>, at: gpui::Point<Pixels>, dir: Dir) -> f32 {
    let (start, length, pos) = match dir {
        Dir::Row => (bounds.left(), bounds.size.width, at.x),
        Dir::Col => (bounds.top(), bounds.size.height, at.y),
    };
    f32::from(pos - start) / f32::from(length).max(1.)
}

/// Whether the thread list shows `info` in its groups: the project chat has its own row.
pub(super) fn in_groups(info: &cedian_wire::ThreadInfo) -> bool {
    !project::is_chat(info)
}
