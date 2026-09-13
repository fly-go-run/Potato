use crate::{Runtime, lock};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct Fixture {
    relay: String,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    status: Arc<std::sync::atomic::AtomicU16>,
    catalog_wait: Arc<std::sync::atomic::AtomicBool>,
    catalog_started: Arc<tokio::sync::Notify>,
    catalog_release: Arc<tokio::sync::Notify>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let relay = format!("http://{}/", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let status = Arc::new(std::sync::atomic::AtomicU16::new(200));
        let catalog_wait = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let catalog_started = Arc::new(tokio::sync::Notify::new());
        let catalog_release = Arc::new(tokio::sync::Notify::new());
        let (url, rows, http_status, wait, started, release) = (
            relay.clone(),
            requests.clone(),
            status.clone(),
            catalog_wait.clone(),
            catalog_started.clone(),
            catalog_release.clone(),
        );
        let task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let (url, rows, http_status, wait, started, release) = (
                    url.clone(),
                    rows.clone(),
                    http_status.clone(),
                    wait.clone(),
                    started.clone(),
                    release.clone(),
                );
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut chunk = [0; 4096];
                    let (headers, body) = loop {
                        let n = socket.read(&mut chunk).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&chunk[..n]);
                        if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&bytes[..end]).to_string();
                            let len = headers
                                .lines()
                                .find_map(|l| {
                                    l.to_lowercase()
                                        .strip_prefix("content-length: ")
                                        .and_then(|v| v.parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + len {
                                break (
                                    headers,
                                    if len > 0 {
                                        serde_json::from_slice::<Value>(
                                            &bytes[end + 4..end + 4 + len],
                                        )
                                        .unwrap()
                                    } else {
                                        Value::Null
                                    },
                                );
                            }
                        }
                    };
                    let path = headers
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    rows.lock().unwrap().push((path.to_owned(), body.clone()));
                    let response = match path {
                        "/v1/cloud/auth/start" => {
                            assert_eq!(body["role"], "cloud");
                            let id = uuid::Uuid::new_v4().to_string();
                            json!({"id":id,"verification_url":format!("{url}v1/cloud/auth/authorize?id={id}"),"code":"ABC12345","expires":chrono::Utc::now().timestamp_millis()+300000})
                        }
                        "/v1/cloud/auth/poll" => {
                            json!({"status":"authorized","owner":"a".repeat(64),"email":"fixture@example.test","expires":chrono::Utc::now().timestamp_millis()+86400000})
                        }
                        "/v1/models" => {
                            assert!(
                                headers.to_lowercase().contains(&format!(
                                    "authorization: bearer {}.",
                                    "a".repeat(64)
                                ))
                            );
                            if wait.load(std::sync::atomic::Ordering::SeqCst) {
                                started.notify_one();
                                release.notified().await;
                            }
                            json!({"default_model":"fixture/one","data":[{"id":"fixture/one","name":"One","reasoning_effort_options":["low","high"]}]})
                        }
                        "/v1/cloud/account/logout" => json!({"ok":true}),
                        _ => panic!("Unexpected route {path}"),
                    };
                    let status = if matches!(path, "/v1/models" | "/v1/cloud/account/logout") {
                        http_status.load(std::sync::atomic::Ordering::SeqCst)
                    } else {
                        200
                    };
                    let text = response.to_string();
                    let _ = socket.write_all(format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len()).as_bytes()).await;
                });
            }
        });
        Self {
            relay,
            requests,
            status,
            catalog_wait,
            catalog_started,
            catalog_release,
            task,
        }
    }
    async fn login(&self, core: &Arc<Runtime>) {
        let login = core
            .begin_cloud_login(json!({"relay":self.relay}))
            .await
            .unwrap();
        assert_eq!(login["login"]["code"], "ABC12345");
        assert!(login["login"]["client_token"].is_null());
        let config = core.poll_cloud_login().await.unwrap();
        assert_eq!(config["signed_in"], true);
        assert_eq!(config["model_count"], 1);
    }
}

#[tokio::test]
async fn cloud_email_login_seals_credentials_and_creates_usable_managed_provider() {
    let fixture = Fixture::start().await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    fixture.login(&core).await;
    assert_eq!(
        core.request("GET", "/api/models/active", Value::Null)
            .await
            .unwrap()["active_llm"],
        json!({"provider_id":"potato-cloud","model":"fixture/one"})
    );
    let connection = core.connection().unwrap();
    assert_eq!(connection.url, format!("{}v1/desktop", fixture.relay));
    assert!(!connection.responses);
    let config = core.db().unwrap().get("cloud_config", Value::Null).unwrap();
    assert_ne!(config["session_token"], connection.key);
    assert_eq!(
        core.db()
            .unwrap()
            .unseal(config["session_token"].as_str().unwrap())
            .unwrap(),
        connection.key
    );
    let providers = core
        .request("GET", "/api/models", Value::Null)
        .await
        .unwrap();
    assert!(!providers.to_string().contains(&connection.key));
    assert_eq!(core.remote_settings().unwrap()["enabled"], false);
    assert!(core.remote_settings().unwrap()["auth_mode"].is_null());
    core.request(
        "PUT",
        "/api/models/potato-cloud/models/fixture%2Fone/config",
        json!({"reasoning_effort":"high"}),
    )
    .await
    .unwrap();
    assert_eq!(
        core.connection().unwrap().options["reasoning_effort"],
        "high"
    );
    for (path, body) in [
        (
            "/api/models/potato-cloud/config",
            json!({"api_key":"stolen"}),
        ),
        (
            "/api/models/potato-cloud/models/fixture%2Fone/config",
            json!({"reasoning_effort_options":["ultra"]}),
        ),
    ] {
        assert_eq!(
            core.request("PUT", path, body).await.unwrap_err().status,
            403
        );
    }
    assert!(
        core.request(
            "PUT",
            "/api/models/active",
            json!({"provider_id":"potato-cloud","model":"not-enabled"})
        )
        .await
        .is_err()
    );
    core.request(
        "PUT",
        "/api/models/deepseek/config",
        json!({"api_key":"fixture-custom-key"}),
    )
    .await
    .unwrap();
    assert!(
        !core
            .db()
            .unwrap()
            .get("providers", Value::Null)
            .unwrap()
            .to_string()
            .contains("potato-cloud")
    );
    // Keep an explicit cloud selection after adding a usable local provider.
    core.request("PUT", "/api/models/active", json!({"provider_id":"potato-cloud","model":"fixture/one"})).await.unwrap();
    drop(core);
    let core = Runtime::open(dir.path()).unwrap();
    assert_eq!(
        core.connection().unwrap().options["reasoning_effort"],
        "high"
    );
    core.logout_cloud().await.unwrap();
    assert!(core.cloud_provider().unwrap().is_none());
    assert!(core.cloud_connection("fixture/one").is_err());
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|(p, _)| !p.starts_with("/v1/remote/"))
    );
}

#[tokio::test]
async fn cloud_logout_wins_over_inflight_catalog_and_expired_sessions_can_logout() {
    let fixture = Fixture::start().await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    fixture.login(&core).await;
    fixture
        .catalog_wait
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let other = core.clone();
    let refresh = tokio::spawn(async move { other.refresh_cloud_models().await });
    fixture.catalog_started.notified().await;
    fixture
        .status
        .store(401, std::sync::atomic::Ordering::SeqCst);
    core.logout_cloud().await.unwrap();
    fixture.catalog_release.notify_one();
    assert_eq!(refresh.await.unwrap().unwrap_err().status, 409);
    assert_eq!(core.cloud_settings().unwrap()["signed_in"], false);
    assert!(core.cloud_provider().unwrap().is_none());
}

#[tokio::test]
async fn cloud_revocation_clears_catalog_and_does_not_replace_custom_model() {
    let fixture = Fixture::start().await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    core.request("PUT", "/api/models/deepseek/config", json!({"api_key":"local-fixture-key"})).await.unwrap();
    core.request(
        "PUT",
        "/api/models/active",
        json!({"provider_id":"deepseek","model":"deepseek-chat"}),
    )
    .await
    .unwrap();
    fixture.login(&core).await;
    assert_eq!(
        core.request("GET", "/api/models/active", Value::Null)
            .await
            .unwrap()["active_llm"]["provider_id"],
        "deepseek"
    );
    fixture
        .status
        .store(403, std::sync::atomic::Ordering::SeqCst);
    let state = core.refresh_cloud_models().await.unwrap();
    assert_eq!(state["model_count"], 0);
    assert!(state["error"].is_string());
    assert!(core.cloud_connection("fixture/one").is_err());
    assert!(
        core.begin_cloud_login(json!({"relay":"https://attacker.invalid/"}))
            .await
            .is_err()
    );
    core.cancel_cloud_login().unwrap();
    assert!(lock(&core.cloud_generation).is_ok());
}

#[tokio::test]
async fn cloud_is_fallback_for_empty_local_config_but_respects_configured_local_and_manual_cloud() {
    let fixture = Fixture::start().await;
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    // A saved selection without credentials must not block the cloud default.
    core.db().unwrap().put("active", &json!({"provider_id":"deepseek","model":"deepseek-chat"})).unwrap();
    fixture.login(&core).await;
    assert_eq!(core.connection().unwrap().model, "fixture/one");
    core.request("PUT", "/api/models/deepseek/config", json!({"api_key":"local-fixture-key"})).await.unwrap();
    // Newly available local credentials also win over a previous automatic cloud fallback.
    assert_eq!(core.connection().unwrap().key, "local-fixture-key");
    // Configured provider with no selection takes precedence during cloud login.
    core.db().unwrap().put("active", &Value::Null).unwrap();
    core.refresh_cloud_models().await.unwrap();
    assert_eq!(core.connection().unwrap().key, "local-fixture-key");
    core.request("PUT", "/api/models/active", json!({"provider_id":"potato-cloud","model":"fixture/one"})).await.unwrap();
    core.refresh_cloud_models().await.unwrap();
    assert_eq!(core.connection().unwrap().model, "fixture/one");
}
