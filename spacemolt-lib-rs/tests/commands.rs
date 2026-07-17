mod mock_socket;

use std::sync::Arc;

use mock_socket::{MockSocket, MockSocketFactory};
use serde_json::json;
use spacemolt_lib_rs::account::{Account, AccountOptions};
use spacemolt_lib_rs::commands::SpacemoltJumpParams;
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
    socket.server_send(RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(serde_json::to_value(welcome_payload()).expect("welcome")),
    });
    account.wait_for_welcome().await.expect("welcome");
    (account, socket)
}

#[tokio::test]
async fn commands_facade_dispatches_a_query_with_the_right_tool_and_action() {
    let (account, socket) = connected().await;

    let pending = account.commands().spacemolt().get_status();
    let request_id = socket.last_request_id();
    socket.server_send(RawFrame {
        kind: "result".to_string(),
        request_id: Some(request_id),
        payload: Some(json!({
            "result": "ok",
            "structuredContent": { "credits": 5000 }
        })),
    });

    let res = pending.await.expect("query");
    let state = res.structured_content.expect("state");
    assert_eq!(state.credits, Some(5000));
    let sent = socket.sent();
    assert_eq!(sent.last().expect("sent").tool, "spacemolt");
    assert_eq!(sent.last().expect("sent").action, "get_status");
}

#[tokio::test]
async fn commands_facade_dispatches_a_mutation_and_forwards_typed_params() {
    let (account, socket) = connected().await;

    let pending = account.commands().spacemolt().jump(SpacemoltJumpParams {
        id: "sol".to_string(),
    });
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
            "tick": 9,
            "result": {
                "location": { "system_id": "sol" },
                "details": {
                    "action": "jumped",
                    "from_system": "alpha",
                    "system": "Sol",
                    "system_id": "sol",
                    "navigation_xp": 5
                }
            }
        })),
    });

    let res = pending.await.expect("mutation");
    assert_eq!(res.tick, 9);
    assert_eq!(res.delta["location"]["system_id"], "sol");
    let details = res.details.expect("jump details");
    let spacemolt_lib_rs::schema::JumpCommandResponse::JumpResponse(details) = details else {
        panic!("expected direct jump response");
    };
    assert_eq!(details.system_id, "sol");
    assert_eq!(details.navigation_xp, 5);
    assert_eq!(
        socket.sent().last().expect("sent").payload,
        Some(json!({ "id": "sol" }))
    );
}

#[test]
fn commands_facade_is_grouped_by_tool() {
    let account = Account::new(AccountOptions::default());
    let commands = account.commands();
    let _market = commands.spacemolt_market();
    let _spacemolt = commands.spacemolt();
}
