//! Session-scoped pending approvals and questions. Polling never grants permission.
use crate::accessibility::{button, text_input};
use crate::{App, Message};
use iced::widget::{column, container, row, scrollable, text, Column};
use iced::{Element, Fill, Task};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Deserialize)]
pub struct Approval {
    pub request_id: String,
    pub root_session_id: String,
    pub user_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub tool_display_name: String,
    #[serde(default)]
    pub exact_target: String,
    #[serde(default)]
    pub allow_session: bool,
    #[serde(default)]
    pub action_detail: String,
    #[serde(default)]
    pub justification: Option<String>,
    #[serde(default)]
    pub permission_increment: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub result_summary: Value,
    pub created_at: f64,
    pub timeout_seconds: f64,
}
impl Approval {
    fn current(&self, session: &crate::backend::Session) -> bool {
        self.root_session_id == session.id
            && self.user_id == session.user
            && self.created_at + self.timeout_seconds > chrono::Utc::now().timestamp() as f64
    }
}
#[derive(Clone, Deserialize)]
pub struct Question {
    pub request_id: String,
    pub session_id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub options: Vec<QuestionOption>,
}
#[derive(Clone, Deserialize)]
pub struct QuestionOption {
    pub id: String,
    pub label: String,
}
#[derive(Default)]
pub struct Draft {
    selected: Vec<String>,
    text: String,
}
#[derive(Default)]
pub struct State {
    approvals: Vec<Approval>,
    questions: Vec<Question>,
    drafts: HashMap<String, Draft>,
    completed: HashSet<String>,
    pending: HashSet<String>,
    errors: HashMap<String, String>,
    polling: bool,
    grant_count: usize,
    poll_error: String,
}
impl State {
    pub fn blocks_send(&self, session: &crate::backend::Session) -> bool {
        self.approvals.iter().any(|a| a.current(session))
            || !self.questions.is_empty()
            || !self.pending.is_empty()
    }
}
#[derive(Clone)]
pub enum Event {
    Poll,
    Polled(
        u64,
        Result<(Vec<Approval>, usize), String>,
        Result<Vec<Question>, String>,
    ),
    Approval(String, bool, bool),
    RevokeGrants,
    GrantsRevoked(u64, Result<(), String>),
    Select(String, String),
    Text(String, String),
    Answer(String, bool),
    Done(u64, String, Result<(), String>),
}
fn answer(question: &Question, draft: &Draft, skip: bool) -> Result<Value, String> {
    if skip {
        return Ok(json!({"selected":[], "text":"", "skip":true}));
    }
    let mut selected = Vec::new();
    for id in &draft.selected {
        if question.options.iter().any(|option| option.id == *id) && !selected.contains(id) {
            selected.push(id.clone());
        }
    }
    if !question.multiple && selected.len() > 1 {
        return Err("此问题只能选择一项".into());
    }
    let text = draft.text.trim();
    if selected.is_empty() && text.is_empty() {
        return Err("请选择选项或填写回答".into());
    }
    Ok(json!({"selected":selected,"text":text,"skip":false}))
}
impl App {
    pub(super) fn interaction(&mut self, event: Event) -> Task<Message> {
        let generation = self.generation;
        match event {
            Event::Poll
                if self.connected
                    && !self.interactions.polling
                    && (self.selected.is_some() || self.streaming || self.uncertain) =>
            {
                if let Some(backend) = self.backend.clone() {
                    self.interactions.polling = true;
                    let session = self.session.id.clone();
                    return Task::perform(
                        async move {
                            let approvals = backend.pending_approvals(&session).await;
                            let questions = backend.questions(&session).await;
                            (approvals, questions)
                        },
                        move |(a, q)| Message::Interaction(Event::Polled(generation, a, q)),
                    );
                }
            }
            Event::Polled(g, approvals, questions) if g == generation => {
                self.interactions.polling = false;
                self.interactions.poll_error.clear();
                match approvals {
                    Ok((items, count)) => {
                        self.interactions.grant_count = count;
                        self.interactions.approvals = items
                            .into_iter()
                            .filter(|a| {
                                a.current(&self.session)
                                    && !self.interactions.completed.contains(&a.request_id)
                            })
                            .collect()
                    }
                    Err(e) => self.interactions.poll_error = format!("审批刷新失败：{e}"),
                }
                match questions {
                    Ok(items) => {
                        self.interactions.questions = items
                            .into_iter()
                            .filter(|q| {
                                q.session_id == self.session.id
                                    && q.status == "pending"
                                    && !self.interactions.completed.contains(&q.request_id)
                            })
                            .collect()
                    }
                    Err(e) => self
                        .interactions
                        .poll_error
                        .push_str(&format!(" 问题刷新失败：{e}")),
                }
            }
            Event::RevokeGrants => {
                if let Some(backend) = self.backend.clone() {
                    let session = self.session.clone();
                    return Task::perform(
                        async move {
                            backend
                                .request(
                                    "POST",
                                    "/api/approval/revoke-session",
                                    json!({"session_id":session.id,"user_id":session.user}),
                                )
                                .await
                                .map(|_| ())
                        },
                        move |r| Message::Interaction(Event::GrantsRevoked(generation, r)),
                    );
                }
            }
            Event::GrantsRevoked(g, result) if g == generation => match result {
                Ok(()) => self.interactions.grant_count = 0,
                Err(e) => self.interactions.poll_error = e,
            },
            Event::Approval(id, approve, remember) if !self.interactions.pending.contains(&id) => {
                if let (Some(backend), Some(approval)) = (
                    self.backend.clone(),
                    self.interactions
                        .approvals
                        .iter()
                        .find(|a| a.request_id == id && a.current(&self.session))
                        .cloned(),
                ) {
                    self.interactions.pending.insert(id.clone());
                    self.interactions.errors.remove(&id);
                    return Task::perform(
                        backend.act_approval(approval, approve, remember),
                        move |r| Message::Interaction(Event::Done(generation, id.clone(), r)),
                    );
                }
            }
            Event::Select(id, option) if !self.interactions.pending.contains(&id) => {
                if let Some(q) = self
                    .interactions
                    .questions
                    .iter()
                    .find(|q| q.request_id == id)
                {
                    if q.options.iter().any(|o| o.id == option) {
                        let draft = self.interactions.drafts.entry(id).or_default();
                        if !q.multiple {
                            draft.selected = vec![option];
                        } else if draft.selected.contains(&option) {
                            draft.selected.retain(|s| s != &option);
                        } else {
                            draft.selected.push(option);
                        }
                    }
                }
            }
            Event::Text(id, text) if !self.interactions.pending.contains(&id) => {
                self.interactions.drafts.entry(id).or_default().text = text
            }
            Event::Answer(id, skip) if !self.interactions.pending.contains(&id) => {
                if let (Some(backend), Some(q)) = (
                    self.backend.clone(),
                    self.interactions
                        .questions
                        .iter()
                        .find(|q| q.request_id == id),
                ) {
                    match answer(
                        q,
                        self.interactions.drafts.entry(id.clone()).or_default(),
                        skip,
                    ) {
                        Ok(body) => {
                            self.interactions.pending.insert(id.clone());
                            self.interactions.errors.remove(&id);
                            let request_id = id.clone();
                            return Task::perform(
                                async move { backend.answer_question(&request_id, body).await },
                                move |r| {
                                    Message::Interaction(Event::Done(generation, id.clone(), r))
                                },
                            );
                        }
                        Err(e) => {
                            self.interactions.errors.insert(id, e);
                        }
                    }
                }
            }
            Event::Done(g, id, result) if g == generation => {
                self.interactions.pending.remove(&id);
                match result {
                    Ok(()) => {
                        self.interactions.completed.insert(id.clone());
                        self.interactions.drafts.remove(&id);
                        self.interactions.approvals.retain(|a| a.request_id != id);
                        self.interactions.questions.retain(|q| q.request_id != id);
                    }
                    Err(e) => {
                        self.interactions.errors.insert(id, e);
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
    pub(super) fn interaction_view(&self) -> Element<'_, Message> {
        let state = &self.interactions;
        let mut cards = Column::new().spacing(8);
        if state.grant_count > 0 {
            cards = cards.push(
                button(text(format!("已记住 {} 项临时授权 · 清除", state.grant_count)).size(12))
                    .style(crate::ui::nav)
                    .on_press(Message::Interaction(Event::RevokeGrants)),
            );
        }
        if !state.poll_error.is_empty() {
            cards = cards.push(text(&state.poll_error).size(12));
        }
        if let Some(a) = state.approvals.iter().find(|a| a.current(&self.session)) {
            let available = !state.pending.contains(&a.request_id);
            let title = if a.tool_display_name.is_empty() {
                &a.tool_name
            } else {
                &a.tool_display_name
            };
            let mut card =
                column![
                    text(format!("需要你的确认 · {title} · {}", a.severity)).size(14),
                    text(format!(
                        "{}\n{}\n{}\n{}\n{}",
                        a.exact_target,
                        a.action_detail,
                        a.justification.as_deref().unwrap_or_default(),
                        a.permission_increment,
                        if a.result_summary.is_null() {
                            String::new()
                        } else {
                            a.result_summary.to_string()
                        }
                    ))
                    .size(13),
                    row![
                        button("拒绝")
                            .style(crate::ui::nav)
                            .padding([8, 12])
                            .on_press_maybe(available.then(|| Message::Interaction(
                                Event::Approval(a.request_id.clone(), false, false)
                            ))),
                        button(if available {
                            "允许本次"
                        } else {
                            "提交中…"
                        })
                        .on_press_maybe(available.then(|| {
                            Message::Interaction(Event::Approval(a.request_id.clone(), true, false))
                        }))
                    ]
                    .spacing(8)
                ]
                .spacing(8);
            if a.allow_session {
                card = card.push(button("当前会话记住相同参数（1 小时）").on_press_maybe(
                    available.then(|| {
                        Message::Interaction(Event::Approval(a.request_id.clone(), true, true))
                    }),
                ));
            }
            if let Some(e) = state.errors.get(&a.request_id) {
                card = card.push(text(e).size(12));
            }
            cards = cards.push(
                container(card)
                    .padding(14)
                    .width(Fill)
                    .style(crate::ui::card),
            );
        }
        if let Some(q) = state.questions.first() {
            let available = !state.pending.contains(&q.request_id);
            let draft = state.drafts.get(&q.request_id);
            let mut card =
                column![text("需要你的回答").size(13), text(&q.title).size(15)].spacing(8);
            for option in &q.options {
                let selected = draft.is_some_and(|d| d.selected.contains(&option.id));
                card = card.push(
                    button(
                        text(format!(
                            "{} {}",
                            if selected { "●" } else { "○" },
                            option.label
                        ))
                        .size(14),
                    )
                    .width(Fill)
                    .style(if selected {
                        crate::ui::selected
                    } else {
                        crate::ui::nav
                    })
                    .padding([8, 12])
                    .on_press_maybe(available.then(|| {
                        Message::Interaction(Event::Select(q.request_id.clone(), option.id.clone()))
                    })),
                );
            }
            let id = q.request_id.clone();
            card = card.push(
                text_input(
                    "填写回答或补充说明…",
                    draft.map(|d| d.text.as_str()).unwrap_or(""),
                )
                .id(iced::widget::Id::from(format!("question-{id}")))
                .on_input_maybe(
                    available.then_some(move |s| Message::Interaction(Event::Text(id.clone(), s))),
                )
                .padding(10),
            );
            card =
                card.push(
                    row![
                        button("跳过")
                            .style(crate::ui::nav)
                            .padding([8, 12])
                            .on_press_maybe(available.then(|| Message::Interaction(
                                Event::Answer(q.request_id.clone(), true)
                            ))),
                        button(if available { "提交" } else { "提交中…" })
                            .style(crate::ui::selected)
                            .padding([8, 12])
                            .on_press_maybe(available.then(|| Message::Interaction(
                                Event::Answer(q.request_id.clone(), false)
                            )))
                    ]
                    .spacing(8),
                );
            if let Some(e) = state.errors.get(&q.request_id) {
                card = card.push(text(e).size(12));
            }
            if state.questions.len() > 1 {
                card =
                    card.push(text(format!("还有 {} 个问题", state.questions.len() - 1)).size(12));
            }
            cards = cards.push(
                container(card)
                    .padding(14)
                    .width(Fill)
                    .style(crate::ui::card),
            );
        }
        container(scrollable(cards).height(iced::Length::Shrink))
            .max_height(self.window_size.height * 0.38)
            .max_width(720)
            .into()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approval_cards_accept_absent_null_and_text_justification() {
        let mut card = json!({"request_id":"a","root_session_id":"s","user_id":"default","tool_name":"read_file","driver":null,"created_at":chrono::Utc::now().timestamp(),"timeout_seconds":300});
        assert!(serde_json::from_value::<Approval>(card.clone()).is_ok());
        for justification in [Value::Null, json!(""), json!("Run project tests")] {
            card["justification"] = justification;
            assert!(serde_json::from_value::<Approval>(card.clone()).is_ok());
        }
    }
    fn question() -> Question {
        serde_json::from_value(json!({"request_id":"q","session_id":"s","title":"choose","status":"pending","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]})).unwrap()
    }
    #[test]
    fn approvals_require_live_matching_session_and_user() {
        let session = crate::backend::Session {
            id: "s".into(),
            user: "u".into(),
            channel: "console".into(),
        };
        let mut a: Approval = serde_json::from_value(json!({"request_id":"a","root_session_id":"s","user_id":"u","tool_name":"shell","created_at":chrono::Utc::now().timestamp(),"timeout_seconds":60})).unwrap();
        assert!(a.current(&session));
        a.user_id = "other".into();
        assert!(!a.current(&session));
        a.user_id = "u".into();
        a.root_session_id = "other".into();
        assert!(!a.current(&session));
        a.root_session_id = "s".into();
        a.created_at = 0.;
        assert!(!a.current(&session));
    }
    #[test]
    fn completed_question_is_not_resurrected_by_inflight_poll() {
        let mut app = App::default();
        app.session.id = "s".into();
        let _ = app.interaction(Event::Done(0, "q".into(), Ok(())));
        let _ = app.interaction(Event::Polled(0, Ok((vec![], 0)), Ok(vec![question()])));
        assert!(app.interactions.questions.is_empty());
    }
    #[test]
    fn validates_and_normalizes_answers() {
        let q = question();
        assert!(answer(&q, &Draft::default(), false).is_err());
        assert!(answer(
            &q,
            &Draft {
                selected: vec!["a".into(), "b".into()],
                text: String::new()
            },
            false
        )
        .is_err());
        assert_eq!(
            answer(
                &q,
                &Draft {
                    selected: vec!["a".into(), "a".into(), "unknown".into()],
                    text: " hi ".into()
                },
                false
            )
            .unwrap(),
            json!({"selected":["a"],"text":"hi","skip":false})
        );
        assert_eq!(
            answer(
                &q,
                &Draft {
                    selected: vec!["a".into()],
                    text: "ignore".into()
                },
                true
            )
            .unwrap(),
            json!({"selected":[],"text":"","skip":true})
        );
    }
    #[test]
    fn ignores_stale_polls_and_preserves_failed_answer_draft() {
        let mut app = App {
            generation: 2,
            ..App::default()
        };
        let _ = app.interaction(Event::Polled(1, Ok((vec![], 0)), Ok(vec![question()])));
        assert!(app.interactions.questions.is_empty());
        app.interactions.drafts.insert(
            "q".into(),
            Draft {
                selected: vec!["a".into()],
                text: "keep".into(),
            },
        );
        app.interactions.pending.insert("q".into());
        let _ = app.interaction(Event::Done(2, "q".into(), Err("retry".into())));
        assert_eq!(app.interactions.drafts["q"].text, "keep");
        assert!(!app.interactions.pending.contains("q"));
    }
}
