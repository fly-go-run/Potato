//! Configure an isolated UI acceptance workspace against a local fixture server.
use serde_json::json;
fn main() {
    let path = std::env::args().nth(1).expect("isolated review directory");
    assert!(path.starts_with("/tmp/gpui-outbox-") || path.starts_with("/private/tmp/gpui-outbox-"));
    let url = std::env::args().nth(2).expect("local fixture URL");
    assert!(url.starts_with("http://127.0.0.1:"));
    let executor = tokio::runtime::Runtime::new().unwrap();
    let core = potato_core::Runtime::open(std::path::Path::new(&path)).unwrap();
    executor.block_on(async {
        core.request("PUT","/api/models/deepseek/config",json!({"api_key":"fixture-only","base_url":url,"chat_model":"OpenAIChatModel"})).await.unwrap();
        core.request("PUT","/api/models/active",json!({"provider_id":"deepseek","model":"deepseek-chat"})).await.unwrap();
        core.request("PUT","/api/native/preferences",json!({"width":1080,"height":760,"remember_window":true,"dark":false,"follow_system":false})).await.unwrap();
    });
}
