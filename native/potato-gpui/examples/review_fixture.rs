//! Seed an explicitly supplied, isolated review directory with synthetic history.
use serde_json::json;
fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("review data directory required");
    assert!(
        path.contains("gpui-chat-review"),
        "Use a dedicated gpui-chat-review directory"
    );
    let executor = tokio::runtime::Runtime::new().unwrap();
    let core = potato_core::Runtime::open(std::path::Path::new(&path)).unwrap();
    let message = |id, kind, role, text| json!({"id":id,"type":kind,"role":role,"status":"completed","content":[{"type":"text","text":text}]});
    let long = std::env::args().any(|a| a == "--long");
    let ending = if long {
        (1..=40)
            .map(|i| format!("第 {i} 段：用于检查长消息滚动与阅读位置保持。\n\n"))
            .collect::<String>()
            + "长消息结束。"
    } else {
        "已完成这一轮检查。\n\n| 区域 | 结果 |\n| --- | --- |\n| 首页与设置 | 布局一致 |\n| 工具与思考 | 可展开查看 |\n\n接下来可继续检查长消息和附件。".into()
    };
    let bundle = json!({"format":"potato-native-history-v1","chats":[{"spec":{"id":"visual-review","session_id":"visual-review-session","name":"聊天界面验收","user_id":"default","channel":"console"},"messages":[
        message("u1","message","user","请检查这次界面迁移，并给出简要结果。"),
        message("r1","reasoning","assistant","先检查布局与按钮层级，再核对工具结果和最终回复。"),
        {"id":"c1","type":"plugin_call","role":"assistant","status":"completed","content":[{"type":"data","data":{"call_id":"review-call","name":"read_file","arguments":"{\"path\":\"README.md\"}"}}]},
        {"id":"o1","type":"plugin_call_output","role":"tool","status":"completed","content":[{"type":"data","data":{"call_id":"review-call","name":"read_file","output":"# Potato\nRust 原生界面迁移说明。","state":"success"}}]},
        message("a1","message","assistant",&ending)
    ]}]});
    executor
        .block_on(core.request("POST", "/api/native/import-history", bundle))
        .unwrap();
    if std::env::args().any(|a| a == "--compact") {
        executor.block_on(core.request("PUT", "/api/native/preferences",
            json!({"width":800,"height":580,"remember_window":true,"dark":true,"follow_system":false}))).unwrap();
    }
    println!("Seeded synthetic review history");
}
