//! Opt-in live check against an already authenticated, disposable QA profile.
use potato_core::Runtime;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::var("POTATO_CLOUD_QA_DIR")?).canonicalize()?;
    assert!(
        root.starts_with(std::env::temp_dir().canonicalize()?) || root.starts_with("/private/tmp")
    );
    let core = Runtime::open(&root)?;
    let account = core
        .request("GET", "/api/native/cloud", Value::Null)
        .await?;
    assert_eq!(
        account["signed_in"], true,
        "Sign in using the QA client first"
    );
    println!("Persisted account: signed in");
    let catalog = core
        .request("POST", "/api/native/cloud/refresh", json!({}))
        .await?;
    assert!(catalog["model_count"].as_u64().unwrap_or(0) > 0);
    println!("Refreshed models: {}", catalog["model_count"]);
    let model = std::env::var("POTATO_CLOUD_QA_MODEL").ok();
    if let Some(model) = model {
        let providers = core.request("GET", "/api/models", Value::Null).await?;
        let model = providers
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "potato-cloud")
            .unwrap()["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == model || m["name"] == model)
            .expect("Unknown cloud QA model")["id"]
            .as_str()
            .unwrap()
            .to_owned();
        core.request(
            "PUT",
            "/api/models/active",
            json!({"provider_id":"potato-cloud","model":model}),
        )
        .await?;
    }
    let session = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    core.start(uuid::Uuid::new_v4().to_string(),json!({"session_id":session,"input":[{"role":"user","content":[{"type":"text","text":"Use execute_shell_command exactly once to execute this shell builtin: echo $((17 * 23)) . Do not read files or use any other command. Reply with the command output only."}]}]}),Arc::new(move|v|{let _=tx.send(v);Ok(())}))?;
    tokio::time::timeout(Duration::from_secs(90), async {
        let mut tool_succeeded = false;
        let mut answered = false;
        while let Some(v) = rx.recv().await {
            if v["type"] == "function_call_output" {
                let output: Value = serde_json::from_str(
                    v["content"][0]["data"]["output"].as_str().unwrap_or("null"),
                )
                .unwrap_or(Value::Null);
                tool_succeeded |= output["exit_code"] == 0
                    && output["preview"]["stdout"]
                        .as_str()
                        .is_some_and(|s| s.trim() == "391");
            }
            if v["object"] == "message" && v["type"] == "message" && v["status"] == "completed" {
                answered |= v["content"][0]["text"]
                    .as_str()
                    .is_some_and(|s| s.trim() == "391");
            }
            if v["object"] == "response" && v["status"] != "in_progress" {
                assert_eq!(v["status"], "completed");
                assert!(tool_succeeded, "No successful local arithmetic tool result");
                assert!(answered, "Model did not return the tool result");
                println!("PASS: local shell exit 0, output 391, cloud final answer 391");
                return;
            }
        }
        panic!("No terminal response");
    })
    .await?;
    Ok(())
}
