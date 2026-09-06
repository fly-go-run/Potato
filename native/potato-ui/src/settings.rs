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
}
#[derive(Default)]
pub struct Settings {
    providers: Vec<Value>,
    fields: BTreeMap<&'static str, String>,
    saved: BTreeMap<&'static str, String>,
    section: Section,
    detail: bool,
    creating: bool,
    add_open: bool,
    pending: Option<Destination>,
    busy: bool,
    loaded: bool,
    notice: String,
    error: bool,
    speech_enabled: bool,
    speech_configured: bool,
}
#[derive(Clone)]
pub enum Event {
    Load,
    Loaded(Result<Vec<Value>, String>),
    Navigate(Destination),
    KeepEditing,
    Discard,
    ToggleAdd,
    Field(&'static str, String),
    Run(Operation),
    Done(Operation, Result<(String, Vec<Value>), String>),
    Appearance(bool),
    Activate(String, String),
}
impl Settings {
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
        self.error = false;
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
            Operation::Security => self.checkpoint(&["sandbox"]),
            Operation::Test | Operation::Discover | Operation::Export | Operation::Activate => {}
        }
    }
    fn navigate(&mut self, destination: Destination) -> bool {
        self.notice.clear();
        self.error = false;
        match destination {
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
        if values.len() != 6 {
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
                "sandbox",
                values[4]["sandbox_mode"].as_str().unwrap_or("read-only"),
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
        self.speech_enabled = values[1]["enabled"] == true;
        self.speech_configured = !values[1]["api_key"].as_str().unwrap_or("").is_empty();
        self.put("key", "");
        self.put("speech", "");
        self.saved = self.fields.clone();
        self.loaded = true;
        Ok(())
    }
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
                self.dark = dark;
                self.preferences_dirty = true;
                return Task::none();
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
            state.error = true;
            return Task::none();
        };
        match event {
            Event::Activate(id, model) => {
                if state.busy || self.streaming {
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
            Event::Load => {
                if state.busy || state.dirty() {
                    return Task::none();
                }
                state.busy = true;
                state.notice.clear();
                state.section = Section::Models;
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
                    state.error = true;
                }
            }
            Event::Done(operation, result) => {
                state.busy = false;
                match result {
                    Ok((notice, providers)) => state.accept(operation, notice, providers),
                    Err(error) => {
                        state.notice = error;
                        state.error = true;
                        return Task::none();
                    }
                }
                let generation = self.generation;
                return Task::perform(backend.bootstrap(), move |v| {
                    Message::Connected(generation, v)
                });
            }
            Event::Run(operation) => {
                if state.busy || self.streaming || state.pending.is_some() {
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
                                        json!({"sandbox_mode":get("sandbox")}),
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
