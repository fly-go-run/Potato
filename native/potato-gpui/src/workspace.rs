use crate::view::{heading, icon_button, muted};
use crate::*;
use gpui_kit::base::FocusTrapElement;
use gpui_kit::component::{button::*, switch::Switch};
use gpui_kit::prelude::*;
#[derive(Default)]
pub struct WorkspaceState {
    pub list: Vec<Value>,
    pub editing: bool,
    pub editor_generation: u64,
    pub editor_loading: bool,
    pub editor_saving: bool,
    pub editor_load_error: bool,
    pub selected: Option<String>,
    pub expected: Value,
    pub original: String,
    pub original_name: String,
    pub confirm: bool,
    pub discard: bool,
    pub task: Value,
    pub tab: String,
    pub category: String,
    pub loading: bool,
    pub history: Vec<Value>,
    pub schedule_baseline: BTreeMap<String, String>,
}
impl Page {
    pub fn path(self) -> &'static str {
        match self {
            Self::Tasks => "/api/cron/jobs",
            Self::Skills => "/api/skills",
            Self::Memory => "/api/workspace/memory",
            _ => "/api/workspace/files",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Tasks => "定时任务",
            Self::Skills => "技能与插件",
            Self::Memory => "记忆",
            _ => "角色与工作区",
        }
    }
    fn subtitle(self) -> &'static str {
        match self {
            Self::Tasks => "让重复的事情，按时自动完成。",
            Self::Skills => "管理 agent 可调用的工作技能与插件扩展。",
            Self::Memory => "查看和管理助手记住的信息。",
            _ => "定义助手的角色、偏好与工作方式。",
        }
    }
}
impl Potato {
    pub fn open_page(&mut self, page: Page, w: &mut Window, cx: &mut Context<Self>) {
        if self.settings.open
            || self.conversations.editing.is_some()
            || self.conversations.archive_open
        {
            return;
        }
        if self.workspace.editing {
            self.notice = "请先保存或关闭编辑器".into();
            return;
        }
        self.page = page;
        self.menu = None;
        self.notice.clear();
        self.workspace.tab = String::new();
        self.workspace.category = String::new();
        self.search.update(cx, |v, cx| v.set_value("", w, cx));
        self.load_page(w, cx);
    }
    fn load_page(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.epoch += 1;
        let epoch = self.epoch;
        self.workspace.loading = true;
        self.workspace.list.clear();
        let path = if self.page == Page::Skills && self.workspace.tab == "plugins" {
            "/api/plugins"
        } else {
            self.page.path()
        };
        self.request_result("GET", path, Value::Null, w, cx, move |s, result, _, _| {
            if s.epoch == epoch {
                s.workspace.loading = false;
                match result {
                    Ok(v) => s.workspace.list = array(v),
                    Err(error) => s.notice = error,
                }
            }
        });
    }
    fn edit_item(&mut self, item: Option<Value>, w: &mut Window, cx: &mut Context<Self>) {
        if self.workspace.editor_saving {
            return;
        }
        self.workspace.editor_generation += 1;
        let generation = self.workspace.editor_generation;
        self.workspace.editor_loading = item.is_some();
        self.workspace.editor_load_error = false;
        self.fields.retain(|k, _| !k.starts_with("document-"));
        self.notice.clear();
        self.workspace.confirm = false;
        self.workspace.discard = false;
        self.workspace.original = String::new();
        self.workspace.original_name = String::new();
        self.workspace.expected = Value::Null;
        self.workspace.task = Value::Null;
        self.workspace.history.clear();
        self.workspace.schedule_baseline.clear();
        self.workspace.selected = None;
        self.workspace.editing = true;
        self.modal_focus.focus(w, cx);
        self.editor.update(cx, |v, cx| v.set_value("", w, cx));
        if let Some(item) = item {
            let id = if self.page == Page::Tasks {
                string(&item, "id")
            } else {
                item["filename"]
                    .as_str()
                    .or_else(|| item["name"].as_str())
                    .unwrap_or("")
                    .into()
            };
            self.workspace.selected = Some(id.clone());
            let page = self.page;
            if page == Page::Tasks {
                let history_id = id.clone();
                self.request(
                    "GET",
                    &format!("/api/cron/jobs/{}/history", segment(&id)),
                    Value::Null,
                    w,
                    cx,
                    move |s, v, _, _| {
                        if s.workspace.editor_generation == generation
                            && s.workspace.editing
                            && s.workspace.selected.as_deref() == Some(&history_id)
                        {
                            s.workspace.history = array(v);
                        }
                    },
                );
            }
            self.request_result(
                "GET",
                &item_path(page, &id),
                Value::Null,
                w,
                cx,
                move |s, result, w, cx| {
                    if s.workspace.editor_generation != generation || !s.workspace.editing {
                        return;
                    }
                    s.workspace.editor_loading = false;
                    let v = match result {
                        Ok(value) => value,
                        Err(error) => {
                            s.workspace.editor_load_error = true;
                            s.notice = error;
                            return;
                        }
                    };
                    s.workspace.schedule_baseline.clear();
                    let text = if page == Page::Tasks {
                        if v["task_type"] == "text" {
                            string(&v, "text")
                        } else {
                            v["request"]["input"][0]["content"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|c| c["text"].as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    } else {
                        string(&v, "content")
                    };
                    let name = if page == Page::Tasks {
                        string(&v, "name")
                    } else {
                        id
                    };
                    s.workspace.original = text.clone();
                    s.workspace.original_name = name.clone();
                    s.workspace.expected = v["content"].clone();

                    s.workspace.task = v;
                    s.fields.retain(|k, _| !k.starts_with("document-"));
                    s.field("document-name", &name, "名称", w, cx);
                    s.editor.update(cx, |v, cx| v.set_value(text, w, cx));
                },
            );
        } else {
            self.field(
                "document-name",
                "",
                if self.page == Page::Tasks {
                    "任务名称"
                } else {
                    "例如 notes.md"
                },
                w,
                cx,
            );
        }
        cx.notify();
    }
    fn document_dirty(&self, cx: &App) -> bool {
        self.workspace
            .schedule_baseline
            .iter()
            .any(|(k, v)| self.value(k, cx) != *v)
            || self.editor.read(cx).value().as_ref() != self.workspace.original
            || self.value("document-name", cx) != self.workspace.original_name
    }
    pub fn close_editor(&mut self, cx: &mut Context<Self>) {
        if self.workspace.editor_saving {
            return;
        }
        if !self.workspace.editor_loading
            && !self.workspace.editor_load_error
            && self.document_dirty(cx)
        {
            self.workspace.discard = true;
        } else {
            self.workspace.editing = false;
        }
        cx.notify();
    }
    pub fn workspace_view(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut title = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_6()
            .child(heading(self.page.title(), self.page.subtitle(), cx));
        let empty_tasks = self.page == Page::Tasks && self.workspace.list.is_empty();
        if !empty_tasks {
            title = title.child(
                Button::new("new-item")
                    .primary()
                    .small()
                    .icon(IconName::Plus)
                    .label(if self.page == Page::Tasks {
                        "新建任务"
                    } else if self.page == Page::Skills && self.workspace.tab == "plugins" {
                        if self.workspace.list.iter().any(|p| p["id"] == "gpt-image2") {
                            "已安装图片生成"
                        } else {
                            "安装图片生成"
                        }
                    } else {
                        "添加"
                    })
                    .h(px(32.))
                    .disabled(
                        self.page == Page::Skills
                            && self.workspace.tab == "plugins"
                            && self.workspace.list.iter().any(|p| p["id"] == "gpt-image2"),
                    )
                    .on_click(cx.listener(|s, _, w, cx| {
                        if s.page == Page::Skills && s.workspace.tab == "plugins" {
                            s.request(
                                "POST",
                                "/api/plugins/install",
                                json!({"source":"builtin:gpt-image2"}),
                                w,
                                cx,
                                |s, _, w, cx| s.load_page(w, cx),
                            );
                        } else {
                            s.edit_item(None, w, cx);
                        }
                    })),
            );
        }
        let mut tabs = div().flex().gap_2().items_center();
        if self.page == Page::Skills {
            for (id, label) in [("", "技能"), ("plugins", "插件")] {
                tabs = tabs.child(
                    Button::new(label)
                        .ghost()
                        .small()
                        .label(label)
                        .when(self.workspace.tab == id, |b| b.bg(cx.theme().muted))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.workspace.tab = id.into();
                            s.load_page(w, cx);
                        })),
                );
            }
        } else if self.page == Page::Memory {
            for label in ["全部", "日记", "知识", "流程", "其它"] {
                let category = self.workspace.category.clone();
                tabs = tabs.child(
                    Button::new(label)
                        .ghost()
                        .small()
                        .label(label)
                        .when(
                            category == label || category.is_empty() && label == "全部",
                            |b| b.bg(cx.theme().muted),
                        )
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.workspace.category = label.into();
                            cx.notify();
                        })),
                );
            }
        }
        tabs = tabs.child(div().flex_1()).child(
            div().w(px(280.)).child(
                Input::new(&self.search)
                    .small()
                    .prefix(IconName::Search)
                    .cleanable(true),
            ),
        );
        let query = self.search.read(cx).value().to_lowercase();
        let mut list = div().flex_1().min_w_0().flex().flex_col();
        let mut count = 0;
        let mut entries = self.workspace.list.clone();
        if self.page == Page::Memory {
            entries.sort_by_key(|v| std::cmp::Reverse(v["modified_time"].as_i64().unwrap_or(0)));
        }
        for (i, item) in entries.iter().enumerate() {
            if !query.is_empty()
                && !["name", "filename", "description", "tags"]
                    .iter()
                    .any(|k| item[*k].to_string().to_lowercase().contains(&query))
            {
                continue;
            }
            let name = item["name"]
                .as_str()
                .or_else(|| item["filename"].as_str())
                .unwrap_or("未命名")
                .to_owned();
            if self.page == Page::Memory
                && !self.workspace.category.is_empty()
                && self.workspace.category != "全部"
                && memory_category(&name) != self.workspace.category
            {
                continue;
            }
            count += 1;
            let selected = item.clone();
            let id = string(item, "id");
            let description = if self.page == Page::Tasks {
                if item["schedule"]["type"] == "once" {
                    string(&item["schedule"], "run_at")
                } else {
                    format!(
                        "{} · {}",
                        string(&item["schedule"], "cron"),
                        item["schedule"]["timezone"]
                            .as_str()
                            .unwrap_or("Asia/Shanghai")
                    )
                }
            } else {
                string(item, "description")
            };
            let mut item_row = div()
                .flex()
                .items_center()
                .gap_3()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .size(px(32.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_lg()
                        .bg(cx.theme().muted)
                        .child(if self.page == Page::Tasks {
                            IconName::Clock
                        } else if self.page == Page::Skills {
                            IconName::Blocks
                        } else {
                            IconName::FileText
                        }),
                )
                .child(
                    Button::new(("item", i))
                        .accessibility_label(name.clone())
                        .ghost()
                        .flex_1()
                        .h_auto()
                        .justify_start()
                        .py_2()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_start()
                                .gap_1()
                                .child(name.clone())
                                .when(!description.is_empty(), |d| d.child(muted(description, cx))),
                        )
                        .child(div().flex_1())
                        .on_click(cx.listener(move |s, _, w, cx| {
                            if s.page == Page::Skills && s.workspace.tab == "plugins" {
                                s.notice = format!(
                                    "{} · {}",
                                    string(&selected, "name"),
                                    if selected["loaded"] == true {
                                        "已加载"
                                    } else {
                                        "请在设置 → 能力中配置图片模型"
                                    }
                                );
                                cx.notify();
                            } else {
                                s.edit_item(Some(selected.clone()), w, cx)
                            }
                        })),
                );
            if self.page == Page::Skills && self.workspace.tab != "plugins" {
                let name = name.clone();
                item_row = item_row.child(
                    Switch::new(("enabled", i))
                        .small()
                        .checked(item["enabled"] == true)
                        .accessibility_label(format!("启用 {name}"))
                        .on_click(cx.listener(move |s, on, w, cx| {
                            s.request(
                                "POST",
                                &format!(
                                    "/api/skills/{}/{}",
                                    segment(&name),
                                    if *on { "enable" } else { "disable" }
                                ),
                                Value::Null,
                                w,
                                cx,
                                |s, _, w, cx| s.load_page(w, cx),
                            );
                        })),
                );
            }
            if self.page == Page::Tasks {
                let task_id = id.clone();
                item_row = item_row
                    .child(
                        Button::new(("run", i))
                            .ghost()
                            .small()
                            .icon(IconName::Play)
                            .tooltip("立即运行")
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.request(
                                    "POST",
                                    &format!("/api/cron/jobs/{}/run", segment(&task_id)),
                                    Value::Null,
                                    w,
                                    cx,
                                    |s, _, _, _| s.notice = "任务已启动".into(),
                                )
                            })),
                    )
                    .child(
                        Switch::new(("task-enabled", i))
                            .small()
                            .checked(item["enabled"] == true)
                            .accessibility_label(format!("启用 {name}"))
                            .on_click(cx.listener(move |s, on, w, cx| {
                                s.request(
                                    "POST",
                                    &format!(
                                        "/api/cron/jobs/{}/{}",
                                        segment(&id),
                                        if *on { "resume" } else { "pause" }
                                    ),
                                    Value::Null,
                                    w,
                                    cx,
                                    |s, _, w, cx| s.load_page(w, cx),
                                )
                            })),
                    );
            }
            list = list.child(item_row);
        }
        if count == 0 {
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .py(px(if self.page == Page::Tasks { 40. } else { 90. }))
                    .child(
                        Icon::new(if self.page == Page::Tasks {
                            IconName::Clock
                        } else {
                            IconName::Files
                        })
                        .size(px(30.))
                        .text_color(cx.theme().muted_foreground),
                    )
                    .child(if self.workspace.loading {
                        "正在加载…"
                    } else if !query.is_empty() {
                        "没有找到匹配内容"
                    } else if self.page == Page::Tasks {
                        "还没有定时任务"
                    } else {
                        "这里还没有内容"
                    })
                    .child(muted(
                        if self.page == Page::Tasks {
                            "从下方选择模板，或新建一个任务。"
                        } else {
                            "添加内容后，它们会显示在这里。"
                        },
                        cx,
                    ))
                    .when(empty_tasks && !self.workspace.loading, |d| {
                        d.child(
                            Button::new("empty-new-task")
                                .primary()
                                .icon(IconName::Plus)
                                .label("新建任务")
                                .mt_2()
                                .on_click(cx.listener(|s, _, w, cx| s.edit_item(None, w, cx))),
                        )
                    }),
            );
        }
        let mut columns = div().flex().flex_col().gap_7().child(list);
        if self.page == Page::Tasks {
            let available =
                (f32::from(w.viewport_size().width) - if self.sidebar { 236. } else { 0. } - 64.)
                    .min(1040.);
            let column_count = if available >= 840. {
                3
            } else if available >= 560. {
                2
            } else {
                1
            };
            let card_width = (available - 12. * (column_count - 1) as f32) / column_count as f32;
            let mut templates = div()
                .flex()
                .flex_col()
                .gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("任务模板"))
                .child(muted("选择模板后，可修改内容和时间。", cx));
            let icons = [
                IconName::FileText,
                IconName::Clock,
                IconName::Files,
                IconName::Check,
                IconName::Notebook,
                IconName::Database,
                IconName::Notebook,
                IconName::Folder,
            ];
            for (row_index, chunk) in TEMPLATES.chunks(column_count).enumerate() {
                let mut row = div().flex().gap_3().w_full();
                for (offset, (name, cron, prompt)) in chunk.iter().enumerate() {
                    let i = row_index * column_count + offset;
                    row = row.child(Button::new(("template", i))
                        .accessibility_label(*name).outline().w(px(card_width)).flex_shrink_0().min_w_0()
                        .h(px(112.)).justify_start().p_4()
                        .child(div().flex().items_start().gap_3().w_full().min_w_0()
                            .child(Icon::new(icons[i]).size(px(18.)).flex_shrink_0().text_color(cx.theme().muted_foreground))
                            .child(div().flex_1().min_w_0().flex().flex_col().items_start().gap_2()
                                .child(div().font_weight(FontWeight::MEDIUM).child(*name))
                                .child(div().w_full().text_ellipsis().text_sm().text_color(cx.theme().muted_foreground).child(*prompt))
                                .child(muted(template_schedule(cron), cx))))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.edit_item(None, w, cx);
                            s.workspace.task = json!({"task_type":"agent","schedule":{"type":"cron","cron":cron,"timezone":"Asia/Shanghai"}});
                            if let Some(f) = s.fields.get("document-name") {
                                f.update(cx, |v, cx| v.set_value(*name, w, cx));
                            }
                            s.editor.update(cx, |v, cx| v.set_value(*prompt, w, cx));
                            cx.notify();
                        })));
                }
                for _ in chunk.len()..column_count {
                    row = row.child(div().flex_1().min_w_0());
                }
                templates = templates.child(row);
            }
            columns = columns.child(templates);
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .px_5()
                    .when(!self.sidebar, |d| {
                        d.child(
                            icon_button(
                                "expand-workspace-sidebar",
                                IconName::PanelLeft,
                                "展开侧栏",
                            )
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.sidebar = true;
                                cx.notify();
                            })),
                        )
                    }),
            )
            .child(
                div()
                    .id("workspace-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_8()
                    .pb_8()
                    .child(
                        div()
                            .max_w(px(1040.))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_7()
                            .child(title)
                            .when(!empty_tasks, |d| d.child(tabs))
                            .child(columns)
                            .when(!self.notice.is_empty(), |d| {
                                d.child(muted(self.notice.clone(), cx))
                            }),
                    ),
            )
            .into_any_element()
    }
    pub fn editor_view(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let name = self.field(
            "document-name",
            &self.workspace.original_name.clone(),
            "名称",
            w,
            cx,
        );
        let mut body =
            div().flex().flex_col().gap_4().child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(div().flex_1().child(
                        Input::new(&name).aria_label("名称").readonly(
                            self.workspace.selected.is_some() && self.page != Page::Tasks,
                        ),
                    ))
                    .child(
                        icon_button("close-editor", IconName::X, "关闭编辑")
                            .on_click(cx.listener(|s, _, _, cx| s.close_editor(cx))),
                    ),
            );
        if self.page == Page::Tasks {
            let schedule = self.workspace.task["schedule"].clone();
            if schedule["type"] == "once" {
                let initial = string(&schedule, "run_at");
                self.workspace
                    .schedule_baseline
                    .entry("document-run-at".into())
                    .or_insert(initial.clone());
                let time = self.field("document-run-at", &initial, "RFC 3339 时间", w, cx);
                body = body
                    .child(muted("单次执行时间（包含时区）", cx))
                    .child(Input::new(&time).aria_label("单次执行时间"));
            } else {
                for (key, value) in [
                    (
                        "document-cron",
                        schedule["cron"].as_str().unwrap_or("0 9 * * *"),
                    ),
                    (
                        "document-zone",
                        schedule["timezone"].as_str().unwrap_or("Asia/Shanghai"),
                    ),
                ] {
                    self.workspace
                        .schedule_baseline
                        .entry(key.into())
                        .or_insert(value.into());
                }
                let cron = self.field(
                    "document-cron",
                    schedule["cron"].as_str().unwrap_or("0 9 * * *"),
                    "Cron 表达式",
                    w,
                    cx,
                );
                let zone = self.field(
                    "document-zone",
                    schedule["timezone"].as_str().unwrap_or("Asia/Shanghai"),
                    "时区",
                    w,
                    cx,
                );
                body = body.child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            div()
                                .flex_1()
                                .child(muted("执行时间（Cron）", cx))
                                .child(Input::new(&cron).aria_label("Cron 表达式")),
                        )
                        .child(
                            div()
                                .w(px(180.))
                                .child(muted("时区", cx))
                                .child(Input::new(&zone).aria_label("时区")),
                        ),
                );
            }
        }
        body = body.child(Textarea::new(&self.editor).h(px(330.)).aria_label("内容"));
        if !self.notice.is_empty() {
            body = body.child(muted(self.notice.clone(), cx));
        }
        if self.workspace.discard {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child("放弃尚未保存的修改？")
                    .child(
                        Button::new("continue-edit")
                            .outline()
                            .small()
                            .label("继续编辑")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.workspace.discard = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("discard-document")
                            .danger()
                            .small()
                            .label("放弃")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.workspace.editing = false;
                                s.workspace.discard = false;
                                cx.notify();
                            })),
                    ),
            );
        }
        if self.workspace.confirm {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child("确认删除此项？")
                    .child(
                        Button::new("cancel-document-delete")
                            .outline()
                            .small()
                            .label("取消")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.workspace.confirm = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("confirm-document-delete")
                            .danger()
                            .small()
                            .label("删除")
                            .on_click(cx.listener(|s, _, w, cx| {
                                if s.workspace.editor_saving || s.workspace.editor_loading {
                                    return;
                                }
                                if let Some(id) = s.workspace.selected.clone() {
                                    s.workspace.editor_saving = true;
                                    let generation = s.workspace.editor_generation;
                                    s.request_result(
                                        "DELETE",
                                        item_path(s.page, &id).trim_end_matches("/content"),
                                        Value::Null,
                                        w,
                                        cx,
                                        move |s, result, w, cx| {
                                            if s.workspace.editor_generation != generation {
                                                return;
                                            }
                                            s.workspace.editor_saving = false;
                                            match result {
                                                Ok(_) => {
                                                    s.workspace.editing = false;
                                                    s.load_page(w, cx);
                                                }
                                                Err(error) => s.notice = error,
                                            }
                                        },
                                    );
                                }
                            })),
                    ),
            );
        }
        body = body.child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    Button::new("delete-document")
                        .ghost()
                        .small()
                        .label("删除")
                        .text_color(cx.theme().danger)
                        .disabled(self.workspace.selected.is_none())
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.workspace.confirm = true;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("save-document")
                        .primary()
                        .small()
                        .label("保存")
                        .disabled(self.busy)
                        .on_click(cx.listener(|s, _, w, cx| s.save_document(w, cx))),
                ),
        );
        if !self.workspace.history.is_empty() {
            body = body.child(muted("最近执行", cx)).children(
                self.workspace.history.iter().take(5).map(|v| {
                    muted(
                        format!("{} · {}", string(v, "run_at"), string(v, "status")),
                        cx,
                    )
                }),
            );
        }
        if self.workspace.editor_loading
            || self.workspace.editor_saving
            || self.workspace.editor_load_error
        {
            body = div().flex().flex_col().gap_4().child(muted(
                if self.workspace.editor_loading {
                    "正在加载…".to_owned()
                } else if self.workspace.editor_saving {
                    "正在处理…".to_owned()
                } else {
                    format!("加载失败：{}。请关闭后重试。", self.notice)
                },
                cx,
            ));
            if !self.workspace.editor_saving {
                body = body.child(
                    Button::new("close-editor-status")
                        .outline()
                        .label("关闭")
                        .on_click(cx.listener(|s, _, _, cx| s.close_editor(cx))),
                );
            }
        }
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x14141448))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("document-editor")
                    .occlude()
                    .tab_group()
                    .w(px(800.))
                    .max_w_full()
                    .max_h_full()
                    .overflow_y_scroll()
                    .p_6()
                    .rounded(px(16.))
                    .bg(cx.theme().background)
                    .shadow(vec![design::shadow()])
                    .child(body),
            )
            .track_focus(&self.modal_focus)
            .focus_trap("editor-trap", &self.modal_focus)
            .into_any_element()
    }
    fn save_document(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy
            || self.workspace.editor_loading
            || self.workspace.editor_saving
            || self.workspace.editor_load_error
        {
            return;
        }
        let name = self.value("document-name", cx).trim().to_owned();
        if name.is_empty() {
            self.notice = "请填写名称".into();
            return;
        }
        let content = self.editor.read(cx).value().to_string();
        let selected = self.workspace.selected.clone();
        let page = self.page;
        let (method, path, body) = if page == Page::Tasks {
            let mut spec = if self.workspace.task.is_object() {
                self.workspace.task.clone()
            } else {
                json!({})
            };
            spec["name"] = json!(name);
            spec["enabled"] = spec.get("enabled").cloned().unwrap_or(json!(true));
            spec["schedule"] = if spec["schedule"]["type"] == "once" {
                json!({"type":"once","run_at": self.value("document-run-at",cx)})
            } else {
                json!({"type":"cron","cron":self.value("document-cron",cx),"timezone":self.value("document-zone",cx)})
            };
            if spec["dispatch"].is_null() {
                spec["dispatch"] = json!({"type":"channel","channel":"console","target":{"session_id":self.session,"user_id":self.user}});
            }
            if spec["task_type"] == "text" {
                spec["text"] = json!(content);
            } else {
                spec["task_type"] = json!("agent");
                spec["request"] = task_request(
                    &spec["request"],
                    &self.session,
                    &self.user,
                    &self.channel,
                    &content,
                );
            }
            spec.as_object_mut().unwrap().remove("execution_history");
            spec.as_object_mut().unwrap().remove("execution_state");
            (
                if selected.is_some() { "PUT" } else { "POST" },
                selected
                    .as_ref()
                    .map(|id| item_path(page, id))
                    .unwrap_or(page.path().into()),
                spec,
            )
        } else if page == Page::Skills && selected.is_none() {
            (
                "POST",
                page.path().into(),
                json!({"name":name,"content":content}),
            )
        } else {
            (
                "PUT",
                item_path(page, selected.as_deref().unwrap_or(&name)),
                json!({"content":content,"expected_content":self.workspace.expected}),
            )
        };
        self.busy = true;
        self.workspace.editor_saving = true;
        let generation = self.workspace.editor_generation;
        self.request_result(method, &path, body, w, cx, move |s, result, w, cx| {
            if s.workspace.editor_generation != generation {
                return;
            }
            s.busy = false;
            s.workspace.editor_saving = false;
            match result {
                Ok(_) => {
                    s.workspace.editing = false;
                    s.notice = "已保存".into();
                    s.load_page(w, cx);
                }
                Err(error) => s.notice = error,
            }
        });
    }
}
// Editing a task changes its prompt without retargeting the stored request.
fn task_request(existing: &Value, session: &str, user: &str, channel: &str, prompt: &str) -> Value {
    let generated = backend::request_body(session, user, channel, prompt, vec![]);
    if existing.is_object() {
        let mut request = existing.clone();
        request["input"] = generated["input"].clone();
        request
    } else {
        generated
    }
}

fn item_path(page: Page, id: &str) -> String {
    format!(
        "{}/{}{}",
        page.path(),
        segment(id),
        if page == Page::Skills { "/content" } else { "" }
    )
}
fn memory_category(name: &str) -> &'static str {
    let b = name.as_bytes();
    if b.len() > 10
        && b.get(4) == Some(&b'-')
        && b.get(7) == Some(&b'-')
        && (b.get(10) == Some(&b'/') || name.ends_with(".md") && b.len() == 13)
    {
        "日记"
    } else if name.starts_with("digest/procedure/") {
        "流程"
    } else if name.starts_with("digest/wiki/") {
        "知识"
    } else {
        "其它"
    }
}
const TEMPLATES: &[(&str, &str, &str)] = &[
    (
        "每周工作周报",
        "0 17 * * 5",
        "汇总我本周的工作内容，按项目分组整理成周报草稿，列出进展、风险与下周计划。",
    ),
    (
        "会议前准备",
        "30 9 * * 1-5",
        "检查我今天的会议安排，为每个会议整理议题、相关背景材料和需要确认的问题。",
    ),
    (
        "每日资讯摘要",
        "0 9 * * 1-5",
        "收集当天与我工作领域相关的重要资讯，精选 5 条，每条一句话摘要加链接。",
    ),
    (
        "周五待办清点",
        "0 16 * * 5",
        "盘点我本周未完成的待办事项，标出已过期的，给出下周优先级建议。",
    ),
    (
        "邮件整理提醒",
        "0 18 * * 1-5",
        "提醒我处理今天未回复的重要邮件，并整理一份待回复清单。",
    ),
    (
        "月度数据报告",
        "0 10 1 * *",
        "生成上个月的工作数据汇总报告，包含关键指标变化和趋势分析。",
    ),
    (
        "每日学习卡片",
        "0 8 * * *",
        "挑一个与我工作相关的知识点，用 200 字讲清楚，附一个实际应用例子。",
    ),
    (
        "文件归档整理",
        "0 17 * * 5",
        "检查我本周产生的文档和文件，按项目归类，列出建议归档或清理的清单。",
    ),
];

fn template_schedule(cron: &str) -> &str {
    match cron {
        "0 17 * * 5" => "每周五 17:00",
        "30 9 * * 1-5" => "工作日 09:30",
        "0 9 * * 1-5" => "工作日 09:00",
        "0 16 * * 5" => "每周五 16:00",
        "0 18 * * 1-5" => "工作日 18:00",
        "0 10 1 * *" => "每月 1 日 10:00",
        "0 8 * * *" => "每天 08:00",
        _ => cron,
    }
}

#[cfg(test)]
mod tests {
    use super::task_request;
    use crate::backend;
    use serde_json::{Value, json};

    #[test]
    fn editing_task_preserves_hidden_request_fields() {
        let existing = json!({
            "session_id": "original-session", "user_id": "original-user",
            "channel": "original-channel", "stream": false,
            "request_context": {"custom_context": "keep"},
            "custom": {"value": 42},
            "input": [{"role": "user", "content": [{"type": "text", "text": "old"}]}]
        });
        let actual = task_request(
            &existing,
            "current-session",
            "current-user",
            "console",
            "new prompt",
        );
        let mut expected = existing;
        expected["input"][0]["content"][0]["text"] = json!("new prompt");
        assert_eq!(actual, expected);
    }

    #[test]
    fn new_task_uses_current_session_request_defaults() {
        assert_eq!(
            task_request(&Value::Null, "session", "user", "console", "prompt"),
            backend::request_body("session", "user", "console", "prompt", vec![])
        );
    }
}
