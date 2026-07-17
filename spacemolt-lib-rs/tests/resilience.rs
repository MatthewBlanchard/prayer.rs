mod mock_socket;

use std::time::{Duration, Instant};

use mock_socket::{MockSocket, MockSocketFactory};
use serde_json::json;
use spacemolt_lib_rs::account::{Account, AccountOptions, LoginParams, ReconnectOptions};
use spacemolt_lib_rs::auth::AuthCredentials;
use spacemolt_lib_rs::errors::ClientError;
use spacemolt_lib_rs::protocol::{RawFrame, WelcomePayload};
use std::sync::Arc;

fn welcome_payload() -> WelcomePayload {
    WelcomePayload {
        version: "0.452.0".to_string(),
        release_date: "2026-06-20".to_string(),
        release_notes: Vec::new(),
        tick_rate: 5,
        current_tick: 1,
        server_time: 1,
        motd: None,
        game_info: String::new(),
        website: String::new(),
        help_text: String::new(),
        terms: String::new(),
    }
}

async fn connected(opts: AccountOptions) -> (Account, MockSocket) {
    let factory = Arc::new(MockSocketFactory::new());
    let account = Account::with_socket_factory(opts, factory.clone());
    account.connect().await.expect("connect");
    let socket = factory.latest();
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    account.wait_for_welcome().await.expect("welcome");
    (account, socket)
}

async fn connected_with_factory(
    opts: AccountOptions,
    factory: Arc<MockSocketFactory>,
) -> (Account, MockSocket) {
    let account = Account::with_socket_factory(opts, factory.clone());
    account.connect().await.expect("connect");
    let socket = factory.latest();
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    account.wait_for_welcome().await.expect("welcome");
    (account, socket)
}

async fn wait_for_socket_count(factory: &MockSocketFactory, len: usize) {
    for _ in 0..200 {
        if factory.len() >= len {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {len} sockets; saw {}", factory.len());
}

fn reconnect_opts(base_delay_ms: u64) -> ReconnectOptions {
    ReconnectOptions {
        max_retries: Some(3),
        base_delay_ms,
        max_delay_ms: 10_000,
    }
}

fn reconnect_account_options(reconnect: ReconnectOptions) -> AccountOptions {
    AccountOptions {
        url: "ws://mock/ws/v2".to_string(),
        seed_state: false,
        reconnect: Some(reconnect),
        credentials: Some(AuthCredentials::Login {
            username: "Nova".to_string(),
            password: "pw".to_string(),
        }),
        ..timeout_opts()
    }
}

async fn log_in(account: &Account, socket: &MockSocket) {
    let login = account.login(LoginParams {
        username: "Nova".to_string(),
        password: "pw".to_string(),
    });
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({ "player": { "username": "Nova" } })),
    });
    login.await.expect("login");
}

async fn serve_reconnect(factory: &MockSocketFactory, index: usize) -> MockSocket {
    wait_for_socket_count(factory, index + 1).await;
    let socket = factory.get(index);
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    wait_for_sent_len(&socket, 1).await;
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({ "player": { "username": "Nova" } })),
    });
    socket
}

fn timeout_opts() -> AccountOptions {
    AccountOptions {
        url: "ws://mock/ws/v2".to_string(),
        seed_state: false,
        query_timeout_ms: 20,
        mutation_timeout_ms: 60,
        fast_mutation_timeout_ms: 20,
        ..AccountOptions::default()
    }
}

#[tokio::test]
async fn reconnects_and_reauthenticates_after_unexpected_close() {
    let factory = Arc::new(MockSocketFactory::new());
    let (account, socket) = connected_with_factory(
        reconnect_account_options(reconnect_opts(1)),
        factory.clone(),
    )
    .await;
    log_in(&account, &socket).await;
    assert!(account.authenticated());
    let reconnected = Arc::new(tokio::sync::Notify::new());
    let reconnected_for_handler = Arc::clone(&reconnected);
    account.on_reconnected(move || reconnected_for_handler.notify_one());

    socket.close(1006, "abnormal");
    serve_reconnect(&factory, 1).await;
    tokio::time::timeout(Duration::from_millis(500), reconnected.notified())
        .await
        .expect("reconnected notification");

    assert!(account.authenticated());
    assert_eq!(factory.len(), 2);
}

#[tokio::test]
async fn does_not_reconnect_on_session_replaced_close() {
    let factory = Arc::new(MockSocketFactory::new());
    let (account, socket) = connected_with_factory(
        reconnect_account_options(reconnect_opts(1)),
        factory.clone(),
    )
    .await;
    log_in(&account, &socket).await;
    let disconnected = Arc::new(tokio::sync::Notify::new());
    let code_seen = Arc::new(std::sync::Mutex::new(None));
    let disconnected_for_handler = Arc::clone(&disconnected);
    let code_for_handler = Arc::clone(&code_seen);
    account.on_disconnected(move |err| {
        *code_for_handler.lock().expect("code") = err.code;
        disconnected_for_handler.notify_one();
    });

    socket.close(4001, "session_replaced");
    tokio::time::timeout(Duration::from_millis(500), disconnected.notified())
        .await
        .expect("disconnected notification");
    tokio::time::sleep(Duration::from_millis(30)).await;

    assert_eq!(*code_seen.lock().expect("code"), Some(4001));
    assert_eq!(factory.len(), 1);
    assert!(!account.authenticated());
}

#[tokio::test]
async fn connection_rate_limited_reconnect_honors_retry_after_hint() {
    let factory = Arc::new(MockSocketFactory::new());
    let (account, socket) = connected_with_factory(
        reconnect_account_options(reconnect_opts(5_000)),
        factory.clone(),
    )
    .await;
    log_in(&account, &socket).await;
    let reconnected = Arc::new(tokio::sync::Notify::new());
    let reconnected_for_handler = Arc::clone(&reconnected);
    account.on_reconnected(move || reconnected_for_handler.notify_one());

    let start = Instant::now();
    socket.close(4003, "connection_rate_limited retry_after=1");
    wait_for_socket_count(&factory, 2).await;
    assert!(start.elapsed() >= Duration::from_millis(900));
    assert!(start.elapsed() < Duration::from_millis(3_000));
    serve_reconnect(&factory, 1).await;
    tokio::time::timeout(Duration::from_millis(500), reconnected.notified())
        .await
        .expect("reconnected notification");

    assert!(account.authenticated());
}

async fn wait_for_sent_len(socket: &MockSocket, len: usize) {
    for _ in 0..100 {
        if socket.sent().len() >= len {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "timed out waiting for {len} sent frames; saw {}",
        socket.sent().len()
    );
}

#[tokio::test]
async fn welcome_timeout_closes_stalled_connection() {
    let factory = Arc::new(MockSocketFactory::default());
    let account = Account::with_socket_factory(
        AccountOptions {
            connect_timeout_ms: 10,
            ..timeout_opts()
        },
        factory.clone(),
    );
    account.connect().await.expect("socket connect");
    let err = account
        .wait_for_welcome()
        .await
        .expect_err("welcome timeout");
    assert!(matches!(err, ClientError::Timeout(_)));
}

#[tokio::test]
async fn auth_timeout_clears_pending_exchange_for_a_retry() {
    let (account, socket) = connected(AccountOptions {
        connect_timeout_ms: 10,
        ..timeout_opts()
    })
    .await;
    let first = account
        .login(LoginParams {
            username: "Nova".into(),
            password: "secret".into(),
        })
        .await
        .expect_err("auth timeout");
    assert!(matches!(first, ClientError::Timeout(_)));

    let second = account.login(LoginParams {
        username: "Nova".into(),
        password: "secret".into(),
    });
    wait_for_sent_len(&socket, 2).await;
    socket.server_send(RawFrame {
        kind: "logged_in".into(),
        request_id: None,
        payload: Some(serde_json::json!({"player": {"username": "Nova"}})),
    });
    second.await.expect("second auth is not blocked");
}

#[tokio::test]
async fn query_retries_after_rate_limited_error() {
    let (account, socket) = connected(AccountOptions {
        max_rate_limit_retries: 2,
        ..timeout_opts()
    })
    .await;

    let account_for_query = account.clone();
    let query = tokio::spawn(async move {
        account_for_query
            .query("spacemolt", "get_status", None)
            .await
    });
    wait_for_sent_len(&socket, 1).await;
    let first_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(first_request_id),
        payload: Some(json!({
            "code": "rate_limited",
            "message": "Too many requests. Retry in 0 seconds."
        })),
    });
    wait_for_sent_len(&socket, 2).await;

    let second_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({ "result": "ok" })),
    });

    let result = query.await.expect("task").expect("query");
    assert_eq!(result.result, json!("ok"));
    assert_eq!(socket.sent().len(), 2);
}

#[tokio::test]
async fn query_retry_delay_uses_structured_retry_after_details() {
    let (account, socket) = connected(AccountOptions {
        max_rate_limit_retries: 2,
        ..timeout_opts()
    })
    .await;

    let account_for_query = account.clone();
    let query = tokio::spawn(async move {
        account_for_query
            .query("spacemolt", "get_status", None)
            .await
    });
    wait_for_sent_len(&socket, 1).await;
    let first_request_id = socket.last_request_id();
    let start = Instant::now();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(first_request_id),
        payload: Some(json!({
            "code": "rate_limited",
            "message": "Too many requests.",
            "details": { "retry_after": 0 }
        })),
    });
    wait_for_sent_len(&socket, 2).await;
    assert!(start.elapsed() < Duration::from_millis(800));

    let second_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({ "result": "ok" })),
    });

    let result = query.await.expect("task").expect("query");
    assert_eq!(result.result, json!("ok"));
}

#[tokio::test]
async fn login_retries_after_rate_limited_error() {
    let (account, socket) = connected(AccountOptions {
        max_rate_limit_retries: 2,
        ..timeout_opts()
    })
    .await;

    let account_for_login = account.clone();
    let login = tokio::spawn(async move {
        account_for_login
            .login(spacemolt_lib_rs::account::LoginParams {
                username: "Nova".to_string(),
                password: "pw".to_string(),
            })
            .await
    });
    wait_for_sent_len(&socket, 1).await;
    let first_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(first_request_id),
        payload: Some(json!({
            "code": "rate_limited",
            "message": "Too many requests. Retry in 0 seconds."
        })),
    });
    wait_for_sent_len(&socket, 2).await;

    let second_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({ "player": { "username": "Nova" } })),
    });

    login.await.expect("task").expect("login");
    assert!(account.authenticated());
    assert_eq!(socket.sent().len(), 2);
}

#[tokio::test]
async fn mutation_retries_after_rate_limited_error_without_releasing_lane() {
    let (account, socket) = connected(AccountOptions {
        max_rate_limit_retries: 2,
        ..timeout_opts()
    })
    .await;

    let account_for_first = account.clone();
    let first = tokio::spawn(async move {
        account_for_first
            .mutate("spacemolt", "jump", Some(json!({ "target_system": "a" })))
            .await
    });
    let account_for_second = account.clone();
    let second = tokio::spawn(async move {
        account_for_second
            .mutate("spacemolt", "jump", Some(json!({ "target_system": "b" })))
            .await
    });
    tokio::task::yield_now().await;
    assert_eq!(socket.sent().len(), 1);

    let first_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(first_request_id),
        payload: Some(json!({
            "code": "rate_limited",
            "message": "Too many requests. Retry in 0 seconds."
        })),
    });
    wait_for_sent_len(&socket, 2).await;
    assert_eq!(
        socket.sent()[1].payload,
        Some(json!({ "target_system": "a" }))
    );

    let retry_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(retry_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(retry_request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 1,
            "result": { "location": { "system_id": "a" } }
        })),
    });
    first.await.expect("task").expect("first mutation");
    wait_for_sent_len(&socket, 3).await;

    let second_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(second_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 2,
            "result": { "location": { "system_id": "b" } }
        })),
    });
    second.await.expect("task").expect("second mutation");
}

#[tokio::test]
async fn rate_limited_error_stops_after_configured_retry_count() {
    let (account, socket) = connected(AccountOptions {
        max_rate_limit_retries: 1,
        ..timeout_opts()
    })
    .await;

    let account_for_query = account.clone();
    let query = tokio::spawn(async move {
        account_for_query
            .query("spacemolt", "get_status", None)
            .await
    });
    wait_for_sent_len(&socket, 1).await;
    let first_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(first_request_id),
        payload: Some(json!({
            "code": "rate_limited",
            "message": "Too many requests. Retry in 0 seconds."
        })),
    });
    wait_for_sent_len(&socket, 2).await;

    let second_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({
            "code": "rate_limited",
            "message": "Too many requests. Retry in 0 seconds."
        })),
    });

    match query.await.expect("task").expect_err("rate limited") {
        ClientError::Server(err) => assert_eq!(err.code, "rate_limited"),
        other => panic!("expected server error, got {other:?}"),
    }
    assert_eq!(socket.sent().len(), 2);
}

#[tokio::test]
async fn query_times_out_and_cancels_late_response() {
    let (account, socket) = connected(timeout_opts()).await;

    let result = tokio::time::timeout(
        Duration::from_millis(100),
        account.query("spacemolt", "get_status", None),
    )
    .await
    .expect("query should time itself out");

    match result.expect_err("query timeout") {
        ClientError::Timeout(message) => {
            assert!(message.contains("No response to spacemolt/get_status within 20ms"));
        }
        other => panic!("expected timeout, got {other:?}"),
    }

    let late_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(late_request_id),
        payload: Some(json!({ "result": "too late" })),
    });
}

#[tokio::test]
async fn mutation_times_out_before_pending_ack_and_releases_later_mutations() {
    let (account, socket) = connected(timeout_opts()).await;

    let first = tokio::time::timeout(
        Duration::from_millis(100),
        account.mutate("spacemolt", "jump", Some(json!({ "target_system": "a" }))),
    )
    .await
    .expect("mutation should time itself out");

    match first.expect_err("mutation timeout") {
        ClientError::Timeout(message) => {
            assert!(message.contains("No response to mutation"));
            assert!(message.contains("within 20ms"));
        }
        other => panic!("expected timeout, got {other:?}"),
    }

    let late_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(late_request_id),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "late" }
        })),
    });

    let second = account.mutate("spacemolt", "jump", Some(json!({ "target_system": "b" })));
    let second_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(second_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 2,
            "result": { "location": { "system_id": "b" } }
        })),
    });

    let result = second.await.expect("second mutation");
    assert_eq!(result.delta["location"]["system_id"], json!("b"));
}

#[tokio::test]
async fn mutation_times_out_after_pending_ack_and_late_result_does_not_wedge_lane() {
    let (account, socket) = connected(timeout_opts()).await;

    let first = account.mutate("spacemolt", "mine", None);
    let first_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(first_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "mine", "message": "queued" }
        })),
    });

    let result = tokio::time::timeout(Duration::from_millis(100), first)
        .await
        .expect("mutation should time itself out");
    match result.expect_err("mutation timeout") {
        ClientError::Timeout(message) => {
            assert!(message.contains("No action_result for mutation"));
            assert!(message.contains("within 20ms of its ack"));
        }
        other => panic!("expected timeout, got {other:?}"),
    }

    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(first_request_id),
        payload: Some(json!({
            "command": "mine",
            "tick": 5,
            "result": { "cargo": { "ore": 1 } }
        })),
    });

    let second = account.mutate("spacemolt", "jump", Some(json!({ "target_system": "b" })));
    let second_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(second_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 6,
            "result": { "location": { "system_id": "b" } }
        })),
    });

    let result = second.await.expect("second mutation");
    assert_eq!(result.delta["location"]["system_id"], json!("b"));
}

#[tokio::test]
async fn mutations_are_serialized_per_account() {
    let (account, socket) = connected(timeout_opts()).await;

    let first = account.mutate("spacemolt", "jump", Some(json!({ "target_system": "a" })));
    let account_for_second = account.clone();
    let second = tokio::spawn(async move {
        account_for_second
            .mutate("spacemolt", "jump", Some(json!({ "target_system": "b" })))
            .await
    });
    tokio::task::yield_now().await;

    let jumps = || {
        socket
            .sent()
            .into_iter()
            .filter(|frame| frame.action == "jump")
            .collect::<Vec<_>>()
    };
    assert_eq!(jumps().len(), 1);

    let first_request_id = jumps()[0].request_id.clone().expect("request id");
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(first_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(first_request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 1,
            "result": { "location": { "system_id": "a" } }
        })),
    });
    first.await.expect("first mutation");
    tokio::task::yield_now().await;

    assert_eq!(jumps().len(), 2);
    let second_request_id = jumps()[1].request_id.clone().expect("request id");
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(second_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 2,
            "result": { "location": { "system_id": "b" } }
        })),
    });

    let second_result = second.await.expect("task").expect("second mutation");
    assert_eq!(second_result.delta["location"]["system_id"], json!("b"));
}

#[tokio::test]
async fn cancelling_active_mutation_releases_next_waiter() {
    let (account, socket) = connected(timeout_opts()).await;

    let first = account.mutate("spacemolt", "jump", Some(json!({ "target_system": "a" })));
    assert_eq!(
        socket
            .sent()
            .iter()
            .filter(|frame| frame.action == "jump")
            .count(),
        1
    );

    let account_for_second = account.clone();
    let second = tokio::spawn(async move {
        account_for_second
            .mutate("spacemolt", "jump", Some(json!({ "target_system": "b" })))
            .await
    });
    tokio::task::yield_now().await;

    // Simulate the script runner dropping its in-flight command on halt.
    drop(first);
    tokio::task::yield_now().await;

    let jumps = socket
        .sent()
        .into_iter()
        .filter(|frame| frame.action == "jump")
        .collect::<Vec<_>>();
    assert_eq!(
        jumps.len(),
        2,
        "the queued mutation should start after cancellation"
    );

    let second_request_id = jumps[1].request_id.clone().expect("request id");
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(second_request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(second_request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 2,
            "result": { "location": { "system_id": "b" } }
        })),
    });

    let result = second.await.expect("task").expect("second mutation");
    assert_eq!(result.delta["location"]["system_id"], json!("b"));
}
