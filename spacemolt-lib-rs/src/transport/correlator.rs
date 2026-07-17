//! Correlates server response frames to in-flight requests by `request_id`.

use std::collections::HashMap;

use serde_json::Value;
use tokio::sync::oneshot;

use crate::errors::{ClientError, ConnectionClosedError, SpacemoltError};
use crate::protocol::{MutationAck, MutationResult, QueryResult, RawFrame};

/// Request shape registered with the correlator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// Query resolves on the synchronous `result` frame.
    Query,
    /// Mutation resolves on `action_result`, unless the result frame has no
    /// pending marker and therefore represents a synchronous final outcome.
    Mutation,
}

enum Pending {
    Query {
        tx: oneshot::Sender<Result<QueryResult, ClientError>>,
    },
    Mutation {
        tx: oneshot::Sender<Result<MutationResult, ClientError>>,
        #[allow(dead_code)]
        ack: Option<MutationAck>,
        on_ack: Option<Box<dyn FnMut(MutationAck) + Send>>,
    },
}

/// Query result receiver returned by [`Correlator::await_query`].
pub type QueryReceiver = oneshot::Receiver<Result<QueryResult, ClientError>>;
/// Mutation result receiver returned by [`Correlator::await_mutation`].
pub type MutationReceiver = oneshot::Receiver<Result<MutationResult, ClientError>>;

/// In-flight request correlator.
#[derive(Default)]
pub struct Correlator {
    pending: HashMap<String, Pending>,
}

impl Correlator {
    /// Register a query request; resolves on its `result` frame.
    pub fn await_query(&mut self, request_id: impl Into<String>) -> QueryReceiver {
        let (tx, rx) = oneshot::channel();
        self.pending
            .insert(request_id.into(), Pending::Query { tx });
        rx
    }

    /// Register a mutation request; resolves on its mutation outcome.
    pub fn await_mutation(
        &mut self,
        request_id: impl Into<String>,
        on_ack: Option<Box<dyn FnMut(MutationAck) + Send>>,
    ) -> MutationReceiver {
        let (tx, rx) = oneshot::channel();
        self.pending.insert(
            request_id.into(),
            Pending::Mutation {
                tx,
                ack: None,
                on_ack,
            },
        );
        rx
    }

    /// True when a request id is in flight.
    pub fn has(&self, request_id: &str) -> bool {
        self.pending.contains_key(request_id)
    }

    /// Drop a pending request without settling it.
    pub fn cancel(&mut self, request_id: &str) {
        self.pending.remove(request_id);
    }

    /// Feed a frame to the correlator. Returns true if it matched an in-flight
    /// request and was consumed, false when callers should treat it as a push.
    pub fn handle(&mut self, frame: &RawFrame) -> bool {
        let Some(request_id) = frame.request_id.as_deref() else {
            return false;
        };
        if !self.pending.contains_key(request_id) {
            return false;
        }

        match frame.kind.as_str() {
            "result" => self.handle_result(request_id, frame),
            "action_result" => self.handle_action_result(request_id, frame),
            "action_error" => self.reject_action_error(request_id, frame),
            "error" => self.reject_error(request_id, frame),
            _ => false,
        }
    }

    /// Reject every in-flight request, used when the socket closes.
    pub fn reject_all_connection_closed(&mut self, err: ConnectionClosedError) {
        let pending = std::mem::take(&mut self.pending);
        for item in pending.into_values() {
            match item {
                Pending::Query { tx } => {
                    let _ = tx.send(Err(ClientError::ConnectionClosed(err.clone())));
                }
                Pending::Mutation { tx, .. } => {
                    let _ = tx.send(Err(ClientError::ConnectionClosed(err.clone())));
                }
            }
        }
    }

    /// Reject every in-flight request with a server error.
    pub fn reject_all_server_error(&mut self, err: SpacemoltError) {
        let pending = std::mem::take(&mut self.pending);
        for item in pending.into_values() {
            match item {
                Pending::Query { tx } => {
                    let _ = tx.send(Err(ClientError::Server(err.clone())));
                }
                Pending::Mutation { tx, .. } => {
                    let _ = tx.send(Err(ClientError::Server(err.clone())));
                }
            }
        }
    }

    fn handle_result(&mut self, request_id: &str, frame: &RawFrame) -> bool {
        let Some(payload) = frame.payload.as_ref() else {
            return true;
        };
        let result = payload.get("result").cloned().unwrap_or(Value::Null);
        let structured = payload
            .get("structuredContent")
            .or_else(|| payload.get("structured_content"))
            .cloned();

        match self.pending.remove(request_id) {
            Some(Pending::Query { tx }) => {
                let _ = tx.send(Ok(QueryResult {
                    result,
                    structured_content: structured,
                }));
                true
            }
            Some(Pending::Mutation {
                tx,
                ack: _,
                mut on_ack,
            }) => {
                let pending_ack = extract_pending_ack(structured.as_ref());
                if let Some(next_ack) = pending_ack {
                    if let Some(handler) = on_ack.as_mut() {
                        handler(next_ack.clone());
                    }
                    self.pending.insert(
                        request_id.to_string(),
                        Pending::Mutation {
                            tx,
                            ack: Some(next_ack),
                            on_ack,
                        },
                    );
                    return true;
                }

                let command = structured
                    .as_ref()
                    .and_then(|v| v.get("command"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let _ = tx.send(Ok(MutationResult {
                    command,
                    tick: 0,
                    delta: serde_json::json!({ "details": structured }),
                    auto_docked: false,
                    auto_undocked: false,
                }));
                true
            }
            None => false,
        }
    }

    fn handle_action_result(&mut self, request_id: &str, frame: &RawFrame) -> bool {
        let Some(Pending::Mutation { tx, .. }) = self.pending.remove(request_id) else {
            return true;
        };
        let payload = frame.payload.as_ref();
        let command = payload
            .and_then(|v| v.get("command"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let tick = payload
            .and_then(|v| v.get("tick"))
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let delta = payload
            .and_then(|v| v.get("result"))
            .cloned()
            .unwrap_or(Value::Null);
        let auto_docked = payload
            .and_then(|v| v.get("auto_docked"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let auto_undocked = payload
            .and_then(|v| v.get("auto_undocked"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let _ = tx.send(Ok(MutationResult {
            command,
            tick,
            delta,
            auto_docked,
            auto_undocked,
        }));
        true
    }

    fn reject_action_error(&mut self, request_id: &str, frame: &RawFrame) -> bool {
        let Some(pending) = self.pending.remove(request_id) else {
            return false;
        };
        let err = action_error_from_raw(frame);
        match pending {
            Pending::Query { tx } => {
                let _ = tx.send(Err(ClientError::Server(err)));
            }
            Pending::Mutation { tx, .. } => {
                let _ = tx.send(Err(ClientError::Server(err)));
            }
        }
        true
    }

    fn reject_error(&mut self, request_id: &str, frame: &RawFrame) -> bool {
        let Some(pending) = self.pending.remove(request_id) else {
            return false;
        };
        let err = error_from_raw(frame);
        match pending {
            Pending::Query { tx } => {
                let _ = tx.send(Err(ClientError::Server(err)));
            }
            Pending::Mutation { tx, .. } => {
                let _ = tx.send(Err(ClientError::Server(err)));
            }
        }
        true
    }
}

/// Extract a pending mutation ack from both known server shapes:
/// top-level `{ pending: true }` and nested `{ details: { pending: true } }`.
pub fn extract_pending_ack(structured: Option<&Value>) -> Option<MutationAck> {
    let structured = structured?;
    if structured
        .get("pending")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Some(MutationAck {
            pending: true,
            command: structured
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            message: structured
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    let details = structured.get("details")?;
    if !details
        .get("pending")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    Some(MutationAck {
        pending: true,
        command: details
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        message: details
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn error_from_raw(frame: &RawFrame) -> SpacemoltError {
    let payload = frame.payload.as_ref();
    SpacemoltError {
        code: payload
            .and_then(|v| v.get("code"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        message: payload
            .and_then(|v| v.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        details: payload.and_then(|v| v.get("details")).cloned(),
        request_id: frame.request_id.clone(),
        command: None,
        tick: None,
        pending_command: payload
            .and_then(|v| v.get("pending_command"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn action_error_from_raw(frame: &RawFrame) -> SpacemoltError {
    let payload = frame.payload.as_ref();
    SpacemoltError {
        code: payload
            .and_then(|v| v.get("code"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        message: payload
            .and_then(|v| v.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        details: payload.and_then(|v| v.get("details")).cloned(),
        request_id: frame.request_id.clone(),
        command: payload
            .and_then(|v| v.get("command"))
            .and_then(Value::as_str)
            .map(str::to_string),
        tick: payload.and_then(|v| v.get("tick")).and_then(Value::as_u64),
        pending_command: None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn frame(kind: &str, request_id: &str, payload: Value) -> RawFrame {
        RawFrame {
            kind: kind.to_string(),
            request_id: Some(request_id.to_string()),
            payload: Some(payload),
        }
    }

    #[tokio::test]
    async fn query_resolves_on_result_frame() {
        let mut correlator = Correlator::default();
        let rx = correlator.await_query("q1");
        assert!(correlator.handle(&frame(
            "result",
            "q1",
            json!({ "result": "ok", "structuredContent": { "answer": 42 } })
        )));

        let result = rx.await.expect("sender").expect("query");
        assert_eq!(result.result, json!("ok"));
        assert_eq!(result.structured_content, Some(json!({ "answer": 42 })));
        assert!(!correlator.has("q1"));
    }

    #[tokio::test]
    async fn query_resolves_snake_case_structured_content() {
        let mut correlator = Correlator::default();
        let rx = correlator.await_query("q1");
        assert!(correlator.handle(&frame(
            "result",
            "q1",
            json!({ "result": "ok", "structured_content": { "answer": 42 } })
        )));

        let result = rx.await.expect("sender").expect("query");
        assert_eq!(result.result, json!("ok"));
        assert_eq!(result.structured_content, Some(json!({ "answer": 42 })));
        assert!(!correlator.has("q1"));
    }

    #[tokio::test]
    async fn mutation_waits_after_top_level_pending_ack() {
        let mut correlator = Correlator::default();
        let rx = correlator.await_mutation("m1", None);
        assert!(correlator.handle(&frame(
            "result",
            "m1",
            json!({
                "result": "pending",
                "structuredContent": { "pending": true, "command": "mine", "message": "queued" }
            })
        )));
        assert!(correlator.has("m1"));

        assert!(correlator.handle(&frame(
            "action_result",
            "m1",
            json!({ "command": "mine", "tick": 7, "result": { "cargo": [] } })
        )));
        let result = rx.await.expect("sender").expect("mutation");
        assert_eq!(result.command, "mine");
        assert_eq!(result.tick, 7);
        assert_eq!(result.delta, json!({ "cargo": [] }));
    }

    #[tokio::test]
    async fn mutation_waits_after_nested_pending_ack() {
        let mut correlator = Correlator::default();
        let rx = correlator.await_mutation("m2", None);
        assert!(correlator.handle(&frame(
            "result",
            "m2",
            json!({
                "result": "pending",
                "structuredContent": {
                    "details": { "pending": true, "command": "jump", "message": "queued" },
                    "location": {}
                }
            })
        )));
        assert!(correlator.has("m2"));
        assert!(correlator.handle(&frame(
            "action_result",
            "m2",
            json!({ "command": "jump", "tick": 8, "result": { "location": { "system_id": "sol" } } })
        )));
        let result = rx.await.expect("sender").expect("mutation");
        assert_eq!(result.command, "jump");
        assert_eq!(result.delta, json!({ "location": { "system_id": "sol" } }));
    }

    #[tokio::test]
    async fn mutation_result_without_pending_resolves_synchronously() {
        let mut correlator = Correlator::default();
        let rx = correlator.await_mutation("m3", None);
        assert!(correlator.handle(&frame(
            "result",
            "m3",
            json!({ "result": "quote", "structuredContent": { "dry_run": true } })
        )));
        let result = rx.await.expect("sender").expect("mutation");
        assert_eq!(result.tick, 0);
        assert_eq!(result.delta, json!({ "details": { "dry_run": true } }));
    }
}
