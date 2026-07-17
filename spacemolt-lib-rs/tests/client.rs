mod mock_socket;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mock_socket::{MockSocket, MockSocketFactory};
use serde_json::json;
use spacemolt_lib_rs::account::ReconnectOptions;
use spacemolt_lib_rs::auth::{
    AuthCredentials, CredentialStore, FileCredentialStore, MemoryCredentialStore, StoredAccount,
};
use spacemolt_lib_rs::client::{ConnectAllOptions, SpacemoltClient, SpacemoltClientOptions};
use spacemolt_lib_rs::protocol::{RawFrame, WelcomePayload};
use spacemolt_lib_rs::{ClientError, RegisterParams};

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

fn client_options(factory: Arc<MockSocketFactory>) -> SpacemoltClientOptions {
    SpacemoltClientOptions {
        url: "ws://mock/ws/v2".to_string(),
        seed_state: false,
        connect_stagger_ms: 0,
        connect_batch_size: 100,
        connect_batch_wait_ms: 65_000,
        socket_factory: Some(factory),
        ..SpacemoltClientOptions::default()
    }
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

async fn wait_until(label: &str, mut predicate: impl FnMut() -> bool) {
    for _ in 0..200 {
        if predicate() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {label}");
}

async fn auto_serve_login(socket: &MockSocket, username: &str) {
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    wait_for_sent_len(socket, 1).await;
    assert_eq!(socket.sent()[0].action, "login");
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "player": { "id": format!("plr_{username}"), "username": username }
        })),
    });
}

async fn begin_registration(
    factory: &MockSocketFactory,
    client: SpacemoltClient<MemoryCredentialStore>,
) -> (
    tokio::task::JoinHandle<(
        SpacemoltClient<MemoryCredentialStore>,
        Result<(spacemolt_lib_rs::Account, spacemolt_lib_rs::RegisterResult), ClientError>,
    )>,
    MockSocket,
) {
    let task = tokio::spawn(async move {
        let result = client
            .register(RegisterParams {
                username: "Nova".to_string(),
                empire: "solarian".to_string(),
                registration_code: "code".to_string(),
            })
            .await;
        (client, result)
    });
    wait_for_socket_count(factory, 1).await;
    let socket = factory.get(0);
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    wait_for_sent_len(&socket, 1).await;
    assert_eq!(socket.sent()[0].action, "register");
    (task, socket)
}

fn finish_registration(socket: &MockSocket, state: serde_json::Value) {
    socket.server_send(RawFrame {
        kind: "registered".to_string(),
        request_id: None,
        payload: Some(json!({ "password": "generated", "player_id": "plr_1" })),
    });
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: None,
        payload: Some(state),
    });
}

fn reconnecting_options(factory: Arc<MockSocketFactory>) -> SpacemoltClientOptions {
    SpacemoltClientOptions {
        connect_stagger_ms: 0,
        connect_batch_size: 100,
        connect_batch_wait_ms: 65_000,
        ..client_options(factory)
    }
}

#[test]
fn memory_credential_store_put_get_list_remove() {
    let mut store = MemoryCredentialStore::default();

    store
        .put(spacemolt_lib_rs::auth::StoredAccount {
            id: "Nova".to_string(),
            credentials: spacemolt_lib_rs::auth::AuthCredentials::Login {
                username: "Nova".to_string(),
                password: "pw".to_string(),
            },
            player_id: None,
        })
        .expect("put");

    assert_eq!(
        store.get("Nova").expect("stored").credentials,
        spacemolt_lib_rs::auth::AuthCredentials::Login {
            username: "Nova".to_string(),
            password: "pw".to_string()
        }
    );
    assert_eq!(store.list().len(), 1);
    assert!(store.remove("Nova").is_some());
    assert!(store.get("Nova").is_none());
}

#[test]
fn file_credential_store_round_trips_through_disk() {
    let path = std::env::temp_dir().join(format!(
        "spacemolt-lib-rs-credentials-{}.json",
        uuid::Uuid::new_v4()
    ));

    let mut store = FileCredentialStore::open(&path).expect("open store");
    store
        .put(StoredAccount {
            id: "Nova".to_string(),
            credentials: AuthCredentials::Login {
                username: "Nova".to_string(),
                password: "pw".to_string(),
            },
            player_id: Some("plr_1".to_string()),
        })
        .expect("put");
    store
        .put(StoredAccount {
            id: "Clerked".to_string(),
            credentials: AuthCredentials::Clerk {
                player_id: "plr_2".to_string(),
                api_key: "sk_test".to_string(),
                http_base_url: "https://game.spacemolt.com".to_string(),
            },
            player_id: Some("plr_2".to_string()),
        })
        .expect("put");

    let mut reopened = FileCredentialStore::open(&path).expect("reopen store");
    assert_eq!(
        reopened
            .list()
            .into_iter()
            .map(|account| account.id.clone())
            .collect::<Vec<_>>(),
        vec!["Nova".to_string(), "Clerked".to_string()]
    );
    assert_eq!(
        reopened.get("Nova").expect("nova").credentials,
        AuthCredentials::Login {
            username: "Nova".to_string(),
            password: "pw".to_string(),
        }
    );
    assert!(reopened.remove("Nova").is_some());

    let reopened = FileCredentialStore::open(&path).expect("reopen after remove");
    assert!(reopened.get("Nova").is_none());
    assert!(reopened.get("Clerked").is_some());

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn connect_retries_failed_startup_before_authenticating_stored_account() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut opts = client_options(factory.clone());
    opts.connect_retry = Some(ReconnectOptions {
        max_retries: Some(2),
        base_delay_ms: 1,
        max_delay_ms: 1,
    });
    let client = SpacemoltClient::new(opts, MemoryCredentialStore::default());
    client.add_login("Nova", "pw");

    let task = tokio::spawn(async move {
        let account = client.connect("Nova").await.expect("connect");
        (client, account)
    });
    wait_for_socket_count(&factory, 1).await;
    factory
        .get(0)
        .close(4003, "connection_rate_limited retry_after=0");
    wait_for_socket_count(&factory, 2).await;
    auto_serve_login(&factory.get(1), "Nova").await;
    let (client, account) = task.await.expect("task");

    assert!(account.authenticated());
    assert_eq!(client.account("Nova").expect("account").id(), Some("Nova"));
    assert_eq!(factory.len(), 2);
}

#[tokio::test]
async fn connect_authenticates_a_stored_account_and_captures_player_id() {
    let factory = Arc::new(MockSocketFactory::new());
    let client = SpacemoltClient::new(
        client_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    client.add_login("Nova", "pw");

    let task = tokio::spawn(async move {
        let account = client.connect("Nova").await.expect("connect");
        (client, account)
    });
    wait_for_socket_count(&factory, 1).await;
    auto_serve_login(&factory.get(0), "Nova").await;
    let (client, account) = task.await.expect("task");

    assert!(account.authenticated());
    assert_eq!(account.id(), Some("Nova"));
    assert_eq!(client.account("Nova").expect("account").id(), Some("Nova"));
    assert_eq!(
        client
            .credential_store()
            .get("Nova")
            .expect("stored")
            .player_id
            .as_deref(),
        Some("plr_Nova")
    );
}

#[tokio::test]
async fn connect_all_connects_every_stored_account_and_reports_incrementally() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut client = SpacemoltClient::new(
        client_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    client.add_login("Nova", "pw");
    client.add_login("Rex", "pw");
    let persistent_seen = Arc::new(Mutex::new(Vec::new()));
    let persistent_for_handler = Arc::clone(&persistent_seen);
    client.on_account_connected(move |account| {
        persistent_for_handler
            .lock()
            .expect("persistent")
            .push(account.id().unwrap_or_default().to_string());
    });
    let per_call_seen = Arc::new(Mutex::new(Vec::new()));
    let per_call_for_handler = Arc::clone(&per_call_seen);

    let task = tokio::spawn(async move {
        let accounts = client
            .connect_all(ConnectAllOptions::new(move |account| {
                per_call_for_handler
                    .lock()
                    .expect("per call")
                    .push(account.id().unwrap_or_default().to_string());
            }))
            .await;
        (client, accounts)
    });

    wait_for_socket_count(&factory, 1).await;
    auto_serve_login(&factory.get(0), "Nova").await;
    for _ in 0..100 {
        if per_call_seen.lock().expect("per call").as_slice() == ["Nova"] {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(
        *per_call_seen.lock().expect("per call"),
        vec!["Nova".to_string()]
    );

    wait_for_socket_count(&factory, 2).await;
    auto_serve_login(&factory.get(1), "Rex").await;
    let (client, accounts) = task.await.expect("task");

    assert_eq!(accounts.len(), 2);
    let mut ids = client.ids();
    ids.sort();
    assert_eq!(ids, vec!["Nova".to_string(), "Rex".to_string()]);
    assert_eq!(
        *persistent_seen.lock().expect("persistent"),
        vec!["Nova".to_string(), "Rex".to_string()]
    );
    assert_eq!(
        *per_call_seen.lock().expect("per call"),
        vec!["Nova".to_string(), "Rex".to_string()]
    );
}

#[tokio::test]
async fn connect_all_pauses_between_batches_only_after_batch_size_is_exceeded() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut opts = client_options(factory.clone());
    opts.connect_stagger_ms = 1;
    opts.connect_batch_size = 2;
    opts.connect_batch_wait_ms = 150;
    let client = SpacemoltClient::new(opts, MemoryCredentialStore::default());
    client.add_login("A", "pw");
    client.add_login("B", "pw");
    client.add_login("C", "pw");

    let task = tokio::spawn(async move {
        let accounts = client.connect_all(ConnectAllOptions::default()).await;
        (client, accounts)
    });
    let mut created_at = Vec::new();
    for i in 0..3 {
        wait_for_socket_count(&factory, i + 1).await;
        created_at.push(std::time::Instant::now());
        auto_serve_login(&factory.get(i), &format!("acct{i}")).await;
    }
    let (_client, accounts) = task.await.expect("task");

    assert_eq!(accounts.len(), 3);
    let within_batch_gap = created_at[1].duration_since(created_at[0]);
    let across_batch_gap = created_at[2].duration_since(created_at[1]);
    assert!(within_batch_gap < Duration::from_millis(100));
    assert!(across_batch_gap >= Duration::from_millis(140));
}

#[tokio::test]
async fn connect_all_skips_one_failed_account_without_aborting_the_fleet() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut opts = client_options(factory.clone());
    opts.connect_retry = None;
    let client = SpacemoltClient::new(opts, MemoryCredentialStore::default());
    client.add_login("Nova", "pw");
    client.add_login("Bad", "pw");
    client.add_login("Rex", "pw");

    let task = tokio::spawn(async move {
        let accounts = client.connect_all(ConnectAllOptions::default()).await;
        (client, accounts)
    });
    wait_for_socket_count(&factory, 1).await;
    auto_serve_login(&factory.get(0), "Nova").await;
    wait_for_socket_count(&factory, 2).await;
    factory.get(1).close(1006, "abnormal");
    wait_for_socket_count(&factory, 3).await;
    auto_serve_login(&factory.get(2), "Rex").await;
    let (client, accounts) = task.await.expect("task");

    assert_eq!(accounts.len(), 2);
    assert!(client.account("Bad").is_none());
    let mut ids = client.ids();
    ids.sort();
    assert_eq!(ids, vec!["Nova".to_string(), "Rex".to_string()]);
}

#[tokio::test]
async fn remove_closes_forgets_and_removes_credentials() {
    let factory = Arc::new(MockSocketFactory::new());
    let client = SpacemoltClient::new(
        client_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    client.add_login("Nova", "pw");
    let task = tokio::spawn(async move {
        let account = client.connect("Nova").await.expect("connect");
        (client, account)
    });
    wait_for_socket_count(&factory, 1).await;
    auto_serve_login(&factory.get(0), "Nova").await;
    let (client, _account) = task.await.expect("task");

    client.remove("Nova");

    assert!(client.account("Nova").is_none());
    assert!(client.credential_store().get("Nova").is_none());
}

#[tokio::test]
async fn reconnect_after_unexpected_close_reuses_same_account_instance() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut client = SpacemoltClient::new(
        reconnecting_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    client.add_login("Nova", "pw");
    let reconnected = Arc::new(Mutex::new(Vec::new()));
    let reconnected_for_handler = Arc::clone(&reconnected);
    client.on_account_reconnected(move |account| {
        reconnected_for_handler
            .lock()
            .expect("reconnected")
            .push(account.id().unwrap_or_default().to_string());
    });

    let task = tokio::spawn(async move {
        let account = client.connect("Nova").await.expect("connect");
        (client, account)
    });
    wait_for_socket_count(&factory, 1).await;
    auto_serve_login(&factory.get(0), "Nova").await;
    let (client, account) = task.await.expect("task");

    factory.get(0).close(1006, "abnormal");
    wait_for_socket_count(&factory, 2).await;
    auto_serve_login(&factory.get(1), "Nova").await;
    wait_until("reconnected listener", || {
        !reconnected.lock().expect("reconnected").is_empty()
    })
    .await;

    assert_eq!(
        *reconnected.lock().expect("reconnected"),
        vec!["Nova".to_string()]
    );
    assert_eq!(client.account("Nova").expect("account").id(), account.id());
    assert!(account.authenticated());
}

#[tokio::test]
async fn reconnect_retries_when_reconnect_socket_drops_before_auth_completes() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut opts = reconnecting_options(factory.clone());
    opts.connect_retry = Some(ReconnectOptions {
        max_retries: Some(2),
        base_delay_ms: 1,
        max_delay_ms: 1,
    });
    let client = SpacemoltClient::new(opts, MemoryCredentialStore::default());
    client.add_login("Nova", "pw");

    let task = tokio::spawn(async move {
        let account = client.connect("Nova").await.expect("connect");
        (client, account)
    });
    wait_for_socket_count(&factory, 1).await;
    auto_serve_login(&factory.get(0), "Nova").await;
    let (client, account) = task.await.expect("task");

    factory.get(0).close(1006, "abnormal");
    wait_for_socket_count(&factory, 2).await;
    let first_reconnect = factory.get(1);
    first_reconnect.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    wait_for_sent_len(&first_reconnect, 1).await;
    assert_eq!(first_reconnect.sent()[0].action, "login");
    first_reconnect.close(1006, "abnormal");

    wait_for_socket_count(&factory, 3).await;
    auto_serve_login(&factory.get(2), "Nova").await;
    wait_until("reconnected account", || account.authenticated()).await;

    assert_eq!(factory.len(), 3);
    assert_eq!(client.account("Nova").expect("account").id(), account.id());
}

#[tokio::test]
async fn terminal_close_removes_account_and_notifies_disconnected_without_reconnect() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut client = SpacemoltClient::new(
        reconnecting_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    client.add_login("Nova", "pw");
    let disconnected = Arc::new(Mutex::new(Vec::new()));
    let disconnected_for_handler = Arc::clone(&disconnected);
    client.on_account_disconnected(move |id, err| {
        disconnected_for_handler
            .lock()
            .expect("disconnected")
            .push((id, err.code));
    });

    let task = tokio::spawn(async move {
        let account = client.connect("Nova").await.expect("connect");
        (client, account)
    });
    wait_for_socket_count(&factory, 1).await;
    auto_serve_login(&factory.get(0), "Nova").await;
    let (client, _account) = task.await.expect("task");

    factory.get(0).close(4001, "session_replaced");
    wait_until("disconnected listener", || {
        !disconnected.lock().expect("disconnected").is_empty()
    })
    .await;
    tokio::time::sleep(Duration::from_millis(30)).await;

    assert_eq!(
        *disconnected.lock().expect("disconnected"),
        vec![("Nova".to_string(), Some(4001))]
    );
    assert!(client.account("Nova").is_none());
    assert_eq!(factory.len(), 1);
}

#[tokio::test]
async fn mass_reconnects_are_paced_by_client_batch_settings() {
    let factory = Arc::new(MockSocketFactory::new());
    let mut opts = reconnecting_options(factory.clone());
    opts.connect_stagger_ms = 1;
    opts.connect_batch_size = 2;
    opts.connect_batch_wait_ms = 150;
    let client = SpacemoltClient::new(opts, MemoryCredentialStore::default());
    client.add_login("A", "pw");
    client.add_login("B", "pw");
    client.add_login("C", "pw");

    let task = tokio::spawn(async move {
        let accounts = client.connect_all(ConnectAllOptions::default()).await;
        (client, accounts)
    });
    for i in 0..3 {
        wait_for_socket_count(&factory, i + 1).await;
        auto_serve_login(&factory.get(i), &format!("acct{i}")).await;
    }
    let (_client, accounts) = task.await.expect("task");
    assert_eq!(accounts.len(), 3);

    for i in 0..3 {
        factory.get(i).close(1006, "abnormal");
    }

    wait_for_socket_count(&factory, 5).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(factory.len(), 5);

    wait_for_socket_count(&factory, 6).await;
    for i in 0..3 {
        auto_serve_login(&factory.get(3 + i), &format!("acct{i}")).await;
    }
}

#[tokio::test]
async fn registration_with_complete_login_state_is_ready_without_refresh() {
    let factory = Arc::new(MockSocketFactory::new());
    let client = SpacemoltClient::new(
        client_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    let (task, socket) = begin_registration(&factory, client).await;
    finish_registration(
        &socket,
        json!({
            "player": { "id": "plr_1", "username": "Nova" },
            "ship": { "class_id": "shuttle" },
            "location": { "system_id": "sol", "poi_id": "earth_station" }
        }),
    );

    let (client, result) = task.await.expect("registration task");
    let (account, result) = result.expect("ready account");
    assert_eq!(socket.sent().len(), 1);
    assert_eq!(
        account.player().and_then(|player| player.id),
        Some("plr_1".into())
    );
    assert_eq!(
        account.location().and_then(|location| location.system_id),
        Some("sol".into())
    );
    assert_eq!(result.state["location"]["poi_id"], json!("earth_station"));
    let store = client.credential_store();
    let stored = store.get("Nova").expect("credentials");
    assert_eq!(stored.player_id.as_deref(), Some("plr_1"));
    assert!(matches!(
        &stored.credentials,
        AuthCredentials::Login { username, password }
            if username == "Nova" && password == "generated"
    ));
}

#[tokio::test]
async fn registration_hydrates_incomplete_login_state_before_returning() {
    let factory = Arc::new(MockSocketFactory::new());
    let client = SpacemoltClient::new(
        client_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    let (task, socket) = begin_registration(&factory, client).await;
    finish_registration(
        &socket,
        json!({
            "player": { "id": "plr_1", "username": "Nova" },
            "ship": { "class_id": "shuttle" }
        }),
    );
    wait_for_sent_len(&socket, 2).await;
    assert_eq!(socket.sent()[1].action, "get_status");
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "status",
            "structuredContent": {
                "player": { "id": "plr_1", "username": "Nova" },
                "ship": { "class_id": "shuttle" },
                "location": { "system_id": "sol", "poi_id": "earth_station" }
            }
        })),
    });

    let (_client, result) = task.await.expect("registration task");
    let (account, _) = result.expect("hydrated account");
    assert_eq!(
        account.location().and_then(|location| location.poi_id),
        Some("earth_station".into())
    );
}

#[tokio::test]
async fn registration_returns_recovery_credentials_when_hydration_has_no_location() {
    let factory = Arc::new(MockSocketFactory::new());
    let client = SpacemoltClient::new(
        client_options(factory.clone()),
        MemoryCredentialStore::default(),
    );
    let (task, socket) = begin_registration(&factory, client).await;
    finish_registration(
        &socket,
        json!({ "player": { "id": "plr_1", "username": "Nova" } }),
    );
    wait_for_sent_len(&socket, 2).await;
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "status",
            "structuredContent": { "player": { "id": "plr_1", "username": "Nova" } }
        })),
    });

    let (client, result) = task.await.expect("registration task");
    match result.expect_err("not state-ready") {
        ClientError::PostRegistration {
            username,
            password,
            player_id,
            message,
        } => {
            assert_eq!(username, "Nova");
            assert_eq!(password, "generated");
            assert_eq!(player_id, "plr_1");
            assert!(message.contains("no system location"));
        }
        other => panic!("expected recoverable registration error, got {other:?}"),
    }
    assert!(client.account("Nova").is_none());
    assert!(client.credential_store().get("Nova").is_some());
}
