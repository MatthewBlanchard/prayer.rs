mod mock_socket;

use std::sync::{Arc, Mutex};

use mock_socket::{MockSocket, MockSocketFactory};
use serde_json::{json, Value};
use spacemolt_lib_rs::account::{
    Account, AccountOptions, CommandResult, LoginParams, RegisterParams,
};
use spacemolt_lib_rs::errors::ClientError;
use spacemolt_lib_rs::events::ListenerId;
use spacemolt_lib_rs::protocol::{RawFrame, StateSection, WelcomePayload};

fn welcome_payload() -> WelcomePayload {
    WelcomePayload {
        version: "0.452.0".to_string(),
        release_date: "2026-06-20".to_string(),
        release_notes: Vec::new(),
        tick_rate: 5,
        current_tick: 1,
        server_time: 1_750_860_000,
        motd: None,
        game_info: "test".to_string(),
        website: "https://www.spacemolt.com".to_string(),
        help_text: "help".to_string(),
        terms: "terms".to_string(),
    }
}

async fn connected() -> (Account, MockSocket) {
    let factory = Arc::new(MockSocketFactory::new());
    let account = Account::with_socket_factory(
        AccountOptions {
            url: "ws://mock/ws/v2".to_string(),
            seed_state: false,
            ..AccountOptions::default()
        },
        factory.clone(),
    );
    account.connect().await.expect("connect");
    let socket = factory.latest();
    assert_eq!(socket.url(), "ws://mock/ws/v2");
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    account.wait_for_welcome().await.expect("welcome");
    (account, socket)
}

async fn wait_for_sent_len(socket: &MockSocket, len: usize) {
    for _ in 0..100 {
        if socket.sent().len() >= len {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!(
        "timed out waiting for {len} sent frames; saw {}",
        socket.sent().len()
    );
}

#[tokio::test]
async fn connect_records_welcome_payload() {
    let (account, _) = connected().await;

    assert_eq!(account.welcome().expect("welcome").version, "0.452.0");
    assert_eq!(account.welcome().expect("welcome").tick_rate, 5);
}

#[tokio::test]
async fn query_sends_frame_and_resolves_on_result() {
    let (account, socket) = connected().await;

    let pending = account.query("spacemolt", "get_status", None);
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "You are in Sol.",
            "structuredContent": { "credits": 5000 }
        })),
    });

    let res = pending.await.expect("query");
    let sent = socket.sent();
    assert_eq!(sent[0].tool, "spacemolt");
    assert_eq!(sent[0].action, "get_status");
    assert_eq!(res.result, json!("You are in Sol."));
    assert_eq!(res.structured_content, Some(json!({ "credits": 5000 })));
}

#[tokio::test]
async fn mutation_fires_ack_applies_delta_before_resolving() {
    let (account, socket) = connected().await;
    let ack_seen = Arc::new(Mutex::new(None));
    let ack_for_callback = Arc::clone(&ack_seen);

    let pending = account.mutate_with_ack(
        "spacemolt",
        "jump",
        Some(json!({ "target_system": "sol" })),
        move |ack| {
            *ack_for_callback.lock().expect("ack") = Some(ack);
        },
    );
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 1523,
            "result": { "ship": { "fuel": 60 }, "queue": { "has_pending": false } }
        })),
    });

    let res = pending.await.expect("mutation");
    assert_eq!(
        ack_seen.lock().expect("ack").as_ref().expect("ack").command,
        "jump"
    );
    assert_eq!(res.tick, 1523);
    assert_eq!(res.delta["ship"]["fuel"], json!(60));
    assert_eq!(account.state_snapshot()["ship"]["fuel"], json!(60));
    assert_eq!(
        account.state_snapshot()["queue"]["has_pending"],
        json!(false)
    );
}

#[tokio::test]
async fn automatic_dock_transition_refreshes_authoritative_location() {
    let (account, socket) = connected().await;

    let pending = account.mutate(
        "spacemolt",
        "repair_module",
        Some(json!({ "id": "module_1" })),
    );
    let mutation_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(mutation_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": {
                "pending": true,
                "command": "repair_module",
                "message": "queued"
            }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(mutation_id),
        payload: Some(json!({
            "command": "repair_module",
            "tick": 1524,
            "auto_docked": true,
            "result": { "details": { "repaired": true } }
        })),
    });

    let task = tokio::spawn(pending);
    wait_for_sent_len(&socket, 2).await;
    let sent = socket.sent();
    assert_eq!(sent[1].tool, "spacemolt");
    assert_eq!(sent[1].action, "get_location");
    let location_request_id = sent[1].request_id.clone().expect("request id");
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(location_request_id),
        payload: Some(json!({
            "result": "location",
            "structuredContent": {
                "system_id": "sol",
                "poi_id": "earth_station",
                "docked_at": "earth_station"
            }
        })),
    });

    let result = task.await.expect("mutation task").expect("mutation");
    assert!(result.auto_docked);
    assert_eq!(
        account.state_snapshot()["location"]["docked_at"],
        json!("earth_station")
    );
}

#[tokio::test]
async fn state_change_listeners_receive_action_result_sections_with_isolation() {
    let (account, socket) = connected().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_handler = Arc::clone(&seen);
    account.on_state_change(|_| panic!("listener failed"));
    account.on_state_change(move |changed| {
        seen_for_handler
            .lock()
            .expect("seen")
            .extend_from_slice(changed);
    });

    let pending = account.mutate("spacemolt", "jump", Some(json!({ "target_system": "sol" })));
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 1523,
            "result": { "ship": { "fuel": 60 }, "queue": { "has_pending": false } }
        })),
    });

    pending.await.expect("mutation");
    assert_eq!(
        *seen.lock().expect("seen"),
        vec![StateSection::Ship, StateSection::Queue]
    );
}

#[tokio::test]
async fn mutation_rejects_on_action_error_with_command_and_tick() {
    let (account, socket) = connected().await;

    let pending = account.mutate(
        "spacemolt",
        "jump",
        Some(json!({ "target_system": "nowhere" })),
    );
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "queued" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_error".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 1530,
            "code": "invalid_target",
            "message": "unreachable"
        })),
    });

    match pending.await.expect_err("action error") {
        ClientError::Server(err) => {
            assert_eq!(err.code, "invalid_target");
            assert_eq!(err.command.as_deref(), Some("jump"));
            assert_eq!(err.tick, Some(1530));
        }
        other => panic!("expected server error, got {other:?}"),
    }
}

#[tokio::test]
async fn generic_error_rejects_query_and_preserves_pending_command() {
    let (account, socket) = connected().await;

    let pending = account.query("spacemolt", "get_status", None);
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "code": "action_pending",
            "message": "already queued",
            "pending_command": "jump"
        })),
    });

    match pending.await.expect_err("server error") {
        ClientError::Server(err) => {
            assert_eq!(err.code, "action_pending");
            assert_eq!(err.pending_command.as_deref(), Some("jump"));
        }
        other => panic!("expected server error, got {other:?}"),
    }
}

#[tokio::test]
async fn unknown_push_frames_bypass_correlator_and_emit_events() {
    let (account, socket) = connected().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_handler = Arc::clone(&seen);
    let _listener: ListenerId = account.on("chat_message", move |payload| {
        seen_for_handler
            .lock()
            .expect("seen")
            .push(payload["content"].as_str().unwrap_or_default().to_string());
    });

    socket.server_send(RawFrame {
        kind: "chat_message".to_string(),
        request_id: None,
        payload: Some(json!({ "content": "still alive" })),
    });

    assert_eq!(*seen.lock().expect("seen"), vec!["still alive".to_string()]);
}

#[tokio::test]
async fn push_with_unknown_request_id_still_emits_event() {
    let (account, socket) = connected().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_handler = Arc::clone(&seen);
    account.on("chat_message", move |payload| {
        seen_for_handler
            .lock()
            .expect("seen")
            .push(payload["content"].as_str().unwrap_or_default().to_string());
    });

    socket.server_send(RawFrame {
        kind: "chat_message".to_string(),
        request_id: Some("not-pending".to_string()),
        payload: Some(json!({ "content": "push" })),
    });

    assert_eq!(*seen.lock().expect("seen"), vec!["push".to_string()]);
}

#[tokio::test]
async fn nested_pending_ack_waits_for_action_result() {
    let (account, socket) = connected().await;
    let ack_seen = Arc::new(Mutex::new(None));
    let ack_for_callback = Arc::clone(&ack_seen);

    let pending = account.mutate_with_ack(
        "spacemolt",
        "jump",
        Some(json!({ "target_system": "sol" })),
        move |ack| {
            *ack_for_callback.lock().expect("ack") = Some(ack);
        },
    );
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": {
                "details": { "pending": true, "command": "jump", "message": "queued nested" },
                "location": {}
            }
        })),
    });
    assert_eq!(
        ack_seen.lock().expect("ack").as_ref().expect("ack").message,
        "queued nested"
    );

    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 8,
            "result": { "location": { "system_id": "sol" } }
        })),
    });

    let res = pending.await.expect("mutation");
    assert_eq!(res.command, "jump");
    assert_eq!(res.delta["location"]["system_id"], json!("sol"));
}

#[tokio::test]
async fn mutation_result_without_pending_resolves_synchronously() {
    let (account, socket) = connected().await;

    let pending = account.mutate("spacemolt", "jump", Some(json!({ "target_system": "sol" })));
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "quote",
            "structuredContent": { "dry_run": true, "command": "jump" }
        })),
    });

    let res = pending.await.expect("sync mutation");
    assert_eq!(res.command, "jump");
    assert_eq!(res.tick, 0);
    assert_eq!(
        res.delta,
        json!({ "details": { "dry_run": true, "command": "jump" } })
    );
}

#[tokio::test]
async fn logged_in_seeds_state_and_marks_authenticated() {
    let (account, socket) = connected().await;

    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: None,
        payload: Some(json!({
            "player": { "username": "Nova", "credits": 42 },
            "ship": { "class_id": "shuttle" }
        })),
    });

    assert!(account.authenticated());
    assert_eq!(
        account.state_snapshot()["player"]["username"],
        json!("Nova")
    );
    assert_eq!(account.state().credits(), Some(42));
}

#[tokio::test]
async fn login_sends_auth_frame_and_resolves_on_logged_in() {
    let (account, socket) = connected().await;

    let pending = account.login(LoginParams {
        username: "Nova".to_string(),
        password: "pw".to_string(),
    });
    let sent = socket.sent();
    assert_eq!(sent[0].tool, "spacemolt_auth");
    assert_eq!(sent[0].action, "login");
    assert_eq!(
        sent[0].payload,
        Some(json!({ "username": "Nova", "password": "pw" }))
    );
    let request_id = sent[0].request_id.clone();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id,
        payload: Some(json!({
            "player": { "username": "Nova", "credits": 5000 },
            "ship": { "class_id": "shuttle" }
        })),
    });

    let state = pending.await.expect("login");
    assert_eq!(state["player"]["username"], json!("Nova"));
    assert!(account.authenticated());
    assert_eq!(account.state().credits(), Some(5000));
}

#[tokio::test]
async fn authenticate_with_seed_state_fetches_status_before_resolving() {
    let factory = Arc::new(MockSocketFactory::new());
    let account = Account::with_socket_factory(
        AccountOptions {
            url: "ws://mock/ws/v2".to_string(),
            seed_state: true,
            ..AccountOptions::default()
        },
        factory.clone(),
    );
    account.connect().await.expect("connect");
    let socket = factory.latest();
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    account.wait_for_welcome().await.expect("welcome");

    let pending = account.authenticate(spacemolt_lib_rs::auth::AuthCredentials::LoginToken {
        token: "tok_1".to_string(),
    });
    let task = tokio::spawn(pending);
    assert_eq!(socket.sent()[0].action, "login_token");
    let auth_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(auth_request_id),
        payload: Some(json!({
            "player": { "id": "player_1", "username": "Nova" }
        })),
    });

    wait_for_sent_len(&socket, 2).await;
    assert_eq!(socket.sent()[1].tool, "spacemolt");
    assert_eq!(socket.sent()[1].action, "get_status");
    let status_request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(status_request_id),
        payload: Some(json!({
            "result": "status",
            "structuredContent": {
                "player": { "id": "player_1", "username": "Nova", "credits": 42 },
                "ship": { "fuel": 80, "max_fuel": 100 },
                "location": { "system_id": "sol", "poi_id": "earth_station" }
            }
        })),
    });

    task.await.expect("auth task").expect("authenticate");
    assert_eq!(
        account.state_snapshot()["location"]["system_id"],
        json!("sol")
    );
    assert_eq!(account.state_snapshot()["ship"]["fuel"], json!(80));
}

#[tokio::test]
async fn login_rejects_with_typed_error_on_auth_failure() {
    let (account, socket) = connected().await;

    let pending = account.login(LoginParams {
        username: "Nova".to_string(),
        password: "wrong".to_string(),
    });
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "code": "invalid_credentials",
            "message": "bad password"
        })),
    });

    match pending.await.expect_err("auth error") {
        ClientError::Server(err) => {
            assert_eq!(err.code, "invalid_credentials");
            assert_eq!(err.message, "bad password");
        }
        other => panic!("expected server error, got {other:?}"),
    }
    assert!(!account.authenticated());
}

#[tokio::test]
async fn register_resolves_with_generated_credentials_and_state() {
    let (account, socket) = connected().await;

    let pending = account.register(RegisterParams {
        username: "Nova".to_string(),
        empire: "solarian".to_string(),
        registration_code: "code".to_string(),
    });
    let sent = socket.sent();
    assert_eq!(sent[0].tool, "spacemolt_auth");
    assert_eq!(sent[0].action, "register");
    assert_eq!(
        sent[0].payload,
        Some(json!({
            "username": "Nova",
            "empire": "solarian",
            "registration_code": "code"
        }))
    );
    socket.server_send(RawFrame {
        kind: "registered".to_string(),
        request_id: None,
        payload: Some(json!({ "password": "deadbeef", "player_id": "plr_1" })),
    });
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: None,
        payload: Some(json!({ "ship": { "class_id": "shuttle" } })),
    });

    let result = pending.await.expect("register");
    assert_eq!(result.password, "deadbeef");
    assert_eq!(result.player_id, "plr_1");
    assert_eq!(result.state["ship"]["class_id"], json!("shuttle"));
    assert!(account.authenticated());
}

#[tokio::test]
async fn register_rejects_if_logged_in_arrives_without_credentials() {
    let (account, socket) = connected().await;

    let pending = account.register(RegisterParams {
        username: "Nova".to_string(),
        empire: "solarian".to_string(),
        registration_code: "code".to_string(),
    });
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: None,
        payload: Some(json!({ "ship": { "class_id": "shuttle" } })),
    });

    match pending.await.expect_err("missing credentials") {
        ClientError::Server(err) => {
            assert_eq!(err.code, "missing_credentials");
        }
        other => panic!("expected server error, got {other:?}"),
    }
}

#[tokio::test]
async fn register_rejects_blank_required_fields_without_sending_a_frame() {
    let (account, socket) = connected().await;

    let err = account
        .register(RegisterParams {
            username: "Nova".to_string(),
            empire: "solarian".to_string(),
            registration_code: "  ".to_string(),
        })
        .await
        .expect_err("blank registration code");

    match err {
        ClientError::Server(err) => assert_eq!(err.code, "invalid_registration"),
        other => panic!("expected validation error, got {other:?}"),
    }
    assert!(socket.sent().is_empty());
}

#[tokio::test]
async fn socket_close_rejects_pending_auth() {
    let (account, socket) = connected().await;

    let pending = account.login(LoginParams {
        username: "Nova".to_string(),
        password: "pw".to_string(),
    });
    socket.close(1006, "rejected");

    match pending.await.expect_err("closed") {
        ClientError::ConnectionClosed(err) => {
            assert_eq!(err.code, Some(1006));
            assert_eq!(err.reason.as_deref(), Some("rejected"));
        }
        other => panic!("expected connection closed, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_rejects_when_another_auth_exchange_is_pending() {
    let (account, socket) = connected().await;

    let _pending = account.login(LoginParams {
        username: "Nova".to_string(),
        password: "pw".to_string(),
    });
    let err = account
        .register(RegisterParams {
            username: "Alt".to_string(),
            empire: "solarian".to_string(),
            registration_code: "code".to_string(),
        })
        .await
        .expect_err("pending auth");

    match err {
        ClientError::Server(err) => assert_eq!(err.code, "auth_in_progress"),
        other => panic!("expected server error, got {other:?}"),
    }
    assert_eq!(socket.sent().len(), 1);
}

#[tokio::test]
async fn auth_rejects_when_already_authenticated() {
    let (account, socket) = connected().await;

    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: None,
        payload: Some(json!({ "player": { "username": "Nova" } })),
    });
    let err = account
        .login(LoginParams {
            username: "Nova".to_string(),
            password: "pw".to_string(),
        })
        .await
        .expect_err("already authenticated");

    match err {
        ClientError::Server(err) => assert_eq!(err.code, "already_authenticated"),
        other => panic!("expected server error, got {other:?}"),
    }
    assert!(socket.sent().is_empty());
}

#[tokio::test]
async fn validation_errors_do_not_send_frames() {
    let (account, socket) = connected().await;

    let err = account
        .query("spacemolt", "jump", Some(json!({ "target_system": "sol" })))
        .await
        .expect_err("wrong kind");
    assert!(matches!(err, ClientError::UnknownAction(_)));

    let err = account
        .mutate("spacemolt", "get_status", None)
        .await
        .expect_err("wrong kind");
    assert!(matches!(err, ClientError::UnknownAction(_)));

    let err = account
        .send("spacemolt", "definitely_not_real", None)
        .await
        .expect_err("unknown");
    assert!(matches!(err, ClientError::UnknownAction(_)));
    assert!(socket.sent().is_empty());
}

#[tokio::test]
async fn in_flight_requests_reject_when_socket_closes() {
    let (account, socket) = connected().await;

    let pending = account.query("spacemolt", "get_status", None);
    socket.close(1006, "rejected");

    match pending.await.expect_err("closed") {
        ClientError::ConnectionClosed(err) => {
            assert_eq!(err.code, Some(1006));
            assert_eq!(err.reason.as_deref(), Some("rejected"));
        }
        other => panic!("expected connection closed, got {other:?}"),
    }
}

#[tokio::test]
async fn newline_delimited_raw_message_routes_each_frame_and_drops_bad_lines() {
    let (account, socket) = connected().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_handler = Arc::clone(&seen);
    account.on("chat_message", move |payload| {
        seen_for_handler
            .lock()
            .expect("seen")
            .push(payload["content"].as_str().unwrap_or_default().to_string());
    });

    let batch = [
        json_frame("chat_message", json!({ "content": "before" })),
        "not valid json{{{".to_string(),
        json_frame("chat_message", json!({ "content": "after" })),
    ]
    .join("\n");
    socket.server_send_raw(&batch);

    assert_eq!(
        *seen.lock().expect("seen"),
        vec!["before".to_string(), "after".to_string()]
    );
}

#[tokio::test]
async fn malformed_unicode_raw_message_is_dropped_without_panicking() {
    let (_account, socket) = connected().await;
    let malformed = format!("{}{{{{", "a".to_string() + &"😀".repeat(100));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        socket.server_send_raw(&malformed)
    }));

    assert!(result.is_ok());
}

#[tokio::test]
async fn send_dispatches_by_generated_action_kind() {
    let (account, socket) = connected().await;

    let mutation = account.send("spacemolt", "jump", Some(json!({ "target_system": "sol" })));
    let mutation_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(mutation_id.clone()),
        payload: Some(json!({
            "result": "pending",
            "structuredContent": { "pending": true, "command": "jump", "message": "q" }
        })),
    });
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some(mutation_id),
        payload: Some(json!({
            "command": "jump",
            "tick": 7,
            "result": { "location": { "system_id": "sol" } }
        })),
    });
    match mutation.await.expect("mutation") {
        CommandResult::Mutation(result) => {
            assert_eq!(result.delta["location"]["system_id"], json!("sol"));
        }
        other => panic!("expected mutation, got {other:?}"),
    }

    let query = account.send("spacemolt", "get_status", None);
    let query_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(query_id),
        payload: Some(json!({ "result": "ok" })),
    });
    match query.await.expect("query") {
        CommandResult::Query(result) => assert_eq!(result.result, json!("ok")),
        other => panic!("expected query, got {other:?}"),
    }
}

fn json_frame(kind: &str, payload: Value) -> String {
    serde_json::to_string(&RawFrame {
        kind: kind.to_string(),
        request_id: None,
        payload: Some(payload),
    })
    .expect("frame")
}
