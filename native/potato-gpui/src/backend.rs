//! In-process core adapter: the native GUI owns no HTTP server.
use futures::channel::{mpsc, oneshot};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
pub type Reply = Result<Value, String>;
#[derive(Clone)]
pub struct Backend {
    #[cfg(test)]
    synchronous_ui_requests: bool,
    pub core: Arc<potato_core::Runtime>,
    pub executor: Arc<tokio::runtime::Runtime>,
    pub data_dir: PathBuf,
    background_sessions: Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>,
}
impl Backend {
    pub fn open() -> anyhow::Result<Self> {
        let data_dir = std::env::var_os("POTATO_NATIVE_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or(
                dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("无法找到用户主目录"))?
                    .join(".potato/native-v1"),
            );
        Self::open_at_with_legacy(data_dir, std::env::var_os("POTATO_NATIVE_DATA_DIR").is_none())
    }
    #[cfg(test)]
    pub(crate) fn open_at(data_dir: PathBuf) -> anyhow::Result<Self> {
        Self::open_at_with_legacy(data_dir, false)
    }
    fn open_at_with_legacy(data_dir: PathBuf, restore_legacy: bool) -> anyhow::Result<Self> {
        let executor = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
        );
        let core = potato_core::Runtime::open(&data_dir)?;
        if restore_legacy { core.restore_local_model_settings()?; }
        let background_sessions = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let changes = background_sessions.clone();
        core.set_background_listener(Arc::new(move |event| {
            if let Some(session) = event["session_id"].as_str() {
                changes.lock().map_err(|_| potato_core::Error::new(500, "Background queue lock failed"))?.insert(session.to_owned());
            }
            Ok(())
        }))?;
        let executable = std::env::current_exe()?;
        let directory = executable
            .parent()
            .ok_or_else(|| anyhow::anyhow!("missing executable directory"))?;
        let bundled = if cfg!(target_os = "macos") && directory.ends_with("Contents/MacOS") {
            directory
                .parent()
                .unwrap()
                .join("Resources/computer-driver")
        } else {
            directory.join("computer-driver")
        };
        let driver_dir = if cfg!(debug_assertions)
            && directory.starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"))
            && !bundled.is_dir()
        {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/computer-driver")
        } else {
            bundled
        };
        let host = if executable
            .to_string_lossy()
            .contains("Potato GPUI Review.app")
        {
            "dev.potato.gpui-review"
        } else {
            "dev.potato.gpui"
        };
        core.configure_computer_driver(
            driver_dir.join(if cfg!(windows) {
                "cua-driver.exe"
            } else {
                "cua-driver"
            }),
            host.into(),
        )?;
        executor.spawn(core.clone().serve_scheduler());
        executor.spawn(core.clone().serve_remote());
        Ok(Self {
            #[cfg(test)]
            synchronous_ui_requests: false,
            core,
            executor,
            data_dir,
            background_sessions,
        })
    }
    pub fn take_background_sessions(&self) -> std::collections::BTreeSet<String> {
        self.background_sessions.lock().map(|mut pending| std::mem::take(&mut *pending)).unwrap_or_default()
    }
    pub fn request(&self, method: &str, path: &str, body: Value) -> oneshot::Receiver<Reply> {
        let (tx, rx) = oneshot::channel();
        // View tests use real core responses, but complete them before the
        // deterministic GPUI scheduler registers a cross-thread waker.
        #[cfg(test)]
        if self.synchronous_ui_requests {
            let result = self
                .executor
                .block_on(self.core.request(method, path, body))
                .map_err(|e| e.message);
            let _ = tx.send(result);
            return rx;
        }
        let (core, method, path) = (self.core.clone(), method.to_owned(), path.to_owned());
        self.executor.spawn(async move {
            let _ = tx.send(
                core.request(&method, &path, body)
                    .await
                    .map_err(|e| e.message),
            );
        });
        rx
    }
    #[cfg(test)]
    pub(crate) fn for_ui_test(data_dir: PathBuf) -> anyhow::Result<Self> {
        let mut backend = Self::open_at(data_dir)?;
        backend.synchronous_ui_requests = true;
        Ok(backend)
    }
    pub fn stream(&self, body: Value) -> mpsc::Receiver<Reply> {
        let (tx, rx) = mpsc::channel(256);
        let sender = Arc::new(std::sync::Mutex::new(tx));
        let output = sender.clone();
        let emit: potato_core::Emit = Arc::new(move |frame| {
            output
                .lock()
                .map_err(|_| potato_core::Error::new(500, "Output lock failed"))?
                .try_send(Ok(frame))
                .map_err(|_| potato_core::Error::new(429, "界面输出队列已满，请重新加载会话"))
        });
        let _guard = self.executor.enter();
        if let Err(e) = self
            .core
            .start(uuid::Uuid::new_v4().to_string(), body, emit)
        {
            let _ = sender.lock().unwrap().try_send(Err(e.message));
        }
        rx
    }
}
pub fn segment(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
pub fn request_body(
    session: &str,
    user: &str,
    channel: &str,
    prompt: &str,
    attachments: Vec<Value>,
) -> Value {
    let mut content = vec![json!({"type":"text","text":prompt})];
    content.extend(attachments);
    json!({"session_id":session,"user_id":user,"channel":channel,"stream":true,
        "input":[{"role":"user","content":content}],"request_context":{"last_user_message":prompt}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{StreamExt, executor::block_on};
    #[test]
    fn adapter_persists_documents_and_rejects_stale_edits() {
        let dir = std::env::temp_dir().join(format!("potato-gpui-test-{}", uuid::Uuid::new_v4()));
        let backend = Backend::open_at(dir.clone()).unwrap();
        let path = format!("/api/workspace/memory/{}", segment("验收笔记.md"));
        let request = |method, body| block_on(backend.request(method, &path, body)).unwrap();
        request("PUT", json!({"content":"原内容", "expected_content":null})).unwrap();
        assert_eq!(request("GET", Value::Null).unwrap()["content"], "原内容");
        request(
            "PUT",
            json!({"content":"新内容", "expected_content":"原内容"}),
        )
        .unwrap();
        assert!(
            request(
                "PUT",
                json!({"content":"过期覆盖", "expected_content":"原内容"})
            )
            .is_err()
        );
        assert_eq!(request("GET", Value::Null).unwrap()["content"], "新内容");
        request("DELETE", Value::Null).unwrap();
        drop(backend);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unconfigured_stream_reports_failure_without_hanging() {
        let dir = std::env::temp_dir().join(format!("potato-gpui-test-{}", uuid::Uuid::new_v4()));
        let backend = Backend::open_at(dir.clone()).unwrap();
        let mut stream = backend.stream(request_body("test", "default", "console", "你好", vec![]));
        let result = backend.executor.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                let mut turn = crate::stream::Turn::default();
                while let Some(frame) = stream.next().await {
                    match frame {
                        Ok(v) => turn.apply(v),
                        Err(_) => return true,
                    }
                    if turn.terminal() {
                        return turn.error.is_some() || turn.status == "failed";
                    }
                }
                false
            })
            .await
            .unwrap()
        });
        assert!(result);
        drop(stream);
        drop(backend);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
