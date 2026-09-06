use crate::{required, string, Error, Result};
use serde_json::{json, Value};

/// Convert the public history contract, never Python implementation objects.
/// Preserve original display frames; unsupported content stays visible there.
pub(crate) fn wire(frame: &Value) -> Option<Value> {
    match string(frame, "type") {
        "message" if matches!(string(frame, "role"), "user" | "assistant" | "system") => {
            let blocks = frame["content"].as_array()?;
            let text = blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() {
                None
            } else {
                Some(json!({"role":frame["role"],"content":text}))
            }
        }
        "function_call" | "mcp_tool_call" | "plugin_call" => {
            let data = &frame["content"][0]["data"];
            let name = data["name"].as_str()?;
            let id = data["call_id"].as_str()?;
            Some(
                json!({"role":"assistant","content":"","tool_calls":[{"id":id,"type":"function","function":{"name":name,"arguments":data["arguments"]}}]}),
            )
        }
        "function_call_output" | "mcp_tool_call_output" | "plugin_call_output" => {
            let data = &frame["content"][0]["data"];
            Some(
                json!({"role":"tool","tool_call_id":data["call_id"].as_str()?,"content":data["output"].as_str()?}),
            )
        }
        _ => None,
    }
}

pub(crate) fn validate(bundle: &Value) -> Result<&Vec<Value>> {
    if bundle["format"] != "potato-native-history-v1" {
        return Err(Error::new(400, "Unsupported history export format"));
    }
    let chats = bundle["chats"]
        .as_array()
        .ok_or_else(|| Error::new(400, "Missing exported chats"))?;
    for chat in chats {
        required(&chat["spec"], "id")?;
        required(&chat["spec"], "session_id")?;
        required(&chat["spec"], "name")?;
        let frames = chat["messages"]
            .as_array()
            .ok_or_else(|| Error::new(400, "Missing exported messages"))?;
        for frame in frames {
            required(frame, "id")?;
            required(frame, "type")?;
            if !frame["content"].is_array() {
                return Err(Error::new(400, "Invalid exported message content"));
            }
        }
    }
    Ok(chats)
}
