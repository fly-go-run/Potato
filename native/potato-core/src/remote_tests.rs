use crate::{lock, Runtime, Run, Approval, replay::Replay};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[test]
fn display_messages_keeps_tool_call_structure() {
    use crate::protocol;
    let frames = [
        protocol::message("call", "function_call", "assistant", json!([
            protocol::data("call", json!({"call_id":"c1","name":"read_file","arguments":{"path":"a.md"}}))
        ]), "completed"),
        protocol::message("output", "function_call_output", "tool", json!([
            protocol::data("output", json!({"call_id":"c1","name":"read_file","output":"文".repeat(4001),"state":"success"}))
        ]), "completed"),
    ];
    let rows = crate::remote::display_messages(&frames);
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["call_id"], "c1");
    assert_eq!(rows[1]["call_id"], "c1");
    assert_eq!(rows[0]["name"], "read_file");
    assert!(rows[0]["arguments"].as_str().unwrap().contains("a.md"));
    assert!(rows[0]["text"].as_str().unwrap().starts_with("read_file\n"));
    assert_eq!(rows[1]["output"].as_str().unwrap().chars().count(), 4002);
    assert_eq!(rows[1]["output"], format!("{}\n…", "文".repeat(4000)));
    assert_eq!(rows[1]["state"], "success");
    assert_eq!(rows[1]["text"], "文".repeat(4001));
    let plain = protocol::message("answer", "message", "assistant", json!([protocol::text("answer", "正文", false)]), "completed");
    let rows = crate::remote::display_messages(&[plain]);
    assert!(rows[0].get("call_id").is_none());
}

fn command(op: &str, args: Value) -> Value { json!({"id":uuid::Uuid::new_v4().to_string(),"op":op,"args":args}) }
#[test]
fn display_messages_exposes_user_operation_for_queue_transition() {
    let mut user = crate::protocol::message("u", "message", "user", json!([crate::protocol::text("u", "任务", false)]), "completed");
    user["metadata"] = json!({"remote_operation_id":"queued-operation","private_field":"hidden"});
    let mut assistant = user.clone(); assistant["id"] = json!("a"); assistant["role"] = json!("assistant");
    let rows = crate::remote::display_messages(&[user, assistant]);
    assert_eq!(rows[0]["remote_operation_id"], "queued-operation");
    assert!(rows[0].get("metadata").is_none());
    assert!(rows[1].get("remote_operation_id").is_none());
}

fn running(core: &Runtime, session: &str) -> Value {
    let chat = core.db().unwrap().ensure_chat(session,"原来的桌面任务").unwrap();
    lock(&core.runs).unwrap().insert(session.into(),Run{request_id:"desktop-request".into(),accepting_steering:true,cancel:CancellationToken::new(),replay:Arc::new(Mutex::new(Replay::new("desktop-request".into(),Arc::new(|_|Ok(())))))});
    chat
}

#[tokio::test]
async fn remote_unfinished_receipt_recovers_tagged_steering_after_delivery_and_restart() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = running(&core,"recover-session");
    let request = command("send",json!({"chat_id":chat["id"],"text":"Only once","expected_run_id":"desktop-request"}));
    core.remote_command(&request).await.unwrap();
    let key = format!("remote_receipt:{}",request["id"].as_str().unwrap());
    let mut receipt = core.db().unwrap().get(&key,Value::Null).unwrap();
    receipt.as_object_mut().unwrap().remove("result");
    core.db().unwrap().put(&key,&receipt).unwrap();
    assert_eq!(core.remote_command(&request).await.unwrap()["delivery"],"recovered");
    core.db().unwrap().put(&key,&receipt).unwrap();
    core.deliver_steering(chat["id"].as_str().unwrap(),&(Arc::new(|_|Ok(())) as crate::Emit)).unwrap();
    drop(core);
    let core = Runtime::open(dir.path()).unwrap();
    for _ in 0..2 {
        let result = core.remote_command(&request).await.unwrap();
        assert_eq!(result["chat"]["id"],chat["id"]); assert_eq!(result["delivery"],"recovered");
    }
    let history = core.db().unwrap().history(chat["id"].as_str().unwrap(),false).unwrap();
    assert_eq!(history.len(),1);
    assert_eq!(history[0]["metadata"]["remote_operation_id"],request["id"]);
    assert_eq!(history[0]["metadata"]["steering_state"],"delivered");
    assert_eq!(core.db().unwrap().history(chat["id"].as_str().unwrap(),true).unwrap().len(),1);
    assert!(lock(&core.runs).unwrap().is_empty());
}

#[tokio::test]
async fn remote_unfinished_receipt_never_guesses_from_text_or_an_empty_chat() {
    use sha2::{Digest,Sha256};
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let request = command("send",json!({"text":"Same text"}));
    let id = request["id"].as_str().unwrap();
    let key = format!("remote_receipt:{id}");
    let fingerprint = format!("{:x}",Sha256::digest(json!({"op":request["op"],"args":request["args"]}).to_string().as_bytes()));
    let reservation = json!({"fingerprint":fingerprint});
    core.db().unwrap().put(&key,&reservation).unwrap();
    assert_eq!(core.remote_command(&request).await.unwrap_err().status,409);
    let chat = core.db().unwrap().ensure_chat(&format!("remote-{id}"),"Same text").unwrap();
    assert_eq!(core.remote_command(&request).await.unwrap_err().status,409);
    let frame = crate::protocol::message("unrelated","message","user",json!([crate::protocol::text("unrelated","Same text",false)]),"completed");
    core.db().unwrap().append(chat["id"].as_str().unwrap(),&frame,None).unwrap();
    assert_eq!(core.remote_command(&request).await.unwrap_err().status,409);
    assert_eq!(core.db().unwrap().get(&key,Value::Null).unwrap(),reservation);
    assert_eq!(core.db().unwrap().history(chat["id"].as_str().unwrap(),false).unwrap().len(),1);
    let mut changed = request.clone(); changed["args"]["text"] = json!("changed");
    assert_eq!(core.remote_command(&changed).await.unwrap_err().status,409);
    core.db().unwrap().delete_chat(chat["id"].as_str().unwrap()).unwrap();
    assert_eq!(core.remote_command(&request).await.unwrap_err().status,409);
    assert_eq!(core.db().unwrap().get(&key,Value::Null).unwrap(),reservation);
    assert!(lock(&core.runs).unwrap().is_empty());
}

#[tokio::test]
async fn remote_followup_retries_and_restart_never_duplicate_user_instructions() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = running(&core,"desktop-session");
    let request = command("send",json!({"chat_id":chat["id"],"text":"请先检查输入校验"}));
    let (a,b) = tokio::join!(core.remote_command(&request),core.remote_command(&request));
    assert_eq!(a.unwrap()["chat"],chat); assert_eq!(b.unwrap()["chat"],chat);
    let view = core.remote_command(&command("chat",json!({"chat_id":chat["id"]}))).await.unwrap();
    assert_eq!(view["status"],"running");
    assert_eq!(view["live"].as_array().unwrap().iter().filter(|v|v["text"] == "请先检查输入校验").count(),1);
    drop(core); let core = Runtime::open(dir.path()).unwrap();
    assert_eq!(core.remote_command(&request).await.unwrap()["chat"],chat);
    let history = core.db().unwrap().history(chat["id"].as_str().unwrap(),false).unwrap();
    assert_eq!(history.len(),1);
    let mut changed = request; changed["args"]["text"] = json!("另一个请求");
    assert_eq!(core.remote_command(&changed).await.unwrap_err().status,409);
}

#[tokio::test]
async fn remote_stop_cancels_the_existing_desktop_run_and_pin_is_persistent() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap(); let chat = running(&core,"same-session");
    let cancel = lock(&core.runs).unwrap()["same-session"].cancel.clone();
    core.remote_command(&command("stop",json!({"chat_id":chat["id"],"expected_run_id":"desktop-request"}))).await.unwrap();
    assert!(cancel.is_cancelled());
    core.remote_command(&command("pin",json!({"chat_id":chat["id"],"pinned":true}))).await.unwrap();
    assert_eq!(core.db().unwrap().chat(chat["id"].as_str().unwrap()).unwrap()["pinned"],true);
    let invalid = command("pin",json!({"chat_id":chat["id"],"pinned":"yes"}));
    assert_eq!(core.remote_command(&invalid).await.unwrap_err().status,400);
    // Repeating a rejected command returns the same rejection, not a stuck reservation.
    assert_eq!(core.remote_command(&invalid).await.unwrap_err().status,400);
}

#[tokio::test]
async fn remote_stop_is_bound_to_the_confirmed_run_and_never_pauses_a_replacement() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = running(&core,"stop-session");
    core.outbox_request(&json!({"session_id":"stop-session","action":"add","id":"queued-followup","request":{"session_id":"stop-session","input":[{"role":"user","content":[{"type":"text","text":"A saved follow-up"}]}]}})).unwrap();
    let snapshot = core.remote_command(&command("chat",json!({"chat_id":chat["id"]}))).await.unwrap();
    assert_eq!(snapshot["stop_protocol"],1);
    let request = command("stop",json!({"chat_id":chat["id"],"expected_run_id":snapshot["running_request_id"]}));
    let replacement = CancellationToken::new();
    { let mut runs = lock(&core.runs).unwrap(); let run = runs.get_mut("stop-session").unwrap(); run.request_id = "replacement".into(); run.cancel = replacement.clone(); }
    let before = core.db().unwrap().get("follow_up_outbox",Value::Null).unwrap();
    assert_eq!(before["stop-session"]["paused"],false);
    assert_eq!(before["stop-session"]["items"].as_array().unwrap().len(),1);
    for _ in 0..2 { assert_eq!(core.remote_command(&request).await.unwrap_err().status,412); }
    assert!(!replacement.is_cancelled());
    assert_eq!(core.db().unwrap().get("follow_up_outbox",Value::Null).unwrap(),before);
    for invalid in [Value::Null,json!(""),json!(42)] {
        assert_eq!(core.remote_command(&command("stop",json!({"chat_id":chat["id"],"expected_run_id":invalid}))).await.unwrap_err().status,422);
    }
    assert_eq!(core.remote_command(&command("stop",json!({"chat_id":chat["id"]}))).await.unwrap_err().status,422);
    assert!(!replacement.is_cancelled());
    let valid = command("stop",json!({"chat_id":chat["id"],"expected_run_id":"replacement"}));
    assert_eq!(core.remote_command(&valid).await.unwrap()["stopped"],true);
    assert!(replacement.is_cancelled());
    assert_eq!(core.db().unwrap().get("follow_up_outbox",Value::Null).unwrap()["stop-session"]["paused"],true);
    assert_eq!(core.db().unwrap().get("follow_up_outbox",Value::Null).unwrap()["stop-session"]["items"],before["stop-session"]["items"]);
    // A lost response retries the original receipt, even after another run starts.
    let next = CancellationToken::new();
    { let mut runs = lock(&core.runs).unwrap(); let run = runs.get_mut("stop-session").unwrap(); run.request_id = "next".into(); run.cancel = next.clone(); }
    assert_eq!(core.remote_command(&valid).await.unwrap()["stopped"],true);
    assert!(!next.is_cancelled());
    lock(&core.runs).unwrap().remove("stop-session");
    assert_eq!(core.remote_command(&command("stop",json!({"chat_id":chat["id"],"expected_run_id":"next"}))).await.unwrap_err().status,412);
    // Desktop's existing explicit stop also pauses an idle queue.
    assert_eq!(core.request("POST",&format!("/api/console/chat/stop?chat_id={}",chat["id"].as_str().unwrap()),Value::Null).await.unwrap()["stopped"],false);
}

#[tokio::test]
async fn remote_cannot_answer_or_approve_another_conversation() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = core.db().unwrap().ensure_chat("own","Own").unwrap();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    lock(&core.approvals).unwrap().insert("approval".into(),Approval{view:json!({"request_id":"approval","root_session_id":"other","user_id":"default"}),reply:tx});
    let request = command("approval",json!({"chat_id":chat["id"],"request_id":"approval","allow":true}));
    assert_eq!(core.remote_command(&request).await.unwrap_err().status,403);
    assert!(lock(&core.approvals).unwrap().contains_key("approval"));
    core.db().unwrap().save_question(&json!({"request_id":"question","session_id":"other","title":"目标？","status":"pending","options":[],"multiple":false})).unwrap();
    assert_eq!(core.remote_command(&command("answer",json!({"chat_id":chat["id"],"request_id":"question","selected":[],"text":"yes","skip":false}))).await.unwrap_err().status,403);
    assert_eq!(core.db().unwrap().question("question").unwrap()["status"],"pending");
    assert_eq!(core.remote_command(&command("/api/models",json!({}))).await.unwrap_err().status,403);
}

#[tokio::test]
async fn remote_snapshot_contains_actual_approval_details_and_answers_question() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap(); let chat = running(&core,"s");
    let (tx,_rx) = tokio::sync::oneshot::channel();
    let approval = json!({"request_id":"approval","root_session_id":"s","user_id":"default","tool_name":"execute_shell_command","findings_summary":"需要授权","exact_target":"项目目录","action_detail":"{\"command\":\"ls\"}"});
    lock(&core.approvals).unwrap().insert("approval".into(),Approval{view:approval.clone(),reply:tx});
    let (tx,rx) = tokio::sync::oneshot::channel(); lock(&core.questions).unwrap().insert("q".into(),tx);
    core.db().unwrap().save_question(&json!({"request_id":"q","session_id":"s","title":"检查范围？","status":"pending","options":[{"id":"all","label":"全部"}],"multiple":false})).unwrap();
    let view = core.remote_command(&command("chat",json!({"chat_id":chat["id"]}))).await.unwrap();
    assert_eq!(view["approvals"][0],approval);
    core.remote_command(&command("answer",json!({"chat_id":chat["id"],"request_id":"q","selected":["all"],"text":"","skip":false}))).await.unwrap();
    assert_eq!(rx.await.unwrap()["answer"]["selected"],json!(["all"]));
}

#[tokio::test]
async fn native_login_persists_sealed_credentials_and_requires_explicit_remote_enable() {
    check_native_login_logout(200).await;
}

#[tokio::test]
async fn expired_remote_login_can_be_cleared_for_signing_in_again() {
    check_native_login_logout(401).await;
}

async fn check_native_login_logout(logout_status: u16) {
    use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay = format!("http://{}/",listener.local_addr().unwrap());
    let server_relay = relay.clone();
    let received = Arc::new(Mutex::new(Vec::new())); let output = received.clone();
    let id = uuid::Uuid::new_v4().to_string(); let server_id = id.clone();
    let server = tokio::spawn(async move {
        while let Ok((mut socket,_)) = listener.accept().await {
            let mut bytes = Vec::new(); let mut chunk=[0;4096];
            let (headers,body) = loop {
                let n=socket.read(&mut chunk).await.unwrap(); if n==0 {return;} bytes.extend_from_slice(&chunk[..n]);
                if let Some(end)=bytes.windows(4).position(|p|p==b"\r\n\r\n") {
                    let headers=String::from_utf8_lossy(&bytes[..end]).to_string();
                    let len=headers.lines().find_map(|l|l.to_lowercase().strip_prefix("content-length: ").and_then(|v|v.parse::<usize>().ok())).unwrap_or(0);
                    if bytes.len()>=end+4+len {break (headers,serde_json::from_slice::<Value>(&bytes[end+4..end+4+len]).unwrap());}
                }
            };
            let path=headers.lines().next().unwrap().split_whitespace().nth(1).unwrap();
            let response=match path {
                "/v1/remote/auth/start"=> {assert_eq!(body["role"],"host"); json!({"id":server_id,"verification_url":format!("{server_relay}v1/remote/auth/authorize?id={server_id}"),"code":"ABC12345","expires":chrono::Utc::now().timestamp_millis()+300000})},
                "/v1/remote/auth/poll"=> json!({"status":"authorized","owner":"a".repeat(64),"email":"fixture@example.test"}),
                "/v1/remote/account/register"=> {assert_eq!(body["host_token"].as_str().unwrap().len(),64);json!({"id":server_id,"name":"Fixture Mac"})},
                "/v1/remote/account/logout"=> json!({"ok":true}),
                _=>panic!("Unexpected route {path}"),
            };
            output.lock().unwrap().push((path.to_owned(),body));
            let status = if path == "/v1/remote/account/logout" { logout_status } else { 200 };
            let text=response.to_string();let wire=format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len());
            socket.write_all(wire.as_bytes()).await.unwrap();
        }
    });
    let dir=tempfile::tempdir().unwrap();let core=Runtime::open(dir.path()).unwrap();
    let started=core.begin_remote_login(json!({"relay":relay,"name":"Fixture Mac"})).await.unwrap();
    assert_eq!(started["login"]["code"],"ABC12345");assert!(started["login"]["client_token"].is_null());
    let state=core.poll_remote_login().await.unwrap();assert_eq!(state["auth_mode"],"account");assert_eq!(state["enabled"],false);assert_eq!(state["email"],"fixture@example.test");
    assert!(state["session_token"].is_null());assert!(state["host_token"].is_null());
    let stored=core.db().unwrap().get("remote_config",Value::Null).unwrap();
    let raw=core.db().unwrap().unseal(stored["session_token"].as_str().unwrap()).unwrap();
    assert!(raw.starts_with(&format!("{}.{}.","a".repeat(64),id)));assert_ne!(stored["session_token"],raw);
    assert_eq!(core.configure_remote(json!({"enabled":true})).await.unwrap()["enabled"],true);
    core.logout_remote().await.unwrap();assert_eq!(core.remote_settings().unwrap()["enabled"],false);
    assert!(core.db().unwrap().get("remote_config",Value::Null).unwrap()["host_token"].is_null());
    assert_eq!(received.lock().unwrap().len(),4);server.abort();
}

#[tokio::test]
async fn remote_model_changes_cannot_steer_running_work_and_steering_binds_exact_run() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = running(&core,"model-session");
    let args = json!({"chat_id":chat["id"],"text":"Synthetic model choice","model_choice":{"provider_id":"p","model":"m","reasoning_effort":null}});
    let request = command("send",args);
    assert_eq!(core.remote_command(&request).await.unwrap_err().status,412);
    let changed = command("send",json!({"chat_id":chat["id"],"text":"Stale steering","expected_run_id":"old-run"}));
    assert_eq!(core.remote_command(&changed).await.unwrap_err().status,412);
    assert!(core.db().unwrap().history(chat["id"].as_str().unwrap(),false).unwrap().is_empty());
    let snapshot = core.remote_command(&command("chat",json!({"chat_id":chat["id"]}))).await.unwrap();
    assert_eq!(snapshot["running_request_id"],"desktop-request");
    let accepted = command("send",json!({"chat_id":chat["id"],"text":"Correct steering","expected_run_id":"desktop-request"}));
    core.remote_command(&accepted).await.unwrap();
    lock(&core.runs).unwrap().remove("model-session");
    assert!(core.remote_command(&accepted).await.is_ok(),"The receipt is stable even after completion");
    let ended = command("send",json!({"chat_id":chat["id"],"text":"After completion","expected_run_id":"desktop-request"}));
    assert_eq!(core.remote_command(&ended).await.unwrap_err().status,412);
    assert_eq!(core.db().unwrap().history(chat["id"].as_str().unwrap(),false).unwrap().len(),1);
}

#[tokio::test]
async fn remote_directory_approval_uses_declared_scope_and_directory() {
    let dir = tempfile::tempdir().unwrap();
    let core = Runtime::open(dir.path()).unwrap();
    let grant_dir = tempfile::tempdir().unwrap();
    let chat = core.db().unwrap().ensure_chat("scope", "Scope").unwrap();
    for scope in ["session_directory", "persistent_directory"] {
        let (tx, rx) = tokio::sync::oneshot::channel();
        lock(&core.approvals).unwrap().insert("scope".into(), Approval { view: json!({
            "request_id":"scope", "root_session_id":"scope", "user_id":"default",
            "created_at":chrono::Utc::now().timestamp(), "allow_directory":true,
            "suggested_directory":grant_dir.path(), "exact_target":grant_dir.path().canonicalize().unwrap(), "tool_name":"list_dir", "directory_recursive":false
        }), reply:tx });
        core.remote_command(&command("approval", json!({"chat_id":chat["id"],"request_id":"scope","allow":true,"scope":scope,"directory":"/"}))).await.unwrap();
        match rx.await.unwrap() {
            crate::approval::Reply::Directory(rule) => {
                assert_eq!(rule.path, grant_dir.path().canonicalize().unwrap());
                assert!(!rule.recursive);
                assert_eq!(rule.session_id.as_deref(), if scope == "session_directory" {Some("scope")} else {None});
            }
            _ => panic!("expected directory grant"),
        }
    }
    let (tx, _rx) = tokio::sync::oneshot::channel();
    lock(&core.approvals).unwrap().insert("once".into(), Approval {view:json!({"root_session_id":"scope","user_id":"default","allow_directory":false}),reply:tx});
    assert_eq!(core.remote_command(&command("approval",json!({"chat_id":chat["id"],"request_id":"once","allow":true,"scope":"persistent_directory"}))).await.unwrap_err().status,400);
    assert!(lock(&core.approvals).unwrap().contains_key("once"));
}

#[tokio::test]
async fn remote_outbox_queues_in_order_without_steering_and_recovers_receipts() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = running(&core,"phone-queue");
    let mut requests = Vec::new();
    for text in ["第一条", "第二条", "第三条"] {
        let request = command("send",json!({"chat_id":chat["id"],"text":text,"delivery_mode":"queue"}));
        assert_eq!(core.remote_command(&request).await.unwrap()["delivery"],"queued");
        core.remote_command(&request).await.unwrap();
        requests.push(request);
    }
    let snapshot = core.remote_command(&command("chat",json!({"chat_id":chat["id"]}))).await.unwrap();
    assert_eq!(snapshot["outbox_protocol"],1);
    assert_eq!(snapshot["outbox"]["items"].as_array().unwrap().iter().map(|v|v["text"].as_str().unwrap()).collect::<Vec<_>>(),vec!["第一条","第二条","第三条"]);
    assert!(snapshot["outbox"]["items"][0].get("request").is_none());
    assert!(core.db().unwrap().history(chat["id"].as_str().unwrap(),false).unwrap().is_empty());
    assert!(!lock(&core.runs).unwrap()["phone-queue"].cancel.is_cancelled());
    let key = format!("remote_receipt:{}",requests[0]["id"].as_str().unwrap());
    let mut receipt = core.db().unwrap().get(&key,Value::Null).unwrap();
    receipt.as_object_mut().unwrap().remove("result");
    core.db().unwrap().put(&key,&receipt).unwrap();
    assert_eq!(core.remote_command(&requests[0]).await.unwrap()["delivery"],"recovered");
    assert_eq!(core.outbox_request(&json!({"session_id":"phone-queue"})).unwrap()["items"].as_array().unwrap().len(),3);
}

#[tokio::test]
async fn remote_outbox_interrupt_is_atomic_and_preserves_other_messages() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = running(&core,"phone-interrupt");
    core.remote_command(&command("send",json!({"chat_id":chat["id"],"text":"later","delivery_mode":"queue"}))).await.unwrap();
    let before = core.db().unwrap().get("follow_up_outbox",Value::Null).unwrap();
    for expected in [json!("old-run"), Value::Null] {
        assert_eq!(core.remote_command(&command("send",json!({"chat_id":chat["id"],"text":"urgent","delivery_mode":"interrupt","expected_run_id":expected}))).await.unwrap_err().status,412);
        assert_eq!(core.db().unwrap().get("follow_up_outbox",Value::Null).unwrap(),before);
        assert!(!lock(&core.runs).unwrap()["phone-interrupt"].cancel.is_cancelled());
    }
    let urgent = command("send",json!({"chat_id":chat["id"],"text":"urgent","delivery_mode":"interrupt","expected_run_id":"desktop-request"}));
    core.remote_command(&urgent).await.unwrap();
    core.remote_command(&urgent).await.unwrap();
    let q = core.outbox_request(&json!({"session_id":"phone-interrupt"})).unwrap();
    assert_eq!(q["items"].as_array().unwrap().len(),2);
    assert_eq!(q["items"][0]["id"],urgent["id"]);
    assert_eq!(q["items"][1],before["phone-interrupt"]["items"][0]);
    assert!(lock(&core.runs).unwrap()["phone-interrupt"].cancel.is_cancelled());
    assert_eq!(lock(&core.runs).unwrap().len(),1);
}

#[tokio::test]
async fn remote_outbox_management_is_scoped_and_edit_preserves_position() {
    let dir = tempfile::tempdir().unwrap(); let core = Runtime::open(dir.path()).unwrap();
    let chat = running(&core,"phone-manage"); let other = running(&core,"other-chat");
    let queued = command("send",json!({"chat_id":chat["id"],"text":"before","delivery_mode":"queue"}));
    core.remote_command(&queued).await.unwrap();
    assert_eq!(core.remote_command(&command("outbox",json!({"chat_id":other["id"],"action":"delete","item_id":queued["id"]}))).await.unwrap_err().status,404);
    let edited = core.remote_command(&command("outbox",json!({"chat_id":chat["id"],"action":"save","item_id":queued["id"],"text":"after"}))).await.unwrap();
    assert_eq!(edited["items"][0]["text"],"after");
    assert_eq!(core.remote_command(&command("outbox",json!({"chat_id":chat["id"],"action":"promote","item_id":queued["id"],"expected_run_id":"old-run"}))).await.unwrap_err().status,412);
    let paused = core.remote_command(&command("outbox",json!({"chat_id":chat["id"],"action":"pause"}))).await.unwrap();
    assert_eq!(paused["paused"],true);
    let resumed = core.remote_command(&command("outbox",json!({"chat_id":chat["id"],"action":"resume"}))).await.unwrap();
    assert_eq!(resumed["paused"],false);
    let deleted = core.remote_command(&command("outbox",json!({"chat_id":chat["id"],"action":"delete","item_id":queued["id"]}))).await.unwrap();
    assert!(deleted["items"].as_array().unwrap().is_empty());
}
