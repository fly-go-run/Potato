//! In-process core adapter: the native GUI owns no HTTP server.
use futures::channel::{mpsc, oneshot};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
pub type Reply = Result<Value, String>;
#[derive(Clone)]
pub struct Backend {
    pub core: Arc<potato_core::Runtime>,
    pub executor: Arc<tokio::runtime::Runtime>,
    pub data_dir: PathBuf,
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
        Self::open_at(data_dir)
    }
    pub(crate) fn open_at(data_dir: PathBuf) -> anyhow::Result<Self> {
        let executor = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
        );
        let core = potato_core::Runtime::open(&data_dir)?;
        executor.spawn(core.clone().serve_scheduler());
        Ok(Self {
            core,
            executor,
            data_dir,
        })
    }
    pub fn request(&self, method: &str, path: &str, body: Value) -> oneshot::Receiver<Reply> {
        let (tx, rx) = oneshot::channel();
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
