use crate::Runtime;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct Fixture {
    relay: String,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    rows: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn start(post_status: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let relay = format!("http://{}/", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let rows = Arc::new(Mutex::new(Vec::new()));
        let (seen, memories) = (requests.clone(), rows.clone());
        let task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut bytes = Vec::new();
                let mut chunk = [0; 4096];
                let (headers, body) = loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_string();
                        let len = headers
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|s| s.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + len {
                            break (
                                headers,
                                if len == 0 {
                                    Value::Null
                                } else {
                                    serde_json::from_slice(&bytes[end + 4..end + 4 + len]).unwrap()
                                },
                            );
                        }
                    }
                };
                assert!(headers
                    .to_lowercase()
                    .contains("authorization: bearer fixture-token"));
                let route = headers
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ");
                seen.lock().unwrap().push((route.clone(), body.clone()));
                let (status, response) = match route.as_str() {
                    "POST /v1/recall/memory" => {
                        if body["forget"] == true {
                            (post_status, json!({"error":"conflict"}))
                        } else {
                            let memory = json!({"id":body["id"],"text":body["text"],"revision":"a".repeat(64),"updated":"2026-09-13T00:00:00Z","sources":[],"forgotten":false});
                            memories.lock().unwrap().push(memory.clone());
                            (post_status, memory)
                        }
                    }
                    "GET /v1/recall/status" => (
                        200,
                        json!({"version":1,"scope":"cloud","entries":0,"memories":*memories.lock().unwrap()}),
                    ),
                    "POST /v1/cloud/account/logout" => (200, json!({"ok":true})),
                    _ => panic!("Unexpected route {route}"),
                };
                let text = response.to_string();
                socket.write_all(format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len()).as_bytes()).await.unwrap();
            }
        });
        Self {
            relay,
            requests,
            rows,
            task,
        }
    }
    fn login(&self, core: &Runtime) -> String {
        let db = core.db().unwrap();
        let token = db.seal("fixture-token").unwrap();
        db.put("cloud_config", &json!({"relay":self.relay,"email":"memory@example.test","expires":chrono::Utc::now().timestamp_millis()+86400000,"session_token":token})).unwrap();
        format!(
            "cloud_memory_cache:{:x}",
            Sha256::digest(format!("{}\nmemory@example.test", self.relay))
        )
    }
}

#[tokio::test]
async fn remember_saves_and_populates_account_cache() {
    let fixture = Fixture::start(200).await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    let key = fixture.login(&core);
    let result = core.cloud_remember("喜欢无糖咖啡").await.unwrap();
    assert_eq!(result["saved"], true);
    assert_eq!(
        uuid::Uuid::parse_str(result["id"].as_str().unwrap())
            .unwrap()
            .get_version_num(),
        4
    );
    assert_eq!(
        fixture.requests.lock().unwrap()[0],
        (
            "POST /v1/recall/memory".into(),
            json!({"id":result["id"],"text":"喜欢无糖咖啡","base":null,"forget":false})
        )
    );
    assert!(core
        .cloud_memory_guidance()
        .unwrap()
        .unwrap()
        .contains("喜欢无糖咖啡"));
    let cache = core.db().unwrap().get(&key, Value::Null).unwrap();
    assert_eq!(cache["memories"][0]["id"], result["id"]);
    assert!(cache["fetched_at"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn remember_500_is_confirmed_using_original_id() {
    let fixture = Fixture::start(500).await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    fixture.login(&core);
    let result = core.cloud_remember("I prefer tea").await.unwrap();
    assert_eq!(result["saved"], true);
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests[0].1["id"], result["id"]);
    assert_eq!(
        requests
            .iter()
            .filter(|(r, _)| r == "POST /v1/recall/memory")
            .count(),
        1
    );
    assert_eq!(requests[1].0, "GET /v1/recall/status");
    assert_eq!(requests[2].0, "GET /v1/recall/status");
}

#[tokio::test]
async fn forget_409_uses_revision_and_empty_text_then_refreshes() {
    let fixture = Fixture::start(409).await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    let key = fixture.login(&core);
    let id = uuid::Uuid::new_v4().to_string();
    let old = json!({"id":id,"text":"old","revision":"a".repeat(64),"updated":"2026-09-12T00:00:00Z","sources":[],"forgotten":false});
    let mut new = old.clone();
    new["text"] = json!("new");
    new["revision"] = json!("b".repeat(64));
    fixture.rows.lock().unwrap().push(new.clone());
    core.db()
        .unwrap()
        .put(
            &key,
            &json!({"memories":[old],"fetched_at":chrono::Utc::now().timestamp_millis()}),
        )
        .unwrap();
    let error = core.cloud_forget(&id).await.unwrap_err();
    assert_eq!(error.status, 409);
    assert_eq!(
        fixture.requests.lock().unwrap()[0].1,
        json!({"id":id,"text":"","base":"a".repeat(64),"forget":true})
    );
    assert_eq!(
        core.db().unwrap().get(&key, Value::Null).unwrap()["memories"][0],
        new
    );
    assert_eq!(
        fixture.requests.lock().unwrap()[1].0,
        "GET /v1/recall/status"
    );
}

#[tokio::test]
async fn signed_out_hides_cloud_tools_and_guidance() {
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    core.db()
        .unwrap()
        .put("cloud_config", &Value::Null)
        .unwrap();
    assert!(core.definitions(true).unwrap().iter().all(|d| !matches!(
        d["function"]["name"].as_str(),
        Some("remember" | "forget_memory")
    )));
    assert_eq!(core.cloud_memory_guidance().unwrap(), None);
}

#[tokio::test]
async fn logout_clears_account_memory_cache() {
    let fixture = Fixture::start(200).await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    let key = fixture.login(&core);
    core.refresh_cloud_memory(true).await.unwrap();
    assert!(core
        .db()
        .unwrap()
        .get(&key, Value::Null)
        .unwrap()
        .is_object());
    core.logout_cloud().await.unwrap();
    assert_eq!(
        core.db()
            .unwrap()
            .get(&key, json!({"missing":true}))
            .unwrap(),
        Value::Null
    );
}
