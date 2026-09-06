#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod accessibility;
mod backend;
mod chat;
mod conversations;
mod hover;
mod interactions;
mod media;
mod menus;
mod pages;
mod rich;
mod settings;
mod stream;
mod ui;
mod voice;

use backend::{Backend, Chat, History, NetworkEvent, Session};
use iced::widget::text_editor;
use iced::{window, Size, Subscription, Task, Theme};

fn app_shortcut(
    key: &iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Message> {
    if !modifiers.command() || modifiers.shift() || modifiers.alt() {
        return None;
    }
    match key.as_ref() {
        iced::keyboard::Key::Character("n" | "N") => Some(Message::NewChat),
        iced::keyboard::Key::Character("k" | "K") => Some(Message::Search),
        iced::keyboard::Key::Character("b" | "B") => Some(Message::Collapse),
        iced::keyboard::Key::Character(",") => Some(Message::Settings),
        iced::keyboard::Key::Character("/") => {
            Some(Message::OpenSettings(settings::Section::Shortcuts))
        }
        _ => None,
    }
}

fn main() -> iced::Result {
    if std::env::args().any(|a| a == "--startup-smoke") {
        let started = std::time::Instant::now();
        let (app, _) = App::boot();
        println!(
            "{}",
            serde_json::json!({"ready":app.connected,"bootstrap_ms":started.elapsed().as_secs_f64()*1000.,"chats":app.chats.len(),"transport":"in-process","model_configured":app.model.is_some()})
        );
        std::process::exit(if app.connected { 0 } else { 1 });
    }

    let initial = App::boot();
    let size = initial.0.window_size;
    let position = initial
        .0
        .window_position
        .map(window::Position::Specific)
        .unwrap_or(window::Position::Centered);
    let initial = std::cell::RefCell::new(Some(initial));
    iced::application(
        move || initial.borrow_mut().take().expect("application boots once"),
        App::update,
        App::view,
    )
    .title("Potato Native")
    .theme(App::theme)
    .subscription(App::subscription)
    .window_size((size.width, size.height))
    .window(iced::window::Settings {
        icon: Some(
            window::icon::from_file_data(
                include_bytes!("../../../console/src-tauri/icons/icon.png"),
                None,
            )
            .expect("bundled Potato icon must be valid"),
        ),
        position,
        visible: !cfg!(target_os = "macos"),
        min_size: Some(iced::Size::new(760.0, 540.0)),
        #[cfg(target_os = "macos")]
        platform_specific: window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },
        ..Default::default()
    })
    .default_font(iced::Font::with_name(if cfg!(target_os = "windows") {
        "Microsoft YaHei"
    } else {
        "PingFang SC"
    }))
    .run()
}

struct App {
    copy_notice_until: Option<std::time::Instant>,
    accessibility_options: Option<Vec<(String, Message)>>,
    follow_system: bool,
    remember_window: bool,
    window_position: Option<iced::Point>,
    pending_drops: Vec<std::path::PathBuf>,
    preferences_dirty: bool,
    preferences_saving: bool,
    background_version: u64,
    conversations: conversations::Conversations,
    pages: pages::Pages,
    voice: voice::Voice,
    attachments: Vec<serde_json::Value>,
    preferences: settings::Settings,
    menus: menus::Menus,
    hovered_message: Option<usize>,
    expanded_tools: std::collections::HashSet<(usize, usize)>,
    interactions: interactions::State,
    streaming: bool,
    accepted: bool,
    stop_pending: bool,
    uncertain: bool,
    connected: bool,
    model: Option<String>,
    approval: Option<String>,
    sandbox: Option<String>,
    session: Session,
    turn: stream::Turn,
    base_messages: Vec<rich::ChatMessage>,
    follow_bottom: bool,
    fullscreen: bool,
    window_size: Size,
    normal_size: Size,
    backend: Option<Backend>,
    chats: Vec<Chat>,
    selected: Option<String>,
    messages: Vec<rich::ChatMessage>,
    draft: text_editor::Content,
    pending_submission: Option<(String, Vec<serde_json::Value>)>,
    editing_backup: Option<(String, Vec<serde_json::Value>)>,
    drafts: std::collections::HashMap<String, chat::Draft>,
    collapsed: bool,
    search: bool,
    filter: String,
    status: String,
    settings: bool,
    dark: bool,
    busy: bool,
    generation: u64,
}

impl Default for App {
    fn default() -> Self {
        Self {
            normal_size: Size::new(1080., 760.),
            copy_notice_until: None,
            accessibility_options: None,
            follow_system: false,
            remember_window: true,
            window_position: None,
            pending_drops: Vec::new(),
            preferences_dirty: false,
            preferences_saving: false,
            background_version: 0,
            conversations: conversations::Conversations::default(),
            pages: pages::Pages::default(),
            voice: voice::Voice::default(),
            attachments: vec![],
            preferences: settings::Settings::default(),
            menus: menus::Menus::default(),
            hovered_message: None,
            expanded_tools: Default::default(),
            interactions: interactions::State::default(),
            streaming: false,
            accepted: false,
            stop_pending: false,
            uncertain: false,
            connected: false,
            model: None,
            approval: None,
            sandbox: None,
            session: Session::default(),
            turn: stream::Turn::default(),
            base_messages: vec![],
            follow_bottom: true,
            fullscreen: false,
            window_size: Size::new(1080., 760.),
            backend: None,
            chats: vec![],
            selected: None,
            messages: vec![],
            draft: text_editor::Content::new(),
            drafts: Default::default(),
            editing_backup: None,
            pending_submission: None,
            collapsed: false,
            search: false,
            filter: String::new(),
            status: "本地应用尚未就绪".into(),
            settings: false,
            dark: false,
            busy: false,
            generation: 0,
        }
    }
}

// Never derive Debug for connection settings: tokens must not enter logs.
#[derive(Clone)]
enum Message {
    Tick,
    AccessibilityFocus(iced::widget::Id),
    AccessibilityScale(f32),
    AccessibilityWindowFocus(bool),
    AccessibilityOptions(Option<Vec<(String, Message)>>),
    AccessibilityChoose(Box<Message>),
    SystemTheme(iced::theme::Mode),
    WindowMoved(iced::Point),
    AccessibilityReady(window::Id),
    PreferencesSaved(Result<(), String>),
    Conversation(conversations::Event),
    Page(pages::Event),
    Voice(voice::Event),
    Media(media::Event),
    Menu(menus::Event),
    Preferences(settings::Event),
    HoverAnswer(usize),
    LeaveAnswer(usize),
    ToggleTool(usize, usize),
    Copy(String),
    Link(String),
    Interaction(interactions::Event),
    WindowChanged(window::Id, Size),
    WindowOpened(window::Id),
    WindowMode(window::Mode),
    DragWindow,
    ToggleFullscreen,
    Edit(text_editor::Action),
    SetDraft(String),
    Collapse,
    Search,
    Unavailable(&'static str),
    Filter(String),
    Settings,
    OpenSettings(settings::Section),
    Theme,
    Submit,
    Steered(u64, String, Result<(), String>),
    ReuseTurn(usize, bool),
    CancelMessageEdit,
    NewChat,
    Stop,
    Reconnect,
    Refresh,
    Scrolled(bool),
    Network(u64, NetworkEvent),
    Stopped(u64, Result<bool, String>),
    Select(String),
    Chats(u64, Result<Vec<Chat>, String>),
    History(u64, Result<History, String>),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UiEvent")
    }
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let mut app = Self::default();
        match Backend::data_dir()
            .and_then(|p| Backend::open(&p))
            .and_then(|backend| {
                backend
                    .initial_connection()
                    .map(|connection| (backend, connection))
            }) {
            Ok((backend, connection)) => {
                app.dark = connection.preferences["dark"] == true;
                app.follow_system = connection.preferences["follow_system"] == true;
                app.remember_window = connection.preferences["remember_window"] != false;
                if app.remember_window {
                    app.window_position = connection.preferences["x"]
                        .as_f64()
                        .zip(connection.preferences["y"].as_f64())
                        .map(|(x, y)| iced::Point::new(x as f32, y as f32));
                }
                app.collapsed = connection.preferences["collapsed"] == true;
                app.window_size = Size::new(
                    connection.preferences["width"].as_f64().unwrap_or(1080.) as f32,
                    connection.preferences["height"].as_f64().unwrap_or(760.) as f32,
                );
                if !app.remember_window {
                    app.window_size = Size::new(1080., 760.);
                }
                app.normal_size = app.window_size;
                if let Some(chat) = connection
                    .chats
                    .iter()
                    .find(|c| connection.preferences["selected"] == c.id)
                {
                    if let Ok(history) = backend.saved_history(chat.id.clone()) {
                        app.selected = Some(chat.id.clone());
                        app.session = Session {
                            id: chat.session_id.clone(),
                            user: chat.user_id.clone(),
                            channel: chat.channel.clone(),
                        };
                        app.messages = history
                            .messages
                            .iter()
                            .map(backend::display_message)
                            .collect();
                        rich::merge_tools(&mut app.messages);
                        app.uncertain = history.status == "running";
                    }
                }
                app.background_version = backend.updates();
                app.backend = Some(backend);
                app.connected = true;
                app.menus.project_name = connection.project_name;
                app.chats = connection.chats;
                app.model = connection.model;
                app.approval = connection.approval;
                app.sandbox = connection.sandbox;
                app.status = if app.model.is_some() {
                    String::new()
                } else {
                    "请在设置中添加供应商并选择模型".into()
                };
            }
            Err(error) => {
                app.status = format!("无法打开本地数据：{error}");
                app.busy = true;
            }
        }
        (app, iced::system::theme().map(Message::SystemTheme))
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::system::theme_changes().map(Message::SystemTheme),
            if self.accessibility_options.is_some() {
                iced::event::listen_with(|event, _, _| {
                    matches!(
                        event,
                        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                            ..
                        })
                    )
                    .then_some(Message::AccessibilityOptions(None))
                })
            } else {
                Subscription::none()
            },
            if self.menus.is_open() && self.accessibility_options.is_none() {
                iced::event::listen_with(|event, _, _| {
                    matches!(
                        event,
                        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                            ..
                        })
                    )
                    .then_some(Message::Menu(menus::Event::Close))
                })
            } else {
                Subscription::none()
            },
            if self.settings && self.accessibility_options.is_none() {
                iced::event::listen_with(|event, _, _| {
                    // The modal consumes keyboard events; Escape still belongs to the panel.
                    matches!(
                        event,
                        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                            ..
                        })
                    )
                    .then_some(Message::Preferences(
                        settings::Event::Navigate(settings::Destination::Close),
                    ))
                })
            } else {
                Subscription::none()
            },
            accessibility::subscription(),
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Tick),
            if self.connected && (self.selected.is_some() || self.streaming || self.uncertain) {
                iced::time::every(std::time::Duration::from_millis(1500))
                    .map(|_| Message::Interaction(interactions::Event::Poll))
            } else {
                Subscription::none()
            },
            window::events().filter_map(|(_, event)| match event {
                window::Event::FileDropped(path) => {
                    Some(Message::Media(media::Event::DropFile(path)))
                }
                window::Event::Moved(position) => Some(Message::WindowMoved(position)),
                window::Event::Focused => Some(Message::AccessibilityWindowFocus(true)),
                window::Event::Unfocused => Some(Message::AccessibilityWindowFocus(false)),
                window::Event::Rescaled(scale) => Some(Message::AccessibilityScale(scale)),
                _ => None,
            }),
            window::resize_events().map(|(id, size)| Message::WindowChanged(id, size)),
            window::open_events().map(Message::WindowOpened),
            iced::event::listen_with(|event, status, _| {
                let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) = event
                else {
                    return None;
                };
                // App commands also work while a text editor owns keyboard focus.
                if let Some(message) = app_shortcut(&key, modifiers) {
                    return Some(message);
                }
                if status == iced::event::Status::Captured {
                    return None;
                }
                let mac_shortcut = cfg!(target_os = "macos")
                    && modifiers.control()
                    && modifiers.logo()
                    && matches!(key.as_ref(), iced::keyboard::Key::Character("f" | "F"));
                let windows_shortcut = !cfg!(target_os = "macos")
                    && key == iced::keyboard::Key::Named(iced::keyboard::key::Named::F11);
                (mac_shortcut || windows_shortcut).then_some(Message::ToggleFullscreen)
            }),
        ])
    }

    fn theme(&self) -> Theme {
        ui::theme(self.dark)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if self.menus.is_open()
            && matches!(
                message,
                Message::Submit | Message::Edit(_) | Message::SetDraft(_)
            )
        {
            return Task::none();
        }
        if self.settings
            && matches!(
                &message,
                Message::Edit(_)
                    | Message::SetDraft(_)
                    | Message::Submit
                    | Message::Select(_)
                    | Message::NewChat
                    | Message::Media(_)
                    | Message::Voice(_)
                    | Message::ReuseTurn(..)
                    | Message::CancelMessageEdit
                    | Message::Collapse
                    | Message::Page(_)
                    | Message::Conversation(_)
                    | Message::Search
                    | Message::Filter(_)
            )
        {
            return Task::none();
        }
        if matches!(
            &message,
            Message::NewChat | Message::Select(_) | Message::Settings | Message::OpenSettings(_)
        ) && !self.pages.leave()
        {
            return Task::none();
        }
        if matches!(
            message,
            Message::OpenSettings(_)
                | Message::Settings
                | Message::Page(_)
                | Message::NewChat
                | Message::Select(_)
        ) {
            self.menus.close();
        }
        match message {
            Message::AccessibilityOptions(options) => {
                self.accessibility_options = options;
                return Task::none();
            }
            Message::AccessibilityChoose(message) => {
                self.accessibility_options = None;
                return self.update(*message);
            }
            Message::SystemTheme(mode) => {
                if self.follow_system {
                    self.dark = mode == iced::theme::Mode::Dark;
                }
                return Task::none();
            }
            Message::WindowMoved(point) => {
                if self.remember_window && !self.fullscreen {
                    self.window_position = Some(point);
                    self.preferences_dirty = true;
                }
                return Task::none();
            }
            Message::SetDraft(value) => {
                if !self.voice.active {
                    self.draft = text_editor::Content::with_text(&value);
                }
                return Task::none();
            }
            Message::AccessibilityWindowFocus(focused) => {
                accessibility::window_focus(focused);
                return Task::none();
            }
            Message::AccessibilityScale(scale) => {
                accessibility::set_scale(scale);
                return Task::none();
            }
            Message::AccessibilityFocus(id) => return iced::widget::operation::focus(id),
            Message::AccessibilityReady(id) => return window::set_mode(id, window::Mode::Windowed),
            Message::Tick => {
                if self
                    .copy_notice_until
                    .is_some_and(|until| std::time::Instant::now() >= until)
                {
                    self.copy_notice_until = None;
                    if self.status == "已复制" {
                        self.status.clear();
                    }
                }
                let Some(backend) = self.backend.clone() else {
                    return Task::none();
                };
                let mut tasks = Vec::new();
                if !self.busy && !self.pending_drops.is_empty() {
                    tasks.push(self.media_event(media::Event::FlushDrops));
                }
                if self.preferences_dirty && !self.preferences_saving {
                    self.preferences_dirty = false;
                    self.preferences_saving = true;
                    let api = backend.clone();
                    let mut body = serde_json::json!({"dark":self.dark,"follow_system":self.follow_system,"remember_window":self.remember_window,"collapsed":self.collapsed,"selected":self.selected.clone().unwrap_or_default()});
                    if self.remember_window {
                        body["width"] =
                            serde_json::json!(self.normal_size.width.clamp(760., 4000.));
                        body["height"] =
                            serde_json::json!(self.normal_size.height.clamp(540., 2400.));
                        if let Some(position) = self.window_position {
                            body["x"] = serde_json::json!(position.x);
                            body["y"] = serde_json::json!(position.y);
                        }
                    }
                    tasks.push(Task::perform(
                        async move {
                            api.request("PUT", "/api/native/preferences", body)
                                .await
                                .map(|_| ())
                        },
                        Message::PreferencesSaved,
                    ));
                }
                if backend.updates() != self.background_version && !self.busy {
                    self.background_version = backend.updates();
                    tasks.push(
                        self.conversation_event(conversations::Event::Search(self.filter.clone())),
                    );
                    if !self.streaming {
                        if let Some(id) = self.selected.clone() {
                            let generation = self.generation;
                            tasks.push(Task::perform(backend.history(id), move |v| {
                                Message::History(generation, v)
                            }));
                        }
                    }
                }
                return Task::batch(tasks);
            }
            Message::PreferencesSaved(result) => {
                self.preferences_saving = false;
                if let Err(e) = result {
                    self.status = format!("偏好保存失败：{e}");
                }
            }
            Message::Conversation(event) => return self.conversation_event(event),
            Message::Page(event) => return self.page_event(event),
            Message::Voice(event) => return self.voice_event(event),
            Message::Menu(event) => return self.menu_event(event),
            Message::Media(event) => return self.media_event(event),
            Message::Preferences(event) => return self.settings_event(event),
            Message::HoverAnswer(index) => self.hovered_message = Some(index),
            Message::LeaveAnswer(index) => {
                if self.hovered_message == Some(index) {
                    self.hovered_message = None;
                }
            }
            Message::ToggleTool(message, tool) => {
                if !self.expanded_tools.remove(&(message, tool)) {
                    self.expanded_tools.insert((message, tool));
                }
            }
            Message::Interaction(event) => return self.interaction(event),
            Message::Copy(value) => {
                self.status = "已复制".into();
                self.copy_notice_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
                return iced::clipboard::write(value);
            }
            Message::Link(value) => match reqwest::Url::parse(&value) {
                Ok(url) if matches!(url.scheme(), "http" | "https") => {
                    if let Err(error) = open::that_detached(url.as_str()) {
                        self.status = format!("无法打开链接：{error}");
                    }
                }
                _ => self.status = "仅支持打开 HTTP/HTTPS 链接".into(),
            },

            Message::Network(generation, event) => return self.network_event(generation, event),
            Message::NewChat if !self.busy && !self.voice.active => {
                self.new_chat();
                return Task::batch([
                    self.conversation_event(conversations::Event::Search(self.filter.clone())),
                    iced::widget::operation::focus(iced::widget::Id::new("composer")),
                ]);
            }
            Message::Refresh if !self.streaming && !self.busy => {
                if let Some(id) = self.selected.clone() {
                    return self.update(Message::Select(id));
                }
                if let Some(backend) = self.backend.clone() {
                    self.busy = true;
                    let generation = self.generation;
                    return Task::perform(backend.chats(), move |result| {
                        Message::Chats(generation, result)
                    });
                }
            }
            Message::Reconnect if !self.streaming && !self.busy && self.connected => {
                return self.start_network(String::new(), true)
            }
            Message::Stop if (self.accepted || self.uncertain) && !self.stop_pending => {
                if let Some(backend) = self.backend.clone() {
                    self.stop_pending = true;
                    let generation = self.generation;
                    return Task::perform(
                        backend.stop(self.session.clone(), self.selected.clone()),
                        move |result| Message::Stopped(generation, result),
                    );
                }
            }
            Message::Stopped(generation, result) if generation == self.generation => match result {
                Ok(true) => {
                    self.status = "停止请求已发送，等待后台确认结束…".into();
                    if !self.streaming {
                        return self.start_network(String::new(), true);
                    }
                }
                Ok(false) => {
                    self.stop_pending = false;
                    self.status = "后台未确认停止，请刷新会话检查状态".into();
                }
                Err(error) => {
                    self.stop_pending = false;
                    self.status = error;
                }
            },
            Message::Scrolled(bottom) => self.follow_bottom = bottom,
            Message::NewChat
            | Message::Refresh
            | Message::Reconnect
            | Message::Stop
            | Message::Stopped(..) => {}
            Message::WindowChanged(id, size) => {
                self.window_size = size;
                return window::mode(id).map(Message::WindowMode);
            }
            Message::WindowOpened(id) => {
                return window::scale_factor(id).then(move |scale| {
                    window::run(id, move |window| {
                        accessibility::set_scale(scale);
                        if let Ok(handle) = window.window_handle() {
                            accessibility::attach(handle.as_raw());
                        }
                        Message::AccessibilityReady(id)
                    })
                });
            }
            Message::WindowMode(mode) => {
                self.fullscreen = mode == window::Mode::Fullscreen;
                if mode == window::Mode::Windowed {
                    self.normal_size = self.window_size;
                    self.preferences_dirty = true;
                }
            }
            Message::ToggleFullscreen => {
                let mode = if self.fullscreen {
                    window::Mode::Windowed
                } else {
                    window::Mode::Fullscreen
                };
                return window::latest().and_then(move |id| window::set_mode(id, mode));
            }
            Message::DragWindow => return window::latest().and_then(window::drag),
            Message::Edit(action) => {
                if !self.voice.active {
                    self.draft.perform(action);
                }
            }
            Message::Collapse => {
                self.collapsed = !self.collapsed;
                self.preferences_dirty = true;
            }
            Message::Search => {
                self.search = !self.search;
                if self.search {
                    self.collapsed = false;
                    self.preferences_dirty = true;
                    return iced::widget::operation::focus(iced::widget::Id::new("chat-search"));
                }
                // A hidden search must not silently keep filtering the conversation list.
                return self.conversation_event(conversations::Event::Search(String::new()));
            }
            Message::Unavailable(name) => self.status = format!("{name}尚未迁移，当前为界面预览"),
            Message::Filter(value) => {
                return self.conversation_event(conversations::Event::Search(value))
            }
            Message::Settings => {
                if self.voice.active {
                    self.status = "请先结束语音录入".into();
                    return Task::none();
                }
                if self.settings {
                    return self
                        .settings_event(settings::Event::Navigate(settings::Destination::Close));
                }
                self.settings = true;
                return self.settings_event(settings::Event::Load(settings::Section::Models));
            }
            Message::OpenSettings(section) => {
                if self.voice.active {
                    self.status = "请先结束语音录入".into();
                    return Task::none();
                }
                if self.settings {
                    return self.settings_event(settings::Event::Navigate(
                        settings::Destination::Section(section),
                    ));
                }
                self.settings = true;
                return self.settings_event(settings::Event::Load(section));
            }
            Message::Theme => {
                self.follow_system = false;
                self.dark = !self.dark;
                self.preferences_dirty = true;
            }
            Message::ReuseTurn(index, send) => return self.reuse_message(index, send),
            Message::CancelMessageEdit => self.cancel_message_edit(),
            Message::Steered(generation, text, result) if generation == self.generation => {
                self.busy = false;
                match result {
                    Ok(()) => {
                        if self.composer_text().trim() == text {
                            self.draft = text_editor::Content::new();
                        }
                        self.status = "补充指令已送达，将在当前步骤结束后采用".into();
                    }
                    Err(error) => self.status = error,
                }
            }
            Message::Steered(..) => {}
            Message::Submit if self.can_steer() => return self.send_steering(),
            Message::Submit
                if self.backend.is_some()
                    && self.can_send()
                    && (!self.draft.text().trim().is_empty() || !self.attachments.is_empty()) =>
            {
                return self.start_network(self.draft.text().trim().to_owned(), false);
            }
            Message::Submit => {}
            Message::Chats(generation, result) if generation == self.generation => {
                self.busy = false;
                match result {
                    Ok(chats) => {
                        self.selected = chats
                            .iter()
                            .find(|chat| {
                                chat.session_id == self.session.id
                                    && chat.user_id == self.session.user
                                    && chat.channel == self.session.channel
                            })
                            .map(|chat| chat.id.clone())
                            .or_else(|| self.selected.clone());
                        self.preferences_dirty = true;
                        self.chats = chats;
                        if !self.streaming && self.uncertain {
                            self.status =
                                "后台状态未确认：可打开会话刷新或重连，请勿重复发送".into();
                        }
                    }
                    Err(error) => self.status = error,
                }
            }
            Message::Select(id) if !self.busy && !self.voice.active => {
                if let Some(backend) = self.backend.clone() {
                    self.save_draft();
                    self.pending_submission = None;
                    self.streaming = false;
                    self.accepted = false;
                    self.stop_pending = false;
                    self.base_messages.clear();
                    self.hovered_message = None;
                    self.expanded_tools.clear();
                    self.interactions = interactions::State::default();
                    self.generation += 1;
                    let generation = self.generation;
                    if let Some(chat) = self.chats.iter().find(|chat| chat.id == id) {
                        self.session = Session {
                            id: chat.session_id.clone(),
                            user: chat.user_id.clone(),
                            channel: chat.channel.clone(),
                        };
                    }
                    self.selected = Some(id.clone());
                    self.preferences_dirty = true;
                    self.uncertain = true;
                    self.turn = stream::Turn::default();
                    self.messages.clear();
                    self.restore_draft();
                    self.busy = true;
                    self.status.clear();
                    return Task::perform(backend.history(id), move |result| {
                        Message::History(generation, result)
                    });
                }
            }
            Message::History(generation, result) if generation == self.generation => {
                self.busy = false;
                match result {
                    Ok(history) => {
                        self.messages = history
                            .messages
                            .iter()
                            .map(backend::display_message)
                            .collect();
                        rich::merge_tools(&mut self.messages);
                        self.hovered_message = None;
                        self.expanded_tools.clear();
                        if let Some(chat) = self
                            .chats
                            .iter_mut()
                            .find(|chat| Some(&chat.id) == self.selected.as_ref())
                        {
                            chat.status = history.status.clone();
                        }
                        self.uncertain = history.status == "running";
                        self.status = if self.uncertain {
                            "后台正在运行，可重新连接查看输出或停止任务".into()
                        } else {
                            self.turn.error.clone().unwrap_or_default()
                        };
                        if self.follow_bottom {
                            return iced::widget::operation::snap_to(
                                iced::widget::Id::new("messages"),
                                iced::widget::scrollable::RelativeOffset::END,
                            );
                        }
                    }
                    Err(error) => self.status = error,
                }
            }
            Message::Select(_) | Message::Chats(..) | Message::History(..) => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_commands_preserve_drafts_and_modal_isolation() {
        use iced::keyboard::{Key, Modifiers};
        let command = if cfg!(target_os = "macos") {
            Modifiers::LOGO
        } else {
            Modifiers::CTRL
        };
        let key = |s: &str| Key::Character(s.into());
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            backend: Some(Backend::open(dir.path()).unwrap()),
            draft: text_editor::Content::with_text("保留草稿"),
            collapsed: true,
            ..App::default()
        };
        let _ = app.update(app_shortcut(&key("k"), command).unwrap());
        assert!(app.search);
        assert!(!app.collapsed);
        app.filter = "temporary filter".into();
        let _ = app.update(app_shortcut(&key("k"), command).unwrap());
        assert!(!app.search);
        assert!(app.filter.is_empty());
        let _ = app.update(app_shortcut(&key("b"), command).unwrap());
        assert!(app.collapsed);
        assert_eq!(app.draft.text(), "保留草稿");
        app.settings = true;
        let session = app.session.id.clone();
        for character in ["n", "k", "b"] {
            let _ = app.update(app_shortcut(&key(character), command).unwrap());
        }
        assert_eq!(app.session.id, session);
        assert!(!app.search);
        assert!(app.collapsed);
        assert_eq!(app.draft.text(), "保留草稿");
        assert!(app_shortcut(&key("n"), Modifiers::empty()).is_none());
        assert!(app_shortcut(&key("n"), command | Modifiers::SHIFT).is_none());
    }

    #[test]
    fn switching_to_demo_discards_late_network_results() {
        let mut app = App::default();
        let _ = app.update(Message::NewChat);
        let _ = app.update(Message::History(
            0,
            Ok(History {
                messages: vec![serde_json::json!({"content":"stale"})],
                status: "idle".into(),
            }),
        ));
        assert!(app.messages.is_empty());
    }
}
