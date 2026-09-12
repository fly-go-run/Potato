//! Durable, per-session follow-ups. Lock order: outbox gate -> runs -> store.
//! A dispatch claim is persisted before starting; uncertain claims never replay.
use crate::{lock, required, string, Emit, Error, Result, Runtime};
use serde_json::{json, Value};
use std::sync::Arc;

const KEY: &str = "follow_up_outbox";
fn empty() -> Value {
    json!({"items":[],"paused":false,"reason":"","interrupt":false})
}
impl Runtime {
    pub(crate) fn recover_outbox(&self) -> Result<()> {
        let _gate = lock(&self.outbox_gate)?;
        let db = self.db()?;
        let mut all = db.get(KEY, json!({}))?;
        for queue in all.as_object_mut().unwrap().values_mut() {
            queue["paused"] = json!(true);
            queue["interrupt"] = json!(false);
            queue["reason"] = json!("应用已重启，请确认待发送内容后继续");
            for item in queue["items"].as_array_mut().unwrap() {
                if item["state"] == "dispatching" {
                    item["state"] = json!("uncertain");
                    queue["reason"] = json!("发送结果待确认，请检查历史并删除或编辑该条消息");
                    break;
                }
            }
        }
        db.put(KEY, &all)
    }
    pub(crate) fn outbox_request(&self, body: &Value) -> Result<Value> {
        let session = required(body, "session_id")?;
        let _gate = lock(&self.outbox_gate)?;
        let runs = lock(&self.runs)?;
        let db = self.db()?;
        let mut all = db.get(KEY, json!({}))?;
        let q = all
            .as_object_mut()
            .unwrap()
            .entry(session.to_owned())
            .or_insert_with(empty);
        if q["chat_id"].is_null() {
            q["chat_id"] = db
                .chats()?
                .iter()
                .find(|c| c["session_id"] == session)
                .map(|c| c["id"].clone())
                .unwrap_or(Value::Null);
        }
        let action = string(body, "action");
        if q["interrupt"] == true && !matches!(action, "" | "list" | "pause") {
            return Err(Error::new(409, "正在停止，请稍候"));
        }
        match action {
            "" | "list" => {}
            "add" => {
                let id = required(body, "id")?;
                let request = &body["request"];
                if request["session_id"] != session
                    || request["input"]
                        .as_array()
                        .is_none_or(|v| v.len() != 1 || v[0]["role"] != "user")
                {
                    return Err(Error::new(400, "Invalid queued request"));
                }
                let blocks = request["input"][0]["content"]
                    .as_array()
                    .ok_or_else(|| Error::new(400, "Invalid message"))?;
                if !blocks
                    .iter()
                    .any(|b| string(b, "type") != "text" || !string(b, "text").trim().is_empty())
                {
                    return Err(Error::new(400, "Message is empty"));
                }
                if q["items"].as_array().unwrap().len() >= 100 {
                    return Err(Error::new(400, "待发送消息最多 100 条"));
                }
                let accepted = q["receipts"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|v| v == id));
                if !accepted && !q["items"].as_array().unwrap().iter().any(|i| i["id"] == id) {
                    if !q["receipts"].is_array() {
                        q["receipts"] = json!([]);
                    }
                    let receipts = q["receipts"].as_array_mut().unwrap();
                    receipts.push(json!(id));
                    if receipts.len() > 1024 {
                        receipts.remove(0);
                    }
                    let model = db.get("active", Value::Null)?;
                    q["items"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"id":id,"request":request,"model":model,"state":"pending"}));
                }
                if !accepted && body["immediate"] == true {
                    promote(q, id)?;
                }
            }
            "promote" => {
                promote(q, required(body, "id")?)?;
            }
            "delete" | "edit" | "save" | "cancel_edit" | "up" | "down" => {
                let id = required(body, "id")?;
                let items = q["items"].as_array_mut().unwrap();
                let i = items
                    .iter()
                    .position(|i| i["id"] == id)
                    .ok_or_else(|| Error::new(404, "待发送消息已开始或已删除"))?;
                match action {
                    "delete" => {
                        items.remove(i);
                    }
                    "edit" => {
                        items[i]["state"] = json!("editing");
                    }
                    "cancel_edit" => {
                        items[i]["state"] = json!("pending");
                    }
                    "save" => {
                        let text = required(body, "text")?;
                        let blocks = items[i]["request"]["input"][0]["content"]
                            .as_array_mut()
                            .unwrap();
                        blocks.retain(|b| b["type"] != "text");
                        blocks.insert(0, json!({"type":"text","text":text}));
                        items[i]["request"]["request_context"]["last_user_message"] = json!(text);
                        items[i]["state"] = json!("pending");
                    }
                    "up" if i > 0 => items.swap(i, i - 1),
                    "down" if i + 1 < items.len() => items.swap(i, i + 1),
                    _ => {}
                }
            }
            "resume" => {
                q["paused"] = json!(false);
                q["reason"] = json!("");
            }
            "pause" => {
                q["paused"] = json!(true);
                q["interrupt"] = json!(false);
                q["reason"] = json!("已暂停");
            }
            _ => return Err(Error::new(400, "Unknown outbox action")),
        }
        let mut result = q.clone();
        result["running"] = json!(runs.contains_key(session));
        result["recovery_revision"] = db.get(&format!("shell_recovery_revision:{session}"),Value::Null)?;
        result["chat_id"] = db
            .chats()?
            .iter()
            .find(|c| c["session_id"] == session)
            .map(|c| c["id"].clone())
            .unwrap_or(Value::Null);
        let interrupt = q["interrupt"] == true;
        db.put(KEY, &all)?;
        if interrupt {
            if let Some(run) = runs.get(session) {
                run.cancel.cancel();
            }
        }
        Ok(result)
    }
    pub(crate) fn finish_outbox(&self, session: &str, status: &str) -> Result<()> {
        // Called under the outbox gate, before releasing the session run slot.
        let db = self.db()?;
        let mut all = db.get(KEY, json!({}))?;
        if let Some(q) = all.get_mut(session) {
            if status != "completed" && q["interrupt"] != true {
                q["paused"] = json!(true);
                q["reason"] = json!(if status == "cancelled" {
                    "已停止，待发送消息已暂停"
                } else {
                    "回复失败，待发送消息已保留"
                });
            }
            q["interrupt"] = json!(false);
            db.put(KEY, &all)?;
        }
        Ok(())
    }
    pub(crate) fn tick_outbox(self: &Arc<Self>) -> Result<()> {
        let _gate = lock(&self.outbox_gate)?;
        let mut all = self.db()?.get(KEY, json!({}))?;
        for (session, q) in all.as_object_mut().unwrap() {
            if q["interrupt"] == true
                && chrono::Utc::now().timestamp_millis() - q["interrupt_at"].as_i64().unwrap_or(0)
                    > 15_000
            {
                q["paused"] = json!(true);
                q["interrupt"] = json!(false);
                q["reason"] = json!("停止超时，消息已保留，请等待当前任务结束后继续");
                let mut saved = self.db()?.get(KEY, json!({}))?;
                saved[session] = q.clone();
                self.db()?.put(KEY, &saved)?;
            }
            if let Some(id) = q["chat_id"].as_str() {
                if self.db()?.chat(id).is_err() || self.db()?.chat(id)?["archived"] == true {
                    continue;
                }
            }
            if q["paused"] == true || lock(&self.runs)?.contains_key(session) {
                continue;
            }
            let Some(item) = q["items"].as_array().and_then(|v| v.first()).cloned() else {
                continue;
            };
            if item["state"] != "pending" {
                continue;
            }
            let mut request = item["request"].clone();
            request["queued_model"] = item["model"].clone();
            q["items"][0]["state"] = json!("dispatching");
            // Save this session independently so a crash cannot silently retry a claim.
            let mut saved = self.db()?.get(KEY, json!({}))?;
            saved[session] = q.clone();
            self.db()?.put(KEY, &saved)?;
            let emit: Emit = Arc::new(|_| Ok(()));
            match self.start(uuid::Uuid::new_v4().to_string(), request, emit) {
                Ok(()) => {
                    q["items"].as_array_mut().unwrap().remove(0);
                    q["interrupt"] = json!(false);
                }
                Err(error) => {
                    q["items"][0]["state"] = json!("pending");
                    q["paused"] = json!(true);
                    q["reason"] = json!(error.message);
                }
            }
            saved[session] = q.clone();
            self.db()?.put(KEY, &saved)?;
            self.notify_background(session);
        }
        Ok(())
    }
}
fn promote(q: &mut Value, id: &str) -> Result<()> {
    if q["interrupt"] == true {
        return Err(Error::new(409, "正在停止，请稍候"));
    }
    let items = q["items"].as_array_mut().unwrap();
    let i = items
        .iter()
        .position(|i| i["id"] == id)
        .ok_or_else(|| Error::new(404, "待发送消息已开始"))?;
    if items[i]["state"] != "pending" {
        return Err(Error::new(409, "请先完成编辑或确认发送结果"));
    }
    let item = items.remove(i);
    items.insert(0, item);
    q["paused"] = json!(false);
    q["reason"] = json!("");
    q["interrupt"] = json!(true);
    q["interrupt_at"] = json!(chrono::Utc::now().timestamp_millis());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runtime() -> (tempfile::TempDir, Arc<Runtime>) {
        let dir = tempfile::tempdir().unwrap();
        let rt = Runtime::open(dir.path()).unwrap();
        (dir, rt)
    }
    fn add(rt: &Runtime, id: &str) -> Value {
        rt.outbox_request(&json!({"session_id":"s","action":"add","id":id,"request":{"session_id":"s","input":[{"role":"user","content":[{"type":"text","text":id}]}]}})).unwrap()
    }
    fn action(rt: &Runtime, a: &str, id: &str) -> Value {
        rt.outbox_request(&json!({"session_id":"s","action":a,"id":id}))
            .unwrap()
    }
    #[test]
    fn outbox_is_durable_idempotent_and_pauses_on_restart() {
        let (dir, rt) = runtime();
        add(&rt, "a");
        add(&rt, "a");
        add(&rt, "b");
        assert_eq!(
            action(&rt, "list", "")["items"].as_array().unwrap().len(),
            2
        );
        drop(rt);
        let rt = Runtime::open(dir.path()).unwrap();
        let q = action(&rt, "list", "");
        assert_eq!(q["paused"], true);
        assert_eq!(q["items"][0]["id"], "a");
    }
    #[test]
    fn outbox_promote_preserves_remaining_order_and_edit_blocks_dispatch() {
        let (_dir, rt) = runtime();
        for id in ["a", "b", "c"] {
            add(&rt, id);
        }
        let q = action(&rt, "promote", "c");
        assert_eq!(q["items"][0]["id"], "c");
        assert_eq!(q["items"][1]["id"], "a");
        assert!(rt
            .outbox_request(&json!({"session_id":"s","action":"promote","id":"b"}))
            .is_err());
        action(&rt, "pause", "");
        action(&rt, "edit", "c");
        action(&rt, "resume", "");
        rt.tick_outbox().unwrap();
        assert_eq!(action(&rt, "list", "")["items"][0]["state"], "editing");
    }
    #[test]
    fn outbox_failure_keeps_content_and_manual_stop_pauses() {
        let (_dir, rt) = runtime();
        add(&rt, "a");
        rt.tick_outbox().unwrap();
        let q = action(&rt, "list", "");
        assert_eq!(q["paused"], true);
        assert_eq!(q["items"][0]["state"], "pending");
        action(&rt, "resume", "");
        rt.finish_outbox("s", "cancelled").unwrap();
        assert_eq!(action(&rt, "list", "")["paused"], true);
        action(&rt, "promote", "a");
        rt.finish_outbox("s", "cancelled").unwrap();
        assert_eq!(action(&rt, "list", "")["paused"], false);
    }
    #[test]
    fn outbox_uncertain_claim_never_replays_after_restart() {
        let (dir, rt) = runtime();
        add(&rt, "a");
        {
            let db = rt.db().unwrap();
            let mut all = db.get(KEY, json!({})).unwrap();
            all["s"]["items"][0]["state"] = json!("dispatching");
            db.put(KEY, &all).unwrap();
        }
        drop(rt);
        let rt = Runtime::open(dir.path()).unwrap();
        action(&rt, "resume", "");
        rt.tick_outbox().unwrap();
        assert_eq!(action(&rt, "list", "")["items"][0]["state"], "uncertain");
    }
    #[test]
    fn outbox_retry_after_delete_is_not_a_new_message_and_sessions_are_isolated() {
        let (_dir, rt) = runtime();
        add(&rt, "a");
        action(&rt, "delete", "a");
        assert!(add(&rt, "a")["items"].as_array().unwrap().is_empty());
        let other = rt.outbox_request(&json!({"session_id":"other"})).unwrap();
        assert!(other["items"].as_array().unwrap().is_empty());
        add(&rt, "b");
        action(&rt, "promote", "b");
        {
            let db = rt.db().unwrap();
            let mut all = db.get(KEY, json!({})).unwrap();
            all["s"]["interrupt_at"] = json!(0);
            db.put(KEY, &all).unwrap();
        }
        rt.tick_outbox().unwrap();
        let q = action(&rt, "list", "");
        assert_eq!(q["paused"], true);
        assert_eq!(q["items"][0]["state"], "pending");
        assert!(q["reason"].as_str().unwrap().contains("超时"));
    }
    #[test]
    fn outbox_edit_keeps_attachments_and_queue_position() {
        let (_dir, rt) = runtime();
        rt.outbox_request(&json!({"session_id":"s","action":"add","id":"file","request":{"session_id":"s","input":[{"role":"user","content":[{"type":"text","text":"before"},{"type":"image","image_url":"data:image/png;base64,YQ=="}]}]}})).unwrap();
        add(&rt, "second");
        action(&rt, "edit", "file");
        let q = rt
            .outbox_request(&json!({"session_id":"s","action":"save","id":"file","text":"after"}))
            .unwrap();
        assert_eq!(
            q["items"][0]["request"]["input"][0]["content"][1]["image_url"],
            "data:image/png;base64,YQ=="
        );
        assert_eq!(q["items"][1]["id"], "second");
        assert_eq!(q["items"][0]["state"], "pending");
    }
}
