use crate::{Error, Result};
use serde_json::{json, Value};

pub fn message(id: &str, kind: &str, role: &str, content: Value, status: &str) -> Value {
    json!({"object":"message", "id":id,"type":kind,"role":role,
        "content":content,"status":status,"metadata":null})
}

pub fn text(id: &str, value: &str, delta: bool) -> Value {
    json!({"object":"content","type":"text","msg_id":id,"index":0,
        "text":value,"delta":delta,"status":if delta {"in_progress"} else {"completed"}})
}

pub fn data(id: &str, value: Value) -> Value {
    json!({"object":"content","type":"data","msg_id":id,"index":0,
        "data":value,"delta":false,"status":"completed"})
}

pub fn response(id: &str, session: &str, status: &str) -> Value {
    json!({"object":"response","id":id,"session_id":session,"status":status,
        "output":[],"metadata":null,"created_at":null,"completed_at":null})
}

/// Parse complete SSE lines as bytes so split UTF-8 and split CRLF are lossless.
#[derive(Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    data: Vec<String>,
}
impl SseDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>> {
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() > 2_000_000 {
            return Err(Error::new(502, "Model stream event too large"));
        }
        let mut events = Vec::new();
        while let Some(pos) = self.buffer.iter().position(|b| *b == b'\n') {
            let bytes: Vec<u8> = self.buffer.drain(..=pos).collect();
            let line = std::str::from_utf8(&bytes[..bytes.len() - 1])
                .map_err(|_| Error::new(502, "Invalid UTF-8 model stream"))?
                .trim_end_matches('\r');
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.join("\n"));
                    self.data.clear();
                }
            } else if let Some(value) = line.strip_prefix("data:") {
                self.data
                    .push(value.strip_prefix(' ').unwrap_or(value).to_owned());
                if self.data.iter().map(String::len).sum::<usize>() > 2_000_000 {
                    return Err(Error::new(502, "Model stream event too large"));
                }
            }
        }
        Ok(events)
    }
}
