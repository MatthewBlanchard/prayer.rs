mod mock_socket;

use std::sync::{Arc, Mutex};

use mock_socket::MockSocketFactory;
use serde_json::json;
use spacemolt_lib_rs::account::{Account, AccountOptions};
use spacemolt_lib_rs::notifications::{
    find_notification, NotificationChatMessage, TYPED_NOTIFICATION_TYPES,
};
use spacemolt_lib_rs::protocol::{RawFrame, WelcomePayload};

fn welcome_payload() -> WelcomePayload {
    WelcomePayload {
        version: "0.478.2".to_string(),
        release_date: "2026-07-08".to_string(),
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

async fn connected() -> (Account, mock_socket::MockSocket) {
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
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    account.wait_for_welcome().await.expect("welcome");
    (account, socket)
}

#[test]
fn generated_notifications_include_core_push_types() {
    for kind in [
        "chat_message",
        "mining_yield",
        "market_update",
        "player_died",
    ] {
        assert!(TYPED_NOTIFICATION_TYPES.contains(&kind));
        let def = find_notification(kind).expect("notification metadata");
        assert!(def.payload_type.starts_with("Notification"));
    }
}

#[tokio::test]
async fn typed_notification_listener_decodes_payloads() {
    let (account, socket) = connected().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_handler = Arc::clone(&seen);
    account.on_typed("chat_message", move |payload: &NotificationChatMessage| {
        seen_for_handler
            .lock()
            .expect("seen")
            .push(payload.content.clone());
    });

    socket.server_send(RawFrame {
        kind: "chat_message".to_string(),
        request_id: None,
        payload: Some(json!({
            "channel": "global",
            "content": "hello from typed notifications",
            "sender": "Nova"
        })),
    });

    tokio::time::timeout(std::time::Duration::from_millis(500), async {
        loop {
            if !seen.lock().expect("seen").is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("typed listener");

    assert_eq!(
        seen.lock().expect("seen").as_slice(),
        &[Some("hello from typed notifications".to_string())]
    );
}
