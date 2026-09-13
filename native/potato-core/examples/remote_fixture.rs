//! iPhone → Worker → native core verification with a disposable workspace and
//! scripted model. Default is loopback; --account explicitly tests our live relay.
use potato_core::Runtime;
use serde_json::{json,Value};
use std::{path::PathBuf,sync::Arc,time::Duration};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener};
fn reply(text:&str)->String {format!("data: {}\n\ndata: [DONE]\n\n",json!({"choices":[{"delta":{"content":text},"finish_reason":"stop"}]}))}
fn call(name:&str,args:Value)->String {format!("data: {}\n\ndata: [DONE]\n\n",json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":uuid::Uuid::new_v4().to_string(),"function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":"tool_calls"}]}))}
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>> {
    let args=std::env::args().collect::<Vec<_>>();
    let account=args.len()==5 && args[4]=="--account";
    if args.len()!=4 && !account {return Err("Usage: remote_fixture RELAY OUTPUT_JSON DISPOSABLE_ROOT [--account]".into());}
    let relay=reqwest::Url::parse(&args[1])?;
    if account {
        if relay.as_str()!="https://potato-remote.recodex.top/" {return Err("Account fixture only permits the deployed Potato relay".into());}
    } else if relay.scheme()!="http" || relay.host_str()!=Some("127.0.0.1") {return Err("Fixture requires an explicit loopback relay".into());}
    let root=PathBuf::from(&args[3]); if root.exists(){return Err("Fixture root must be new".into());}
    let model=TcpListener::bind("127.0.0.1:0").await?;let base=format!("http://{}",model.local_addr()?);
    tokio::spawn(async move {
        while let Ok((mut socket,_))=model.accept().await {
            tokio::spawn(async move {
                let mut data=Vec::new();let mut chunk=[0;8192];
                let request=loop {
                    let Ok(n)=socket.read(&mut chunk).await else{return};if n==0{return}data.extend_from_slice(&chunk[..n]);if data.len()>2_000_000{return}
                    if let Some(end)=data.windows(4).position(|s|s==b"\r\n\r\n") {
                        let headers=String::from_utf8_lossy(&data[..end]).to_lowercase();
                        let len=headers.lines().find_map(|l|l.strip_prefix("content-length: ").and_then(|v|v.parse::<usize>().ok())).unwrap_or(0);
                        if data.len()>=end+4+len {let Ok(value)=serde_json::from_slice::<Value>(&data[end+4..end+4+len])else{return};break value;}
                    }
                };
                let messages=request["messages"].as_array().cloned().unwrap_or_default();
                let user=messages.iter().rev().find(|m|m["role"]=="user").map(|m|m["content"].to_string()).unwrap_or_default();
                let tool=messages.last().is_some_and(|m|m["role"]=="tool");
                let text=if user.contains("remote wait") {tokio::time::sleep(Duration::from_secs(25)).await;reply("WAIT_COMPLETED")}
                    else if user.contains("remote question") {if tool {reply("POTATO_QUESTION_OK")}else{call("request_user_input",json!({"title":"检查范围？","options":[{"id":"all","label":"全部"}],"multiple":false}))}}
                    else if user.contains("remote approval") {if tool {if messages.last().unwrap()["content"].to_string().contains("POTATO_APPROVED") {reply("POTATO_APPROVAL_OK")}else{reply("APPROVAL_DID_NOT_EXECUTE")}}else{call("execute_shell_command",json!({"command":"echo POTATO_APPROVED","sandbox_permissions":"require_escalated","justification":"合成远程审批测试，仅输出固定字符串"}))}}
                    else if user.contains("remote continue") {if messages.iter().any(|m|m["role"]=="assistant" && m["content"].to_string().contains("POTATO_REMOTE_OK")){reply("POTATO_CONTINUED_OK")}else{reply("CONTEXT_MISSING")}}
                    else {reply("POTATO_REMOTE_OK")};
                let response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len());let _=socket.write_all(response.as_bytes()).await;
            });
        }
    });
    let core=Runtime::open(&root)?;
    core.request("PUT","/api/workspace/running-config",json!({"reviewer":"user","approval_level":"STRICT"})).await?;
    core.request("PUT","/api/models/deepseek/config",json!({"api_key":"synthetic-only","base_url":base,"chat_model":"OpenAIChatModel"})).await?;
    core.request("POST","/api/models/deepseek/models",json!({"id":"fixture","name":"Fixture"})).await?;
    core.request("PUT","/api/models/active",json!({"provider_id":"deepseek","model":"fixture"})).await?;
    let remote=if account {
        let login=core.request("POST","/api/native/remote/login/start",json!({"relay":relay.as_str(),"name":"Potato 外网验收临时电脑"})).await?;
        std::fs::write(&args[2],serde_json::to_vec(&login)?)?;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let status=core.request("POST","/api/native/remote/login/poll",json!({})).await?;
            if status["auth_mode"]=="account" {break;}
        }
        core.request("POST","/api/native/remote",json!({"enabled":true})).await?
    } else {core.request("POST","/api/native/remote",json!({"enabled":true,"relay":relay.as_str(),"name":"本地联调电脑","service_token":"local-fixture-only-".repeat(4)})).await?};
    std::fs::write(&args[2],serde_json::to_vec(&remote)?)?;
    let listener=core.clone();tokio::spawn(async move{listener.serve_remote().await});
    // Hard limit prevents an accidentally orphaned fixture from running forever.
    for _ in 0..900 {
        if root.join("stop-fixture").exists() {break;}
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if account {core.request("POST","/api/native/remote/logout",json!({})).await?;}
    drop(Arc::clone(&core));Ok(())
}
