//! Typed event dispatch for server push frames.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::protocol::RawFrame;

type PayloadHandler = Arc<dyn Fn(&Value) + Send + Sync>;
type FrameHandler = Arc<dyn Fn(&RawFrame) + Send + Sync>;

/// Unsubscribe token returned by listener registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenerId(u64);

/// Receive side of a push-event stream.
pub type EventStream<T> = mpsc::UnboundedReceiver<T>;

/// Loosely typed push-frame emitter.
#[derive(Default)]
pub struct TypedEmitter {
    next_id: u64,
    handlers: HashMap<String, Vec<(ListenerId, PayloadHandler)>>,
    any_handlers: Vec<(ListenerId, FrameHandler)>,
    streams: HashMap<String, Vec<mpsc::UnboundedSender<Value>>>,
    any_streams: Vec<mpsc::UnboundedSender<RawFrame>>,
}

impl TypedEmitter {
    /// Listen for one push type.
    pub fn on<F>(&mut self, kind: impl Into<String>, handler: F) -> ListenerId
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        let id = self.next_listener_id();
        self.handlers
            .entry(kind.into())
            .or_default()
            .push((id, Arc::new(handler)));
        id
    }

    /// Listen for every push frame.
    pub fn on_any<F>(&mut self, handler: F) -> ListenerId
    where
        F: Fn(&RawFrame) + Send + Sync + 'static,
    {
        let id = self.next_listener_id();
        self.any_handlers.push((id, Arc::new(handler)));
        id
    }

    /// Remove a callback listener.
    pub fn off(&mut self, id: ListenerId) {
        for handlers in self.handlers.values_mut() {
            handlers.retain(|(handler_id, _)| *handler_id != id);
        }
        self.any_handlers
            .retain(|(handler_id, _)| *handler_id != id);
    }

    /// Stream payloads for one push type.
    pub fn stream(&mut self, kind: impl Into<String>) -> EventStream<Value> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.streams.entry(kind.into()).or_default().push(tx);
        rx
    }

    /// Stream every push frame.
    pub fn any_stream(&mut self) -> EventStream<RawFrame> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.any_streams.push(tx);
        rx
    }

    /// Dispatch a push frame. Callback panics are isolated so one consumer
    /// cannot prevent other listeners or streams from receiving the frame.
    pub fn emit(&mut self, frame: &RawFrame) {
        let payload = frame.payload.clone().unwrap_or(Value::Null);
        if let Some(handlers) = self.handlers.get(&frame.kind) {
            for (_, handler) in handlers.clone() {
                safe_call(|| handler(&payload));
            }
        }
        for (_, handler) in self.any_handlers.clone() {
            safe_call(|| handler(frame));
        }
        if let Some(streams) = self.streams.get_mut(&frame.kind) {
            streams.retain(|stream| stream.send(payload.clone()).is_ok());
        }
        self.any_streams
            .retain(|stream| stream.send(frame.clone()).is_ok());
    }

    /// End every open stream. Callback listeners are kept.
    pub fn close_streams(&mut self) {
        self.streams.clear();
        self.any_streams.clear();
    }

    fn next_listener_id(&mut self) -> ListenerId {
        let id = ListenerId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }
}

fn safe_call(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;

    fn frame(kind: &str, payload: Value) -> RawFrame {
        RawFrame {
            kind: kind.to_string(),
            request_id: None,
            payload: Some(payload),
        }
    }

    #[test]
    fn matching_listener_receives_payload() {
        let mut emitter = TypedEmitter::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_for_handler = Arc::clone(&seen);
        emitter.on("chat_message", move |payload| {
            seen_for_handler
                .lock()
                .expect("lock")
                .push(payload["content"].as_str().unwrap_or_default().to_string());
        });

        emitter.emit(&frame("chat_message", json!({ "content": "hi" })));
        emitter.emit(&frame("mining_yield", json!({ "quantity": 5 })));

        assert_eq!(*seen.lock().expect("lock"), vec!["hi".to_string()]);
    }

    #[test]
    #[allow(clippy::panic)]
    fn throwing_listener_does_not_stop_other_consumers() {
        let mut emitter = TypedEmitter::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_for_handler = Arc::clone(&seen);
        emitter.on("chat_message", |_| std::panic::panic_any("boom"));
        emitter.on("chat_message", move |payload| {
            seen_for_handler
                .lock()
                .expect("lock")
                .push(payload["content"].as_str().unwrap_or_default().to_string());
        });
        let mut stream = emitter.stream("chat_message");

        emitter.emit(&frame("chat_message", json!({ "content": "hi" })));

        assert_eq!(*seen.lock().expect("lock"), vec!["hi".to_string()]);
        assert_eq!(stream.try_recv().expect("event")["content"], "hi");
    }

    #[test]
    fn unsubscribe_removes_listener() {
        let mut emitter = TypedEmitter::default();
        let count = Arc::new(Mutex::new(0));
        let count_for_handler = Arc::clone(&count);
        let id = emitter.on("chat_message", move |_| {
            *count_for_handler.lock().expect("lock") += 1;
        });

        emitter.emit(&frame("chat_message", json!({})));
        emitter.off(id);
        emitter.emit(&frame("chat_message", json!({})));

        assert_eq!(*count.lock().expect("lock"), 1);
    }
}
