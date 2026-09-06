use crate::accessibility::{button, text_input};
use crate::{backend::Chat, App, Message};
use iced::widget::{column, row, text};
use iced::{Element, Task};
use serde_json::{json, Value};
#[derive(Default)]
pub struct Conversations {
    pub archived: bool,
    pub menu: Option<String>,
    name: String,
    confirm: bool,
    busy: bool,
    epoch: u64,
    notice: String,
}
#[derive(Clone)]
pub enum Event {
    Menu(String),
    Close,
    Name(String),
    Rename,
    Pin,
    Archive,
    Delete,
    ConfirmDelete,
    ToggleArchived,
    Search(String),
    Loaded(u64, Result<Vec<Chat>, String>),
    Done(u64, String, bool, Result<(), String>),
}
fn path(id: &str) -> String {
    let mut u = reqwest::Url::parse("http://local/").unwrap();
    u.path_segments_mut().unwrap().extend(["api", "chats", id]);
    u.path().to_owned()
}
impl App {
    pub fn conversation_event(&mut self, event: Event) -> Task<Message> {
        let Some(backend) = self.backend.clone() else {
            return Task::none();
        };
        if matches!(event, Event::ToggleArchived)
            && !self.streaming
            && !self.voice.active
            && !self.busy
            && !self.conversations.busy
        {
            self.save_draft();
        }
        let state = &mut self.conversations;
        match event {
            Event::Menu(id) => {
                if state.busy {
                    return Task::none();
                };
                if let Some(chat) = self.chats.iter().find(|c| c.id == id) {
                    state.name = chat.name.clone();
                    state.menu = Some(id);
                    state.confirm = false;
                    state.notice.clear();
                }
            }
            Event::Close => {
                if !state.busy {
                    state.menu = None;
                    state.confirm = false;
                }
            }
            Event::Name(value) => state.name = value,
            Event::Delete => state.confirm = true,
            Event::ToggleArchived => {
                if self.streaming || self.voice.active || self.busy || state.busy {
                    return Task::none();
                };
                state.archived = !state.archived;
                self.generation += 1;
                self.preferences_dirty = true;
                self.session = crate::backend::Session::default();
                self.uncertain = false;
                self.base_messages.clear();
                self.turn = crate::stream::Turn::default();
                state.menu = None;
                self.selected = None;
                self.messages.clear();
                self.attachments.clear();
                self.restore_draft();
                return self.conversation_event(Event::Search(self.filter.clone()));
            }
            Event::Search(query) => {
                self.filter = query.clone();
                if state.busy {
                    return Task::none();
                }
                state.epoch += 1;
                let epoch = state.epoch;
                let archived = state.archived;
                return Task::perform(
                    async move {
                        let mut u = reqwest::Url::parse("http://local/api/chats").unwrap();
                        u.query_pairs_mut()
                            .append_pair("archived", if archived { "true" } else { "false" })
                            .append_pair("q", &query);
                        let path = format!("{}?{}", u.path(), u.query().unwrap());
                        let v = backend.request("GET", &path, Value::Null).await?;
                        serde_json::from_value(v).map_err(|_| "会话列表格式错误".into())
                    },
                    move |v| Message::Conversation(Event::Loaded(epoch, v)),
                );
            }
            Event::Loaded(epoch, result) if epoch == state.epoch => match result {
                Ok(v) => self.chats = v,
                Err(e) => {
                    self.status = e.clone();
                    state.notice = e;
                }
            },
            Event::Done(epoch, id, clear_selection, result) if epoch == state.epoch => {
                state.busy = false;
                match result {
                    Ok(()) => {
                        state.menu = None;
                        state.notice.clear();
                        if clear_selection && self.selected.as_deref() == Some(&id) {
                            self.new_chat();
                        }
                        return self.conversation_event(Event::Search(self.filter.clone()));
                    }
                    Err(e) => state.notice = e,
                }
            }
            Event::Rename | Event::Pin | Event::Archive | Event::ConfirmDelete => {
                if state.busy {
                    return Task::none();
                };
                let Some(id) = state.menu.clone() else {
                    return Task::none();
                };
                let Some(chat) = self.chats.iter().find(|c| c.id == id) else {
                    return Task::none();
                };
                if chat.status == "running" || self.streaming {
                    state.notice = "请先停止正在运行的会话".into();
                    return Task::none();
                }
                let body = match event {
                    Event::Rename => json!({"name":state.name.trim()}),
                    Event::Pin => json!({"pinned":!chat.pinned}),
                    Event::Archive => json!({"archived":!chat.archived}),
                    _ => Value::Null,
                };
                let clear_selection = matches!(event, Event::Archive | Event::ConfirmDelete);
                let method = if matches!(event, Event::ConfirmDelete) {
                    "DELETE"
                } else {
                    "PUT"
                };
                state.busy = true;
                state.epoch += 1;
                let epoch = state.epoch;
                return Task::perform(
                    async move {
                        backend
                            .request(method, &path(&id), body)
                            .await
                            .map(|_| ())
                            .map(|_| id.clone())
                            .map_err(|e| (id, e))
                    },
                    move |v| match v {
                        Ok(id) => {
                            Message::Conversation(Event::Done(epoch, id, clear_selection, Ok(())))
                        }
                        Err((id, e)) => {
                            Message::Conversation(Event::Done(epoch, id, clear_selection, Err(e)))
                        }
                    },
                );
            }
            _ => {}
        }
        Task::none()
    }
}
impl Conversations {
    pub fn view(&self) -> Element<'_, Message> {
        let action = |label: &'static str, e| {
            button(label)
                .padding(7)
                .on_press_maybe((!self.busy).then_some(Message::Conversation(e)))
        };
        column![
            text("会话管理").size(18),
            text_input("会话名称", &self.name)
                .padding(8)
                .on_input(|v| Message::Conversation(Event::Name(v))),
            row![
                action("保存名称", Event::Rename),
                action("切换置顶", Event::Pin),
                action(
                    if self.archived {
                        "恢复会话"
                    } else {
                        "归档"
                    },
                    Event::Archive
                ),
                action("删除", Event::Delete),
                action("关闭", Event::Close)
            ]
            .spacing(8),
            if self.confirm {
                row![
                    text("会话及消息将永久删除。"),
                    action("确认删除", Event::ConfirmDelete),
                    action("取消", Event::Close)
                ]
                .spacing(8)
            } else {
                row![]
            },
            text(&self.notice)
        ]
        .spacing(10)
        .padding(16)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_during_mutation_cannot_discard_its_completion_or_clear_active_chat() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            backend: Some(crate::backend::Backend::open(dir.path()).unwrap()),
            selected: Some("family".into()),
            ..App::default()
        };
        app.conversations.busy = true;
        app.conversations.epoch = 7;
        let _ = app.conversation_event(Event::Search("公园".into()));
        assert_eq!(app.conversations.epoch, 7);
        let _ = app.conversation_event(Event::Done(7, "family".into(), false, Ok(())));
        assert!(!app.conversations.busy);
        assert_eq!(app.selected.as_deref(), Some("family"));
        assert_eq!(app.filter, "公园");
    }
}
