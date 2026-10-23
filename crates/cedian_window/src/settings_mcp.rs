//! Settings, MCP (Interface, Settings window): the person's MCP servers, as the Claude desktop
//! app and Cursor manage theirs. The list is cedia's (`cedia mcp list`), so a server added here
//! is the one every run of cedia gets; a server shows what it runs or reaches, the names of its
//! keys but never their values, and why cedia would refuse it. ตรวจ starts it and counts its
//! tools, ลบ asks in the row first, and the form under the list adds one, checked at once.

use std::collections::HashMap;

use cedian_protocol::{McpCheck, McpEntry, McpList};
use cedian_wire::{McpAdd, McpTarget, Request};
use editor::Editor;
use gpui::{
    AnyElement, AppContext as _, Context, Div, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use crate::settings_view::{SettingsEvent, SettingsView};
use crate::tokens;
use crate::type_scale::TypeScale as _;

#[derive(Default)]
pub struct McpSection {
    /// cedia offers `mcp.manage`.
    pub capable: bool,
    list: Option<McpList>,
    /// Why cedia refused the last add or remove.
    refused: Option<String>,
    /// Each server's last check, or `None` while it runs.
    checks: HashMap<String, Option<McpCheck>>,
    /// The row asking whether to remove its server.
    removing: Option<String>,
    form: Option<Form>,
    /// The server just sent to be added, checked once cedia lists it.
    adding: Option<String>,
    /// cedia added what the form said: the form empties when next drawn.
    clear_form: bool,
}

struct Form {
    name: Entity<Editor>,
    target: Entity<Editor>,
    pairs: Entity<Editor>,
    read_only: bool,
}

/// What the form says, read as `cedia mcp add` takes it.
fn read_form(
    name: &str,
    target: &str,
    pairs: &str,
    read_only: bool,
) -> Result<(String, McpAdd), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("ใส่ชื่อ server".into());
    }
    let target = target.trim();
    let target = if target.starts_with("http://") || target.starts_with("https://") {
        McpTarget::Url {
            url: target.to_owned(),
        }
    } else {
        let mut words = shlex::split(target)
            .ok_or("คำสั่งมีเครื่องหมายคำพูดไม่ครบคู่")?
            .into_iter();
        let command = words.next().ok_or("ใส่คำสั่งที่เริ่ม server หรือ URL ของมัน")?;
        McpTarget::Command {
            command,
            args: words.collect(),
        }
    };
    let pairs = shlex::split(pairs.trim())
        .ok_or("ค่า key มีเครื่องหมายคำพูดไม่ครบคู่")?
        .into_iter()
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) if !key.is_empty() => Ok((key.to_owned(), value.to_owned())),
            _ => Err(format!("{pair} ต้องเป็น NAME=value")),
        })
        .collect::<Result<_, _>>()?;
    Ok((
        name.to_owned(),
        McpAdd {
            target,
            pairs,
            read_only,
        },
    ))
}

/// What a server runs or reaches, in one line.
fn reach(server: &McpEntry) -> String {
    match (&server.url, &server.command) {
        (Some(url), _) => url.clone(),
        (None, Some(command)) => std::iter::once(command.as_str())
            .chain(server.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        (None, None) => String::new(),
    }
}

impl SettingsView {
    fn send(&self, request: Request, cx: &mut Context<Self>) {
        cx.emit(SettingsEvent::Send(Box::new(request)));
    }

    /// Opening the section asks cedia for the list again.
    pub(crate) fn open_mcp(&mut self, cx: &mut Context<Self>) {
        if self.mcp.capable {
            self.send(Request::McpList, cx);
        }
    }

    /// cedia's list, after a change or a fresh ask.
    pub fn set_mcp(&mut self, list: McpList, refused: Option<String>, cx: &mut Context<Self>) {
        let added = self
            .mcp
            .adding
            .take()
            .filter(|name| refused.is_none() && list.servers.iter().any(|s| &s.name == name));
        if let Some(name) = added {
            self.mcp.clear_form = true;
            self.check(name, cx);
        }
        self.mcp
            .checks
            .retain(|name, _| list.servers.iter().any(|s| &s.name == name));
        self.mcp.list = Some(list);
        self.mcp.refused = refused;
        cx.notify();
    }

    pub fn set_mcp_check(&mut self, check: McpCheck, cx: &mut Context<Self>) {
        let name = match &check {
            McpCheck::Variant0 { name, .. } | McpCheck::Variant1 { name, .. } => name.clone(),
        };
        self.mcp.checks.insert(name, Some(check));
        cx.notify();
    }

    fn check(&mut self, name: String, cx: &mut Context<Self>) {
        self.mcp.checks.insert(name.clone(), None);
        self.send(Request::McpCheck { name }, cx);
        cx.notify();
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.mcp.form else {
            return;
        };
        let text = |field: &Entity<Editor>| field.read(cx).text(cx);
        match read_form(
            &text(&form.name),
            &text(&form.target),
            &text(&form.pairs),
            form.read_only,
        ) {
            Ok((name, server)) => {
                self.mcp.refused = None;
                self.mcp.adding = Some(name.clone());
                self.send(Request::McpAdd { name, server }, cx);
            }
            Err(error) => self.mcp.refused = Some(error),
        }
        cx.notify();
    }

    fn form(&mut self, window: &mut Window, cx: &mut Context<Self>) -> &Form {
        self.mcp.form.get_or_insert_with(|| {
            let mut field = |placeholder: &'static str, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    let mut editor = Editor::single_line(window, cx);
                    editor.set_placeholder_text(placeholder, window, cx);
                    editor
                })
            };
            Form {
                name: field("github", cx),
                target: field(
                    "npx -y @modelcontextprotocol/server-github หรือ https://…",
                    cx,
                ),
                pairs: field("GITHUB_TOKEN=… (env ของคำสั่ง หรือ header ของ URL)", cx),
                read_only: false,
            }
        })
    }

    pub(crate) fn render_mcp(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors().clone();
        if !self.mcp.capable {
            return div().text_ui().text_color(colors.text_muted).child(
                "cedia ตัวนี้ยังจัดการ MCP server ไม่ได้ ตอนนี้แก้ได้ที่ mcpServers ใน settings.json โดยตรง",
            );
        }
        let mut section = div().flex().flex_col().gap_4().child(
            div()
                .text_meta()
                .text_color(colors.text_muted)
                .child("cedia เขียน mcpServers ใน settings.json ให้ ทุก thread ได้ server ชุดนี้ ค่าของ key ไม่แสดงซ้ำหลังบันทึก"),
        );
        section = match self.mcp.list.as_ref().map(|l| l.servers.clone()) {
            None => section.child(
                div()
                    .text_ui()
                    .text_color(colors.text_muted)
                    .child("กำลังอ่าน…"),
            ),
            Some(servers) if servers.is_empty() => section.child(
                div()
                    .text_ui()
                    .text_color(colors.text_muted)
                    .child("ยังไม่มี MCP server เพิ่มด้านล่างได้เลย"),
            ),
            Some(servers) => section.child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.border)
                    .children(
                        servers
                            .iter()
                            .enumerate()
                            .map(|(n, server)| self.render_server(n, server, cx)),
                    ),
            ),
        };
        section.child(self.render_form(window, cx))
    }

    fn render_server(&self, n: usize, server: &McpEntry, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let status = cx.theme().status();
        let name = server.name.clone();
        let mut keys: Vec<String> = server.env.iter().chain(&server.headers).cloned().collect();
        if server.read_only == Some(true) {
            keys.push("อ่านอย่างเดียว".into());
        }
        let check = self.mcp.checks.get(&server.name);
        let state: Option<(String, gpui::Hsla)> = match check {
            Some(None) => Some(("กำลังตรวจ…".into(), colors.text_muted)),
            Some(Some(McpCheck::Variant0 { tools, .. })) => {
                Some((format!("ต่อได้ · {} tools", tools.len()), status.success))
            }
            Some(Some(McpCheck::Variant1 { error, .. })) => Some((error.clone(), status.error)),
            None => server.error.clone().map(|e| (e, status.error)),
        };
        let button = |id: String, label: &'static str| {
            div()
                .id(SharedString::from(id.clone()))
                .debug_selector(move || id.clone())
                .flex_none()
                .h(px(24.))
                .px_2()
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_meta()
                .child(label)
        };
        let actions = if self.mcp.removing.as_deref() == Some(&server.name) {
            let yes = name.clone();
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().text_meta().child("ลบ?"))
                .child(
                    button(format!("mcp-remove-yes-{name}"), "ลบ")
                        .bg(status.error.opacity(0.15))
                        .text_color(status.error)
                        .hover(|b| b.bg(status.error.opacity(0.25)))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.mcp.removing = None;
                            view.send(Request::McpRemove { name: yes.clone() }, cx);
                        })),
                )
                .child(
                    button(format!("mcp-remove-no-{name}"), "ยกเลิก")
                        .text_color(colors.text_muted)
                        .hover(|b| b.bg(colors.element_hover))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.mcp.removing = None;
                            cx.notify();
                        })),
                )
        } else {
            let (to_check, to_remove) = (name.clone(), name.clone());
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    button(format!("mcp-check-{name}"), "ตรวจ")
                        .text_color(colors.text)
                        .hover(|b| b.bg(colors.element_hover))
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.check(to_check.clone(), cx)),
                        ),
                )
                .child(
                    button(format!("mcp-remove-{name}"), "ลบ")
                        .text_color(colors.text_muted)
                        .hover(|b| b.bg(colors.element_hover))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.mcp.removing = Some(to_remove.clone());
                            cx.notify();
                        })),
                )
        };
        div()
            .debug_selector(move || format!("mcp-server-{name}"))
            .flex()
            .items_start()
            .gap_3()
            .px_3()
            .py_2()
            .when(n > 0, |row| row.border_t_1().border_color(colors.border))
            .child(
                div()
                    .flex_grow(1.)
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_ui()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(server.name.clone()),
                    )
                    .child(
                        div()
                            .font(tokens::code_font())
                            .text_meta()
                            .text_color(colors.text_muted)
                            .truncate()
                            .child(reach(server)),
                    )
                    .when(!keys.is_empty(), |info| {
                        info.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_1()
                                .children(keys.into_iter().map(|key| {
                                    div()
                                        .px(px(5.))
                                        .rounded(px(4.))
                                        .border_1()
                                        .border_color(colors.border)
                                        .font(tokens::code_font())
                                        .text_size(px(10.))
                                        .text_color(colors.text_muted)
                                        .child(key)
                                })),
                        )
                    })
                    .children(
                        state.map(|(text, color)| div().text_meta().text_color(color).child(text)),
                    ),
            )
            .child(actions)
            .into_any_element()
    }

    fn render_form(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors().clone();
        let form = self.form(window, cx);
        let (name, target, pairs, read_only) = (
            form.name.clone(),
            form.target.clone(),
            form.pairs.clone(),
            form.read_only,
        );
        let read_only = read_only && !self.mcp.clear_form;
        if std::mem::take(&mut self.mcp.clear_form) {
            if let Some(form) = &mut self.mcp.form {
                form.read_only = false;
            }
            for field in [&name, &target, &pairs] {
                field.update(cx, |field, cx| field.set_text("", window, cx));
            }
        }
        let field = |label: &'static str, editor: Entity<Editor>| {
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(110.))
                        .flex_none()
                        .text_meta()
                        .text_color(colors.text_muted)
                        .child(label),
                )
                .child(
                    div()
                        .flex_grow(1.)
                        .min_w_0()
                        .h(px(28.))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.editor_background)
                        .text_ui()
                        .child(editor),
                )
        };
        div()
            .debug_selector(|| "mcp-add".into())
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_label()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text_muted)
                    .child("เพิ่ม server"),
            )
            .child(field("ชื่อ", name))
            .child(field("คำสั่งหรือ URL", target))
            .child(field("key", pairs))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().w(px(110.)).flex_none())
                    .child(
                        div()
                            .id("mcp-read-only")
                            .debug_selector(|| "mcp-read-only".into())
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .cursor_pointer()
                            .text_meta()
                            .child(
                                div()
                                    .size(px(14.))
                                    .rounded(px(3.))
                                    .border_1()
                                    .border_color(colors.border)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(read_only, |b| {
                                        b.bg(colors.text).text_color(colors.background).child("✓")
                                    }),
                            )
                            .child("อ่านอย่างเดียว")
                            .on_click(cx.listener(|view, _, _, cx| {
                                if let Some(form) = &mut view.mcp.form {
                                    form.read_only = !form.read_only;
                                }
                                cx.notify();
                            })),
                    )
                    .child(div().flex_grow(1.))
                    .child(
                        div()
                            .id("mcp-add-button")
                            .debug_selector(|| "mcp-add-button".into())
                            .h(px(28.))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded(px(7.))
                            .cursor_pointer()
                            .bg(colors.element_selected)
                            .hover(|b| b.bg(colors.element_hover))
                            .text_ui()
                            .child(if self.mcp.adding.is_some() {
                                "กำลังเพิ่ม…"
                            } else {
                                "เพิ่ม"
                            })
                            .on_click(cx.listener(|view, _, _, cx| view.add(cx))),
                    ),
            )
            .children(self.mcp.refused.clone().map(|refused| {
                div()
                    .debug_selector(|| "mcp-refused".into())
                    .text_meta()
                    .text_color(cx.theme().status().error)
                    .child(refused)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_reads_a_command_with_quotes_or_a_url() {
        let (name, add) = read_form(
            " gh ",
            "npx -y 'my server' --flag",
            "TOKEN=a=b HOME='/x y'",
            true,
        )
        .expect("form");
        assert_eq!(name, "gh");
        assert_eq!(
            add,
            McpAdd {
                target: McpTarget::Command {
                    command: "npx".into(),
                    args: vec!["-y".into(), "my server".into(), "--flag".into()],
                },
                pairs: vec![
                    ("TOKEN".into(), "a=b".into()),
                    ("HOME".into(), "/x y".into())
                ],
                read_only: true,
            }
        );
        let (_, add) = read_form("x", "https://mcp.example.com/v1", "", false).expect("url");
        assert_eq!(
            add.target,
            McpTarget::Url {
                url: "https://mcp.example.com/v1".into()
            }
        );
        assert!(add.pairs.is_empty());
        assert!(read_form("", "npx", "", false).is_err());
        assert!(read_form("x", "", "", false).is_err());
        assert!(read_form("x", "npx", "=v", false).is_err());
        assert!(read_form("x", "npx 'open", "", false).is_err());
    }
}
