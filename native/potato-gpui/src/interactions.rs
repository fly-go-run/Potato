use crate::view::{muted, row_button};
use crate::*;
use gpui_kit::component::button::*;
use gpui_kit::prelude::*;
#[derive(Default)]
pub struct Interactions {
    pub approvals: Vec<Value>,
    pub questions: Vec<Value>,
    pub choices: BTreeMap<String, Vec<String>>,
    pub polling: bool,
    pub submitting: std::collections::BTreeSet<String>,
    pub completed: std::collections::BTreeSet<String>,
}
impl Potato {
    pub fn poll_interactions(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.interactions.polling || !self.streaming && self.selected.is_none() {
            return;
        }
        self.interactions.polling = true;
        let api = self.backend.clone();
        let session = self.session.clone();
        let a = api.request(
            "GET",
            &format!("/api/approval/list?session_id={}", segment(&session)),
            Value::Null,
        );
        let q = api.request(
            "GET",
            &format!("/api/questions?session_id={}", segment(&session)),
            Value::Null,
        );
        cx.spawn_in(w, async move |this, cx| {
            let (a, q) = futures::join!(a, q);
            let _ = this.update_in(cx, |s, _, cx| {
                s.interactions.polling = false;
                if s.session != session {
                    return;
                }
                let previous = (
                    s.interactions.approvals.clone(),
                    s.interactions.questions.clone(),
                );
                if let Ok(Ok(v)) = a {
                    s.interactions.approvals = array(v["pending_approvals"].clone())
                        .into_iter()
                        .filter(|a| !s.interactions.completed.contains(&string(a, "request_id")))
                        .collect();
                }
                if let Ok(Ok(v)) = q {
                    s.interactions.questions = array(v["questions"].clone())
                        .into_iter()
                        .filter(|v| {
                            pending_for(v, &session)
                                && !s.interactions.completed.contains(&string(v, "request_id"))
                        })
                        .collect();
                }
                if previous
                    != (
                        s.interactions.approvals.clone(),
                        s.interactions.questions.clone(),
                    )
                {
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn submit_interaction(
        &mut self,
        id: &str,
        path: &str,
        body: Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.interactions.submitting.insert(id.into()) {
            return;
        }
        let id = id.to_owned();
        let session = self.session.clone();
        self.request_result("POST", path, body, w, cx, move |s, result, _, _| {
            s.interactions.submitting.remove(&id);
            match result {
                Ok(_) => {
                    s.interactions.completed.insert(id.clone());
                    s.interactions.approvals.retain(|v| v["request_id"] != id);
                    s.interactions.questions.retain(|v| v["request_id"] != id);
                    s.interactions.choices.remove(&id);
                    s.fields.remove(&format!("question-{id}"));
                }
                Err(e) if s.session == session => s.notice = e,
                Err(_) => {}
            }
        });
        cx.notify();
    }
    pub fn interaction_view(
        &mut self,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let now = chrono::Utc::now().timestamp() as f64;
        let mut cards = div().flex().flex_col().gap_3();
        let mut count = 0;
        for (i, a) in self
            .interactions
            .approvals
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a["root_session_id"] == self.session
                    && a["user_id"] == self.user
                    && a["created_at"].as_f64().unwrap_or(0.)
                        + a["timeout_seconds"].as_f64().unwrap_or(0.)
                        > now
            })
        {
            count += 1;
            let id = string(a, "request_id");
            let mut actions = div().flex().justify_end().gap_2();
            for (approve, label) in [(false, "拒绝"), (true, "允许本次")] {
                let a = a.clone();
                let id = id.clone();
                actions = actions.child(Button::new((if approve { "approve" } else { "deny" }, i))
                    .outline().small().label(label)
                    .disabled(self.interactions.submitting.contains(&id))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.submit_interaction(&id, if approve { "/api/approval/approve" } else { "/api/approval/deny" },
                            json!({"request_id":a["request_id"],"session_id":a["root_session_id"],"user_id":a["user_id"],"scope":"exact"}), w, cx);
                    })));
            }
            cards = cards.child(
                div()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child("需要你的确认")
                    .child(muted(
                        format!(
                            "{}\n{}",
                            string(a, "tool_display_name"),
                            string(a, "action_detail")
                        ),
                        cx,
                    ))
                    .child(muted(string(a, "exact_target"), cx))
                    .child(actions),
            );
        }
        let session = self.session.clone();
        for (i, q) in self
            .interactions
            .questions
            .clone()
            .iter()
            .enumerate()
            .filter(|(_, q)| pending_for(q, &session))
        {
            count += 1;
            let id = string(q, "request_id");
            let multiple = q["multiple"] == true;
            let submitting = self.interactions.submitting.contains(&id);
            let mut card = div()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_lg()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(string(q, "title"));
            for (oi, opt) in q["options"].as_array().into_iter().flatten().enumerate() {
                let qid = id.clone();
                let oid = string(opt, "id");
                let selected = self
                    .interactions
                    .choices
                    .get(&id)
                    .is_some_and(|s| s.contains(&oid));
                card = card.child(
                    row_button(format!("question-{i}-{oi}"), string(opt, "label"))
                        .disabled(submitting)
                        .when(selected, |b| b.icon(IconName::Check))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            let list = s.interactions.choices.entry(qid.clone()).or_default();
                            if list.contains(&oid) {
                                list.retain(|v| v != &oid);
                            } else {
                                if !multiple {
                                    list.clear();
                                }
                                list.push(oid.clone());
                            }
                            cx.notify();
                        })),
                );
            }
            let field = self.field(&format!("question-{id}"), "", "也可以填写补充说明", w, cx);
            let mut actions = div().flex().justify_end().gap_2();
            for (skip, label) in [(true, "跳过"), (false, "提交回答")] {
                let id = id.clone();
                let empty = field.read(cx).value().trim().is_empty()
                    && self.interactions.choices.get(&id).is_none_or(Vec::is_empty);
                actions = actions.child(
                    Button::new((if skip { "skip-question" } else { "answer" }, i))
                        .small()
                        .label(label)
                        .when(!skip, |b| b.primary())
                        .disabled(submitting || !skip && empty)
                        .on_click(cx.listener(move |s, _, w, cx| {
                            let text = s.value(&format!("question-{id}"), cx);
                            let selected =
                                s.interactions.choices.get(&id).cloned().unwrap_or_default();
                            if !skip && text.trim().is_empty() && selected.is_empty() {
                                return;
                            }
                            s.submit_interaction(
                                &id,
                                &format!("/api/questions/{}/answer", segment(&id)),
                                json!({"selected":selected,"text":text,"skip":skip}),
                                w,
                                cx,
                            );
                        })),
                );
            }
            card = card
                .child(
                    Input::new(&field)
                        .small()
                        .readonly(submitting)
                        .aria_label("回答或补充说明"),
                )
                .child(actions);
            cards = cards.child(card);
        }
        (count > 0).then(|| {
            div()
                .id("pending-interactions")
                .max_h(px(260.))
                .overflow_y_scroll()
                .child(cards)
                .into_any_element()
        })
    }
}
fn pending_for(question: &Value, session: &str) -> bool {
    question["session_id"] == session && question["status"] == "pending"
}

#[cfg(test)]
mod tests {
    use super::pending_for;
    use serde_json::json;
    #[test]
    fn questions_never_cross_sessions_or_survive_completion() {
        assert!(pending_for(
            &json!({"session_id":"a", "status":"pending"}),
            "a"
        ));
        assert!(!pending_for(
            &json!({"session_id":"a", "status":"pending"}),
            "b"
        ));
        assert!(!pending_for(
            &json!({"session_id":"a", "status":"answered"}),
            "a"
        ));
    }
}
