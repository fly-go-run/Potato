use potato_core::{protocol::SseDecoder, Runtime};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};

async fn fixture(responses: Vec<(String, String)>) -> (String, mpsc::UnboundedReceiver<String>) {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        for (content_type, body) in responses {
            let (mut socket, _) = server.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let size = socket.read(&mut chunk).await.unwrap();
                if size == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..size]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("content-length: ")
                                .and_then(|n| n.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            tx.send(String::from_utf8_lossy(&request).to_string()).ok();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
            // Exercise arbitrary TCP chunking, including inside CJK characters.
            for chunk in body.as_bytes().chunks(7) {
                if socket.write_all(chunk).await.is_err() {
                    break;
                }
            }
        }
    });
    (url, rx)
}

fn sse(value: Value) -> String {
    format!("data: {value}\n\n")
}
fn answer(text: &str) -> String {
    sse(json!({"choices":[{"delta":{"content":text},"finish_reason":null}]}))
        + &sse(json!({"choices":[{"delta":{},"finish_reason":"stop"}]}))
        + "data: [DONE]\n\n"
}
async fn configure(runtime: &Runtime, url: &str, protocol: &str) {
    runtime
        .request(
            "PUT",
            "/api/models/deepseek/config",
            json!({"api_key":"secret-test-key","base_url":url,"chat_model":protocol}),
        )
        .await
        .unwrap();
    runtime
        .request(
            "PUT",
            "/api/models/active",
            json!({"provider_id":"deepseek","model":"test-model"}),
        )
        .await
        .unwrap();
}
fn start(runtime: &Arc<Runtime>, session: &str, id: &str) -> mpsc::UnboundedReceiver<Value> {
    let (tx, rx) = mpsc::unbounded_channel();
    runtime.start(id.into(),json!({"session_id":session,"input":[{"role":"user","content":[{"type":"text","text":"你好"}]}]}),Arc::new(move |v|{tx.send(v).unwrap();Ok(())})).unwrap();
    rx
}

async fn approve_next(runtime: &Runtime, session: &str) {
    let approval = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let pending = runtime
                .request(
                    "GET",
                    &format!("/api/console/push-messages?session_id={session}"),
                    Value::Null,
                )
                .await
                .unwrap();
            if !pending["pending_approvals"][0].is_null() {
                break pending["pending_approvals"][0].clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    runtime
        .request(
            "POST",
            "/api/approval/approve",
            json!({"request_id":approval["request_id"],"session_id":session,"user_id":"default"}),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn attachment_only_text_is_decoded_for_the_model_and_saved_for_preview() {
    use base64::Engine;
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let (url, mut requests) =
        fixture(vec![("text/event-stream".into(), answer("Read document"))]).await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let file_url = format!(
        "data:text/plain;base64,{}",
        base64::engine::general_purpose::STANDARD.encode("家庭文档")
    );
    let (tx, mut rx) = mpsc::unbounded_channel();
    runtime.start("run".into(),json!({"session_id":"attachments","input":[{"role":"user","content":[{"type":"text","text":""},{"type":"file","filename":"notes.md","file_url":file_url}]}]}),Arc::new(move|v|{let _=tx.send(v);Ok(())})).unwrap();
    assert_eq!(finish(&mut rx).await.last().unwrap()["status"], "completed");
    let request = requests.recv().await.unwrap();
    assert!(request.contains("家庭文档"));
    assert!(!request.contains("data:text/plain"));
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    let history = runtime
        .request(
            "GET",
            &format!("/api/chats/{}", chats[0]["id"].as_str().unwrap()),
            Value::Null,
        )
        .await
        .unwrap();
    assert!(history.to_string().contains(&file_url));
}

#[tokio::test]
async fn long_history_is_compacted_once_without_destroying_display_history() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let messages:Vec<_>=(0..8).map(|i|json!({"id":format!("old-{i}"),"type":"message","role":"user","status":"completed","content":[{"type":"text","text":format!("{i}{}","x".repeat(20_000))}]})).collect();
    runtime.request("POST","/api/native/import-history",json!({"format":"potato-native-history-v1","chats":[{"spec":{"id":"long","session_id":"long","name":"Long history"},"messages":messages}]})).await.unwrap();
    let mut replies: Vec<_> = (0..4)
        .map(|_| {
            (
                "text/event-stream".into(),
                answer("Saved goals and decisions"),
            )
        })
        .collect();
    replies.push(("text/event-stream".into(), answer("First answer")));
    replies.push(("text/event-stream".into(), answer("Second answer")));
    let (url, mut requests) = fixture(replies).await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    assert_eq!(
        finish(&mut start(&runtime, "long", "run1"))
            .await
            .last()
            .unwrap()["status"],
        "completed"
    );
    for _ in 0..4 {
        assert!(requests
            .recv()
            .await
            .unwrap()
            .contains("Summarize conversation history"));
    }
    let request = requests.recv().await.unwrap();
    assert!(request.contains("Saved goals and decisions"));
    assert!(request.len() < 40_000);
    assert_eq!(
        finish(&mut start(&runtime, "long", "run2"))
            .await
            .last()
            .unwrap()["status"],
        "completed"
    );
    let next = requests.recv().await.unwrap();
    assert!(!next.contains("Summarize conversation history"));
    assert!(next.contains("First answer"));
    let history = runtime
        .request("GET", "/api/chats/long", Value::Null)
        .await
        .unwrap();
    assert_eq!(
        history["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .len(),
        20_001
    );
}

#[tokio::test]
async fn saved_model_limits_and_reasoning_are_sent_in_both_protocols() {
    for responses in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        let body = if responses {
            sse(json!({"type":"response.output_text.delta","delta":"ok"}))
                + &sse(json!({"type":"response.completed"}))
        } else {
            answer("ok")
        };
        let (url, mut requests) = fixture(vec![("text/event-stream".into(), body)]).await;
        configure(
            &runtime,
            &url,
            if responses {
                "OpenAIResponseModel"
            } else {
                "OpenAIChatModel"
            },
        )
        .await;
        runtime
            .request(
                "POST",
                "/api/models/deepseek/models",
                json!({"id":"test-model","name":"Test"}),
            )
            .await
            .unwrap();
        runtime
            .request(
                "PUT",
                "/api/models/deepseek/models/test-model/config",
                json!({"max_tokens":1024,"reasoning_effort":"high"}),
            )
            .await
            .unwrap();
        assert_eq!(
            finish(&mut start(&runtime, "settings", "run"))
                .await
                .last()
                .unwrap()["status"],
            "completed"
        );
        let request = requests.recv().await.unwrap();
        let payload: Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(
            payload[if responses {
                "max_output_tokens"
            } else {
                "max_tokens"
            }],
            1024
        );
        if responses {
            assert_eq!(payload["reasoning"]["effort"], "high");
        } else {
            assert_eq!(payload["reasoning_effort"], "high");
        }
    }
}

#[tokio::test]
async fn hosted_search_preserves_citations_and_uses_separate_responses_request() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let call = sse(
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"search-1","function":{"name":"web_search","arguments":"{\"query\":\"latest news\"}"}}]},"finish_reason":"tool_calls"}]}),
    ) + "data: [DONE]\n\n";
    let search = sse(
        json!({"type":"response.completed","response":{"output":[{"type":"message","content":[{"type":"output_text","text":"Verified fact","annotations":[{"type":"url_citation","url":"https://example.org/source","title":"Source"}]}]}]}}),
    );
    let (url, mut requests) = fixture(vec![
        ("text/event-stream".into(), call),
        ("text/event-stream".into(), search),
        ("text/event-stream".into(), answer("Cited answer")),
    ])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    runtime.request("PUT","/api/workspace/web-search-backend",json!({"web_search_backend":"hosted","web_search_provider_id":"deepseek","web_search_model":"search-model"})).await.unwrap();
    let mut stream = start(&runtime, "search", "run");
    approve_next(&runtime, "search").await;
    assert_eq!(
        finish(&mut stream).await.last().unwrap()["status"],
        "completed"
    );
    requests.recv().await.unwrap();
    let search = requests.recv().await.unwrap();
    assert!(search.starts_with("POST /responses"));
    assert!(search.contains("search-model"));
    let followup = requests.recv().await.unwrap();
    assert!(followup.contains("https://example.org/source"));
    assert!(followup.contains("Untrusted web content"));
}

#[tokio::test]
async fn backup_contains_portable_documents_without_credentials() {
    use base64::Engine;
    use std::io::Read;
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    configure(&runtime, "http://127.0.0.1:9", "OpenAIChatModel").await;
    runtime
        .request(
            "PUT",
            "/api/workspace/memory/family.md",
            json!({"content":"Saved preference"}),
        )
        .await
        .unwrap();
    let exported = runtime
        .request("GET", "/api/workspace/download", Value::Null)
        .await
        .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(exported["native_binary"].as_str().unwrap())
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(archive.by_name("master.key").is_err());
    assert!(archive.by_name("providers.json").is_err());
    let mut memory = String::new();
    archive
        .by_name("memory/family.md")
        .unwrap()
        .read_to_string(&mut memory)
        .unwrap();
    assert_eq!(memory, "Saved preference");
    for index in 0..archive.len() {
        let mut text = String::new();
        archive
            .by_index(index)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert!(!text.contains("secret-test-key"));
    }
}

#[tokio::test]
async fn scheduled_agent_runs_through_native_model_and_saves_history() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let (url, mut requests) = fixture(vec![(
        "text/event-stream".into(),
        answer("Scheduled answer"),
    )])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let job=runtime.request("POST","/api/cron/jobs",json!({"name":"Agent task","enabled":true,"schedule":{"type":"once","run_at":(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339()},"task_type":"agent","request":{"input":[{"role":"user","content":[{"type":"text","text":"Scheduled prompt"}]}]},"dispatch":{"type":"channel","channel":"console","target":{"session_id":"scheduled","user_id":"default"}}})).await.unwrap();
    let id = job["id"].as_str().unwrap();
    let scheduler = tokio::spawn(runtime.clone().serve_scheduler());
    runtime
        .request("POST", &format!("/api/cron/jobs/{id}/run"), Value::Null)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state = runtime
                .request("GET", &format!("/api/cron/jobs/{id}/state"), Value::Null)
                .await
                .unwrap();
            if state["last_status"] == "success" {
                break;
            }
            assert_ne!(state["last_status"], "error", "{state}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(requests.recv().await.unwrap().contains("Scheduled prompt"));
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    let history = runtime
        .request(
            "GET",
            &format!("/api/chats/{}", chats[0]["id"].as_str().unwrap()),
            Value::Null,
        )
        .await
        .unwrap();
    assert!(history.to_string().contains("Scheduled answer"));
    scheduler.abort();
}
async fn finish(rx: &mut mpsc::UnboundedReceiver<Value>) -> Vec<Value> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut frames = Vec::new();
        while let Some(frame) = rx.recv().await {
            let terminal = frame["object"] == "response"
                && matches!(
                    frame["status"].as_str(),
                    Some("completed" | "failed" | "cancelled")
                );
            frames.push(frame);
            if terminal {
                return frames;
            }
        }
        panic!("stream closed without terminal response");
    })
    .await
    .unwrap()
}

#[test]
fn sse_survives_every_byte_boundary_and_multiline_crlf() {
    let mut decoder = SseDecoder::default();
    let mut result = Vec::new();
    for byte in ": ping\r\ndata: 你好\r\ndata: 世界\r\n\r\n".as_bytes() {
        result.extend(decoder.push(&[*byte]).unwrap());
    }
    assert_eq!(result, vec!["你好\n世界"]);
}

#[tokio::test]
async fn streams_and_persists_history_without_python() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let (url, mut requests) = fixture(vec![
        ("text/event-stream".into(), answer("你好，家人")),
        ("text/event-stream".into(), answer("第二轮")),
    ])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let frames = finish(&mut start(&runtime, "session", "run-1")).await;
    assert_eq!(frames.last().unwrap()["status"], "completed");
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    let id = chats[0]["id"].as_str().unwrap();
    let history = runtime
        .request("GET", &format!("/api/chats/{id}"), Value::Null)
        .await
        .unwrap();
    assert_eq!(history["messages"][1]["content"][0]["text"], "你好，家人");
    finish(&mut start(&runtime, "session", "run-2")).await;
    assert!(requests
        .recv()
        .await
        .unwrap()
        .starts_with("POST /chat/completions"));
    let second = requests.recv().await.unwrap();
    assert!(second.contains("你好，家人"));
    drop(runtime);
    let reopened = Runtime::open(tmp.path()).unwrap();
    let history = reopened
        .request("GET", &format!("/api/chats/{id}"), Value::Null)
        .await
        .unwrap();
    assert_eq!(history["messages"].as_array().unwrap().len(), 4);
    assert_eq!(history["status"], "idle");
}

#[tokio::test]
async fn workspace_documents_survive_restart_and_feed_both_model_protocols() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let original = runtime
        .request("GET", "/api/workspace/files/AGENTS.md", Value::Null)
        .await
        .unwrap();
    assert!(original["content"].as_str().unwrap().contains("可用能力"));
    let custom="Answer in short sentences.\n<!-- heartbeat:start -->\nDO_NOT_SEND_SCHEDULER_INSTRUCTIONS\n<!-- heartbeat:end -->";
    runtime
        .request(
            "PUT",
            "/api/workspace/files/AGENTS.md",
            json!({"content":custom}),
        )
        .await
        .unwrap();
    runtime
        .request(
            "PUT",
            "/api/workspace/memory/digest/wiki/family.md",
            json!({"content":"Family preference"}),
        )
        .await
        .unwrap();
    runtime
        .request("PUT", "/api/workspace/language", json!({"language":"en"}))
        .await
        .unwrap();
    drop(runtime);
    let runtime = Runtime::open(tmp.path()).unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/workspace/files/AGENTS.md", Value::Null)
            .await
            .unwrap()["content"],
        custom
    );
    assert_eq!(
        runtime
            .request(
                "GET",
                "/api/workspace/memory/digest/wiki/family.md",
                Value::Null
            )
            .await
            .unwrap()["content"],
        "Family preference"
    );
    let responses = sse(json!({"type":"response.output_text.delta","delta":"ok"}))
        + &sse(json!({"type":"response.completed"}));
    let (url, mut requests) = fixture(vec![
        ("text/event-stream".into(), answer("ok")),
        ("text/event-stream".into(), responses),
    ])
    .await;
    for protocol in ["OpenAIChatModel", "OpenAIResponseModel"] {
        configure(&runtime, &url, protocol).await;
        let frames = finish(&mut start(&runtime, protocol, protocol)).await;
        assert_eq!(frames.last().unwrap()["status"], "completed");
        let request = requests.recv().await.unwrap();
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        let messages = if protocol == "OpenAIChatModel" {
            &body["messages"]
        } else {
            &body["input"]
        };
        assert_eq!(messages[0]["role"], "system");
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("Answer in short sentences."));
        assert!(!request.contains("DO_NOT_SEND_SCHEDULER_INSTRUCTIONS"));
        assert!(!request.contains("Family preference")); // Memory files aren't silently all injected.
    }
}

#[tokio::test]
async fn document_api_rejects_invalid_paths_and_preserves_saved_content() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    for path in [
        "/api/workspace/files/a%2Fb.md",
        "/api/workspace/memory/a%2F..%2Fx.md",
        "/api/workspace/files/C:%5Cx.md",
    ] {
        assert_eq!(
            runtime
                .request("PUT", path, json!({"content":"bad"}))
                .await
                .unwrap_err()
                .status,
            400
        );
    }
    assert_eq!(
        runtime
            .request(
                "PUT",
                "/api/workspace/system-prompt-files",
                json!(["missing.md"])
            )
            .await
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        runtime
            .request(
                "PUT",
                "/api/workspace/files/AGENTS.md",
                json!({"content":"a".repeat(128_001)})
            )
            .await
            .unwrap_err()
            .status,
        413
    );
    assert!(runtime
        .request("GET", "/api/workspace/files/AGENTS.md", Value::Null)
        .await
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .contains("可用能力"));
    assert_eq!(
        runtime
            .request("PUT", "/api/workspace/system-prompt-files", json!([]))
            .await
            .unwrap(),
        json!([])
    );
}

#[tokio::test]
async fn existing_voice_switch_keeps_credentials_and_persists_disabled_state() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    runtime
        .request(
            "PUT",
            "/api/native/doubao-settings",
            json!({"api_key":"dummy-key","enabled":true}),
        )
        .await
        .unwrap();
    for kind in ["disabled", "doubao_asr", "disabled"] {
        runtime
            .request(
                "PUT",
                "/api/workspace/transcription-provider-type",
                json!({"transcription_provider_type":kind}),
            )
            .await
            .unwrap();
        let status = runtime
            .request("GET", "/api/workspace/speech-status", Value::Null)
            .await
            .unwrap();
        assert_eq!(status["transcription_provider_type"], kind);
        assert_eq!(status["doubao_credentials_configured"], true);
        assert_eq!(status["ready"], kind == "doubao_asr");
    }
    drop(runtime);
    let runtime = Runtime::open(tmp.path()).unwrap();
    assert_eq!(
        runtime
            .request(
                "GET",
                "/api/workspace/transcription-provider-type",
                Value::Null
            )
            .await
            .unwrap()["transcription_provider_type"],
        "disabled"
    );
    assert_eq!(
        runtime
            .request("GET", "/api/native/doubao-settings", Value::Null)
            .await
            .unwrap()["api_key"],
        "********"
    );
    assert_eq!(
        runtime
            .transcribe("x.wav".into(), "audio/wav".into(), vec![1])
            .await
            .unwrap_err()
            .status,
        400
    );
}

#[tokio::test]
async fn project_creation_git_preview_and_session_selection_work_without_python() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let project = runtime
        .request(
            "POST",
            "/api/workspace/coding-project/create",
            json!({"name":"家庭项目"}),
        )
        .await
        .unwrap();
    let path = std::path::PathBuf::from(project["path"].as_str().unwrap());
    assert!(path.join(".git").is_dir());
    assert_eq!(project["name"], "家庭项目");
    std::fs::write(path.join("hello world.txt"), "第一行\n第二行\n").unwrap();
    let status = runtime
        .request("GET", "/api/workspace/git/status", Value::Null)
        .await
        .unwrap();
    assert!(status["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["path"] == "hello world.txt" && v["staged"] == false));
    let diff = runtime
        .request(
            "GET",
            "/api/workspace/git/diff?path=hello%20world.txt&untracked=true",
            Value::Null,
        )
        .await
        .unwrap();
    assert!(diff["diff"].as_str().unwrap().contains("+第一行"));
    assert_eq!(
        runtime
            .request(
                "GET",
                "/api/workspace/git/diff?path=../master.key&untracked=true",
                Value::Null
            )
            .await
            .unwrap_err()
            .status,
        400
    );
    assert_eq!(
        runtime
            .request(
                "PUT",
                "/api/workspace/coding-project",
                json!({"path":tmp.path()})
            )
            .await
            .unwrap_err()
            .status,
        403
    );
    assert_eq!(
        runtime
            .request(
                "POST",
                "/api/workspace/coding-project/create",
                json!({"name":"家庭项目"})
            )
            .await
            .unwrap_err()
            .status,
        409
    );
    assert_eq!(
        runtime
            .request("GET", "/api/workspace/coding-project/list", Value::Null)
            .await
            .unwrap()[0]["name"],
        "家庭项目"
    );
    drop(runtime);
    let runtime = Runtime::open(tmp.path()).unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/workspace/coding-project", Value::Null)
            .await
            .unwrap()["path"],
        project["path"]
    );
    let other = tempfile::tempdir().unwrap();
    let (url, mut requests) = fixture(vec![("text/event-stream".into(), answer("ok"))]).await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    runtime.start("project-chat".into(),json!({"session_id":"project-chat","input":[{"role":"user","content":[{"type":"text","text":"hello"}]}],"request_context":{"potato.coding_project_dir":other.path()}}),Arc::new(move|v|{tx.send(v).unwrap();Ok(())})).unwrap();
    assert_eq!(finish(&mut rx).await.last().unwrap()["status"], "completed");
    let request = requests.recv().await.unwrap();
    assert!(request.contains(other.path().canonicalize().unwrap().to_str().unwrap()));
    assert_eq!(
        runtime
            .request("GET", "/api/workspace/coding-project", Value::Null)
            .await
            .unwrap()["path"],
        project["path"]
    );
}

#[tokio::test]
async fn reconnect_replays_existing_turn_without_duplicating_user_input() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let file = tempfile::NamedTempFile::new().unwrap();
    let args = json!({"path":file.path()}).to_string();
    let tools = sse(
        json!({"choices":[{"delta":{"content":"I will read the file","tool_calls":[{"index":0,"id":"read-1","function":{"name":"read_file","arguments":args}}]},"finish_reason":"tool_calls"}]}),
    ) + "data: [DONE]\n\n";
    let (url, mut requests) = fixture(vec![("text/event-stream".into(), tools)]).await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let mut original = start(&runtime, "replay-session", "original");
    let pending = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let value = runtime
                .request(
                    "GET",
                    "/api/console/push-messages?session_id=replay-session",
                    Value::Null,
                )
                .await
                .unwrap();
            if !value["pending_approvals"].as_array().unwrap().is_empty() {
                break value;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(pending["pending_approvals"].as_array().unwrap().len(), 1);
    let (tx, mut replay) = mpsc::unbounded_channel();
    runtime
        .start(
            "reattached".into(),
            json!({"session_id":"replay-session","reconnect":true}),
            Arc::new(move |value| {
                tx.send(value).unwrap();
                Ok(())
            }),
        )
        .unwrap();
    assert!(runtime.cancel("reattached").unwrap());
    let frames = finish(&mut replay).await;
    assert_eq!(frames.last().unwrap()["status"], "cancelled");
    assert!(frames
        .iter()
        .any(|v| v.to_string().contains("I will read the file")));
    assert_eq!(
        finish(&mut original).await.last().unwrap()["status"],
        "cancelled"
    );
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    let history = runtime
        .request(
            "GET",
            &format!("/api/chats/{}", chats[0]["id"].as_str().unwrap()),
            Value::Null,
        )
        .await
        .unwrap();
    assert_eq!(
        history["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["role"] == "user")
            .count(),
        1
    );
    assert!(requests.recv().await.is_some());
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn responses_protocol_uses_input_and_requires_completed_event() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let stream = sse(json!({"type":"response.output_text.delta","delta":"Responses 正常"}))
        + &sse(json!({"type":"response.completed"}));
    let (url, mut requests) = fixture(vec![("text/event-stream".into(), stream)]).await;
    configure(&runtime, &url, "OpenAIResponseModel").await;
    let frames = finish(&mut start(&runtime, "session", "run")).await;
    assert_eq!(frames.last().unwrap()["status"], "completed");
    let request = requests.recv().await.unwrap();
    assert!(request.starts_with("POST /responses"));
    assert!(request.contains("\"input\":"));
    assert!(!request.contains("\"messages\":"));
}

#[tokio::test]
async fn truncated_stream_is_failed_and_partial_text_survives() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let (url, _) = fixture(vec![(
        "text/event-stream".into(),
        sse(json!({"choices":[{"delta":{"content":"partial"}}]})),
    )])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let frames = finish(&mut start(&runtime, "session", "run")).await;
    assert_eq!(frames.last().unwrap()["status"], "failed");
    assert!(frames.iter().any(|v| v["object"] == "message"
        && v["content"][0]["text"] == "partial"
        && v["status"] == "failed"));
}

#[tokio::test]
async fn duplicate_turn_rejected_and_cancel_interrupts_waiting_network() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    configure(
        &runtime,
        &format!("http://{}", listener.local_addr().unwrap()),
        "OpenAIChatModel",
    )
    .await;
    let mut rx = start(&runtime, "session", "run");
    let error=runtime.start("other".into(),json!({"session_id":"session","input":[{"role":"user","content":[{"type":"text","text":"duplicate"}]}]}),Arc::new(|_|Ok(()))).unwrap_err();
    assert_eq!(error.status, 409);
    assert!(runtime.cancel("run").unwrap());
    assert_eq!(finish(&mut rx).await.last().unwrap()["status"], "cancelled");
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    assert_eq!(chats[0]["status"], "idle");
}

#[tokio::test]
async fn credentials_encrypted_at_rest_and_masked_in_ui() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    configure(&runtime, "https://example.test/v1", "OpenAIChatModel").await;
    let providers = runtime
        .request("GET", "/api/models", Value::Null)
        .await
        .unwrap();
    assert_eq!(providers[0]["api_key"], "********");
    drop(runtime);
    for entry in std::fs::read_dir(tmp.path()).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("secret-test-key"));
    }
    std::fs::remove_file(tmp.path().join("master.key")).unwrap();
    assert!(Runtime::open(tmp.path()).is_err());
}

#[tokio::test]
async fn approval_is_session_bound_and_tool_loop_returns_to_model() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&tmp.path().join("runtime")).unwrap();
    let file = tmp.path().join("example.txt");
    std::fs::write(&file, "approved content").unwrap();
    let args = json!({"path":file}).to_string();
    let call = sse(
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"read_file","arguments":args}}]},"finish_reason":"tool_calls"}]}),
    ) + "data: [DONE]\n\n";
    let (url, mut requests) = fixture(vec![
        ("text/event-stream".into(), call),
        ("text/event-stream".into(), answer("done")),
    ])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let mut rx = start(&runtime, "session", "run");
    let approval = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let pending = runtime
                .request(
                    "GET",
                    "/api/console/push-messages?session_id=session",
                    Value::Null,
                )
                .await
                .unwrap();
            if !pending["pending_approvals"][0].is_null() {
                return pending["pending_approvals"][0].clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let wrong = runtime
        .request(
            "POST",
            "/api/approval/approve",
            json!({"request_id":approval["request_id"],"session_id":"other","user_id":"default"}),
        )
        .await
        .unwrap_err();
    assert_eq!(wrong.status, 403);
    runtime
        .request(
            "POST",
            "/api/approval/approve",
            json!({"request_id":approval["request_id"],"session_id":"session","user_id":"default"}),
        )
        .await
        .unwrap();
    assert_eq!(finish(&mut rx).await.last().unwrap()["status"], "completed");
    requests.recv().await.unwrap();
    assert!(requests.recv().await.unwrap().contains("approved content"));
}

#[tokio::test]
async fn speech_posts_multipart_wav_to_separately_selected_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let (url, mut requests) = fixture(vec![(
        "application/json".into(),
        json!({"text":"语音识别"}).to_string(),
    )])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    runtime.request("PUT","/api/native/media-settings",json!({"speech_provider_id":"deepseek","speech_model":"test-asr","image_provider_id":"","image_model":""})).await.unwrap();
    assert_eq!(
        runtime
            .transcribe("input.wav".into(), "audio/wav".into(), b"RIFFtest".to_vec())
            .await
            .unwrap()["text"],
        "语音识别"
    );
    let request = requests.recv().await.unwrap();
    assert!(request.starts_with("POST /audio/transcriptions"));
    assert!(request.contains("multipart/form-data"));
    assert!(request.contains("test-asr"));
}

#[tokio::test]
async fn history_import_is_atomic_idempotent_and_can_continue() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let bundle = json!({"format":"potato-native-history-v1","chats":[{"spec":{"id":"old-id","session_id":"old-session","name":"Old chat","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","pinned":false},
        "messages":[{"id":"old-message","object":"message","role":"user","type":"message","status":"completed","content":[{"object":"content","type":"text","text":"remember this"}]}]}]});
    assert_eq!(
        runtime
            .request("POST", "/api/native/import-history", bundle.clone())
            .await
            .unwrap()["imported"],
        1
    );
    assert_eq!(
        runtime
            .request("POST", "/api/native/import-history", bundle.clone())
            .await
            .unwrap()["skipped"],
        1
    );
    let mut broken = bundle;
    broken["chats"][0]["spec"]["id"] = json!("new-id");
    broken["chats"][0]["messages"] = json!(false);
    assert!(runtime
        .request("POST", "/api/native/import-history", broken)
        .await
        .is_err());
    assert_eq!(
        runtime
            .request("GET", "/api/chats", Value::Null)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (url, mut requests) =
        fixture(vec![("text/event-stream".into(), answer("continued"))]).await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    finish(&mut start(&runtime, "old-session", "run")).await;
    assert!(requests.recv().await.unwrap().contains("remember this"));
}

#[tokio::test]
async fn images_are_displayed_without_sending_base64_back_to_chat_model() {
    image_roundtrip(false).await;
}

#[tokio::test]
async fn attached_images_are_uploaded_for_editing_and_results_are_displayed() {
    image_roundtrip(true).await;
}

async fn image_roundtrip(edit: bool) {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let call = sse(
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"image-1","function":{"name":if edit {"edit_image"}else{"generate_image_gpt"},"arguments":"{\"prompt\":\"a potato\"}"}}]},"finish_reason":"tool_calls"}]}),
    ) + "data: [DONE]\n\n";
    let encoded = "iVBORw0KGgo=";
    let (url, mut requests) = fixture(vec![
        ("text/event-stream".into(), call),
        (
            "application/json".into(),
            json!({"data":[{"b64_json":encoded}]}).to_string(),
        ),
        ("text/event-stream".into(), answer("Image ready")),
    ])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    runtime.request("PUT","/api/native/media-settings",json!({"speech_provider_id":"","speech_model":"","image_provider_id":"deepseek","image_model":"test-image"})).await.unwrap();
    let mut rx = if edit {
        let (tx, rx) = mpsc::unbounded_channel();
        runtime.start("run".into(),json!({"session_id":"session","input":[{"role":"user","content":[{"type":"text","text":"edit this"},{"type":"image","image_url":"data:image/png;base64,aW5wdXQ="}]}]}),Arc::new(move|value|{let _=tx.send(value);Ok(())})).unwrap();
        rx
    } else {
        start(&runtime, "session", "run")
    };
    let approval = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let pending = runtime
                .request(
                    "GET",
                    "/api/console/push-messages?session_id=session",
                    Value::Null,
                )
                .await
                .unwrap();
            if !pending["pending_approvals"][0].is_null() {
                break pending["pending_approvals"][0].clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    runtime
        .request(
            "POST",
            "/api/approval/approve",
            json!({"request_id":approval["request_id"],"session_id":"session","user_id":"default"}),
        )
        .await
        .unwrap();
    let frames = finish(&mut rx).await;
    assert_eq!(frames.last().unwrap()["status"], "completed");
    assert!(frames.iter().any(|f| f["content"][0]["image_url"]
        .as_str()
        .is_some_and(|u| u.contains(encoded))));
    requests.recv().await.unwrap();
    let request = requests.recv().await.unwrap();
    assert!(request.starts_with(if edit {
        "POST /images/edits"
    } else {
        "POST /images/generations"
    }));
    if edit {
        assert!(request.contains("multipart/form-data"));
        assert!(request.contains("name=\"image[]\""));
        assert!(request.contains("input"));
    }
    assert!(!requests.recv().await.unwrap().contains(encoded));
}

#[tokio::test]
async fn question_answer_is_streamed_and_saved_before_model_continues() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(root.path()).unwrap();
    let arguments=json!({"title":"选择颜色","multiple":true,"options":[{"id":"red","label":"红色"},{"id":"blue","label":"蓝色"}]}).to_string();
    let call = sse(
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"q-1","function":{"name":"request_user_input","arguments":arguments}}]},"finish_reason":"tool_calls"}]}),
    ) + "data: [DONE]\n\n";
    let (url, mut requests) = fixture(vec![
        ("text/event-stream".into(), call),
        ("text/event-stream".into(), answer("按您的选择处理")),
    ])
    .await;
    configure(&runtime, &url, "OpenAIChatModel").await;
    let mut rx = start(&runtime, "session", "question-run");
    let question = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let value = runtime
                .request("GET", "/api/questions?session_id=session", Value::Null)
                .await
                .unwrap();
            if !value["questions"][0].is_null() {
                return value["questions"][0].clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/chats", Value::Null)
            .await
            .unwrap()[0]["status"],
        "running"
    );
    runtime
        .request(
            "POST",
            &format!(
                "/api/questions/{}/answer",
                question["request_id"].as_str().unwrap()
            ),
            json!({"selected":["red","blue"],"text":"浅色一些","skip":false}),
        )
        .await
        .unwrap();
    let frames = finish(&mut rx).await;
    assert_eq!(frames.last().unwrap()["status"], "completed");
    let user = frames
        .iter()
        .find(|f| f["metadata"]["question_request_id"] == question["request_id"])
        .unwrap();
    assert_eq!(user["metadata"]["question_title"], "选择颜色");
    assert_eq!(user["content"][0]["text"], "红色、蓝色\n浅色一些");
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    let history = runtime
        .request(
            "GET",
            &format!("/api/chats/{}", chats[0]["id"].as_str().unwrap()),
            Value::Null,
        )
        .await
        .unwrap();
    assert!(history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["metadata"]["question_request_id"] == question["request_id"]));
    requests.recv().await.unwrap();
    assert!(requests.recv().await.unwrap().contains("浅色一些"));
}

#[tokio::test]
async fn native_editors_reject_stale_saves_and_delete_prompt_references() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let path = "/api/workspace/files/family.md";
    runtime
        .request(
            "PUT",
            path,
            json!({"content":"first","expected_content":null}),
        )
        .await
        .unwrap();
    runtime
        .request(
            "PUT",
            "/api/workspace/system-prompt-files",
            json!(["family.md"]),
        )
        .await
        .unwrap();
    runtime
        .request(
            "PUT",
            path,
            json!({"content":"changed","expected_content":"first"}),
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .request(
                "PUT",
                path,
                json!({"content":"lost update","expected_content":"first"})
            )
            .await
            .unwrap_err()
            .status,
        409
    );
    assert_eq!(
        runtime.request("GET", path, Value::Null).await.unwrap()["content"],
        "changed"
    );
    runtime.request("DELETE", path, Value::Null).await.unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/workspace/system-prompt-files", Value::Null)
            .await
            .unwrap(),
        json!([])
    );
    assert_eq!(
        runtime
            .request("GET", path, Value::Null)
            .await
            .unwrap_err()
            .status,
        404
    );
    runtime
        .request(
            "POST",
            "/api/skills",
            json!({"name":"family","content":"first"}),
        )
        .await
        .unwrap();
    let skill = "/api/skills/family/content";
    runtime
        .request(
            "PUT",
            skill,
            json!({"content":"changed","expected_content":"first"}),
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .request(
                "PUT",
                skill,
                json!({"content":"lost update","expected_content":"first"})
            )
            .await
            .unwrap_err()
            .status,
        409
    );
    drop(runtime);
    let runtime = Runtime::open(tmp.path()).unwrap();
    assert_eq!(
        runtime.request("GET", skill, Value::Null).await.unwrap()["content"],
        "changed"
    );
}

#[tokio::test]
async fn native_conversation_management_search_and_preferences_persist() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    runtime.request("POST","/api/native/import-history",json!({"format":"potato-native-history-v1","chats":[{"spec":{"id":"family","session_id":"family","name":"旧标题"},"messages":[{"id":"family-message","type":"message","object":"message","status":"completed","role":"user","content":[{"type":"text","text":"周末去公园"}]}]}]})).await.unwrap();
    runtime
        .request(
            "PUT",
            "/api/chats/family",
            json!({"name":"家庭安排","pinned":true}),
        )
        .await
        .unwrap();
    let chats = runtime
        .request("GET", "/api/chats?q=公园", Value::Null)
        .await
        .unwrap();
    assert_eq!(chats[0]["name"], "家庭安排");
    assert_eq!(chats[0]["pinned"], true);
    runtime
        .request("PUT", "/api/chats/family", json!({"archived":true}))
        .await
        .unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/chats", Value::Null)
            .await
            .unwrap(),
        json!([])
    );
    assert_eq!(
        runtime
            .request("GET", "/api/chats?archived=true", Value::Null)
            .await
            .unwrap()[0]["id"],
        "family"
    );
    runtime
        .request(
            "PUT",
            "/api/native/preferences",
            json!({"dark":true,"width":1200,"selected":"family"}),
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .request("PUT", "/api/native/preferences", json!({"width":-1}))
            .await
            .unwrap_err()
            .status,
        400
    );
    drop(runtime);
    let runtime = Runtime::open(tmp.path()).unwrap();
    let prefs = runtime
        .request("GET", "/api/native/preferences", Value::Null)
        .await
        .unwrap();
    assert_eq!(prefs["dark"], true);
    assert_eq!(prefs["width"], 1200);
    runtime
        .request("PUT", "/api/chats/family", json!({"archived":false}))
        .await
        .unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/chats", Value::Null)
            .await
            .unwrap()[0]["id"],
        "family"
    );
}
