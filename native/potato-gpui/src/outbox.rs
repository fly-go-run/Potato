use super::*;
use gpui_kit::component::button::*;
use gpui_kit::prelude::*;
#[derive(Default)]
pub(super) struct Outbox {
    pub session: String,
    pub value: Value,
    pub polling: bool,
    pub sending: bool,
    pub expanded: bool,
    pub menu: Option<String>,
    pub editing: Option<String>,
    pub editor: Option<Entity<InputState>>,
    pub send_hover: bool,
    pub hover_epoch: u64,
}
impl Potato {
    pub(super) fn queue_items(&self) -> Vec<Value> {
        if self.outbox.session != self.session {
            return vec![];
        }
        array(self.outbox.value["items"].clone())
    }
    pub(super) fn running(&self) -> bool {
        self.streaming
            || self.outbox.session == self.session && self.outbox.value["running"] == true
    }
    pub(super) fn poll_outbox(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.outbox.polling {
            return;
        }
        self.outbox.polling = true;
        let epoch = self.chat_epoch;
        let session = self.session.clone();
        self.request_result(
            "POST",
            "/api/agent/outbox",
            json!({"session_id":session}),
            w,
            cx,
            move |s, r, w, cx| {
                s.outbox.polling = false;
                if s.session != session || s.chat_epoch != epoch {
                    return;
                }
                if let Ok(v) = r {
                    s.outbox.session = session.clone();
                    let running = v["running"] == true;
                    let id = string(&v, "chat_id");
                    let changed = s.outbox.value["items"] != v["items"]
                        || s.outbox.value["running"] == true
                        || s.outbox.value["recovery_revision"] != v["recovery_revision"];
                    s.outbox.value = v;
                    if (running || changed)
                        && !id.is_empty()
                        && !s.streaming
                        && !s.chat.loading
                        && !s.busy
                    {
                        s.chat.loading = true;
                        s.request_result(
                            "GET",
                            &format!("/api/chats/{}", segment(&id)),
                            Value::Null,
                            w,
                            cx,
                            move |s, r, w, cx| {
                                if s.session != session || s.chat_epoch != epoch {
                                    return;
                                }
                                s.chat.loading = false;
                                if let Ok(v) = r {
                                    s.selected = Some(id);
                                    s.history = array(v["messages"].clone());
                                    s.turn = stream::Turn::default();
                                    if v["status"] != "running" {
                                        s.refresh(w, cx);
                                        return;
                                    }
                                    s.notice.clear();
                                    s.streaming = true;
                                    s.chat.run_started = Some(std::time::Instant::now());
                                    let rx = s
                                        .backend
                                        .stream(json!({"session_id":session,"reconnect":true}));
                                    s.listen_turn(rx, w, cx);
                                }
                            },
                        );
                    }
                }
            },
        );
    }
    pub(super) fn queue_action(
        &mut self,
        action: &str,
        id: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.session.clone();
        let text = self
            .outbox
            .editor
            .as_ref()
            .map(|e| e.read(cx).value().to_string())
            .unwrap_or_default();
        let action = action.to_owned();
        let id = id.to_owned();
        let edit_text = self
            .queue_items()
            .iter()
            .find(|i| i["id"] == id)
            .map(queue_text)
            .unwrap_or_default();
        self.request(
            "POST",
            "/api/agent/outbox",
            json!({"session_id":session,"action":action,"id":id,"text":text}),
            w,
            cx,
            move |s, v, w, cx| {
                if s.session != session {
                    return;
                }
                s.outbox.session = session;
                s.outbox.value = v;
                s.outbox.menu = None;
                if action == "edit" {
                    s.outbox.editing = Some(id);
                    let editor = cx.new(|cx| InputState::new(w, cx).default_value(edit_text));
                    s.outbox.editor = Some(editor);
                } else if matches!(action.as_str(), "save" | "cancel_edit" | "delete") {
                    s.outbox.editing = None;
                    s.outbox.editor = None;
                }
            },
        );
    }
    pub(super) fn enqueue(&mut self, immediate: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.outbox.sending {
            return;
        }
        let session = self.session.clone();
        let text = self.composer.read(cx).value().to_string();
        let attachments = self.attachments.clone();
        let request = backend::request_body(
            &session,
            &self.user,
            &self.channel,
            &text,
            attachments.clone(),
        );
        self.outbox.sending = true;
        self.outbox.send_hover = false;
        self.request_result("POST","/api/agent/outbox",json!({"session_id":session,"action":"add","id":uuid::Uuid::new_v4().to_string(),"request":request,"immediate":immediate}),w,cx,move |s,r,w,cx| {
            s.outbox.sending=false;
            match r {
                Ok(v) if s.session==session => {
                    s.outbox.session=session;s.outbox.value=v;
                    if s.composer.read(cx).value().as_ref()==text {
                        s.composer.update(cx,|e,cx|e.set_value("",w,cx));
                    }
                    if s.attachments==attachments {s.attachments.clear();}
                    s.notice.clear();
                },
                Ok(_)=>{},Err(e)=>s.notice=e,
            }
        });
    }
    pub(super) fn send_hover(&mut self, over: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.outbox.hover_epoch += 1;
        let epoch = self.outbox.hover_epoch;
        if over {
            self.outbox.send_hover = true;
            cx.notify();
            return;
        }
        cx.spawn_in(w, async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(180))
                .await;
            let _ = this.update_in(cx, |s, _, cx| {
                if s.outbox.hover_epoch == epoch {
                    s.outbox.send_hover = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
pub(super) fn queue_text(item: &Value) -> String {
    item["request"]["input"][0]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}
impl Potato {
    pub(super) fn outbox_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let items = self.queue_items();
        let mut rows = div()
            .id("follow-up-queue")
            .debug_selector(|| "outbox-shelf".into())
            .mx_3()
            .rounded_t(px(16.))
            .bg(cx.theme().muted.opacity(0.35))
            .border_1()
            .border_color(cx.theme().border)
            .overflow_y_scroll()
            .max_h(px(180.));
        for item in items
            .iter()
            .take(if self.outbox.expanded { 100 } else { 3 })
        {
            let id = string(item, "id");
            let mut row = div()
                .relative()
                .flex()
                .items_center()
                .h(px(40.))
                .px_3()
                .gap_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    Icon::new(IconName::ListEnd)
                        .size(px(16.))
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_ellipsis()
                        .child(outbox::queue_text(item)),
                );
            let promote = id.clone();
            let delete = id.clone();
            let menu = id.clone();
            row = row
                .child(
                    Button::new(SharedString::from(format!("queue-now-{id}")))
                        .ghost()
                        .small()
                        .label("立即发送")
                        .text_color(cx.theme().muted_foreground)
                        .rounded(px(10.))
                        .disabled(
                            item["state"] != "pending" || self.outbox.value["interrupt"] == true,
                        )
                        .tooltip("停止当前回复并发送这条消息")
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.queue_action("promote", &promote, w, cx)
                        })),
                )
                .child(
                    Button::new(SharedString::from(format!("queue-delete-{id}")))
                        .ghost()
                        .small()
                        .icon(IconName::Trash)
                        .size(px(30.))
                        .rounded(px(10.))
                        .text_color(cx.theme().muted_foreground)
                        .tooltip("删除待发送消息")
                        .accessibility_label("删除待发送消息")
                        .on_click(
                            cx.listener(move |s, _, w, cx| {
                                s.queue_action("delete", &delete, w, cx)
                            }),
                        ),
                )
                .child(
                    Button::new(SharedString::from(format!("queue-more-{id}")))
                        .ghost()
                        .small()
                        .icon(IconName::Ellipsis)
                        .size(px(30.))
                        .rounded(px(10.))
                        .text_color(cx.theme().muted_foreground)
                        .when(self.outbox.menu.as_ref() == Some(&id), |b| {
                            b.bg(cx.theme().muted)
                        })
                        .tooltip("编辑或调整顺序")
                        .accessibility_label("编辑或调整顺序")
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.outbox.menu = if s.outbox.menu.as_ref() == Some(&menu) {
                                None
                            } else {
                                Some(menu.clone())
                            };
                            cx.notify();
                        })),
                );
            if self.outbox.menu.as_ref() == Some(&id) {
                let mut menu = div()
                    .id(SharedString::from(format!("queue-menu-{id}")))
                    .debug_selector(|| "queue-action-menu".into())
                    .absolute()
                    .top(px(36.))
                    .right(px(8.))
                    .w(px(216.))
                    .p_2()
                    .rounded(px(18.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow(vec![design::shadow()])
                    .flex()
                    .flex_col();
                for (action, label, icon) in [
                    ("edit", "编辑消息", IconName::SquarePen),
                    ("up", "上移", IconName::ArrowUp),
                    ("down", "下移", IconName::ArrowDown),
                ] {
                    let id = id.clone();
                    menu = menu.child(
                        Button::new(SharedString::from(format!("{action}-{id}")))
                            .ghost()
                            .w_full()
                            .h(px(38.))
                            .rounded(px(10.))
                            .justify_start()
                            .icon(icon)
                            .label(label)
                            .on_click(
                                cx.listener(move |s, _, w, cx| s.queue_action(action, &id, w, cx)),
                            ),
                    );
                }
                row = row.child(deferred(menu));
            }
            rows = rows.child(row);
            if self.outbox.editing.as_ref() == Some(&id)
                && let Some(editor) = &self.outbox.editor
            {
                let save = id.clone();
                let cancel = id.clone();
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .px_2()
                        .gap_1()
                        .child(Input::new(editor).flex_1())
                        .child(
                            Button::new("save-queued-edit")
                                .small()
                                .label("保存")
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.queue_action("save", &save, w, cx)
                                })),
                        )
                        .child(
                            Button::new("cancel-queued-edit")
                                .small()
                                .ghost()
                                .label("取消")
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.queue_action("cancel_edit", &cancel, w, cx)
                                })),
                        ),
                );
            }
        }
        if items.len() > 3 {
            rows = rows.child(
                Button::new("expand-queue")
                    .ghost()
                    .small()
                    .label(if self.outbox.expanded {
                        "收起".into()
                    } else {
                        format!("还有 {} 条", items.len() - 3)
                    })
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.outbox.expanded = !s.outbox.expanded;
                        cx.notify();
                    })),
            );
        }
        if self.outbox.value["paused"] == true && !items.is_empty() {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .px_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .child(string(&self.outbox.value, "reason")),
                    )
                    .child(
                        Button::new("resume-queue")
                            .ghost()
                            .small()
                            .label("继续发送")
                            .on_click(
                                cx.listener(|s, _, w, cx| s.queue_action("resume", "", w, cx)),
                            ),
                    ),
            );
        } else if self.outbox.value["interrupt"] == true {
            rows = rows.child(div().px_2().child("正在停止，随后发送…"));
        }
        rows.into_any_element()
    }
    pub(super) fn send_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let draft =
            !self.composer.read(cx).value().trim().is_empty() || !self.attachments.is_empty();
        let stop = self.running() && !draft;
        let disabled = self.busy
            || self.outbox.sending
            || self.voice.active
            || self.chat.loading
            || self.chat.load_error
            || (!stop && !draft);
        let mut control = div()
            .id("send-hover-region")
            .debug_selector(|| "send-region".into())
            .relative()
            .flex_shrink_0()
            .on_hover(cx.listener(|s, over, w, cx| s.send_hover(*over, w, cx)))
            .child(
                Button::new("send")
                    .primary()
                    .rounded_full()
                    .size(px(34.))
                    .icon(if stop {
                        IconName::Square
                    } else {
                        IconName::ArrowUp
                    })
                    .accessibility_label(if stop { "停止生成" } else { "发送消息" })
                    .disabled(disabled)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.outbox.send_hover = false;
                        if stop { s.stop(w, cx) } else { s.send(w, cx) }
                    })),
            );
        if self.outbox.send_hover && self.running() && draft && !disabled {
            control = control.child(deferred(
                div()
                    .id("send-hover-menu")
                    .debug_selector(|| "send-menu".into())
                    .absolute()
                    .bottom(px(34.))
                    .right_0()
                    .pb_2()
                    .on_hover(cx.listener(|s, over, w, cx| s.send_hover(*over, w, cx)))
                    .child(
                        div()
                            .w(px(208.))
                            .p_2()
                            .rounded(px(18.))
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow(vec![design::shadow()])
                            .flex()
                            .flex_col()
                            .child(
                                Button::new("queue-draft")
                                    .ghost()
                                    .h(px(36.))
                                    .w_full()
                                    .rounded(px(10.))
                                    .justify_between()
                                    .child(div().child("加入队列"))
                                    .child(div().text_color(cx.theme().muted_foreground).child("↩"))
                                    .accessibility_label("加入队列")
                                    .on_click(cx.listener(|s, _, w, cx| s.send_mode(false, w, cx))),
                            )
                            .child(
                                Button::new("interrupt-draft")
                                    .ghost()
                                    .h(px(36.))
                                    .w_full()
                                    .rounded(px(10.))
                                    .justify_between()
                                    .child(div().child("打断并发送"))
                                    .child(
                                        div().text_color(cx.theme().muted_foreground).child("⌘↩"),
                                    )
                                    .accessibility_label("打断并发送")
                                    .on_click(cx.listener(|s, _, w, cx| s.send_mode(true, w, cx))),
                            ),
                    ),
            ));
        }
        control.into_any_element()
    }
}
