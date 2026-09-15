//! Outbound-only remote control. The relay cannot select arbitrary local API paths.
use crate::{lock, required, string, Error, Result, Runtime};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

#[derive(Default)]
pub(crate) struct State { pub status: String, pub generation: u64, pub login_generation: u64 }

fn relay_url(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| Error::new(400, "请输入 HTTPS 服务地址"))?;
    let local = cfg!(debug_assertions) && url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost"));
    if (url.scheme() != "https" && !local) || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() || url.path() != "/" {
        return Err(Error::new(400, "服务地址必须为 HTTPS 根地址"));
    }
    Ok(url)
}
fn segment(s: &str) -> String { percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string() }

pub(crate) fn display_messages(frames: &[Value]) -> Value {
    let mut rows: Vec<Value> = Vec::new();
    for frame in frames {
        if frame["object"] == "message" || frame["role"].is_string() {
            let id = string(frame,"id");
            let text = if let Some(s) = frame["content"].as_str() {s.to_owned()} else {frame["content"].as_array().map(|a|a.iter().filter_map(|block| {
                if let Some(text) = block["text"].as_str() {return Some(text.to_owned());}
                let data = &block["data"];
                if let Some(output) = data["output"].as_str() {return Some(output.to_owned());}
                data["name"].as_str().map(|name| format!("{}\n{}",name,data["arguments"].as_str().map(str::to_owned).unwrap_or_else(||data["arguments"].to_string())))
            }).collect::<Vec<_>>().join("\n")).unwrap_or_default()};
            let mut row = json!({"id":id,"role":frame["role"],"kind":frame["type"],"text":text,"status":frame["metadata"]["activity"]["state"].as_str().map(|s|json!(s)).unwrap_or_else(||frame["status"].clone())});
            if matches!(frame["type"].as_str(), Some("function_call" | "function_call_output")) {
                if let Some(data) = frame["content"].as_array().and_then(|blocks| blocks.iter().map(|block| &block["data"]).find(|data| data.get("call_id").is_some())) {
                    let fields: &[&str] = if frame["type"] == "function_call" { &["call_id", "name", "arguments"] } else { &["call_id", "name", "output", "state"] };
                    for &field in fields {
                        if let Some(value) = data.get(field) {
                            if matches!(field, "arguments" | "output") {
                                let value = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string());
                                row[field] = json!(if value.chars().count() > 4000 { format!("{}\n…", value.chars().take(4000).collect::<String>()) } else { value });
                            } else if let Some(value) = value.as_str() {
                                row[field] = json!(value);
                            }
                        }
                    }
                }
            }
            if let Some(i) = rows.iter().position(|r|r["id"] == id) { if !text.is_empty() {rows[i] = row;} else {rows[i]["status"] = frame["status"].clone();} } else {rows.push(row);}
        } else if frame["object"] == "content" && frame["type"] == "text" {
            if let Some(row) = rows.iter_mut().find(|r|r["id"] == frame["msg_id"]) {
                let text = if frame["delta"] == true {format!("{}{}",string(row,"text"),string(frame,"text"))} else {string(frame,"text").to_owned()};
                row["text"] = json!(text);
            }
        }
    }
    let start = rows.len().saturating_sub(120);
    let mut rows = rows.split_off(start);
    if start > 0 {rows.insert(0,json!({"id":"remote-history-notice","role":"system","kind":"notice","text":"这里显示最近 120 条消息，更早内容可在电脑查看。"}));}
    for row in &mut rows { let text = string(row,"text"); if text.chars().count() > 8000 { row["text"] = json!(format!("{}\n…内容较长，请在电脑查看全文",text.chars().take(8000).collect::<String>())); } }
    json!(rows)
}

impl Runtime {
    pub(crate) fn remote_settings(&self) -> Result<Value> {
        let config = self.db()?.get("remote_config", json!({}))?;
        let pending = self.db()?.get("remote_login",Value::Null)?;
        let login = if pending.is_object() { json!({"verification_url":pending["verification_url"],"code":pending["code"]}) } else {Value::Null};
        Ok(json!({"enabled":config["enabled"] == true,"relay":config["relay"],"name":config["name"],"id":config["id"],"email":config["email"],"auth_mode":config["auth_mode"],"login":login,"status":lock(&self.remote)?.status}))
    }
    pub(crate) async fn configure_remote(&self, body: Value) -> Result<Value> {
        if body["enabled"] == false {
            let mut state = lock(&self.remote)?;
            let mut config = self.db()?.get("remote_config", json!({}))?; config["enabled"] = json!(false);
            self.db()?.put("remote_config", &config)?;
            state.generation += 1; state.status = "远程连接已关闭".into();
            drop(state); return self.remote_settings();
        }
        let old = self.db()?.get("remote_config", json!({}))?;
        if old["auth_mode"] == "account" {
            let mut state = lock(&self.remote)?;
            let mut config = old; config["enabled"] = json!(true);
            self.db()?.put("remote_config", &config)?; state.generation += 1; state.status = "正在连接".into();
            drop(state); return self.remote_settings();
        }
        let relay = relay_url(body["relay"].as_str().unwrap_or(string(&old, "relay")))?;
        let name = body["name"].as_str().unwrap_or("我的电脑").trim();
        if name.is_empty() || name.len() > 240 { return Err(Error::new(400, "请输入电脑名称")); }
        // A disable or newer setup wins over an older network response.
        let generation = { let mut state = lock(&self.remote)?; state.generation += 1; state.generation };
        let (id, token, pair) = if old["relay"] == relay.as_str() && old["id"].as_str().is_some() {
            let token = self.db()?.unseal(required(&old, "host_token")?)?;
            let reply = self.remote_post(relay.join(&format!("v1/remote/{}/pairing", required(&old,"id")?)).unwrap(), &token, json!({})).await?;
            (required(&old,"id")?.to_owned(), token, required(&reply,"pair_token")?.to_owned())
        } else {
            let reply = self.remote_post(relay.join("v1/remote/register").unwrap(), required(&body,"service_token")?, json!({"name":name})).await?;
            (required(&reply,"id")?.to_owned(), required(&reply,"host_token")?.to_owned(), required(&reply,"pair_token")?.to_owned())
        };
        uuid::Uuid::parse_str(&id).map_err(|_|Error::new(502,"远程服务返回了无效设备编号"))?;
        if token.len() != 64 || pair.len() != 64 || !token.bytes().chain(pair.bytes()).all(|b|b.is_ascii_hexdigit()) {return Err(Error::new(502,"远程服务返回了无效配对凭据"));}
        let sealed = self.db()?.seal(&token)?;
        {
            let mut state = lock(&self.remote)?;
            if state.generation != generation { return Err(Error::new(409,"远程设置已改变，本次连接设置已取消")); }
            self.db()?.put("remote_config", &json!({"enabled":true,"relay":relay.as_str(),"name":name,"id":id,"host_token":sealed}))?;
            state.status = "正在连接".into();
        }
        let mut result = self.remote_settings()?;
        result["pairing_code"] = json!(format!("{}v1/remote/{}/pair#{}", relay, id, pair));
        Ok(result)
    }
    pub(crate) fn cancel_remote_login(&self) -> Result<Value> {
        let mut state = lock(&self.remote)?; state.login_generation += 1;
        self.db()?.put("remote_login",&Value::Null)?; drop(state); self.remote_settings()
    }
    pub(crate) async fn begin_remote_login(&self, body: Value) -> Result<Value> {
        let relay = relay_url(required(&body,"relay")?)?;
        let name = required(&body,"name")?;
        if name.len() > 80 { return Err(Error::new(400,"电脑名称过长")); }
        let generation = { let mut state = lock(&self.remote)?; state.login_generation += 1; state.login_generation };
        let token = format!("{}{}",uuid::Uuid::new_v4().simple(),uuid::Uuid::new_v4().simple());
        let mut login = self.remote_post(relay.join("v1/remote/auth/start").unwrap(),"",json!({"role":"host","name":name,"client_token":token})).await?;
        let id = required(&login,"id")?;
        uuid::Uuid::parse_str(id).map_err(|_|Error::new(502,"登录编号无效"))?;
        let url = reqwest::Url::parse(required(&login,"verification_url")?).map_err(|_|Error::new(502,"登录地址无效"))?;
        if url.origin() != relay.origin() || url.path() != "/v1/remote/auth/authorize" || url.fragment().is_some() || url.username() != "" || url.password().is_some() || url.query_pairs().find(|(k,_)|k=="id").map(|(_,v)|v.into_owned()).as_deref() != Some(id) {return Err(Error::new(502,"登录地址不属于当前服务"));}
        login["client_token"] = json!(self.db()?.seal(&token)?);
        login["host_token"] = json!(self.db()?.seal(&format!("{}{}",uuid::Uuid::new_v4().simple(),uuid::Uuid::new_v4().simple()))?);
        login["relay"] = json!(relay.as_str()); login["name"] = json!(name);
        { let state = lock(&self.remote)?; if state.login_generation != generation {return Err(Error::new(409,"登录已取消"));} self.db()?.put("remote_login",&login)?; }
        self.remote_settings()
    }
    pub(crate) async fn poll_remote_login(&self) -> Result<Value> {
        let (login,generation) = {let state = lock(&self.remote)?; (self.db()?.get("remote_login",Value::Null)?,state.login_generation)};
        if login.is_null() {return self.remote_settings();}
        if login["expires"].as_i64().unwrap_or(0) < chrono::Utc::now().timestamp_millis() {self.cancel_remote_login()?; return Err(Error::new(401,"登录已过期，请重新开始"));}
        let relay = relay_url(required(&login,"relay")?)?;
        let token = self.db()?.unseal(required(&login,"client_token")?)?;
        let reply = self.remote_post(relay.join("v1/remote/auth/poll").unwrap(),"",json!({"id":login["id"],"client_token":token})).await?;
        if reply["status"] == "pending" {return self.remote_settings();}
        let owner = required(&reply,"owner")?;
        if reply["status"] != "authorized" || owner.len() != 64 || !owner.bytes().all(|b|b.is_ascii_hexdigit()) {return Err(Error::new(502,"登录服务返回无效身份"));}
        let session = format!("{}.{}.{}",owner,required(&login,"id")?,token);
        let host_token = self.db()?.unseal(required(&login,"host_token")?)?;
        let device = self.remote_post(relay.join("v1/remote/account/register").unwrap(),&session,json!({"host_token":host_token})).await?;
        if device["id"] != login["id"] { return Err(Error::new(502,"登录服务返回无效电脑编号")); }
        let mut state = lock(&self.remote)?;
        if state.login_generation != generation {return Err(Error::new(409,"登录已取消"));}
        let sealed_session = self.db()?.seal(&session)?;
        self.db()?.put("remote_config",&json!({"enabled":false,"auth_mode":"account","relay":relay.as_str(),"name":device["name"],"id":device["id"],"email":reply["email"],"session_token":sealed_session,"host_token":login["host_token"]}))?;
        self.db()?.put("remote_login",&Value::Null)?; state.generation += 1; state.status = "已登录，开启远程访问后手机即可连接".into();
        drop(state); self.remote_settings()
    }
    pub(crate) async fn logout_remote(&self) -> Result<Value> {
        self.configure_remote(json!({"enabled":false})).await?;
        let config = self.db()?.get("remote_config",json!({}))?;
        if config["auth_mode"] == "account" {
            let relay = relay_url(required(&config,"relay")?)?;
            let token = self.db()?.unseal(required(&config,"session_token")?)?;
            // An expired or already-revoked session must still be removable
            // locally, so the user can sign in again. Network failures retain it.
            if let Err(error) = self.remote_post(relay.join("v1/remote/account/logout").unwrap(),&token,json!({})).await {
                if error.status != 401 { return Err(error); }
            }
        }
        self.db()?.put("remote_config",&json!({"relay":config["relay"],"name":config["name"]}))?;
        self.cancel_remote_login()
    }
    async fn remote_post(&self, url: reqwest::Url, token: &str, body: Value) -> Result<Value> {
        let response = self.client.post(url).bearer_auth(token).timeout(Duration::from_secs(15)).json(&body).send().await?;
        let status = response.status();
        if !status.is_success() { return Err(Error::new(status.as_u16(), "远程服务拒绝请求，请检查登录状态、服务地址和配置")); }
        if response.content_length().is_some_and(|n|n > 32768) { return Err(Error::new(502,"远程响应过大")); }
        let mut bytes = Vec::new(); let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await { bytes.extend_from_slice(&chunk?); if bytes.len() > 32768 { return Err(Error::new(502,"远程响应过大")); } }
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub async fn serve_remote(self: Arc<Self>) {
        loop {
            let _ = self.remote_connection().await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    async fn remote_connection(self: &Arc<Self>) -> Result<()> {
        let (config, generation) = {
            let state = lock(&self.remote)?;
            (self.db()?.get("remote_config", json!({}))?, state.generation)
        };
        if config["enabled"] != true { return Ok(()); }
        let mut url = relay_url(required(&config,"relay")?)?.join(&format!("v1/remote/{}/connect", required(&config,"id")?)).unwrap();
        let scheme = if url.scheme() == "https" {"wss"} else {"ws"}; url.set_scheme(scheme).unwrap();
        let token = self.db()?.unseal(required(&config,"host_token")?)?;
        let mut request = url.as_str().into_client_request().map_err(|_|Error::new(400,"无效的连接地址"))?;
        request.headers_mut().insert("Authorization", format!("Bearer {token}").parse().map_err(|_|Error::new(400,"无效令牌"))?);
        lock(&self.remote)?.status = "正在连接".into();
        let connection = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(request)).await;
        let (mut socket, _) = match connection { Ok(Ok(v)) => v, _ => { let mut state = lock(&self.remote)?; if state.generation == generation { state.status = "连接中断，正在重试".into(); } return Ok(()); } };
        if lock(&self.remote)?.generation != generation { let _ = socket.close(None).await; return Ok(()); }
        lock(&self.remote)?.status = "已连接，等待手机操作".into();
        let mut tick = tokio::time::interval(Duration::from_secs(1)); let mut ticks = 0;
        let mut last_received = std::time::Instant::now();
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    if lock(&self.remote)?.generation != generation { let _ = socket.close(None).await; break; }
                    if last_received.elapsed() > Duration::from_secs(60) { let _ = socket.close(None).await; break; }
                    ticks += 1;
                    if ticks % 20 == 0 && socket.send(Message::Text("ping".into())).await.is_err() { break; }
                }
                message = socket.next() => {
                    last_received = std::time::Instant::now();
                    match message {
                        Some(Ok(Message::Text(text))) if text == "pong" => {},
                        Some(Ok(Message::Text(text))) if text.len() <= 131072 => {
                            let Ok(command) = serde_json::from_str::<Value>(&text) else { continue; };
                            if lock(&self.remote)?.generation != generation { break; }
                            let result = self.remote_command(&command).await;
                            let mut reply = json!({"transport_id":command["transport_id"]});
                            match result { Ok(v) => reply["result"] = v, Err(e) => { reply["error"] = json!(e.message); reply["status"] = json!(e.status); } }
                            let wire = reply.to_string();
                            let wire = if wire.len() > 1_800_000 { json!({"transport_id":command["transport_id"],"error":"会话内容过大，请在电脑查看","status":413}).to_string() } else {wire};
                            if socket.send(Message::Text(wire.into())).await.is_err() { break; }
                        },
                        Some(Ok(Message::Ping(data))) => { if socket.send(Message::Pong(data)).await.is_err() { break; } },
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                        _ => {},
                    }
                }
            }
        }
        if lock(&self.remote)?.generation == generation { lock(&self.remote)?.status = "连接中断，正在重试".into(); }
        Ok(())
    }

    pub(crate) async fn remote_command(self: &Arc<Self>, command: &Value) -> Result<Value> {
        let op = required(command,"op")?;
        let args = &command["args"];
        if op == "overview" {
            let chats = self.request("GET","/api/chats",Value::Null).await?;
            let mut projects = self.request("GET","/api/workspace/coding-project/list",Value::Null).await?;
            let current = self.request("GET","/api/workspace/coding-project",Value::Null).await?;
            if let Some(list) = projects.as_array_mut() { if !list.iter().any(|p| p["path"] == current["path"]) { list.insert(0,current); } }
            return Ok(json!({"chats":chats,"projects":projects,"model_catalog":self.remote_model_catalog()?}));
        }
        if op == "chat" {
            let chat = self.db()?.chat(required(args,"chat_id")?)?;
            let session = required(&chat,"session_id")?;
            let mut value = self.request("GET", &format!("/api/chats/{}",segment(required(&chat,"id")?)),Value::Null).await?;
            value["messages"] = display_messages(value["messages"].as_array().map(Vec::as_slice).unwrap_or_default());
            value["chat"] = chat.clone();
            value["outcome"] = self.db()?.get(&format!("run_outcome:{session}"),Value::Null)?;
            value["approval_scope_protocol"] = json!(1);
                value["approvals"] = self.request("GET", &format!("/api/approval/list?session_id={}",segment(session)),Value::Null).await?["pending_approvals"].clone();
            value["questions"] = self.request("GET", &format!("/api/questions?session_id={}",segment(session)),Value::Null).await?["questions"].clone();
            let runs = lock(&self.runs)?;
            value["live"] = if let Some(run) = runs.get(session) { lock(&run.replay)?.remote_snapshot() } else {json!([])};
            value["running_request_id"] = runs.get(session).map(|run|json!(run.request_id)).unwrap_or(Value::Null);
            value["stop_protocol"] = json!(1);
            return Ok(value);
        }
        if !matches!(op,"send"|"stop"|"approval"|"answer"|"pin") { return Err(Error::new(403,"不支持的远程操作")); }
        let id = required(command,"id")?;
        uuid::Uuid::parse_str(id).map_err(|_|Error::new(400,"无效的操作编号"))?;
        let fingerprint = format!("{:x}",Sha256::digest(json!({"op":op,"args":args}).to_string().as_bytes()));
        let key = format!("remote_receipt:{id}");
        {
            let db = self.db()?;
            let old = db.get(&key,Value::Null)?;
            if !old.is_null() {
                if old["fingerprint"] != fingerprint { return Err(Error::new(409,"操作编号已用于其他请求")); }
                if let Some(result) = old.get("result") { return Ok(result.clone()); }
                if let Some(error) = old.get("error") { return Err(Error::new(error["status"].as_u64().unwrap_or(500) as u16,string(error,"message"))); }
                if op == "send" {
                    // Message history and receipts use different durable stores.
                    // Recover only an exact operation tag written with the input,
                    // never from matching text or an empty derived conversation.
                    let chat = db.chats()?.into_iter().find(|chat| {
                        if let Some(chat_id) = args["chat_id"].as_str() { chat["id"] == chat_id }
                        else { chat["session_id"] == format!("remote-{id}") }
                    });
                    if let Some(chat) = chat {
                        let history = db.history(required(&chat,"id")?,false)
                            .map_err(|_|Error::new(409,"原指令记录暂时无法核对，请在电脑检查；不会重复发送"))?;
                        let accepted = history.iter().any(|frame|
                            frame["role"] == "user" && frame["metadata"]["remote_operation_id"] == id);
                        if accepted {
                            let result = json!({"chat":chat,"delivery":"recovered"});
                            db.put(&key,&json!({"fingerprint":fingerprint,"result":result}))?;
                            return Ok(result);
                        }
                    }
                }
                return Err(Error::new(409,"该操作已接收，请刷新任务确认结果；不会重复执行"));
            }
            // Hold one lock across reservation. Parallel callers cannot both
            // observe a missing receipt and execute the same command.
            db.put(&key,&json!({"fingerprint":fingerprint}))?;
        }
        match self.remote_mutation(op,args,id).await {
            Ok(result) => { self.db()?.put(&key,&json!({"fingerprint":fingerprint,"result":result}))?; Ok(result) },
            Err(error) => { self.db()?.put(&key,&json!({"fingerprint":fingerprint,"error":error}))?; Err(error) },
        }
    }
    async fn remote_mutation(self: &Arc<Self>, op: &str, args: &Value, id: &str) -> Result<Value> {
        if op == "send" {
            let prompt = required(args,"text")?;
            if prompt.len() > 32000 {return Err(Error::new(400,"消息过长"));}
            let chat = if let Some(chat_id) = args["chat_id"].as_str() { self.db()?.chat(chat_id)? } else {json!({"session_id":format!("remote-{id}")})};
            let session = required(&chat,"session_id")?;
            if let Some(expected) = args.get("expected_run_id") {
                let expected = expected.as_str().filter(|s|!s.is_empty()).ok_or_else(||Error::new(422,"任务状态无效，请刷新"))?;
                if args.get("model_choice").is_some() { return Err(Error::new(422,"运行中的补充指令沿用当前任务配置")); }
                if lock(&self.runs)?.get(session).is_none_or(|run|run.request_id != expected) {
                    return Err(Error::new(412,"原任务已结束或改变，草稿已保留，请刷新后重新发送"));
                }
            }
            if lock(&self.runs)?.contains_key(session) {
                if args.get("model_choice").is_some() { return Err(Error::new(412,"任务已开始，不能切换运行中的模型，请刷新后重新发送")); }
                let queued = self.steer(&json!({"session_id":session,"text":prompt,"expected_run_id":args["expected_run_id"],"remote_operation_id":id}))?;
                self.notify_background(session);
                return Ok(json!({"chat":chat,"delivery":queued["status"]}));
            }
            let mut context = json!({"last_user_message":prompt});
            if let Some(path) = args["project_path"].as_str().filter(|p| !p.is_empty()) {
                let projects = self.request("GET","/api/workspace/coding-project/list",Value::Null).await?;
                let current = self.project_dir().await?;
                if path != current.to_string_lossy() && !projects.as_array().is_some_and(|ps|ps.iter().any(|p|p["path"] == path)) {return Err(Error::new(403,"只能选择电脑上已有的项目"));}
                context["potato.coding_project_dir"] = json!(path);
            }
            // Existing conversations always keep their saved project.
            if let Some(path) = chat["project_path"].as_str() { context["potato.coding_project_dir"] = json!(path); }
            // A newer phone binds a steering request to one exact run; it may never become a new turn.
            if args.get("expected_run_id").is_some() { return Err(Error::new(412,"原任务已结束，请刷新后重新发送")); }
            let mut body = json!({"session_id":session,"user_id":"default","channel":"console","stream":true,"input":[{"role":"user","content":[{"type":"text","text":prompt}]}],"request_context":context});
            body["remote_operation_id"] = json!(id);
            if let Some(choice) = args.get("model_choice") { body["remote_model"] = choice.clone(); }
            let core = Arc::downgrade(self);
            let notification_session = session.to_owned();
            self.start(id.to_owned(),body,Arc::new(move |_| {if let Some(core) = core.upgrade() {core.notify_background(&notification_session);} Ok(())})).map_err(|error| {
                if error.status == 409 && args.get("model_choice").is_some() { Error::new(412,"任务已开始，请刷新后重新发送") } else { error }
            })?;
            self.notify_background(session);
            let chat = self.db()?.chats()?.into_iter().find(|c|c["session_id"] == session).ok_or_else(||Error::new(500,"会话未保存"))?;
            return Ok(json!({"chat":chat}));
        }
        let chat = self.db()?.chat(required(args,"chat_id")?)?;
        let session = required(&chat,"session_id")?;
        match op {
            "stop" => {
                let expected = args["expected_run_id"].as_str().filter(|id| !id.is_empty())
                    .ok_or_else(||Error::new(422,"请更新手机端并刷新任务后再停止"))?;
                self.request("POST",&format!("/api/console/chat/stop?chat_id={}",segment(required(&chat,"id")?)),json!({"expected_run_id":expected})).await
            },
            "pin" => {
                let pinned = args["pinned"].as_bool().ok_or_else(||Error::new(400,"pinned 必须为布尔值"))?;
                let result = self.request("PUT",&format!("/api/chats/{}",segment(required(&chat,"id")?)),json!({"pinned":pinned})).await?;
                self.notify_background(session); Ok(result)
            },
            "approval" => {
                let approval = { let pending = lock(&self.approvals)?; pending.get(required(args,"request_id")?).map(|a|a.view.clone()).ok_or_else(||Error::new(409,"审批已过期"))? };
                if approval["root_session_id"] != session {return Err(Error::new(403,"审批不属于当前会话"));}
                let allow = args["allow"].as_bool().ok_or_else(||Error::new(400,"请选择允许或拒绝"))?;
                let scope = args["scope"].as_str().unwrap_or("exact");
                if !matches!(scope, "exact" | "session_directory" | "persistent_directory") || (!allow && scope != "exact") {
                    return Err(Error::new(400,"不支持的远程审批范围"));
                }
                if scope != "exact" && (approval["allow_directory"] != true || approval["suggested_directory"].as_str().filter(|s| !s.is_empty()).is_none()) {
                    return Err(Error::new(400,"此操作不支持目录授权"));
                }
                self.request("POST",if allow {"/api/approval/approve"} else {"/api/approval/deny"},json!({"request_id":args["request_id"],"session_id":session,"user_id":approval["user_id"],"scope":scope,"directory":approval["suggested_directory"],"recursive":approval["directory_recursive"].as_bool().unwrap_or(true)})).await
            },
            "answer" => {
                let q = self.db()?.question(required(args,"request_id")?)?;
                if q["session_id"] != session { return Err(Error::new(403,"提问不属于当前会话")); }
                self.answer_question(required(args,"request_id")?, &json!({"skip":args["skip"].as_bool().unwrap_or(false),"selected":args["selected"].as_array().cloned().unwrap_or_default(),"text":args["text"].as_str().unwrap_or("")}))
            },
            _ => Err(Error::new(403,"不支持的操作")),
        }
    }
}
