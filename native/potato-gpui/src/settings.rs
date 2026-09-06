use crate::view::{icon_button, muted, row_button};
use crate::*;
use gpui_kit::base::FocusTrapElement;
use gpui_kit::component::{button::*, switch::Switch};
use gpui_kit::prelude::*;
#[derive(Default)]
pub struct SettingsState {
    pub scroll: ScrollHandle,
    pub loading: std::collections::BTreeSet<String>,
    pub load_errors: BTreeMap<String, String>,
    pub generation: u64,
    pub open: bool,
    pub section: usize,
    pub provider: Option<Value>,
    pub model: Option<Value>,
    pub creating: bool,
    pub dropdown: Option<String>,
    pub saved: BTreeMap<String, String>,
    pub discard: bool,
    pub confirm: Option<(String, String, Value)>,
    pub data: BTreeMap<String, Value>,
}
const SECTIONS: &[(&str, IconName)] = &[
    ("模型与服务商", IconName::Bot),
    ("通用", IconName::SlidersHorizontal),
    ("能力", IconName::Sparkles),
    ("安全", IconName::ShieldCheck),
    ("数据", IconName::Database),
    ("快捷键", IconName::Keyboard),
    ("关于", IconName::Info),
];
impl Potato {
    pub fn open_settings(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.workspace.editing
            || self.conversations.editing.is_some()
            || self.conversations.archive_open
        {
            return;
        }
        if self.settings.open {
            return;
        }
        self.reset_settings_fields();
        self.settings.generation += 1;
        let generation = self.settings.generation;
        self.settings.loading.clear();
        self.settings.load_errors.clear();
        self.modal_focus.focus(w, cx);
        self.settings.open = true;
        self.menu = None;
        self.notice.clear();
        self.refresh(w, cx);
        for (key, path) in [
            ("search", "/api/workspace/web-search-backend"),
            ("speech", "/api/native/doubao-settings"),
            ("media", "/api/native/media-settings"),
            ("legacy", "/api/native/legacy-settings"),
            ("health", "/api/healthz"),
            ("version", "/api/version"),
        ] {
            self.settings.loading.insert(key.into());
            self.request_result("GET", path, Value::Null, w, cx, move |s, result, _, _| {
                if s.settings.generation != generation {
                    return;
                }
                s.settings.loading.remove(key);
                match result {
                    Ok(v) => {
                        s.settings.data.insert(key.into(), v);
                    }
                    Err(e) => {
                        s.settings.load_errors.insert(key.into(), e);
                    }
                }
            });
        }
        cx.notify();
    }
    fn settings_dirty(&self, cx: &App) -> bool {
        self.settings
            .saved
            .iter()
            .any(|(k, v)| self.value(k, cx) != *v)
    }
    pub fn close_panel(&mut self, cx: &mut Context<Self>) {
        if self.settings.open {
            if self.busy {
                return;
            }
            if self.settings_dirty(cx) {
                self.settings.discard = true;
            } else {
                self.settings.open = false;
                self.settings.confirm = None;
            }
        } else if self.workspace.editing {
            self.close_editor(cx);
        }
    }
    fn settings_field(
        &mut self,
        key: &str,
        value: &str,
        label: &str,
        secret: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let key = format!("settings-{key}");
        if !self.fields.contains_key(&key) {
            let state = cx.new(|cx| {
                InputState::new(w, cx)
                    .default_value(value.to_owned())
                    .placeholder(if secret {
                        "留空保留已保存密钥"
                    } else {
                        ""
                    })
                    .masked(secret)
            });
            self.subscriptions
                .push(cx.observe(&state, |_, _, cx| cx.notify()));
            self.fields.insert(key.clone(), state);
            self.settings.saved.insert(key.clone(), value.into());
        }
        let state = self.fields[&key].clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_size(px(13.)).child(label.to_owned()))
            .child(
                Input::new(&state)
                    .aria_label(label.to_owned())
                    .h(px(36.))
                    .text_size(px(14.))
                    .readonly(
                        self.busy
                            || key == "settings-url"
                                && self
                                    .settings
                                    .provider
                                    .as_ref()
                                    .is_some_and(|p| p["freeze_url"] == true),
                    )
                    .when(secret, |i| i.mask_toggle())
                    .w_full(),
            )
    }
    fn settings_choice(
        &mut self,
        key: &str,
        value: &str,
        label: &str,
        options: Vec<(String, String)>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        use gpui_kit::component::popover::Popover;
        let storage = format!("settings-{key}");
        self.settings_field(key, value, label, false, w, cx);
        let current = self.value(&storage, cx);
        let display = options
            .iter()
            .find(|(v, _)| v == &current)
            .map(|(_, label)| label.clone())
            .unwrap_or(current.clone());
        let owner = cx.entity();
        let open_key = storage.clone();
        let click_key = storage.clone();
        let mut choices = div().w(px(290.)).flex().flex_col().gap_1();
        for (i, (value, label)) in options.into_iter().enumerate() {
            let field = storage.clone();
            choices = choices.child(
                row_button(format!("choice-{key}-{i}"), label)
                    .when(value == current, |b| b.icon(IconName::Check))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if let Some(f) = s.fields.get(&field) {
                            f.update(cx, |v, cx| v.set_value(value.clone(), w, cx));
                        }
                        s.settings.dropdown = None;
                        cx.notify();
                    })),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_size(px(13.)).child(label.to_owned()))
            .child(
                Popover::new(SharedString::from(format!("select-{key}")))
                    .open(self.settings.dropdown.as_ref() == Some(&storage))
                    .trigger(
                        Button::new(SharedString::from(format!("select-trigger-{key}")))
                            .disabled(self.busy)
                            .accessibility_label(label.to_owned())
                            .outline()
                            .w_full()
                            .text_size(px(14.))
                            .label(display)
                            .dropdown_caret(true)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.settings.dropdown = Some(click_key.clone());
                                cx.notify();
                            })),
                    )
                    .on_open_change(move |open, _, cx| {
                        owner.update(cx, |s, cx| {
                            s.settings.dropdown = if *open { Some(open_key.clone()) } else { None };
                            cx.notify();
                        })
                    })
                    .child(choices)
                    .p_1(),
            )
    }
    fn provider_options(&self) -> Vec<(String, String)> {
        std::iter::once((String::new(), "默认".into()))
            .chain(
                self.providers
                    .iter()
                    .map(|p| (string(p, "id"), string(p, "name"))),
            )
            .collect()
    }
    fn reset_settings_fields(&mut self) {
        self.settings.scroll.set_offset(point(px(0.), px(0.)));
        self.fields.retain(|k, _| !k.starts_with("settings-"));
        self.settings.saved.clear();
        self.settings.dropdown = None;
        self.notice.clear();
    }
    fn settings_value(&self, k: &str, cx: &App) -> String {
        self.value(&format!("settings-{k}"), cx)
    }
    fn save_setting(
        &mut self,
        method: &str,
        path: &str,
        body: Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let keys: Vec<&str> = if path.ends_with("/web-search-backend") {
            vec!["search-backend", "search-provider", "search-model"]
        } else if path.ends_with("/doubao-settings") {
            vec!["speech-app", "speech-key", "speech-resource"]
        } else if path.ends_with("/media-settings") {
            vec!["image-provider", "image-model"]
        } else if path.ends_with("/legacy-settings") {
            vec!["working", "secret"]
        } else if path.ends_with("/models") && method == "POST" {
            vec!["new-model"]
        } else if path.ends_with("/config") && path.matches("/models/").count() > 1 {
            vec!["name", "max_tokens", "max_input_length", "reasoning_effort"]
        } else if path.ends_with("/config") {
            vec!["url", "protocol", "key"]
        } else {
            vec![]
        };
        let submitted: Vec<_> = keys
            .iter()
            .map(|k| (format!("settings-{k}"), self.settings_value(k, cx)))
            .collect();
        let data_key = if path.ends_with("/doubao-settings") {
            Some("speech")
        } else if path.ends_with("/media-settings") {
            Some("media")
        } else if path.ends_with("/web-search-backend") {
            Some("search")
        } else {
            None
        };
        self.busy = true;
        self.request(method, path, body, w, cx, move |s, v, w, cx| {
            s.busy = false;
            s.notice = "已保存".into();
            for (key, value) in submitted {
                s.settings.saved.insert(key, value);
            }
            if let Some(key) = data_key {
                s.settings.data.insert(key.into(), v);
            }
            s.refresh(w, cx);
        });
    }
    pub fn settings_view(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let body = self.settings_content(w, cx);
        let mut nav = div()
            .w(px(184.))
            .flex_shrink_0()
            .h_full()
            .p_3()
            .rounded_tl(px(16.))
            .rounded_bl(px(16.))
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .flex()
            .flex_col()
            .gap_1();
        for (i, (label, icon)) in SECTIONS.iter().enumerate() {
            nav = nav.child(
                row_button(*label, *label)
                    .disabled(self.busy)
                    .icon(*icon)
                    .when(self.settings.section == i, |b| {
                        b.bg(cx.theme().background).shadow_sm()
                    })
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if s.settings_dirty(cx) {
                            s.notice = "请先保存修改，或关闭面板放弃修改".into();
                            cx.notify();
                            return;
                        }
                        s.settings.section = i;
                        s.settings.provider = None;
                        s.settings.model = None;
                        s.settings.creating = false;
                        s.reset_settings_fields();
                        cx.notify();
                    })),
            );
        }
        let mut main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(
                        self.settings.provider.is_some() || self.settings.creating,
                        |d| {
                            d.child(
                                icon_button("settings-back", IconName::ChevronLeft, "返回")
                                    .disabled(self.busy)
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        if s.settings_dirty(cx) {
                                            s.notice = "请先保存修改".into();
                                            return;
                                        }
                                        if s.settings.model.is_some() {
                                            s.settings.model = None;
                                        } else {
                                            s.settings.provider = None;
                                            s.settings.creating = false;
                                        }
                                        s.reset_settings_fields();
                                        cx.notify();
                                    })),
                            )
                        },
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::MEDIUM)
                            .child(SECTIONS[self.settings.section].0),
                    )
                    .child(
                        icon_button("settings-close", IconName::X, "关闭设置").on_click(
                            cx.listener(|s, _, _, cx| {
                                s.close_panel(cx);
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .track_scroll(&self.settings.scroll)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            );
        if !self.notice.is_empty() {
            main = main.child(muted(self.notice.clone(), cx));
        }
        if self.busy {
            main = main.child(muted("正在处理…", cx));
        }
        if self.settings.discard {
            main = main.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child("有尚未保存的修改")
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("keep-editing")
                                    .outline()
                                    .small()
                                    .h(px(32.))
                                    .self_start()
                                    .label("继续编辑")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.settings.discard = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("discard-settings")
                                    .danger()
                                    .small()
                                    .h(px(32.))
                                    .self_start()
                                    .label("放弃修改")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.settings.open = false;
                                        s.settings.discard = false;
                                        s.reset_settings_fields();
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        if let Some((method, path, body)) = self.settings.confirm.clone() {
            main = main.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child("确认执行此操作？")
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-delete")
                                    .outline()
                                    .small()
                                    .h(px(32.))
                                    .self_start()
                                    .label("取消")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.settings.confirm = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("confirm-delete")
                                    .danger()
                                    .small()
                                    .h(px(32.))
                                    .self_start()
                                    .label("确认")
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        s.settings.confirm = None;
                                        s.save_setting(&method, &path, body.clone(), w, cx);
                                        s.settings.provider = None;
                                        s.settings.model = None;
                                        s.reset_settings_fields();
                                    })),
                            ),
                    ),
            );
        }
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x14141448))
            .child(
                div()
                    .id("settings-panel")
                    .occlude()
                    .tab_group()
                    .flex()
                    .w(px(860.))
                    .max_w_full()
                    .h(px(620.))
                    .max_h_full()
                    .rounded(px(16.))
                    .overflow_hidden()
                    .bg(cx.theme().background)
                    .shadow(vec![design::shadow()])
                    .child(nav)
                    .child(main),
            )
            .track_focus(&self.modal_focus)
            .focus_trap("settings-trap", &self.modal_focus)
            .into_any_element()
    }
    fn settings_content(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let required: &[&str] = match self.settings.section {
            1 => &["search"],
            2 => &["speech", "media"],
            4 => &["legacy"],
            _ => &[],
        };
        if required.iter().any(|k| self.settings.loading.contains(*k)) {
            return muted("正在加载设置…", cx).into_any_element();
        }
        if let Some(error) = required
            .iter()
            .find_map(|k| self.settings.load_errors.get(*k))
        {
            return div()
                .flex()
                .flex_col()
                .gap_3()
                .child(muted(format!("设置加载失败：{error}"), cx))
                .child(
                    Button::new("retry-settings")
                        .outline()
                        .label("重新加载")
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.settings.open = false;
                            s.open_settings(w, cx);
                        })),
                )
                .into_any_element();
        }
        match self.settings.section {
            0 => self.providers_view(w, cx),
            1 => {
                let search = self
                    .settings
                    .data
                    .get("search")
                    .cloned()
                    .unwrap_or_default();
                let provider = self.settings_choice(
                    "search-provider",
                    &string(&search, "web_search_provider_id"),
                    "搜索服务商",
                    self.provider_options(),
                    w,
                    cx,
                );
                let model = self.settings_field(
                    "search-model",
                    &string(&search, "web_search_model"),
                    "搜索模型",
                    false,
                    w,
                    cx,
                );
                let backend = self.settings_choice(
                    "search-backend",
                    search["web_search_backend"].as_str().unwrap_or("auto"),
                    "搜索方式",
                    vec![
                        ("auto".into(), "自动选择".into()),
                        ("hosted".into(), "模型内置搜索".into()),
                        ("exa".into(), "Exa".into()),
                        ("tavily".into(), "Tavily".into()),
                    ],
                    w,
                    cx,
                );
                div().flex().flex_col().gap_6()
                    .child(group("外观",cx).child(div().flex().items_center().justify_between().child("深色模式")
                        .child(Switch::new("dark-mode").accessibility_label("深色模式").checked(self.dark).on_click(cx.listener(|s,_,w,cx|s.toggle_theme(w,cx)))))
                        .child(div().flex().items_center().justify_between().child("跟随系统外观").child(Switch::new("follow-system").accessibility_label("跟随系统外观").checked(self.preferences["follow_system"] == true).on_click(cx.listener(|s,on,w,cx|{
                            s.preferences["follow_system"] = json!(*on);
                            if *on { s.dark=matches!(w.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark); design::apply(s.dark,Some(w),cx); }
                            s.request("PUT","/api/native/preferences",s.preferences.clone(),w,cx,|_,_,_,_|{});
                        })))))
                    .child(group("窗口",cx)
                        .child(div().flex().items_center().justify_between().child("记住窗口大小").child(Switch::new("remember-window").accessibility_label("记住窗口大小").checked(self.preferences["remember_window"] != false).on_click(cx.listener(|s,on,w,cx|{
                            s.preferences["remember_window"]=json!(*on); s.window_save_epoch+=1;
                            if *on { let bounds=w.bounds();s.preferences["width"]=json!(f32::from(bounds.size.width));s.preferences["height"]=json!(f32::from(bounds.size.height)); }
                            s.request("PUT","/api/native/preferences",s.preferences.clone(),w,cx,|_,_,_,_|{});
                        }))))
                        .child(Button::new("reset-window").outline().small().h(px(32.)).self_start().label("恢复默认大小").on_click(cx.listener(|_,_,w,_|w.resize(size(px(1180.),px(800.)))))))
                    .child(group("联网搜索",cx).child(backend).child(provider).child(model)
                        .child(Button::new("save-search").outline().small().h(px(32.)).self_start().label("保存").disabled(self.busy).on_click(cx.listener(|s,_,w,cx|{
                            let b=json!({"web_search_backend":s.settings_value("search-backend",cx),"web_search_provider_id":s.settings_value("search-provider",cx),"web_search_model":s.settings_value("search-model",cx)});
                            s.save_setting("PUT","/api/workspace/web-search-backend",b,w,cx);
                        })))).into_any_element()
            }
            2 => {
                let speech = self
                    .settings
                    .data
                    .get("speech")
                    .cloned()
                    .unwrap_or_default();
                let media = self.settings.data.get("media").cloned().unwrap_or_default();
                let app = self.settings_field(
                    "speech-app",
                    &string(&speech, "app_id"),
                    "语音应用 ID",
                    false,
                    w,
                    cx,
                );
                let key = self.settings_field("speech-key", "", "语音 API Key", true, w, cx);
                let resource = self.settings_field(
                    "speech-resource",
                    speech["resource_id"]
                        .as_str()
                        .unwrap_or("volc.seedasr.sauc.duration"),
                    "资源 ID",
                    false,
                    w,
                    cx,
                );
                let provider = self.settings_choice(
                    "image-provider",
                    &string(&media, "image_provider_id"),
                    "图片服务商",
                    self.provider_options(),
                    w,
                    cx,
                );
                let model = self.settings_field(
                    "image-model",
                    media["image_model"].as_str().unwrap_or("gpt-image-2"),
                    "图片模型",
                    false,
                    w,
                    cx,
                );
                div().flex().flex_col().gap_6().child(group("语音输入",cx).child(app).child(key).child(resource)
                    .child(Button::new("save-speech").outline().small().h(px(32.)).self_start().label("保存并启用").disabled(self.busy).on_click(cx.listener(|s,_,w,cx|{
                        let mut b=json!({"enabled":true,"app_id":s.settings_value("speech-app",cx),"resource_id":s.settings_value("speech-resource",cx)});
                        let key=s.settings_value("speech-key",cx);if !key.is_empty(){b["api_key"]=json!(key);}s.save_setting("PUT","/api/native/doubao-settings",b,w,cx);
                    }))))
                    .child(group("图片生成",cx).child(provider).child(model)
                        .child(Button::new("save-image").outline().small().h(px(32.)).self_start().label("保存").disabled(self.busy).on_click(cx.listener(|s,_,w,cx|{
                            let mut b=s.settings.data.get("media").cloned().unwrap_or(json!({}));
                            b["image_provider_id"]=json!(s.settings_value("image-provider",cx));b["image_model"]=json!(s.settings_value("image-model",cx));
                            s.save_setting("PUT","/api/native/media-settings",b,w,cx);
                        })))).into_any_element()
            }
            3 => {
                let mut box_ = group("访问权限", cx).child(muted(
                    "控制助手能够访问的文件范围；需要确认的操作仍会单独询问。",
                    cx,
                ));
                for (id, label) in [
                    ("read-only", "只读"),
                    ("workspace-write", "工作区读写"),
                    ("full-access", "完全访问"),
                ] {
                    box_ = box_.child(
                        row_button(id, label)
                            .when(self.config["sandbox_mode"] == id, |b| {
                                b.icon(IconName::Check)
                            })
                            .disabled(self.streaming)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.save_setting(
                                    "PUT",
                                    "/api/workspace/running-config",
                                    json!({"sandbox_mode":id}),
                                    w,
                                    cx,
                                )
                            })),
                    );
                }
                box_.into_any_element()
            }
            4 => {
                let legacy = self
                    .settings
                    .data
                    .get("legacy")
                    .cloned()
                    .unwrap_or_default();
                let working = self.settings_field(
                    "working",
                    &string(&legacy, "working_dir"),
                    "原版工作目录",
                    false,
                    w,
                    cx,
                );
                let secret = self.settings_field(
                    "secret",
                    &string(&legacy, "secret_dir"),
                    "原版密钥目录",
                    false,
                    w,
                    cx,
                );
                div().flex().flex_col().gap_6().child(group("导入原版配置",cx).child(working).child(secret)
                    .child(muted("导入服务商和模型连接，原版数据保持不变。",cx))
                    .child(Button::new("import-config").outline().small().h(px(32.)).self_start().label("导入配置").disabled(self.busy).on_click(cx.listener(|s,_,w,cx|s.save_setting("POST","/api/native/legacy-settings",json!({"working_dir":s.settings_value("working",cx),"secret_dir":s.settings_value("secret",cx)}),w,cx)))))
                    .child(group("备份与恢复",cx)
                        .child(row_button("export","导出工作区").icon(IconName::Download).on_click(cx.listener(|s,_,w,cx|s.export_workspace(w,cx))))
                        .child(row_button("import-history","导入会话记录").icon(IconName::Upload).on_click(cx.listener(|s,_,w,cx|s.import_history(w,cx))))
                        .child(row_button("open-data","打开数据目录").icon(IconName::FolderOpen).on_click(cx.listener(|s,_,_,_|{let _=open::that(&s.backend.data_dir);}))))
                    .into_any_element()
            }
            5 => {
                let mut list = group("键盘快捷键", cx);
                for (name, keys) in [
                    ("新建会话", "⌘ N"),
                    ("搜索会话", "⌘ K"),
                    ("展开或收起侧栏", "⌘ B"),
                    ("打开设置", "⌘ ,"),
                    ("发送消息", "Enter"),
                    ("换行", "Shift Enter"),
                    ("关闭浮层", "Esc"),
                ] {
                    list = list.child(
                        div()
                            .flex()
                            .justify_between()
                            .py_2()
                            .child(name)
                            .child(muted(keys, cx)),
                    );
                }
                list.into_any_element()
            }
            _ => group("Potato", cx)
                .child(muted("你的桌面 AI 助手", cx))
                .child(muted("GPUI Kit 0.6 · Rust 原生前端与后端", cx))
                .child(muted(
                    format!(
                        "版本 {}",
                        self.settings
                            .data
                            .get("version")
                            .and_then(|v| v["version"].as_str())
                            .unwrap_or("0.1.0")
                    ),
                    cx,
                ))
                .child(muted(
                    format!(
                        "运行状态：{}",
                        self.settings
                            .data
                            .get("health")
                            .and_then(|v| v["status"].as_str())
                            .unwrap_or("检查中")
                    ),
                    cx,
                ))
                .into_any_element(),
        }
    }
    fn providers_view(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.settings.model.is_some() {
            return self.model_settings(w, cx);
        }
        if let Some(p) = self.settings.provider.clone() {
            return self.provider_settings(p, w, cx);
        }
        if self.settings.creating {
            return self.provider_settings(json!({}), w, cx);
        }
        let mut list = group("服务商", cx).gap_1();
        for (i, p) in self.providers.iter().enumerate() {
            let provider = p.clone();
            let n = p["models"].as_array().map_or(0, Vec::len)
                + p["extra_models"].as_array().map_or(0, Vec::len);
            list = list.child(
                row_button(format!("provider-{i}"), string(p, "name"))
                    .h(px(38.))
                    .child(div().flex_1())
                    .child(muted(format!("{n} 个模型"), cx))
                    .child(IconName::ChevronRight)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.settings.provider = Some(provider.clone());
                        s.reset_settings_fields();
                        cx.notify();
                    })),
            );
        }
        list.child(div().h(px(1.)).bg(cx.theme().border).my_2())
            .child(
                row_button("add-provider", "添加服务商")
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.settings.creating = true;
                        s.reset_settings_fields();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    fn provider_settings(
        &mut self,
        p: Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let creating = self.settings.creating;
        let name = self.settings_field("name", &string(&p, "name"), "名称", false, w, cx);
        let url = self.settings_field("url", &string(&p, "base_url"), "API 地址", false, w, cx);
        let key = self.settings_field("key", "", "API Key", true, w, cx);
        let protocol = self.settings_choice(
            "protocol",
            p["chat_model"].as_str().unwrap_or("OpenAIChatModel"),
            "协议",
            vec![
                ("OpenAIChatModel".into(), "OpenAI Chat Completions".into()),
                ("OpenAIResponseModel".into(), "OpenAI Responses".into()),
            ],
            w,
            cx,
        );
        let mut panel = div().flex().flex_col().gap_5().child(
            group(
                if creating {
                    "添加服务商"
                } else {
                    p["name"].as_str().unwrap_or("服务商")
                },
                cx,
            )
            .when(creating, |v| v.child(name))
            .child(url)
            .child(key)
            .child(protocol)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("save-provider")
                            .primary()
                            .small()
                            .h(px(32.))
                            .label("保存连接")
                            .disabled(self.busy)
                            .on_click(cx.listener(|s, _, w, cx| s.save_provider(w, cx))),
                    )
                    .when(!creating, |v| {
                        v.child(
                            Button::new("test-provider")
                                .outline()
                                .small()
                                .h(px(32.))
                                .label("测试连接")
                                .disabled(self.busy)
                                .on_click(
                                    cx.listener(|s, _, w, cx| s.provider_action("test", w, cx)),
                                ),
                        )
                    }),
            ),
        );
        if !creating {
            let mut models = group("模型", cx).gap_1();
            for (i, m) in p["models"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(p["extra_models"].as_array().into_iter().flatten())
                .enumerate()
            {
                let model = m.clone();
                let provider = string(&p, "id");
                let id = string(m, "id");
                models = models.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            row_button(
                                format!("model-detail-{i}"),
                                m["name"].as_str().unwrap_or(&id).to_owned(),
                            )
                            .w_auto()
                            .flex_1()
                            .min_w_0()
                            .on_click(cx.listener(
                                move |s, _, _, cx| {
                                    if s.busy || s.settings_dirty(cx) {
                                        s.notice = "请先保存连接修改，再编辑模型".into();
                                        cx.notify();
                                        return;
                                    }
                                    s.settings.model = Some(model.clone());
                                    s.reset_settings_fields();
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            Button::new(("activate", i))
                                .ghost()
                                .small()
                                .h(px(32.))
                                .label(
                                    if self.model["model"] == id
                                        && self.model["provider_id"] == provider
                                    {
                                        "使用中"
                                    } else {
                                        "使用"
                                    },
                                )
                                .disabled(self.streaming)
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.save_setting(
                                        "PUT",
                                        "/api/models/active",
                                        json!({"provider_id":provider,"model":id}),
                                        w,
                                        cx,
                                    )
                                })),
                        ),
                );
            }
            let model = self.settings_field("new-model", "", "模型 ID", false, w, cx);
            models = models.child(div().mt_3().child(model)).child(
                div()
                    .flex()
                    .gap_2()
                    .mt_2()
                    .child(
                        Button::new("add-model")
                            .outline()
                            .small()
                            .h(px(32.))
                            .label("添加模型")
                            .on_click(cx.listener(|s, _, w, cx| {
                                let id = s
                                    .settings
                                    .provider
                                    .as_ref()
                                    .map(|p| string(p, "id"))
                                    .unwrap_or_default();
                                let model = s.settings_value("new-model", cx);
                                if model.trim().is_empty() {
                                    s.notice = "请填写模型 ID".into();
                                    return;
                                }
                                s.save_setting(
                                    "POST",
                                    &format!("/api/models/{}/models", segment(&id)),
                                    json!({"id":model.trim(),"name":model.trim()}),
                                    w,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("discover-models")
                            .ghost()
                            .small()
                            .h(px(32.))
                            .label("获取模型列表")
                            .on_click(
                                cx.listener(|s, _, w, cx| s.provider_action("discover", w, cx)),
                            ),
                    ),
            );
            panel = panel.child(models).child(
                Button::new("clear-key")
                    .ghost()
                    .small()
                    .h(px(32.))
                    .label("清除已保存密钥")
                    .on_click(cx.listener(|s, _, _, cx| {
                        let id = s
                            .settings
                            .provider
                            .as_ref()
                            .map(|p| string(p, "id"))
                            .unwrap_or_default();
                        s.settings.confirm = Some((
                            "PUT".into(),
                            format!("/api/models/{}/config", segment(&id)),
                            json!({"api_key":""}),
                        ));
                        cx.notify();
                    })),
            );
            if p["is_custom"] == true {
                panel = panel.child(
                    Button::new("delete-provider")
                        .ghost()
                        .small()
                        .h(px(32.))
                        .label("删除服务商")
                        .text_color(cx.theme().danger)
                        .on_click(cx.listener(|s, _, _, cx| {
                            let id = s
                                .settings
                                .provider
                                .as_ref()
                                .map(|p| string(p, "id"))
                                .unwrap_or_default();
                            s.settings.confirm = Some((
                                "DELETE".into(),
                                format!("/api/models/{}", segment(&id)),
                                Value::Null,
                            ));
                            cx.notify();
                        })),
                );
            }
        }
        panel.into_any_element()
    }
    fn save_provider(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let mut b = json!({"base_url":self.settings_value("url",cx).trim(),"chat_model":self.settings_value("protocol",cx)});
        let key = self.settings_value("key", cx);
        if !key.is_empty() {
            b["api_key"] = json!(key);
        }
        if self.busy {
            return;
        }
        if self.settings.creating {
            self.busy = true;
            b["id"] = json!(uuid::Uuid::new_v4().to_string());
            b["name"] = json!(self.settings_value("name", cx));
            b["default_base_url"] = b["base_url"].clone();
            self.request(
                "POST",
                "/api/models/custom-providers",
                b,
                w,
                cx,
                |s, _, w, cx| {
                    s.busy = false;
                    s.settings.creating = false;
                    s.reset_settings_fields();
                    s.refresh(w, cx);
                },
            );
        } else {
            let id = self
                .settings
                .provider
                .as_ref()
                .map(|p| string(p, "id"))
                .unwrap_or_default();
            self.save_setting(
                "PUT",
                &format!("/api/models/{}/config", segment(&id)),
                b,
                w,
                cx,
            );
        }
    }
    fn provider_action(&mut self, action: &str, w: &mut Window, cx: &mut Context<Self>) {
        let id = self
            .settings
            .provider
            .as_ref()
            .map(|p| string(p, "id"))
            .unwrap_or_default();
        let mut b = json!({"base_url":self.settings_value("url",cx)});
        let key = self.settings_value("key", cx);
        if !key.is_empty() {
            b["api_key"] = json!(key);
        }
        if self.busy {
            return;
        }
        self.busy = true;
        let testing = action == "test";
        self.request(
            "POST",
            &format!("/api/models/{}/{action}", segment(&id)),
            b,
            w,
            cx,
            move |s, v, w, cx| {
                s.busy = false;
                s.notice = if testing {
                    format!(
                        "连接测试：{}",
                        v.get("message").and_then(Value::as_str).unwrap_or("完成")
                    )
                } else {
                    "模型发现已完成".into()
                };
                s.refresh(w, cx);
            },
        );
    }
    fn model_settings(&mut self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let model = self.settings.model.clone().unwrap();
        let mut panel = group(&string(&model, "id"), cx);
        for (key, label) in [
            ("name", "显示名称"),
            ("max_tokens", "最大输出 token"),
            ("max_input_length", "上下文容量"),
            ("reasoning_effort", "推理强度"),
        ] {
            let value = if model[key].is_number() {
                model[key].to_string()
            } else {
                string(&model, key)
            };
            panel = panel.child(self.settings_field(key, &value, label, false, w, cx));
        }
        panel.child(Button::new("save-model-config").primary().small().h(px(32.)).self_start().label("保存").on_click(cx.listener(|s,_,w,cx|{
            let mut body=json!({"name":s.settings_value("name",cx),"reasoning_effort":s.settings_value("reasoning_effort",cx)});
            for key in ["max_tokens","max_input_length"]{let text=s.settings_value(key,cx);body[key]=if text.is_empty(){Value::Null}else{match text.parse::<u64>(){Ok(n)=>json!(n),Err(_)=>{s.notice="Token 数量应为正整数".into();return;}}};}
            let p=s.settings.provider.as_ref().map(|p|string(p,"id")).unwrap_or_default();let m=s.settings.model.as_ref().map(|p|string(p,"id")).unwrap_or_default();
            s.save_setting("PUT",&format!("/api/models/{}/models/{}/config",segment(&p),segment(&m)),body,w,cx);
        }))).into_any_element()
    }
    fn export_workspace(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let api = self.backend.clone();
        cx.spawn_in(w, async move |this, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_file_name("potato-workspace.zip")
                .save_file()
                .await
            {
                let result = async {
                    use base64::Engine;
                    let v = api
                        .request("GET", "/api/workspace/download", Value::Null)
                        .await
                        .map_err(|_| "后台已停止")??;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(v["native_binary"].as_str().ok_or("导出格式错误")?)
                        .map_err(|_| "导出内容无法解码")?;
                    file.write(&bytes).await.map_err(|_| "无法保存文件")?;
                    Ok::<_, String>(())
                }
                .await;
                let _ = this.update_in(cx, |s, _, cx| {
                    s.notice = result.map(|_| "工作区已导出".into()).unwrap_or_else(|e| e);
                    cx.notify();
                });
            }
        })
        .detach();
    }
    fn import_history(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(w, async move |this, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("会话记录", &["json"])
                .pick_file()
                .await
            {
                let bytes = file.read().await;
                let _ = this.update_in(cx, |s, w, cx| {
                    if bytes.len() > 50_000_000 {
                        s.notice = "文件超过 50 MB".into();
                        return;
                    }
                    match serde_json::from_slice(&bytes) {
                        Ok(v) => s.save_setting("POST", "/api/native/import-history", v, w, cx),
                        Err(_) => s.notice = "会话记录不是有效 JSON".into(),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
fn group(title: &str, cx: &App) -> Div {
    div()
        .rounded(px(12.))
        .border_1()
        .border_color(cx.theme().border)
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .child(title.to_owned()),
        )
}
