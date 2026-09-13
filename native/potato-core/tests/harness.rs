//! Real HTTP/SSE request replays, no provider credentials or billed calls.
use potato_core::Runtime;
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};

fn sse(v: Value) -> String {
    format!("data: {v}\n\n")
}
fn answer(text: &str) -> String {
    sse(json!({"choices":[{"delta":{"content":text},"finish_reason":"stop"}]})) + "data: [DONE]\n\n"
}
fn call(id: &str, name: &str, args: Value, text: &str) -> String {
    sse(
        json!({"choices":[{"delta":{"content":text,"tool_calls":[{"index":0,"id":id,"function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":"tool_calls"}]}),
    ) + "data: [DONE]\n\n"
}
async fn server(
    mut handler: impl FnMut(&Value) -> (u16, String) + Send + 'static,
) -> (String, Arc<Mutex<Vec<Value>>>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let output = requests.clone();
    let task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut bytes = Vec::new();
            let mut chunk = [0; 8192];
            let body = loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .map(|n| n.parse::<usize>().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break serde_json::from_slice::<Value>(&bytes[end + 4..end + 4 + length])
                            .unwrap();
                    }
                }
            };
            let (status, text) = handler(&body);
            output.lock().unwrap().push(body);
            let reply=format!("HTTP/1.1 {status} Reply\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len());
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    (url, requests, task)
}
async fn configure(r: &Runtime, url: &str, responses: bool, capacity: Option<u64>) {
    // These scripted main-model replays use the manual approval helper below.
    r.request("PUT", "/api/workspace/running-config", json!({"reviewer":"user"})).await.unwrap();
    r.request("PUT","/api/models/deepseek/config",json!({"api_key":"fixture-key","base_url":url,"chat_model":if responses{"OpenAIResponseModel"}else{"OpenAIChatModel"}})).await.unwrap();
    r.request(
        "POST",
        "/api/models/deepseek/models",
        json!({"id":"fixture","name":"Fixture"}),
    )
    .await
    .unwrap();
    if let Some(capacity) = capacity {
        r.request(
            "PUT",
            "/api/models/deepseek/models/fixture/config",
            json!({"max_input_length":capacity,"max_tokens":1024}),
        )
        .await
        .unwrap();
    }
    r.request(
        "PUT",
        "/api/models/active",
        json!({"provider_id":"deepseek","model":"fixture"}),
    )
    .await
    .unwrap();
}
fn start(r: &Arc<Runtime>, session: &str, text: &str) -> mpsc::UnboundedReceiver<Value> {
    let (tx, rx) = mpsc::unbounded_channel();
    r.start(uuid::Uuid::new_v4().to_string(),json!({"session_id":session,"input":[{"role":"user","content":[{"type":"text","text":text}]}]}),Arc::new(move|v|{let _=tx.send(v);Ok(())})).unwrap();
    rx
}
async fn finish(mut rx: mpsc::UnboundedReceiver<Value>) -> Value {
    tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(v) = rx.recv().await {
            if v["object"] == "response" && v["status"] != "in_progress" {
                return v;
            }
        }
        panic!("no terminal response")
    })
    .await
    .unwrap()
}
async fn approve(r: &Runtime, session: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let pending = r
                .request(
                    "GET",
                    &format!("/api/console/push-messages?session_id={session}"),
                    Value::Null,
                )
                .await
                .unwrap();
            if let Some(id) = pending["pending_approvals"][0]["request_id"].as_str() {
                r.request(
                    "POST",
                    "/api/approval/approve",
                    json!({"request_id":id,"session_id":session,"user_id":"default"}),
                )
                .await
                .unwrap();
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
async fn stats(r: &Runtime, session: &str) -> Value {
    let chats = r.request("GET", "/api/chats", Value::Null).await.unwrap();
    let list = chats
        .as_array()
        .or_else(|| chats["chats"].as_array())
        .unwrap();
    let id = list.iter().find(|c| c["session_id"] == session).unwrap()["id"]
        .as_str()
        .unwrap();
    r.request(
        "GET",
        &format!("/api/chats/{id}/context-stats"),
        Value::Null,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn request_prefix_and_routing_key_survive_restart_in_both_protocols() {
    for responses in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (url,requests,task)=server(move |_|(200,if responses {
            sse(json!({"type":"response.output_text.delta","delta":"done"}))+&sse(json!({"type":"response.completed","response":{"usage":{"input_tokens":120,"output_tokens":8,"input_tokens_details":{"cached_tokens":90}}}}))
        }else{
            sse(json!({"choices":[{"delta":{"content":"done"},"finish_reason":"stop"}]}))+&sse(json!({"choices":[],"usage":{"prompt_tokens":120,"completion_tokens":8,"prompt_cache_hit_tokens":90}}))+"data: [DONE]\n\n"
        })).await;
        let r = Runtime::open(dir.path()).unwrap();
        configure(&r, &url, responses, None).await;
        assert_eq!(
            finish(start(&r, "cache", "first")).await["status"],
            "completed"
        );
        drop(r);
        let r = Runtime::open(dir.path()).unwrap();
        assert_eq!(
            finish(start(&r, "cache", "second")).await["status"],
            "completed"
        );
        let stats = stats(&r, "cache").await;
        assert_eq!(stats["usage"]["cached_input_tokens"], 90);
        assert_eq!(stats["usage"]["input_tokens"], 120);
        assert_eq!(
            stats["context"]["measurement_source"],
            "provider_plus_estimate"
        );
        assert_eq!(stats["context"]["estimate_only"], false);
        let frames = requests.lock().unwrap();
        assert_eq!(frames.len(), 2);
        let field = if responses { "input" } else { "messages" };
        let first = frames[0][field].as_array().unwrap();
        let second = frames[1][field].as_array().unwrap();
        assert_eq!(first, &second[..first.len()]);
        assert_eq!(frames[0]["tools"], frames[1]["tools"]);
        assert!(!first[0]["content"]
            .as_str()
            .unwrap()
            .contains("Current time (UTC):"));
        assert!(first[1].to_string().contains("Current time (UTC):"));
        if responses {
            assert!(!frames[0]["prompt_cache_key"].as_str().unwrap().is_empty());
            assert_eq!(frames[0]["prompt_cache_key"], frames[1]["prompt_cache_key"]);
        }
        task.abort();
    }
}

#[tokio::test]
async fn large_result_is_previewed_and_original_is_recalled_inside_the_tool_loop() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("big.txt");
    std::fs::write(
        &path,
        format!("{}MIDDLE-FACT{}", "a".repeat(30_000), "z".repeat(30_000)),
    )
    .unwrap();
    let mut turn = 0;
    let (url, requests, task) = server(move |_| {
        turn += 1;
        (
            200,
            match turn {
                1 => call("read", "read_file", json!({"file_path":path}), ""),
                2 => call(
                    "recall",
                    "recall_history",
                    json!({"op":"recall_tool","message_index":2,"offset":29_000,"limit":2000}),
                    "",
                ),
                _ => answer("done"),
            },
        )
    })
    .await;
    let r = Runtime::open(&root.path().join("runtime")).unwrap();
    configure(&r, &url, false, None).await;
    let rx = start(&r, "large", "Find the middle fact");
    approve(&r, "large").await;
    assert_eq!(finish(rx).await["status"], "completed");
    let frames = requests.lock().unwrap();
    let after_read = &frames[1]["messages"];
    let output = after_read
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "tool")
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(output.len() < 17_000);
    assert!(output.contains("message_index=2"));
    assert!(!output.contains("MIDDLE-FACT"));
    let recall = frames[2]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "tool")
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(recall.contains("MIDDLE-FACT"));
    task.abort();
}

#[tokio::test]
async fn a_long_single_turn_compacts_closed_steps_and_retains_the_actual_user_request() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("result.txt");
    std::fs::write(&path, "tool-result\n".repeat(1200)).unwrap();
    let mut step = 0;
    let (url, requests, task) = server(move |request| {
        if request["messages"][0]["content"]
            .as_str()
            .unwrap_or("")
            .starts_with("Summarize conversation history")
        {
            return (
                200,
                answer(
                    "Goal: preserve exact task. Several reads completed. Continue remaining reads.",
                ),
            );
        }
        step += 1;
        (
            200,
            if step <= 12 {
                call(
                    &format!("read-{step}"),
                    "read_file",
                    json!({"file_path":path}),
                    &format!("Step {step} {}", "analysis ".repeat(400)),
                )
            } else {
                answer("done")
            },
        )
    })
    .await;
    let r = Runtime::open(&root.path().join("runtime")).unwrap();
    // Leave room for the native prompt/tool catalog and an unconsumed result;
    // twelve verbose exchanges must still force compaction (asserted below).
    configure(&r, &url, false, Some(18_000)).await;
    let rx = start(
        &r,
        "long-turn",
        "USER-GOAL-EXACT: inspect twelve reads and preserve this request.",
    );
    let mut completion = tokio::spawn(finish(rx));
    for _ in 0..12 {
        tokio::select! {
            _ = approve(&r, "long-turn") => {},
            result = &mut completion => panic!("Turn ended before all twelve reads: {}", result.unwrap()),
        }
    }
    let end = completion.await.unwrap();
    assert_eq!(end["status"], "completed", "{end}");
    let state = stats(&r, "long-turn").await;
    assert!(state["context"]["covered_messages"].as_u64().unwrap() > 0);
    let frames = requests.lock().unwrap();
    assert!(frames.iter().any(|r| r["messages"][0]["content"]
        .as_str()
        .unwrap_or("")
        .starts_with("Summarize conversation history")));
    for request in frames.iter().filter(|r| {
        !r["messages"][0]["content"]
            .as_str()
            .unwrap_or("")
            .starts_with("Summarize conversation history")
    }) {
        let messages = request["messages"].as_array().unwrap();
        assert!(messages
            .iter()
            .any(|m| m["role"] == "user" && m.to_string().contains("USER-GOAL-EXACT")));
        for (i, m) in messages.iter().enumerate() {
            if let Some(calls) = m["tool_calls"].as_array() {
                let outputs: Vec<_> = messages[i + 1..]
                    .iter()
                    .take_while(|m| m["role"] == "tool")
                    .collect();
                for call in calls {
                    assert_eq!(
                        outputs
                            .iter()
                            .filter(|m| m["tool_call_id"] == call["id"])
                            .count(),
                        1
                    );
                }
            }
        }
    }
    task.abort();
}

#[tokio::test]
async fn malformed_arguments_are_returned_to_the_model_as_a_recoverable_error() {
    let mut step = 0;
    let (url,requests,task)=server(move |_|{step+=1;(200,if step==1{
        sse(json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"bad","function":{"name":"read_file","arguments":"{broken"}}]},"finish_reason":"tool_calls"}]}))+"data: [DONE]\n\n"
    }else{answer("corrected")})}).await;
    let root = tempfile::tempdir().unwrap();
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, false, None).await;
    assert_eq!(
        finish(start(&r, "bad", "read")).await["status"],
        "completed"
    );
    assert!(requests.lock().unwrap()[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "tool")
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .contains("valid JSON object"));
    task.abort();
}

fn is_summary(request: &Value) -> bool {
    request["messages"][0]["content"]
        .as_str()
        .unwrap_or("")
        .starts_with("Summarize conversation history")
}
fn assert_tool_pairs(messages: &[Value]) {
    let mut pending = std::collections::HashSet::new();
    for message in messages {
        if message["role"] == "tool" {
            assert!(
                pending.remove(message["tool_call_id"].as_str().unwrap()),
                "orphan or duplicate result: {message}"
            );
        } else {
            assert!(
                pending.is_empty(),
                "tool results must immediately follow their calls"
            );
            for call in message["tool_calls"].as_array().into_iter().flatten() {
                assert!(pending.insert(call["id"].as_str().unwrap()));
            }
        }
    }
    assert!(pending.is_empty(), "missing tool result");
}
// Replay the archive independently so persistence assertions do not only
// exercise Runtime's own history reader.
fn transcript_rows(root: &std::path::Path, chat: &str) -> Vec<(Value, Option<Value>)> {
    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, chat.as_bytes());
    let dir = root.join("workspace/history/sessions").join(id.to_string());
    let source = std::fs::read_to_string(dir.join("transcript.jsonl")).unwrap();
    let mut rows: Vec<(Value, Option<Value>)> = Vec::new();
    for line in source.lines() {
        let envelope: Value = serde_json::from_str(line).unwrap();
        assert_eq!(envelope["format"], "potato-transcript-v1");
        let mut record = envelope["record"].clone();
        for reference in envelope["text_refs"].as_array().unwrap() {
            let content =
                std::fs::read_to_string(dir.join(reference["path"].as_str().unwrap())).unwrap();
            *record
                .pointer_mut(reference["pointer"].as_str().unwrap())
                .unwrap() = json!(content);
        }
        for event in record["events"].as_array().unwrap() {
            match event["type"].as_str().unwrap() {
                "session" => {}
                "append" => rows.push((
                    event["frame"].clone(),
                    (!event["wire"].is_null()).then(|| event["wire"].clone()),
                )),
                "replace_frame" => {
                    rows[event["index"].as_u64().unwrap() as usize].0 = event["frame"].clone()
                }
                "replace_wire" => {
                    rows[event["index"].as_u64().unwrap() as usize].1 =
                        (!event["wire"].is_null()).then(|| event["wire"].clone())
                }
                "deleted" => rows.clear(),
                other => panic!("Unknown transcript event: {other}"),
            }
        }
    }
    rows
}
fn all_transcript_rows(root: &std::path::Path) -> Vec<(Value, Option<Value>)> {
    let db = rusqlite::Connection::open(root.join("potato.sqlite3")).unwrap();
    let mut statement = db.prepare("SELECT id FROM chats").unwrap();
    let chats = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap();
    chats
        .flat_map(|chat| transcript_rows(root, &chat.unwrap()))
        .collect()
}
fn durable_context(root: &std::path::Path, session: &str) -> (String, Vec<String>) {
    let db = rusqlite::Connection::open(root.join("potato.sqlite3")).unwrap();
    let chat: String = db
        .query_row("SELECT id FROM chats WHERE session_id=?", [session], |r| {
            r.get(0)
        })
        .unwrap();
    let checkpoint: String = db
        .query_row(
            "SELECT value FROM settings WHERE key=?",
            [format!("context_summary:{chat}")],
            |r| r.get(0),
        )
        .unwrap();
    let history = transcript_rows(root, &chat)
        .into_iter()
        .filter_map(|(_, wire)| wire.map(|value| value.to_string()))
        .collect();
    (checkpoint, history)
}

#[tokio::test]
async fn context_regression_provider_overflow_compacts_then_retries_once() {
    let root = tempfile::tempdir().unwrap();
    let mut step = 0;
    let (url,requests,task)=server(move |request|{
        if is_summary(request){return (200,answer("Checkpoint: continue USER-OVERFLOW-GOAL after the completed searches."));}
        step+=1;
        match step {
            1..=3=>(200,call(&format!("search-{step}"),"recall_history",json!({"op":"search","query":"missing-fixture"}),"Searching previous evidence")),
            4=>(400,json!({"error":{"code":"context_length_exceeded","message":"maximum context length exceeded"}}).to_string()),
            5=>(200,answer("recovered")),
            _=>panic!("unexpected extra retry"),
        }
    }).await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, false, None).await;
    // Exercise summary recovery specifically; default folding may resolve overflow
    // before a summary is needed. Earlier successful calls consumed the prefix.
    r.request(
        "PUT",
        "/api/workspace/running-config",
        json!({"context_policy":{"fold":false}}),
    )
    .await
    .unwrap();
    let end = finish(start(
        &r,
        "overflow",
        "USER-OVERFLOW-GOAL: inspect previous evidence",
    ))
    .await;
    assert_eq!(end["status"], "completed", "{end}");
    let frames = requests.lock().unwrap();
    let conversation: Vec<_> = frames.iter().filter(|r| !is_summary(r)).collect();
    assert_eq!(conversation.len(), 5);
    assert!(frames.iter().any(is_summary));
    assert_ne!(conversation[3]["messages"], conversation[4]["messages"]);
    for request in conversation {
        let messages = request["messages"].as_array().unwrap();
        assert_tool_pairs(messages);
        assert!(messages
            .iter()
            .any(|m| m["role"] == "user" && m.to_string().contains("USER-OVERFLOW-GOAL")));
    }
    let (checkpoint, history) = durable_context(root.path(), "overflow");
    assert!(
        serde_json::from_str::<Value>(&checkpoint).unwrap()["covered"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        history
            .iter()
            .filter(|m| serde_json::from_str::<Value>(m).unwrap()["role"] == "tool")
            .count(),
        3
    );
    task.abort();
}

#[tokio::test]
async fn context_regression_failed_summaries_preserve_checkpoint_and_raw_history() {
    for failure in ["http", "truncated", "empty", "tool_call"] {
        let root = tempfile::tempdir().unwrap();
        let db_root = root.path().to_owned();
        let mut step = 0;
        let before = Arc::new(Mutex::new(None));
        let captured = before.clone();
        let (url,requests,task)=server(move |request|{
            if is_summary(request){
                *captured.lock().unwrap()=Some(durable_context(&db_root,"summary-failure"));
                return match failure {
                    "http"=>(500,json!({"error":{"message":"fixture summary unavailable"}}).to_string()),
                    "truncated"=>(200,sse(json!({"choices":[{"delta":{"content":"partial checkpoint must not commit"}}]}))),
                    "empty"=>(200,answer("")),
                    _=>(200,call("unsafe-summary-call","recall_history",json!({"op":"search","query":"x"}),"not a checkpoint")),
                };
            }
            step+=1;
            if step<=2 {(200,call(&format!("evidence-{step}"),"recall_history",json!({"op":"search","query":"missing-fixture"}),"RAW-EVIDENCE-MUST-SURVIVE"))}
            else {(400,json!({"error":{"code":"context_length_exceeded"}}).to_string())}
        }).await;
        let r = Runtime::open(root.path()).unwrap();
        configure(&r, &url, false, None).await;
        // Exercise summary recovery specifically; default folding may resolve overflow
        // before a summary is needed. Earlier successful calls consumed the prefix.
        r.request(
            "PUT",
            "/api/workspace/running-config",
            json!({"context_policy":{"fold":false}}),
        )
        .await
        .unwrap();
        let end = finish(start(&r, "summary-failure", "USER-FAILURE-GOAL")).await;
        assert_eq!(end["status"], "failed", "{failure}: {end}");
        let old = before
            .lock()
            .unwrap()
            .clone()
            .expect("summary was requested");
        let after = durable_context(root.path(), "summary-failure");
        assert_eq!(after.0, old.0, "{failure} advanced the checkpoint");
        assert_eq!(
            &after.1[..old.1.len()],
            old.1.as_slice(),
            "{failure} changed raw history"
        );
        assert!(after
            .1
            .iter()
            .any(|m| m.contains("RAW-EVIDENCE-MUST-SURVIVE")));
        assert_eq!(
            requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| !is_summary(r))
                .count(),
            3,
            "retry after unusable summary"
        );
        drop(r);
        let reopened = Runtime::open(root.path()).unwrap();
        assert_eq!(
            durable_context(root.path(), "summary-failure"),
            after,
            "restart changed durable evidence"
        );
        drop(reopened);
        task.abort();
    }
}

#[tokio::test]
async fn context_regression_uncompactable_inputs_fail_before_provider_request() {
    for source in ["user", "system", "tools"] {
        let root = tempfile::tempdir().unwrap();
        let (url, requests, task) = server(|_| (200, answer("should never be requested"))).await;
        let r = Runtime::open(root.path()).unwrap();
        configure(&r, &url, false, Some(14_000)).await;
        let huge = "UNCOMPACTABLE-EVIDENCE ".repeat(4500);
        if source == "system" {
            r.request(
                "PUT",
                "/api/workspace/files/AGENTS.md",
                json!({"content":huge}),
            )
            .await
            .unwrap();
        } else if source == "tools" {
            // Seed an already-discovered MCP catalog; no remote MCP service is involved.
            let db = rusqlite::Connection::open(root.path().join("potato.sqlite3")).unwrap();
            let clients = json!({"fixture":{"enabled":true,"catalog":[{"name":"large_schema","native_name":"mcp_fixture_large_schema","description":huge,"inputSchema":{"type":"object","properties":{}}}]}});
            db.execute("INSERT INTO settings(key,value) VALUES ('mcp_clients',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[clients.to_string()]).unwrap();
        }
        let end = finish(start(
            &r,
            "uncompactable",
            if source == "user" {
                &huge
            } else {
                "small request"
            },
        ))
        .await;
        assert_eq!(end["status"], "failed", "{source}: {end}");
        assert!(
            end.to_string().contains("Context cannot fit"),
            "{source}: {end}"
        );
        assert!(
            requests.lock().unwrap().is_empty(),
            "{source}: oversized request reached provider"
        );
        task.abort();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn execution_regression_failed_shell_job_is_archived_and_recalled() {
    let root = tempfile::tempdir().unwrap();
    let mut step = 0;
    let (url,requests,task)=server(move |request|{
        step+=1;(200,match step {
            1=>call("shell-failure","execute_shell_command",json!({"command":"printf 'fixture-stdout'; printf 'fixture-stderr' >&2; exit 7","run_in_background":false,"timeout":5}),""),
            2=>{
                let output:Value=serde_json::from_str(request["messages"].as_array().unwrap().iter().rev().find(|m| m["role"] == "tool").unwrap()["content"].as_str().unwrap()).unwrap();
                assert_eq!(output["status"],"completed");assert_eq!(output["return_code"],7);
                call("retrieve-job","job_output",json!({"job_id":output["job_id"],"stream":"stderr"}),"")
            },
            _=>answer("Failure recorded and stderr inspected"),
        })
    }).await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, false, None).await;
    r.request(
        "PUT",
        "/api/workspace/running-config",
        json!({"sandbox_mode":"danger-full-access"}),
    )
    .await
    .unwrap();
    let mut rx = start(
        &r,
        "shell-job",
        "Execute the fixture command and inspect its failed job output",
    );
    approve(&r, "shell-job").await;
    let events = tokio::time::timeout(Duration::from_secs(20), async {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            let done = event["object"] == "response" && event["status"] != "in_progress";
            events.push(event);
            if done {
                return events;
            }
        }
        panic!("no terminal response")
    })
    .await
    .unwrap();
    assert_eq!(events.last().unwrap()["status"], "completed");
    let failed = events
        .iter()
        .find(|e| e["type"] == "function_call_output" && e["status"] == "completed")
        .expect("completed shell tool frame must be emitted");
    assert_eq!(failed["content"][0]["data"]["state"], "success");
    assert_eq!(failed["content"][0]["data"]["call_id"], "shell-failure");
    let frames = requests.lock().unwrap();
    assert_eq!(frames.len(), 3);
    let recalled: Value = serde_json::from_str(
        frames[2]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|m| m["role"] == "tool")
            .unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(recalled["output"], "fixture-stderr");
    assert_eq!(recalled["status"], "completed");
    assert_eq!(recalled["return_code"], 7);
    let id = recalled["job_id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok());
    let state: Value = serde_json::from_slice(
        &std::fs::read(
            root.path()
                .join("workspace/history/jobs")
                .join(id)
                .join("state.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(state["job_id"], id);
    assert_eq!(state["status"], "completed");
    assert_eq!(state["return_code"], 7);
    let persisted = all_transcript_rows(root.path())
        .into_iter()
        .filter(|(frame, _)| frame == failed)
        .count();
    assert_eq!(persisted, 1);
    task.abort();
}

#[tokio::test]
async fn execution_regression_responses_replays_encrypted_reasoning_without_chat_metadata() {
    let root = tempfile::tempdir().unwrap();
    let mut step = 0;
    let reasoning = json!({"id":"rs_fixture","type":"reasoning","summary":[],"encrypted_content":"opaque-fixture-ciphertext+/="});
    let output_reasoning = reasoning.clone();
    let (url,requests,task)=server(move |request|{
        step+=1;
        (200,if step==1 {
            sse(json!({"type":"response.output_item.done","output_index":0,"item":output_reasoning}))
                +&sse(json!({"type":"response.output_item.done","output_index":1,"item":{"type":"function_call","id":"fc_fixture","call_id":"call_fixture","name":"get_token_usage","arguments":"{}"}}))
                +&sse(json!({"type":"response.completed","response":{}}))
        } else if request.get("input").is_some() {
            sse(json!({"type":"response.output_text.delta","delta":"replayed"}))+&sse(json!({"type":"response.completed","response":{}}))
        } else {answer("chat protocol works")})
    }).await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, true, None).await;
    let end = finish(start(&r, "reasoning-replay", "Search previous evidence")).await;
    assert_eq!(end["status"], "completed", "{end}");
    {
        let frames = requests.lock().unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0]["store"], false);
        assert!(frames[0]["include"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "reasoning.encrypted_content"));
        let input = frames[1]["input"].as_array().unwrap();
        let index = input
            .iter()
            .position(|item| item == &reasoning)
            .expect("exact encrypted reasoning item must be replayed");
        assert_eq!(input[index + 1]["type"], "function_call");
        assert_eq!(input[index + 1]["call_id"], "call_fixture");
        assert_eq!(input[index + 2]["type"], "function_call_output");
        assert_eq!(input[index + 2]["call_id"], "call_fixture");
    }
    let (_, history) = durable_context(root.path(), "reasoning-replay");
    assert!(
        history
            .iter()
            .any(|m| m.contains("opaque-fixture-ciphertext+/=")),
        "reasoning must survive restart"
    );
    drop(r);
    let r = Runtime::open(root.path()).unwrap();
    r.request(
        "PUT",
        "/api/models/deepseek/config",
        json!({"api_key":"fixture-key","base_url":url,"chat_model":"OpenAIChatModel"}),
    )
    .await
    .unwrap();
    let end = finish(start(
        &r,
        "reasoning-replay",
        "Continue using Chat protocol",
    ))
    .await;
    assert_eq!(end["status"], "completed", "{end}");
    let frames = requests.lock().unwrap();
    assert_eq!(frames.len(), 3);
    assert!(!frames[2].to_string().contains("_responses_reasoning"));
    assert!(!frames[2].to_string().contains("_responses_output"));
    assert!(!frames[2].to_string().contains("opaque-fixture-ciphertext"));
    assert_tool_pairs(frames[2]["messages"].as_array().unwrap());
    task.abort();
}

#[tokio::test]
async fn responses_preserves_interleaved_output_order_across_restart() {
    let root = tempfile::tempdir().unwrap();
    let output = json!([
        {"id":"rs_a","type":"reasoning","summary":[],"encrypted_content":"cipher-a"},
        {"id":"fc_a","type":"function_call","call_id":"call_a","name":"get_token_usage","arguments":"{}"},
        {"id":"rs_b","type":"reasoning","summary":[],"encrypted_content":"cipher-b"},
        {"id":"msg_b","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Checking evidence","annotations":[]}]},
        {"id":"rs_c","type":"reasoning","summary":[],"encrypted_content":"cipher-c"},
        {"id":"fc_c","type":"function_call","call_id":"call_c","name":"get_token_usage","arguments":"{}"}
    ]);
    let expected = output.as_array().unwrap().clone();
    let mut step = 0;
    let (url, requests, task) = server(move |_| {
        step += 1;
        let body = if step == 1 {
            let mut body = sse(json!({"type":"response.output_text.delta","output_index":3,"item_id":"msg_b","delta":"Checking evidence"}));
            // Arrival order is deliberately different from output order.
            for i in [5, 0, 3, 2, 1, 4] {
                body += &sse(json!({"type":"response.output_item.done","output_index":i,"item":output[i]}));
            }
            body
        } else {
            sse(json!({"type":"response.output_text.delta","output_index":0,"delta":"done"}))
        };
        (200, body + &sse(json!({"type":"response.completed","response":{}})))
    }).await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, true, None).await;
    assert_eq!(
        finish(start(&r, "ordered", "Check usage twice")).await["status"],
        "completed"
    );
    drop(r);
    let r = Runtime::open(root.path()).unwrap();
    assert_eq!(
        finish(start(&r, "ordered", "Continue")).await["status"],
        "completed"
    );
    let frames = requests.lock().unwrap();
    assert_eq!(frames.len(), 3);
    for frame in &frames[1..] {
        let input = frame["input"].as_array().unwrap();
        let start = input.iter().position(|v| v["id"] == "rs_a").unwrap();
        assert_eq!(&input[start..start + expected.len()], expected.as_slice());
        assert_eq!(input[start + 6]["call_id"], "call_a");
        assert_eq!(input[start + 7]["call_id"], "call_c");
        assert_eq!(input.iter().filter(|v| v["id"] == "msg_b").count(), 1);
    }
    task.abort();
}

fn calls(items: &[(&str, &str, Value)]) -> String {
    let calls: Vec<_> = items.iter().enumerate().map(|(index,(id,name,args))| json!({"index":index,"id":id,"function":{"name":name,"arguments":args.to_string()}})).collect();
    sse(json!({"choices":[{"delta":{"tool_calls":calls},"finish_reason":"tool_calls"}]}))
        + "data: [DONE]\n\n"
}
async fn pending_count(r: &Runtime, session: &str, n: usize) -> Vec<Value> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let v = r
                .request(
                    "GET",
                    &format!("/api/approval/list?session_id={session}"),
                    Value::Null,
                )
                .await
                .unwrap();
            let pending = v["pending_approvals"].as_array().unwrap();
            if pending.len() == n {
                return pending.clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
async fn approve_id(r: &Runtime, session: &str, id: &Value) {
    r.request(
        "POST",
        "/api/approval/approve",
        json!({"request_id":id,"session_id":session,"user_id":"default"}),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn parallel_reads_are_bounded_ordered_and_unconsumed_results_stay_visible() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..5)
        .map(|i| {
            let p = root.path().join(format!("{i}.txt"));
            std::fs::write(&p, format!("FACT-{i}:{}", "x".repeat(14_000))).unwrap();
            p
        })
        .collect();
    let tool_calls = calls(
        &paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    match i {
                        0 => "r0",
                        1 => "r1",
                        2 => "r2",
                        3 => "r3",
                        _ => "r4",
                    },
                    "read_file",
                    json!({"file_path":p}),
                )
            })
            .collect::<Vec<_>>(),
    );
    let mut step = 0;
    let (url, requests, server) = server(move |_| {
        step += 1;
        (
            200,
            if step == 1 {
                tool_calls.clone()
            } else {
                answer("done")
            },
        )
    })
    .await;
    let r = Runtime::open(&root.path().join("runtime")).unwrap();
    configure(&r, &url, false, Some(80_000)).await;
    r.request("PUT","/api/workspace/running-config",json!({"approval_level":"STRICT","max_parallel_reads":2,"context_policy":{"trigger_ratio":0.20,"target_ratio":0.1}})).await.unwrap();
    let rx = start(&r, "parallel", "Read all five facts before deciding");
    let pending = pending_count(&r, "parallel", 2).await;
    assert_eq!(
        pending.len(),
        2,
        "both independent reads must reach approval concurrently"
    );
    // Approve in reverse order; model evidence must still follow call order.
    let second = pending
        .iter()
        .find(|p| {
            p["tool_params"]["path"]
                .as_str()
                .unwrap()
                .ends_with("1.txt")
        })
        .unwrap();
    approve_id(&r, "parallel", &second["request_id"]).await;
    let first = pending
        .iter()
        .find(|p| p["request_id"] != second["request_id"])
        .unwrap();
    approve_id(&r, "parallel", &first["request_id"]).await;
    for _ in 0..3 {
        approve(&r, "parallel").await;
    }
    let end = finish(rx).await;
    assert_eq!(end["status"], "completed", "{end}");
    let frames = requests.lock().unwrap();
    assert_eq!(
        frames.len(),
        2,
        "normal pressure must not summarize this active turn"
    );
    let messages = frames[1]["messages"].as_array().unwrap();
    assert_tool_pairs(messages);
    let outputs: Vec<_> = messages.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(outputs.len(), 5);
    for (i, m) in outputs.iter().enumerate() {
        assert_eq!(m["tool_call_id"], format!("r{i}"));
        assert!(m["content"]
            .as_str()
            .unwrap()
            .contains(&format!("FACT-{i}")));
    }
    assert!(messages.last().unwrap()["content"]
        .as_str()
        .unwrap()
        .contains("runtime_context_notice"));
    server.abort();
}

#[tokio::test]
async fn steering_releases_approval_skips_unstarted_actions_and_preserves_pairs() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("secret-action.txt");
    std::fs::write(&file, "unchanged").unwrap();
    let mut step = 0;
    let (url, requests, server) = server(move |_| {
        step += 1;
        (
            200,
            if step == 1 {
                calls(&[
                    ("read", "read_file", json!({"file_path":file})),
                    (
                        "shell",
                        "execute_shell_command",
                        json!({"command":"echo MUST-NOT-RUN"}),
                    ),
                ])
            } else {
                answer("followed correction")
            },
        )
    })
    .await;
    let r = Runtime::open(&root.path().join("runtime")).unwrap();
    configure(&r, &url, false, None).await;
    r.request(
        "PUT",
        "/api/workspace/running-config",
        json!({"approval_level":"STRICT"}),
    )
    .await
    .unwrap();
    let rx = start(&r, "steering", "initial request");
    pending_count(&r, "steering", 1).await;
    assert!(r
        .request(
            "POST",
            "/api/agent/steer",
            json!({"session_id":"other","text":"wrong chat"})
        )
        .await
        .is_err());
    let accepted=r.request("POST","/api/agent/steer",json!({"session_id":"steering","text":"CORRECTION: stop reading and explain the design"})).await.unwrap();
    assert_eq!(accepted["status"], "queued");
    let end = finish(rx).await;
    assert_eq!(end["status"], "completed", "{end}");
    assert!(r
        .request(
            "POST",
            "/api/agent/steer",
            json!({"session_id":"steering","text":"too late"})
        )
        .await
        .is_err());
    {
        let frames = requests.lock().unwrap();
        assert_eq!(frames.len(), 2);
        let messages = frames[1]["messages"].as_array().unwrap();
        assert_tool_pairs(messages);
        let shell = messages
            .iter()
            .find(|m| m["tool_call_id"] == "shell")
            .unwrap();
        assert!(shell["content"].as_str().unwrap().contains("not started"));
        let correction = messages
            .iter()
            .position(|m| m.to_string().contains("CORRECTION"))
            .unwrap();
        let shell_pos = messages
            .iter()
            .position(|m| m["tool_call_id"] == "shell")
            .unwrap();
        assert!(correction > shell_pos);
    }
    assert!(!stats(&r, "steering").await.is_null());
    assert!(
        !root
            .path()
            .join("runtime/workspace/history/jobs")
            .read_dir()
            .is_ok_and(|mut d| d.next().is_some()),
        "no shell job was started"
    );
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steering_during_final_model_response_is_consumed_before_success() {
    let root = tempfile::tempdir().unwrap();
    let (ready, mut received) = mpsc::unbounded_channel();
    let (release, wait) = std::sync::mpsc::channel();
    let mut step = 0;
    let (url, requests, server) = server(move |_| {
        step += 1;
        if step == 1 {
            ready.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        (
            200,
            answer(if step == 1 {
                "FIRST-FINAL"
            } else {
                "CORRECTED-FINAL"
            }),
        )
    })
    .await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, false, None).await;
    let rx = start(&r, "boundary", "initial");
    tokio::time::timeout(Duration::from_secs(5), received.recv())
        .await
        .unwrap()
        .unwrap();
    let accepted = r
        .request(
            "POST",
            "/api/agent/steer",
            json!({"session_id":"boundary","text":"BOUNDARY-CORRECTION"}),
        )
        .await
        .unwrap();
    assert_eq!(accepted["status"], "queued");
    release.send(()).unwrap();
    let end = finish(rx).await;
    assert_eq!(end["status"], "completed", "{end}");
    let frames = requests.lock().unwrap();
    assert_eq!(frames.len(), 2);
    assert!(frames[1]["messages"]
        .to_string()
        .contains("BOUNDARY-CORRECTION"));
    server.abort();
}

#[tokio::test]
async fn writes_are_barriers_between_read_batches() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("shared.txt");
    std::fs::write(&file, "OLD-CONTENT").unwrap();
    let mut step = 0;
    let (url, requests, server) = server(move |_| {
        step += 1;
        (
            200,
            if step == 1 {
                calls(&[
                    ("before", "read_file", json!({"file_path":"shared.txt"})),
                    (
                        "write",
                        "write_file",
                        json!({"file_path":"shared.txt","content":"NEW-CONTENT"}),
                    ),
                    ("after", "read_file", json!({"file_path":"shared.txt"})),
                ])
            } else {
                answer("done")
            },
        )
    })
    .await;
    let r = Runtime::open(&root.path().join("runtime")).unwrap();
    configure(&r, &url, false, None).await;
    let (tx, rx) = mpsc::unbounded_channel();
    r.start("barrier-run".into(),json!({"session_id":"barrier","request_context":{"potato.coding_project_dir":project,"sandbox_mode":"workspace-write","approval_level":"STRICT"},"input":[{"role":"user","content":[{"type":"text","text":"read, write, then read"}]}]}),Arc::new(move|v|{let _=tx.send(v);Ok(())})).unwrap();
    for expected in ["read_file", "write_file", "read_file"] {
        let pending = pending_count(&r, "barrier", 1).await;
        assert_eq!(pending[0]["tool_name"], expected);
        if expected == "write_file" {
            assert_eq!(std::fs::read_to_string(&file).unwrap(), "OLD-CONTENT");
        }
        approve_id(&r, "barrier", &pending[0]["request_id"]).await;
    }
    let end = finish(rx).await;
    assert_eq!(end["status"], "completed", "{end}");
    let frames = requests.lock().unwrap();
    let messages = frames[1]["messages"].as_array().unwrap();
    assert_tool_pairs(messages);
    assert!(messages
        .iter()
        .find(|m| m["tool_call_id"] == "before")
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .contains("OLD-CONTENT"));
    assert!(messages
        .iter()
        .find(|m| m["tool_call_id"] == "after")
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .contains("NEW-CONTENT"));
    server.abort();
}

#[tokio::test]
async fn disabled_compaction_preserves_raw_evidence_and_still_rejects_overflow() {
    let root = tempfile::tempdir().unwrap();
    let (url, requests, server) = server(|_| (200, answer("unexpected"))).await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, false, Some(14_000)).await;
    let config = r
        .request(
            "PUT",
            "/api/workspace/running-config",
            json!({"context_policy":{"automatic":false,"fold":false,"summarize":false}}),
        )
        .await
        .unwrap();
    assert_eq!(config["context_policy"]["protect_recent"], 5);
    assert!(r
        .request(
            "PUT",
            "/api/workspace/running-config",
            json!({"context_policy":{"trigger_ratio":0.5,"target_ratio":0.7}})
        )
        .await
        .is_err());
    let end = finish(start(&r, "disabled", &"UNREAD ".repeat(30_000))).await;
    assert_eq!(end["status"], "failed");
    assert!(end.to_string().contains("Context cannot fit"));
    assert!(requests.lock().unwrap().is_empty());
    let raw = all_transcript_rows(root.path())
        .into_iter()
        .find_map(|(_, wire)| wire)
        .unwrap()
        .to_string();
    assert!(raw.contains(&"UNREAD ".repeat(30_000)));
    server.abort();
}

#[tokio::test]
async fn ordinary_steps_do_not_accumulate_notices_or_repeat_static_guidance_and_indexes() {
    let root = tempfile::tempdir().unwrap();
    let mut step = 0;
    let (url, requests, server_task) = server(move |_| {
        step += 1;
        (
            200,
            if step <= 12 {
                call(&format!("usage-{step}"), "get_token_usage", json!({}), "")
            } else {
                answer("done")
            },
        )
    })
    .await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, false, Some(128_000)).await;
    r.request(
        "PUT",
        "/api/workspace/memory/MEMORY.md",
        json!({"content":"INDEX_MARKER_FIRST"}),
    )
    .await
    .unwrap();
    assert_eq!(
        finish(start(&r, "thin-context", "first")).await["status"],
        "completed"
    );
    assert_eq!(
        finish(start(&r, "thin-context", "second")).await["status"],
        "completed"
    );
    r.request(
        "PUT",
        "/api/workspace/memory/MEMORY.md",
        json!({"content":"INDEX_MARKER_CHANGED"}),
    )
    .await
    .unwrap();
    assert_eq!(
        finish(start(&r, "thin-context", "third")).await["status"],
        "completed"
    );
    let all = requests.lock().unwrap();
    assert_eq!(all.len(), 15);
    for request in all.iter() {
        let messages = request["messages"].as_array().unwrap();
        assert!(!messages
            .iter()
            .any(|m| m.to_string().contains("runtime_context_notice")));
        let system = messages[0]["content"].as_str().unwrap();
        assert!(system.contains("Approval never silently changes session defaults."));
        assert!(system.contains("The model chooses note organization and retrieval."));
        assert!(!messages.iter().skip(1).any(|m| m
            .to_string()
            .contains("Approval never silently changes session defaults.")));
    }
    let final_request = all.last().unwrap().to_string();
    assert_eq!(final_request.matches("INDEX_MARKER_FIRST").count(), 1);
    assert_eq!(final_request.matches("INDEX_MARKER_CHANGED").count(), 1);
    drop(all);
    server_task.abort();
}

#[tokio::test]
async fn activity_completion_precedes_ordered_parallel_output_and_survives_history() {
    let root = tempfile::tempdir().unwrap();
    let one = root.path().join("one.txt");
    let two = root.path().join("two.txt");
    std::fs::write(&one, "one").unwrap();
    std::fs::write(&two, "two").unwrap();
    let mut step = 0;
    let (url, _, server) = server(move |_| {
        step += 1;
        (200, if step == 1 {
            calls(&[("first", "read_file", json!({"file_path":one})),
                    ("second", "read_file", json!({"file_path":two}))])
        } else { answer("done") })
    }).await;
    let r = Runtime::open(&root.path().join("runtime")).unwrap();
    configure(&r, &url, false, None).await;
    r.request("PUT", "/api/workspace/running-config", json!({"approval_level":"STRICT","max_parallel_reads":2})).await.unwrap();
    let mut rx = start(&r, "activity-timing", "Read both");
    let pending = pending_count(&r, "activity-timing", 2).await;
    let second = pending.iter().find(|p| p["tool_params"]["path"].as_str().unwrap().ends_with("two.txt")).unwrap();
    approve_id(&r, "activity-timing", &second["request_id"]).await;
    let timing = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(frame) = rx.recv().await {
            assert_ne!(frame["type"], "function_call_output", "ordered evidence must still wait for first call");
            if frame["type"] == "function_call" && frame["content"][0]["data"]["call_id"] == "second"
                && frame["metadata"]["activity"]["state"] == "completed" {
                return frame["metadata"]["activity"].clone();
            }
        }
        panic!("missing immediate completion");
    }).await.unwrap();
    assert!(timing["elapsed_ms"].is_u64());
    let first = pending.iter().find(|p| p["request_id"] != second["request_id"]).unwrap();
    approve_id(&r, "activity-timing", &first["request_id"]).await;
    assert_eq!(finish(rx).await["status"], "completed");
    let chats = r.request("GET", "/api/chats", Value::Null).await.unwrap();
    let list = chats.as_array().or_else(|| chats["chats"].as_array()).unwrap();
    let id = list.iter().find(|c| c["session_id"] == "activity-timing").unwrap()["id"].as_str().unwrap();
    let history = r.request("GET", &format!("/api/chats/{id}"), Value::Null).await.unwrap();
    let output = history["messages"].as_array().unwrap().iter().find(|m| m["type"] == "function_call_output" && m["content"][0]["data"]["call_id"] == "second").unwrap();
    assert_eq!(output["metadata"]["activity"], timing, "history must keep actual completion time, not ordered delivery time");
    server.abort();
}

#[tokio::test]
async fn response_commentary_and_final_keep_separate_live_and_saved_messages() {
    let root = tempfile::tempdir().unwrap();
    let (url, _, server) = server(|_| {
        let mut body = String::new();
        for (index, phase, text) in [(0, "commentary", "Checking the source."), (1, "final_answer", "The verified answer.")] {
            let item = json!({"id":format!("item-{index}"),"type":"message","role":"assistant","phase":phase,"content":[{"type":"output_text","text":text}]});
            body += &sse(json!({"type":"response.output_item.added","output_index":index,"item":item}));
            body += &sse(json!({"type":"response.output_text.delta","output_index":index,"delta":text}));
            body += &sse(json!({"type":"response.output_item.done","output_index":index,"item":item}));
        }
        (200, body + &sse(json!({"type":"response.completed","response":{}})))
    }).await;
    let r = Runtime::open(root.path()).unwrap();
    configure(&r, &url, true, None).await;
    let mut rx = start(&r, "phased-activity", "Check and answer");
    let events = tokio::time::timeout(Duration::from_secs(5), async {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            let done = event["object"] == "response" && event["status"] != "in_progress";
            events.push(event);
            if done { return events; }
        }
        panic!("missing completion");
    }).await.unwrap();
    assert_eq!(events.last().unwrap()["status"], "completed");
    let commentary = events.iter().find(|v| v["phase"] == "commentary").unwrap();
    let final_answer = events.iter().find(|v| v["phase"] == "final_answer").unwrap();
    assert_ne!(commentary["id"], final_answer["id"]);
    let chats = r.request("GET", "/api/chats", Value::Null).await.unwrap();
    let id = chats.as_array().unwrap().iter().find(|c| c["session_id"] == "phased-activity").unwrap()["id"].as_str().unwrap();
    let history = r.request("GET", &format!("/api/chats/{id}"), Value::Null).await.unwrap();
    for (phase, text) in [("commentary", "Checking the source."), ("final_answer", "The verified answer.")] {
        let rows: Vec<_> = history["messages"].as_array().unwrap().iter().filter(|v| v["phase"] == phase).collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["content"][0]["text"], text);
    }
    server.abort();
}
