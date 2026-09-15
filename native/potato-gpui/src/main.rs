#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod chat;
mod cloud;
mod conversations;
mod effort;
mod icons;
mod interactions;
mod media;
mod outbox;
mod process;
#[cfg(debug_assertions)]
mod review;
mod side_panel;
mod side_panel_data;
#[cfg(test)]
mod state_tests;
mod window_preferences;
mod theme_mode;
use theme_mode::ThemePreference;
use gpui_kit::component::input::InputEvent;
use icons::IconName;
mod backend;
mod design;
mod settings;
#[path = "../../potato-ui/src/stream.rs"]
mod stream;
mod view;
mod voice;
mod workspace;
use backend::{Backend, segment};
use futures::StreamExt;
use gpui_kit::component::{input::*, *};
use gpui_kit::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
actions!(
    potato,
    [
        NewChat,
        Search,
        Settings,
        ToggleSidebar,
        Dismiss,
        Quit,
        ShowSendOptions
    ]
);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Chat,
    Tasks,
    Skills,
    Memory,
    Workspace,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Project,
    Model,
    Effort,
    Permission,
    Conversations,
}
struct Potato {
    effort: effort::Effort,
    files: side_panel::SidePanel,
    outbox: outbox::Outbox,
    chat: chat::ChatState,
    conversations: conversations::Conversations,
    interactions: interactions::Interactions,
    voice: voice::Voice,
    backend: Backend,
    page: Page,
    focus: FocusHandle,
    modal_focus: FocusHandle,
    archive_trigger_focus: FocusHandle,
    window_save_epoch: u64,
    composer: Entity<TextareaState>,
    search: Entity<InputState>,
    conversation_search: Entity<InputState>,
    archive_search: Entity<InputState>,
    editor: Entity<TextareaState>,
    fields: BTreeMap<String, Entity<InputState>>,
    subscriptions: Vec<Subscription>,
    chats: Vec<Value>,
    providers: Vec<Value>,
    model: Value,
    config: Value,
    preferences: Value,
    project: Value,
    projects: Vec<Value>,
    project_creating: bool,
    session: String,
    user: String,
    channel: String,
    selected: Option<String>,
    history: Vec<Value>,
    turn: stream::Turn,
    streaming: bool,
    attachments: Vec<Value>,
    drafts: BTreeMap<String, (String, Vec<Value>)>,
    sidebar: bool,
    dark: bool,
    search_open: bool,
    menu: Option<Menu>,
    notice: String,
    busy: bool,
    epoch: u64,
    // Separate from workspace loads; invalidates streams even on an A → B → A switch.
    chat_epoch: u64,
    settings: settings::SettingsState,
    workspace: workspace::WorkspaceState,
}
impl Potato {
    fn new(backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("描述任务…")
                .auto_grow(1, 7)
                .submit_on_enter(true)
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索名称、描述或标签"));
        let conversation_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索会话名称"));
        let archive_search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索已归档会话"));
        let editor = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("在这里编辑内容…")
                .rows(16)
        });
        let subscriptions = vec![
            cx.subscribe_in(&composer, window, |this, _, event, window, cx| {
                if let InputEvent::PressEnter {
                    shift: false,
                    secondary,
                } = event
                {
                    this.send_mode(*secondary, window, cx);
                }
                cx.notify();
            }),
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.observe(&conversation_search, |_, _, cx| cx.notify()),
            cx.observe(&archive_search, |_, _, cx| cx.notify()),
            cx.observe(&editor, |_, _, cx| cx.notify()),
        ];
        let mut this = Self {
            files: side_panel::SidePanel::default(),
            outbox: outbox::Outbox::default(),
            chat: chat::ChatState::default(),
            conversations: conversations::Conversations::default(),
            interactions: interactions::Interactions::default(),
            voice: voice::Voice::default(),
            backend,
            page: Page::Chat,
            focus: cx.focus_handle(),
            modal_focus: cx.focus_handle(),
            archive_trigger_focus: cx.focus_handle(),
            window_save_epoch: 0,
            composer,
            search,
            conversation_search,
            archive_search,
            editor,
            fields: BTreeMap::new(),
            effort: effort::Effort::default(),
            subscriptions,
            chats: vec![],
            providers: vec![],
            model: Value::Null,
            config: Value::Null,
            preferences: json!({}),
            project: Value::Null,
            projects: vec![],
            project_creating: false,
            session: uuid::Uuid::new_v4().to_string(),
            user: "default".into(),
            channel: "console".into(),
            selected: None,
            history: vec![],
            turn: stream::Turn::default(),
            streaming: false,
            attachments: vec![],
            drafts: BTreeMap::new(),
            sidebar: true,
            dark: Theme::global(cx).is_dark(),
            search_open: false,
            menu: None,
            notice: String::new(),
            busy: false,
            epoch: 0,
            chat_epoch: 0,
            settings: settings::SettingsState::default(),
            workspace: workspace::WorkspaceState::default(),
        };
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                if this
                    .update_in(cx, |s, w, cx| {
                        s.poll_interactions(w, cx);
                        s.poll_outbox(w, cx);
                        s.poll_background_changes(w, cx);
                        s.poll_cloud_login_ui(w, cx);
                        s.poll_remote_login_ui(w, cx);
                        if s.streaming {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        this.subscriptions
            .push(cx.observe_window_appearance(window, |s, w, cx| {
                if ThemePreference::from_preferences(&s.preferences) == ThemePreference::System {
                    s.dark = matches!(
                        w.appearance(),
                        WindowAppearance::Dark | WindowAppearance::VibrantDark
                    );
                    design::apply(s.dark, Some(w), cx);
                    cx.notify();
                }
            }));
        this.subscriptions
            .push(cx.observe_window_bounds(window, |s, w, cx| {
                if s.preferences["remember_window"] == false || w.is_fullscreen() {
                    return;
                }
                let bounds = w.bounds();
                s.preferences["width"] = json!(f32::from(bounds.size.width));
                s.preferences["height"] = json!(f32::from(bounds.size.height));
                s.window_save_epoch += 1;
                let epoch = s.window_save_epoch;
                cx.spawn_in(w, async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let _ = this.update_in(cx, |s, w, cx| {
                        if s.window_save_epoch == epoch && s.preferences["remember_window"] != false
                        {
                            s.request(
                                "PUT",
                                "/api/native/preferences",
                                s.preferences.clone(),
                                w,
                                cx,
                                |_, _, _, _| {},
                            );
                        }
                    });
                })
                .detach();
            }));
        this.focus.focus(window, cx);
        this.refresh(window, cx);
        this.request_result("GET", "/api/native/cloud", Value::Null, window, cx, |s,r,w,cx| {
            if s.settings.cloud_generation != 0 { return; }
            if let Ok(v) = r {
                let signed_in = v["signed_in"] == true;
                s.settings.data.insert("cloud".into(), v);
                if signed_in { s.cloud_action("refresh", w, cx); }
            }
        });
        this.request(
            "GET",
            "/api/native/preferences",
            Value::Null,
            window,
            cx,
            |this, v, window, cx| {
                this.preferences = v;
                this.dark = ThemePreference::from_preferences(&this.preferences)
                    .is_dark(window.appearance());
                design::apply(this.dark, Some(window), cx);
            },
        );
        #[cfg(debug_assertions)]
        review::load(&mut this, window, cx);
        this
    }
    fn request(
        &mut self,
        method: &str,
        path: &str,
        body: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Value, &mut Window, &mut Context<Self>) + 'static,
    ) {
        self.request_result(
            method,
            path,
            body,
            window,
            cx,
            move |s, result, w, cx| match result {
                Ok(v) => done(s, v, w, cx),
                Err(e) => {
                    s.notice = e;
                    s.busy = false;
                }
            },
        );
    }
    fn request_result(
        &mut self,
        method: &str,
        path: &str,
        body: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, backend::Reply, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let rx = self.backend.request(method, path, body);
        cx.spawn_in(window, async move |this, cx| {
            let result = rx.await.unwrap_or_else(|_| Err("后台任务已结束".into()));
            let _ = this.update_in(cx, |this, window, cx| {
                done(this, result, window, cx);
                cx.notify();
            });
        })
        .detach();
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request(
            "GET",
            "/api/chats",
            Value::Null,
            window,
            cx,
            |s, v, _, _| {
                #[cfg(debug_assertions)]
                if s.session == "visual-review"
                    && std::env::var_os("POTATO_GPUI_REVIEW_FIXTURE").is_some()
                    && std::env::var_os("POTATO_NATIVE_DATA_DIR").is_some()
                {
                    return;
                }
                s.chats = array(v);
                if s.selected.is_none() && !s.history.is_empty() {
                    s.selected = s
                        .chats
                        .iter()
                        .find(|c| c["session_id"] == s.session)
                        .map(|c| string(c, "id"));
                }
            },
        );
        self.request(
            "GET",
            "/api/models",
            Value::Null,
            window,
            cx,
            |s, v, _, _| {
                s.providers = array(v);
                if let Some(id) = s.settings.provider.as_ref().map(|p| string(p, "id")) {
                    s.settings.provider = s.providers.iter().find(|p| p["id"] == id).cloned();
                }
            },
        );
        self.request(
            "GET",
            "/api/models/active",
            Value::Null,
            window,
            cx,
            |s, v, _, _| s.model = v["active_llm"].clone(),
        );
        self.request(
            "GET",
            "/api/workspace/running-config",
            Value::Null,
            window,
            cx,
            |s, v, _, _| s.config = v,
        );
        self.request(
            "GET",
            "/api/workspace/coding-project",
            Value::Null,
            window,
            cx,
            |s, v, _, _| s.project = v,
        );
        self.request(
            "GET",
            "/api/workspace/coding-project/list",
            Value::Null,
            window,
            cx,
            |s, v, _, _| s.projects = array(v),
        );
    }
    fn poll_background_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sessions = self.backend.take_background_sessions();
        if sessions.is_empty() { return; }
        self.request("GET", "/api/chats", Value::Null, window, cx, |s, v, _, _| s.chats = array(v));
        // A remote start uses this same core. Attach its live replay when the
        // desktop is already looking at that conversation, preserving drafts.
        if !sessions.contains(&self.session) || self.streaming || self.chat.loading || self.conversations.editing.is_some() { return; }
        let Some(id) = self.selected.clone() else { return; };
        let epoch = self.chat_epoch;
        self.request("GET", &format!("/api/chats/{}", segment(&id)), Value::Null, window, cx, move |s, v, w, cx| {
            if s.chat_epoch != epoch || s.streaming { return; }
            s.history = array(v["messages"].clone());
            s.turn = stream::Turn::default();
            if v["status"] == "running" {
                s.streaming = true;
                s.chat.run_started = Some(std::time::Instant::now());
                let rx = s.backend.stream(json!({"session_id":s.session,"reconnect":true}));
                s.listen_turn(rx, w, cx);
            }
        });
    }
    fn field(
        &mut self,
        key: &str,
        value: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.fields.get(key) {
            return input.clone();
        }
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value.to_owned())
                .placeholder(placeholder.to_owned())
        });
        self.subscriptions
            .push(cx.observe(&state, |_, _, cx| cx.notify()));
        self.fields.insert(key.into(), state.clone());
        state
    }
    fn value(&self, key: &str, cx: &App) -> String {
        self.fields
            .get(key)
            .map(|v| v.read(cx).value().to_string())
            .unwrap_or_default()
    }
    fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.open
            || self.outbox.sending
            || self.workspace.editing
            || self.conversations.editing.is_some()
            || self.conversations.archive_open
        {
            return;
        }
        self.cancel_chat_edit(window, cx);
        self.chat.frozen_preview.clear();
        self.chat.motion = process::ProcessMotion::default();
        self.chat.scroll_paused = false;
        self.streaming = false;
        self.chat.run_started = None;
        self.outbox.menu = None;
        self.outbox.editing = None;
        self.outbox.editor = None;
        self.outbox.send_hover = false;
        self.stash_draft(cx);
        let (draft, attachments) = self.drafts.remove(chat::NEW_DRAFT).unwrap_or_default();
        self.selected = None;
        self.session = uuid::Uuid::new_v4().to_string();
        self.user = "default".into();
        self.channel = "console".into();
        self.history.clear();
        self.turn = stream::Turn::default();
        self.page = Page::Chat;
        self.menu = None;
        self.attachments = attachments;
        self.chat.loading = false;
        self.chat.load_error = false;
        self.interactions.approvals.clear();
        self.interactions.questions.clear();
        self.notice.clear();
        self.search_open = false;
        self.epoch += 1;
        self.chat_epoch += 1;
        self.composer.update(cx, |s, cx| {
            s.set_value(draft, window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }
    fn stash_draft(&mut self, cx: &App) {
        self.drafts.insert(
            chat::draft_key(
                &self.session,
                self.selected.is_none() && self.history.is_empty(),
            )
            .into(),
            (
                self.composer.read(cx).value().to_string(),
                self.attachments.clone(),
            ),
        );
    }
    fn select_chat(&mut self, chat: Value, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.open
            || self.outbox.sending
            || self.workspace.editing
            || self.conversations.editing.is_some()
            || self.conversations.archive_open
        {
            return;
        }
        if self.session == string(&chat, "session_id") {
            self.page = Page::Chat;
            self.search_open = false;
            cx.notify();
            return;
        }
        self.cancel_chat_edit(window, cx);
        self.chat.frozen_preview.clear();
        self.chat.motion = process::ProcessMotion::default();
        self.chat.scroll_paused = false;
        self.streaming = false;
        self.chat.run_started = None;
        self.outbox.menu = None;
        self.outbox.editing = None;
        self.outbox.editor = None;
        self.outbox.send_hover = false;
        self.stash_draft(cx);
        self.selected = Some(string(&chat, "id"));
        self.session = string(&chat, "session_id");
        self.user = chat["user_id"].as_str().unwrap_or("default").into();
        self.channel = chat["channel"].as_str().unwrap_or("console").into();
        self.history.clear();
        self.menu = None;
        self.interactions.approvals.clear();
        self.interactions.questions.clear();
        self.turn = stream::Turn::default();
        self.page = Page::Chat;
        self.search_open = false;
        self.notice.clear();
        let (draft, attachments) = self.drafts.get(&self.session).cloned().unwrap_or_default();
        self.attachments = attachments;
        self.composer
            .update(cx, |s, cx| s.set_value(draft, window, cx));
        self.epoch += 1;
        self.chat_epoch += 1;
        let epoch = self.chat_epoch;
        self.chat.loading = true;
        self.chat.load_error = false;
        self.request_result(
            "GET",
            &format!("/api/chats/{}", segment(&string(&chat, "id"))),
            Value::Null,
            window,
            cx,
            move |s, result, w, cx| {
                if s.chat_epoch == epoch {
                    s.chat.loading = false;
                    match result {
                        Ok(v) => {
                            s.history = array(v["messages"].clone());
                            if v["status"] == "running" {
                                s.streaming = true;
                                s.chat.run_started = Some(std::time::Instant::now());
                                let rx = s
                                    .backend
                                    .stream(json!({"session_id":s.session,"reconnect":true}));
                                s.listen_turn(rx, w, cx);
                            }
                        }
                        Err(e) => {
                            s.chat.load_error = true;
                            s.notice = e;
                        }
                    }
                }
            },
        );
        cx.notify();
    }
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.send_mode(false, window, cx);
    }
    fn send_mode(&mut self, immediate: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.effort.saving {
            self.notice = "思考深度正在保存，请稍后发送".into();
            cx.notify();
            return;
        }
        if self.composer.update(cx, |state, cx| {
            state.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        if self.busy
            || self.chat.loading
            || self.chat.load_error
            || self.voice.active
            || self.settings.open
            || self.workspace.editing
            || self.conversations.editing.is_some()
            || self.conversations.archive_open
            || self.menu.is_some()
        {
            return;
        }
        if self.selected.as_ref().is_some_and(|id| {
            self.chats
                .iter()
                .any(|c| c["id"] == *id && c["archived"] == true)
        }) {
            self.notice = "请先恢复归档会话，再发送消息".into();
            return;
        }
        let text = self.composer.read(cx).value().to_string();
        if text.trim().is_empty() && self.attachments.is_empty() {
            return;
        }
        if self.model["model"].as_str().is_none_or(str::is_empty) {
            self.notice = "请先在设置中配置服务商并选择模型".into();
            self.open_settings(window, cx);
            return;
        }
        if self.running() || !self.queue_items().is_empty() {
            self.enqueue(immediate, window, cx);
            return;
        }
        if self.selected.is_none() && self.history.is_empty() {
            self.drafts.remove(chat::NEW_DRAFT);
        }
        let body = backend::request_body(
            &self.session,
            &self.user,
            &self.channel,
            &text,
            self.attachments.clone(),
        );
        self.history.push(json!({"id":uuid::Uuid::new_v4().to_string(),"role":"user","content":body["input"][0]["content"]}));
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.attachments.clear();
        if let Some((draft, attachments)) = self.chat.edit_backup.take() {
            self.composer
                .update(cx, |s, cx| s.set_value(draft, window, cx));
            self.attachments = attachments;
        }
        self.notice.clear();
        self.turn = stream::Turn::default();
        self.chat.scroll_paused = false;
        self.chat.run_started = Some(std::time::Instant::now());
        self.streaming = true;
        self.page = Page::Chat;
        let rx = self.backend.stream(body);
        self.listen_turn(rx, window, cx);
    }
    fn listen_turn(
        &mut self,
        mut rx: futures::channel::mpsc::Receiver<backend::Reply>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.session.clone();
        let disconnected_session = session.clone();
        let epoch = self.chat_epoch;
        cx.spawn_in(window, async move |this, cx| {
            while let Some(frame) = rx.next().await {
                let terminal = this
                    .update_in(cx, |s, window, cx| {
                        if s.session != session || s.chat_epoch != epoch {
                            // Dropping this listener leaves the core run alive for replay.
                            return true;
                        }
                        match frame {
                            Ok(v) => {
                                if v["object"] == "message" {
                                    s.history.retain(|m| m["id"] != v["id"]);
                                }
                                s.turn.apply(v)
                            }
                            Err(e) => {
                                s.turn.error = Some(e);
                                s.turn.status = "failed".into();
                            }
                        }
                        if let Some(e) = &s.turn.error {
                            s.notice = e.clone();
                        }
                        let terminal = s.turn.terminal();
                        if terminal {
                            s.finish_process();
                            s.streaming = false;
                            s.history.append(&mut s.turn.messages);
                            s.refresh(window, cx);
                        }
                        cx.notify();
                        terminal
                    })
                    .unwrap_or(true);
                if terminal {
                    return;
                }
            }
            let _ = this.update_in(cx, |s, _, cx| {
                if s.session != disconnected_session || s.chat_epoch != epoch {
                    return;
                }
                s.turn.status = "incomplete".into();
                s.finish_process();
                s.streaming = false;
                s.history.append(&mut s.turn.messages);
                s.notice = "连接已结束，请重新加载会话确认后台状态".into();
                cx.notify();
            });
        })
        .detach();
    }
    fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.queue_action("pause", "", window, cx);
        let session = self.session.clone();
        self.request(
            "GET",
            "/api/chats",
            Value::Null,
            window,
            cx,
            move |s, v, window, cx| {
                if let Some(chat) = array(v).iter().find(|c| c["session_id"] == session) {
                    s.request(
                        "POST",
                        &format!(
                            "/api/console/chat/stop?chat_id={}",
                            segment(&string(chat, "id"))
                        ),
                        Value::Null,
                        window,
                        cx,
                        |_, _, _, _| {},
                    );
                }
            },
        );
    }
    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_theme(
            ThemePreference::from_preferences(&self.preferences).next(),
            window,
            cx,
        );
    }
    fn set_theme(&mut self, mode: ThemePreference, window: &mut Window, cx: &mut Context<Self>) {
        self.dark = mode.is_dark(window.appearance());
        mode.save(&mut self.preferences);
        design::apply(self.dark, Some(window), cx);
        cx.notify();
        self.request(
            "PUT",
            "/api/native/preferences",
            self.preferences.clone(),
            window,
            cx,
            |_, _, _, _| {},
        );
    }
    fn pick_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session.clone();
        cx.spawn_in(window, async move |this, cx| {
            if let Some(files) = rfd::AsyncFileDialog::new().pick_files().await {
                let paths = files
                    .iter()
                    .map(|f| f.path().to_owned())
                    .collect::<Vec<_>>();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        paths
                            .iter()
                            .map(|p| {
                                potato_core::attachments::upload_path(p).map_err(|e| e.message)
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .await;
                let _ = this.update_in(cx, |s, _, cx| {
                    match result {
                        Ok(v) if s.session == session => s.attachments.extend(v),
                        Ok(v) => s.drafts.entry(session).or_default().1.extend(v),
                        Err(e) => s.notice = e,
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_owned()
}
fn array(v: Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}
fn main() -> anyhow::Result<()> {
    // Run the packaged executable without a display server on CI. Require an
    // isolated data directory so this probe cannot run a user's scheduled jobs.
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--startup-smoke")) {
        let report = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing smoke report path"))?;
        anyhow::ensure!(
            std::env::var_os("POTATO_NATIVE_DATA_DIR").is_some(),
            "smoke requires POTATO_NATIVE_DATA_DIR"
        );
        let backend = Backend::open()?;
        let preferences = backend.executor.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(20),
                backend.request("GET", "/api/native/preferences", Value::Null),
            )
            .await
        })??;
        let preferences = preferences.map_err(anyhow::Error::msg)?;
        anyhow::ensure!(preferences.is_object(), "invalid initial preferences");
        let computer = backend
            .executor
            .block_on(backend.request("GET", "/api/computer-use", Value::Null))?
            .map_err(anyhow::Error::msg)?;
        std::fs::write(
            report,
            serde_json::to_vec(&json!({
                "ok": true, "version": env!("CARGO_PKG_VERSION"),
                "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
                "computer_driver_available": computer["driver_available"],
                "computer_driver_version": computer["driver_version"],
            }))?,
        )?;
        return Ok(());
    }
    let backend = Backend::open()?;
    let saved = backend
        .executor
        .block_on(backend.request("GET", "/api/native/preferences", Value::Null))
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    gpui_kit::application()
        .with_assets(icons::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let cleanup = backend.clone();
            cx.on_app_quit(move |_| {
                let core = cleanup.core.clone();
                let task = cleanup.executor.spawn(async move {
                    core.cancel_mcp().await;
                    core.cancel_computer().await;
                });
                async move {
                    let _ = task.await;
                }
            })
            .detach();
            design::apply(saved["dark"].as_bool().unwrap_or(false), None, cx);
            cx.bind_keys([
                KeyBinding::new("cmd-n", NewChat, None),
                KeyBinding::new("cmd-k", Search, None),
                KeyBinding::new("cmd-,", Settings, None),
                KeyBinding::new("cmd-b", ToggleSidebar, None),
                KeyBinding::new("ctrl-n", NewChat, None),
                KeyBinding::new("ctrl-k", Search, None),
                KeyBinding::new("ctrl-,", Settings, None),
                KeyBinding::new("ctrl-b", ToggleSidebar, None),
                KeyBinding::new("escape", Dismiss, None),
                KeyBinding::new("alt-down", ShowSendOptions, Some("Potato")),
                KeyBinding::new("cmd-q", Quit, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        window_preferences::restored_size(
                            &saved,
                            cx.primary_display()
                                .map(|d| d.bounds().size)
                                .unwrap_or(size(px(1800.), px(1200.))),
                        ),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Potato".into()),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(800.), px(580.))),
                    ..Default::default()
                },
                |window, cx| {
                    let dark = ThemePreference::from_preferences(&saved).is_dark(window.appearance());
                    design::apply(dark, Some(window), cx);
                    let view = cx.new(|cx| Potato::new(backend, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("open Potato window");
            cx.activate(true);
        });
    Ok(())
}
