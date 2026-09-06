//! Potato SSE framing and reducer, independent of rendering and network transport.
use serde_json::{Value, json};

#[cfg(test)]
#[derive(Default)]
pub struct Parser {
    pending: Vec<u8>,
}

#[cfg(test)]
impl Parser {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Value>, String> {
        self.pending.extend_from_slice(bytes);
        let mut frames = vec![];
        loop {
            let boundary = self
                .pending
                .windows(2)
                .position(|s| s == b"\n\n")
                .map(|p| (p, 2))
                .into_iter()
                .chain(
                    self.pending
                        .windows(4)
                        .position(|s| s == b"\r\n\r\n")
                        .map(|p| (p, 4)),
                )
                .min_by_key(|(p, _)| *p);
            let Some((end, delimiter)) = boundary else {
                break;
            };
            if end > 2 * 1024 * 1024 {
                return Err("单个流式事件过大".into());
            }
            let event =
                std::str::from_utf8(&self.pending[..end]).map_err(|_| "流式响应不是有效 UTF-8")?;
            let data = event
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("data:")
                        .map(|value| value.strip_prefix(' ').unwrap_or(value))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() && data != "[DONE]" {
                frames.push(serde_json::from_str(&data).map_err(|_| "流式事件 JSON 格式错误")?);
            }
            self.pending.drain(..end + delimiter);
        }
        if self.pending.len() > 2 * 1024 * 1024 {
            return Err("单个流式事件过大".into());
        }
        Ok(frames)
    }

    pub fn finish(&self) -> Result<(), String> {
        if self.pending.iter().all(u8::is_ascii_whitespace) {
            Ok(())
        } else {
            Err("连接在事件传输中断开，请刷新会话确认后台状态".into())
        }
    }
}

#[derive(Default)]
pub struct Turn {
    pub messages: Vec<Value>,
    pub status: String,
    pub error: Option<String>,
    pub session_id: Option<String>,
    pub cleared: bool,
    response_id: String,
    sequence: u64,
}

impl Turn {
    pub fn terminal(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed" | "cancelled")
    }

    pub fn apply(&mut self, frame: Value) {
        if frame["object"] == "response"
            && frame["id"]
                .as_str()
                .is_some_and(|id| id != self.response_id)
        {
            self.response_id = frame["id"].as_str().unwrap().into();
            self.sequence = 0;
        }
        if let Some(seq) = frame["sequence_number"].as_u64() {
            if seq <= self.sequence {
                return;
            }
            self.sequence = seq;
        }
        if !frame["error"].is_null() {
            self.error = Some(error_text(&frame["error"]));
            self.status = "failed".into();
        }
        match frame["object"].as_str() {
            Some("response") => {
                if let Some(status) = frame["status"].as_str() {
                    self.status = status.into();
                }
                if let Some(id) = frame["session_id"].as_str() {
                    self.session_id = Some(id.into());
                }
                if let Some(messages) = frame["output"].as_array() {
                    for message in messages {
                        self.upsert(message.clone());
                    }
                }
            }
            Some("message") => self.upsert(frame),
            Some("content") => {
                let Some(id) = frame["msg_id"].as_str() else {
                    return;
                };
                let Some(message) = self.messages.iter_mut().find(|m| m["id"] == id) else {
                    return;
                };
                let index = frame["index"].as_u64().unwrap_or(0) as usize;
                if index > 4096 {
                    self.error = Some("内容块索引超出范围".into());
                    return;
                }
                let content = message["content"].as_array_mut().unwrap();
                content.resize(content.len().max(index + 1), Value::Null);
                let previous = &content[index];
                let mut merged = frame.clone();
                if frame["type"] == "text" && frame["delta"] == true {
                    merged["text"] = json!(format!(
                        "{}{}",
                        previous["text"].as_str().unwrap_or(""),
                        frame["text"].as_str().unwrap_or("")
                    ));
                } else if frame["type"] == "data" {
                    let mut data = previous["data"].as_object().cloned().unwrap_or_default();
                    if let Some(incoming) = frame["data"].as_object() {
                        for (key, value) in incoming {
                            if key == "arguments" && frame["delta"] == true {
                                data.insert(
                                    key.clone(),
                                    json!(format!(
                                        "{}{}",
                                        data.get(key).and_then(Value::as_str).unwrap_or(""),
                                        value.as_str().unwrap_or("")
                                    )),
                                );
                            } else {
                                data.insert(key.clone(), value.clone());
                            }
                        }
                    }
                    merged["data"] = Value::Object(data);
                }
                content[index] = merged;
            }
            _ => {}
        }
    }

    fn upsert(&mut self, mut message: Value) {
        if message["metadata"]["clear_history"] == true {
            self.messages.clear();
            self.cleared = true;
            return;
        }
        let Some(id) = message["id"].as_str() else {
            return;
        };
        let position = self.messages.iter().position(|m| m["id"] == id);
        if message["content"].as_array().is_none_or(|c| c.is_empty()) {
            message["content"] = position
                .map(|i| self.messages[i]["content"].clone())
                .unwrap_or_else(|| json!([]));
        }
        if let Some(i) = position {
            self.messages[i] = message;
        } else {
            self.messages.push(message);
        }
    }
}

pub fn error_text(value: &Value) -> String {
    value
        .as_str()
        .or_else(|| value["message"].as_str())
        .or_else(|| value["code"].as_str())
        .unwrap_or("后端返回错误")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEXT: &[u8] = include_bytes!("../../../app/fixtures/sse/simple-text.sse.txt");
    const TOOL: &[u8] = include_bytes!("../../../app/fixtures/sse/tool-call.sse.txt");

    fn replay(data: &[u8], size: usize) -> Turn {
        let mut parser = Parser::default();
        let mut turn = Turn::default();
        for chunk in data.chunks(size) {
            for frame in parser.push(chunk).unwrap() {
                turn.apply(frame);
            }
        }
        parser.finish().unwrap();
        turn
    }

    #[test]
    fn real_text_fixture_survives_every_byte_boundary_and_final_snapshots() {
        for size in [1, 2, 3, 7, 64, 4096] {
            let turn = replay(TEXT, size);
            assert!(turn.terminal());
            assert_eq!(turn.messages.len(), 1);
            assert_eq!(
                turn.messages[0]["content"][0]["text"],
                "你好，我是 Potato。"
            );
        }
    }

    #[test]
    fn tools_are_preserved_without_duplicate_messages() {
        let turn = replay(TOOL, 7);
        assert!(turn.terminal());
        assert!(turn.messages.iter().any(|m| m["type"] == "plugin_call"));
        let ids: std::collections::HashSet<_> = turn.messages.iter().map(|m| &m["id"]).collect();
        assert_eq!(ids.len(), turn.messages.len());
    }

    #[test]
    fn crlf_and_multiline_data_and_partial_eof() {
        let mut parser = Parser::default();
        assert!(
            parser
                .push(b": ping\r\n\r\ndata: {\r\ndata: \"x\":1}\r")
                .unwrap()
                .is_empty()
        );
        assert_eq!(parser.push(b"\n\r\n").unwrap(), vec![json!({"x":1})]);
        parser.push(b"data: {").unwrap();
        assert!(parser.finish().is_err());
    }

    #[test]
    fn new_response_resets_sequence_and_clear_history() {
        let mut turn = replay(TEXT, 100);
        turn.apply(
            json!({"object":"response", "id":"new", "status":"in_progress", "sequence_number":1}),
        );
        assert!(!turn.terminal());
        turn.apply(
            json!({"object":"message", "metadata":{"clear_history":true}, "sequence_number":2}),
        );
        assert!(turn.cleared);
        assert!(turn.messages.is_empty());
    }
}
