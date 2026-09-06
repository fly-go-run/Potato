//! Explicit, opt-in live-service smoke test. Uses a disposable encrypted database.
//! cargo run --example service_acceptance -- LEGACY_DIR SECRET_DIR MODEL PCM_FILE
//! PCM must be signed little-endian 16-bit, 16 kHz, mono; use non-private test audio.
//! To persist connections without making calls or importing chat history:
//! cargo run --example service_acceptance -- --import-only LEGACY_DIR SECRET_DIR NATIVE_DIR
use potato_core::{Error, Result, Runtime};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};

async fn chat(core: &Arc<Runtime>, provider: &str, model: &str) -> Result<Value> {
    core.request(
        "PUT",
        "/api/models/active",
        json!({"provider_id":provider,"model":model}),
    )
    .await?;
    let request_id = uuid::Uuid::new_v4().to_string();
    let session = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let started = std::time::Instant::now();
    core.start(request_id.clone(), json!({"session_id":session,"user_id":"native-acceptance","channel":"console","stream":true,"input":[{"role":"user","content":[{"type":"text","text":"这是客户端连通性测试。不要调用工具。只回复：你好，验收通过。"}]}]}), Arc::new(move |v| { let _ = tx.send(v); Ok(()) }))?;
    let result = tokio::time::timeout(Duration::from_secs(90), async {
        let mut deltas = 0;
        while let Some(frame) = rx.recv().await {
            if frame["object"] == "content" && frame["type"] == "text" && frame["delta"] == true { deltas += 1; }
            if frame["object"] == "response" && matches!(frame["status"].as_str(), Some("completed" | "failed" | "cancelled")) {
                if frame["status"] != "completed" { return Err(Error::new(502, frame["error"]["message"].as_str().unwrap_or("Chat did not complete"))); }
                let chats = core.request("GET", "/api/chats", Value::Null).await?;
                let chat = chats.as_array().and_then(|v| v.iter().find(|c| c["session_id"] == session)).ok_or_else(|| Error::new(500,"Acceptance chat missing"))?;
                let path = format!("/api/chats/{}", chat["id"].as_str().unwrap());
                let history = core.request("GET", &path, Value::Null).await?;
                let persisted = history["messages"].as_array().is_some_and(|items| items.iter().any(|m| m["role"] == "assistant" && m.to_string().contains("验收通过")));
                if deltas == 0 || !persisted || history["status"] != "idle" { return Err(Error::new(500,"Expected streamed Chinese reply and idle persisted history")); }
                return Ok(json!({"passed":true,"model":model,"text_delta_events":deltas,"history_persisted":persisted,"history_status":history["status"],"elapsed_ms":started.elapsed().as_millis()}));
            }
        }
        Err(Error::new(502,"Chat stream ended without terminal event"))
    }).await;
    if result.is_err() {
        let _ = core.cancel(&request_id);
    }
    result.map_err(|_| Error::new(504, "Live chat test timed out"))?
}

async fn speech(core: &Arc<Runtime>, pcm: &Path) -> Result<Value> {
    let audio = std::fs::read(pcm)?;
    if audio.is_empty() || audio.len() > 960_000 || !audio.len().is_multiple_of(2) {
        return Err(Error::new(
            400,
            "Provide at most 30 seconds of mono 16 kHz s16le PCM",
        ));
    }
    core.request(
        "PUT",
        "/api/native/doubao-settings",
        json!({"enabled":true}),
    )
    .await?;
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    core.voice_start(
        id.clone(),
        Arc::new(move |v| {
            let _ = tx.send(v);
            Ok(())
        }),
    )
    .await?;
    let result = tokio::time::timeout(Duration::from_secs(45), async {
        for bytes in audio.chunks(3200) {
            core.voice_audio(&id, bytes.to_vec())?;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        core.voice_end(&id, false).await?;
        let mut partials = 0;
        while let Some(frame) = rx.recv().await {
            if frame["type"] == "error" { return Err(Error::new(502,frame["message"].as_str().unwrap_or("Speech failed"))); }
            if frame["type"] == "partial" { partials += 1; }
            if frame["type"] == "final" {
                let transcript = frame["text"].as_str().unwrap_or("");
                if !transcript.contains("语音") || !transcript.contains("测试") { return Err(Error::new(502,"Speech transcript did not match the test phrase")); }
                return Ok(json!({"passed":true,"partial_events":partials,"final_received":true,"transcript":transcript,"pcm_bytes":audio.len(),"microphone_tested":false}));
            }
        }
        Err(Error::new(502,"Speech stream ended without final"))
    }).await;
    let _ = core.voice_end(&id, true).await;
    result.map_err(|_| Error::new(504, "Live speech test timed out"))?
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 4 && args[0] == "--import-only" {
        let core = Runtime::open(Path::new(&args[3]))?;
        let result = core.import_legacy_settings(Path::new(&args[1]), Path::new(&args[2]))?;
        println!("{}", result);
        return Ok(());
    }
    if args.len() != 4 {
        return Err("Usage: service_acceptance LEGACY_DIR SECRET_DIR MODEL PCM_FILE | --import-only LEGACY_DIR SECRET_DIR NATIVE_DIR".into());
    }
    let temporary = tempfile::tempdir()?;
    let core = Runtime::open(temporary.path())?;
    let import = core.import_legacy_settings(Path::new(&args[0]), Path::new(&args[1]))?;
    println!("{}", json!({"stage":"import","summary":import}));
    let mut passed = true;
    for stage in ["deepseek", "deepseek-response", "sub2api", "doubao"] {
        let result = match stage {
            "deepseek" => chat(&core, stage, "deepseek-v4-flash").await,
            "deepseek-response" => chat(&core, stage, "deepseek-v4-flash").await,
            "sub2api" => chat(&core, stage, &args[2]).await,
            _ => speech(&core, Path::new(&args[3])).await,
        };
        match result {
            Ok(report) => println!("{}", json!({"stage":stage,"report":report})),
            Err(error) => {
                passed = false;
                println!(
                    "{}",
                    json!({"stage":stage,"passed":false,"error":error.message})
                );
            }
        }
    }
    if !passed {
        return Err("One or more live-service checks failed".into());
    }
    Ok(())
}
