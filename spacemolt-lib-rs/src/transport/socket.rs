//! Injectable socket boundary for account transport behavior.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error as TungsteniteError, Message};

use crate::errors::ConnectionClosedError;
use crate::protocol::{InboundFrame, RawFrame};

/// Future returned by a socket factory while opening one connection.
pub type BoxedConnect =
    Pin<Box<dyn Future<Output = Result<Arc<dyn SocketHandle>, ConnectionClosedError>> + Send>>;

/// Minimal socket handle the account needs after connection.
pub trait SocketHandle: Send + Sync {
    /// Serialize and send one client frame.
    fn send(&self, frame: InboundFrame) -> Result<(), ConnectionClosedError>;

    /// Close the socket.
    fn close(&self);
}

/// Factory used by accounts to create sockets.
pub trait SocketFactory: Send + Sync {
    /// Open a socket for `url` and wire account callbacks.
    fn connect(&self, url: String, callbacks: SocketCallbacks) -> BoxedConnect;
}

/// Tokio/tungstenite production WebSocket factory.
#[derive(Debug, Clone, Default)]
pub struct TokioWebSocketFactory;

impl SocketFactory for TokioWebSocketFactory {
    fn connect(&self, url: String, callbacks: SocketCallbacks) -> BoxedConnect {
        Box::pin(async move {
            let (stream, _) = tokio_tungstenite::connect_async(url.as_str())
                .await
                .map_err(connection_error_from_tungstenite)?;
            let (mut write, mut read) = stream.split();
            let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
            let closed = Arc::new(AtomicBool::new(false));

            let callbacks_for_read = callbacks.clone();
            let closed_for_read = Arc::clone(&closed);
            tokio::spawn(async move {
                while let Some(next) = read.next().await {
                    match next {
                        Ok(Message::Text(text)) => callbacks_for_read.raw_message(&text),
                        Ok(Message::Binary(bytes)) => {
                            if let Ok(text) = std::str::from_utf8(&bytes) {
                                callbacks_for_read.raw_message(text);
                            }
                        }
                        Ok(Message::Close(frame)) => {
                            notify_close_once(
                                &callbacks_for_read,
                                &closed_for_read,
                                close_error_from_frame(frame),
                            );
                            return;
                        }
                        Ok(Message::Ping(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {}
                        Err(err) => {
                            notify_close_once(
                                &callbacks_for_read,
                                &closed_for_read,
                                connection_error_from_tungstenite(err),
                            );
                            return;
                        }
                    }
                }
                notify_close_once(
                    &callbacks_for_read,
                    &closed_for_read,
                    ConnectionClosedError::new("WebSocket connection closed", None, None),
                );
            });

            let callbacks_for_write = callbacks;
            let closed_for_write = Arc::clone(&closed);
            tokio::spawn(async move {
                while let Some(message) = rx.recv().await {
                    if let Err(err) = write.send(message).await {
                        notify_close_once(
                            &callbacks_for_write,
                            &closed_for_write,
                            connection_error_from_tungstenite(err),
                        );
                        return;
                    }
                }
            });

            Ok(Arc::new(TokioWebSocketHandle { tx }) as Arc<dyn SocketHandle>)
        })
    }
}

struct TokioWebSocketHandle {
    tx: mpsc::UnboundedSender<Message>,
}

impl SocketHandle for TokioWebSocketHandle {
    fn send(&self, frame: InboundFrame) -> Result<(), ConnectionClosedError> {
        let text = serde_json::to_string(&frame).map_err(|err| {
            ConnectionClosedError::new(format!("failed to serialize frame: {err}"), None, None)
        })?;
        self.tx
            .send(Message::Text(text))
            .map_err(|_| ConnectionClosedError::new("cannot send on a closed socket", None, None))
    }

    fn close(&self) {
        let _ = self.tx.send(Message::Close(None));
    }
}

/// Callbacks a socket uses to report inbound frames and closure.
#[derive(Clone)]
pub struct SocketCallbacks {
    on_frame: Arc<dyn Fn(RawFrame) + Send + Sync>,
    on_close: Arc<dyn Fn(ConnectionClosedError) + Send + Sync>,
}

impl SocketCallbacks {
    /// Build a callback pair.
    pub fn new(
        on_frame: impl Fn(RawFrame) + Send + Sync + 'static,
        on_close: impl Fn(ConnectionClosedError) + Send + Sync + 'static,
    ) -> Self {
        Self {
            on_frame: Arc::new(on_frame),
            on_close: Arc::new(on_close),
        }
    }

    /// Route an already parsed frame.
    pub fn frame(&self, frame: RawFrame) {
        (self.on_frame)(frame);
    }

    /// Parse one WebSocket text message as newline-delimited JSON frames.
    pub fn raw_message(&self, data: &str) {
        for line in data.split('\n') {
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<RawFrame>(line) {
                Ok(frame) => self.frame(frame),
                Err(err) => {
                    let head = sample_head(line, 200);
                    let tail = if line.chars().count() > 400 {
                        Some(sample_tail(line, 200))
                    } else {
                        None
                    };
                    if let Some(tail) = tail.as_deref() {
                        eprintln!(
                            "[spacemolt] dropped unparseable frame ({} bytes): {err} | head={head:?} tail={tail:?}",
                            line.len()
                        );
                    } else {
                        eprintln!(
                            "[spacemolt] dropped unparseable frame ({} bytes): {err} | head={head:?}",
                            line.len()
                        );
                    }
                }
            }
        }
    }

    /// Notify account runtime that the socket closed.
    pub fn close(&self, err: ConnectionClosedError) {
        (self.on_close)(err);
    }
}

fn sample_head(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn sample_tail(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars().rev().take(max_chars).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

fn notify_close_once(callbacks: &SocketCallbacks, closed: &AtomicBool, err: ConnectionClosedError) {
    if !closed.swap(true, Ordering::SeqCst) {
        callbacks.close(err);
    }
}

fn connection_error_from_tungstenite(err: TungsteniteError) -> ConnectionClosedError {
    ConnectionClosedError::new(err.to_string(), None, None)
}

fn close_error_from_frame(frame: Option<CloseFrame<'static>>) -> ConnectionClosedError {
    let code = frame
        .as_ref()
        .and_then(|frame| close_code_as_u16(frame.code));
    let reason = frame.map(|frame| frame.reason.to_string());
    ConnectionClosedError::new("WebSocket connection closed", code, reason)
}

fn close_code_as_u16(code: CloseCode) -> Option<u16> {
    match code {
        CloseCode::Normal => Some(1000),
        CloseCode::Away => Some(1001),
        CloseCode::Protocol => Some(1002),
        CloseCode::Unsupported => Some(1003),
        CloseCode::Status => Some(1005),
        CloseCode::Abnormal => Some(1006),
        CloseCode::Invalid => Some(1007),
        CloseCode::Policy => Some(1008),
        CloseCode::Size => Some(1009),
        CloseCode::Extension => Some(1010),
        CloseCode::Error => Some(1011),
        CloseCode::Restart => Some(1012),
        CloseCode::Again => Some(1013),
        CloseCode::Tls => Some(1015),
        CloseCode::Library(value) | CloseCode::Iana(value) | CloseCode::Bad(value) => Some(value),
        CloseCode::Reserved(_) => None,
    }
}
