mod mock_socket;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use mock_socket::{MockSocket, MockSocketFactory};
use serde_json::{json, Value};
use spacemolt_lib_rs::account::{Account, AccountOptions, ReconnectOptions};
use spacemolt_lib_rs::auth::{
    AuthCredentials, ClerkHttpClient, ClerkPlayer, ClerkSource, CredentialStore,
    MemoryCredentialStore,
};
use spacemolt_lib_rs::client::{ConnectOwnedOptions, SpacemoltClient, SpacemoltClientOptions};
use spacemolt_lib_rs::protocol::{RawFrame, WelcomePayload};

#[derive(Default)]
struct MockClerkHttp {
    calls: Mutex<Vec<(String, String, Option<Value>)>>,
}

#[async_trait]
impl ClerkHttpClient for MockClerkHttp {
    async fn request_json(
        &self,
        method: &str,
        url: &str,
        api_key: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        self.calls.lock().expect("calls").push((
            method.to_string(),
            url.to_string(),
            Some(json!(api_key)),
        ));
        if url.ends_with("/api/registration-code") {
            return Ok(json!({
                "registration_code": "rc",
                "players": [
                    { "id": "plr_1", "username": "Nova", "empire": "solarian", "hidden": false },
                    { "id": "plr_2", "username": "Ghost", "empire": "martian", "hidden": true }
                ]
            }));
        }
        if url.ends_with("/api/player/plr_1/ws-token") {
            return Ok(json!({ "token": "tok_1", "expires_in": 300 }));
        }
        if url.ends_with("/api/player/plr_2/ws-token") {
            return Ok(json!({ "token": "tok_2", "expires_in": 300 }));
        }
        if let Some(body) = body {
            return Ok(body);
        }
        Err(format!("unmatched {method} {url}"))
    }
}

impl MockClerkHttp {
    fn calls(&self) -> Vec<(String, String, Option<Value>)> {
        self.calls.lock().expect("calls").clone()
    }
}

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

async fn auto_serve_token(socket: &MockSocket, username: &str) {
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    wait_for_sent_len(socket, 1).await;
    assert_eq!(socket.sent()[0].action, "login_token");
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "player": { "id": format!("plr_{username}"), "username": username }
        })),
    });
}

#[tokio::test]
async fn clerk_source_lists_players_and_mints_ws_tokens_with_bearer_key() {
    let http = Arc::new(MockClerkHttp::default());
    let source = ClerkSource::with_http_client(
        "sk_test".to_string(),
        "https://game.spacemolt.com/".to_string(),
        http.clone(),
    );

    assert_eq!(source.http_base_url(), "https://game.spacemolt.com");
    let players = source.list_players().await.expect("players");
    assert_eq!(players.len(), 2);
    assert_eq!(players[0].username, "Nova");

    let token = source.mint_ws_token("plr_1").await.expect("token");
    assert_eq!(token, "tok_1");
    let calls = http.calls();
    assert!(calls.iter().any(|(method, url, key)| method == "GET"
        && url.ends_with("/api/registration-code")
        && key.as_ref() == Some(&json!("sk_test"))));
    assert!(calls
        .iter()
        .any(|(method, url, _)| method == "POST" && url.ends_with("/api/player/plr_1/ws-token")));
}

#[tokio::test]
async fn account_authenticates_via_clerk_by_minting_a_fresh_login_token() {
    let factory = Arc::new(MockSocketFactory::new());
    let http = Arc::new(MockClerkHttp::default());
    let account = Account::with_socket_factory(
        AccountOptions {
            url: "ws://mock/ws/v2".to_string(),
            seed_state: false,
            clerk_http_client: Some(http.clone()),
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

    let account_for_auth = account.clone();
    let auth = tokio::spawn(async move {
        account_for_auth
            .authenticate(AuthCredentials::Clerk {
                player_id: "plr_1".to_string(),
                api_key: "sk_test".to_string(),
                http_base_url: "https://game.spacemolt.com".to_string(),
            })
            .await
    });
    wait_for_sent_len(&socket, 1).await;
    assert_eq!(socket.sent()[0].action, "login_token");
    assert_eq!(
        socket.sent()[0].payload.as_ref().expect("payload")["token"],
        "tok_1"
    );
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({ "player": { "username": "Nova" } })),
    });

    auth.await.expect("task").expect("auth");
    assert!(account.authenticated());
    assert_eq!(
        http.calls()
            .iter()
            .filter(|(_, url, _)| url.ends_with("/api/player/plr_1/ws-token"))
            .count(),
        1
    );
}

#[tokio::test]
async fn clerk_auth_retries_rate_limit_by_minting_a_fresh_token() {
    let factory = Arc::new(MockSocketFactory::new());
    let http = Arc::new(MockClerkHttp::default());
    let account = Account::with_socket_factory(
        AccountOptions {
            url: "ws://mock/ws/v2".to_string(),
            seed_state: false,
            max_rate_limit_retries: 2,
            clerk_http_client: Some(http.clone()),
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

    let account_for_auth = account.clone();
    let auth = tokio::spawn(async move {
        account_for_auth
            .authenticate(AuthCredentials::Clerk {
                player_id: "plr_1".to_string(),
                api_key: "sk_test".to_string(),
                http_base_url: "https://game.spacemolt.com".to_string(),
            })
            .await
    });
    wait_for_sent_len(&socket, 1).await;
    let first_request = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "error".to_string(),
        request_id: Some(first_request),
        payload: Some(json!({
            "code": "rate_limited",
            "message": "Too many requests. Retry in 0 seconds."
        })),
    });
    wait_for_sent_len(&socket, 2).await;
    let second_request = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(second_request),
        payload: Some(json!({ "player": { "username": "Nova" } })),
    });

    auth.await.expect("task").expect("auth");
    assert!(account.authenticated());
    assert_eq!(
        http.calls()
            .iter()
            .filter(|(_, url, _)| url.ends_with("/api/player/plr_1/ws-token"))
            .count(),
        2
    );
    assert_eq!(
        socket
            .sent()
            .iter()
            .filter(|frame| frame.action == "login_token")
            .count(),
        2
    );
}

#[tokio::test]
async fn connect_owned_filters_players_stores_clerk_credentials_and_connects_selected_accounts() {
    let factory = Arc::new(MockSocketFactory::new());
    let http = Arc::new(MockClerkHttp::default());
    let client = SpacemoltClient::new(
        SpacemoltClientOptions {
            url: "ws://mock/ws/v2".to_string(),
            seed_state: false,
            connect_stagger_ms: 0,
            clerk_api_key: Some("sk_test".to_string()),
            socket_factory: Some(factory.clone()),
            clerk_http_client: Some(http),
            ..SpacemoltClientOptions::default()
        },
        MemoryCredentialStore::default(),
    );

    let task = tokio::spawn(async move {
        let accounts = client
            .connect_owned(ConnectOwnedOptions::new(|player: &ClerkPlayer| {
                !player.hidden
            }))
            .await
            .expect("connect owned");
        (client, accounts)
    });
    wait_for_socket_count(&factory, 1).await;
    auto_serve_token(&factory.get(0), "Nova").await;
    let (client, accounts) = task.await.expect("task");

    assert_eq!(accounts.len(), 1);
    assert_eq!(client.ids(), vec!["Nova".to_string()]);
    let stored = client
        .credential_store()
        .get("Nova")
        .expect("stored")
        .clone();
    assert_eq!(stored.player_id.as_deref(), Some("plr_Nova"));
    assert_eq!(
        stored.credentials,
        AuthCredentials::Clerk {
            player_id: "plr_1".to_string(),
            api_key: "sk_test".to_string(),
            http_base_url: "https://game.spacemolt.com".to_string(),
        }
    );
}

#[tokio::test]
async fn connect_owned_retries_startup_failure_and_mints_a_fresh_token() {
    let factory = Arc::new(MockSocketFactory::new());
    let http = Arc::new(MockClerkHttp::default());
    let client = SpacemoltClient::new(
        SpacemoltClientOptions {
            url: "ws://mock/ws/v2".to_string(),
            seed_state: false,
            connect_stagger_ms: 0,
            connect_retry: Some(ReconnectOptions {
                max_retries: Some(2),
                base_delay_ms: 1,
                max_delay_ms: 1,
            }),
            clerk_api_key: Some("sk_test".to_string()),
            socket_factory: Some(factory.clone()),
            clerk_http_client: Some(http.clone()),
            ..SpacemoltClientOptions::default()
        },
        MemoryCredentialStore::default(),
    );

    let task = tokio::spawn(async move {
        let accounts = client
            .connect_owned(ConnectOwnedOptions::new(|player: &ClerkPlayer| {
                !player.hidden
            }))
            .await
            .expect("connect owned");
        (client, accounts)
    });
    wait_for_socket_count(&factory, 1).await;
    let first_socket = factory.get(0);
    first_socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    wait_for_sent_len(&first_socket, 1).await;
    assert_eq!(first_socket.sent()[0].action, "login_token");
    first_socket.close(4003, "connection_rate_limited retry_after=0");
    wait_for_socket_count(&factory, 2).await;
    auto_serve_token(&factory.get(1), "Nova").await;
    let (_client, accounts) = task.await.expect("task");

    assert_eq!(accounts.len(), 1);
    assert_eq!(
        http.calls()
            .iter()
            .filter(|(_, url, _)| url.ends_with("/api/player/plr_1/ws-token"))
            .count(),
        2
    );
}

#[tokio::test]
async fn list_owned_players_requires_a_clerk_api_key() {
    let client = SpacemoltClient::new(
        SpacemoltClientOptions::default(),
        MemoryCredentialStore::default(),
    );

    let err = client.list_owned_players().await.expect_err("missing key");
    assert!(err.to_string().contains("clerk_api_key"));
}
