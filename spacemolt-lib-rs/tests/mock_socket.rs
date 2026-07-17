#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use spacemolt_lib_rs::errors::ConnectionClosedError;
use spacemolt_lib_rs::protocol::{InboundFrame, RawFrame};
use spacemolt_lib_rs::transport::socket::{
    BoxedConnect, SocketCallbacks, SocketFactory, SocketHandle,
};

#[derive(Clone, Default)]
pub struct MockSocketFactory {
    sockets: Arc<Mutex<Vec<MockSocket>>>,
}

impl MockSocketFactory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn latest(&self) -> MockSocket {
        self.sockets
            .lock()
            .expect("sockets")
            .last()
            .expect("socket")
            .clone()
    }

    pub fn len(&self) -> usize {
        self.sockets.lock().expect("sockets").len()
    }

    pub fn get(&self, index: usize) -> MockSocket {
        self.sockets.lock().expect("sockets")[index].clone()
    }
}

impl SocketFactory for MockSocketFactory {
    fn connect(&self, url: String, callbacks: SocketCallbacks) -> BoxedConnect {
        let socket = MockSocket::new(url, callbacks);
        self.sockets.lock().expect("sockets").push(socket.clone());
        Box::pin(async move { Ok(Arc::new(socket) as Arc<dyn SocketHandle>) })
    }
}

#[derive(Clone)]
pub struct MockSocket {
    inner: Arc<MockSocketInner>,
}

struct MockSocketInner {
    url: String,
    sent: Mutex<Vec<InboundFrame>>,
    callbacks: SocketCallbacks,
    closed: Mutex<bool>,
}

impl MockSocket {
    fn new(url: String, callbacks: SocketCallbacks) -> Self {
        Self {
            inner: Arc::new(MockSocketInner {
                url,
                sent: Mutex::new(Vec::new()),
                callbacks,
                closed: Mutex::new(false),
            }),
        }
    }

    pub fn url(&self) -> &str {
        &self.inner.url
    }

    pub fn sent(&self) -> Vec<InboundFrame> {
        self.inner.sent.lock().expect("sent").clone()
    }

    pub fn last_request_id(&self) -> String {
        self.sent()
            .last()
            .and_then(|frame| frame.request_id.clone())
            .expect("request id")
    }

    pub fn server_send(&self, frame: RawFrame) {
        self.inner.callbacks.frame(frame);
    }

    pub fn server_send_raw(&self, data: &str) {
        self.inner.callbacks.raw_message(data);
    }

    pub fn close(&self, code: u16, reason: &str) {
        let mut closed = self.inner.closed.lock().expect("closed");
        if *closed {
            return;
        }
        *closed = true;
        self.inner.callbacks.close(ConnectionClosedError::new(
            "WebSocket connection closed",
            Some(code),
            Some(reason.to_string()),
        ));
    }
}

impl SocketHandle for MockSocket {
    fn send(&self, frame: InboundFrame) -> Result<(), ConnectionClosedError> {
        if *self.inner.closed.lock().expect("closed") {
            return Err(ConnectionClosedError::new(
                "cannot send on a closed socket",
                None,
                None,
            ));
        }
        self.inner.sent.lock().expect("sent").push(frame);
        Ok(())
    }

    fn close(&self) {
        MockSocket::close(self, 1000, "");
    }
}
