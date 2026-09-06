//! Native workspace screens. Requests carry an epoch so old results cannot replace an editor.
use crate::{App, Message};
use base64::{engine::general_purpose::STANDARD, Engine};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_editor, text_input, Column,
};
use iced::{Element, Fill, Task};
use serde_json::{json, Value};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Memory,
    Workspace,
    Skills,
    Tasks,
}
impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Memory => "记忆",
            Self::Workspace => "角色与工作区",
            Self::Skills => "技能",
            Self::Tasks => "定时任务",
        }
    }
    fn path(self) -> &'static str {
        match self {
            Self::Memory => "/api/workspace/memory",
            Self::Workspace => "/api/workspace/files",
            Self::Skills => "/api/skills",
            Self::Tasks => "/api/cron/jobs",
        }
    }
}
#[derive(Default)]
pub struct Pages {
    pub kind: Option<Kind>,
    epoch: u64,
    list: Vec<Value>,
    selected: Option<String>,
    name: String,
    editor: text_editor::Content,
    expected: Value,
    dirty: bool,
    busy: bool,
    notice: String,
    delete_confirm: bool,
    task: Value,
    history: Vec<Value>,
    schedule: String,
    time: String,
    zone: String,
    task_type: String,
}
#[derive(Clone)]
pub enum Event {
    Open(Kind),
    Back,
    Load,
    Loaded(u64, Result<Vec<Value>, String>),
    Select(String),
    Selected(u64, Result<Value, String>),
    New,
    Name(String),
    Edit(text_editor::Action),
    Save,
    Delete,
    ConfirmDelete,
    CancelDelete,
    Toggle(String, bool),
    Import,
    TaskAction(String, &'static str),
    Done(u64, Result<(), String>),
    Field(&'static str, String),
    Discard,
}
fn segment(value: &str) -> String {
    let mut url = reqwest::Url::parse("http://local/").unwrap();
    url.path_segments_mut().unwrap().push(value);
    url.path()[1..].to_owned()
}
fn item_path(kind: Kind, id: &str) -> String {
    format!(
        "{}/{}{}",
        kind.path(),
        segment(id),
        if kind == Kind::Skills { "/content" } else { "" }
    )
}
impl App {
    pub fn page_event(&mut self, event: Event) -> Task<Message> {
        let Some(backend) = self.backend.clone() else {
            return Task::none();
        };
        let state = &mut self.pages;
        match event {
            Event::Open(kind) => {
                if state.dirty || state.busy {
                    state.notice = "请先保存或放弃当前编辑".into();
                    return Task::none();
                }
                if self.voice.active {
                    self.status = "请先结束语音录入".into();
                    return Task::none();
                }
                let epoch = state.epoch + 1;
                *state = Pages {
                    kind: Some(kind),
                    epoch,
                    schedule: "每天".into(),
                    time: "09:00".into(),
                    zone: "Asia/Shanghai".into(),
                    task_type: "提醒".into(),
                    ..Pages::default()
                };
                self.settings = false;
                return self.page_event(Event::Load);
            }
            Event::Back => {
                if state.dirty || state.busy {
                    state.notice = "请先保存或放弃当前编辑".into();
                } else {
                    state.kind = None;
                    state.epoch += 1;
                }
            }
            Event::Discard => {
                if !state.busy {
                    state.dirty = false;
                    state.notice.clear();
                    if let Some(id) = state.selected.clone() {
                        return self.page_event(Event::Select(id));
                    }
                    state.editor = text_editor::Content::new();
                }
            }
            Event::Load => {
                let Some(kind) = state.kind else {
                    return Task::none();
                };
                if state.busy {
                    return Task::none();
                };
                state.busy = true;
                let epoch = state.epoch;
                return Task::perform(
                    async move {
                        let v = backend.request("GET", kind.path(), Value::Null).await?;
                        serde_json::from_value(v).map_err(|_| "列表格式不兼容".into())
                    },
                    move |v| Message::Page(Event::Loaded(epoch, v)),
                );
            }
            Event::Loaded(epoch, result) if epoch == state.epoch => {
                state.busy = false;
                match result {
                    Ok(v) => state.list = v,
                    Err(e) => state.notice = e,
                }
            }
            Event::Select(id) => {
                if state.dirty || state.busy {
                    state.notice = "请先保存或放弃当前编辑".into();
                    return Task::none();
                };
                let Some(kind) = state.kind else {
                    return Task::none();
                };
                state.epoch += 1;
                let epoch = state.epoch;
                state.selected = Some(id.clone());
                state.busy = true;
                state.delete_confirm = false;
                state.history.clear();
                return Task::perform(
                    async move {
                        let mut v = backend
                            .request("GET", &item_path(kind, &id), Value::Null)
                            .await?;
                        if kind == Kind::Tasks {
                            v["execution_state"] = backend
                                .request(
                                    "GET",
                                    &format!("{}/state", item_path(kind, &id)),
                                    Value::Null,
                                )
                                .await?;
                            v["execution_history"] = backend
                                .request(
                                    "GET",
                                    &format!("{}/history", item_path(kind, &id)),
                                    Value::Null,
                                )
                                .await?;
                        }
                        Ok(v)
                    },
                    move |v| Message::Page(Event::Selected(epoch, v)),
                );
            }
            Event::Selected(epoch, result) if epoch == state.epoch => {
                state.busy = false;
                match result {
                    Ok(v) => {
                        state.dirty = false;
                        state.notice.clear();
                        if state.kind == Some(Kind::Tasks) {
                            state.name = v["name"].as_str().unwrap_or_default().into();
                            state.task_type = if v["task_type"] == "agent" {
                                "模型任务"
                            } else {
                                "提醒"
                            }
                            .into();
                            let content = if state.task_type == "提醒" {
                                v["text"].as_str().unwrap_or_default().to_owned()
                            } else {
                                v["request"]["input"][0]["content"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|b| b["text"].as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            };
                            state.editor = text_editor::Content::with_text(&content);
                            state.schedule = if v["schedule"]["type"] == "once" {
                                "单次"
                            } else {
                                "Cron"
                            }
                            .into();
                            state.time = v["schedule"][if v["schedule"]["type"] == "once" {
                                "run_at"
                            } else {
                                "cron"
                            }]
                            .as_str()
                            .unwrap_or_default()
                            .into();
                            state.zone = v["schedule"]["timezone"]
                                .as_str()
                                .unwrap_or("Asia/Shanghai")
                                .into();
                            if state.schedule == "单次" {
                                if let (Ok(date), Ok(zone)) = (
                                    chrono::DateTime::parse_from_rfc3339(&state.time),
                                    state.zone.parse::<chrono_tz::Tz>(),
                                ) {
                                    state.time = date
                                        .with_timezone(&zone)
                                        .format("%Y-%m-%d %H:%M")
                                        .to_string();
                                }
                            }
                            state.history = v["execution_history"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default();
                            state.task = v;
                        } else {
                            state.name = state.selected.clone().unwrap_or_default();
                            state.expected = v["content"].clone();
                            state.editor = text_editor::Content::with_text(
                                v["content"].as_str().unwrap_or_default(),
                            );
                        }
                    }
                    Err(e) => state.notice = e,
                }
            }
            Event::New => {
                if state.dirty || state.busy {
                    state.notice = "请先保存或放弃当前编辑".into();
                    return Task::none();
                };
                state.selected = None;
                state.name.clear();
                state.editor = text_editor::Content::new();
                state.expected = Value::Null;
                state.history.clear();
                state.task = Value::Null;
                state.epoch += 1;
                state.schedule = "每天".into();
                state.time = "09:00".into();
                state.zone = "Asia/Shanghai".into();
                state.task_type = "提醒".into();
                state.notice.clear();
            }
            Event::Name(value) if !state.busy => {
                state.name = value;
                state.dirty = true;
            }
            Event::Edit(action) if !state.busy => {
                let old = state.editor.text();
                state.editor.perform(action);
                state.dirty |= state.editor.text() != old;
            }
            Event::Field(field, value) if !state.busy => {
                match field {
                    "schedule" => state.schedule = value,
                    "time" => state.time = value,
                    "zone" => state.zone = value,
                    "type" => state.task_type = value,
                    _ => {}
                }
                state.dirty = true;
            }
            Event::Save => {
                if state.busy {
                    return Task::none();
                };
                let Some(kind) = state.kind else {
                    return Task::none();
                };
                let name = state.name.trim().to_owned();
                if name.is_empty() {
                    state.notice = "请填写名称".into();
                    return Task::none();
                }
                let content = state.editor.text();
                let expected = state.expected.clone();
                let selected = state.selected.clone();
                let task = if kind == Kind::Tasks {
                    match task_spec(state, &self.session.id) {
                        Ok(v) => v,
                        Err(e) => {
                            state.notice = e;
                            return Task::none();
                        }
                    }
                } else {
                    Value::Null
                };
                state.busy = true;
                let epoch = state.epoch;
                return Task::perform(
                    async move {
                        if kind == Kind::Tasks {
                            backend
                                .request(
                                    if selected.is_some() { "PUT" } else { "POST" },
                                    &selected
                                        .as_ref()
                                        .map(|id| item_path(kind, id))
                                        .unwrap_or(kind.path().into()),
                                    task,
                                )
                                .await?;
                        } else if kind == Kind::Skills && selected.is_none() {
                            backend
                                .request(
                                    "POST",
                                    kind.path(),
                                    json!({"name":name,"content":content}),
                                )
                                .await?;
                        } else {
                            backend
                                .request(
                                    "PUT",
                                    &item_path(kind, selected.as_deref().unwrap_or(&name)),
                                    json!({"content":content,"expected_content":expected}),
                                )
                                .await?;
                        }
                        Ok(())
                    },
                    move |v| Message::Page(Event::Done(epoch, v)),
                );
            }
            Event::Delete => {
                if !state.busy {
                    state.delete_confirm = true;
                }
            }
            Event::CancelDelete => state.delete_confirm = false,
            Event::ConfirmDelete => {
                if state.busy {
                    return Task::none();
                };
                let (Some(kind), Some(id)) = (state.kind, state.selected.clone()) else {
                    return Task::none();
                };
                state.busy = true;
                let epoch = state.epoch;
                return Task::perform(
                    async move {
                        backend
                            .request(
                                "DELETE",
                                &format!("{}/{}", kind.path(), segment(&id)),
                                Value::Null,
                            )
                            .await?;
                        Ok(())
                    },
                    move |v| Message::Page(Event::Done(epoch, v)),
                );
            }
            Event::Toggle(id, enabled) => {
                if state.busy || state.dirty {
                    return Task::none();
                };
                state.busy = true;
                let epoch = state.epoch;
                return Task::perform(
                    async move {
                        backend
                            .request(
                                "POST",
                                &format!(
                                    "/api/skills/{}/{}",
                                    segment(&id),
                                    if enabled { "disable" } else { "enable" }
                                ),
                                Value::Null,
                            )
                            .await?;
                        Ok(())
                    },
                    move |v| Message::Page(Event::Done(epoch, v)),
                );
            }
            Event::TaskAction(id, action) => {
                if state.busy || state.dirty {
                    return Task::none();
                };
                state.busy = true;
                let epoch = state.epoch;
                return Task::perform(
                    async move {
                        backend
                            .request(
                                "POST",
                                &format!("/api/cron/jobs/{}/{action}", segment(&id)),
                                Value::Null,
                            )
                            .await?;
                        Ok(())
                    },
                    move |v| Message::Page(Event::Done(epoch, v)),
                );
            }
            Event::Import => {
                if state.busy || state.dirty {
                    return Task::none();
                };
                state.busy = true;
                let epoch = state.epoch;
                return Task::perform(
                    async move {
                        if let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("技能 ZIP", &["zip"])
                            .pick_file()
                            .await
                        {
                            if std::fs::metadata(file.path())
                                .map_err(|_| "无法读取文件")?
                                .len()
                                > 20_000_000
                            {
                                return Err("技能包超过 20 MB".into());
                            }
                            let bytes = file.read().await;
                            if bytes.len() > 20_000_000 {
                                return Err("技能包超过 20 MB".into());
                            }
                            backend.request("POST","/api/skills/upload",json!({"filename":file.file_name(),"base64":STANDARD.encode(bytes)})).await?;
                        }
                        Ok(())
                    },
                    move |v| Message::Page(Event::Done(epoch, v)),
                );
            }
            Event::Done(epoch, result) if epoch == state.epoch => {
                state.busy = false;
                match result {
                    Ok(()) => {
                        state.dirty = false;
                        state.selected = None;
                        state.editor = text_editor::Content::new();
                        state.name.clear();
                        state.delete_confirm = false;
                        state.notice = "操作已完成".into();
                        return self.page_event(Event::Load);
                    }
                    Err(e) => state.notice = e,
                }
            }
            _ => {}
        }
        Task::none()
    }
}
fn task_spec(state: &Pages, session: &str) -> Result<Value, String> {
    let schedule = match state.schedule.as_str() {
        "每天" => {
            let (h, m) = state.time.split_once(':').ok_or("时间格式应为 HH:MM")?;
            let h: u32 = h.parse().map_err(|_| "小时无效")?;
            let m: u32 = m.parse().map_err(|_| "分钟无效")?;
            if h > 23 || m > 59 {
                return Err("时间超出范围".into());
            }
            json!({"type":"cron","cron":format!("{m} {h} * * *"),"timezone":state.zone})
        }
        "单次" => {
            use chrono::TimeZone;
            let zone: chrono_tz::Tz = state.zone.parse().map_err(|_| "时区无效")?;
            let local = chrono::NaiveDateTime::parse_from_str(&state.time, "%Y-%m-%d %H:%M")
                .map_err(|_| "请输入日期和时间，例如 2026-09-07 09:00")?;
            let date = zone
                .from_local_datetime(&local)
                .single()
                .ok_or("该时间处于夏令时跳变区间，请选择其他时间")?;
            json!({"type":"once","run_at":date.to_rfc3339(),"timezone":state.zone})
        }
        "Cron" => json!({"type":"cron","cron":state.time,"timezone":state.zone}),
        _ => return Err("请选择执行时间".into()),
    };
    let mut spec = if state.task.is_object() {
        state.task.clone()
    } else {
        json!({"enabled":true,"dispatch":{"type":"channel","channel":"console","target":{"session_id":session,"user_id":"default"}}})
    };
    spec.as_object_mut().unwrap().remove("execution_state");
    spec.as_object_mut().unwrap().remove("execution_history");
    spec["name"] = json!(state.name.trim());
    spec["schedule"] = schedule;
    if state.task_type == "模型任务" {
        spec["task_type"] = json!("agent");
        spec["request"] = crate::backend::request_body(
            &crate::backend::Session {
                id: session.into(),
                user: "default".into(),
                channel: "console".into(),
            },
            &state.editor.text(),
            false,
        );
    } else {
        spec["task_type"] = json!("text");
        spec["text"] = json!(state.editor.text());
    }
    Ok(spec)
}
impl Pages {
    pub fn leave(&mut self) -> bool {
        if self.dirty || self.busy {
            self.notice = "请先保存或放弃当前编辑".into();
            false
        } else {
            self.kind = None;
            self.epoch += 1;
            true
        }
    }
    pub fn view(&self) -> Element<'_, Message> {
        let kind = self.kind.unwrap();
        let event = |v| Message::Page(v);
        let action = |label: &'static str, e: Event| {
            button(label)
                .padding(8)
                .on_press_maybe((!self.busy).then_some(event(e)))
        };
        let mut list = Column::new().spacing(8);
        for item in &self.list {
            let id = item[match kind {
                Kind::Skills => "name",
                Kind::Tasks => "id",
                _ => "filename",
            }]
            .as_str()
            .unwrap_or_default();
            let label = if kind == Kind::Tasks {
                item["name"].as_str().unwrap_or(id)
            } else {
                id
            };
            list = list.push(
                button(text(label))
                    .width(Fill)
                    .padding(9)
                    .on_press_maybe((!self.busy).then_some(event(Event::Select(id.into())))),
            );
            if kind == Kind::Skills {
                list = list.push(action(
                    if item["enabled"] == true {
                        "停用"
                    } else {
                        "启用"
                    },
                    Event::Toggle(id.into(), item["enabled"] == true),
                ));
            }
            if kind == Kind::Tasks {
                list = list.push(
                    row![
                        action(
                            if item["enabled"] == true {
                                "暂停"
                            } else {
                                "恢复"
                            },
                            Event::TaskAction(
                                id.into(),
                                if item["enabled"] == true {
                                    "pause"
                                } else {
                                    "resume"
                                }
                            )
                        ),
                        action("立即运行", Event::TaskAction(id.into(), "run"))
                    ]
                    .spacing(6),
                );
            }
        }
        let mut editor = column![
            row![text(kind.title()).size(24), action("返回聊天", Event::Back)].spacing(20),
            row![
                action("新建", Event::New),
                action("刷新列表", Event::Load),
                action("保存", Event::Save),
                action("放弃编辑", Event::Discard)
            ]
            .spacing(8)
        ]
        .spacing(12);
        if kind == Kind::Skills {
            editor = editor.push(action("导入技能 ZIP", Event::Import));
        }
        if kind == Kind::Memory || kind == Kind::Workspace {
            editor = editor.push(
                text("Markdown 文档；文件名需以 .md 结尾。保存会影响后续对话使用的内容。").size(12),
            );
        }
        if self.selected.is_none() || kind == Kind::Tasks {
            editor = editor.push(
                text_input("名称", &self.name)
                    .padding(9)
                    .on_input(move |v| event(Event::Name(v))),
            );
        } else {
            editor = editor.push(text(&self.name));
        }
        if kind == Kind::Tasks {
            editor =
                editor.push(text("任务在应用打开且电脑未休眠时执行，结果进入对应会话。").size(12));
            editor = editor.push(
                row![
                    pick_list(
                        vec!["提醒".to_owned(), "模型任务".to_owned()],
                        Some(self.task_type.clone()),
                        move |v| event(Event::Field("type", v))
                    ),
                    pick_list(
                        vec!["每天".to_owned(), "单次".to_owned(), "Cron".to_owned()],
                        Some(self.schedule.clone()),
                        move |v| event(Event::Field("schedule", v))
                    )
                ]
                .spacing(8),
            );
            editor = editor.push(
                text_input(
                    match self.schedule.as_str() {
                        "每天" => "09:00",
                        "单次" => "2026-09-07 09:00",
                        _ => "Cron 表达式，例如 0 9 * * 1-5",
                    },
                    &self.time,
                )
                .padding(9)
                .on_input(move |v| event(Event::Field("time", v))),
            );
            editor = editor.push(
                text_input("时区，例如 Asia/Shanghai", &self.zone)
                    .padding(9)
                    .on_input(move |v| event(Event::Field("zone", v))),
            );
            if !self.task["execution_state"].is_null() {
                editor = editor.push(
                    text(format!(
                        "下次执行：{}  上次状态：{}",
                        self.task["execution_state"]["next_run_at"],
                        self.task["execution_state"]["last_status"]
                    ))
                    .size(12),
                );
            }
        }
        editor = editor.push(
            text_editor(&self.editor)
                .height(260)
                .on_action(move |a| event(Event::Edit(a))),
        );
        if self.selected.is_some() {
            editor = editor.push(if self.delete_confirm {
                row![
                    text("确定永久删除此项？"),
                    action("确认删除", Event::ConfirmDelete),
                    action("取消", Event::CancelDelete)
                ]
                .spacing(8)
            } else {
                row![action("删除", Event::Delete)]
            });
        }
        for run in &self.history {
            editor = editor.push(
                text(format!(
                    "{}  {}  {}",
                    run["run_at"].as_str().unwrap_or(""),
                    run["status"].as_str().unwrap_or(""),
                    run["error"].as_str().unwrap_or("")
                ))
                .size(12),
            );
        }
        editor = editor.push(
            text(if self.busy {
                "正在读取或保存…"
            } else {
                &self.notice
            })
            .size(13),
        );
        row![
            container(scrollable(list)).width(190).height(Fill),
            container(scrollable(editor)).width(Fill).height(Fill)
        ]
        .spacing(20)
        .padding([32, 20])
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reminder_local_time_has_explicit_timezone_and_rejects_dst_ambiguity() {
        let mut pages = Pages {
            schedule: "单次".into(),
            time: "2026-09-07 09:00".into(),
            zone: "Asia/Shanghai".into(),
            name: "提醒".into(),
            task_type: "提醒".into(),
            editor: text_editor::Content::with_text("买牛奶"),
            ..Pages::default()
        };
        let spec = task_spec(&pages, "family").unwrap();
        assert_eq!(spec["schedule"]["run_at"], "2026-09-07T09:00:00+08:00");
        assert_eq!(spec["dispatch"]["target"]["session_id"], "family");
        pages.zone = "America/New_York".into();
        pages.time = "2026-11-01 01:30".into();
        assert!(task_spec(&pages, "family").is_err());
        pages.time = "2026-03-08 02:30".into();
        assert!(task_spec(&pages, "family").is_err());
    }
    #[test]
    fn dirty_editor_cannot_be_left_silently() {
        let mut pages = Pages {
            kind: Some(Kind::Memory),
            dirty: true,
            editor: text_editor::Content::with_text("未保存"),
            ..Pages::default()
        };
        assert!(!pages.leave());
        assert_eq!(pages.editor.text().trim_end(), "未保存");
        pages.dirty = false;
        assert!(pages.leave());
        assert!(pages.kind.is_none());
    }
}
