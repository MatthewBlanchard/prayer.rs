use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use spacemolt_lib_rs::account::{Account, AccountOptions};
use spacemolt_lib_rs::protocol::InboundFrame;
use spacemolt_lib_rs::transport::socket::{SocketCallbacks, SocketFactory, TokioWebSocketFactory};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};

#[tokio::test]
async fn account_new_uses_the_production_socket_factory() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept ws");
        socket
            .send(Message::Text(
                json!({
                    "type": "welcome",
                    "payload": {
                        "version": "0.452.0",
                        "release_date": "2026-06-20",
                        "release_notes": [],
                        "tick_rate": 5,
                        "current_tick": 1,
                        "server_time": 1,
                        "game_info": "",
                        "website": "",
                        "help_text": "",
                        "terms": ""
                    }
                })
                .to_string(),
            ))
            .await
            .expect("welcome");
    });

    let account = Account::new(AccountOptions {
        url: format!("ws://{addr}"),
        seed_state: false,
        ..AccountOptions::default()
    });

    account.connect().await.expect("connect");
    let welcome = account.wait_for_welcome().await.expect("welcome");
    assert_eq!(welcome.version, "0.452.0");
}

#[tokio::test]
async fn production_socket_sends_frames_and_routes_newline_messages() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept ws");
        let sent = socket
            .next()
            .await
            .expect("client message")
            .expect("message");
        let text = sent.into_text().expect("text");
        socket
            .send(Message::Text(
                json!({ "type": "welcome", "payload": { "version": "test" } }).to_string()
                    + "\n"
                    + &json!({ "type": "chat_message", "payload": { "content": "hi" } })
                        .to_string(),
            ))
            .await
            .expect("send frames");
        text
    });

    let (tx, mut rx) = mpsc::unbounded_channel();
    let callbacks = SocketCallbacks::new(
        move |frame| {
            tx.send(frame).expect("frame send");
        },
        |_| {},
    );
    let factory = TokioWebSocketFactory::default();
    let socket = factory
        .connect(format!("ws://{addr}"), callbacks)
        .await
        .expect("connect");

    socket
        .send(InboundFrame {
            tool: "spacemolt".to_string(),
            action: "get_status".to_string(),
            payload: None,
            request_id: Some("r1".to_string()),
        })
        .expect("send");

    let text = server.await.expect("server");
    let outbound: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(outbound["tool"], "spacemolt");
    assert_eq!(outbound["action"], "get_status");
    assert_eq!(outbound["request_id"], "r1");

    let first = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("first frame")
        .expect("frame");
    let second = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("second frame")
        .expect("frame");
    assert_eq!(first.kind, "welcome");
    assert_eq!(second.kind, "chat_message");
}

#[tokio::test]
async fn production_socket_reports_peer_close_code_and_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept ws");
        socket
            .close(Some(CloseFrame {
                code: CloseCode::Library(4001),
                reason: "session_replaced".into(),
            }))
            .await
            .expect("close");
    });

    let closed = Arc::new(Mutex::new(None));
    let closed_for_callback = Arc::clone(&closed);
    let callbacks = SocketCallbacks::new(
        |_| {},
        move |err| {
            *closed_for_callback.lock().expect("closed") = Some(err);
        },
    );
    let factory = TokioWebSocketFactory::default();
    let _socket = factory
        .connect(format!("ws://{addr}"), callbacks)
        .await
        .expect("connect");

    for _ in 0..100 {
        if closed.lock().expect("closed").is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let err = closed.lock().expect("closed").clone().expect("closed");
    assert_eq!(err.code, Some(4001));
    assert_eq!(err.reason.as_deref(), Some("session_replaced"));
}
