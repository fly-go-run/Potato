use crate::*;
use gpui_kit::component::{button::*, popover::Popover};
use gpui_kit::prelude::*;

pub fn icon_button(id: &'static str, icon: IconName, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .icon(icon)
        .tooltip(label)
        .accessibility_label(label)
}
pub fn row_button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Button {
    Button::new(ElementId::Name(id.into()))
        .ghost()
        .small()
        .label(label)
        .w_full()
        .justify_start()
        .h(px(34.))
        .font_weight(FontWeight::NORMAL)
        .child(div().flex_1())
}
// The kit's small-button label fixes its own 4px gap; use an explicit row for navigation.
fn sidebar_nav_button(id: &'static str, label: &'static str, icon: IconName) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .w_full()
        .h(px(36.))
        .px_3()
        .accessibility_label(label)
        .child(
            div()
                .flex()
                .items_center()
                .w_full()
                .gap_2()
                .child(Icon::new(icon).size(px(16.)).flex_shrink_0())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(15.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::NORMAL)
                        .child(label),
                ),
        )
}

pub fn muted(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
}
pub fn heading(title: &str, detail: &str, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .child(muted(detail.to_owned(), cx))
}
impl Render for Potato {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if self.page == Page::Chat {
            self.chat_view(window, cx)
        } else {
            self.workspace_view(window, cx)
        };
        let settings = self.settings.open.then(|| self.settings_view(window, cx));
        let edit = self.workspace.editing.then(|| self.editor_view(window, cx));
        let conversation = self
            .conversations
            .editing
            .is_some()
            .then(|| self.conversation_dialog(window, cx));
        let archive = self
            .conversations
            .archive_open
            .then(|| self.archive_dialog(window, cx));
        div()
            .id("potato")
            .key_context("Potato")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_size(px(14.))
            .on_mouse_move(cx.listener(|s, e, w, cx| s.resize_files(e, w, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| s.finish_file_resize(w, cx)),
            )
            .on_action(cx.listener(|s, _: &NewChat, w, cx| s.new_chat(w, cx)))
            .on_action(cx.listener(|s, _: &ShowSendOptions, w, cx| {
                if !s.settings.open && s.menu.is_none() {
                    s.send_hover(true, w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &Settings, w, cx| s.open_settings(w, cx)))
            .on_action(cx.listener(|s, _: &ToggleSidebar, _, cx| {
                if s.conversations.archive_open {
                    return;
                }
                s.sidebar = !s.sidebar;
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &Search, w, cx| {
                s.toggle_search(w, cx);
            }))
            .on_action(cx.listener(|s, _: &Dismiss, w, cx| {
                if s.conversations.archive_open {
                    s.close_archive(w, cx);
                    return;
                }
                if s.outbox.send_hover {
                    s.outbox.send_hover = false;
                    cx.notify();
                    return;
                }
                if s.outbox.menu.take().is_some() {
                    cx.notify();
                    return;
                }
                if let Some(id) = s.outbox.editing.clone() {
                    s.queue_action("cancel_edit", &id, w, cx);
                    return;
                }
                if s.running() && s.menu.is_none() && !s.settings.open && !s.workspace.editing {
                    s.stop(w, cx);
                    return;
                }
                s.menu = None;
                s.conversations.editing = None;
                s.search_open = false;
                s.close_panel(cx);
                if !s.settings.open && !s.workspace.editing {
                    s.focus.focus(w, cx);
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .size_full()
                    .when(self.sidebar, |v| v.child(self.sidebar_view(cx)))
                    .child(div().flex_1().min_w_0().size_full().child(content)),
            )
            .children(settings)
            .children(edit)
            .children(conversation)
            .children(archive)
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
impl Potato {
    fn toggle_search(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.settings.open
            || self.workspace.editing
            || self.conversations.editing.is_some()
            || self.conversations.archive_open
        {
            return;
        }
        self.sidebar = true;
        self.search_open = !self.search_open;
        if self.search_open {
            self.conversation_search.update(cx, |v, cx| v.focus(w, cx));
        } else {
            self.composer.update(cx, |v, cx| v.focus(w, cx));
        }
        cx.notify();
    }
    fn sidebar_view(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = self.conversation_search.read(cx).value().to_lowercase();
        let mut chats = div()
            .id("conversation-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .pb_2()
            .flex()
            .flex_col()
            .gap_1();
        for (i, chat) in self
            .chats
            .iter()
            .filter(|c| c["archived"] != true)
            .filter(|c| !self.search_open || string(c, "name").to_lowercase().contains(&query))
            .enumerate()
        {
            let selected = self.selected.as_deref() == chat["id"].as_str();
            let chat = chat.clone();
            let context_chat = chat.clone();
            chats = chats.child(
                div()
                    .id(("conversation-row", i))
                    .group("conversation-row")
                    .flex()
                    .items_center()
                    .w_full()
                    .rounded(px(8.))
                    .pr_1()
                    .hover(|d| d.bg(cx.theme().accent.opacity(if selected { 1. } else { 0.65 })))
                    .when(selected, |d| d.bg(cx.theme().accent))
                    .child(
                        Button::new(("chat", i))
                            .text()
                            .small()
                            .flex_1()
                            .min_w_0()
                            .h(px(34.))
                            .px_2()
                            .font_weight(FontWeight::NORMAL)
                            .accessibility_label(string(&chat, "name"))
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_left()
                                    .text_ellipsis()
                                    .child(string(&chat, "name")),
                            )
                            .on_click(
                                cx.listener(move |s, _, w, cx| s.select_chat(chat.clone(), w, cx)),
                            ),
                    )
                    .child(
                        Button::new(("chat-options", i))
                            .opacity(if selected { 1. } else { 0. })
                            .group_hover("conversation-row", |b| b.opacity(1.))
                            .focus(|b| b.opacity(1.))
                            .text()
                            .small()
                            .w(px(28.))
                            .h(px(28.))
                            .flex_shrink_0()
                            .icon(IconName::Ellipsis)
                            .text_color(cx.theme().muted_foreground)
                            .tooltip("会话设置")
                            .accessibility_label("会话设置")
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.edit_conversation(context_chat.clone(), w, cx)
                            })),
                    ),
            );
        }
        div()
            .w(px(236.))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().muted)
            .border_r_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .pt_2()
            .child(div().flex().justify_end().h(px(30.)).px_3().child(
                icon_button("collapse", IconName::PanelLeft, "收起侧栏 · ⌘B").on_click(
                    cx.listener(|s, _, _, cx| {
                        s.sidebar = false;
                        cx.notify();
                    }),
                ),
            ))
            .child(
                div()
                    .px_3()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        sidebar_nav_button("new", "新建", IconName::SquarePen)
                            .on_click(cx.listener(|s, _, w, cx| s.new_chat(w, cx))),
                    )
                    .child(
                        sidebar_nav_button("search", "搜索", IconName::Search)
                            .on_click(cx.listener(|s, _, w, cx| s.toggle_search(w, cx))),
                    )
                    .children(
                        [
                            (Page::Tasks, "定时", IconName::Clock),
                            (Page::Skills, "技能", IconName::LayoutGrid),
                            (Page::Memory, "记忆", IconName::Notebook),
                        ]
                        .into_iter()
                        .map(|(page, label, icon)| {
                            sidebar_nav_button(label, label, icon)
                                .when(self.page == page, |b| b.bg(cx.theme().accent))
                                .on_click(cx.listener(move |s, _, w, cx| s.open_page(page, w, cx)))
                        }),
                    ),
            )
            .when(self.search_open, |v| {
                v.child(
                    div().p_3().child(
                        Input::new(&self.conversation_search)
                            .small()
                            .prefix(IconName::Search)
                            .aria_label("搜索会话名称"),
                    ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .pt_6()
                    .pb_2()
                    .group("conversation-heading")
                    .child(muted("会话", cx))
                    .child(self.archive_menu(cx)),
            )
            .child(chats)
            .child(
                div()
                    .flex()
                    .items_center()
                    .p_3()
                    .gap_2()
                    .child(
                        icon_button("settings", IconName::Settings, "设置 · ⌘,")
                            .size(px(32.))
                            .flex_shrink_0()
                            .on_click(cx.listener(|s, _, w, cx| s.open_settings(w, cx))),
                    )
                    .child(div().flex_1().min_w_0().text_center().child("Potato"))
                    .child(
                        icon_button(
                            "theme",
                            if self.dark {
                                IconName::Sun
                            } else {
                                IconName::Moon
                            },
                            "切换主题",
                        )
                        .size(px(32.))
                        .flex_shrink_0()
                        .on_click(cx.listener(|s, _, w, cx| s.toggle_theme(w, cx))),
                    ),
            )
    }
    fn chat_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.sync_side_panel(window, cx);
        let empty = self.history.is_empty() && self.turn.messages.is_empty();
        let composer = self.composer_view(window, cx);
        let header = div()
            .h(px(44.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_5()
            .when(!self.sidebar, |v| {
                v.child(
                    icon_button("expand", IconName::PanelLeft, "展开侧栏").on_click(cx.listener(
                        |s, _, _, cx| {
                            s.sidebar = true;
                            cx.notify();
                        },
                    )),
                )
            })
            .when(!empty, |v| {
                v.child(
                    div().flex_1().text_size(px(13.)).child(
                        self.chats
                            .iter()
                            .find(|c| Some(string(c, "id")) == self.selected)
                            .map(|c| string(c, "name"))
                            .unwrap_or("新会话".into()),
                    ),
                )
            });
        let header = header.when(empty, |d| d.child(div().flex_1())).child(
            icon_button("toggle-files", IconName::PanelRight, "文件与改动")
                .on_click(cx.listener(|s, _, w, cx| s.toggle_files(w, cx))),
        );
        let mut body = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(header);
        if empty {
            body = body.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .px_8()
                    .pb(px(110.))
                    .child(
                        div()
                            .text_size(px(30.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .mb(px(56.))
                            .child(if self.chat.loading {
                                "正在加载会话…"
                            } else if self.chat.load_error {
                                "会话加载失败"
                            } else {
                                greeting()
                            }),
                    )
                    .when(self.chat.load_error, |d| {
                        d.child(
                            Button::new("retry-chat")
                                .outline()
                                .label("重新加载")
                                .mb_4()
                                .on_click(cx.listener(|s, _, w, cx| {
                                    if let Some(chat) = s
                                        .chats
                                        .iter()
                                        .find(|c| c["id"].as_str() == s.selected.as_deref())
                                        .cloned()
                                    {
                                        s.select_chat(chat, w, cx);
                                    }
                                })),
                        )
                    })
                    .child(
                        div()
                            .w_full()
                            .max_w(px(crate::design::CHAT_WIDTH))
                            .child(composer),
                    ),
            );
        } else {
            let mut messages = div()
                .id("messages")
                .track_scroll(&self.chat.scroll)
                .on_scroll_wheel(cx.listener(|s, event: &ScrollWheelEvent, _, cx| {
                    if event.delta.pixel_delta(px(14.)).y > px(0.) {
                        if !s.chat.scroll_paused {
                            s.preserve_process_reading();
                        }
                        s.chat.scroll_paused = true;
                    } else if s.chat.scroll.max_offset().y + s.chat.scroll.offset().y < px(48.) {
                        s.chat.scroll_paused = false;
                    }
                    cx.notify();
                }))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_8()
                .pb_8()
                .flex()
                .flex_col()
                .items_center()
                .gap_6();
            let mut all: Vec<Value> = self
                .history
                .iter()
                .chain(self.turn.messages.iter())
                .cloned()
                .collect();
            // Follow changing layout through disclosure frames as well as text
            // deltas. Reading or opening a detail pauses this behavior.
            if !self.chat.scroll_paused {
                self.chat.scroll.scroll_to_bottom();
            }
            for (i, message) in all.iter_mut().enumerate() {
                let identity = message["id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| i.to_string());
                if let Some(summary) = self
                    .chat
                    .completed_runs
                    .get(&format!("{}:{identity}", self.session))
                {
                    message["_presentation"] = summary.clone();
                }
            }
            self.chat
                .markdown
                .sync(&self.session, &all, !self.streaming, cx);
            let blocks = crate::chat::chat_blocks(&all, self.streaming, &self.turn.status);
            let has_active_process = blocks
                .iter()
                .any(|block| matches!(block, crate::chat::ChatBlock::Process { active: true, .. }));

            if let Some(source) = self.files.locate.take() {
                let index = blocks.iter().position(|block| match block {
                    crate::chat::ChatBlock::Message { index, .. } => *index == source,
                    crate::chat::ChatBlock::Process { rows, .. } => {
                        rows.iter().any(|(i, _)| *i == source)
                    }
                });
                if let Some(index) = index {
                    self.chat.scroll.scroll_to_item(index);
                }
            }
            let current_start = all
                .iter()
                .rposition(|m| {
                    m["role"] == "user"
                        && m["metadata"]["steering_state"].is_null()
                        && m["metadata"]["question_request_id"].is_null()
                })
                .unwrap_or(0);
            let has_terminal_process = blocks.iter().any(|b| {
                matches!(b,
                crate::chat::ChatBlock::Process {index, state, ..}
                    if *index >= current_start && state == &self.turn.status)
            });
            let last_answer = match blocks.last() {
                Some(crate::chat::ChatBlock::Message {
                    index,
                    final_answer: true,
                    ..
                }) => Some(*index),
                _ => None,
            };
            for block in blocks {
                messages = messages.child(match block {
                    crate::chat::ChatBlock::Message {
                        index,
                        message,
                        final_answer,
                    } => self.message_view(
                        index,
                        &message,
                        final_answer,
                        Some(index) == last_answer,
                        cx,
                    ),
                    crate::chat::ChatBlock::Process {
                        index,
                        rows,
                        finished,
                        state,
                        elapsed,
                        answering,
                        ..
                    } => self.process_view(index, &rows, &state, finished, elapsed, answering, cx),
                });
            }
            if self.streaming && !has_active_process {
                messages = messages.child(
                    div()
                        .w_full()
                        .max_w(px(crate::design::CHAT_WIDTH))
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::process::activity_spinner(
                            "reply-wait",
                            cx.theme().muted_foreground,
                        ))
                        .child(
                            if all
                                .iter()
                                .rev()
                                .take_while(|m| m["role"] != "user")
                                .any(crate::chat::answer_text)
                            {
                                "正在回答"
                            } else {
                                "正在思考"
                            },
                        ),
                );
            } else if !self.streaming
                && !has_terminal_process
                && matches!(
                    self.turn.status.as_str(),
                    "failed" | "cancelled" | "incomplete"
                )
            {
                messages = messages.child(muted(
                    if self.turn.status == "cancelled" {
                        "已停止"
                    } else {
                        "已中断"
                    },
                    cx,
                ));
            }
            body = body
                .child(messages)
                .when(
                    self.chat.scroll_paused
                        && self.chat.scroll.max_offset().y + self.chat.scroll.offset().y > px(48.),
                    |d| {
                        d.child(
                            div()
                                .relative()
                                .h_0()
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .child(
                                    icon_button(
                                        "latest-message",
                                        IconName::ArrowDown,
                                        "回到最新消息",
                                    )
                                    .absolute()
                                    .bottom(px(8.))
                                    .size(px(32.))
                                    .rounded_full()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .shadow_sm()
                                    .on_click(cx.listener(
                                        |s, _, _, cx| {
                                            s.chat.scroll_paused = false;
                                            s.chat.scroll.scroll_to_bottom();
                                            cx.notify();
                                        },
                                    )),
                                ),
                        )
                    },
                )
                .child(
                    div().flex().justify_center().px_8().pb_5().child(
                        div()
                            .w_full()
                            .max_w(px(crate::design::CHAT_WIDTH))
                            .child(composer),
                    ),
                );
        }
        div()
            .flex()
            .size_full()
            .child(body)
            .when(self.files.open, |d| {
                d.child(self.file_side_view(window, cx))
            })
            .into_any_element()
    }
    pub(super) fn composer_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let is_home = self.history.is_empty() && self.turn.messages.is_empty();
        let interactions = self.interaction_view(window, cx);
        let queue = self.outbox_view(cx);
        let send_control = self.send_control(cx);
        let project_label = if self.project["is_workspace_default"] == true {
            "默认".into()
        } else {
            self.project["name"].as_str().unwrap_or("默认").to_owned()
        };
        let model_label = self
            .active_model_info()
            .map(|(_, m)| string(m, "name"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                self.model["model"]
                    .as_str()
                    .unwrap_or("选择模型")
                    .to_owned()
            });
        let has_effort = !self.effort_options().is_empty();
        let effort_label = self.effort_label();
        let effort_menu = self.effort_view(window, cx);
        let mut files = div().flex().flex_wrap().gap_2();
        for (i, file) in self.attachments.iter().enumerate() {
            files = files.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(file["type"] == "image", |d| {
                        d.child(crate::media::image_view(file, true))
                    })
                    .child(
                        Button::new(("attachment", i))
                            .small()
                            .outline()
                            .icon(IconName::Paperclip)
                            .label(file["file_name"].as_str().unwrap_or("附件").to_owned())
                            .tooltip("点击移除附件")
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.attachments.remove(i);
                                cx.notify();
                            })),
                    ),
            );
        }
        let project_menu = self.menu_view(Menu::Project, window, cx);
        let model_menu = self.menu_view(Menu::Model, window, cx);
        let permission_menu = self.menu_view(Menu::Permission, window, cx);
        let pop = |kind: Menu,
                   id: &'static str,
                   trigger: Button,
                   content: AnyElement,
                   cx: &Context<Self>| {
            let owner = cx.entity();
            Popover::new(id)
                .anchor(Anchor::BottomLeft)
                .open(self.menu == Some(kind))
                .trigger(trigger.on_click(cx.listener(move |s, _, w, cx| {
                    if kind == Menu::Effort {
                        s.prepare_effort(w, cx);
                    }
                    s.menu = Some(kind);
                    cx.notify();
                })))
                .on_open_change(move |open, _, cx| {
                    owner.update(cx, |s, cx| {
                        if *open {
                            s.menu = Some(kind);
                        } else if s.menu == Some(kind) {
                            s.menu = None;
                        }
                        cx.notify();
                    })
                })
                .child(content)
                .p_1()
                .rounded(px(12.))
        };
        div()
            .flex()
            .flex_col()
            .children(interactions)
            .when(!self.queue_items().is_empty(), |d| d.child(queue))
            .when(self.chat.edit_backup.is_some(), |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_2()
                        .child(muted("编辑后将作为新消息发送", cx))
                        .child(
                            Button::new("cancel-message-edit")
                                .ghost()
                                .small()
                                .label("取消编辑")
                                .on_click(cx.listener(|s, _, w, cx| s.cancel_chat_edit(w, cx))),
                        ),
                )
            })
            .child(
                div()
                    .rounded(px(20.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow(vec![design::shadow()])
                    .px_3()
                    .py_2()
                    .when(!self.attachments.is_empty(), |d| d.child(files))
                    .child(
                        Textarea::new(&self.composer)
                            .appearance(false)
                            .bordered(false)
                            .aria_label("描述任务")
                            .text_size(px(16.))
                            .line_height(px(24.))
                            .when(is_home, |input| input.min_h(px(96.)))
                            .px_1()
                            .py_0(),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mt_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .min_w_0()
                                    .child(
                                        icon_button("attach", IconName::Plus, "添加附件").on_click(
                                            cx.listener(|s, _, w, cx| s.pick_files(w, cx)),
                                        ),
                                    )
                                    .child(pop(
                                        Menu::Project,
                                        "project-menu",
                                        Button::new("project")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Folder)
                                            .max_w(px(140.))
                                            .min_w_0()
                                            .tooltip(project_label.clone())
                                            .label(project_label),
                                        project_menu,
                                        cx,
                                    ))
                                    .child(pop(
                                        Menu::Permission,
                                        "permission-menu",
                                        Button::new("permissions")
                                            .ghost()
                                            .small()
                                            .icon(IconName::ShieldCheck)
                                            .label(crate::interactions::approval_mode_label(
                                                &self.config,
                                            )),
                                        permission_menu,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .ml_auto()
                                    .min_w_0()
                                    .child(pop(
                                        Menu::Model,
                                        "model-menu",
                                        Button::new("model")
                                            .ghost()
                                            .small()
                                            .max_w(px(180.))
                                            .min_w_0()
                                            .disabled(self.streaming || self.effort.saving)
                                            .when(self.menu == Some(Menu::Model), |b| {
                                                b.bg(cx.theme().accent)
                                            })
                                            .tooltip(model_label.clone())
                                            .label(model_label),
                                        model_menu,
                                        cx,
                                    ))
                                    .when(has_effort, |d| {
                                        d.child(pop(
                                            Menu::Effort,
                                            "effort-menu",
                                            Button::new("effort")
                                                .ghost()
                                                .small()
                                                .label(effort_label)
                                                .accessibility_label("选择思考深度")
                                                .disabled(self.streaming || self.effort.saving)
                                                .when(self.menu == Some(Menu::Effort), |b| {
                                                    b.bg(cx.theme().accent)
                                                }),
                                            effort_menu,
                                            cx,
                                        ))
                                    })
                                    .child(
                                        icon_button("voice", IconName::Mic, "语音输入")
                                            .when(self.voice.active, |b| {
                                                b.text_color(cx.theme().danger)
                                            })
                                            .disabled(self.streaming)
                                            .on_click(
                                                cx.listener(|s, _, w, cx| s.toggle_voice(w, cx)),
                                            ),
                                    )
                                    .child(send_control),
                            ),
                    ),
            )
            .when(!self.notice.is_empty(), |d| {
                d.child(muted(self.notice.clone(), cx))
            })
    }
    fn menu_view(&mut self, kind: Menu, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut menu = div().w(px(280.)).flex().flex_col().gap_1();
        match kind {
            Menu::Project => {
                if self.project_creating {
                    let name = self.field("project-name", "", "项目名称", window, cx);
                    menu = menu
                        .child(muted("新建项目", cx))
                        .child(Input::new(&name).small())
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .pt_2()
                                .child(
                                    Button::new("cancel-project")
                                        .ghost()
                                        .small()
                                        .label("返回")
                                        .on_click(cx.listener(|s, _, _, cx| {
                                            s.project_creating = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("create-project")
                                        .primary()
                                        .small()
                                        .label("创建")
                                        .on_click(cx.listener(|s, _, w, cx| {
                                            let name = s.value("project-name", cx);
                                            if name.trim().is_empty() {
                                                return;
                                            }
                                            s.request(
                                                "POST",
                                                "/api/workspace/coding-project/create",
                                                json!({"name":name.trim()}),
                                                w,
                                                cx,
                                                |s, _, w, cx| {
                                                    s.menu = None;
                                                    s.project_creating = false;
                                                    s.refresh(w, cx);
                                                },
                                            );
                                        })),
                                ),
                        );
                } else {
                    let search = self.field("project-search", "", "搜索项目", window, cx);
                    let query = search.read(cx).value().to_lowercase();
                    menu = menu
                        .child(Input::new(&search).small().prefix(IconName::Search))
                        .child(
                            row_button("default-project", "默认")
                                .icon(IconName::Folder)
                                .when(self.project["is_workspace_default"] == true, |b| {
                                    b.bg(cx.theme().accent)
                                })
                                .on_click(
                                    cx.listener(|s, _, w, cx| s.set_project(Value::Null, w, cx)),
                                ),
                        );
                    for (i, p) in self.projects.iter().enumerate().filter(|(_, p)| {
                        query.is_empty() || string(p, "name").to_lowercase().contains(&query)
                    }) {
                        let path = p["path"].clone();
                        menu = menu.child(
                            row_button(format!("project-{i}"), string(p, "name"))
                                .icon(IconName::Folder)
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.set_project(path.clone(), w, cx)
                                })),
                        );
                    }
                    menu = menu
                        .child(div().h(px(1.)).bg(cx.theme().border).my_1())
                        .child(
                            row_button("browse-project", "浏览目录…")
                                .icon(IconName::FolderOpen)
                                .on_click(cx.listener(|_, _, w, cx| {
                                    cx.spawn_in(w, async move |this, cx| {
                                        if let Some(folder) =
                                            rfd::AsyncFileDialog::new().pick_folder().await
                                        {
                                            let _ = this.update_in(cx, |s, w, cx| {
                                                s.set_project(json!(folder.path()), w, cx)
                                            });
                                        }
                                    })
                                    .detach();
                                })),
                        )
                        .child(
                            row_button("new-project", "新建项目")
                                .icon(IconName::Plus)
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.project_creating = true;
                                    cx.notify();
                                })),
                        )
                        .child(
                            row_button("workspace", "角色与工作区")
                                .icon(IconName::FileText)
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.menu = None;
                                    s.open_page(Page::Workspace, w, cx);
                                })),
                        );
                }
            }
            Menu::Model => {
                menu = menu.child(muted("选择模型", cx));
                for (pi, p) in self
                    .providers
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p["is_local"] == true || !string(p, "api_key").is_empty())
                {
                    menu = menu.child(div().px_2().pt_2().child(muted(string(p, "name"), cx)));
                    for (i, m) in p["models"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .chain(p["extra_models"].as_array().into_iter().flatten())
                        .enumerate()
                    {
                        let provider = string(p, "id");
                        let model = string(m, "id");
                        menu = menu.child(
                            Button::new(ElementId::Name(format!("model-{pi}-{i}").into()))
                                .ghost()
                                .small()
                                .w_full()
                                .h(px(34.))
                                .px_2()
                                .accessibility_label(
                                    m["name"].as_str().unwrap_or(&model).to_owned(),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .w_full()
                                        .min_w_0()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_left()
                                                .text_ellipsis()
                                                .child(
                                                    m["name"].as_str().unwrap_or(&model).to_owned(),
                                                ),
                                        )
                                        .when(
                                            m["id"] == self.model["model"]
                                                && p["id"] == self.model["provider_id"],
                                            |d| d.child(Icon::new(IconName::Check).size(px(16.))),
                                        ),
                                )
                                .disabled(self.streaming || self.effort.saving)
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.request(
                                        "PUT",
                                        "/api/models/active",
                                        json!({"provider_id":provider,"model":model}),
                                        w,
                                        cx,
                                        |s, v, _, _| {
                                            s.model = v["active_llm"].clone();
                                            s.menu = None;
                                        },
                                    );
                                })),
                        );
                    }
                }
                menu = menu
                    .child(div().h(px(1.)).bg(cx.theme().border).my_1())
                    .child(
                        row_button("manage-models", "管理模型与服务商")
                            .icon(IconName::Settings)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.menu = None;
                                s.open_settings(w, cx);
                            })),
                    );
            }
            Menu::Conversations | Menu::Effort => {}
            Menu::Permission => {
                menu = menu.child(muted(
                    format!(
                        "文件访问 · {} 个永久目录规则",
                        self.config["directory_rule_count"].as_u64().unwrap_or(0)
                    ),
                    cx,
                ));
                for (_, id, label) in potato_core::permissions::FileMode::OPTIONS
                    .into_iter()
                    .filter(|(_, id, _)| *id != "read-only")
                {
                    menu = menu.child(
                        row_button(id, label)
                            .when(self.config["sandbox_mode"] == id, |b| {
                                b.icon(IconName::Check)
                            })
                            .disabled(self.streaming)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.request(
                                    "PUT",
                                    "/api/workspace/running-config",
                                    json!({"sandbox_mode":id}),
                                    w,
                                    cx,
                                    |s, v, _, _| {
                                        s.config = v;
                                        s.menu = None;
                                    },
                                )
                            })),
                    );
                }
                menu = menu.child(muted(
                    format!(
                        "审批方式 · {}",
                        crate::interactions::approval_mode_label(&self.config)
                    ),
                    cx,
                ));
                for (reviewer, label) in [("model", "自动审批"), ("user", "手动审批")] {
                    menu = menu.child(
                        row_button(format!("reviewer-{reviewer}"), label)
                            .when(
                                self.config["reviewer"].as_str().unwrap_or("model") == reviewer
                                    && self.config["approval_level"].as_str().unwrap_or("AUTO")
                                        == "AUTO",
                                |b| b.icon(IconName::Check),
                            )
                            .disabled(self.busy || self.streaming)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.busy = true;
                                s.request_result(
                                    "PUT",
                                    "/api/workspace/running-config",
                                    json!({"reviewer":reviewer, "approval_level":"AUTO"}),
                                    w,
                                    cx,
                                    |s, result, _, _| {
                                        s.busy = false;
                                        match result {
                                            Ok(config) => {
                                                s.config = config;
                                                s.menu = None;
                                            }
                                            Err(error) => s.notice = error,
                                        }
                                    },
                                );
                            })),
                    );
                }
                menu = menu.child(
                    row_button("permission-settings", "管理目录授权与审批策略").on_click(
                        cx.listener(|s, _, w, cx| {
                            s.menu = None;
                            s.open_settings(w, cx);
                            s.settings.section = 3;
                        }),
                    ),
                );
            }
        }
        div()
            .id("composer-menu-scroll")
            .max_h(px(400.))
            .overflow_y_scroll()
            .child(menu)
            .into_any_element()
    }
    fn set_project(&mut self, path: Value, w: &mut Window, cx: &mut Context<Self>) {
        self.request(
            "PUT",
            "/api/workspace/coding-project",
            json!({"path":path}),
            w,
            cx,
            |s, v, w, cx| {
                s.project = v;
                s.menu = None;
                s.refresh(w, cx);
            },
        );
    }
}
fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=11 => "上午好，从哪件事开始？",
        12..=17 => "下午好，从哪件事开始？",
        _ => "晚上好，从哪件事开始？",
    }
}
pub fn message_text(v: &Value) -> String {
    if let Some(s) = v["content"].as_str() {
        return s.into();
    }
    v["content"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| match b["type"].as_str() {
            Some("text") => string(b, "text"),
            Some("file") => format!("附件：{}", string(b, "file_name")),
            Some("image") => format!(
                "![图片]({})",
                b["image_url"]["url"]
                    .as_str()
                    .or_else(|| b["image_url"].as_str())
                    .or_else(|| b["url"].as_str())
                    .unwrap_or("")
            ),
            Some("data") => format!(
                "**{}**\n\n```\n{}\n```",
                b["data"]["name"].as_str().unwrap_or("工具"),
                b["data"]["output"]
                    .as_str()
                    .or_else(|| b["data"]["arguments"].as_str())
                    .unwrap_or("执行中…")
            ),
            _ => format!("[{}]", b["type"].as_str().unwrap_or("内容")),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
