mod api;
mod cloud;
mod cloud_memory;
#[cfg(test)]
mod cloud_memory_tests;
#[cfg(test)]
mod cloud_tests;
mod remote;
mod remote_models;
#[cfg(test)]
mod remote_tests;
mod approval;
mod reviewer;
mod reviewer_cache;
#[cfg(test)]
mod reviewer_tests;
pub mod permissions;
#[cfg(test)]
mod approval_tests;
pub mod attachments;
mod compaction;
mod computer;
mod context;
mod context_policy;
mod file_ops;
mod file_search;
mod jobs;
mod legacy_settings;
mod local_models;
mod mcp;
mod memory;
mod migration;
mod model;
mod office;
mod outbox;
mod processes;
mod sandbox;
mod execution_recovery;
mod shell_followup;
mod projects;
mod prompts;
pub mod protocol;
mod questions;
mod replay;
pub mod reasoning;
mod scheduler;
mod search;
mod skills;
mod steering;
mod store;
mod tool_execution;
mod tool_registry;
mod tools;
mod transcript;
mod voice;
mod workspace;

use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub type Result<T> = std::result::Result<T, Error>;
pub type Emit = Arc<dyn Fn(Value) -> Result<()> + Send + Sync>;

#[derive(Debug, Serialize)]
pub struct Error {
    pub status: u16,
    pub message: String,
}
impl Error {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(500, e.to_string())
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::new(500, e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::new(400, e.to_string())
    }
}
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        // URLs may contain user-supplied secrets; never forward them to UI/logs.
        Self::new(
            502,
            if e.is_timeout() {
                "Model request timed out"
            } else {
                "Model connection failed"
            },
        )
    }
}

struct Run {
    accepting_steering: bool,
    request_id: String,
    cancel: CancellationToken,
    replay: Arc<Mutex<replay::Replay>>,
}
struct Approval {
    view: Value,
    reply: oneshot::Sender<approval::Reply>,
}

pub struct Runtime {
    remote: Mutex<remote::State>,
    cloud_generation: Mutex<u64>,
    shell_followups: Mutex<shell_followup::State>,
    self_ref: std::sync::OnceLock<std::sync::Weak<Self>>,
    computer: tokio::sync::Mutex<Option<computer::Computer>>,
    mcp_connections: Mutex<HashMap<String, std::sync::Arc<mcp::Connection>>>,
    observations: Mutex<HashMap<String, computer::Observation>>,
    started_at: std::time::Instant,
    background_emit: Arc<Mutex<Option<Emit>>>,
    jobs: jobs::Jobs,
    store: Mutex<store::Store>,
    runs: Mutex<HashMap<String, Run>>,
    approvals: Mutex<HashMap<String, Approval>>,
    approval_grants: Mutex<Vec<approval::Grant>>,
    approval_epochs: Mutex<HashMap<String, u64>>,
    permissions: Mutex<permissions::State>,
    reviews: Mutex<reviewer::State>,
    voices: Mutex<HashMap<String, tokio::sync::mpsc::Sender<voice::Input>>>,
    questions: Mutex<HashMap<String, oneshot::Sender<Value>>>,
    client: reqwest::Client,
    root: PathBuf,
    memory_lock: Mutex<()>,
    outbox_gate: Mutex<()>,
}

pub(crate) fn lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>> {
    value
        .lock()
        .map_err(|_| Error::new(500, "Native runtime lock poisoned"))
}
pub(crate) fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
pub(crate) fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let text = string(value, key);
    if text.trim().is_empty() {
        Err(Error::new(400, format!("{key} is required")))
    } else {
        Ok(text)
    }
}

impl Runtime {
    pub fn open(root: &Path) -> Result<Arc<Self>> {
        // reqwest 0.12 and the MCP transport enable different Rustls providers.
        // Direct WSS connections otherwise panic when both features are unified.
        // Respect an embedding host's provider if it already installed one.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let started = std::time::Instant::now();
        let store = store::Store::open(root)?;
        let jobs_root = store.jobs_root(root)?;
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let runtime = Arc::new(Self {
            remote: Mutex::new(remote::State::default()),
            cloud_generation: Mutex::new(0),
            shell_followups: Mutex::new(shell_followup::State::default()),
            self_ref: std::sync::OnceLock::new(),
            computer: tokio::sync::Mutex::new(None),
            mcp_connections: Mutex::new(HashMap::new()),
            observations: Mutex::new(HashMap::new()),
            started_at: started,
            background_emit: Arc::new(Mutex::new(None)),
            jobs: jobs::Jobs::open(jobs_root)?,
            store: Mutex::new(store),
            runs: Mutex::new(HashMap::new()),
            approvals: Mutex::new(HashMap::new()),
            approval_grants: Mutex::new(Vec::new()),
            approval_epochs: Mutex::new(HashMap::new()),
            permissions: Mutex::new(permissions::State::default()),
            reviews: Mutex::new(reviewer::State::default()),
            voices: Mutex::new(HashMap::new()),
            questions: Mutex::new(HashMap::new()),
            client,
            root: root.to_path_buf(),
            memory_lock: Mutex::new(()),
            outbox_gate: Mutex::new(()),
        });
        let _ = runtime.self_ref.set(Arc::downgrade(&runtime));
        runtime.install_builtin_skills()?;
        runtime.migrate_memory()?;
        runtime.recover_outbox()?;
        runtime.db()?.put(
            "last_startup_ms",
            &json!(started.elapsed().as_millis() as u64),
        )?;
        Ok(runtime)
    }
    fn db(&self) -> Result<MutexGuard<'_, store::Store>> {
        lock(&self.store)
    }

    pub fn set_background_listener(&self, emit: Emit) -> Result<()> {
        *lock(&self.background_emit)? = Some(emit);
        Ok(())
    }

    pub(crate) fn notify_background(&self, session: &str) {
        let emit = lock(&self.background_emit)
            .ok()
            .and_then(|value| value.clone());
        if let Some(emit) = emit {
            let _ = emit(json!({"session_id":session}));
        }
    }

    pub fn cancel(&self, request_id: &str) -> Result<bool> {
        let runs = lock(&self.runs)?;
        let run = runs.iter().find(|(_, r)| {
            r.request_id == request_id || lock(&r.replay).is_ok_and(|r| r.contains(request_id))
        });
        if let Some((session,run)) = run {
            run.cancel.cancel();
            *lock(&self.approval_epochs)?.entry(session.clone()).or_default() += 1;
        }
        Ok(run.is_some())
    }

    /// Reserve and persist the user turn before returning, so the sidebar can
    /// immediately navigate to the new chat. At most one run per session.
    pub fn start(self: &Arc<Self>, request_id: String, body: Value, emit: Emit) -> Result<()> {
        self.start_internal(request_id,body,emit,None)
    }

    pub(crate) fn start_shell_followup(self: &Arc<Self>, body: Value, notice: shell_followup::Notice) -> Result<()> {
        self.start_internal(uuid::Uuid::new_v4().to_string(),body,Arc::new(|_|Ok(())),Some(notice))
    }

    fn start_internal(self: &Arc<Self>, request_id: String, body: Value, emit: Emit, notice: Option<shell_followup::Notice>) -> Result<()> {
        let session = required(&body, "session_id")?.to_owned();
        if body["reconnect"] == true {
            let runs = lock(&self.runs)?;
            let run = runs
                .get(&session)
                .ok_or_else(|| Error::new(409, "Turn already finished; reload its history"))?;
            return lock(&run.replay)?.attach(request_id, emit);
        }
        let input = body["input"]
            .as_array()
            .ok_or_else(|| Error::new(400, "input must be an array"))?;
        if input.len() != 1 || input[0]["role"] != "user" {
            return Err(Error::new(400, "One user message is required"));
        }
        let content = input[0]["content"]
            .as_array()
            .ok_or_else(|| Error::new(400, "content must be an array"))?;
        let mut wire_content = Vec::new();
        let mut title = String::new();
        for block in content {
            match string(block, "type") {
                "text" => {
                    let text = block["text"]
                        .as_str()
                        .ok_or_else(|| Error::new(400, "Message text must be a string"))?;
                    if text.trim().is_empty() {
                        continue;
                    }
                    title.push_str(text);
                    wire_content.push(json!({"type":"text","text":text}));
                }
                "image" => {
                    let url = required(block, "image_url")?;
                    if !url.starts_with("data:image/") && !url.starts_with("https://") {
                        return Err(Error::new(
                            400,
                            "Native image input requires a data URL or HTTPS URL",
                        ));
                    }
                    wire_content.push(json!({"type":"image_url","image_url":{"url":url}}));
                }
                "file" => {
                    use base64::Engine;
                    let encoded = required(block, "file_url")?
                        .strip_prefix("data:text/plain;base64,")
                        .ok_or_else(|| {
                            Error::new(415, "File requires a supported native document parser")
                        })?;
                    if encoded.len() > 1_340_000 {
                        return Err(Error::new(413, "Text attachment exceeds 1 MB"));
                    }
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .map_err(|_| Error::new(400, "Invalid text attachment encoding"))?;
                    if bytes.len() > 1_000_000 {
                        return Err(Error::new(413, "Text attachment exceeds 1 MB"));
                    }
                    let text = String::from_utf8(bytes)
                        .map_err(|_| Error::new(415, "Attachment must be UTF-8 text"))?;
                    if text.contains('\0') {
                        return Err(Error::new(415, "Binary attachment is not text"));
                    }
                    let filename = block["file_name"]
                        .as_str()
                        .or_else(|| block["filename"].as_str())
                        .unwrap_or_default();
                    wire_content.push(json!({"type":"text","text":attachments::model_file_text(block, filename, &text)?}));
                }
                _ => {
                    return Err(Error::new(
                        501,
                        "This attachment type has not been migrated to the native runtime",
                    ))
                }
            }
        }
        if wire_content.is_empty() {
            return Err(Error::new(400, "Message is empty"));
        }
        let connection = if body.get("remote_model").is_some() {
            self.remote_model_connection(&body["remote_model"])?
        } else if body["queued_model"].is_object() {
            self.provider_connection(
                string(&body["queued_model"], "provider_id"),
                string(&body["queued_model"], "model"),
            )?
        } else {
            self.connection()?
        };
        let mut runs = lock(&self.runs)?;
        if runs.contains_key(&session) || runs.values().any(|r| r.request_id == request_id) {
            return Err(Error::new(409, "A turn is already running"));
        }
        if let Some(notice) = &notice { notice.guard.check(&notice.cancel)?; }
        let id = {
            let mut db = self.db()?;
            let chat = db.ensure_chat(&session, if title.is_empty() { "Image" } else { &title })?;
            let id = required(&chat, "id")?.to_owned();
            // Corrections queued before a failed/cancelled/restarted run stay
            // durable and precede the user's next ordinary message.
            db.deliver_steering(&id)?;
            let user_id = uuid::Uuid::new_v4().to_string();
            let mut blocks = content.clone();
            for (index, block) in blocks.iter_mut().enumerate() {
                block["object"] = json!("content");
                block["delta"] = json!(false);
                block["index"] = json!(index);
                block["msg_id"] = json!(user_id);
                block["status"] = json!("completed");
            }
            let mut frame = protocol::message(&user_id, "message", "user", json!(blocks), "completed");
            if body["remote_operation_id"].as_str() == Some(request_id.as_str()) {
                frame["metadata"]["remote_operation_id"] = json!(request_id);
            }
            if notice.is_none() { db.append(
                &id,
                &frame,
                Some(&json!({"role":"user","content":wire_content})),
            )?; }
            id
        };
        if let Some(notice) = &notice { self.append_shell_notice(&id,&title,&notice.job)?; }
        self.db()?.put(&format!("run_outcome:{session}"), &Value::Null)?;
        let cancel = CancellationToken::new();
        let replay = Arc::new(Mutex::new(replay::Replay::new(request_id.clone(), emit)));
        let publish = replay.clone();
        let emit: Emit = Arc::new(move |frame| {
            lock(&publish)?.publish(frame);
            Ok(())
        });
        runs.insert(
            session.clone(),
            Run {
                accepting_steering: true,
                request_id: request_id.clone(),
                cancel: cancel.clone(),
                replay,
            },
        );
        drop(runs);
        let runtime = Arc::clone(self);
        tokio::spawn(async move {
            let result = runtime
                .run_turn(
                    &id,
                    &session,
                    &request_id,
                    &body,
                    connection,
                    &cancel,
                    &emit,
                )
                .await;
            let status = if cancel.is_cancelled() {
                "cancelled"
            } else if result.is_err() {
                "failed"
            } else {
                "completed"
            };
            if let Ok(mut approvals) = lock(&runtime.approvals) {
                approvals.retain(|_, a| a.view["root_session_id"] != session || a.view["background_job_id"].is_string());
            }
            let mut frame = protocol::response(&request_id, &session, status);
            if let Err(error) = result {
                frame["error"] = json!({"code":"NATIVE_TURN_FAILED","message":error.message});
            }
            {
                let _gate = lock(&runtime.outbox_gate);
                let _ = runtime.finish_outbox(&session, status);
                if let Ok(mut runs) = lock(&runtime.runs) {
                    // Store the outcome before allowing a new turn to start and
                    // clear it, so an older completion cannot overwrite it.
                    if let Ok(db) = runtime.db() { let _ = db.put(&format!("run_outcome:{session}"), &frame); }
                    runs.remove(&session);
                }
            }
            let _ = emit(frame);
            runtime.notify_background(&session);
            if let Some(notice) = notice {
                let _ = runtime.jobs.annotate(&session,&notice.job,json!({"continuation":status}));
                runtime.notify_background(&session);
            }
        });
        Ok(())
    }
}
