//! Durable user steering. Deliver only between complete model/tool exchanges.
use crate::{lock, protocol, required, Emit, Error, Result, Runtime};
use serde_json::{json, Value};

impl Runtime {
    pub(crate) fn steer(&self, body: &Value) -> Result<Value> {
        let session = required(body, "session_id")?;
        let text = required(body, "text")?;
        if text.len() > 64_000 {
            return Err(Error::new(413, "Steering text exceeds 64000 bytes"));
        }
        // The finish boundary takes this same lock: an accepted message can
        // never fall between the last queue check and a successful completion.
        let runs = lock(&self.runs)?;
        let run = runs
            .get(session)
            .filter(|r| r.accepting_steering && !r.cancel.is_cancelled())
            .ok_or_else(|| {
                Error::new(409, "Turn has finished or is stopping; send a new message")
            })?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut frame = protocol::message(
            &id,
            "message",
            "user",
            json!([protocol::text(&id, text, false)]),
            "completed",
        );
        frame["metadata"] = json!({"steering_state":"queued"});
        let mut db = self.db()?;
        let chat = db.ensure_chat(session, text)?;
        db.append(required(&chat, "id")?, &frame, None)?;
        let publish = run.replay.clone();
        drop(db);
        drop(runs);
        // Pending approvals are not actions in progress. Release them so the
        // loop can consume the correction before starting another action.
        lock(&self.approvals)?.retain(|_, a| a.view["root_session_id"] != session);
        let questions = self.db()?.questions(session)?;
        let mut pending = lock(&self.questions)?;
        for q in questions {
            if let Some(id) = q["request_id"].as_str() {
                pending.remove(id);
            }
        }
        drop(pending);
        lock(&publish)?.publish(frame);
        Ok(json!({"id":id,"status":"queued"}))
    }

    pub(crate) fn has_steering(&self, session: &str) -> Result<bool> {
        let db = self.db()?;
        let Some(chat) = db.chats()?.into_iter().find(|c| c["session_id"] == session) else {
            return Ok(false);
        };
        db.has_steering(required(&chat, "id")?)
    }

    pub(crate) fn deliver_steering(&self, chat: &str, emit: &Emit) -> Result<bool> {
        let frames = self.db()?.deliver_steering(chat)?;
        let delivered = !frames.is_empty();
        for frame in frames {
            emit(frame)?;
        }
        Ok(delivered)
    }

    pub(crate) fn finish_unless_steered(&self, chat: &str, session: &str) -> Result<bool> {
        let mut runs = lock(&self.runs)?;
        if self.db()?.has_steering(chat)? {
            return Ok(false);
        }
        if let Some(run) = runs.get_mut(session) {
            run.accepting_steering = false;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio_util::sync::CancellationToken;

    fn register(r: &Runtime, session: &str) -> String {
        let chat = r.db().unwrap().ensure_chat(session, "initial").unwrap();
        lock(&r.runs).unwrap().insert(
            session.into(),
            crate::Run {
                request_id: "request".into(),
                accepting_steering: true,
                cancel: CancellationToken::new(),
                replay: Arc::new(Mutex::new(crate::replay::Replay::new(
                    "request".into(),
                    Arc::new(|_| Ok(())),
                ))),
            },
        );
        chat["id"].as_str().unwrap().to_owned()
    }

    #[test]
    fn completion_boundary_cannot_succeed_with_an_accepted_correction() {
        let dir = tempfile::tempdir().unwrap();
        let r = Runtime::open(dir.path()).unwrap();
        let chat = register(&r, "s");
        r.steer(&json!({"session_id":"s","text":"correction"}))
            .unwrap();
        assert!(!r.finish_unless_steered(&chat, "s").unwrap());
        r.deliver_steering(&chat, &(Arc::new(|_| Ok(())) as Emit))
            .unwrap();
        assert!(r.finish_unless_steered(&chat, "s").unwrap());
        assert_eq!(
            r.steer(&json!({"session_id":"s","text":"late"}))
                .unwrap_err()
                .status,
            409
        );
    }

    #[test]
    fn queued_corrections_survive_restart_and_deliver_exactly_once_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let r = Runtime::open(dir.path()).unwrap();
        let chat = register(&r, "s");
        for text in ["first", "second"] {
            r.steer(&json!({"session_id":"s","text":text})).unwrap();
        }
        assert!(r.db().unwrap().history(&chat, true).unwrap().is_empty());
        drop(r);
        let r = Runtime::open(dir.path()).unwrap();
        let emit: Emit = Arc::new(|_| Ok(()));
        assert!(r.deliver_steering(&chat, &emit).unwrap());
        assert!(!r.deliver_steering(&chat, &emit).unwrap());
        let raw = r.db().unwrap().history(&chat, true).unwrap();
        assert_eq!(
            raw.iter()
                .map(|m| m["content"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        let display = r.db().unwrap().history(&chat, false).unwrap();
        assert_eq!(display.len(), 2);
        assert!(display
            .iter()
            .all(|m| m["metadata"]["steering_state"] == "delivered"));
    }
}
