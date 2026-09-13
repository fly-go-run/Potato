//! Bootstrap or inspect an isolated automatic-approval UI acceptance workspace.
//! All model calls go to a local fixture; this does not use personal configuration.
use serde_json::{Value, json};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let path = args.get(1).expect("isolated data directory");
    assert!(
        path.starts_with("/tmp/potato-auto-approval-")
            || path.starts_with("/private/tmp/potato-auto-approval-")
    );
    let executor = tokio::runtime::Runtime::new().unwrap();
    let core = potato_core::Runtime::open(std::path::Path::new(path)).unwrap();
    executor.block_on(async {
        if args.get(2).is_some_and(|v| v == "inspect") {
            let chats = core.request("GET", "/api/chats", Value::Null).await.unwrap();
            for chat in chats.as_array().unwrap() {
                let session = chat["session_id"].as_str().unwrap();
                let audit = core.request("GET", &format!("/api/approval/audit?session_id={session}"), Value::Null).await.unwrap();
                println!("{}", json!({"session_id":session,"audit":audit}));
            }
            return;
        }
        let url = args.get(2).expect("local fixture URL");
        assert!(url.starts_with("http://127.0.0.1:"));
        let project = std::path::Path::new(path).parent().unwrap().join("project");
        std::fs::create_dir_all(&project).unwrap();
        core.request("PUT", "/api/models/deepseek/config", json!({"api_key":"fixture-only", "base_url":url, "chat_model":"OpenAIChatModel"})).await.unwrap();
        core.request("PUT", "/api/models/active", json!({"provider_id":"deepseek", "model":"deepseek-chat"})).await.unwrap();
        core.request("PUT", "/api/workspace/coding-project", json!({"path":project})).await.unwrap();
        core.request("PUT", "/api/native/preferences", json!({"width":1080,"height":760,"remember_window":true,"dark":false,"follow_system":false})).await.unwrap();
    });
}
