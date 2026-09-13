use crate::view::{muted, row_button};
use crate::*;
use gpui_kit::component::button::*;
use gpui_kit::prelude::*;
#[derive(Default)]
pub struct Interactions {
    pub approvals: Vec<Value>,
    pub shell_jobs: Vec<Value>,
    pub active_reviews: Vec<Value>,
    pub recent_reviews: Vec<Value>,
    pub review_cache: Value,
    pub questions: Vec<Value>,
    pub choices: BTreeMap<String, Vec<String>>,
    pub polling: bool,
    pub details: std::collections::BTreeSet<String>,
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
                    s.interactions.active_reviews.clone(),
                    s.interactions.recent_reviews.clone(),
                    s.interactions.review_cache.clone(),
                    s.interactions.shell_jobs.clone(),
                );
                if let Ok(Ok(v)) = a {
                    s.interactions.active_reviews = array(v["active_reviews"].clone());
                    s.interactions.recent_reviews = array(v["recent_reviews"].clone());
                    s.interactions.review_cache = v["review_cache"].clone();
                    s.interactions.shell_jobs = array(v["shell_jobs"].clone());
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
                        s.interactions.active_reviews.clone(),
                        s.interactions.recent_reviews.clone(),
                        s.interactions.review_cache.clone(),
                        s.interactions.shell_jobs.clone(),
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
                    s.fields.remove(&format!("approval-directory-{id}"));
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
        let session = self.session.clone();
        let user = self.user.clone();
        for (i, a) in self
            .interactions
            .approvals
            .clone()
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a["root_session_id"] == session
                    && a["user_id"] == user
                    && a["created_at"].as_f64().unwrap_or(0.)
                        + a["timeout_seconds"].as_f64().unwrap_or(0.)
                        > now
            })
        {
            count += 1;
            let id = string(a, "request_id");
            let mut actions = div().flex().justify_end().gap_2();
            let directory_field = if a["allow_directory"] == true {
                Some(self.field(
                    &format!("approval-directory-{id}"),
                    &string(a, "suggested_directory"),
                    "授权目录（绝对路径）",
                    w,
                    cx,
                ))
            } else {
                None
            };
            let scopes = if a["allow_directory"] == true {
                vec![
                    ("deny", "拒绝"),
                    ("exact", "允许本次"),
                    ("session_directory", "本会话允许目录"),
                    ("persistent_directory", "始终允许目录"),
                ]
            } else {
                vec![("deny", "拒绝"), ("exact", "允许本次")]
            };
            for (scope, label) in scopes {
                let approve = scope != "deny";
                let a = a.clone();
                let id = id.clone();
                actions = actions.child(Button::new((scope, i))
                    .outline().small().label(label)
                    .disabled(self.interactions.submitting.contains(&id))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.submit_interaction(&id, if approve { "/api/approval/approve" } else { "/api/approval/deny" },
                            json!({"request_id":a["request_id"],"session_id":a["root_session_id"],"user_id":a["user_id"],"scope":scope,"directory":s.value(&format!("approval-directory-{id}"),cx),"recursive":true}), w, cx);
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
                    .child(div().flex().items_center().justify_between()
                        .child(if a["allow_directory"]==true {"读取文件需要确认"} else {"需要你的确认"})
                        .child(Button::new(("approval-details",i)).small().ghost().label("操作详情").on_click(cx.listener({let id=id.clone();move |s,_,_,cx|{if !s.interactions.details.remove(&id){s.interactions.details.insert(id.clone());}cx.notify();}}))))
                    .child(muted(approval_reason(a),cx))
                    .when(self.interactions.details.contains(&id),|card|card.child(muted(format!("{}\n{}",string(a,"tool_display_name"),string(a,"action_detail")),cx)))
                    .child(muted(string(a, "exact_target"), cx))
                    .when_some(directory_field, |card,field|card
                        .child(muted("目录授权包含子目录，仅允许读取、列目录和搜索；不包含写入或命令执行。可编辑下面的目录范围。",cx))
                        .child(Input::new(&field).small().aria_label("允许读取的目录（含子目录）").readonly(self.interactions.submitting.contains(&id))))
                    .child(actions.flex_wrap()),
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
                .max_h(px(
                    (f32::from(w.viewport_size().height) * 0.45).clamp(240., 380.)
                ))
                .mb_2()
                .overflow_y_scroll()
                .child(cards)
                .into_any_element()
        })
    }

    pub fn approval_history_side_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session = self.session.clone();
        let mut cards = div().flex().flex_col().gap_2();
        let mut count = 0;
        let active: Vec<_> = self
            .interactions
            .active_reviews
            .iter()
            .filter(|review| review["root_session_id"] == session)
            .collect();
        if !active.is_empty() {
            count += 1;
            let mut status = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(format!("正在自动审批 · {} 项操作", active.len()));
            for review in active.iter().take(3) {
                status = status.child(muted(
                    format!(
                        "{} · {}",
                        string(review, "tool_name"),
                        string(review, "model")
                    ),
                    cx,
                ));
            }
            cards = cards.child(status);
        }
        for job in self
            .interactions
            .shell_jobs
            .iter()
            .rev()
            .filter(|job| job["session_id"] == session && !job["recovery"].is_null())
            .take(3)
        {
            count += 1;
            let key = format!("shell-recovery-{}", string(job, "job_id"));
            let expanded = self.interactions.details.contains(&key);
            let mut entry = div().flex().flex_col().gap_2().child(
                Button::new(key.clone())
                    .ghost()
                    .small()
                    .label(shell_recovery_label(job))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if !s.interactions.details.remove(&key) {
                            s.interactions.details.insert(key.clone());
                        }
                        cx.notify();
                    })),
            );
            if expanded {
                entry = entry.child(muted(string(job, "command"), cx));
                if let Some(error) = job["error"].as_str() {
                    entry = entry.child(muted(error.to_owned(), cx));
                }
                if let Some(error) = job["continuation_error"].as_str() {
                    entry = entry.child(muted(error.to_owned(), cx));
                }
                if let Some(error) = job["preview"]["cleanup_error"].as_str() {
                    entry = entry.child(muted(format!("沙箱清理：{error}"), cx));
                }
                if job["recovery"]["stage"] == "needs_diagnosis" {
                    entry = entry.child(muted(
                        "首次执行可能已产生部分结果，助手需要检查后继续执行剩余步骤。",
                        cx,
                    ));
                }
                for attempt in job["attempts"].as_array().into_iter().flatten() {
                    entry = entry.child(muted(
                        format!(
                            "第 {} 次 · {} · 联网 {} · 退出码 {}",
                            attempt["attempt"].as_u64().unwrap_or(0) + 1,
                            if attempt["sandbox"]["unsandboxed"] == true {
                                "宿主权限"
                            } else if attempt["sandbox"]["backend"] == "windows-lpac" {
                                "Windows 沙箱 · 项目只读"
                            } else {
                                "文件沙箱"
                            },
                            if attempt["sandbox"]["network"] == "enabled" {
                                "允许"
                            } else {
                                "禁止"
                            },
                            attempt["exit_code"]
                                .as_i64()
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "—".into())
                        ),
                        cx,
                    ));
                }
            }
            cards = cards.child(entry);
        }
        let reviews: Vec<_> = self
            .interactions
            .recent_reviews
            .iter()
            .filter(|review| review["root_session_id"] == session)
            .cloned()
            .collect();
        if !reviews.is_empty() {
            count += 1;
            let expanded = self.interactions.details.contains("auto-review-history");
            let latest = reviews.last().unwrap();
            let mut history = div().flex().flex_col().gap_2().child(
                Button::new("auto-review-history")
                    .ghost()
                    .small()
                    .label(format!(
                        "自动审批 · {} · {}条记录 {}",
                        review_outcome_label(latest),
                        reviews.len(),
                        if expanded { "收起" } else { "查看" }
                    ))
                    .on_click(cx.listener(|s, _, _, cx| {
                        if !s.interactions.details.remove("auto-review-history") {
                            s.interactions.details.insert("auto-review-history".into());
                        }
                        cx.notify();
                    })),
            );
            if expanded {
                let clearing = self.interactions.submitting.contains("clear-review-cache");
                history = history
                    .child(muted("相同的低风险文件读取可在本任务内复用 10 分钟。文件、用户授权或权限改变后重新审批；重启后不保留。",cx))
                    .child(Button::new("clear-review-cache").ghost().small()
                        .label(if clearing {"正在清除…"} else {"清除本任务的审批复用"})
                        .disabled(clearing)
                        .on_click(cx.listener(|s,_,w,cx| {
                            s.clear_review_cache(w,cx);
                        })));
                for review in reviews.iter().rev() {
                    let mut entry = div()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(format!(
                            "{} · {}",
                            review_outcome_label(review),
                            string(review, "tool_name")
                        ))
                        .child(muted(string(review, "exact_target"), cx))
                        .child(muted(string(review, "rationale"), cx))
                        .child(muted(review_execution_label(review), cx));
                    if let Some(context) = review_context_label(review) {
                        entry = entry.child(muted(context, cx));
                    }
                    history = history.child(entry);
                }
            }
            cards = cards.child(history);
        }
        (count > 0).then(|| cards.into_any_element())
    }

    fn clear_review_cache(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self
            .interactions
            .submitting
            .insert("clear-review-cache".into())
        {
            return;
        }
        let session = self.session.clone();
        self.request_result(
            "DELETE",
            "/api/approval/review-cache",
            json!({"session_id":session,"user_id":self.user}),
            w,
            cx,
            move |s, result, _, cx| {
                s.interactions.submitting.remove("clear-review-cache");
                if s.session != session {
                    return;
                }
                match result {
                    Ok(_) => {
                        s.interactions.review_cache = Value::Null;
                        s.notice = "已清除本任务的审批复用，后续需要审批的操作会重新审查。".into();
                    }
                    Err(error) => s.notice = error,
                }
                cx.notify();
            },
        );
    }
}
fn pending_for(question: &Value, session: &str) -> bool {
    question["session_id"] == session && question["status"] == "pending"
}

fn approval_reason(approval: &Value) -> String {
    if approval["review_outcome"] == "failure" {
        return format!(
            "自动审批未完成，尚未执行此操作。{}",
            string(approval, "review_rationale")
        );
    }
    if approval["review_outcome"] == "ask_user" {
        return format!(
            "自动审批需要你补充授权：{}",
            string(approval, "review_rationale")
        );
    }
    match approval["findings_summary"].as_str().unwrap_or("") {
        "Read outside the conversation project" => {
            "目标位于当前项目之外，需要你确认读取权限。".into()
        }
        "Access sensitive data or persistent agent instructions" => {
            "目标涉及敏感数据或长期生效的助手指令，需要单独确认。".into()
        }
        "Run this command with the computer account's permissions, without an OS sandbox" => {
            "本次命令将使用当前电脑账号的权限执行，不受命令沙箱限制。".into()
        }
        "Run this command inside the OS sandbox" => "命令将在操作系统沙箱中执行。".into(),
        "Sandbox blocked the command; review a single unsandboxed retry" => {
            "命令受到沙箱限制，申请仅本次使用宿主权限重试。".into()
        }
        "Sandbox blocked network access; review a network-enabled retry with file isolation" => {
            "命令联网受到限制，申请联网后重试，文件沙箱仍然生效。".into()
        }
        "Interact with an external application or service" => {
            "此操作将访问外部应用或服务，需要确认。".into()
        }
        "Change persistent user-wide memory" => "此操作将修改跨会话保存的记忆，需要确认。".into(),
        "Approve this concrete action" => "请确认以下操作。".into(),
        other => other.to_owned(),
    }
}

pub fn approval_mode_label(config: &Value) -> &'static str {
    match config["approval_level"].as_str().unwrap_or("AUTO") {
        "STRICT" => "每次确认",
        "NEVER" => "需审批即阻止",
        _ if config["reviewer"].as_str().unwrap_or("model") == "model" => "自动审批",
        _ => "手动审批",
    }
}

fn review_outcome_label(review: &Value) -> &'static str {
    if review["source"] == "model_allow_cache" && review["outcome"] == "allow" {
        return "复用批准";
    }
    if review["source"] == "model_denial_cache" && review["outcome"] == "deny" {
        return "复用拒绝";
    }
    match review["outcome"].as_str() {
        Some("allow") => "已批准",
        Some("deny") => "已拒绝",
        Some("ask_user") => "需你授权",
        Some("failure") => "审批故障",
        Some("cancelled") => "已取消",
        _ => "审批结束",
    }
}

fn review_execution_label(review: &Value) -> String {
    match review["source"].as_str() {
        Some("model_allow_cache") => "沿用本任务内的批准 · 本次未调用审批模型".into(),
        Some("model_denial_cache") => "条件未变，沿用拒绝结果 · 本次未调用审批模型".into(),
        _ => format!(
            "{} · {:.1}秒",
            string(review, "model"),
            review["elapsed_ms"].as_f64().unwrap_or(0.) / 1000.
        ),
    }
}

fn review_context_label(review: &Value) -> Option<String> {
    let context = review.get("review_context")?.as_object()?;
    let count = |key: &str| context.get(key).and_then(Value::as_u64).unwrap_or(0);
    let state =
        if review["source"] == "model_allow_cache" || review["source"] == "model_denial_cache" {
            "已核对当前授权"
        } else if context.get("trunk_reused") == Some(&Value::Bool(true)) {
            "延续审批上下文"
        } else {
            "新建审批上下文"
        };
    Some(format!(
        "{} · {} 条用户消息 · {} 条交互回答 · {} 条参考记录",
        state,
        count("user_messages"),
        count("user_answers"),
        count("untrusted_entries")
    ))
}

fn shell_recovery_label(job: &Value) -> &'static str {
    match job["continuation"].as_str() {
        Some("dispatched") => return "沙箱受限 · 助手正在继续处理",
        Some("completed" | "observed") => return "沙箱受限 · 助手已接续处理",
        Some("stopped" | "interrupted" | "cancelled") => return "沙箱恢复已停止 · 查看详情",
        _ => {}
    }
    match job["status"].as_str() {
        Some("diagnosing") => "沙箱受限 · 正在分析",
        Some("reviewing" | "awaiting_approval") => "沙箱受限 · 正在审批重试",
        Some("retrying") => "审批通过 · 正在重试",
        Some("cancelled") => "命令已停止",
        Some("failed" | "timed_out" | "terminated") => "命令未完成 · 查看原因",
        _ if job["recovery"]["stage"] == "needs_diagnosis" => "沙箱受限 · 等待助手继续处理",
        _ if job["exit_code"] == 0 => "沙箱恢复完成 · 命令成功",
        _ => "命令恢复记录 · 查看详情",
    }
}

#[cfg(test)]
mod tests {
    use super::pending_for;
    use serde_json::json;
    #[test]
    fn automatic_label_requires_an_active_model_reviewer() {
        assert_eq!(
            super::approval_mode_label(&json!({"approval_level":"AUTO"})),
            "自动审批"
        );
        assert_eq!(
            super::approval_mode_label(&json!({"approval_level":"AUTO","reviewer":"model"})),
            "自动审批"
        );
        assert_eq!(
            super::approval_mode_label(&json!({"approval_level":"STRICT","reviewer":"model"})),
            "每次确认"
        );
        assert_eq!(
            super::approval_mode_label(&json!({"approval_level":"NEVER","reviewer":"model"})),
            "需审批即阻止"
        );
        let failure = super::approval_reason(
            &json!({"review_outcome":"failure","review_failure":"connection_or_stream","review_rationale":"审批服务连接失败。"}),
        );
        assert!(failure.contains("尚未执行"));
        assert!(failure.contains("审批服务连接失败"));
        assert!(!failure.contains("connection_or_stream"));
        let reused = json!({"source":"model_allow_cache","outcome":"allow","model":"fixture", "review_context":{"user_messages":2,"user_answers":1,"untrusted_entries":3,"trunk_reused":true}});
        assert_eq!(super::review_outcome_label(&reused), "复用批准");
        assert!(super::review_execution_label(&reused).contains("未调用审批模型"));
        assert!(
            super::review_context_label(&reused)
                .unwrap()
                .starts_with("已核对当前授权")
        );
        let fresh = json!({"source":"model","outcome":"allow","model":"fixture", "review_context":{"trunk_reused":true}});
        assert_eq!(super::review_outcome_label(&fresh), "已批准");
        assert!(!super::review_execution_label(&fresh).contains("未调用"));
        assert!(
            super::review_context_label(&fresh)
                .unwrap()
                .starts_with("延续审批上下文")
        );
    }
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
