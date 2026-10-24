//! The Browser block (Interface, Phase C: Browser): the project's page in the shared Chromium the
//! daemon runs. The page is drawn from its screencast and the person's clicks, scrolls and keys
//! go back to it, so it works as a browser while cedia's browser tool drives the same page for
//! the project's runs. A row above it has back, reload, the address, and the mobile and desktop
//! widths, and จับภาพและวาด; a line under the row says when the agent is using the page. A
//! capture takes the page's place until it is attached to the composer or put away
//! (`browser_marks.rs`).

use std::{path::PathBuf, sync::Arc};

use cedian_wire::BrowserPage;
use editor::Editor;
use futures::StreamExt as _;
use gpui::{
    App, AppContext as _, Bounds, ClipboardItem, Context, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement as _, PathBuilder, Pixels, Point, Render, RenderImage,
    ScrollWheelEvent, StatefulInteractiveElement as _, Styled as _, Task, UTF16Selection, Window,
    actions, canvas, div, img, point, prelude::FluentBuilder as _, px, svg,
};
use serde_json::{Value, json};
use theme::ActiveTheme as _;

use crate::{
    browser_marks::{self, Marks, Spot},
    cdp::{Cdp, CdpEvent},
    tokens,
    type_scale::TypeScale as _,
};

actions!(browser_pane, [Go]);

/// The mobile width, an iPhone's in CSS pixels.
const MOBILE_WIDTH: f32 = 390.;
/// Pixels a line of a wheel that scrolls by lines moves.
const LINE: f32 = 40.;

pub enum BrowserEvent {
    /// The block wants the project's page: ask the daemon.
    Wanted,
    /// A marked capture to attach to the open thread's composer.
    Captured(PathBuf),
}

pub struct BrowserPane {
    focus: FocusHandle,
    address: Entity<Editor>,
    /// The project whose page this is, once asked.
    cwd: Option<PathBuf>,
    page: Option<BrowserPage>,
    error: Option<String>,
    cdp: Option<Cdp>,
    reader: Option<Task<()>>,
    frame: Option<Arc<RenderImage>>,
    /// The page's size in CSS pixels, as the last frame shows it.
    frame_size: (f32, f32),
    /// The block's header draws the toolbar (the block holds no other tab), so the page goes
    /// up to the header.
    pub bar_in_header: bool,
    /// Frames drawn and replaced, dropped from the sprite atlas at the next frame.
    spent: Vec<Arc<RenderImage>>,
    /// Where the page is drawn, and the viewport last set: width, height, scale, mobile.
    area: Bounds<Pixels>,
    viewport: Option<(u32, u32, u32, bool)>,
    mobile: bool,
    url: String,
    /// The URL the address field was last set to.
    shown_url: Option<String>,
    loading: bool,
    /// The open thread works and its last tool call is the browser.
    agent: bool,
    /// The id of the selection asked for by ⌘C.
    copying: Option<u64>,
    /// The id of the history asked for by back.
    going_back: Option<u64>,
    /// The id of the capture asked for by จับภาพและวาด, with the page's size then.
    capturing: Option<(u64, (f32, f32))>,
    /// The capture being marked, shown in the page's place.
    marks: Option<Marks>,
    /// Why the capture could not be taken or attached.
    marks_error: Option<String>,
}

impl EventEmitter<BrowserEvent> for BrowserPane {}

impl BrowserPane {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let address = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("พิมพ์ที่อยู่ เช่น localhost:3000", window, cx);
            editor
        });
        Self {
            focus: cx.focus_handle(),
            address,
            cwd: None,
            page: None,
            error: None,
            cdp: None,
            reader: None,
            frame: None,
            bar_in_header: false,
            frame_size: (0., 0.),
            spent: Vec::new(),
            area: Bounds::default(),
            viewport: None,
            mobile: false,
            url: String::new(),
            shown_url: None,
            loading: false,
            agent: false,
            copying: None,
            going_back: None,
            capturing: None,
            marks: None,
            marks_error: None,
        }
    }

    /// Asks for the project's page when the block shows another project's, or none.
    pub fn want(&mut self, cwd: &PathBuf, cx: &mut Context<Self>) {
        if self.cwd.as_ref() != Some(cwd) {
            self.cwd = Some(cwd.clone());
            self.page = None;
            self.disconnect();
            cx.emit(BrowserEvent::Wanted);
        }
    }

    /// Asks again at the next frame the block is drawn: a new daemon connection.
    pub fn forget(&mut self, cx: &mut Context<Self>) {
        self.cwd = None;
        self.page = None;
        self.disconnect();
        cx.notify();
    }

    /// The daemon's answer: connect to the page, or say why there is none.
    pub fn set_page(
        &mut self,
        cwd: PathBuf,
        page: Result<BrowserPage, String>,
        cx: &mut Context<Self>,
    ) {
        if self.cwd.as_ref() != Some(&cwd) {
            return;
        }
        match page {
            Ok(page) if self.page.as_ref() == Some(&page) && self.cdp.is_some() => {}
            Ok(page) => {
                self.disconnect();
                match Cdp::connect(&page.socket) {
                    Ok((cdp, mut events)) => {
                        self.reader = Some(cx.spawn(async move |this, cx| {
                            while let Some(event) = events.next().await {
                                if this
                                    .update(cx, |pane, cx| pane.on_event(event, cx))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }));
                        cdp.call("Page.enable", json!({}));
                        self.cdp = Some(cdp);
                        self.error = None;
                        self.ask_history();
                    }
                    Err(error) => self.error = Some(format!("{error:#}")),
                }
                self.page = Some(page);
            }
            Err(error) => {
                self.disconnect();
                self.error = Some(error);
            }
        }
        cx.notify();
    }

    pub fn set_agent(&mut self, agent: bool, cx: &mut Context<Self>) {
        if self.agent != agent {
            self.agent = agent;
            cx.notify();
        }
    }

    fn disconnect(&mut self) {
        self.cdp = None;
        self.reader = None;
        self.viewport = None;
        self.capturing = None;
        // A capture belongs to the page it was taken of.
        if let Some(marks) = self.marks.take() {
            self.spent.push(marks.shown);
        }
        if let Some(frame) = self.frame.take() {
            self.spent.push(frame);
        }
    }

    fn call(&self, method: &str, params: Value) -> Option<u64> {
        Some(self.cdp.as_ref()?.call(method, params))
    }

    fn ask_history(&mut self) {
        self.going_back = None;
        self.call("Page.getNavigationHistory", json!({}));
    }

    fn on_event(&mut self, event: CdpEvent, cx: &mut Context<Self>) {
        match event {
            CdpEvent::Frame {
                image,
                width,
                height,
            } => {
                if let Some(old) = self.frame.replace(image) {
                    self.spent.push(old);
                }
                self.frame_size = (width, height);
            }
            CdpEvent::Message(message) => self.on_message(message, cx),
            CdpEvent::Closed(why) => {
                log::info!("browser: {why}");
                self.disconnect();
                // The page or Chromium went away: the daemon opens another.
                self.page = None;
                self.cwd = None;
            }
        }
        cx.notify();
    }

    fn on_message(&mut self, message: Value, cx: &mut Context<Self>) {
        let main_frame = |frame: &Value| frame.get("parentId").is_none();
        match message["method"].as_str() {
            Some("Page.frameNavigated") if main_frame(&message["params"]["frame"]) => {
                self.set_url(message["params"]["frame"]["url"].as_str(), cx);
            }
            Some("Page.navigatedWithinDocument") => {
                self.set_url(message["params"]["url"].as_str(), cx);
            }
            Some("Page.frameStartedLoading") => self.loading = true,
            Some("Page.frameStoppedLoading") | Some("Page.loadEventFired") => self.loading = false,
            _ => {}
        }
        let Some(id) = message["id"].as_u64() else {
            return;
        };
        let result = &message["result"];
        if result.get("entries").is_some() {
            let entries = result["entries"].as_array().cloned().unwrap_or_default();
            let current = result["currentIndex"].as_u64().unwrap_or(0) as usize;
            if self.going_back == Some(id) {
                if let Some(entry) = current.checked_sub(1).and_then(|i| entries.get(i)) {
                    self.call(
                        "Page.navigateToHistoryEntry",
                        json!({ "entryId": entry["id"] }),
                    );
                }
            } else if let Some(entry) = entries.get(current) {
                self.set_url(entry["url"].as_str(), cx);
            }
        } else if let Some((asked, size)) = self.capturing
            && asked == id
        {
            self.capturing = None;
            self.read_capture(&message, size, cx);
        } else if self.copying == Some(id) {
            self.copying = None;
            if let Some(text) = result["result"]["value"].as_str()
                && !text.is_empty()
            {
                cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
            }
        }
    }

    fn set_url(&mut self, url: Option<&str>, cx: &mut Context<Self>) {
        let Some(url) = url else { return };
        self.url = url.to_owned();
        self.shown_url = None;
        cx.notify();
    }

    /// The address follows the page, unless the person is typing in it.
    fn sync_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.shown_url.as_deref() == Some(self.url.as_str())
            || self.address.focus_handle(cx).is_focused(window)
        {
            return;
        }
        let shown = if self.url == "about:blank" {
            ""
        } else {
            self.url.as_str()
        };
        self.address
            .update(cx, |editor, cx| editor.set_text(shown, window, cx));
        self.shown_url = Some(self.url.clone());
    }

    fn go(&mut self, _: &Go, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.address.read(cx).text(cx);
        if let Some(url) = address(&typed) {
            self.call("Page.navigate", json!({ "url": url }));
            window.focus(&self.focus, cx);
        }
    }

    fn back(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.going_back = self.call("Page.getNavigationHistory", json!({}));
    }

    fn reload(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.call("Page.reload", json!({}));
    }

    fn set_mobile(&mut self, mobile: bool, cx: &mut Context<Self>) {
        self.mobile = mobile;
        cx.notify();
    }

    /// The area the page is drawn in, measured each frame: the viewport follows it.
    fn measure(&mut self, area: Bounds<Pixels>, scale: f32) {
        self.area = area;
        let width = if self.mobile {
            MOBILE_WIDTH.min(f32::from(area.size.width))
        } else {
            f32::from(area.size.width)
        };
        let height = f32::from(area.size.height);
        if width < 1. || height < 1. {
            return;
        }
        let wanted = (
            width as u32,
            height as u32,
            (scale * 100.) as u32,
            self.mobile,
        );
        if self.viewport == Some(wanted) || self.cdp.is_none() {
            return;
        }
        self.viewport = Some(wanted);
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": wanted.0,
                "height": wanted.1,
                "deviceScaleFactor": scale,
                "mobile": self.mobile,
            }),
        );
        self.call(
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": self.mobile }),
        );
        self.call(
            "Page.startScreencast",
            json!({
                "format": "jpeg",
                "quality": 80,
                "maxWidth": (width * scale) as u32,
                "maxHeight": (height * scale) as u32,
                "everyNthFrame": 1,
            }),
        );
    }

    /// How far from the area's left the page is drawn: the mobile width sits in the middle.
    fn frame_left(&self) -> f32 {
        self.left_of(self.frame_size.0)
    }

    fn left_of(&self, width: f32) -> f32 {
        if self.mobile {
            ((f32::from(self.area.size.width) - width) / 2.).max(0.)
        } else {
            0.
        }
    }

    /// Where a pointer is on the page, in CSS pixels, when it is over the page.
    fn on_page(&self, position: Point<Pixels>) -> Option<(f32, f32)> {
        self.within(position, self.frame_size)
    }

    fn within(&self, position: Point<Pixels>, (width, height): (f32, f32)) -> Option<Spot> {
        let x = f32::from(position.x - self.area.origin.x) - self.left_of(width);
        let y = f32::from(position.y - self.area.origin.y);
        (x >= 0. && y >= 0. && x <= width.max(1.) && y <= height.max(1.)).then_some((x, y))
    }

    /// จับภาพและวาด: asks the page for a capture, at the size it shows now.
    fn capture(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.frame.is_none() || self.marks.is_some() {
            return;
        }
        let size = self.frame_size;
        self.marks_error = None;
        self.capturing = self
            .call(
                "Page.captureScreenshot",
                json!({ "format": "png", "fromSurface": true }),
            )
            .map(|id| (id, size));
        cx.notify();
    }

    /// Decodes the capture off the main thread and shows it to mark.
    fn read_capture(&mut self, message: &Value, size: (f32, f32), cx: &mut Context<Self>) {
        let Some(data) = message["result"]["data"].as_str().map(str::to_owned) else {
            let why = message["error"]["message"]
                .as_str()
                .unwrap_or("no image came back");
            self.marks_error = Some(format!("จับภาพไม่ได้: {why}"));
            return;
        };
        let decoding = cx
            .background_executor()
            .spawn(async move { Marks::from_capture(&data, size) });
        cx.spawn(async move |this, cx| {
            let marks = decoding.await;
            this.update(cx, |pane, cx| {
                match marks {
                    Ok(marks) => pane.marks = Some(marks),
                    Err(error) => pane.marks_error = Some(format!("จับภาพไม่ได้: {error:#}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Attaches the marked capture to the composer and gives the page back.
    fn attach_marks(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(marks) = self.marks.as_ref() else {
            return;
        };
        match marks.save() {
            Ok(path) => {
                self.put_away_marks(cx);
                cx.emit(BrowserEvent::Captured(path));
            }
            Err(error) => {
                log::warn!("saving a capture: {error:#}");
                self.marks_error = Some(format!("แนบภาพไม่ได้: {error:#}"));
                cx.notify();
            }
        }
    }

    /// Shows `marks` as a capture just taken.
    #[cfg(test)]
    pub fn show_capture(&mut self, marks: Marks, cx: &mut Context<Self>) {
        self.marks = Some(marks);
        cx.notify();
    }

    fn put_away_marks(&mut self, cx: &mut Context<Self>) {
        if let Some(marks) = self.marks.take() {
            self.spent.push(marks.shown);
        }
        self.marks_error = None;
        cx.notify();
    }

    fn undo_mark(&mut self, cx: &mut Context<Self>) {
        if let Some(marks) = self.marks.as_mut() {
            marks.undo();
            cx.notify();
        }
    }

    /// While a capture is shown, the pointer marks it instead of reaching the page. True when
    /// it did.
    fn mark(&mut self, position: Point<Pixels>, start: bool, cx: &mut Context<Self>) -> bool {
        let Some(size) = self.marks.as_ref().map(|m| m.size) else {
            return false;
        };
        if let Some(spot) = self.within(position, size)
            && let Some(marks) = self.marks.as_mut()
        {
            if start {
                marks.begin(spot);
            } else {
                marks.extend(spot);
            }
            cx.notify();
        }
        true
    }

    fn mouse(
        &self,
        kind: &str,
        position: Point<Pixels>,
        button: Option<MouseButton>,
        clicks: usize,
        modifiers: Modifiers,
    ) {
        let Some((x, y)) = self.on_page(position) else {
            return;
        };
        let button = match button {
            Some(MouseButton::Left) => "left",
            Some(MouseButton::Right) => "right",
            Some(MouseButton::Middle) => "middle",
            _ => "none",
        };
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": kind,
                "x": x,
                "y": y,
                "button": button,
                "buttons": if button == "left" && kind != "mouseReleased" { 1 } else { 0 },
                "clickCount": clicks,
                "modifiers": modifier_bits(modifiers),
            }),
        );
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if self.marks.is_some() {
            if event.button == MouseButton::Left {
                self.mark(event.position, true, cx);
            }
            return;
        }
        self.mouse(
            "mousePressed",
            event.position,
            Some(event.button),
            event.click_count,
            event.modifiers,
        );
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        if self.marks.is_some() {
            return;
        }
        self.mouse(
            "mouseReleased",
            event.position,
            Some(event.button),
            event.click_count,
            event.modifiers,
        );
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.marks.is_some() {
            if event.pressed_button == Some(MouseButton::Left) {
                self.mark(event.position, false, cx);
            }
            return;
        }
        self.mouse(
            "mouseMoved",
            event.position,
            event.pressed_button,
            0,
            event.modifiers,
        );
    }

    fn scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.marks.is_some() {
            return;
        }
        let Some((x, y)) = self.on_page(event.position) else {
            return;
        };
        let delta = event.delta.pixel_delta(px(LINE));
        // GPUI's delta moves the content; Chromium's moves the view.
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseWheel",
                "x": x,
                "y": y,
                "deltaX": -f32::from(delta.x),
                "deltaY": -f32::from(delta.y),
                "modifiers": modifier_bits(event.modifiers),
            }),
        );
        cx.stop_propagation();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if self.marks.is_some() {
            match keystroke.key.as_str() {
                "escape" => self.put_away_marks(cx),
                "z" if modifiers.platform || modifiers.control => self.undo_mark(cx),
                "enter" => self.attach_marks(window, cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if modifiers.platform {
            match keystroke.key.as_str() {
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                        self.call("Input.insertText", json!({ "text": text }));
                    }
                }
                "c" => {
                    self.copying = self.call(
                        "Runtime.evaluate",
                        json!({ "expression": "String(getSelection())", "returnByValue": true }),
                    );
                }
                "a" => self.press("a", "KeyA", 65, None, modifiers, &["selectAll"]),
                // Everything else with ⌘ is the window's.
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if event.prefer_character_input && keystroke.key_char.is_some() && !modifiers.control {
            return;
        }
        let Some((key, code, key_code)) = special_key(&keystroke.key) else {
            return;
        };
        let text = (key == "Enter").then_some("\r");
        self.press(key, code, key_code, text, modifiers, &[]);
        cx.stop_propagation();
    }

    fn press(
        &self,
        key: &str,
        code: &str,
        key_code: u32,
        text: Option<&str>,
        modifiers: Modifiers,
        commands: &[&str],
    ) {
        let common = json!({
            "key": key,
            "code": code,
            "windowsVirtualKeyCode": key_code,
            "nativeVirtualKeyCode": key_code,
            "modifiers": modifier_bits(modifiers),
        });
        let mut down = common.clone();
        down["type"] = json!("keyDown");
        if let Some(text) = text {
            down["text"] = json!(text);
            down["unmodifiedText"] = json!(text);
        }
        if !commands.is_empty() {
            down["commands"] = json!(commands);
        }
        let mut up = common;
        up["type"] = json!("keyUp");
        self.call("Input.dispatchKeyEvent", down);
        self.call("Input.dispatchKeyEvent", up);
    }

    /// Drops the frames drawn before the one on screen from the sprite atlas.
    fn drop_spent(&mut self, window: &mut Window, cx: &mut App) {
        for frame in self.spent.drain(..) {
            cx.drop_image(frame, Some(&mut *window));
        }
    }
}

/// What the person typed as the URL to open: a full URL as it is, a local address over http,
/// any other host over https. Words with no host are left alone.
pub fn address(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    if typed.contains("://") || typed.starts_with("about:") {
        return Some(typed.to_owned());
    }
    if typed.contains(char::is_whitespace) {
        return None;
    }
    let host = typed.split(['/', '?', '#']).next().unwrap_or(typed);
    let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
    let local = name == "localhost"
        || name.ends_with(".localhost")
        || name.parse::<std::net::Ipv4Addr>().is_ok();
    if local {
        Some(format!("http://{typed}"))
    } else if name.contains('.') {
        Some(format!("https://{typed}"))
    } else {
        None
    }
}

/// DevTools' modifier bits: Alt 1, Ctrl 2, Meta 4, Shift 8.
fn modifier_bits(m: Modifiers) -> u32 {
    u32::from(m.alt)
        | u32::from(m.control) << 1
        | u32::from(m.platform) << 2
        | u32::from(m.shift) << 3
}

/// A key GPUI names that types no text, as DevTools names it: its key, code and key code.
fn special_key(key: &str) -> Option<(&'static str, &'static str, u32)> {
    Some(match key {
        "enter" => ("Enter", "Enter", 13),
        "backspace" => ("Backspace", "Backspace", 8),
        "tab" => ("Tab", "Tab", 9),
        "escape" => ("Escape", "Escape", 27),
        "delete" => ("Delete", "Delete", 46),
        "left" => ("ArrowLeft", "ArrowLeft", 37),
        "right" => ("ArrowRight", "ArrowRight", 39),
        "up" => ("ArrowUp", "ArrowUp", 38),
        "down" => ("ArrowDown", "ArrowDown", 40),
        "home" => ("Home", "Home", 36),
        "end" => ("End", "End", 35),
        "pageup" => ("PageUp", "PageUp", 33),
        "pagedown" => ("PageDown", "PageDown", 34),
        _ => return None,
    })
}

/// The marks over a capture drawn `left` pixels in from `origin`.
fn paint_strokes(strokes: &[Vec<Spot>], origin: Point<Pixels>, left: f32, window: &mut Window) {
    let [r, g, b, _] = browser_marks::RED;
    let colour = gpui::rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b));
    for stroke in strokes {
        let at = |&(x, y): &Spot| point(origin.x + px(left + x), origin.y + px(y));
        let Some(first) = stroke.first() else {
            continue;
        };
        if stroke.len() == 1 {
            let radius = px(browser_marks::STROKE / 2.);
            window.paint_quad(
                gpui::fill(
                    Bounds::centered_at(at(first), gpui::size(radius * 2., radius * 2.)),
                    colour,
                )
                .corner_radii(radius),
            );
            continue;
        }
        let mut builder = PathBuilder::stroke(px(browser_marks::STROKE));
        builder.move_to(at(first));
        for spot in &stroke[1..] {
            builder.line_to(at(spot));
        }
        if let Ok(path) = builder.build() {
            window.paint_path(path, colour);
        }
    }
}

fn tool_button(cx: &App, id: &'static str, icon: &'static str) -> gpui::Stateful<gpui::Div> {
    let colors = cx.theme().colors();
    div()
        .id(id)
        .flex_none()
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .cursor_pointer()
        .hover(|b| b.bg(colors.element_hover))
        .child(svg().path(icon).size(px(14.)).text_color(colors.text_muted))
}

impl BrowserPane {
    /// Back, reload, the address, the widths and จับภาพ: in the block's header when the
    /// block holds only this page, else over the page.
    pub fn toolbar(&mut self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let colors = cx.theme().colors().clone();
        let field = tokens::Tokens::of(cx).field;
        let width = |mobile: bool, label: &'static str, id: &'static str| {
            let on = self.mobile == mobile;
            div()
                .id(id)
                .px_2()
                .h(px(24.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .text_meta()
                .cursor_pointer()
                .when(on, |b| {
                    b.bg(colors.element_selected).text_color(colors.text)
                })
                .when(!on, |b| {
                    b.text_color(colors.text_muted)
                        .hover(|b| b.bg(colors.element_hover))
                })
                .on_click(cx.listener(move |pane, _, _, cx| pane.set_mobile(mobile, cx)))
                .child(label)
        };
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap_1()
            .child(
                tool_button(cx, "browser-back", "icons/arrow_left.svg")
                    .on_click(cx.listener(|pane, _, window, cx| pane.back(window, cx))),
            )
            .child(
                tool_button(cx, "browser-reload", "icons/rotate_cw.svg")
                    .on_click(cx.listener(|pane, _, window, cx| pane.reload(window, cx))),
            )
            .child(
                div()
                    .key_context("BrowserUrl")
                    .on_action(cx.listener(Self::go))
                    .flex_1()
                    .min_w_0()
                    .h(px(28.))
                    .px_2p5()
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .bg(field)
                    .text_ui()
                    .child(self.address.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap_0p5()
                    .child(width(true, "มือถือ", "browser-mobile"))
                    .child(width(false, "เดสก์ท็อป", "browser-desktop")),
            )
            .child(
                tool_button(cx, "browser-capture", "icons/pencil.svg")
                    .debug_selector(|| "browser-capture".into())
                    .when(self.frame.is_none(), |b| b.opacity(0.4))
                    .tooltip(|_, cx| {
                        crate::tooltip::text("จับภาพและวาด แล้วแนบไปกับข้อความ".into(), cx)
                    })
                    .on_click(cx.listener(|pane, _, window, cx| pane.capture(window, cx))),
            )
            .id("browser-toolbar")
            .debug_selector(|| "browser-toolbar".into())
    }

    /// The row over a capture while it is marked: what to do, undo, put away, attach.
    fn render_marking_bar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let colors = cx.theme().colors();
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex_none()
                .h(px(24.))
                .px_2()
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .when(primary, |b| {
                    b.bg(colors.element_selected).text_color(colors.text)
                })
                .when(!primary, |b| {
                    b.border_1()
                        .border_color(colors.border)
                        .text_color(colors.text_muted)
                })
                .hover(|b| b.bg(colors.element_hover))
                .child(label)
        };
        let marked = self.marks.as_ref().is_some_and(|m| !m.strokes.is_empty());
        div()
            .debug_selector(|| "browser-marking".into())
            .flex_none()
            .mx_2()
            .mb_1()
            .px_2p5()
            .py_1()
            .flex()
            .items_center()
            .gap_1()
            .rounded(px(8.))
            .border_1()
            .border_color(colors.border)
            .text_meta()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(colors.text_muted)
                    .children(self.marks_error.clone())
                    .when(self.marks_error.is_none(), |d| {
                        d.child("วาดบนภาพเพื่อชี้จุดที่ต้องการ")
                    }),
            )
            .when(marked, |bar| {
                bar.child(
                    button("browser-marks-undo", "ย้อน", false)
                        .on_click(cx.listener(|pane, _, _, cx| pane.undo_mark(cx))),
                )
            })
            .child(
                button("browser-marks-cancel", "ยกเลิก", false)
                    .on_click(cx.listener(|pane, _, _, cx| pane.put_away_marks(cx))),
            )
            .child(
                button("browser-marks-attach", "แนบไปกับข้อความ", true)
                    .on_click(cx.listener(|pane, _, window, cx| pane.attach_marks(window, cx))),
            )
    }
}

impl Render for BrowserPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drop_spent(window, cx);
        self.sync_address(window, cx);
        let colors = cx.theme().colors().clone();
        let pane = cx.entity();
        let focus = self.focus.clone();
        let toolbar = (!self.bar_in_header).then(|| {
            div()
                .flex_none()
                .h(px(40.))
                .px_2()
                .flex()
                .child(self.toolbar(cx))
        });
        let marking = self.marks.is_some().then(|| self.render_marking_bar(cx));
        let marks_error = (self.marks.is_none())
            .then(|| self.marks_error.clone())
            .flatten()
            .map(|why| {
                div()
                    .flex_none()
                    .mx_2()
                    .mb_1()
                    .text_meta()
                    .text_color(cx.theme().status().error)
                    .child(why)
            });
        let banner = self.agent.then(|| {
            div()
                .flex_none()
                .mx_2()
                .mb_1()
                .px_2p5()
                .py_1()
                .rounded(px(8.))
                .border_1()
                .border_color(colors.border)
                .text_meta()
                .text_color(colors.text_muted)
                .child("agent กำลังใช้หน้านี้ · เปิดได้เฉพาะ localhost และเว็บที่ thread นี้เข้าถึงได้")
        });
        let (frame_width, frame_height) = self.frame_size;
        let page = div()
            .id("browser-page")
            .debug_selector(|| "browser-page".into())
            .key_context("BrowserPage")
            .track_focus(&self.focus)
            .relative()
            .flex_1()
            .min_h_0()
            // Under the header's toolbar the page runs to the block's edges.
            .when(!self.bar_in_header, |page| {
                page.mx_2().mb_2().rounded(px(8.))
            })
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_scroll_wheel(cx.listener(Self::scroll))
            .on_key_down(cx.listener(Self::key_down))
            .child(
                canvas(
                    {
                        let pane = pane.clone();
                        move |bounds, window, cx| {
                            let scale = window.scale_factor();
                            pane.update(cx, |pane, _| pane.measure(bounds, scale));
                        }
                    },
                    move |bounds, _, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, pane), cx)
                    },
                )
                .absolute()
                .size_full(),
            )
            .map(|page| match (&self.marks, &self.frame, &self.error) {
                (Some(marks), _, _) => {
                    let (width, height) = marks.size;
                    let left = self.left_of(width);
                    let strokes = marks.strokes.clone();
                    page.cursor_crosshair()
                        .child(
                            img(gpui::ImageSource::Render(marks.shown.clone()))
                                .absolute()
                                .top_0()
                                .left(px(left))
                                .w(px(width))
                                .h(px(height)),
                        )
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |bounds, _, window, _| {
                                    paint_strokes(&strokes, bounds.origin, left, window)
                                },
                            )
                            .absolute()
                            .size_full(),
                        )
                }
                (None, Some(frame), _) => page.child(
                    img(gpui::ImageSource::Render(frame.clone()))
                        .absolute()
                        .top_0()
                        .left(px(self.frame_left()))
                        .w(px(frame_width))
                        .h(px(frame_height)),
                ),
                (None, None, error) => page.child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p_4()
                        .text_ui()
                        .text_color(colors.text_muted)
                        .child(error.clone().unwrap_or_else(|| {
                            if self.cdp.is_some() {
                                "พิมพ์ที่อยู่ด้านบนเพื่อเปิดหน้าเว็บ".into()
                            } else {
                                "กำลังเปิด browser…".into()
                            }
                        })),
                ),
            });
        div()
            .id("browser")
            .size_full()
            .flex()
            .flex_col()
            .children(toolbar)
            // A quiet line while the page loads, in place of a spinner.
            .child(
                div()
                    .flex_none()
                    .h(px(2.))
                    .mx_2()
                    .rounded_full()
                    .when(self.loading, |line| line.bg(colors.border_focused)),
            )
            .children(banner)
            .children(marking)
            .children(marks_error)
            .child(page)
    }
}

impl gpui::Focusable for BrowserPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for BrowserPane {
    fn text_for_range(
        &mut self,
        _: std::ops::Range<usize>,
        _: &mut Option<std::ops::Range<usize>>,
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

    fn marked_text_range(
        &self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        _: Option<std::ops::Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        if !text.is_empty() {
            self.call("Input.insertText", json!({ "text": text }));
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<std::ops::Range<usize>>,
        _: &str,
        _: Option<std::ops::Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    fn bounds_for_range(
        &mut self,
        _: std::ops::Range<usize>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_gets_the_scheme_it_needs() {
        assert_eq!(
            address("localhost:3000").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            address("127.0.0.1:8080/x").as_deref(),
            Some("http://127.0.0.1:8080/x")
        );
        assert_eq!(
            address("app.localhost").as_deref(),
            Some("http://app.localhost")
        );
        assert_eq!(
            address("docs.rs/gpui").as_deref(),
            Some("https://docs.rs/gpui")
        );
        assert_eq!(address("https://x.dev").as_deref(), Some("https://x.dev"));
        assert_eq!(address("about:blank").as_deref(), Some("about:blank"));
        assert_eq!(address("how to"), None);
        assert_eq!(address("  "), None);
    }

    #[test]
    fn modifiers_are_devtools_bits() {
        let m = Modifiers {
            alt: true,
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(modifier_bits(m), 9);
    }
}
