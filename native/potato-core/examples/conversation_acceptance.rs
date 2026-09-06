//! Opt-in real-model multi-turn/concurrent-session check; uses synthetic data
//! and a disposable database. No credentials or prompts are printed.
//! cargo run --example conversation_acceptance -- LEGACY_DIR SECRET_DIR
use potato_core::Runtime;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};

async fn turn(
    core: &Arc<Runtime>,
    session: &str,
    user: &str,
    text: &str,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    core.start(id.clone(), json!({"session_id":session,"user_id":user,"input":[{"role":"user","content":[{"type":"text","text":text}]}]}), Arc::new(move |frame| { let _ = tx.send(frame); Ok(()) })).map_err(|e|e.message)?;
    let result = tokio::time::timeout(Duration::from_secs(90), async {
        while let Some(frame) = rx.recv().await {
            if frame["object"] == "response"
                && matches!(
                    frame["status"].as_str(),
                    Some("completed" | "failed" | "cancelled")
                )
            {
                if frame["status"] != "completed" {
                    return Err("Turn failed or cancelled".to_string());
                }
                // Read persisted history, independently of the streaming field shape.
                let chats = core
                    .request("GET", "/api/chats", Value::Null)
                    .await
                    .map_err(|e| e.message)?;
                let chat = chats
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["session_id"] == session)
                    .ok_or("Missing chat")?;
                let history = core
                    .request(
                        "GET",
                        &format!("/api/chats/{}", chat["id"].as_str().unwrap()),
                        Value::Null,
                    )
                    .await
                    .map_err(|e| e.message)?;
                if history["status"] != "idle" {
                    return Err("Chat did not return to idle".into());
                }
                return history["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .rev()
                    .find(|m| m["role"] == "assistant")
                    .map(|m| m.to_string())
                    .ok_or("Missing assistant history".into());
            }
        }
        Err("Stream ended without terminal frame".into())
    })
    .await;
    if result.is_err() {
        let _ = core.cancel(&id);
    }
    result.map_err(|_| "Timed out".to_string())?
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: conversation_acceptance LEGACY_DIR SECRET_DIR".into());
    }
    for (provider, model) in [
        ("sub2api", "gpt-5.6-sol"),
        ("deepseek-response", "deepseek-v4-flash"),
    ] {
        let dir = tempfile::tempdir()?;
        let core = Runtime::open(dir.path())?;
        core.import_legacy_settings(Path::new(&args[0]), Path::new(&args[1]))?;
        core.request(
            "PUT",
            "/api/models/active",
            json!({"provider_id":provider,"model":model}),
        )
        .await?;
        let (a, b) = tokio::join!(
            turn(
                &core,
                "session-a",
                "synthetic-alice",
                "记住本会话口令为 MAPLE-7319。不调用工具，只回复已记住。"
            ),
            turn(
                &core,
                "session-b",
                "synthetic-bob",
                "记住本会话口令为 RIVER-2846。不调用工具，只回复已记住。"
            )
        );
        a?;
        b?;
        drop(core);
        let core = Runtime::open(dir.path())?;
        let prompt = "本会话之前告诉你的口令是什么？不要调用工具，只回复口令。";
        let (a, b) = tokio::join!(
            turn(&core, "session-a", "synthetic-alice", prompt),
            turn(&core, "session-b", "synthetic-bob", prompt)
        );
        let a = a?;
        let b = b?;
        if !a.contains("MAPLE-7319")
            || a.contains("RIVER-2846")
            || !b.contains("RIVER-2846")
            || b.contains("MAPLE-7319")
        {
            return Err(format!("{provider}: context isolation or restart recall failed").into());
        }
        println!(
            "{}",
            json!({"provider":provider,"model":model,"passed":true,"concurrent_sessions":2,"distinct_users":2,"restart_recall":true,"cross_session_leak":false})
        );
    }
    Ok(())
}
