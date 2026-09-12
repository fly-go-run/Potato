use crate::view::{icon_button, muted, row_button};
use crate::*;
use gpui_kit::base::FocusTrapElement;
use gpui_kit::component::button::*;
use gpui_kit::component::popover::Popover;
use gpui_kit::prelude::*;
use std::collections::BTreeSet;
#[derive(Default)]
pub struct Conversations {
    pub editing: Option<Value>,
    pub deleting: bool,
    pub archive_open: bool,
    pub archive_loading: bool,
    pub archive_rows: Vec<Value>,
    pub archive_error: Option<String>,
    pub archive_row_errors: BTreeMap<String, String>,
    pub archive_restoring: BTreeSet<String>,
    archive_epoch: u64,
}
impl Potato {
    pub fn edit_conversation(&mut self, chat: Value, w: &mut Window, cx: &mut Context<Self>) {
        if self.settings.open || self.workspace.editing || self.conversations.archive_open {
            return;
        }
        self.modal_focus.focus(w, cx);
        self.fields.remove("conversation-name");
        self.field(
            "conversation-name",
            &string(&chat, "name"),
            "会话名称",
            w,
            cx,
        );
        self.conversations.editing = Some(chat);
        self.conversations.deleting = false;
        cx.notify();
    }
    fn change_conversation(
        &mut self,
        method: &str,
        body: Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let Some(chat) = self.conversations.editing.clone() else {
            return;
        };
        let id = string(&chat, "id");
        let removed = method == "DELETE" || body["archived"] == true;
        self.busy = true;
        self.request(
            method,
            &format!("/api/chats/{}", segment(&id)),
            body,
            w,
            cx,
            move |s, _, w, cx| {
                s.busy = false;
                s.conversations.editing = None;
                if removed && s.selected.as_deref() == Some(&id) {
                    s.new_chat(w, cx);
                }
                s.refresh(w, cx);
            },
        );
    }
    pub fn conversation_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let chat = self.conversations.editing.clone().unwrap();
        let name = self.field(
            "conversation-name",
            &string(&chat, "name"),
            "会话名称",
            w,
            cx,
        );
        let mut panel = div()
            .w(px(420.))
            .max_w_full()
            .p_6()
            .rounded(px(16.))
            .bg(cx.theme().background)
            .shadow(vec![design::shadow()])
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child("会话设置")
                    .child(
                        icon_button("close-conversation", IconName::X, "关闭会话设置").on_click(
                            cx.listener(|s, _, _, cx| {
                                s.conversations.editing = None;
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(Input::new(&name).h(px(36.)).aria_label("会话名称"))
            .child(
                Button::new("rename-conversation")
                    .outline()
                    .label("保存名称")
                    .disabled(self.busy)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.change_conversation(
                            "PUT",
                            json!({"name":s.value("conversation-name",cx)}),
                            w,
                            cx,
                        );
                    })),
            )
            .child(
                Button::new("pin-conversation")
                    .ghost()
                    .label(if chat["pinned"] == true {
                        "取消置顶"
                    } else {
                        "置顶"
                    })
                    .disabled(self.busy)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        let pinned = s
                            .conversations
                            .editing
                            .as_ref()
                            .is_some_and(|v| v["pinned"] == true);
                        s.change_conversation("PUT", json!({"pinned":!pinned}), w, cx);
                    })),
            )
            .child(
                Button::new("archive-conversation")
                    .ghost()
                    .label(if chat["archived"] == true {
                        "恢复会话"
                    } else {
                        "归档会话"
                    })
                    .disabled(self.busy || self.streaming)
                    .on_click(cx.listener(|s, _, w, cx| {
                        let archived = s
                            .conversations
                            .editing
                            .as_ref()
                            .is_some_and(|v| v["archived"] == true);
                        s.change_conversation("PUT", json!({"archived":!archived}), w, cx);
                    })),
            )
            .child(
                Button::new("delete-conversation")
                    .ghost()
                    .text_color(cx.theme().danger)
                    .label("删除会话")
                    .disabled(self.busy || self.streaming)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.conversations.deleting = true;
                        cx.notify();
                    })),
            );
        if self.conversations.deleting {
            panel = panel
                .child(muted("删除后无法恢复。确认删除这条会话及其消息？", cx))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-conversation-delete")
                                .outline()
                                .label("取消")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.conversations.deleting = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("confirm-conversation-delete")
                                .danger()
                                .label("确认删除")
                                .disabled(self.busy)
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.change_conversation("DELETE", Value::Null, w, cx)
                                })),
                        ),
                );
        }
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x14141448))
            .flex()
            .items_center()
            .justify_center()
            .child(div().id("conversation-dialog").occlude().child(panel))
            .track_focus(&self.modal_focus)
            .focus_trap("conversation-trap", &self.modal_focus)
            .into_any_element()
    }
}

impl Potato {
    pub fn archive_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity();
        div()
            .id("archive-menu-trigger")
            .track_focus(&self.archive_trigger_focus)
            .tab_index(0)
            .opacity(if self.menu == Some(Menu::Conversations) {
                1.
            } else {
                0.
            })
            .group_hover("conversation-heading", |d| d.opacity(1.))
            .focus(|d| d.opacity(1.))
            .on_key_down(cx.listener(|s, event: &KeyDownEvent, w, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    s.archive_trigger_focus.focus(w, cx);
                    s.menu = Some(Menu::Conversations);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                Popover::new("conversation-menu")
                    .anchor(Anchor::TopRight)
                    .open(self.menu == Some(Menu::Conversations))
                    .trigger(
                        icon_button("conversation-menu-button", IconName::Ellipsis, "会话菜单")
                            .tab_stop(false)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.archive_trigger_focus.focus(w, cx);
                                s.menu = Some(Menu::Conversations);
                                cx.notify();
                            })),
                    )
                    .on_open_change(move |open, _, cx| {
                        owner.update(cx, |s, cx| {
                            if *open {
                                s.menu = Some(Menu::Conversations);
                            } else if s.menu == Some(Menu::Conversations) {
                                s.menu = None;
                            }
                            cx.notify();
                        });
                    })
                    .p_1()
                    .child(
                        row_button("open-archive", "查看已归档")
                            .w(px(148.))
                            .on_click(cx.listener(|s, _, w, cx| s.open_archive(w, cx))),
                    ),
            )
            .into_any_element()
    }

    pub fn open_archive(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.settings.open || self.workspace.editing || self.conversations.editing.is_some() {
            return;
        }
        self.menu = None;
        self.conversations.archive_open = true;
        self.archive_search.update(cx, |input, cx| {
            input.set_value("", w, cx);
            input.focus(w, cx);
        });
        self.load_archives(w, cx);
    }

    pub fn close_archive(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.conversations.archive_open = false;
        self.conversations.archive_epoch += 1;
        self.archive_trigger_focus.focus(w, cx);
        cx.notify();
    }

    fn load_archives(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.conversations.archive_loading = true;
        self.conversations.archive_error = None;
        self.conversations.archive_epoch += 1;
        let epoch = self.conversations.archive_epoch;
        self.request_result(
            "GET",
            "/api/chats?archived=true",
            Value::Null,
            w,
            cx,
            move |s, result, _, _| {
                if epoch != s.conversations.archive_epoch {
                    return;
                }
                s.conversations.archive_loading = false;
                match result {
                    Ok(v) => s.conversations.archive_rows = array(v),
                    Err(e) => s.conversations.archive_error = Some(e),
                }
            },
        );
        cx.notify();
    }

    pub fn restore_archive(&mut self, id: String, w: &mut Window, cx: &mut Context<Self>) {
        if !self.conversations.archive_restoring.insert(id.clone()) {
            return;
        }
        self.conversations.archive_row_errors.remove(&id);
        self.request_result(
            "PUT",
            &format!("/api/chats/{}", segment(&id)),
            json!({"archived":false}),
            w,
            cx,
            move |s, result, w, cx| s.finish_archive_restore(id, result, w, cx),
        );
        cx.notify();
    }

    pub fn finish_archive_restore(
        &mut self,
        id: String,
        result: backend::Reply,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.conversations.archive_restoring.remove(&id);
        match result {
            Ok(chat) => {
                self.conversations.archive_row_errors.remove(&id);
                self.conversations.archive_rows.retain(|c| c["id"] != id);
                self.chats.retain(|c| c["id"] != id);
                self.chats.insert(0, chat);
                self.chats
                    .sort_by_key(|c| std::cmp::Reverse(c["pinned"] == true));
                // An overlapping load may have read the record before restoration.
                if self.conversations.archive_open && self.conversations.archive_loading {
                    self.load_archives(w, cx);
                }
            }
            Err(e) => {
                self.conversations.archive_row_errors.insert(id, e);
            }
        }
    }

    pub fn archive_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.archive_search.read(cx).value().trim().to_lowercase();
        let rows: Vec<_> = self
            .conversations
            .archive_rows
            .iter()
            .filter(|chat| string(chat, "name").to_lowercase().contains(&query))
            .cloned()
            .collect();
        let mut list = div()
            .id("archive-list")
            .min_h_0()
            .overflow_y_scroll()
            .h(px((self.conversations.archive_rows.len() as f32 * 49.)
                .clamp(
                    144.,
                    (f32::from(w.viewport_size().height) - 240.).clamp(144., 360.),
                )))
            .flex()
            .flex_col();
        if self.conversations.archive_loading {
            list = list.child(muted("正在加载…", cx).py_6());
        } else if let Some(error) = &self.conversations.archive_error {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_4()
                    .child(muted("加载失败，请重试", cx))
                    .child(
                        Button::new("retry-archives")
                            .ghost()
                            .small()
                            .label("重试")
                            .tooltip(error.clone())
                            .on_click(cx.listener(|s, _, w, cx| s.load_archives(w, cx))),
                    ),
            );
        } else if rows.is_empty() {
            list = list.child(
                muted(
                    if query.is_empty() {
                        "暂无已归档会话"
                    } else {
                        "没有匹配的会话"
                    },
                    cx,
                )
                .py_6(),
            );
        } else {
            for (i, chat) in rows.into_iter().enumerate() {
                let id = string(&chat, "id");
                let pending = self.conversations.archive_restoring.contains(&id);
                let error = self.conversations.archive_row_errors.get(&id);
                let title = string(&chat, "name");
                list = list.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_shrink_0()
                        .py_3()
                        .when(i > 0, |d| d.border_t_1().border_color(cx.theme().border))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_4()
                                .child(div().flex_1().min_w_0().text_ellipsis().child(title))
                                .when(error.is_some(), |d| {
                                    d.child(muted("恢复失败", cx).flex_shrink_0())
                                })
                                .child(
                                    Button::new(ElementId::Name(format!("restore-{id}").into()))
                                        .ghost()
                                        .small()
                                        .flex_shrink_0()
                                        .label(if pending {
                                            "恢复中…"
                                        } else if error.is_some() {
                                            "重试"
                                        } else {
                                            "恢复"
                                        })
                                        .disabled(pending)
                                        .when_some(error.cloned(), |b, e| b.tooltip(e))
                                        .on_click(cx.listener(move |s, _, w, cx| {
                                            s.restore_archive(id.clone(), w, cx)
                                        })),
                                ),
                        ),
                );
            }
        }
        let panel = div()
            .id("archive-dialog")
            .occlude()
            .w(px(420.))
            .max_w_full()
            .max_h_full()
            .p_6()
            .rounded(px(12.))
            .bg(cx.theme().background)
            .shadow(vec![design::shadow()])
            .flex()
            .flex_col()
            .gap_4()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("已归档"),
                    )
                    .child(
                        icon_button("close-archive", IconName::X, "关闭已归档")
                            .on_click(cx.listener(|s, _, w, cx| s.close_archive(w, cx))),
                    ),
            )
            .child(
                Input::new(&self.archive_search)
                    .prefix(IconName::Search)
                    .h(px(36.))
                    .aria_label("搜索已归档会话"),
            )
            .child(list);
        div()
            .id("archive-overlay")
            .absolute()
            .inset_0()
            .p_6()
            .occlude()
            .bg(rgba(0x1414140d))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| s.close_archive(w, cx)),
            )
            .track_focus(&self.modal_focus)
            .focus_trap("archive-trap", &self.modal_focus)
            .child(panel)
            .into_any_element()
    }
}
