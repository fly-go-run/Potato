//! Host-owned recovery notifications. They never create user authorization.
//! Pending notifications are intentionally memory-only: restart never replays.
use crate::{execution_recovery::Guard, lock, protocol, required, Error, Result, Runtime};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(crate) struct State {
    pending: HashMap<String, Notice>,
}

pub(crate) struct Notice {
    pub session: String,
    pub job: String,
    pub body: Value,
    pub guard: Guard,
    pub cancel: CancellationToken,
}

impl Runtime {
    pub(crate) fn shell_authority(&self, session: &str) -> Result<String> {
        let db = self.db()?;
        let Some(chat) = db.chats()?.into_iter().find(|c| c["session_id"] == session) else {
            return Ok(String::new());
        };
        if chat["archived"] == true {
            return Err(Error::new(
                409,
                "Conversation archived; shell recovery stopped",
            ));
        }
        let history = db.history(required(&chat, "id")?, false)?;
        Ok(crate::reviewer_cache::hash(
            &history.iter().rev().find(|m| m["role"] == "user"),
        ))
    }

    // Persist the budget across process restarts and tool call IDs. Only a new
    // real user message resets it; changing command spelling does not.
    pub(crate) fn charge_shell_recovery(&self, session: &str, explicit: bool) -> Result<()> {
        let authority = self.shell_authority(session)?;
        let db = self.db()?;
        let key = format!("shell_recovery_budget:{session}");
        let mut budget = db.get(&key, Value::Null)?;
        let used = if budget["authority"] == authority {
            budget["used"].as_u64().unwrap_or(0)
        } else {
            0
        };
        // Initial user-requested network/host execution is not a failed-job retry.
        if explicit && used == 0 {
            return Ok(());
        }
        if used >= 3 {
            return Err(Error::new(409,"本次用户请求的 3 次沙箱恢复计划已用完。不要通过改写命令继续申请；请使用当前权限内的替代方案，或说明具体阻碍。"));
        }
        budget = json!({"authority":authority,"used":used+1});
        db.put(&key, &budget)
    }

    pub(crate) fn queue_shell_followup(&self, notice: Notice) -> Result<()> {
        let mut state = lock(&self.shell_followups)?;
        if state.pending.len() >= 64 {
            return Err(Error::new(409, "Too many pending shell recoveries"));
        }
        self.jobs.annotate(
            &notice.session,
            &notice.job,
            json!({"continuation":"pending"}),
        )?;
        state.pending.insert(notice.job.clone(), notice);
        Ok(())
    }

    pub(crate) fn acknowledge_shell_followup(&self, session: &str, job: &str) -> Result<()> {
        let mut state = lock(&self.shell_followups)?;
        if state.pending.get(job).is_some_and(|n| n.session == session) {
            state.pending.remove(job);
            self.jobs
                .annotate(session, job, json!({"continuation":"observed"}))?;
        }
        Ok(())
    }

    pub(crate) fn tick_shell_followups(self: &Arc<Self>) -> Result<()> {
        // User follow-ups have priority; the scheduler ticks their outbox first.
        // Drain before calling start: it takes runs -> permissions -> store.
        let ids: Vec<_> = lock(&self.shell_followups)?
            .pending
            .keys()
            .cloned()
            .collect();
        for id in ids {
            let notice = lock(&self.shell_followups)?.pending.remove(&id);
            let Some(notice) = notice else {
                continue;
            };
            let state = self.jobs.state(&notice.session, &id)?;
            if state["continuation"] == "observed" {
                continue;
            }
            if crate::jobs::active(&state) || lock(&self.runs)?.contains_key(&notice.session) {
                lock(&self.shell_followups)?.pending.insert(id, notice);
                continue;
            }
            let mut body = notice.body.clone();
            body["session_id"] = json!(notice.session);
            body["input"] = json!([{"role":"user","content":[{"type":"text","text":format!("Runtime event, not a user message and not new authorization: background shell job {} requires diagnosis. Read job_output, inspect partial effects, then continue only the remaining authorized work. Do not blindly replay the compound command.",id)}]}]);
            let session = notice.session.clone();
            self.jobs
                .annotate(&session, &id, json!({"continuation":"dispatched"}))?;
            if let Err(error) = self.start_shell_followup(body, notice) {
                self.jobs.annotate(
                    &session,
                    &id,
                    json!({"continuation":"stopped","continuation_error":error.message}),
                )?;
            }
            self.notify_background(&session);
        }
        Ok(())
    }

    pub(crate) fn append_shell_notice(&self, chat: &str, text: &str, job: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut frame = protocol::message(
            &id,
            "message",
            "assistant",
            json!([protocol::text(
                &id,
                "后台命令受限，正在检查已有结果并继续处理。",
                false
            )]),
            "completed",
        );
        frame["phase"] = json!("commentary");
        frame["metadata"] = json!({"source":"shell_recovery","job_id":job});
        // Display history retains host provenance. Reviewer authority comes from
        // original display user frames, never this transport notification.
        let mut db = self.db()?;
        db.append(chat, &frame, Some(&json!({"role":"user","content":text})))?;
        let session = db.chat(chat)?["session_id"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        db.put(&format!("shell_recovery_revision:{session}"), &json!(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn user(r: &Runtime, text: &str) {
        let mut db = r.db().unwrap();
        let chat = db.ensure_chat("s", text).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        db.append(
            chat["id"].as_str().unwrap(),
            &protocol::message(
                &id,
                "message",
                "user",
                json!([protocol::text(&id, text, false)]),
                "completed",
            ),
            None,
        )
        .unwrap();
    }
    #[test]
    fn recovery_budget_survives_restart_and_only_real_user_input_resets_it() {
        let dir = tempfile::tempdir().unwrap();
        let r = Runtime::open(dir.path()).unwrap();
        user(&r, "Run tests");
        r.charge_shell_recovery("s", true).unwrap();
        for _ in 0..3 {
            r.charge_shell_recovery("s", false).unwrap();
        }
        assert_eq!(r.charge_shell_recovery("s", true).unwrap_err().status, 409);
        let chat = r.db().unwrap().chats().unwrap().remove(0);
        r.append_shell_notice(
            chat["id"].as_str().unwrap(),
            "Runtime recovery notification",
            "fixture",
        )
        .unwrap();
        assert!(r.charge_shell_recovery("s", false).is_err());
        drop(r);
        let r = Runtime::open(dir.path()).unwrap();
        assert!(r.charge_shell_recovery("s", false).is_err());
        user(&r, "Please continue with this new approach");
        r.charge_shell_recovery("s", false).unwrap();
    }
}
