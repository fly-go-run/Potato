//! Settings drafts are separate from saved configuration and never logged.
use crate::{App, Message};
use iced::Task;
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[path = "settings_view.rs"]
mod presentation;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Models,
    General,
    Capabilities,
    Security,
    Data,
    Shortcuts,
    About,
}
impl Section {
    fn title(self) -> &'static str {
        match self {
            Self::Models => "模型与服务商",
            Self::General => "通用",
            Self::Capabilities => "能力",
            Self::Security => "安全",
            Self::Data => "数据",
            Self::Shortcuts => "快捷键",
            Self::About => "关于",
        }
    }
}
#[derive(Clone)]
pub enum Destination {
    Section(Section),
    Provider(String),
    List,
    Create,
    Close,
    Model(String),
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Connection,
    Speech,
    DisableSpeech,
    Image,
    Import,
    Create,
    AddModel,
    Test,
    Discover,
    Export,
    Security,
    Activate,
    ClearKey,
    DeleteProvider,
    DeleteModel,
    SaveModel,
    Search,
    ImportHistory,
}
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum NoticeTone {
    #[default]
    Info,
    Success,
    Error,
}
#[derive(Default)]
pub struct Settings {
    providers: Vec<Value>,
    active: Value,
    fields: BTreeMap<&'static str, String>,
    saved: BTreeMap<&'static str, String>,
    section: Section,
    detail: bool,
    creating: bool,
    add_open: bool,
    pending: Option<Destination>,
    confirmation: Option<Operation>,
    editing_model: bool,
    busy: bool,
    loaded: bool,
    notice: String,
    notice_tone: NoticeTone,
    speech_enabled: bool,
    speech_configured: bool,
}
#[derive(Clone)]
pub enum Event {
    Load(Section),
    Loaded(Result<Vec<Value>, String>),
    Navigate(Destination),
    KeepEditing,
    Discard,
    ToggleAdd,
    Field(&'static str, String),
    Run(Operation),
    Confirm(Operation),
    CancelConfirmation,
    Done(Operation, Result<(String, Vec<Value>), String>),
    Refreshed(u64, Result<(Value, Value), String>),
    Appearance(bool),
    FollowSystem,
    RememberWindow(bool),
    ResetWindow,
    Activate(String, String),
}
impl Settings {
    fn configured(&self, id: &str) -> bool {
        self.providers.iter().any(|p| {
            p["id"] == id
                && (p["is_local"] == true || !p["api_key"].as_str().unwrap_or("").is_empty())
        })
    }
    fn is_active(&self, provider: &str, model: &str) -> bool {
        self.active["provider_id"] == provider && self.active["model"] == model
    }
    fn value(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }
    fn put(&mut self, key: &'static str, value: impl Into<String>) {
        self.fields.insert(key, value.into());
    }
    fn dirty(&self) -> bool {
        self.fields != self.saved
    }
    fn checkpoint(&mut self, keys: &[&'static str]) {
        for &key in keys {
            if let Some(v) = self.fields.get(key) {
                self.saved.insert(key, v.clone());
            } else {
                self.saved.remove(key);
            }
        }
    }
    fn accept(&mut self, operation: Operation, notice: String, providers: Vec<Value>) {
        self.providers = providers;
        self.notice = notice;
        self.notice_tone = NoticeTone::Success;
        match operation {
            Operation::Connection => {
                self.put("key", "");
                self.checkpoint(&["url", "key", "protocol"]);
            }
            Operation::Speech => {
                self.put("speech", "");
                self.speech_enabled = true;
                self.speech_configured = true;
                self.checkpoint(&["speech", "app", "resource"]);
            }
            Operation::DisableSpeech => self.speech_enabled = false,
            Operation::Image => self.checkpoint(&["image_provider", "image_model"]),
            Operation::Import => self.checkpoint(&["working", "secret"]),
            Operation::Create => {
                self.put("name", "");
                self.put("url", "");
                self.put("key", "");
                self.put("protocol", "OpenAIChatModel");
                self.saved = self.fields.clone();
                self.creating = false;
                self.add_open = true;
            }
            Operation::AddModel => {
                self.put("model", "");
                self.put("model_name", "");
                self.checkpoint(&["model", "model_name"]);
            }
            Operation::Security => self.checkpoint(&["sandbox", "approval"]),
            Operation::ClearKey => {
                self.put("key", "");
                self.checkpoint(&["key"]);
            }
            Operation::DeleteProvider => {
                self.detail = false;
                self.editing_model = false;
                self.saved = self.fields.clone();
            }
            Operation::DeleteModel | Operation::SaveModel => {
                self.editing_model = false;
                self.checkpoint(&[
                    "edit_model",
                    "edit_name",
                    "max_tokens",
                    "max_input_length",
                    "reasoning_effort",
                ]);
            }
            Operation::ImportHistory => {}
            Operation::Search => {
                self.checkpoint(&["search_backend", "search_provider", "search_model"])
            }
            Operation::Test | Operation::Discover | Operation::Export | Operation::Activate => {}
        }
    }
    fn navigate(&mut self, destination: Destination) -> bool {
        self.notice.clear();
        self.notice_tone = NoticeTone::Info;
        self.editing_model = false;
        self.confirmation = None;
        match destination {
            Destination::Model(id) => {
                let model = self
                    .providers
                    .iter()
                    .find(|p| p["id"] == self.value("id"))
                    .and_then(|p| {
                        ["extra_models", "models"]
                            .iter()
                            .flat_map(|k| p[*k].as_array().into_iter().flatten())
                            .find(|m| m["id"] == id)
                    })
                    .cloned();
                if let Some(model) = model {
                    self.put("edit_model", id);
                    self.put("edit_name", model["name"].as_str().unwrap_or(""));
                    for key in ["max_tokens", "max_input_length"] {
                        self.put(
                            key,
                            model[key]
                                .as_u64()
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        );
                    }
                    self.put(
                        "reasoning_effort",
                        model["reasoning_effort"].as_str().unwrap_or(""),
                    );
                    self.editing_model = true;
                }
            }
            Destination::Close => return true,
            Destination::Section(section) => {
                self.section = section;
                self.detail = false;
                self.creating = false;
            }
            Destination::List => {
                self.detail = false;
                self.creating = false;
            }
            Destination::Create => {
                self.creating = true;
                self.detail = false;
                self.put("name", "");
                self.put("url", "");
                self.put("key", "");
                self.put("protocol", "OpenAIChatModel");
            }
            Destination::Provider(id) => {
                if let Some(provider) = self.providers.iter().find(|p| p["id"] == id).cloned() {
                    self.put("id", id);
                    self.put("url", provider["base_url"].as_str().unwrap_or(""));
                    self.put(
                        "protocol",
                        provider["chat_model"].as_str().unwrap_or("OpenAIChatModel"),
                    );
                    self.put("key", "");
                    self.put("model", "");
                    self.put("model_name", "");
                    self.detail = true;
                    self.creating = false;
                }
            }
        }
        self.saved = self.fields.clone();
        false
    }
    fn apply_loaded(&mut self, values: Vec<Value>) -> Result<(), String> {
        if values.len() < 7 {
            return Err("设置数据不完整，请重新打开设置".into());
        }
        self.providers = values[0].as_array().cloned().ok_or("服务商列表格式错误")?;
        for (key, value) in [
            ("app", values[1]["app_id"].as_str().unwrap_or("")),
            (
                "resource",
                values[1]["resource_id"]
                    .as_str()
                    .unwrap_or("volc.seedasr.sauc.duration"),
            ),
            (
                "image_provider",
                values[2]["image_provider_id"].as_str().unwrap_or(""),
            ),
            (
                "image_model",
                values[2]["image_model"].as_str().unwrap_or("gpt-image-2"),
            ),
            ("working", values[3]["working_dir"].as_str().unwrap_or("")),
            ("secret", values[3]["secret_dir"].as_str().unwrap_or("")),
            (
                "approval",
                values[4]["approval_level"].as_str().unwrap_or("AUTO"),
            ),
            (
                "sandbox",
                values[4]["sandbox_mode"]
                    .as_str()
                    .unwrap_or("workspace-write"),
            ),
            (
                "version",
                values[5]["version"]
                    .as_str()
                    .unwrap_or(env!("CARGO_PKG_VERSION")),
            ),
        ] {
            self.put(key, value);
        }
        if let Some(search) = values.get(7) {
            for (key, source) in [
                ("search_backend", "web_search_backend"),
                ("search_provider", "web_search_provider_id"),
                ("search_model", "web_search_model"),
            ] {
                self.put(
                    key,
                    search[source]
                        .as_str()
                        .unwrap_or(if key == "search_backend" { "auto" } else { "" }),
                );
            }
        }
        if let Some(prefs) = values.get(8) {
            self.put(
                "follow_system",
                (prefs["follow_system"] == true).to_string(),
            );
            self.put(
                "remember_window",
                (prefs["remember_window"] != false).to_string(),
            );
        }
        if let Some(health) = values.get(9) {
            self.put(
                "uptime",
                format!(
                    "{} 分钟",
                    health["uptime_seconds"].as_u64().unwrap_or(0) / 60
                ),
            );
            self.put(
                "health",
                if health["status"] == "ok" {
                    "已就绪"
                } else {
                    "异常"
                },
            );
        }
        self.active = values[6]["active_llm"].clone();
        self.speech_enabled = values[1]["enabled"] == true;
        self.speech_configured = !values[1]["api_key"].as_str().unwrap_or("").is_empty();
        self.put("key", "");
        self.put("speech", "");
        self.saved = self.fields.clone();
        self.loaded = true;
        Ok(())
    }
}
fn encode_segment(value: &str) -> String {
    let mut url = reqwest::Url::parse("http://local/").unwrap();
    url.path_segments_mut().unwrap().push(value);
    url.path().trim_start_matches('/').to_owned()
}
fn model_path(id: &str, action: &str) -> String {
    let mut url = reqwest::Url::parse("http://local/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .extend(["api", "models", id, action]);
    url.path().to_owned()
}
impl App {
    pub fn settings_event(&mut self, event: Event) -> Task<Message> {
        // Navigation and draft protection also work while the core is unavailable.
        let state = &mut self.preferences;
        match event {
            Event::CancelConfirmation => {
                state.confirmation = None;
                return Task::none();
            }
            Event::Confirm(operation) if !state.busy && state.pending.is_none() => {
                state.confirmation = Some(operation);
                return Task::none();
            }
            Event::Confirm(_) => return Task::none(),
            Event::Navigate(destination) if !state.busy => {
                if state.dirty() {
                    state.pending = Some(destination);
                } else if state.navigate(destination) {
                    self.settings = false;
                }
                return iced::widget::operation::snap_to(
                    iced::widget::Id::new("settings-body"),
                    iced::widget::scrollable::RelativeOffset::START,
                );
            }
            Event::KeepEditing => {
                state.pending = None;
                return Task::none();
            }
            Event::Discard if !state.busy => {
                if let Some(destination) = state.pending.take() {
                    state.fields = state.saved.clone();
                    if state.navigate(destination) {
                        self.settings = false;
                    }
                }
                return Task::none();
            }
            Event::Appearance(dark) if !state.busy => {
                self.follow_system = false;
                state.put("follow_system", "false");
                state.checkpoint(&["follow_system"]);
                self.dark = dark;
                self.preferences_dirty = true;
                return Task::none();
            }
            Event::FollowSystem if !state.busy => {
                self.follow_system = true;
                state.put("follow_system", "true");
                state.checkpoint(&["follow_system"]);
                self.preferences_dirty = true;
                return iced::system::theme().map(Message::SystemTheme);
            }
            Event::RememberWindow(remember) if !state.busy => {
                self.remember_window = remember;
                state.put("remember_window", remember.to_string());
                state.checkpoint(&["remember_window"]);
                self.preferences_dirty = true;
                return Task::none();
            }
            Event::ResetWindow if !state.busy => {
                return iced::window::latest().then(|id| {
                    if let Some(id) = id {
                        Task::batch([
                            iced::window::resize(id, iced::Size::new(1080., 760.)),
                            iced::window::move_to(id, iced::Point::new(100., 100.)),
                        ])
                    } else {
                        Task::none()
                    }
                });
            }
            Event::ToggleAdd if !state.busy => {
                state.add_open = !state.add_open;
                return Task::none();
            }
            Event::Field(key, value) if !state.busy && state.pending.is_none() => {
                state.put(key, value);
                state.notice.clear();
                return Task::none();
            }
            Event::Navigate(_)
            | Event::Discard
            | Event::Appearance(_)
            | Event::ToggleAdd
            | Event::Field(..) => return Task::none(),
            _ => {}
        }
        let Some(backend) = self.backend.clone() else {
            state.notice = "本地数据尚未就绪，请重新启动应用".into();
            state.notice_tone = NoticeTone::Error;
            return Task::none();
        };
        match event {
            Event::Activate(id, model) => {
                if state.busy || state.pending.is_some() {
                    return Task::none();
                }
                if self.streaming || !state.configured(&id) {
                    state.notice = if self.streaming {
                        "请等待当前回复结束后再切换模型"
                    } else {
                        "请先保存服务商连接配置，再使用模型"
                    }
                    .into();
                    state.notice_tone = if self.streaming {
                        NoticeTone::Info
                    } else {
                        NoticeTone::Error
                    };
                    return Task::none();
                }
                state.busy = true;
                return Task::perform(
                    async move {
                        backend
                            .request(
                                "PUT",
                                "/api/models/active",
                                json!({"provider_id":id,"model":model}),
                            )
                            .await?;
                        let providers = backend
                            .request("GET", "/api/models", Value::Null)
                            .await?
                            .as_array()
                            .cloned()
                            .ok_or("服务商列表格式错误")?;
                        Ok(("当前模型已更新".into(), providers))
                    },
                    |v| Message::Preferences(Event::Done(Operation::Activate, v)),
                );
            }
            Event::Load(section) => {
                if state.busy || state.dirty() {
                    return Task::none();
                }
                state.busy = true;
                state.notice.clear();
                state.section = section;
                state.detail = false;
                state.creating = false;
                return Task::perform(
                    async move {
                        let mut values = vec![];
                        for path in [
                            "/api/models",
                            "/api/native/doubao-settings",
                            "/api/native/media-settings",
                            "/api/native/legacy-settings",
                            "/api/workspace/running-config",
                            "/api/version",
                            "/api/models/active",
                            "/api/workspace/web-search-backend",
                            "/api/native/preferences",
                            "/api/healthz",
                        ] {
                            values.push(backend.request("GET", path, Value::Null).await?);
                        }
                        Ok(values)
                    },
                    |v| Message::Preferences(Event::Loaded(v)),
                );
            }
            Event::Loaded(result) => {
                state.busy = false;
                if let Err(error) = result.and_then(|v| state.apply_loaded(v)) {
                    state.notice = error;
                    state.notice_tone = NoticeTone::Error;
                }
            }
            Event::Done(operation, result) => {
                state.busy = false;
                match result {
                    Ok((notice, providers)) => state.accept(operation, notice, providers),
                    Err(error) => {
                        state.notice = error;
                        state.notice_tone = NoticeTone::Error;
                        return Task::none();
                    }
                }
                let refresh_chats = if operation == Operation::ImportHistory {
                    self.conversation_event(crate::conversations::Event::Search(
                        self.filter.clone(),
                    ))
                } else {
                    Task::none()
                };
                let generation = self.generation;
                return Task::batch([
                    refresh_chats,
                    Task::perform(
                        async move {
                            let active = backend
                                .request("GET", "/api/models/active", Value::Null)
                                .await?;
                            let config = backend
                                .request("GET", "/api/workspace/running-config", Value::Null)
                                .await?;
                            Ok((active, config))
                        },
                        move |v| Message::Preferences(Event::Refreshed(generation, v)),
                    ),
                ]);
            }
            Event::Refreshed(generation, result) => {
                if generation != self.generation {
                    return Task::none();
                }
                match result {
                    Ok((active, config)) => {
                        state.active = active["active_llm"].clone();
                        self.model = state.active["model"].as_str().map(str::to_owned);
                        self.approval = config["approval_level"].as_str().map(str::to_owned);
                        self.sandbox = config["sandbox_mode"].as_str().map(str::to_owned);
                    }
                    Err(error) => {
                        state.notice = format!("设置已保存，但刷新失败：{error}");
                        state.notice_tone = NoticeTone::Error;
                    }
                }
            }
            Event::Run(operation) => {
                if matches!(
                    operation,
                    Operation::ClearKey | Operation::DeleteProvider | Operation::DeleteModel
                ) {
                    if state.confirmation != Some(operation) {
                        return Task::none();
                    }
                    state.confirmation = None;
                }
                if state.busy || state.pending.is_some() {
                    return Task::none();
                }
                if self.streaming {
                    state.notice = "请等待当前回复结束后再执行此操作；草稿已保留".into();
                    state.notice_tone = NoticeTone::Info;
                    return Task::none();
                }
                state.busy = true;
                state.notice.clear();
                let fields = state.fields.clone();
                return Task::perform(
                    async move {
                        let get = |key: &str| fields.get(key).map(String::as_str).unwrap_or("");
                        let id = get("id");
                        let mut config =
                            json!({"base_url":get("url").trim(),"chat_model":get("protocol")});
                        if !get("key").is_empty() {
                            config["api_key"] = json!(get("key"));
                        }
                        let mut notice = "设置已保存".to_owned();
                        match operation {
                            Operation::ClearKey => {
                                backend
                                    .request(
                                        "PUT",
                                        &model_path(id, "config"),
                                        json!({"api_key":""}),
                                    )
                                    .await?;
                                notice = "已清除保存的 API key".into();
                            }
                            Operation::DeleteProvider => {
                                backend
                                    .request(
                                        "DELETE",
                                        &model_path("custom-providers", id),
                                        Value::Null,
                                    )
                                    .await?;
                                notice = "服务商已删除".into();
                            }
                            Operation::DeleteModel | Operation::SaveModel => {
                                let path = format!(
                                    "{}/{}",
                                    model_path(id, "models"),
                                    encode_segment(get("edit_model"))
                                );
                                if operation == Operation::DeleteModel {
                                    backend.request("DELETE", &path, Value::Null).await?;
                                    notice = "模型已删除".into();
                                } else {
                                    let mut body = json!({"name":get("edit_name").trim(), "reasoning_effort": if get("reasoning_effort").trim().is_empty() { Value::Null } else { json!(get("reasoning_effort").trim()) }});
                                    for key in ["max_tokens", "max_input_length"] {
                                        body[key] = if get(key).trim().is_empty() {
                                            Value::Null
                                        } else {
                                            let n: u64 = get(key)
                                                .trim()
                                                .parse()
                                                .map_err(|_| "Token 上限必须是正整数")?;
                                            if n == 0 || n > 10_000_000 {
                                                return Err(
                                                    "Token 上限需在 1 至 10000000 之间".into()
                                                );
                                            }
                                            json!(n)
                                        };
                                    }
                                    backend.request("PUT", &path, body).await?;
                                    notice = "模型参数已保存".into();
                                }
                            }
                            Operation::Search => {
                                backend.request("PUT", "/api/workspace/web-search-backend", json!({"web_search_backend":get("search_backend"), "web_search_provider_id":get("search_provider"), "web_search_model":get("search_model").trim()})).await?;
                                notice = "联网搜索设置已保存".into();
                            }
                            Operation::ImportHistory => {
                                if let Some(file) = rfd::AsyncFileDialog::new()
                                    .add_filter("历史 JSON", &["json"])
                                    .pick_file()
                                    .await
                                {
                                    if std::fs::metadata(file.path())
                                        .map_err(|_| "无法读取历史文件")?
                                        .len()
                                        > 50_000_000
                                    {
                                        return Err("历史文件超过 50 MB".into());
                                    }
                                    let bytes = file.read().await;
                                    if bytes.len() > 50_000_000 {
                                        return Err("历史文件超过 50 MB".into());
                                    }
                                    let body = serde_json::from_slice(&bytes)
                                        .map_err(|_| "历史文件不是有效 JSON")?;
                                    let value = backend
                                        .request("POST", "/api/native/import-history", body)
                                        .await?;
                                    notice = format!(
                                        "已导入 {} 个会话，跳过 {} 个已有会话",
                                        value["imported"], value["skipped"]
                                    );
                                } else {
                                    notice.clear();
                                }
                            }
                            Operation::Activate => return Err("请选择模型".into()),
                            Operation::Connection => {
                                backend
                                    .request("PUT", &model_path(id, "config"), config)
                                    .await?;
                            }
                            Operation::Test | Operation::Discover => {
                                backend
                                    .request(
                                        "POST",
                                        &model_path(
                                            id,
                                            if operation == Operation::Test {
                                                "test"
                                            } else {
                                                "discover"
                                            },
                                        ),
                                        config,
                                    )
                                    .await?;
                                notice = if operation == Operation::Test {
                                    "连接成功"
                                } else {
                                    "模型列表已更新"
                                }
                                .into();
                            }
                            Operation::Create => {
                                if get("name").trim().is_empty() {
                                    return Err("请填写服务商名称".into());
                                }
                                let id = uuid::Uuid::new_v4().to_string();
                                // Validate and save all connection fields in one core transaction.
                                backend.request("POST", "/api/models/custom-providers", json!({"id":id,"name":get("name").trim(),"default_base_url":get("url").trim(),"chat_model":get("protocol"),"api_key":get("key")})).await?;
                                notice = "服务商已添加".into();
                            }
                            Operation::AddModel => {
                                if get("model").trim().is_empty() {
                                    return Err("请填写模型 ID".into());
                                }
                                backend.request("POST", &model_path(id, "models"), json!({"id":get("model").trim(),"name":if get("model_name").trim().is_empty() {get("model").trim()} else {get("model_name").trim()}})).await?;
                                notice = "模型已添加；可在聊天输入栏选择".into();
                            }
                            Operation::Speech => {
                                let mut body = json!({"enabled":true,"app_id":get("app").trim(),"resource_id":get("resource").trim()});
                                if !get("speech").is_empty() {
                                    body["api_key"] = json!(get("speech"));
                                }
                                backend
                                    .request("PUT", "/api/native/doubao-settings", body)
                                    .await?;
                            }
                            Operation::DisableSpeech => {
                                backend
                                    .request(
                                        "PUT",
                                        "/api/native/doubao-settings",
                                        json!({"enabled":false}),
                                    )
                                    .await?;
                            }
                            Operation::Image => {
                                let mut body = backend
                                    .request("GET", "/api/native/media-settings", Value::Null)
                                    .await?;
                                body["image_provider_id"] = json!(get("image_provider"));
                                body["image_model"] = json!(get("image_model").trim());
                                backend
                                    .request("PUT", "/api/native/media-settings", body)
                                    .await?;
                            }
                            Operation::Import => {
                                let value = backend.request("POST", "/api/native/legacy-settings", json!({"working_dir":get("working"),"secret_dir":get("secret")})).await?;
                                notice = format!("已导入 {} 个连接", value["providers_imported"]);
                            }
                            Operation::Security => {
                                backend
                                    .request(
                                        "PUT",
                                        "/api/workspace/running-config",
                                        json!({"sandbox_mode":get("sandbox"),"approval_level":get("approval")}),
                                    )
                                    .await?;
                            }
                            Operation::Export => {
                                if let Some(file) = rfd::AsyncFileDialog::new()
                                    .set_file_name("potato-workspace.zip")
                                    .save_file()
                                    .await
                                {
                                    use base64::Engine;
                                    let value = backend
                                        .request("GET", "/api/workspace/download", Value::Null)
                                        .await?;
                                    let bytes = base64::engine::general_purpose::STANDARD
                                        .decode(
                                            value["native_binary"]
                                                .as_str()
                                                .ok_or("导出文件格式错误")?,
                                        )
                                        .map_err(|_| "无法读取导出文件")?;
                                    file.write(&bytes).await.map_err(|_| "无法保存导出文件")?;
                                    notice = "工作区已导出".into();
                                } else {
                                    notice.clear();
                                }
                            }
                        }
                        let providers = backend
                            .request("GET", "/api/models", Value::Null)
                            .await?
                            .as_array()
                            .cloned()
                            .ok_or("服务商列表格式错误")?;
                        Ok((notice, providers))
                    },
                    move |v| Message::Preferences(Event::Done(operation, v)),
                );
            }
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Settings {
        let mut s = Settings::default();
        s.apply_loaded(vec![json!([{"id":"a","name":"Provider A","base_url":"https://a.example","chat_model":"OpenAIChatModel","api_key":"********","models":[],"extra_models":[]},{"id":"b","name":"Provider B","base_url":"https://b.example","models":[],"extra_models":[]}]),json!({"enabled":false,"api_key":"********"}),json!({}),json!({}),json!({}),json!({"version":"0.1.0"}),json!({"active_llm":{"provider_id":"a","model":"a-model"}})]).unwrap();
        s
    }
    #[test]
    fn provider_switch_protects_unsaved_credentials() {
        let mut app = App {
            settings: true,
            preferences: settings(),
            ..App::default()
        };
        let _ = app.settings_event(Event::Navigate(Destination::Provider("a".into())));
        let _ = app.settings_event(Event::Field("key", "new-secret".into()));
        let _ = app.settings_event(Event::Navigate(Destination::Provider("b".into())));
        assert_eq!(app.preferences.value("id"), "a");
        assert_eq!(app.preferences.value("key"), "new-secret");
        assert!(app.preferences.pending.is_some());
        let _ = app.settings_event(Event::KeepEditing);
        assert!(app.preferences.pending.is_none());
        assert_eq!(app.preferences.value("key"), "new-secret");
    }
    #[test]
    fn discard_restores_saved_fields_before_navigation() {
        let mut app = App {
            settings: true,
            preferences: settings(),
            ..App::default()
        };
        let _ = app.settings_event(Event::Navigate(Destination::Provider("a".into())));
        let _ = app.settings_event(Event::Field("url", "https://changed.example".into()));
        let _ = app.settings_event(Event::Navigate(Destination::Close));
        assert!(app.settings);
        let _ = app.settings_event(Event::Discard);
        assert!(!app.settings);
        assert_eq!(app.preferences.value("url"), "https://a.example");
        assert!(!app.preferences.dirty());
    }
    #[test]
    fn saving_images_does_not_clear_unsaved_speech_key() {
        let mut s = settings();
        s.put("speech", "unsaved-speech");
        s.put("image_model", "new-image");
        s.accept(Operation::Image, "保存成功".into(), s.providers.clone());
        assert_eq!(s.value("speech"), "unsaved-speech");
        assert!(s.dirty());
        assert_eq!(s.saved.get("image_model").unwrap(), "new-image");
    }
    #[test]
    fn saving_speech_does_not_mark_image_draft_saved() {
        let mut s = settings();
        s.put("speech", "unsaved-speech");
        s.put("image_model", "new-image");
        s.accept(Operation::Speech, "保存成功".into(), s.providers.clone());
        assert_eq!(s.value("speech"), "");
        assert!(s.dirty());
        assert_ne!(s.saved.get("image_model").unwrap(), "new-image");
    }
    #[test]
    fn failed_save_retains_credentials_and_dirty_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            settings: true,
            backend: Some(crate::backend::Backend::open(dir.path()).unwrap()),
            preferences: settings(),
            ..App::default()
        };
        app.preferences.put("key", "unsaved");
        app.preferences.busy = true;
        let _ = app.settings_event(Event::Done(
            Operation::Connection,
            Err("network failed".into()),
        ));
        assert_eq!(app.preferences.value("key"), "unsaved");
        assert!(app.preferences.dirty());
        assert!(!app.preferences.busy);
        assert_eq!(app.preferences.notice_tone, NoticeTone::Error);
    }
    #[test]
    fn mutations_lock_fields_and_close_until_completed() {
        let mut app = App {
            settings: true,
            preferences: settings(),
            ..App::default()
        };
        app.preferences.busy = true;
        let _ = app.settings_event(Event::Field("key", "ignored".into()));
        let _ = app.settings_event(Event::Navigate(Destination::Close));
        assert_eq!(app.preferences.value("key"), "");
        assert!(app.settings);
    }
    #[test]
    fn testing_connection_does_not_save_drafts() {
        let mut s = settings();
        s.put("key", "draft");
        let saved = s.saved.clone();
        s.accept(Operation::Test, "连接成功".into(), s.providers.clone());
        assert_eq!(s.saved, saved);
        assert_eq!(s.value("key"), "draft");
    }
    #[test]
    fn opening_settings_does_not_allow_background_chat_edits() {
        let mut app = App {
            settings: true,
            ..App::default()
        };
        let before = app.session.id.clone();
        let _ = app.update(Message::NewChat);
        let _ = app.update(Message::Submit);
        assert_eq!(app.session.id, before);
        assert!(app.settings);
    }
    #[test]
    fn every_settings_section_and_confirmation_build_at_minimum_window_size() {
        let mut s = settings();
        let theme = crate::ui::theme(false);
        for section in [
            Section::Models,
            Section::General,
            Section::Capabilities,
            Section::Security,
            Section::Data,
            Section::Shortcuts,
            Section::About,
        ] {
            s.navigate(Destination::Section(section));
            let _ = s.overlay(
                iced::widget::text("").into(),
                &theme,
                iced::Size::new(760., 540.),
            );
        }
        s.navigate(Destination::Provider("a".into()));
        let _ = s.overlay(
            iced::widget::text("").into(),
            &theme,
            iced::Size::new(760., 540.),
        );
        s.pending = Some(Destination::Close);
        let _ = s.overlay(
            iced::widget::text("").into(),
            &theme,
            iced::Size::new(760., 540.),
        );
    }
    #[test]
    fn active_model_is_scoped_to_provider_and_local_connections_need_no_key() {
        let mut s = settings();
        assert!(s.is_active("a", "a-model"));
        assert!(!s.is_active("b", "a-model"));
        assert!(s.configured("a"));
        assert!(!s.configured("b"));
        s.providers.push(json!({"id":"local","is_local":true}));
        assert!(s.configured("local"));
    }
    #[test]
    fn config_refresh_preserves_chat_view_and_inflight_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            backend: Some(crate::backend::Backend::open(dir.path()).unwrap()),
            preferences: settings(),
            busy: true,
            status: "loading chat".into(),
            selected: Some("filtered".into()),
            chats: vec![serde_json::from_value(json!({"id":"filtered","name":"Match"})).unwrap()],
            ..App::default()
        };
        app.conversations.archived = true;
        let _ = app.settings_event(Event::Refreshed(
            app.generation,
            Ok((
                json!({"active_llm":{"provider_id":"a","model":"new"}}),
                json!({"approval_level":"STRICT","sandbox_mode":"read-only"}),
            )),
        ));
        assert!(app.busy);
        assert_eq!(app.status, "loading chat");
        assert_eq!(app.selected.as_deref(), Some("filtered"));
        assert_eq!(app.chats.len(), 1);
        assert_eq!(app.chats[0].id, "filtered");
        assert!(app.conversations.archived);
        assert_eq!(app.model.as_deref(), Some("new"));
        assert!(app.preferences.is_active("a", "new"));
    }
    #[test]
    fn blocked_operations_explain_why_and_keep_drafts() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            backend: Some(crate::backend::Backend::open(dir.path()).unwrap()),
            preferences: settings(),
            streaming: true,
            ..App::default()
        };
        app.preferences.put("key", "draft");
        let _ = app.settings_event(Event::Run(Operation::Connection));
        assert!(app.preferences.notice.contains("当前回复"));
        assert_eq!(app.preferences.notice_tone, NoticeTone::Info);
        let _ = app.settings_event(Event::Activate("a".into(), "a-model".into()));
        assert_eq!(app.preferences.notice_tone, NoticeTone::Info);
        assert_eq!(app.preferences.value("key"), "draft");
        assert!(!app.preferences.busy);
        app.streaming = false;
        let _ = app.settings_event(Event::Activate("b".into(), "model".into()));
        assert!(app.preferences.notice.contains("先保存"));
        assert!(!app.preferences.busy);
    }
    #[test]
    fn malformed_load_is_a_recoverable_error() {
        assert!(Settings::default().apply_loaded(vec![]).is_err());
    }

    #[test]
    fn direct_settings_entry_keeps_destination_and_protects_existing_draft() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            backend: Some(crate::backend::Backend::open(dir.path()).unwrap()),
            ..App::default()
        };
        let _ = app.update(Message::OpenSettings(Section::Security));
        assert!(app.settings);
        assert!(app.preferences.section == Section::Security);
        assert!(app.preferences.busy);
        app.preferences = settings();
        app.preferences.put("key", "unsaved-key");
        let _ = app.update(Message::OpenSettings(Section::Shortcuts));
        assert!(app.preferences.pending.is_some());
        assert_eq!(app.preferences.value("key"), "unsaved-key");
        let _ = app.settings_event(Event::KeepEditing);
        assert!(app.preferences.pending.is_none());
        assert!(app.preferences.dirty());
    }
}
