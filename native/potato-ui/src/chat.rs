use crate::{backend, stream, App, Message};
use backend::{NetworkEvent, Session};
use iced::{
    widget::{scrollable, text_editor},
    Task,
};

#[derive(Clone, Default)]
pub(super) struct Draft {
    text: String,
    pub(super) attachments: Vec<serde_json::Value>,
    backup: Option<(String, Vec<serde_json::Value>)>,
}
impl App {
    pub(super) fn composer_text(&self) -> String {
        self.draft
            .lines()
            .map(|line| line.text.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub(super) fn save_draft(&mut self) {
        let key = self.selected.clone().unwrap_or_default();
        if self.composer_text().is_empty()
            && self.attachments.is_empty()
            && self.editing_backup.is_none()
        {
            self.drafts.remove(&key);
        } else {
            self.drafts.insert(
                key,
                Draft {
                    text: self.composer_text(),
                    attachments: self.attachments.clone(),
                    backup: self.editing_backup.clone(),
                },
            );
        }
    }
    pub(super) fn restore_draft(&mut self) {
        let draft = self
            .drafts
            .get(&self.selected.clone().unwrap_or_default())
            .cloned()
            .unwrap_or_default();
        self.draft = text_editor::Content::with_text(&draft.text);
        self.attachments = draft.attachments;
        self.editing_backup = draft.backup;
    }
    pub(super) fn cancel_message_edit(&mut self) {
        if self.streaming || self.busy {
            return;
        }
        if let Some((text, attachments)) = self.editing_backup.take() {
            self.draft = text_editor::Content::with_text(&text);
            self.attachments = attachments;
            self.status.clear();
        }
    }
    pub(super) fn reuse_message(&mut self, index: usize, send: bool) -> Task<Message> {
        if !self.can_send() || self.editing_backup.is_some() {
            return Task::none();
        }
        if send && index + 1 != self.messages.len() {
            return Task::none();
        }
        let Some(index) = (0..=index.min(self.messages.len().saturating_sub(1)))
            .rev()
            .find(|&i| self.messages.get(i).is_some_and(|m| m.role == "user"))
        else {
            return Task::none();
        };
        let message = &self.messages[index];
        let Some(raw) = message.raw.as_ref() else {
            return Task::none();
        };
        let prompt = raw["content"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| message.body.clone());
        let attachments = if send {
            vec![]
        } else {
            raw["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| v["type"] != "text")
                .cloned()
                .collect()
        };
        if prompt.trim().is_empty() && attachments.is_empty() {
            return Task::none();
        }
        self.editing_backup = Some((self.composer_text(), self.attachments.clone()));
        self.draft = text_editor::Content::with_text(&prompt);
        self.attachments = attachments;
        self.status.clear();
        if send {
            self.start_network(prompt, false)
        } else {
            Task::none()
        }
    }

    pub(super) fn can_steer(&self) -> bool {
        self.streaming
            && self.accepted
            && !self.stop_pending
            && !self.busy
            && !self.uncertain
            && !self.voice.active
            && self.backend.is_some()
            && !self.composer_text().trim().is_empty()
            && self.attachments.is_empty()
    }

    pub(super) fn send_steering(&mut self) -> Task<Message> {
        let Some(backend) = self.backend.clone() else {
            return Task::none();
        };
        let text = self.composer_text().trim().to_owned();
        let generation = self.generation;
        let session = self.session.id.clone();
        self.busy = true;
        Task::perform(
            async move {
                let result = backend
                    .request(
                        "POST",
                        "/api/agent/steer",
                        serde_json::json!({"session_id":session,"text":text}),
                    )
                    .await
                    .map(|_| ());
                (text, result)
            },
            move |(text, result)| Message::Steered(generation, text, result),
        )
    }

    pub(super) fn can_send(&self) -> bool {
        !self
            .selected
            .as_ref()
            .is_some_and(|id| self.chats.iter().any(|c| &c.id == id && c.archived))
            && !self.voice.active
            && !self.interactions.blocks_send(&self.session)
            && !self.streaming
            && !self.busy
            && !self.uncertain
            && (self.backend.is_none() || (self.connected && self.model.is_some()))
    }

    pub(super) fn new_chat(&mut self) {
        self.save_draft();
        self.preferences_dirty = true;
        self.conversations.archived = false;
        self.hovered_message = None;
        self.expanded_tools.clear();
        self.interactions = crate::interactions::State::default();
        self.generation += 1;
        self.pending_submission = None;
        self.streaming = false;
        self.session = Session::default();
        self.selected = None;
        self.messages.clear();
        self.attachments.clear();
        self.base_messages.clear();
        self.turn = stream::Turn::default();
        self.uncertain = false;
        self.restore_draft();
        self.follow_bottom = true;
        self.stop_pending = false;
        self.accepted = false;
        self.status = if self.backend.is_some() {
            String::new()
        } else {
            "本地应用尚未就绪".into()
        };
    }

    pub(super) fn start_network(&mut self, prompt: String, reconnect: bool) -> Task<Message> {
        let Some(backend) = self.backend.clone() else {
            self.cancel_message_edit();
            return Task::none();
        };
        self.hovered_message = None;
        self.expanded_tools.clear();
        self.interactions = crate::interactions::State::default();
        self.generation += 1;
        let generation = self.generation;
        self.streaming = true;
        self.accepted = false;
        self.turn = stream::Turn::default();
        self.follow_bottom = true;
        // Reconnect replays the complete session stream; fetch the persisted history at EOF
        // instead of appending its snapshots to an old copy.
        self.base_messages = if reconnect {
            vec![]
        } else {
            self.messages.clone()
        };
        if !reconnect {
            let mut content = vec![serde_json::json!({"type":"text","text":prompt})];
            content.extend(self.attachments.clone());
            self.base_messages.push(backend::display_message(
                &serde_json::json!({"role":"user","content":content}),
            ));
        }
        self.messages = self.base_messages.clone();
        self.status = if reconnect {
            "正在重新连接…".into()
        } else {
            "正在发送…".into()
        };
        let submitted_attachments = self.attachments.clone();
        if !reconnect {
            if let Some((text, attachments)) = self.editing_backup.take() {
                self.pending_submission = None;
                self.draft = text_editor::Content::with_text(&text);
                self.attachments = attachments;
            } else {
                self.pending_submission =
                    Some((self.composer_text(), submitted_attachments.clone()));
                self.draft = text_editor::Content::new();
                self.attachments.clear();
            }
        }
        Task::run(
            backend.stream(
                self.session.clone(),
                prompt,
                reconnect,
                submitted_attachments,
            ),
            move |event| Message::Network(generation, event),
        )
    }

    pub(super) fn network_event(&mut self, generation: u64, event: NetworkEvent) -> Task<Message> {
        if generation != self.generation {
            return Task::none();
        }
        match event {
            NetworkEvent::Accepted => {
                self.accepted = true;
                self.preferences_dirty = true;
                self.pending_submission = None;
                if let Some((text, attachments)) = self.editing_backup.take() {
                    self.draft = text_editor::Content::with_text(&text);
                    self.attachments = attachments;
                }
                self.drafts
                    .remove(&self.selected.clone().unwrap_or_default());
                self.status = "正在生成…".into();
                if let Some(backend) = self.backend.clone() {
                    return Task::perform(backend.chats(), move |result| {
                        Message::Chats(generation, result)
                    });
                }
            }
            NetworkEvent::Frames(frames) => {
                for frame in frames {
                    self.turn.apply(frame);
                }
                if let Some(id) = &self.turn.session_id {
                    self.session.id = id.clone();
                }
                self.messages = if self.turn.cleared {
                    vec![]
                } else {
                    self.base_messages.clone()
                };
                self.messages.extend(
                    self.turn
                        .messages
                        .iter()
                        .filter(|message| {
                            message["role"] != "user" || self.base_messages.is_empty()
                        })
                        .map(backend::display_message),
                );
                crate::rich::merge_tools(&mut self.messages);
                if let Some(error) = &self.turn.error {
                    self.status = error.clone();
                } else if self
                    .turn
                    .messages
                    .iter()
                    .any(|m| m["type"].as_str().is_some_and(|kind| kind.contains("call")))
                {
                    self.status = "正在执行工具…".into();
                }
                if self.follow_bottom {
                    return iced::widget::operation::snap_to(
                        iced::widget::Id::new("messages"),
                        scrollable::RelativeOffset::END,
                    );
                }
            }
            NetworkEvent::End(result) => {
                self.streaming = false;
                self.stop_pending = false;
                self.uncertain = self.accepted && !self.turn.terminal();
                if !self.accepted {
                    if self.editing_backup.is_some() {
                        self.cancel_message_edit();
                    } else if let Some((text, attachments)) = self.pending_submission.take() {
                        let current = self.composer_text();
                        self.draft = text_editor::Content::with_text(&if current.is_empty() {
                            text
                        } else {
                            format!("{text}\n{current}")
                        });
                        self.attachments.splice(0..0, attachments);
                    }
                    self.pending_submission = None;
                }
                if !self.accepted && !self.turn.terminal() {
                    self.messages = self.base_messages.clone();
                    if self.messages.last().is_some_and(|m| m.role == "user") {
                        self.messages.pop();
                    }
                }
                if let Err(error) = &result {
                    self.turn.error = Some(error.clone());
                }
                self.status = match result {
                    Err(error) => error,
                    Ok(()) if self.uncertain => {
                        "连接已结束，但后台未确认完成。请刷新或重连，勿重复发送".into()
                    }
                    Ok(()) => self.turn.error.clone().unwrap_or_default(),
                };
                self.accepted = false;
                if let Some(backend) = self.backend.clone() {
                    // History is authoritative, including pre-existing messages after reconnect.
                    if let Some(id) = self.selected.clone() {
                        self.busy = true;
                        return Task::perform(backend.history(id), move |result| {
                            Message::History(generation, result)
                        });
                    }
                    return Task::perform(backend.chats(), move |result| {
                        Message::Chats(generation, result)
                    });
                }
            }
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steering_is_available_during_streaming_and_keeps_edits_on_ack() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            backend: Some(backend::Backend::open(dir.path()).unwrap()),
            streaming: true,
            accepted: true,
            ..App::default()
        };
        app.draft = text_editor::Content::with_text("改用第二个方案");
        assert!(app.can_steer());
        assert!(!app.can_send());
        app.stop_pending = true;
        assert!(!app.can_steer());
        app.stop_pending = false;
        app.attachments.push(serde_json::json!({"type":"file"}));
        assert!(!app.can_steer());
        app.attachments.clear();
        app.draft = text_editor::Content::with_text("正在继续编辑的新指令");
        let _ = app.update(Message::Steered(app.generation, "旧指令".into(), Ok(())));
        assert_eq!(app.composer_text(), "正在继续编辑的新指令");
        let _ = app.update(Message::Steered(
            app.generation,
            "正在继续编辑的新指令".into(),
            Ok(()),
        ));
        assert!(app.composer_text().is_empty());
    }
    fn user_message() -> crate::rich::ChatMessage {
        crate::rich::ChatMessage::from_value(
            &serde_json::json!({"role":"user","content":[{"type":"text","text":"原始问题"},{"type":"file","file_url":"upload://a","file_name":"a.txt"}]}),
        )
    }
    #[test]
    fn select_during_stream_loads_target_and_isolates_old_events() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            backend: Some(backend::Backend::open(dir.path()).unwrap()),
            streaming: true,
            accepted: true,
            selected: Some("old".into()),
            draft: text_editor::Content::with_text("草稿"),
            ..App::default()
        };
        let _ = app.update(Message::Select("next".into()));
        assert_eq!(app.selected.as_deref(), Some("next"));
        assert!(!app.streaming);
        assert!(app.busy);
        let _ = app.network_event(0, NetworkEvent::End(Err("旧连接错误".into())));
        assert!(app.status.is_empty());
        app.selected = Some("old".into());
        app.restore_draft();
        assert_eq!(app.composer_text(), "草稿");
    }
    #[test]
    fn rejected_regeneration_restores_draft_and_removes_edit_banner() {
        let mut app = App {
            streaming: true,
            editing_backup: Some(("草稿".into(), vec![])),
            base_messages: vec![user_message()],
            ..App::default()
        };
        let _ = app.network_event(0, NetworkEvent::End(Err("发送失败".into())));
        assert_eq!(app.composer_text(), "草稿");
        assert!(app.editing_backup.is_none());
        assert!(app.can_send());
    }
    #[test]
    fn accepted_send_keeps_newly_typed_followup() {
        let mut app = App {
            streaming: true,
            draft: text_editor::Content::with_text("追问"),
            ..App::default()
        };
        let _ = app.network_event(0, NetworkEvent::Accepted);
        assert_eq!(app.composer_text(), "追问");
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('好'),
        )));
        assert!(app.composer_text().contains('好'));
    }
    #[test]
    fn new_chat_during_stream_restores_independent_draft_and_ignores_old_events() {
        let mut app = App {
            selected: Some("old".into()),
            streaming: true,
            accepted: true,
            draft: text_editor::Content::with_text("旧会话追问"),
            ..App::default()
        };
        let _ = app.update(Message::NewChat);
        assert!(!app.streaming);
        assert!(app.selected.is_none());
        let _ = app.network_event(0, NetworkEvent::Accepted);
        assert!(!app.accepted);
        app.selected = Some("old".into());
        app.restore_draft();
        assert_eq!(app.composer_text(), "旧会话追问");
    }
    #[test]
    fn edit_cancel_restores_unsent_text_and_attachments() {
        let mut app = App {
            messages: vec![user_message()],
            draft: text_editor::Content::with_text("未发送草稿"),
            attachments: vec![serde_json::json!({"type":"file","file_name":"b.txt"})],
            ..App::default()
        };
        let _ = app.reuse_message(0, false);
        assert_eq!(app.composer_text(), "原始问题");
        assert_eq!(app.attachments[0]["file_name"], "a.txt");
        app.cancel_message_edit();
        assert_eq!(app.composer_text(), "未发送草稿");
        assert_eq!(app.attachments[0]["file_name"], "b.txt");
        assert_eq!(app.messages.len(), 1);
    }
    #[test]
    fn session_switch_preserves_edit_and_its_cancel_backup() {
        let mut app = App {
            selected: Some("a".into()),
            messages: vec![user_message()],
            draft: text_editor::Content::with_text("草稿"),
            ..App::default()
        };
        let _ = app.reuse_message(0, false);
        app.draft = text_editor::Content::with_text("修改中");
        app.save_draft();
        app.selected = Some("b".into());
        app.restore_draft();
        assert!(app.attachments.is_empty());
        assert!(app.editing_backup.is_none());
        app.selected = Some("a".into());
        app.restore_draft();
        assert_eq!(app.composer_text(), "修改中");
        app.cancel_message_edit();
        assert_eq!(app.composer_text(), "草稿");
    }
    #[test]
    fn accepted_retry_restores_previous_composer() {
        let mut app = App {
            messages: vec![user_message()],
            draft: text_editor::Content::with_text("草稿"),
            ..App::default()
        };
        let _ = app.reuse_message(0, false);
        let _ = app.network_event(0, NetworkEvent::Accepted);
        assert_eq!(app.composer_text(), "草稿");
        assert!(app.editing_backup.is_none());
    }
    #[test]
    fn rejected_send_keeps_retryable_input_and_removes_optimistic_message() {
        let mut app = App {
            streaming: true,
            draft: text_editor::Content::with_text("保留问题"),
            base_messages: vec![user_message()],
            ..App::default()
        };
        let _ = app.network_event(0, NetworkEvent::End(Err("没有可用模型".into())));
        assert!(!app.uncertain);
        assert!(app.can_send());
        assert!(app.messages.is_empty());
        assert_eq!(app.composer_text(), "保留问题");
        assert_eq!(app.status, "没有可用模型");
    }
    #[test]
    fn regenerate_only_allows_last_answer_and_blocks_during_streaming() {
        let mut app = App {
            messages: vec![
                user_message(),
                ("assistant".into(), "回答".into()).into(),
                user_message(),
            ],
            ..App::default()
        };
        let _ = app.reuse_message(1, true);
        assert!(app.editing_backup.is_none());
        app.streaming = true;
        let _ = app.reuse_message(2, false);
        assert!(app.editing_backup.is_none());
    }
    #[test]
    fn refresh_restores_own_attachments_and_new_chat_restores_unsent_draft() {
        let mut app = App {
            draft: text_editor::Content::with_text("新会话草稿"),
            attachments: vec![serde_json::json!({"file_name":"a.txt"})],
            ..App::default()
        };
        app.save_draft();
        app.selected = Some("existing".into());
        app.restore_draft();
        assert!(app.attachments.is_empty());
        app.new_chat();
        assert_eq!(app.composer_text(), "新会话草稿");
        assert_eq!(app.attachments[0]["file_name"], "a.txt");
    }
    #[test]
    fn stop_acknowledgement_keeps_send_locked_until_terminal_response() {
        let mut app = App {
            streaming: true,
            accepted: true,
            stop_pending: true,
            ..App::default()
        };
        let _ = app.update(Message::Stopped(0, Ok(true)));
        assert!(app.streaming);
        assert!(!app.can_send());
        let _ = app.network_event(
            0,
            NetworkEvent::Frames(vec![
                serde_json::json!({"object":"response", "id":"r", "status":"cancelled"}),
            ]),
        );
        let _ = app.network_event(0, NetworkEvent::End(Ok(())));
        assert!(!app.streaming);
        assert!(app.can_send());
    }

    #[test]
    fn history_clears_running_badge_after_stop() {
        let mut app = App {
            selected: Some("c".into()),
            uncertain: true,
            chats: vec![backend::Chat {
                id: "c".into(),
                name: "chat".into(),
                session_id: "s".into(),
                user_id: "default".into(),
                channel: "console".into(),
                status: "running".into(),
                pinned: false,
                archived: false,
            }],
            ..App::default()
        };
        let _ = app.update(Message::History(
            0,
            Ok(backend::History {
                messages: vec![],
                status: "idle".into(),
            }),
        ));
        assert_eq!(app.chats[0].status, "idle");
        assert!(!app.uncertain);
    }
    #[test]
    fn unexpected_eof_blocks_duplicate_sends() {
        let mut app = App {
            streaming: true,
            accepted: true,
            ..App::default()
        };
        let _ = app.network_event(0, NetworkEvent::End(Ok(())));
        assert!(app.uncertain);
        assert!(!app.can_send());
    }
    #[test]
    fn stale_stream_cannot_write_into_new_chat() {
        let mut app = App::default();
        app.new_chat();
        let _ = app.network_event(
            0,
            NetworkEvent::Frames(vec![
                serde_json::json!({"object":"response", "status":"failed", "error":"old"}),
            ]),
        );
        assert!(app.messages.is_empty());
        assert!(!app.uncertain);
    }
}
