//! Composer menus use the same core settings as the settings panel.
use crate::{
    accessibility::{button, text_input},
    App, Message,
};
use iced::{
    widget::{column, container, mouse_area, opaque, row, scrollable, stack, text, Space},
    Element, Fill, Task,
};
use serde_json::{json, Value};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Models,
    Project,
    Permissions,
}
#[derive(Default)]
pub struct Menus {
    kind: Option<Kind>,
    items: Vec<Value>,
    active: Value,
    busy: bool,
    notice: String,
    name: String,
    pub project_name: String,
}
#[derive(Clone)]
pub enum Event {
    Open(Kind),
    Close,
    Loaded(Kind, Result<(Vec<Value>, Value), String>),
    Model(String, String),
    Project(Option<String>),
    PickProject,
    Name(String),
    CreateProject,
    Permission(String),
    Saved(Kind, Result<Value, String>),
}
impl App {
    pub fn menu_event(&mut self, event: Event) -> Task<Message> {
        let Some(api) = self.backend.clone() else {
            return Task::none();
        };
        let state = &mut self.menus;
        match event {
            Event::Open(kind) => {
                if state.busy || !self.pages.leave() {
                    return Task::none();
                }
                state.kind = Some(kind);
                state.notice.clear();
                state.busy = true;
                return Task::perform(
                    async move {
                        let (items, active) = match kind {
                            Kind::Models => (
                                api.request("GET", "/api/models", Value::Null).await?,
                                api.request("GET", "/api/models/active", Value::Null)
                                    .await?,
                            ),
                            Kind::Project => (
                                api.request(
                                    "GET",
                                    "/api/workspace/coding-project/list",
                                    Value::Null,
                                )
                                .await?,
                                api.request("GET", "/api/workspace/coding-project", Value::Null)
                                    .await?,
                            ),
                            Kind::Permissions => (
                                json!([]),
                                api.request("GET", "/api/workspace/running-config", Value::Null)
                                    .await?,
                            ),
                        };
                        Ok((items.as_array().cloned().unwrap_or_default(), active))
                    },
                    move |v| Message::Menu(Event::Loaded(kind, v)),
                );
            }
            Event::Close if !state.busy => state.kind = None,
            Event::Loaded(kind, result) if state.kind == Some(kind) => {
                state.busy = false;
                match result {
                    Ok((items, active)) => {
                        state.items = items;
                        state.active = active;
                    }
                    Err(e) => state.notice = e,
                }
            }
            Event::Name(name) if !state.busy => state.name = name,
            Event::Model(provider, model) if !state.busy => {
                if self.streaming {
                    state.notice = "请等待当前回复结束后再切换模型".into();
                    return Task::none();
                }
                state.busy = true;
                return Task::perform(
                    async move {
                        api.request(
                            "PUT",
                            "/api/models/active",
                            json!({"provider_id":provider,"model":model}),
                        )
                        .await
                    },
                    |v| Message::Menu(Event::Saved(Kind::Models, v)),
                );
            }
            Event::Project(path) if !state.busy => {
                state.busy = true;
                return Task::perform(
                    async move {
                        api.request("PUT", "/api/workspace/coding-project", json!({"path":path}))
                            .await
                    },
                    |v| Message::Menu(Event::Saved(Kind::Project, v)),
                );
            }
            Event::CreateProject if !state.busy => {
                if state.name.trim().is_empty() {
                    state.notice = "请填写项目名称".into();
                    return Task::none();
                }
                state.busy = true;
                let name = state.name.trim().to_owned();
                return Task::perform(
                    async move {
                        api.request(
                            "POST",
                            "/api/workspace/coding-project/create",
                            json!({"name":name}),
                        )
                        .await
                    },
                    |v| Message::Menu(Event::Saved(Kind::Project, v)),
                );
            }
            Event::PickProject if !state.busy => {
                state.busy = true;
                return Task::perform(
                    async move {
                        let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
                            return Ok(Value::Null);
                        };
                        api.request(
                            "PUT",
                            "/api/workspace/coding-project",
                            json!({"path":folder.path()}),
                        )
                        .await
                    },
                    |v| Message::Menu(Event::Saved(Kind::Project, v)),
                );
            }
            Event::Permission(mode) if !state.busy => {
                if self.streaming {
                    state.notice = "请等待当前回复结束后再修改权限".into();
                    return Task::none();
                }
                state.busy = true;
                return Task::perform(
                    async move {
                        api.request(
                            "PUT",
                            "/api/workspace/running-config",
                            json!({"sandbox_mode":mode}),
                        )
                        .await
                    },
                    |v| Message::Menu(Event::Saved(Kind::Permissions, v)),
                );
            }
            Event::Saved(kind, result) => {
                state.busy = false;
                match result {
                    Ok(value) if !value.is_null() => {
                        match kind {
                            Kind::Models => {
                                self.model =
                                    value["active_llm"]["model"].as_str().map(str::to_owned)
                            }
                            Kind::Project => {
                                state.project_name = if value["is_workspace_default"] == true {
                                    "默认".into()
                                } else {
                                    value["name"].as_str().unwrap_or("项目").into()
                                }
                            }
                            Kind::Permissions => {
                                self.sandbox = value["sandbox_mode"].as_str().map(str::to_owned)
                            }
                        }
                        state.kind = None;
                        state.name.clear();
                    }
                    Ok(_) => {}
                    Err(e) => state.notice = e,
                }
            }
            _ => {}
        }
        Task::none()
    }
}
impl Menus {
    pub fn overlay<'a>(&'a self, base: Element<'a, Message>) -> Element<'a, Message> {
        let Some(kind) = self.kind else {
            return base;
        };
        let mut body = column![row![
            text(match kind {
                Kind::Models => "选择模型",
                Kind::Project => "工作区与项目",
                Kind::Permissions => "权限",
            })
            .size(16),
            Space::new().width(Fill),
            button("关闭").on_press_maybe((!self.busy).then_some(Message::Menu(Event::Close)))
        ]]
        .spacing(12);
        let mut list = column![].spacing(6);
        match kind {
            Kind::Models => {
                for p in &self.items {
                    if p["is_local"] != true && p["api_key"].as_str().unwrap_or("").is_empty() {
                        continue;
                    }
                    list = list.push(text(p["name"].as_str().unwrap_or("服务商")).size(12));
                    let mut seen = std::collections::HashSet::new();
                    for m in ["extra_models", "models"]
                        .iter()
                        .flat_map(|k| p[*k].as_array().into_iter().flatten())
                    {
                        let id = m["id"].as_str().unwrap_or("");
                        if !seen.insert(id) {
                            continue;
                        }
                        let active = self.active["active_llm"]["provider_id"] == p["id"]
                            && self.active["active_llm"]["model"] == id;
                        list = list.push(
                            button(
                                text(format!(
                                    "{}{}",
                                    if active { "✓ " } else { "" },
                                    m["name"].as_str().unwrap_or(id)
                                ))
                                .size(13),
                            )
                            .width(Fill)
                            .style(crate::ui::nav)
                            .on_press_maybe((!self.busy).then_some(Message::Menu(Event::Model(
                                p["id"].as_str().unwrap_or("").into(),
                                id.into(),
                            )))),
                        );
                    }
                }
                list = list.push(
                    button("管理模型与服务商")
                        .on_press(Message::OpenSettings(crate::settings::Section::Models)),
                );
            }
            Kind::Project => {
                list =
                    list.push(button("默认工作区").width(Fill).on_press_maybe(
                        (!self.busy).then_some(Message::Menu(Event::Project(None))),
                    ));
                for p in &self.items {
                    list = list.push(
                        button(text(p["name"].as_str().unwrap_or("项目")))
                            .width(Fill)
                            .on_press_maybe((!self.busy).then_some(Message::Menu(Event::Project(
                                p["path"].as_str().map(str::to_owned),
                            )))),
                    );
                }
                list = list
                    .push(
                        button("选择本地文件夹…").on_press_maybe(
                            (!self.busy).then_some(Message::Menu(Event::PickProject)),
                        ),
                    )
                    .push(
                        row![
                            text_input("新项目名称", &self.name)
                                .on_input(|v| Message::Menu(Event::Name(v))),
                            button("创建").on_press_maybe(
                                (!self.busy).then_some(Message::Menu(Event::CreateProject))
                            )
                        ]
                        .spacing(8),
                    )
                    .push(button("角色与系统提示文件").on_press(Message::Page(
                        crate::pages::Event::Open(crate::pages::Kind::Workspace),
                    )));
            }
            Kind::Permissions => {
                list = list.push(text("文件访问范围").size(13));
                for (value, label) in [
                    ("read-only", "只读"),
                    ("workspace-write", "允许修改工作区"),
                    ("danger-full-access", "完整文件访问"),
                ] {
                    list = list.push(
                        button(text(format!(
                            "{}{}",
                            if self.active["sandbox_mode"] == value {
                                "✓ "
                            } else {
                                ""
                            },
                            label
                        )))
                        .width(Fill)
                        .on_press_maybe(
                            (!self.busy).then_some(Message::Menu(Event::Permission(value.into()))),
                        ),
                    );
                }
                list = list
                    .push(text("工具操作仍逐次审批。文件范围不等于操作系统沙箱隔离。").size(12))
                    .push(
                        button("完整安全设置")
                            .on_press(Message::OpenSettings(crate::settings::Section::Security)),
                    );
            }
        }
        body = body.push(scrollable(list).height(iced::Length::Shrink));
        if self.busy {
            body = body.push(text("正在处理…").size(12));
        }
        if !self.notice.is_empty() {
            body = body.push(text(&self.notice).size(12));
        }
        let panel = container(body)
            .padding(20)
            .width(400)
            .max_height(520)
            .style(container::bordered_box);
        stack![
            base,
            mouse_area(container(Space::new()).width(Fill).height(Fill))
                .on_press(Message::Menu(Event::Close)),
            container(opaque(crate::accessibility::modal(panel.into())))
                .align_right(Fill)
                .align_bottom(Fill)
                .padding(24)
        ]
        .into()
    }
    pub fn is_open(&self) -> bool {
        self.kind.is_some()
    }
    pub fn close(&mut self) {
        if !self.busy {
            self.kind = None;
        }
    }
}
