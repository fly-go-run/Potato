use crate::{Emit, Error, Result};
use serde_json::Value;
use std::collections::HashMap;

pub(crate) struct Replay {
    listeners: HashMap<String, Emit>,
    frames: Vec<Value>,
    bytes: usize,
    overflow: bool,
}
impl Replay {
    pub(crate) fn new(id: String, emit: Emit) -> Self {
        Self {
            listeners: HashMap::from([(id, emit)]),
            frames: Vec::new(),
            bytes: 0,
            overflow: false,
        }
    }
    pub(crate) fn contains(&self, id: &str) -> bool {
        self.listeners.contains_key(id)
    }
    pub(crate) fn attach(&mut self, id: String, emit: Emit) -> Result<()> {
        if self.overflow {
            return Err(Error::new(
                409,
                "Live replay exceeded its buffer; wait for completion or stop the turn",
            ));
        }
        if self.listeners.contains_key(&id) {
            return Err(Error::new(409, "Stream listener already registered"));
        }
        if self.listeners.len() >= 16 {
            return Err(Error::new(429, "Too many listeners for this turn"));
        }
        for frame in &self.frames {
            emit(frame.clone())?;
        }
        self.listeners.insert(id, emit);
        Ok(())
    }
    pub(crate) fn publish(&mut self, frame: Value) {
        if !self.overflow {
            self.bytes += frame.to_string().len();
            if self.bytes > 16_000_000 {
                self.overflow = true;
                self.frames.clear();
            } else {
                self.frames.push(frame.clone());
            }
        }
        self.listeners.retain(|_, emit| emit(frame.clone()).is_ok());
    }
}
