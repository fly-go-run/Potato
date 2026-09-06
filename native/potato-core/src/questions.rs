use crate::{lock, required, Error, Result, Runtime};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

impl Runtime {
    pub(crate) async fn ask_user(
        &self,
        session: &str,
        args: &Value,
        cancel: &CancellationToken,
    ) -> Result<String> {
        let title = required(args, "title")?;
        if title.len() > 4000 {
            return Err(Error::new(400, "Question title is too long"));
        }
        let options = args["options"].as_array().cloned().unwrap_or_default();
        if options.len() > 20 {
            return Err(Error::new(400, "Too many question options"));
        }
        let mut ids = std::collections::HashSet::new();
        for option in &options {
            if !ids.insert(required(option, "id")?) {
                return Err(Error::new(400, "Question option IDs must be unique"));
            }
            required(option, "label")?;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let question = json!({"request_id":id,"session_id":session,"title":title,"options":options,
            "multiple":args["multiple"].as_bool().unwrap_or(false),"status":"pending","created_at":chrono::Utc::now().to_rfc3339()});
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = lock(&self.questions)?;
            self.db()?.save_question(&question)?;
            pending.insert(id.clone(), tx);
        }
        let answer = tokio::select! {
            _=cancel.cancelled()=>None,
            result=rx=>result.ok(),
        };
        let mut pending = lock(&self.questions)?;
        pending.remove(&id);
        let mut stored = self.db()?.question(&id)?;
        if stored["status"] == "pending" {
            stored["status"] = json!("skipped");
            stored["answer"] = json!({"selected":[],"text":""});
            self.db()?.save_question(&stored)?;
        }
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Question cancelled"));
        }
        Ok(answer.unwrap_or(stored).to_string())
    }

    pub(crate) fn answer_question(&self, id: &str, body: &Value) -> Result<Value> {
        let mut pending = lock(&self.questions)?;
        let mut question = self.db()?.question(id)?;
        let skip = body["skip"]
            .as_bool()
            .ok_or_else(|| Error::new(400, "skip must be boolean"))?;
        let selected = body["selected"]
            .as_array()
            .ok_or_else(|| Error::new(400, "selected must be an array"))?;
        let text = body["text"]
            .as_str()
            .ok_or_else(|| Error::new(400, "text must be a string"))?;
        if text.len() > 20_000 {
            return Err(Error::new(400, "Question answer is too long"));
        }
        if question["multiple"] != true && selected.len() > 1 {
            return Err(Error::new(400, "This question accepts a single selection"));
        }
        let options = question["options"].as_array().cloned().unwrap_or_default();
        let mut ids = std::collections::HashSet::new();
        for value in selected {
            let id = value
                .as_str()
                .ok_or_else(|| Error::new(400, "Selection IDs must be strings"))?;
            if !ids.insert(id) || !options.iter().any(|o| o["id"] == id) {
                return Err(Error::new(400, "Invalid question selection"));
            }
        }
        if !skip && selected.is_empty() && text.trim().is_empty() {
            return Err(Error::new(400, "Choose an option or enter an answer"));
        }
        let status = if skip { "skipped" } else { "answered" };
        let answer = if skip {
            json!({"selected":[],"text":""})
        } else {
            json!({"selected":selected,"text":text})
        };
        if question["status"] != "pending" {
            if question["status"] == status && question["answer"] == answer {
                return Ok(question);
            }
            return Err(Error::new(409, "Question already has a different answer"));
        }
        question["status"] = json!(status);
        question["answer"] = answer;
        self.db()?.save_question(&question)?;
        if let Some(tx) = pending.remove(id) {
            let _ = tx.send(question.clone());
        }
        Ok(question)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Duration};
    async fn pending(runtime: &Runtime) -> Value {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let questions = runtime
                    .request("GET", "/api/questions?session_id=s", Value::Null)
                    .await
                    .unwrap();
                if !questions["questions"][0].is_null() {
                    return questions["questions"][0].clone();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn validates_and_persists_idempotent_answers() {
        let root = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(root.path()).unwrap();
        let task_runtime = Arc::clone(&runtime);
        let task = tokio::spawn(async move {
            task_runtime.ask_user("s",&json!({"title":"选择颜色","options":[{"id":"red","label":"红"},{"id":"blue","label":"蓝"}],"multiple":false}),&CancellationToken::new()).await.unwrap()
        });
        let q = pending(&runtime).await;
        let path = format!(
            "/api/questions/{}/answer",
            q["request_id"].as_str().unwrap()
        );
        assert!(runtime
            .request(
                "POST",
                &path,
                json!({"selected":["red","blue"],"text":"","skip":false})
            )
            .await
            .is_err());
        assert!(runtime
            .request(
                "POST",
                &path,
                json!({"selected":["bad"],"text":"","skip":false})
            )
            .await
            .is_err());
        let body = json!({"selected":["blue"],"text":"深蓝","skip":false});
        let answer = runtime.request("POST", &path, body.clone()).await.unwrap();
        assert_eq!(answer["status"], "answered");
        assert_eq!(runtime.request("POST", &path, body).await.unwrap(), answer);
        assert!(task.await.unwrap().contains("深蓝"));
        drop(runtime);
        let runtime = Runtime::open(root.path()).unwrap();
        assert_eq!(
            runtime
                .request("GET", "/api/questions?session_id=s", Value::Null)
                .await
                .unwrap()["questions"][0]["status"],
            "answered"
        );
    }
    #[tokio::test]
    async fn cancellation_releases_waiter_and_persists_skipped() {
        let root = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(root.path()).unwrap();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let task_runtime = Arc::clone(&runtime);
        let task = tokio::spawn(async move {
            task_runtime
                .ask_user("s", &json!({"title":"补充说明"}), &token)
                .await
        });
        pending(&runtime).await;
        cancel.cancel();
        assert!(task.await.unwrap().is_err());
        assert_eq!(
            runtime
                .request("GET", "/api/questions?session_id=s", Value::Null)
                .await
                .unwrap()["questions"][0]["status"],
            "skipped"
        );
    }
}
