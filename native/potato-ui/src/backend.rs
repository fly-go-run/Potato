//! In-process adapter: local operations never connect to a backend service.
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, Deserialize)]
pub struct Chat {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default = "default_user")]
    pub user_id: String,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct History {
    pub messages: Vec<Value>,
    #[serde(default)]
    pub status: String,
}

#[derive(Clone)]
pub struct Backend {
    pub core: Arc<potato_core::Runtime>,
    executor: Arc<Executor>,
    updates: Arc<std::sync::atomic::AtomicU64>,
}
struct Executor(Option<tokio::runtime::Runtime>);
impl std::ops::Deref for Executor {
    type Target = tokio::runtime::Runtime;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_background();
        }
    }
}
impl Backend {
    pub fn data_dir() -> Result<PathBuf, String> {
        if let Some(path) = std::env::var_os("POTATO_NATIVE_DATA_DIR") {
            return Ok(path.into());
        }
        Ok(dirs::data_local_dir()
            .ok_or("无法找到应用数据目录")?
            .join("dev.potato.rust-ui")
            .join("native-v1"))
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let executor = Arc::new(Executor(Some(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| "无法初始化任务执行器")?,
        )));
        let core = potato_core::Runtime::open(path).map_err(|e| e.message)?;
        let updates = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let signal = updates.clone();
        core.set_background_listener(Arc::new(move |_| {
            signal.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }))
        .map_err(|e| e.message)?;
        executor.spawn(core.clone().serve_scheduler());
        Ok(Self {
            core,
            executor,
            updates,
        })
    }
    pub fn updates(&self) -> u64 {
        self.updates.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn saved_history(&self, id: String) -> Result<History, String> {
        self.executor.block_on(self.clone().history(id))
    }
    pub fn initial_connection(&self) -> Result<Connection, String> {
        self.executor.block_on(self.clone().bootstrap())
    }
    pub async fn request(&self, method: &str, path: &str, body: Value) -> Result<Value, String> {
        self.core
            .request(method, path, body)
            .await
            .map_err(|e| e.message)
    }
    fn path(parts: &[&str], query: Option<(&str, &str)>) -> String {
        let mut url = reqwest::Url::parse("http://local/").unwrap();
        url.path_segments_mut().unwrap().extend(parts);
        if let Some((k, v)) = query {
            url.query_pairs_mut().append_pair(k, v);
        }
        format!(
            "{}{}",
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default()
        )
    }
    pub async fn pending_approvals(
        &self,
        session: &str,
    ) -> Result<Vec<crate::interactions::Approval>, String> {
        let v = self
            .request(
                "GET",
                &Self::path(&["api", "approval", "list"], Some(("session_id", session))),
                Value::Null,
            )
            .await?;
        serde_json::from_value(v["pending_approvals"].clone()).map_err(|_| "审批格式错误".into())
    }
    pub async fn questions(
        &self,
        session: &str,
    ) -> Result<Vec<crate::interactions::Question>, String> {
        let v = self
            .request(
                "GET",
                &Self::path(&["api", "questions"], Some(("session_id", session))),
                Value::Null,
            )
            .await?;
        serde_json::from_value(v["questions"].clone()).map_err(|_| "问题格式错误".into())
    }
    pub async fn act_approval(
        self,
        a: crate::interactions::Approval,
        approve: bool,
    ) -> Result<(), String> {
        self.request("POST",if approve{"/api/approval/approve"}else{"/api/approval/deny"},json!({"request_id":a.request_id,"session_id":a.root_session_id,"user_id":a.user_id,"scope":"exact"})).await?;
        Ok(())
    }
    pub async fn answer_question(&self, id: &str, body: Value) -> Result<(), String> {
        self.request(
            "POST",
            &Self::path(&["api", "questions", id, "answer"], None),
            body,
        )
        .await?;
        Ok(())
    }
    pub async fn chats(self) -> Result<Vec<Chat>, String> {
        serde_json::from_value(self.request("GET", "/api/chats", Value::Null).await?)
            .map_err(|_| "会话格式错误".into())
    }
    pub async fn history(self, id: String) -> Result<History, String> {
        serde_json::from_value(
            self.request(
                "GET",
                &Self::path(&["api", "chats", &id], None),
                Value::Null,
            )
            .await?,
        )
        .map_err(|_| "历史格式错误".into())
    }
    pub async fn bootstrap(self) -> Result<Connection, String> {
        let chats = self.clone().chats().await?;
        let model = self
            .request("GET", "/api/models/active", Value::Null)
            .await?;
        let config = self
            .request("GET", "/api/workspace/running-config", Value::Null)
            .await?;
        let preferences = self
            .request("GET", "/api/native/preferences", Value::Null)
            .await?;
        Ok(Connection {
            preferences,
            chats,
            model: model["active_llm"]["model"].as_str().map(str::to_owned),
            approval: config["approval_level"].as_str().map(str::to_owned),
            sandbox: config["sandbox_mode"].as_str().map(str::to_owned),
        })
    }
    pub async fn stop(self, session: Session, chat_id: Option<String>) -> Result<bool, String> {
        let id = match chat_id {
            Some(id) => id,
            None => self
                .clone()
                .chats()
                .await?
                .into_iter()
                .find(|c| c.session_id == session.id)
                .map(|c| c.id)
                .ok_or("会话尚未登记")?,
        };
        let v = self
            .request(
                "POST",
                &Self::path(&["api", "console", "chat", "stop"], Some(("chat_id", &id))),
                Value::Null,
            )
            .await?;
        Ok(v["stopped"] == true)
    }
    pub fn stream(
        self,
        session: Session,
        prompt: String,
        reconnect: bool,
        attachments: Vec<Value>,
    ) -> impl iced::futures::Stream<Item = NetworkEvent> {
        iced::stream::channel(32, async move |mut output| {
            let result: Result<(), String> = async {
                let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
                // Bound outstanding bytes independently of the core replay buffer.
                use std::sync::atomic::{AtomicUsize, Ordering};
                let queued = Arc::new(AtomicUsize::new(0));
                let outstanding = queued.clone();
                let emit = Arc::new(move |frame: Value| {
                    let size = frame.to_string().len();
                    if outstanding.fetch_add(size, Ordering::Relaxed) + size > 64_000_000 {
                        outstanding.fetch_sub(size, Ordering::Relaxed);
                        return Err(potato_core::Error::new(
                            429,
                            "UI output buffer full; reload history",
                        ));
                    }
                    tx.unbounded_send((frame, size))
                        .map_err(|_| potato_core::Error::new(499, "Window closed"))
                });
                let mut body = request_body(&session, &prompt, reconnect);
                if !reconnect {
                    body["input"][0]["content"]
                        .as_array_mut()
                        .unwrap()
                        .extend(attachments);
                }
                {
                    let _guard = self.executor.enter();
                    self.core
                        .start(uuid::Uuid::new_v4().to_string(), body, emit)
                        .map_err(|e| e.message)?;
                }
                output
                    .send(NetworkEvent::Accepted)
                    .await
                    .map_err(|_| "Window closed")?;
                while let Some((frame, size)) = rx.next().await {
                    queued.fetch_sub(size, Ordering::Relaxed);
                    let terminal = frame["object"] == "response"
                        && matches!(
                            frame["status"].as_str(),
                            Some("completed" | "failed" | "cancelled")
                        );
                    output
                        .send(NetworkEvent::Frames(vec![frame]))
                        .await
                        .map_err(|_| "Window closed")?;
                    if terminal {
                        return Ok(());
                    }
                }
                Err("输出结束但未收到任务终态，请刷新会话".into())
            }
            .await;
            let _ = output.send(NetworkEvent::End(result)).await;
        })
    }
}
fn default_user() -> String {
    "default".into()
}
fn default_channel() -> String {
    "console".into()
}

#[derive(Clone, Debug)]
pub struct Connection {
    pub preferences: Value,
    pub chats: Vec<Chat>,
    pub model: Option<String>,
    pub approval: Option<String>,
    pub sandbox: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub user: String,
    pub channel: String,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user: default_user(),
            channel: default_channel(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum NetworkEvent {
    Accepted,
    Frames(Vec<Value>),
    End(Result<(), String>),
}

pub fn request_body(session: &Session, prompt: &str, reconnect: bool) -> Value {
    let mut body = json!({"session_id":session.id, "user_id":session.user, "channel":session.channel, "stream":true});
    if reconnect {
        body["reconnect"] = json!(true);
    } else {
        body["input"] = json!([{"role":"user", "content":[{"type":"text", "text":prompt}]}]);
        // Inherit server permissions instead of overriding its approval/sandbox policy.
        body["request_context"] = json!({"last_user_message":prompt});
    }
    body
}

/// Keep unsupported blocks visible instead of silently dropping tool/media data.
pub fn display_message(value: &Value) -> crate::rich::ChatMessage {
    crate::rich::ChatMessage::from_value(value)
}

pub fn plain_message(value: &Value) -> (String, String) {
    let role = value["role"].as_str().unwrap_or("assistant").to_owned();
    let content = &value["content"];
    let body = if let Some(s) = content.as_str() {
        s.to_owned()
    } else if let Some(blocks) = content.as_array() {
        blocks
            .iter()
            .map(|block| {
                if block["type"] == "file" {
                    return format!(
                        "附件：{}",
                        block["file_name"].as_str().unwrap_or("未命名文件")
                    );
                }
                if block["type"] == "data" {
                    let data = &block["data"];
                    return format!(
                        "工具：{}\n{}",
                        data["name"].as_str().unwrap_or("执行中"),
                        data["output"]
                            .as_str()
                            .or_else(|| data["arguments"].as_str())
                            .unwrap_or("等待结果")
                    );
                }
                block["text"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        format!(
                            "[{}：此原型尚未支持展示]",
                            block["type"].as_str().unwrap_or("内容块")
                        )
                    })
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        "[此消息尚未支持展示]".into()
    };
    (role, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn embedded_bootstrap_and_settings_survive_reopen_without_a_server() {
        let dir = tempfile::tempdir().unwrap();
        {
            let backend = Backend::open(dir.path()).unwrap();
            let initial = backend.initial_connection().unwrap();
            assert!(initial.chats.is_empty());
            assert!(initial.model.is_none());
            backend.executor.block_on(async {
                backend
                    .request(
                        "PUT",
                        "/api/models/deepseek/config",
                        json!({"api_key":"synthetic-key","base_url":"https://example.org"}),
                    )
                    .await
                    .unwrap();
                backend
                    .request(
                        "PUT",
                        "/api/models/active",
                        json!({"provider_id":"deepseek","model":"deepseek-chat"}),
                    )
                    .await
                    .unwrap();
                assert!(backend
                    .pending_approvals("unrelated")
                    .await
                    .unwrap()
                    .is_empty());
            });
        }
        let backend = Backend::open(dir.path()).unwrap();
        assert_eq!(
            backend.initial_connection().unwrap().model.as_deref(),
            Some("deepseek-chat")
        );
    }

    #[test]
    fn embedded_stream_uses_model_service_and_persists_terminal_history() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend::open(dir.path()).unwrap();
        backend.executor.block_on(async {
            let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url=format!("http://{}",listener.local_addr().unwrap());
            let server=tokio::spawn(async move {
                let (mut socket,_)=listener.accept().await.unwrap();let mut request=vec![];let mut buf=[0;4096];
                loop {let n=socket.read(&mut buf).await.unwrap();assert!(n>0);request.extend_from_slice(&buf[..n]);
                    if let Some(end)=request.windows(4).position(|w|w==b"\r\n\r\n") {
                        let headers=String::from_utf8_lossy(&request[..end]).to_lowercase();
                        let len=headers.lines().find_map(|l|l.strip_prefix("content-length: ")).unwrap().parse::<usize>().unwrap();
                        if request.len()>=end+4+len{break;}
                    }
                }
                assert!(String::from_utf8_lossy(&request).contains("/chat/completions"));
                let body=format!("data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",json!({"choices":[{"delta":{"content":"你好，一体化客户端"},"finish_reason":null}]}),json!({"choices":[{"delta":{},"finish_reason":"stop"}]}));
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            });
            backend.request("PUT","/api/models/deepseek/config",json!({"api_key":"test","base_url":url})).await.unwrap();
            backend.request("PUT","/api/models/active",json!({"provider_id":"deepseek","model":"deepseek-chat"})).await.unwrap();
            let session=Session::default();let stream=backend.clone().stream(session.clone(),"你好".into(),false,vec![]);iced::futures::pin_mut!(stream);
            let mut turn=crate::stream::Turn::default();let mut accepted=false;let mut ended=false;
            tokio::time::timeout(std::time::Duration::from_secs(10),async {
                while let Some(event)=stream.next().await {match event {NetworkEvent::Accepted=>accepted=true,NetworkEvent::Frames(frames)=>{for f in frames{turn.apply(f);}},NetworkEvent::End(v)=>{v.unwrap();ended=true;}}}
            }).await.unwrap();
            assert!(accepted&&ended);assert!(turn.messages.iter().any(|v|v.to_string().contains("一体化客户端")));
            let chats=backend.clone().chats().await.unwrap();assert_eq!(chats.len(),1);assert_eq!(chats[0].session_id,session.id);
            let history=backend.clone().history(chats[0].id.clone()).await.unwrap();assert_eq!(history.status,"idle");assert_eq!(history.messages.iter().filter(|m|m["role"]=="user").count(),1);
            server.await.unwrap();
        });
    }
    #[test]
    fn preserves_text_and_marks_unsupported_blocks() {
        let (_, body) = plain_message(&json!({"role":"assistant", "content":[
            {"type":"text", "text":"你好"}, {"type":"tool_call", "name":"shell"}
        ]}));
        assert!(body.contains("你好"));
        assert!(body.contains("tool_call"));
    }

    #[test]
    fn send_and_reconnect_preserve_identity_and_server_permissions() {
        let session = Session {
            id: "existing-session".into(),
            user: "existing-user".into(),
            channel: "console".into(),
        };
        let body = request_body(&session, "你好", false);
        assert_eq!(body["session_id"], "existing-session");
        assert_eq!(body["user_id"], "existing-user");
        assert_eq!(body["input"][0]["content"][0]["text"], "你好");
        assert!(body["request_context"]["approval_level"].is_null());
        assert!(body["request_context"]["sandbox_mode"].is_null());
        let reconnect = request_body(&session, "", true);
        assert_eq!(reconnect["reconnect"], true);
        assert!(reconnect["input"].is_null());
    }
}
