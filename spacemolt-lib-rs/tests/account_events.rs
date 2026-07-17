mod mock_socket;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mock_socket::{MockSocket, MockSocketFactory};
use serde_json::{json, Value};
use spacemolt_lib_rs::account::{Account, AccountOptions, LoginParams, ReconnectOptions};
use spacemolt_lib_rs::auth::AuthCredentials;
use spacemolt_lib_rs::protocol::{RawFrame, StateSection, WelcomePayload};
use spacemolt_lib_rs::state::MarketItem;

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

fn test_options() -> AccountOptions {
    AccountOptions {
        url: "ws://mock/ws/v2".to_string(),
        seed_state: false,
        query_timeout_ms: 250,
        mutation_timeout_ms: 250,
        fast_mutation_timeout_ms: 250,
        ..AccountOptions::default()
    }
}

async fn connected() -> (Account, MockSocket) {
    let factory = Arc::new(MockSocketFactory::new());
    connected_with_factory(test_options(), factory).await
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

async fn wait_for_socket_count(factory: &MockSocketFactory, len: usize) {
    for _ in 0..200 {
        if factory.len() >= len {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {len} sockets; saw {}", factory.len());
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

fn market_snapshot() -> Value {
    json!({
        "action": "subscribe_market",
        "base_id": "earth_station",
        "base_name": "Earth Station",
        "items": [
            { "item_id": "iron_ore", "sell_orders": [{ "price_each": 10, "quantity": 5 }], "buy_orders": [] },
            { "item_id": "water", "sell_orders": [{ "price_each": 2, "quantity": 100 }], "buy_orders": [] }
        ]
    })
}

fn observation_snapshot() -> Value {
    json!({
        "action": "subscribe_observation",
        "poi_id": "earth_station",
        "system_id": "sol",
        "active_scan": false,
        "unknown_signature": false,
        "nearby": [{ "player_id": "p1", "username": "Nova", "in_combat": false }],
        "system_agents": [],
        "cloaked_contacts": []
    })
}

#[tokio::test]
async fn on_any_and_event_streams_receive_push_frames() {
    let (account, socket) = connected().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_handler = Arc::clone(&seen);
    account.on_any(move |frame| {
        seen_for_handler
            .lock()
            .expect("seen")
            .push(frame.kind.clone());
    });
    let mut stream = account.events("chat_message");

    socket.server_send(RawFrame {
        kind: "chat_message".to_string(),
        request_id: None,
        payload: Some(json!({ "content": "one" })),
    });
    socket.server_send(RawFrame {
        kind: "mining_yield".to_string(),
        request_id: None,
        payload: Some(json!({ "item_id": "iron_ore", "quantity": 3 })),
    });

    let chat = tokio::time::timeout(Duration::from_millis(100), stream.recv())
        .await
        .expect("stream event")
        .expect("stream still open");
    assert_eq!(chat["content"], "one");
    assert_eq!(
        *seen.lock().expect("seen"),
        vec!["chat_message".to_string(), "mining_yield".to_string()]
    );
}

#[tokio::test]
async fn subscribe_market_seeds_the_book_and_updates_merge_changed_items() {
    let (account, socket) = connected().await;

    let pending = account.subscribe_market();
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "ok",
            "structuredContent": market_snapshot()
        })),
    });

    let baseline = pending.await.expect("subscribe market");
    assert_eq!(baseline["base_id"], "earth_station");
    assert!(account.market_subscribed());
    assert_eq!(
        account.market("earth_station").expect("market").items.len(),
        2
    );

    socket.server_send(RawFrame {
        kind: "market_update".to_string(),
        request_id: None,
        payload: Some(json!({
            "base_id": "earth_station",
            "tick": 1600,
            "items": [
                { "item_id": "iron_ore", "sell_orders": [{ "price_each": 12, "quantity": 3 }], "buy_orders": [] }
            ]
        })),
    });

    let book = account.market("earth_station").expect("market");
    assert_eq!(book.tick, 1600);
    let MarketItem::Update(iron_ore) = &book.items["iron_ore"] else {
        panic!("iron ore should contain the latest market update");
    };
    assert_eq!(iron_ore.sell_orders[0].price_each, 12);
    let MarketItem::Snapshot(water) = &book.items["water"] else {
        panic!("water should retain its market snapshot");
    };
    assert_eq!(water.sell_orders[0].quantity, 100);
}

#[tokio::test]
async fn market_cache_drops_when_location_leaves_subscribed_base() {
    let (account, socket) = connected().await;
    let pending = account.subscribe_market();
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "ok",
            "structuredContent": market_snapshot()
        })),
    });
    pending.await.expect("subscribe market");
    assert!(account.market("earth_station").is_some());

    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some("undock-1".to_string()),
        payload: Some(json!({
            "command": "undock",
            "tick": 2,
            "result": { "location": { "poi_id": "earth_station", "docked_at": null } }
        })),
    });

    assert!(account.market("earth_station").is_none());
    assert!(!account.market_subscribed());
}

#[tokio::test]
async fn subscribe_observation_bridges_presence_into_location() {
    let (account, socket) = connected().await;
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some("seed".to_string()),
        payload: Some(json!({
            "command": "dock",
            "tick": 1,
            "result": { "location": { "poi_id": "earth_station", "docked_at": "earth_station" } }
        })),
    });
    let changed = Arc::new(Mutex::new(Vec::new()));
    let changed_for_handler = Arc::clone(&changed);
    account.on_state_change(move |sections| {
        changed_for_handler
            .lock()
            .expect("changed")
            .extend_from_slice(sections);
    });

    let pending = account.subscribe_observation(false);
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "ok",
            "structuredContent": observation_snapshot()
        })),
    });

    pending.await.expect("subscribe observation");
    assert!(account.observation_subscribed());
    assert_eq!(
        account.state_snapshot()["location"]["nearby_players"][0]["username"],
        "Nova"
    );
    assert_eq!(
        account.state_snapshot()["location"]["nearby_player_count"],
        json!(1)
    );
    assert_eq!(
        *changed.lock().expect("changed"),
        vec![StateSection::Location]
    );

    socket.server_send(RawFrame {
        kind: "observation_update".to_string(),
        request_id: None,
        payload: Some(json!({
            "poi_id": "earth_station",
            "system_id": "sol",
            "tick": 5,
            "unknown_signature": false,
            "nearby_changed": [{ "player_id": "p2", "username": "Rex", "in_combat": true }],
            "nearby_departed": ["p1"]
        })),
    });

    assert_eq!(
        account.state_snapshot()["location"]["nearby_players"][0]["username"],
        "Rex"
    );
    assert_eq!(
        serde_json::to_value(
            account
                .observation()
                .expect("observation")
                .nearby
                .get("p2")
                .expect("p2"),
        )
        .expect("serialize observed player")["username"],
        "Rex"
    );
}

#[tokio::test]
async fn observation_cache_drops_when_location_leaves_subscribed_poi() {
    let (account, socket) = connected().await;
    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some("seed".to_string()),
        payload: Some(json!({
            "command": "dock",
            "tick": 1,
            "result": { "location": { "poi_id": "earth_station", "docked_at": "earth_station" } }
        })),
    });
    let pending = account.subscribe_observation(false);
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "ok",
            "structuredContent": observation_snapshot()
        })),
    });
    pending.await.expect("subscribe observation");
    assert!(account.observation().is_some());

    socket.server_send(RawFrame {
        kind: "action_result".to_string(),
        request_id: Some("travel-1".to_string()),
        payload: Some(json!({
            "command": "travel",
            "tick": 2,
            "result": { "location": { "poi_id": "mars_station", "docked_at": null } }
        })),
    });

    assert!(account.observation().is_none());
    assert!(!account.observation_subscribed());
}

#[tokio::test]
async fn reconnect_restores_active_market_subscription_before_reconnected_event() {
    let factory = Arc::new(MockSocketFactory::new());
    let (account, socket) = connected_with_factory(
        AccountOptions {
            reconnect: Some(ReconnectOptions {
                max_retries: Some(3),
                base_delay_ms: 1,
                max_delay_ms: 10,
            }),
            credentials: Some(AuthCredentials::Login {
                username: "Nova".to_string(),
                password: "pw".to_string(),
            }),
            ..test_options()
        },
        factory.clone(),
    )
    .await;
    log_in(&account, &socket).await;
    let pending = account.subscribe_market();
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "ok",
            "structuredContent": market_snapshot()
        })),
    });
    pending.await.expect("subscribe market");
    let reconnected = Arc::new(tokio::sync::Notify::new());
    let reconnected_for_handler = Arc::clone(&reconnected);
    account.on_reconnected(move || reconnected_for_handler.notify_one());

    socket.close(1006, "abnormal");
    wait_for_socket_count(&factory, 2).await;
    let reconnect_socket = factory.get(1);
    reconnect_socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    wait_for_sent_len(&reconnect_socket, 1).await;
    assert_eq!(reconnect_socket.sent()[0].action, "login");
    let login_request = reconnect_socket.last_request_id();
    reconnect_socket.server_send(RawFrame {
        kind: "logged_in".to_string(),
        request_id: Some(login_request),
        payload: Some(json!({ "player": { "username": "Nova" } })),
    });
    wait_for_sent_len(&reconnect_socket, 2).await;
    assert_eq!(reconnect_socket.sent()[1].action, "subscribe_market");
    let market_request = reconnect_socket.last_request_id();
    reconnect_socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(market_request),
        payload: Some(json!({
            "result": "ok",
            "structuredContent": market_snapshot()
        })),
    });

    tokio::time::timeout(Duration::from_millis(500), reconnected.notified())
        .await
        .expect("reconnected notification");
    assert!(account.market_subscribed());
    assert!(account.market("earth_station").is_some());
}
