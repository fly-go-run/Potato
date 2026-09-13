//! Session-bound shell jobs. Stream archives survive restarts; live processes do
//! not resume implicitly. This module owns process lifetime, not the model loop.
use crate::{Error, Result, lock};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::sync::CancellationToken;

struct Job {
    session: String,
    state: Mutex<Value>,
    cancel: CancellationToken,
}
pub(crate) fn active(state: &Value) -> bool {
    matches!(
        state["status"].as_str(),
        Some("running" | "diagnosing" | "reviewing" | "awaiting_approval" | "retrying")
    )
}

#[derive(Clone)]
pub(crate) struct Progress {
    job: Arc<Job>,
    directory: PathBuf,
    notify: Arc<dyn Fn() + Send + Sync>,
}
impl Progress {
    pub fn id(&self) -> String {
        self.directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }
    pub fn update(&self, fields: Value) -> Result<()> {
        let mut state = lock(&self.job.state)?;
        for (key, value) in fields.as_object().into_iter().flatten() {
            state[key] = value.clone();
        }
        save_state(&self.directory, &state)?;
        drop(state);
        (self.notify)();
        Ok(())
    }
    pub fn attempt(&self, index: usize, context: Value) -> Result<PathBuf> {
        let directory = self.directory.join(format!("attempt-{index}"));
        std::fs::create_dir(&directory)?;
        let mut state = lock(&self.job.state)?;
        if !state["attempts"].is_array() {
            state["attempts"] = json!([]);
        }
        state["attempts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"attempt":index,"sandbox":context}));
        state["current_attempt"] = json!(index);
        state["sandbox"] = context;
        state["status"] = json!(if index == 0 { "running" } else { "retrying" });
        state["stdout_path"] = json!(directory.join("stdout"));
        state["stderr_path"] = json!(directory.join("stderr"));
        save_state(&self.directory, &state)?;
        drop(state);
        (self.notify)();
        Ok(directory)
    }
    pub fn finish_attempt(&self, index: usize, result: &Result<Value>) -> Result<()> {
        let mut state = lock(&self.job.state)?;
        let item = &mut state["attempts"][index];
        match result {
            Ok(out) => {
                item["exit_code"] = out["exit_code"].clone();
                item["signal"] = out["signal"].clone();
                if out["cleanup_error"].is_string() {
                    item["cleanup_error"] = out["cleanup_error"].clone();
                }
            }
            Err(error) => {
                item["error"] = json!(error.message);
                item["error_status"] = json!(error.status);
            }
        }
        save_state(&self.directory, &state)
    }
}

pub(crate) struct Jobs {
    root: PathBuf,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
}
impl Drop for Jobs {
    fn drop(&mut self) {
        if let Ok(jobs) = self.jobs.lock() {
            for job in jobs.values() {
                job.cancel.cancel();
            }
        }
    }
}

impl Jobs {
    pub fn open(root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        let mut jobs = HashMap::new();
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            let id = entry.file_name().to_string_lossy().to_string();
            if uuid::Uuid::parse_str(&id).is_err() || !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path().join("state.json");
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(mut state) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            let Some(session) = state["session_id"].as_str().map(str::to_owned) else {
                continue;
            };
            let archive = state["current_attempt"]
                .as_u64()
                .filter(|n| *n < 3)
                .map(|n| entry.path().join(format!("attempt-{n}")))
                .unwrap_or_else(|| entry.path());
            let stdout = json!(archive.join("stdout"));
            let stderr = json!(archive.join("stderr"));
            let paths_changed = state["stdout_path"] != stdout || state["stderr_path"] != stderr;
            state["stdout_path"] = stdout;
            state["stderr_path"] = stderr;
            let interrupted = active(&state);
            if state["continuation"] == "pending" || state["continuation"] == "dispatched" {
                state["continuation"] = json!("interrupted");
                save_state(&entry.path(), &state)?;
            }
            if interrupted {
                state["status"] = json!("interrupted");
                state["error"] = json!(
                    "Runtime restarted; this command was not resumed. Inspect the archived output before retrying."
                );
            }
            if interrupted || paths_changed {
                if let Err(error) = save_state(&entry.path(), &state) {
                    state["persistence_error"] = json!(error.message);
                }
            }
            jobs.insert(
                id,
                Arc::new(Job {
                    session,
                    state: Mutex::new(state),
                    cancel: CancellationToken::new(),
                }),
            );
        }
        Ok(Self {
            root,
            jobs: Mutex::new(jobs),
        })
    }
    #[cfg(all(test, unix))]
    pub fn start(
        &self,
        session: &str,
        command: String,
        cwd: PathBuf,
        timeout: u64,
        cancel: CancellationToken,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<String> {
        let metadata = json!({"command":command,"cwd":cwd});
        self.start_with(
            session,
            metadata,
            cancel,
            notify,
            move |_, directory, token| async move {
                let plan = crate::sandbox::Plan::new(
                    command,
                    cwd.clone(),
                    cwd.clone(),
                    cwd.join("private"),
                    "danger-full-access".into(),
                    cwd,
                );
                crate::processes::execute_spooled(&plan, timeout, &token, Some(&directory)).await
            },
        )
    }
    pub fn start_with<F, Fut>(
        &self,
        session: &str,
        metadata: Value,
        cancel: CancellationToken,
        notify: Arc<dyn Fn() + Send + Sync>,
        run: F,
    ) -> Result<String>
    where
        F: FnOnce(Progress, PathBuf, CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Value>> + Send + 'static,
    {
        let mut jobs = lock(&self.jobs)?;
        if jobs
            .values()
            .filter(|j| lock(&j.state).is_ok_and(|s| active(&s)))
            .count()
            >= 16
        {
            return Err(Error::new(409, "Too many running shell jobs (16)"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let directory = self.root.join(&id);
        std::fs::create_dir(&directory)?;
        let mut state = json!({"job_id":id,"session_id":session,"status":"running","stdout_path":directory.join("stdout"),"stderr_path":directory.join("stderr"),"created_at":chrono::Utc::now().to_rfc3339()});
        for (key, value) in metadata.as_object().into_iter().flatten() {
            state[key] = value.clone();
        }
        save_state(&directory, &state)?;
        let job = Arc::new(Job {
            session: session.into(),
            state: Mutex::new(state),
            cancel,
        });
        jobs.insert(id.clone(), job.clone());
        drop(jobs);
        tokio::spawn(async move {
            let progress = Progress {
                job: job.clone(),
                directory: directory.clone(),
                notify: notify.clone(),
            };
            let result = run(progress, directory.clone(), job.cancel.clone()).await;
            if let Ok(mut state) = lock(&job.state) {
                state["completed_at"] = json!(chrono::Utc::now().to_rfc3339());
                match result {
                    Ok(output) => {
                        state["status"] = json!(if output["return_code"].is_null() {
                            "terminated"
                        } else {
                            "completed"
                        });
                        state["signal"] = output["signal"].clone();
                        state["exit_code"] = output["return_code"].clone();
                        state["return_code"] = output["return_code"].clone();
                        state["preview"] = output;
                    }
                    Err(error) => {
                        state["status"] = json!(if job.cancel.is_cancelled() {
                            "cancelled"
                        } else if error.status == 408 {
                            "timed_out"
                        } else {
                            "failed"
                        });
                        state["error"] = json!(error.message);
                        state["error_status"] = json!(error.status);
                    }
                }
                if let Err(error) = save_state(&directory, &state) {
                    state["persistence_error"] = json!(error.message);
                }
            }
            notify();
        });
        Ok(id)
    }
    fn get(&self, session: &str, id: &str) -> Result<Arc<Job>> {
        lock(&self.jobs)?
            .get(id)
            .filter(|j| j.session == session)
            .cloned()
            .ok_or_else(|| Error::new(404, "Job not found in this conversation"))
    }
    pub fn list(&self, session: &str) -> Result<Value> {
        let jobs = lock(&self.jobs)?;
        let mut result = Vec::new();
        for job in jobs.values().filter(|j| j.session == session) {
            let mut state = lock(&job.state)?.clone();
            state.as_object_mut().unwrap().remove("preview");
            result.push(state);
        }
        result.sort_by(|a, b| a["created_at"].as_str().cmp(&b["created_at"].as_str()));
        let total = result.len();
        let keep = total.saturating_sub(100);
        result.drain(..keep);
        Ok(json!({"jobs":result,"total":total,"limit":100}))
    }
    pub fn cancel(&self, session: &str, id: &str) -> Result<Value> {
        let job = self.get(session, id)?;
        job.cancel.cancel();
        Ok(json!({"job_id":id,"cancellation_requested":true}))
    }
    pub fn state(&self, session: &str, id: &str) -> Result<Value> {
        Ok(lock(&self.get(session,id)?.state)?.clone())
    }
    pub fn annotate(&self, session: &str, id: &str, fields: Value) -> Result<()> {
        let job = self.get(session,id)?;
        let mut state = lock(&job.state)?;
        for (k,v) in fields.as_object().into_iter().flatten() { state[k] = v.clone(); }
        save_state(&self.root.join(id),&state)
    }
    pub async fn wait(&self, session: &str, id: &str, cancel: &CancellationToken) -> Result<Value> {
        let job = self.get(session, id)?;
        loop {
            let state = lock(&job.state)?.clone();
            if !active(&state) {
                return Ok(state);
            }
            tokio::select! {
                _=cancel.cancelled()=>{job.cancel.cancel();return Err(Error::new(499,"Command cancelled; partial output remains available through job_output"));},
                _=tokio::time::sleep(Duration::from_millis(25))=>{},
            }
        }
    }
    pub async fn output(&self, session: &str, id: &str, args: &Value) -> Result<Value> {
        let job = self.get(session, id)?;
        let mut state = lock(&job.state)?.clone();
        state.as_object_mut().unwrap().remove("preview");
        let stream = args["stream"].as_str().unwrap_or("stdout");
        if !matches!(stream, "stdout" | "stderr") {
            return Err(Error::new(400, "stream must be stdout or stderr"));
        }
        let offset = args["offset"].as_u64().unwrap_or(0);
        let limit = args["limit"].as_u64().unwrap_or(8000).clamp(4, 16000) as usize;
        let attempt = args
            .get("attempt")
            .map(|v| {
                v.as_u64()
                    .filter(|n| *n < 3)
                    .ok_or_else(|| Error::new(400, "Invalid attempt index"))
            })
            .transpose()?
            .or_else(|| state["current_attempt"].as_u64());
        if let Some(n) = attempt {
            if state["attempts"]
                .as_array()
                .is_none_or(|a| n as usize >= a.len())
            {
                return Err(Error::new(404, "Attempt not found"));
            }
        }
        let directory = attempt
            .map(|n| self.root.join(id).join(format!("attempt-{n}")))
            .unwrap_or_else(|| self.root.join(id));
        let path = directory.join(stream);
        let mut file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                state["output"] = json!("");
                state["next_offset"] = json!(0);
                return Ok(state);
            }
            Err(e) => return Err(e.into()),
        };
        let size = file.metadata().await?.len();
        if offset > size {
            return Err(Error::new(400, "offset exceeds archived stream length"));
        }
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        let mut bytes = vec![0; limit];
        let n = file.read(&mut bytes).await?;
        bytes.truncate(n);
        // Don't split a valid UTF-8 character between successive pages.
        if let Err(e) = std::str::from_utf8(&bytes) {
            if e.error_len().is_none() {
                bytes.truncate(e.valid_up_to());
            }
        }
        state["stream"] = json!(stream);
        state["output"] = json!(String::from_utf8_lossy(&bytes));
        state["offset"] = json!(offset);
        state["next_offset"] = json!(offset + bytes.len() as u64);
        state["total_bytes"] = json!(size);
        state["has_more"] = json!(offset + (bytes.len() as u64) < size);
        state["encoding"] = json!("UTF-8; invalid bytes are displayed with replacement characters");
        Ok(state)
    }
}
fn save_state(directory: &Path, state: &Value) -> Result<()> {
    let temporary = directory.join("state.tmp");
    std::fs::write(&temporary, serde_json::to_vec(state)?)?;
    std::fs::rename(temporary, directory.join("state.json"))?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn background_output_is_bounded_recoverable_and_session_bound() {
        let root = tempfile::tempdir().unwrap();
        let jobs = Jobs::open(root.path().join("jobs")).unwrap();
        let id=jobs.start("one","i=0; while [ $i -lt 2000 ]; do echo '中文-output'; i=$((i + 1)); done; echo failed >&2; exit 7".into(),root.path().into(),10,CancellationToken::new(),Arc::new(||{})).unwrap();
        let state = jobs
            .wait("one", &id, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(state["status"], "completed");
        assert_eq!(state["return_code"], 7);
        assert!(jobs.output("two", &id, &json!({})).await.is_err());
        let mut full = String::new();
        let mut offset = 0;
        loop {
            let page = jobs
                .output("one", &id, &json!({"offset":offset,"limit":997}))
                .await
                .unwrap();
            full.push_str(page["output"].as_str().unwrap());
            if page["has_more"] == false {
                break;
            }
            let next = page["next_offset"].as_u64().unwrap();
            assert!(next > offset);
            offset = next;
        }
        assert_eq!(full, "中文-output\n".repeat(2000));
        drop(jobs);
        let jobs = Jobs::open(root.path().join("jobs")).unwrap();
        assert_eq!(
            jobs.output("one", &id, &json!({"stream":"stderr"}))
                .await
                .unwrap()["output"],
            "failed\n"
        );
    }
}

#[cfg(all(test, unix))]
mod lifecycle_tests {
    use super::*;
    #[tokio::test]
    async fn abnormal_termination_and_timeout_are_distinct_from_nonzero_exit() {
        let dir = tempfile::tempdir().unwrap();
        let jobs = Jobs::open(dir.path().into()).unwrap();
        for (command, timeout, expected) in [
            ("exit 1", 5, "completed"),
            ("kill -TERM $$", 5, "terminated"),
            ("echo partial; sleep 5", 1, "timed_out"),
        ] {
            let cancel = CancellationToken::new();
            let id = jobs
                .start(
                    "s",
                    command.into(),
                    dir.path().into(),
                    timeout,
                    cancel.clone(),
                    Arc::new(|| {}),
                )
                .unwrap();
            let state = jobs.wait("s", &id, &cancel).await.unwrap();
            assert_eq!(state["status"], expected, "{state}");
            if expected == "completed" {
                assert_eq!(state["exit_code"], 1);
            }
            if expected == "terminated" {
                assert!(state["exit_code"].is_null());
                assert_eq!(state["signal"], 15);
            }
            if expected == "timed_out" {
                assert!(
                    jobs.output("s", &id, &json!({})).await.unwrap()["output"]
                        .as_str()
                        .unwrap()
                        .contains("partial")
                );
            }
        }
    }
}
