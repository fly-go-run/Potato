//! Session-bound shell jobs. Stream archives survive restarts; live processes do
//! not resume implicitly. This module owns process lifetime, not the model loop.
use crate::{lock, Error, Result};
use serde_json::{json, Value};
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
            let stdout = json!(entry.path().join("stdout"));
            let stderr = json!(entry.path().join("stderr"));
            let paths_changed = state["stdout_path"] != stdout || state["stderr_path"] != stderr;
            state["stdout_path"] = stdout;
            state["stderr_path"] = stderr;
            let interrupted = state["status"] == "running";
            if interrupted {
                state["status"] = json!("interrupted");
                state["error"]=json!("Runtime restarted; this command was not resumed. Inspect the archived output before retrying.");
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
    pub fn start(
        &self,
        session: &str,
        command: String,
        cwd: PathBuf,
        timeout: u64,
        cancel: CancellationToken,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<String> {
        let mut jobs = lock(&self.jobs)?;
        if jobs
            .values()
            .filter(|j| lock(&j.state).is_ok_and(|s| s["status"] == "running"))
            .count()
            >= 16
        {
            return Err(Error::new(409, "Too many running shell jobs (16)"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let directory = self.root.join(&id);
        std::fs::create_dir(&directory)?;
        let state = json!({"job_id":id,"session_id":session,"status":"running","cwd":cwd,"command":command,"stdout_path":directory.join("stdout"),"stderr_path":directory.join("stderr"),"created_at":chrono::Utc::now().to_rfc3339()});
        save_state(&directory, &state)?;
        let job = Arc::new(Job {
            session: session.into(),
            state: Mutex::new(state),
            cancel,
        });
        jobs.insert(id.clone(), job.clone());
        drop(jobs);
        tokio::spawn(async move {
            let result = crate::processes::execute_spooled(
                &command,
                &cwd,
                timeout,
                &job.cancel,
                Some(&directory),
            )
            .await;
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
    pub async fn wait(&self, session: &str, id: &str, cancel: &CancellationToken) -> Result<Value> {
        let job = self.get(session, id)?;
        loop {
            let state = lock(&job.state)?.clone();
            if state["status"] != "running" {
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
        let path = self.root.join(id).join(stream);
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
                assert!(jobs.output("s", &id, &json!({})).await.unwrap()["output"]
                    .as_str()
                    .unwrap()
                    .contains("partial"));
            }
        }
    }
}
