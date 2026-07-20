use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::engine::GalaxyData;
use crate::read_context::ExecutionReadContext;
use prayer_actions::ActionArg;

fn command(action: &str, args: Vec<ActionArg>) -> ResolvedAction {
    ResolvedAction {
        action: action.to_string(),
        args,
        source_line: None,
    }
}

fn planner(action: &str, args: Vec<ActionArg>) -> CommandPlanner {
    CommandPlanner::new(command(action, args), None, HashSet::new())
}

fn open_order(
    order_id: &str,
    item_id: &str,
    side: &str,
    price_each: i64,
    quantity: i64,
) -> spacemolt_lib_rs::schema::ExchangeOrder {
    spacemolt_lib_rs::schema::ExchangeOrder {
        created_at: String::new(),
        created_by: None,
        faction_order: None,
        filled_quantity: 0,
        item_id: item_id.to_string(),
        item_name: None,
        listing_fee: 0,
        order_id: order_id.to_string(),
        order_type: side.to_string(),
        price_each,
        quantity,
        remaining: quantity,
        side: side.to_string(),
    }
}

fn docked_state(system: &str, poi: &str) -> PlanningState {
    let poi_record = crate::state::PoiKnowledge {
        id: poi.to_string(),
        system_id: system.to_string(),
        info: crate::state::PoiInfoData {
            poi_type: "station".into(),
            base_id: Some(poi.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    PlanningState {
        system: Some(system.to_string()),
        current_poi: Some(poi.to_string()),
        docked: true,
        galaxy: Arc::new(GalaxyData {
            poi_records: HashMap::from([(poi.to_string(), poi_record)]),
            ..GalaxyData::default()
        }),
        ..PlanningState::default()
    }
}

fn expect_complete(op: RuntimeOperation) -> EngineExecutionResult {
    match op {
        RuntimeOperation::Complete { result } => Some(result),
        _ => None,
    }
    .expect("expected Complete operation")
}

fn expect_api_call(op: RuntimeOperation) -> (String, Option<Value>) {
    match op {
        RuntimeOperation::SpaceMoltAction { action, payload } => Some((action, payload)),
        _ => None,
    }
    .expect("expected SpaceMoltAction operation")
}

fn expect_wait_tick(op: RuntimeOperation) -> (String, Duration) {
    match op {
        RuntimeOperation::WaitTick {
            message,
            resume_after,
        } => Some((message, resume_after)),
        _ => None,
    }
    .expect("expected WaitTick operation")
}

fn expect_complete_after_wait(op: RuntimeOperation) -> (String, Duration) {
    match op {
        RuntimeOperation::CompleteAfterWait {
            message,
            resume_after,
        } => Some((message, resume_after)),
        _ => None,
    }
    .expect("expected CompleteAfterWait operation")
}

fn api_failure(message: impl Into<String>) -> ApiOutcome {
    ApiOutcome::Failure(OperationFailure::Policy(message.into()))
}

#[test]
fn api_outcome_preserves_structured_transient_error() {
    let mut server = spacemolt_lib_rs::SpacemoltError::new("rate_limited", "Too many requests");
    server.details = Some(serde_json::json!({ "retry_after": 4 }));
    let error = require_success(Some(ApiOutcome::Failure(OperationFailure::Client(
        spacemolt_lib_rs::ClientError::Server(server),
    ))))
    .expect_err("expected failure");

    assert_eq!(error.server_code(), Some("rate_limited"));
    assert!(error.is_transient());
    assert_eq!(error.retry_after(), Some(Duration::from_secs(4)));
}

#[test]
fn unload_passenger_all_completes_when_manifest_observed_empty() {
    let mut state = PlanningState::default();
    state.passengers.aboard_count = Some(0);

    let mut p = planner("unload_passenger", vec![ActionArg::Any("all".to_string())]);

    let result = expect_complete(p.next(&state, None).expect("start"));

    assert!(result.completed);
    assert!(!result.halt_script);
    assert_eq!(
        result.result_message.as_deref(),
        Some("No passengers aboard; unload_passenger all already complete.")
    );
}

#[test]
fn unload_passenger_all_still_calls_api_when_manifest_unknown() {
    let mut p = planner("unload_passenger", vec![ActionArg::Any("all".to_string())]);

    let (action, payload) = expect_api_call(
        p.next(&docked_state("sol", "station_1"), None)
            .expect("start"),
    );

    assert_eq!(action, "spacemolt/unload_passenger");
    assert_eq!(payload, Some(serde_json::json!({ "id": "all" })));
}

#[test]
fn wait_spends_paced_ticks_then_completes() {
    let state = PlanningState::default();
    let mut continuation = None;
    for remaining in (1..=2u64).rev() {
        let mut p = CommandPlanner::new(
            command("wait", vec![ActionArg::Integer(2)]),
            continuation.take(),
            HashSet::new(),
        );
        let (message, resume_after) = expect_wait_tick(p.next(&state, None).expect("plan"));
        assert_eq!(
            message,
            format!("Waiting ({remaining} tick(s) remaining)...")
        );
        assert_eq!(resume_after, TICK_PAUSE);
        continuation = p.continuation();
    }

    let mut p = CommandPlanner::new(
        command("wait", vec![ActionArg::Integer(2)]),
        continuation,
        HashSet::new(),
    );
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert!(result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Waited 2 tick(s)."));
}

#[test]
fn wait_resumes_from_restored_continuation() {
    let continuation = Some(ActiveCommandState::Wait(WaitState {
        total_ticks: 5,
        remaining_ticks: 1,
        origin: None,
    }));
    let mut p = CommandPlanner::new(command("wait", vec![]), continuation, HashSet::new());
    let (message, _) = expect_wait_tick(p.next(&PlanningState::default(), None).expect("plan"));
    assert_eq!(message, "Waiting (1 tick(s) remaining)...");
    assert_eq!(
        p.continuation(),
        Some(ActiveCommandState::Wait(WaitState {
            total_ticks: 5,
            remaining_ticks: 0,
            origin: None,
        }))
    );
}

#[test]
fn wait_in_transit_does_not_consume_ticks() {
    let state = PlanningState {
        in_transit: true,
        ..PlanningState::default()
    };
    let continuation = Some(ActiveCommandState::Wait(WaitState {
        total_ticks: 3,
        remaining_ticks: 3,
        origin: None,
    }));
    let mut p = CommandPlanner::new(
        command("wait", vec![]),
        continuation.clone(),
        HashSet::new(),
    );
    let (message, _) = expect_wait_tick(p.next(&state, None).expect("plan"));
    assert!(message.contains("In transit to destination"));
    assert_eq!(p.continuation(), continuation);
}

#[test]
fn wait_ticks_are_clamped_to_30() {
    let cmd = command("wait", vec![ActionArg::Integer(99)]);
    assert_eq!(parse_wait_ticks(&cmd), 30);
    let cmd = command("wait", vec![]);
    assert_eq!(parse_wait_ticks(&cmd), 1);
}

#[test]
fn halt_completes_without_api_calls() {
    let mut p = planner("halt", vec![]);
    let result = expect_complete(p.next(&PlanningState::default(), None).expect("plan"));
    assert!(result.halt_script);
    assert!(result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Script halted."));
}

#[test]
fn typed_action_uses_the_existing_planner_path() {
    let mut planner =
        CommandPlanner::from_action(Action::Halt, None, HashSet::new()).expect("typed planner");
    let result = expect_complete(
        planner
            .next(&PlanningState::default(), None)
            .expect("plan typed action"),
    );
    assert!(result.halt_script);
}

#[test]
fn targetless_scan_uses_area_scan_mutation() {
    let mut p = planner("scan", vec![]);
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).unwrap());
    assert_eq!(action, "spacemolt/scan");
    assert_eq!(payload, Some(serde_json::json!({})));
}

#[test]
fn module_transfer_loots_named_wreck_module() {
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("module".into()),
            ActionArg::ModuleId("module-7".into()),
            ActionArg::Any("space:wreck-3".into()),
            ActionArg::Any("cargo".into()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).unwrap());
    assert_eq!(action, "spacemolt_storage/loot");
    assert_eq!(
        payload,
        Some(serde_json::json!({"wreck_id":"wreck-3", "module_id":"module-7"}))
    );
}

#[test]
fn commission_transfer_supplies_materials() {
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".into()),
            ActionArg::ItemId("steel".into()),
            ActionArg::Integer(12),
            ActionArg::Any("cargo".into()),
            ActionArg::Any("commission:commission-2".into()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).unwrap());
    assert_eq!(action, "spacemolt_ship/supply_commission");
    assert_eq!(
        payload,
        Some(serde_json::json!({"id":"commission-2", "item_id":"steel", "quantity":12}))
    );
}

#[test]
fn buy_order_can_route_delivery_to_storage() {
    let mut p = planner(
        "buy",
        vec![
            ActionArg::ItemId("iron".into()),
            ActionArg::Integer(4),
            ActionArg::Integer(10),
            ActionArg::Any("order".into()),
            ActionArg::Any("deliver_to=storage".into()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&station_market_state(), None).unwrap());
    assert_eq!(action, "spacemolt_market/create_buy_order");
    assert_eq!(payload.unwrap()["deliver_to"], "storage");
}

#[test]
fn ported_command_in_transit_yields_paced_wait_tick() {
    let state = PlanningState {
        in_transit: true,
        transit_type: Some("travel".to_string()),
        transit_dest_poi: Some("jovian_extraction_zone".to_string()),
        ..PlanningState::default()
    };
    let mut p = planner("dock", vec![]);
    let (message, resume_after) = match p.next(&state, None).expect("plan") {
        RuntimeOperation::WaitTick {
            message,
            resume_after,
        } => Some((message, resume_after)),
        _ => None,
    }
    .expect("expected WaitTick operation");
    assert_eq!(message, "In travel to jovian_extraction_zone; waiting...");
    assert_eq!(resume_after, TICK_PAUSE);
}

#[test]
fn transit_wait_message_prefers_destination_poi() {
    let state = PlanningState {
        system: Some("sol".to_string()),
        current_poi: Some("sol".to_string()),
        in_transit: true,
        transit_type: Some("travel".to_string()),
        transit_dest_poi: Some("jovian_extraction_zone".to_string()),
        ..PlanningState::default()
    };

    assert_eq!(
        transit_wait_message(&state),
        "In travel to jovian_extraction_zone; waiting..."
    );
}

#[test]
fn dock_when_already_docked_completes() {
    let mut p = planner("dock", vec![]);
    let result = expect_complete(
        p.next(&docked_state("sol", "station_1"), None)
            .expect("plan"),
    );
    assert!(result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Docked."));
}

#[test]
fn say_system_calls_social_chat() {
    let mut p = planner(
        "say",
        vec![
            ActionArg::Any("hello there".to_string()),
            ActionArg::Any("system".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    let payload = payload.expect("payload");
    assert_eq!(action, "spacemolt_social/chat");
    assert_eq!(payload["target"], Value::String("system".to_string()));
    assert_eq!(payload["content"], Value::String("hello there".to_string()));
}

#[test]
fn say_private_requires_and_sends_target() {
    let mut p = planner(
        "say",
        vec![
            ActionArg::Any("ping".to_string()),
            ActionArg::Any("private".to_string()),
            ActionArg::Any("Rowan Pike".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    let payload = payload.expect("payload");
    assert_eq!(action, "spacemolt_social/chat");
    assert_eq!(payload["target"], Value::String("private".to_string()));
    assert_eq!(
        payload["target_id"],
        Value::String("Rowan Pike".to_string())
    );
}

#[test]
fn say_emergency_is_rejected() {
    let mut p = planner(
        "say",
        vec![
            ActionArg::Any("help".to_string()),
            ActionArg::Any("emergency".to_string()),
        ],
    );
    let err = p
        .next(&PlanningState::default(), None)
        .expect_err("emergency is read-only");
    assert!(err.to_string().contains("emergency chat is read-only"));
}

#[test]
fn battle_stance_uses_v2_payload_key() {
    let mut p = planner("stance", vec![ActionArg::Any("evade".to_string())]);
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    assert_eq!(action, "spacemolt_battle/stance");
    assert_eq!(
        payload.expect("payload"),
        serde_json::json!({ "stance": "evade" })
    );
}

#[test]
fn battle_target_uses_v2_payload_key() {
    let mut p = planner("target", vec![ActionArg::Any("SomePlayer".to_string())]);
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    assert_eq!(action, "spacemolt_battle/target");
    assert_eq!(
        payload.expect("payload"),
        serde_json::json!({ "target_id": "SomePlayer" })
    );
}

#[test]
fn battle_reload_uses_v2_payload_keys() {
    let mut p = planner(
        "reload",
        vec![
            ActionArg::Any("weapon_1".to_string()),
            ActionArg::Any("standard_rounds_box".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    assert_eq!(action, "spacemolt_battle/reload");
    assert_eq!(
        payload.expect("payload"),
        serde_json::json!({
            "weapon_instance_id": "weapon_1",
            "ammo_item_id": "standard_rounds_box",
        })
    );
}

#[test]
fn attack_uses_v2_target_id_payload_key() {
    let mut p = planner("attack", vec![ActionArg::Any("pirate_1".to_string())]);
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    assert_eq!(action, "spacemolt/attack");
    assert_eq!(
        payload.expect("payload"),
        serde_json::json!({ "target_id": "pirate_1" })
    );
}

#[test]
fn dock_without_candidates_completes_with_notice() {
    let state = PlanningState {
        system: Some("sol".to_string()),
        ..PlanningState::default()
    };
    let mut p = planner("dock", vec![]);
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert_eq!(
        result.result_message.as_deref(),
        Some("No dockable base available in the current system.")
    );
}

#[test]
fn dock_when_undocked_at_target_issues_dock_then_incomplete() {
    let mut state = docked_state("sol", "station_1");
    state.docked = false;
    let mut p = planner("dock", vec![]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/dock");
    assert!(payload.is_none());

    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
            .expect("plan"),
    );
    assert!(!result.completed);
    assert_eq!(
        result.result_message.as_deref(),
        Some("Docking at station_1...")
    );
}

#[test]
fn set_home_when_docked_calls_set_home_base() {
    let state = docked_state("sol", "station_1");
    let mut p = planner("set_home", vec![]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/set_home_base");
    assert_eq!(
        payload.expect("payload")["base_id"],
        Value::String("station_1".to_string())
    );

    let response = serde_json::json!({ "result": { "message": "Home set." } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("plan"),
    );
    assert!(result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Home set."));
}

#[test]
fn passthrough_command_maps_to_api_action() {
    let mut p = planner("survey", vec![]);
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    assert_eq!(action, "spacemolt/survey_system");
    assert_eq!(payload, Some(Value::Object(Default::default())));
}

#[test]
fn faction_invite_maps_quoted_username_to_api_action() {
    let mut engine = crate::engine::RuntimeEngine::default();
    engine
        .set_script(
            "faction_invite \"Pike Market Bot 4\";",
            Some(ExecutionReadContext::default()),
        )
        .expect("faction command should parse");
    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command.clone(), None, HashSet::new());
    let (action, payload) = expect_api_call(
        planner
            .next(&PlanningState::default(), None)
            .expect("plan faction invite"),
    );
    assert_eq!(action, "spacemolt_faction/invite");
    assert_eq!(payload.expect("payload")["id"], "Pike Market Bot 4");
}

#[test]
fn faction_set_role_maps_to_promote_payload_keys() {
    let mut engine = crate::engine::RuntimeEngine::default();
    engine
        .set_script(
            "faction_set_role \"Joshua Aldrac\" officer;",
            Some(ExecutionReadContext::default()),
        )
        .expect("faction command should parse");
    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command.clone(), None, HashSet::new());
    let (action, payload) = expect_api_call(
        planner
            .next(&PlanningState::default(), None)
            .expect("plan faction set role"),
    );
    let payload = payload.expect("payload");
    assert_eq!(action, "spacemolt_faction_admin/promote");
    assert_eq!(payload["player_id"], "Joshua Aldrac");
    assert_eq!(payload["role_id"], "officer");
    assert!(payload.get("id").is_none());
    assert!(payload.get("text").is_none());
}

#[test]
fn faction_facility_build_maps_to_facility_api() {
    let mut engine = crate::engine::RuntimeEngine::default();
    engine
        .set_script(
            "faction_facility_build faction_storage;",
            Some(ExecutionReadContext::default()),
        )
        .expect("facility command should parse");
    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command.clone(), None, HashSet::new());
    let (action, payload) = expect_api_call(
        planner
            .next(&docked_state("sol", "station_1"), None)
            .expect("plan facility build"),
    );
    assert_eq!(action, "spacemolt_facility/faction_build");
    assert_eq!(
        payload.expect("payload")["facility_type"],
        "faction_storage"
    );
}

#[test]
fn found_station_maps_to_facility_api_without_docking_requirement() {
    let mut engine = crate::engine::RuntimeEngine::default();
    engine
        .set_script(
            "found_station \"Freeport Alpha\" false;",
            Some(ExecutionReadContext::default()),
        )
        .expect("found station command should parse");
    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command.clone(), None, HashSet::new());
    let (action, payload) = expect_api_call(
        planner
            .next(&PlanningState::default(), None)
            .expect("plan station founding"),
    );
    assert_eq!(action, "spacemolt_facility/found_station");
    let payload = payload.expect("payload");
    assert_eq!(payload["name"], "Freeport Alpha");
    assert_eq!(payload["public_access"], false);
}

#[test]
fn facility_upgrade_maps_ids_and_type_to_facility_api() {
    let mut engine = crate::engine::RuntimeEngine::default();
    engine
        .set_script(
            "facility_upgrade \"facility 12\" advanced_workshop;",
            Some(ExecutionReadContext::default()),
        )
        .expect("facility command should parse");
    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command, None, HashSet::new());
    let (action, payload) = expect_api_call(
        planner
            .next(&docked_state("sol", "station_1"), None)
            .expect("plan facility upgrade"),
    );
    let payload = payload.expect("payload");
    assert_eq!(action, "spacemolt_facility/upgrade");
    assert_eq!(payload["facility_id"], "facility 12");
    assert_eq!(payload["facility_type"], "advanced_workshop");
}

#[test]
fn facility_config_commands_map_to_facility_api() {
    let state = docked_state("sol", "station_1");
    let mut engine = crate::engine::RuntimeEngine::default();
    engine
            .set_script(
                "facility_set_access \"facility 12\" public;\nfacility_set_output_price \"facility 12\" steel_plate 150;\nfacility_set_name \"facility 12\" \"North Mill\";",
                Some(ExecutionReadContext::default()),
            )
            .expect("facility config commands should parse");

    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command.clone(), None, HashSet::new());
    let (action, payload) = expect_api_call(planner.next(&state, None).expect("plan set access"));
    let payload = payload.expect("payload");
    assert_eq!(action, "spacemolt_facility/set_access");
    assert_eq!(payload["facility_id"], "facility 12");
    assert_eq!(payload["access"], "public");
    engine.execute_result(
        &command,
        EngineExecutionResult::default(),
        ExecutionReadContext::default(),
    );

    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command.clone(), None, HashSet::new());
    let (action, payload) =
        expect_api_call(planner.next(&state, None).expect("plan set output price"));
    let payload = payload.expect("payload");
    assert_eq!(action, "spacemolt_facility/set_output_price");
    assert_eq!(payload["facility_id"], "facility 12");
    assert_eq!(payload["item_id"], "steel_plate");
    assert_eq!(payload["price"], 150);
    engine.execute_result(
        &command,
        EngineExecutionResult::default(),
        ExecutionReadContext::default(),
    );

    let command = engine
        .decide_next(ExecutionReadContext::default())
        .expect("decide")
        .expect("command");
    let mut planner = CommandPlanner::new(command, None, HashSet::new());
    let (action, payload) = expect_api_call(planner.next(&state, None).expect("plan set name"));
    let payload = payload.expect("payload");
    assert_eq!(action, "spacemolt_facility/set_name");
    assert_eq!(payload["facility_id"], "facility 12");
    assert_eq!(payload["custom_name"], "North Mill");
}

#[test]
fn scrap_ship_requires_ship_id() {
    let mut p = planner("scrap_ship", vec![]);
    let err = p
        .next(&PlanningState::default(), None)
        .expect_err("scrap_ship without id should be rejected");
    assert_eq!(
        err.to_string(),
        "unsupported command 'scrap_ship requires a ship id'"
    );
}

#[test]
fn scrap_ship_maps_to_ship_api() {
    let mut p = planner(
        "scrap_ship",
        vec![ActionArg::ShipId("ship_abc123".to_string())],
    );
    let (action, payload) = expect_api_call(p.next(&PlanningState::default(), None).expect("plan"));
    assert_eq!(action, "spacemolt_ship/scrap_ship");
    assert_eq!(payload.expect("payload")["ship_id"], "ship_abc123");
}

#[test]
fn scrap_ship_can_run_in_transit() {
    let mut state = PlanningState {
        in_transit: true,
        ..PlanningState::default()
    };
    state.transit_dest_system = Some("alpha".to_string());
    let mut p = planner(
        "scrap_ship",
        vec![ActionArg::ShipId("ship_abc123".to_string())],
    );
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_ship/scrap_ship");
}

#[test]
fn unknown_command_errors_like_transport() {
    let mut p = planner("warp_drive", vec![]);
    let err = p
        .next(&PlanningState::default(), None)
        .expect_err("expected error");
    assert_eq!(err.to_string(), "unsupported command 'warp_drive'");
}

#[test]
fn every_high_level_command_plans_a_first_operation() {
    for action in [
        "wait",
        "mine",
        "go",
        "find",
        "refuel",
        "dock",
        "set_home",
        "transfer",
        "buy",
        "sell",
        "cancel_buy",
        "cancel_sell",
        "halt",
    ] {
        let mut p = planner(action, vec![ActionArg::Any("x".to_string())]);
        // Planning may complete, call out, or error on the bare state —
        // but every command is handled engine-side without delegation.
        let _ = p.next(&PlanningState::default(), None);
    }
}

fn nav_state() -> PlanningState {
    // sol -- alpha -- beta; ship undocked at sol/poi_home.
    PlanningState {
        system: Some("sol".to_string()),
        current_poi: Some("poi_home".to_string()),
        galaxy: Arc::new(GalaxyData {
            system_records: HashMap::from([
                (
                    "sol".into(),
                    crate::state::SystemKnowledge {
                        id: "sol".into(),
                        connections: vec!["alpha".into()],
                        ..Default::default()
                    },
                ),
                (
                    "alpha".into(),
                    crate::state::SystemKnowledge {
                        id: "alpha".into(),
                        connections: vec!["sol".into(), "beta".into()],
                        ..Default::default()
                    },
                ),
                (
                    "beta".into(),
                    crate::state::SystemKnowledge {
                        id: "beta".into(),
                        connections: vec!["alpha".into()],
                        ..Default::default()
                    },
                ),
            ]),
            poi_records: HashMap::from([
                (
                    "poi_home".into(),
                    crate::state::PoiKnowledge {
                        id: "poi_home".into(),
                        system_id: "sol".into(),
                        ..Default::default()
                    },
                ),
                (
                    "poi_mine".into(),
                    crate::state::PoiKnowledge {
                        id: "poi_mine".into(),
                        system_id: "sol".into(),
                        ..Default::default()
                    },
                ),
                (
                    "poi_far".into(),
                    crate::state::PoiKnowledge {
                        id: "poi_far".into(),
                        system_id: "beta".into(),
                        ..Default::default()
                    },
                ),
            ]),
            ..GalaxyData::default()
        }),
        ..PlanningState::default()
    }
}

#[test]
fn go_when_already_at_target_completes() {
    let state = nav_state();
    let mut p = planner("go", vec![ActionArg::GoTarget("poi_home".to_string())]);
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert!(result.completed);
    assert_eq!(
        result.result_message.as_deref(),
        Some("Already at poi_home.")
    );
}

#[test]
fn go_travels_within_system_and_records_continuation() {
    let state = nav_state();
    let mut p = planner("go", vec![ActionArg::GoTarget("poi_mine".to_string())]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/travel");
    assert_eq!(
        payload.expect("payload")["target_poi"],
        Value::String("poi_mine".to_string())
    );

    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
            .expect("plan"),
    );
    assert!(!result.completed);
    assert_eq!(
        result.result_message.as_deref(),
        Some("Traveling to poi_mine...")
    );

    let Some(ActiveCommandState::Go(go)) = p.continuation() else {
        unreachable!("go continuation expected");
    };
    assert_eq!(go.resolved_system.as_deref(), Some("sol"));
    assert_eq!(go.resolved_poi.as_deref(), Some("poi_mine"));
    assert!(go.did_move);
}

#[test]
fn go_jumps_along_next_hop_toward_remote_system() {
    let state = nav_state();
    let mut p = planner("go", vec![ActionArg::GoTarget("beta".to_string())]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/jump");
    assert_eq!(
        payload.expect("payload")["target_system"],
        Value::String("alpha".to_string())
    );
}

#[test]
fn go_starts_fresh_when_continuation_target_changes() {
    let state = nav_state();
    let continuation = Some(ActiveCommandState::Go(GoState {
        target: "beta".to_string(),
        resolved_system: Some("beta".to_string()),
        did_move: true,
        ..GoState::default()
    }));
    let mut p = CommandPlanner::new(
        command("go", vec![ActionArg::GoTarget("poi_mine".to_string())]),
        continuation,
        HashSet::new(),
    );

    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/travel");
    assert_eq!(
        payload.expect("payload")["target_poi"],
        Value::String("poi_mine".to_string())
    );

    let Some(ActiveCommandState::Go(go)) = p.continuation() else {
        unreachable!("go continuation expected");
    };
    assert_eq!(go.target, "poi_mine");
    assert_eq!(go.resolved_system.as_deref(), Some("sol"));
    assert_eq!(go.resolved_poi.as_deref(), Some("poi_mine"));
}

#[test]
fn go_undocks_first_when_docked() {
    let mut state = nav_state();
    state.docked = true;
    let mut p = planner("go", vec![ActionArg::GoTarget("poi_mine".to_string())]);
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/undock");
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
            .expect("plan"),
    );
    assert!(!result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Undocking..."));
}

#[test]
fn transit_network_failure_confirms_against_fresh_state() {
    let state = nav_state();
    let mut p = planner("go", vec![ActionArg::GoTarget("poi_mine".to_string())]);
    let _ = expect_api_call(p.next(&state, None).expect("plan"));

    let op = p
        .next(
            &state,
            Some(ApiOutcome::Failure(OperationFailure::Client(
                spacemolt_lib_rs::ClientError::Timeout("timeout".to_string()),
            ))),
        )
        .expect("plan");
    assert!(matches!(op, RuntimeOperation::RefreshState));

    let mut fresh = nav_state();
    fresh.in_transit = true;
    fresh.transit_dest_poi = Some("poi_mine".to_string());
    let result = expect_complete(
        p.next(&fresh, Some(ApiOutcome::Success(Value::Null)))
            .expect("plan"),
    );
    assert!(!result.completed);
    assert_eq!(
        result.result_message.as_deref(),
        Some("Traveling to poi_mine...")
    );
}

#[test]
fn transit_network_failure_without_transit_surfaces_original_error() {
    let state = nav_state();
    let mut p = planner("go", vec![ActionArg::GoTarget("poi_mine".to_string())]);
    let _ = expect_api_call(p.next(&state, None).expect("plan"));
    let op = p
        .next(
            &state,
            Some(ApiOutcome::Failure(OperationFailure::Client(
                spacemolt_lib_rs::ClientError::Timeout("timeout".to_string()),
            ))),
        )
        .expect("plan");
    assert!(matches!(op, RuntimeOperation::RefreshState));
    let err = p
        .next(&state, Some(ApiOutcome::Success(Value::Null)))
        .expect_err("expected error");
    assert_eq!(err.to_string(), "timeout: timeout");
}

#[test]
fn refuel_completes_when_fuel_full() {
    let mut state = nav_state();
    state.fuel_pct = 100;
    let mut p = planner("refuel", vec![]);
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert_eq!(result.result_message.as_deref(), Some("Fuel already full."));
}

#[test]
fn refuel_uses_cargo_without_known_station() {
    let state = nav_state();
    let mut p = planner("refuel", vec![]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/refuel");
    assert_eq!(payload, Some(serde_json::json!({})));

    let response = serde_json::json!({ "result": { "message": "Used cargo fuel cells." } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("plan"),
    );
    assert!(result.completed);
    assert!(!result.halt_script);
    assert_eq!(
        result.result_message.as_deref(),
        Some("Used cargo fuel cells.")
    );
}

#[test]
fn refuel_keeps_its_original_station_when_nearest_station_changes() {
    let station = |id: &str, system_id: &str| crate::state::PoiKnowledge {
        id: id.to_string(),
        system_id: system_id.to_string(),
        info: crate::state::PoiInfoData {
            poi_type: "station".into(),
            base_id: Some(id.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let galaxy = Arc::new(GalaxyData {
        system_records: HashMap::from([
            (
                "alpha".into(),
                crate::state::SystemKnowledge {
                    id: "alpha".into(),
                    connections: vec!["beta".into()],
                    empire: Some("solarian".into()),
                    ..Default::default()
                },
            ),
            (
                "beta".into(),
                crate::state::SystemKnowledge {
                    id: "beta".into(),
                    connections: vec!["alpha".into(), "gamma".into()],
                    empire: Some("solarian".into()),
                    ..Default::default()
                },
            ),
            (
                "gamma".into(),
                crate::state::SystemKnowledge {
                    id: "gamma".into(),
                    connections: vec!["beta".into()],
                    empire: Some("solarian".into()),
                    ..Default::default()
                },
            ),
        ]),
        poi_records: HashMap::from([
            ("alpha_station".into(), station("alpha_station", "alpha")),
            ("gamma_station".into(), station("gamma_station", "gamma")),
        ]),
        ..GalaxyData::default()
    });

    let initial = PlanningState {
        system: Some("alpha".into()),
        current_poi: Some("alpha_field".into()),
        galaxy: Arc::clone(&galaxy),
        ..PlanningState::default()
    };
    let mut first = planner("refuel", vec![]);
    let _ = first.next(&initial, None).expect("select initial station");
    let continuation = first.continuation();
    let Some(ActiveCommandState::Refuel(selected)) = continuation.as_ref() else {
        panic!("refuel continuation expected");
    };
    assert_eq!(selected.target_poi.as_deref(), Some("alpha_station"));

    // At gamma, a fresh nearest-station query would select gamma_station. A
    // resumed refuel command must remain pinned to alpha_station instead.
    let moved = PlanningState {
        system: Some("gamma".into()),
        current_poi: Some("gamma_field".into()),
        galaxy,
        ..PlanningState::default()
    };
    let mut resumed = CommandPlanner::new(command("refuel", vec![]), continuation, HashSet::new());
    let _ = resumed.next(&moved, None).expect("resume refuel");
    let Some(ActiveCommandState::Refuel(selected)) = resumed.continuation() else {
        panic!("refuel continuation expected");
    };
    assert_eq!(selected.target_system.as_deref(), Some("alpha"));
    assert_eq!(selected.target_poi.as_deref(), Some("alpha_station"));
}

#[test]
fn find_with_unknown_target_halts_with_suggestion() {
    let mut state = nav_state();
    let mut catalog = state.catalog.as_ref().clone();
    catalog.items.insert(
        "iron_ore".to_string(),
        serde_json::from_value(serde_json::json!({
            "base_value": 1, "category": "ore", "description": "Iron Ore",
            "id": "iron_ore", "name": "Iron Ore", "size": 1,
            "stackable": true, "tradeable": true
        }))
        .expect("catalog item"),
    );
    state.catalog = Arc::new(catalog);
    let mut p = planner("find", vec![ActionArg::Any("iron_or".to_string())]);
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert!(result.halt_script);
    let message = result.result_message.expect("message");
    assert!(message.contains("Unknown target `iron_or`"));
    assert!(message.contains("Did you mean `iron_ore`?"));
}

fn mining_state() -> PlanningState {
    let mut state = nav_state();
    let mut galaxy = state.galaxy.as_ref().clone();
    galaxy
        .poi_records
        .get_mut("poi_mine")
        .expect("mine POI")
        .resources = vec![crate::state::PoiResourceData {
        resource_id: "iron".into(),
        ..Default::default()
    }];
    state.galaxy = Arc::new(galaxy);
    state.cargo_capacity = 100;
    state
}

fn station_market_state() -> PlanningState {
    let mut state = nav_state();
    let mut galaxy = state.galaxy.as_ref().clone();
    let home = &mut galaxy
        .poi_records
        .get_mut("poi_home")
        .expect("home POI")
        .info;
    home.poi_type = "station".into();
    home.base_id = Some("poi_home".into());
    state.galaxy = Arc::new(galaxy);
    state.docked = true;
    state.cargo_capacity = 100;
    state
}

#[test]
fn mine_at_target_strikes_and_continues() {
    let mut state = mining_state();
    state.current_poi = Some("poi_mine".to_string());
    let mut p = planner("mine", vec![ActionArg::ItemId("iron".to_string())]);
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/mine");

    let response = serde_json::json!({ "result": { "message": "Mined 3 iron." } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("plan"),
    );
    assert!(!result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Mined 3 iron."));
    let Some(ActiveCommandState::Mine(mine)) = p.continuation() else {
        unreachable!("mine continuation expected");
    };
    assert_eq!(mine.target_poi.as_deref(), Some("poi_mine"));
}

#[test]
fn mine_depleted_response_retries_without_extra_wait() {
    let mut state = mining_state();
    state.current_poi = Some("poi_mine".to_string());
    let mut p = planner("mine", vec![ActionArg::ItemId("iron".to_string())]);
    let _ = expect_api_call(p.next(&state, None).expect("plan"));

    let response = serde_json::json!({ "error": { "code": "depleted" }, "result": { "message": "Node depleted." } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("plan"),
    );
    assert!(!result.completed);
    assert!(!result.halt_script);
    assert_eq!(result.result_message.as_deref(), Some("Node depleted."));
}

#[test]
fn mine_depleted_api_failure_retries_without_failure() {
    let mut state = mining_state();
    state.current_poi = Some("poi_mine".to_string());
    let mut p = planner("mine", vec![ActionArg::ItemId("iron".to_string())]);
    let _ = expect_api_call(p.next(&state, None).expect("plan"));

    let result = expect_complete(
            p.next(
                &state,
                Some(api_failure(
                    "api failure (400): {\"error\":{\"code\":\"depleted\",\"message\":\"Node depleted.\"}}",
                )),
            )
            .expect("plan"),
        );
    assert!(!result.completed);
    assert!(!result.halt_script);
    assert_eq!(
        result.result_message.as_deref(),
        Some("`poi_mine` is depleted; retrying...")
    );
}

#[test]
fn mine_cargo_full_api_failure_completes_successfully() {
    let mut state = mining_state();
    state.current_poi = Some("poi_mine".to_string());
    let mut p = planner("mine", vec![ActionArg::ItemId("iron".to_string())]);
    let _ = expect_api_call(p.next(&state, None).expect("plan"));

    let result = expect_complete(
            p.next(
                &state,
                Some(api_failure(
                    "api failure (400): {\"error\":{\"code\":\"cargo_full\",\"message\":\"Cargo hold is full\"}}",
                )),
            )
            .expect("plan"),
        );
    assert!(result.completed);
    assert!(!result.halt_script);
    assert_eq!(result.result_message.as_deref(), Some("Cargo is full."));
}

#[test]
fn mine_cargo_full_error_payload_completes_successfully() {
    let mut state = mining_state();
    state.current_poi = Some("poi_mine".to_string());
    let mut p = planner("mine", vec![ActionArg::ItemId("iron".to_string())]);
    let _ = expect_api_call(p.next(&state, None).expect("plan"));

    let response = serde_json::json!({
        "error": {
            "code": "cargo_full",
            "message": "Cargo hold is full"
        }
    });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("plan"),
    );
    assert!(result.completed);
    assert!(!result.halt_script);
    assert_eq!(result.result_message.as_deref(), Some("Cargo is full."));
}

#[test]
fn mine_targets_known_poi_without_library_blacklist() {
    let state = mining_state();
    let mut p = CommandPlanner::new(
        command("mine", vec![ActionArg::ItemId("iron".to_string())]),
        None,
        HashSet::new(),
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/travel");
    assert_eq!(
        payload.expect("payload")["target_poi"],
        Value::String("poi_mine".to_string())
    );
}

#[test]
fn mine_skips_library_blacklisted_poi() {
    let state = mining_state();
    let mut p = CommandPlanner::new(
        command("mine", vec![ActionArg::ItemId("iron".to_string())]),
        None,
        HashSet::from(["poi_mine".to_string()]),
    );
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert!(result.halt_script);
    assert_eq!(
        result.result_message.as_deref(),
        Some("No known minable locations for iron anywhere in the galaxy!")
    );
}

#[test]
fn mine_travels_toward_selected_target() {
    let state = mining_state();
    let mut p = planner("mine", vec![ActionArg::ItemId("iron".to_string())]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/travel");
    assert_eq!(
        payload.expect("payload")["target_poi"],
        Value::String("poi_mine".to_string())
    );
}

#[test]
fn transfer_item_to_faction_deposits_requested_quantity() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 5)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(3),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["source"], Value::String("cargo".to_string()));
    assert_eq!(payload["target"], Value::String("faction".to_string()));
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(3.into()));
    assert!(payload.get("items").is_none());
}

#[test]
fn transfer_item_to_player_uses_single_item_storage_shape() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("mining_laser_ii".to_string(), 5)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("mining_laser_ii".to_string()),
            ActionArg::Integer(1),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Pike Mining Bot 1".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["source"], Value::String("cargo".to_string()));
    assert_eq!(
        payload["target"],
        Value::String("Pike Mining Bot 1".to_string())
    );
    assert_eq!(
        payload["item_id"],
        Value::String("mining_laser_ii".to_string())
    );
    assert_eq!(payload["quantity"], Value::Number(1.into()));
    assert!(payload.get("items").is_none());
}

#[test]
fn transfer_missing_item_reports_source() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("mining_laser_ii".to_string()),
            ActionArg::Integer(1),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Pike Mining Bot 1".to_string()),
        ],
    );
    let err = p.next(&state, None).expect_err("missing item should fail");
    assert_eq!(
        err.to_string(),
        "Cannot transfer 1 mining_laser_ii from cargo; only 0 available."
    );
}

#[test]
fn transfer_explicit_quantity_fails_when_source_has_less() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("mining_laser_ii".to_string(), 3)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("mining_laser_ii".to_string()),
            ActionArg::Integer(5),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Pike Mining Bot 1".to_string()),
        ],
    );
    let err = p
        .next(&state, None)
        .expect_err("underfilled explicit quantity should fail");
    assert_eq!(
        err.to_string(),
        "Cannot transfer 5 mining_laser_ii from cargo; only 3 available."
    );
}

#[test]
fn transfer_omitted_quantity_missing_item_stays_noop() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("mining_laser_ii".to_string()),
            ActionArg::Any("all".to_string()),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Pike Mining Bot 1".to_string()),
        ],
    );
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert_eq!(
        result.result_message.as_deref(),
        Some("No mining_laser_ii in cargo.")
    );
}

#[test]
fn transfer_block_batches_items_in_one_storage_call() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([
        ("iron".to_string(), 5),
        ("copper".to_string(), 9),
    ]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("items".to_string()),
            ActionArg::Integer(2),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(3),
            ActionArg::ItemId("copper".to_string()),
            ActionArg::Integer(7),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["source"], Value::String("cargo".to_string()));
    assert_eq!(payload["target"], Value::String("faction".to_string()));
    assert_eq!(
        payload["items"],
        serde_json::json!([
            { "item_id": "iron", "quantity": 3 },
            { "item_id": "copper", "quantity": 7 }
        ])
    );
}

#[test]
fn transfer_block_fails_when_explicit_quantity_is_short() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([
        ("iron".to_string(), 5),
        ("copper".to_string(), 2),
    ]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("items".to_string()),
            ActionArg::Integer(2),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(3),
            ActionArg::ItemId("copper".to_string()),
            ActionArg::Integer(7),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let err = p
        .next(&state, None)
        .expect_err("underfilled block item should fail");
    assert_eq!(
        err.to_string(),
        "Cannot transfer 7 copper from cargo; only 2 available."
    );
}

#[test]
fn transfer_item_to_space_jettisons_requested_quantity() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 5)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(3),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("space".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/jettison");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(3.into()));
}

#[test]
fn transfer_item_to_space_fails_when_explicit_quantity_is_short() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 2)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(3),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("space".to_string()),
        ],
    );
    let err = p
        .next(&state, None)
        .expect_err("underfilled jettison should fail");
    assert_eq!(
        err.to_string(),
        "Cannot transfer 3 iron from cargo; only 2 available."
    );
}

#[test]
fn transfer_item_from_space_loots_visible_wreck_cargo() {
    let mut state = station_market_state();
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "wreck_1".to_string(),
            cargo: vec![spacemolt_lib_rs::data::WreckCargoItem {
                item_id: "iron".to_string(),
                quantity: 8,
                name: None,
                size: None,
            }],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(3),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/loot");
    let payload = payload.expect("payload");
    assert_eq!(payload["wreck_id"], Value::String("wreck_1".to_string()));
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(3.into()));
}

#[test]
fn transfer_item_from_space_caps_to_free_cargo() {
    let mut state = station_market_state();
    state.cargo_used = 96;
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "wreck_1".to_string(),
            cargo: vec![spacemolt_lib_rs::data::WreckCargoItem {
                item_id: "copper_ore".to_string(),
                quantity: 9,
                name: None,
                size: None,
            }],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("copper_ore".to_string()),
            ActionArg::Integer(9),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/loot");
    let payload = payload.expect("payload");
    assert_eq!(payload["wreck_id"], Value::String("wreck_1".to_string()));
    assert_eq!(payload["item_id"], Value::String("copper_ore".to_string()));
    assert_eq!(payload["quantity"], Value::Number(4.into()));
}

#[test]
fn transfer_all_from_space_sweeps_visible_cargo_stacks() {
    let mut state = station_market_state();
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "container_1".to_string(),
            cargo: vec![
                spacemolt_lib_rs::data::WreckCargoItem {
                    item_id: "iron".to_string(),
                    quantity: 8,
                    name: None,
                    size: None,
                },
                spacemolt_lib_rs::data::WreckCargoItem {
                    item_id: "copper".to_string(),
                    quantity: 4,
                    name: None,
                    size: None,
                },
            ],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let (action, payload) = expect_api_call(p.next(&state, None).expect("first plan"));
    assert_eq!(action, "spacemolt_storage/loot");
    let payload = payload.expect("payload");
    assert_eq!(
        payload["wreck_id"],
        Value::String("container_1".to_string())
    );
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(8.into()));

    let response = serde_json::json!({ "result": { "message": "Looted iron." } });
    let (action, payload) = expect_api_call(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("second plan"),
    );
    assert_eq!(action, "spacemolt_storage/loot");
    let payload = payload.expect("payload");
    assert_eq!(
        payload["wreck_id"],
        Value::String("container_1".to_string())
    );
    assert_eq!(payload["item_id"], Value::String("copper".to_string()));
    assert_eq!(payload["quantity"], Value::Number(4.into()));
}

#[test]
fn transfer_all_from_space_caps_each_loot_call_to_free_cargo() {
    let mut state = station_market_state();
    state.cargo_used = 94;
    let mut catalog = state.catalog.as_ref().clone();
    catalog.items.insert(
        "iron".to_string(),
        serde_json::from_value(serde_json::json!({
            "base_value": 1,
            "category": "ore",
            "description": "Iron",
            "id": "iron",
            "name": "Iron",
            "size": 2,
            "stackable": true,
            "tradeable": true
        }))
        .expect("valid catalog item"),
    );
    state.catalog = Arc::new(catalog);
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "container_1".to_string(),
            cargo: vec![
                spacemolt_lib_rs::data::WreckCargoItem {
                    item_id: "iron".to_string(),
                    quantity: 4,
                    name: None,
                    size: None,
                },
                spacemolt_lib_rs::data::WreckCargoItem {
                    item_id: "copper".to_string(),
                    quantity: 4,
                    name: None,
                    size: None,
                },
            ],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let (action, payload) = expect_api_call(p.next(&state, None).expect("first plan"));
    assert_eq!(action, "spacemolt_storage/loot");
    let payload = payload.expect("payload");
    assert_eq!(
        payload,
        serde_json::json!({
            "wreck_id": "container_1",
            "item_id": "iron",
            "quantity": 3
        })
    );

    let response = serde_json::json!({ "result": { "message": "Looted iron." } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("second plan"),
    );
    assert!(result.completed);
}

#[test]
fn transfer_from_space_waits_and_completes_when_no_visible_loot() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let (message, resume_after) = expect_complete_after_wait(p.next(&state, None).expect("plan"));
    assert_eq!(message, "No visible space loot; waiting...");
    assert_eq!(resume_after, TICK_PAUSE);
}

#[test]
fn transfer_from_space_completes_when_cargo_is_already_full() {
    let mut state = station_market_state();
    state.cargo_used = state.cargo_capacity;
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "container_1".to_string(),
            cargo: vec![spacemolt_lib_rs::data::WreckCargoItem {
                item_id: "iron".to_string(),
                quantity: 8,
                name: None,
                size: None,
            }],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert_eq!(result.result_message.as_deref(), Some("Cargo full."));
}

#[test]
fn transfer_from_space_waits_when_visible_loot_is_stale() {
    let mut state = station_market_state();
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "container_1".to_string(),
            cargo: vec![spacemolt_lib_rs::data::WreckCargoItem {
                item_id: "iron".to_string(),
                quantity: 8,
                name: None,
                size: None,
            }],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let (action, _) = expect_api_call(p.next(&state, None).expect("first plan"));
    assert_eq!(action, "spacemolt_storage/loot");

    let rejection = serde_json::json!({
        "error": "not_found: Item not in wreck"
    });
    let (message, resume_after) = expect_wait_tick(
        p.next(&state, Some(ApiOutcome::Success(rejection)))
            .expect("second plan"),
    );
    assert_eq!(
        message,
        "Space loot target container_1 is not available; waiting..."
    );
    assert_eq!(resume_after, TICK_PAUSE);
}

#[test]
fn transfer_from_space_waits_when_api_reports_item_not_in_wreck() {
    let mut state = station_market_state();
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "container_1".to_string(),
            cargo: vec![spacemolt_lib_rs::data::WreckCargoItem {
                item_id: "iron".to_string(),
                quantity: 8,
                name: None,
                size: None,
            }],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let (action, _) = expect_api_call(p.next(&state, None).expect("first plan"));
    assert_eq!(action, "spacemolt_storage/loot");

    let (message, resume_after) = expect_wait_tick(
        p.next(
            &state,
            Some(api_failure(
                "api failure (404): not_found: Item not in wreck",
            )),
        )
        .expect("second plan"),
    );
    assert_eq!(
        message,
        "Space loot target container_1 is not available; waiting..."
    );
    assert_eq!(resume_after, TICK_PAUSE);
}

#[test]
fn transfer_from_space_succeeds_when_cargo_is_full() {
    let mut state = station_market_state();
    state.salvage = Arc::new(crate::engine::SalvageData {
        visible_lootables: vec![crate::engine::SpaceLootInfo {
            id: "container_1".to_string(),
            cargo: vec![spacemolt_lib_rs::data::WreckCargoItem {
                item_id: "iron".to_string(),
                quantity: 8,
                name: None,
                size: None,
            }],
            ..crate::engine::SpaceLootInfo::default()
        }],
        ..crate::engine::SalvageData::default()
    });
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("space".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let _ = expect_api_call(p.next(&state, None).expect("first plan"));
    let result = expect_complete(
            p.next(
                &state,
                Some(api_failure(
                    "api failure (400): {\"error\":{\"code\":\"no_space\",\"message\":\"no_space: Not enough cargo space for 42 x titanium_ore.\"}}",
                )),
            )
            .expect("second plan"),
        );
    assert_eq!(result.result_message.as_deref(), Some("Cargo full."));
}

#[test]
fn transfer_faction_to_storage_bypasses_cargo_halving() {
    let mut state = station_market_state();
    state.faction_storage = Arc::new(HashMap::from([("iron".to_string(), 40i64)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Any("all".to_string()),
            ActionArg::Any("faction".to_string()),
            ActionArg::Any("storage".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["source"], Value::String("faction".to_string()));
    assert_eq!(payload["target"], Value::String("self".to_string()));

    let rejection = serde_json::json!({ "error": { "code": "no_cargo_space" } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(rejection)))
            .expect("plan"),
    );
    assert!(result.completed);
}

#[test]
fn transfer_from_faction_fails_when_explicit_quantity_is_short() {
    let mut state = station_market_state();
    state.faction_storage = Arc::new(HashMap::from([("iron".to_string(), 3i64)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(5),
            ActionArg::Any("faction".to_string()),
            ActionArg::Any("storage".to_string()),
        ],
    );
    let err = p
        .next(&state, None)
        .expect_err("underfilled faction transfer should fail");
    assert_eq!(
        err.to_string(),
        "Cannot transfer 5 iron from faction; only 3 available."
    );
}

#[test]
fn transfer_named_item_from_storage_fails_when_cargo_is_full() {
    let mut state = station_market_state();
    state.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("iron".to_string(), 40i64)]),
    )]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Any("all".to_string()),
            ActionArg::Any("storage".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let _ = expect_api_call(p.next(&state, None).expect("plan"));
    let err = p
            .next(
                &state,
                Some(api_failure(
                    "api failure (400): {\"error\":{\"code\":\"no_space\",\"message\":\"no_space: Not enough cargo space for 42 x titanium_ore.\"}}",
                )),
            )
            .expect_err("named item cargo-full transfer should fail");
    assert!(err.to_string().contains("no_space"));
}

#[test]
fn transfer_all_from_storage_succeeds_when_cargo_is_full() {
    let mut state = station_market_state();
    state.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("iron".to_string(), 40i64)]),
    )]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("storage".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );

    let _ = expect_api_call(p.next(&state, None).expect("plan"));
    let result = expect_complete(
            p.next(
                &state,
                Some(api_failure(
                    "api failure (400): {\"error\":{\"code\":\"no_space\",\"message\":\"no_space: Not enough cargo space for 42 x titanium_ore.\"}}",
                )),
            )
            .expect("all-item cargo-full transfer should succeed"),
        );
    assert_eq!(result.result_message.as_deref(), Some("Cargo full."));
}

#[test]
fn transfer_from_storage_fails_when_explicit_quantity_is_short() {
    let mut state = station_market_state();
    state.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("iron".to_string(), 3i64)]),
    )]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(5),
            ActionArg::Any("storage".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let err = p
        .next(&state, None)
        .expect_err("underfilled storage transfer should fail");
    assert_eq!(
        err.to_string(),
        "Cannot transfer 5 iron from storage; only 3 available."
    );
}

#[test]
fn transfer_all_storage_to_faction_batches_with_unified_storage() {
    let mut state = station_market_state();
    state.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([
            ("iron".to_string(), 40i64),
            ("copper".to_string(), 7i64),
            ("empty".to_string(), 0i64),
        ]),
    )]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("storage".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    assert_eq!(
        payload.expect("payload"),
        serde_json::json!({
            "source": "storage",
            "target": "faction",
            "items": [
                { "item_id": "copper", "quantity": 7 },
                { "item_id": "iron", "quantity": 40 }
            ]
        })
    );
}

#[test]
fn transfer_faction_to_cargo_uses_item_size_without_retry() {
    let mut state = station_market_state();
    state.cargo_used = 96;
    state.faction_storage = Arc::new(HashMap::from([("iron".to_string(), 40i64)]));
    let mut catalog = state.catalog.as_ref().clone();
    catalog.items.insert(
        "iron".to_string(),
        serde_json::from_value(serde_json::json!({
            "base_value": 1,
            "category": "ore",
            "description": "Iron",
            "id": "iron",
            "name": "Iron",
            "size": 2,
            "stackable": true,
            "tradeable": true
        }))
        .expect("valid catalog item"),
    );
    state.catalog = Arc::new(catalog);
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Any("all".to_string()),
            ActionArg::Any("faction".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/withdraw");
    let payload = payload.expect("payload");
    // A withdrawal pulls into cargo; `target` names the store to pull from
    // and there is no `source`/`cargo` field.
    assert_eq!(payload["target"], Value::String("faction".to_string()));
    assert!(payload.get("source").is_none());
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(2.into()));
    assert!(payload.get("items").is_none());

    let rejection = serde_json::json!({ "error": { "code": "no_cargo_space" } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(rejection)))
            .expect("plan"),
    );
    assert!(result.completed);
}

#[test]
fn transfer_deposits_all_cargo_in_one_call() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([
        ("iron".to_string(), 5),
        ("water".to_string(), 2),
    ]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("all".to_string()),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("storage".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["source"], Value::String("cargo".to_string()));
    assert_eq!(payload["target"], Value::String("self".to_string()));
    let items = payload["items"].as_array().expect("items array").len();
    assert_eq!(items, 2);

    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
            .expect("plan"),
    );
    assert_eq!(
        result.result_message.as_deref(),
        Some("Transferred all cargo stacks (2 item types).")
    );
}

#[test]
fn transfer_reports_storage_errors() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 5)]));
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("item".to_string()),
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Any("all".to_string()),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("storage".to_string()),
        ],
    );
    let _ = expect_api_call(p.next(&state, None).expect("plan"));
    let response =
        serde_json::json!({ "error": "denied", "result": { "message": "Storage full." } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(response)))
            .expect("plan"),
    );
    assert_eq!(
        result.result_message.as_deref(),
        Some("Storage transfer failed: Storage full.")
    );
}

#[test]
fn transfer_credits_to_faction_deposits_from_cargo() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("credits".to_string()),
            ActionArg::Integer(100),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["source"], Value::String("cargo".to_string()));
    assert_eq!(payload["target"], Value::String("faction".to_string()));
    assert_eq!(payload["item_id"], Value::String("credits".to_string()));
    assert_eq!(payload["quantity"], Value::Number(100.into()));
}

#[test]
fn transfer_credits_to_player_gifts_from_cargo() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("credits".to_string()),
            ActionArg::Integer(100),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Rowan Pike".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["source"], Value::String("cargo".to_string()));
    assert_eq!(payload["target"], Value::String("Rowan Pike".to_string()));
    assert_eq!(payload["item_id"], Value::String("credits".to_string()));
    assert_eq!(payload["quantity"], Value::Number(100.into()));
}

#[test]
fn transfer_credits_halts_without_dockable_base() {
    let state = PlanningState {
        system: Some("empty".to_string()),
        ..PlanningState::default()
    };
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("credits".to_string()),
            ActionArg::Integer(100),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Rowan Pike".to_string()),
        ],
    );
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert!(result.completed);
    assert!(result.halt_script);
    assert_eq!(
        result.result_message.as_deref(),
        Some("No dockable base available in the current system.")
    );
}

#[test]
fn transfer_credits_autodocks_when_undocked() {
    let mut state = docked_state("sol", "station_1");
    state.docked = false;
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("credits".to_string()),
            ActionArg::Integer(100),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    // First tick docks rather than issuing the transfer.
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/dock");
}

#[test]
fn transfer_credits_from_faction_withdraws_to_cargo() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("credits".to_string()),
            ActionArg::Integer(50),
            ActionArg::Any("faction".to_string()),
            ActionArg::Any("cargo".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/withdraw");
    let payload = payload.expect("payload");
    assert_eq!(payload["target"], Value::String("faction".to_string()));
    assert!(payload.get("source").is_none());
    assert_eq!(payload["item_id"], Value::String("credits".to_string()));
    assert_eq!(payload["quantity"], Value::Number(50.into()));
}

#[test]
fn transfer_ship_to_player_uses_storage_gift_deposit() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("ship".to_string()),
            ActionArg::ShipId("ship_abc123".to_string()),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Rowan Pike".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["target"], Value::String("Rowan Pike".to_string()));
    assert_eq!(payload["item_id"], Value::String("ship_abc123".to_string()));
    assert!(payload.get("quantity").is_none());
    assert!(payload.get("source").is_none());
    assert!(payload.get("items").is_none());
}

#[test]
fn transfer_ship_from_storage_uses_storage_gift_deposit() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("ship".to_string()),
            ActionArg::ShipId("ship_abc123".to_string()),
            ActionArg::Any("storage".to_string()),
            ActionArg::Any("player:Rowan Pike".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("ship_abc123".to_string()));
    assert!(payload.get("quantity").is_none());
    assert_eq!(payload["source"], Value::String("storage".to_string()));
}

#[test]
fn transfer_ship_to_faction_uses_faction_garage_deposit() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("ship".to_string()),
            ActionArg::ShipId("ship_abc123".to_string()),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["target"], Value::String("faction".to_string()));
    assert!(payload.get("source").is_none());
    assert_eq!(payload["item_id"], Value::String("ship_abc123".to_string()));
    assert!(payload.get("quantity").is_none());
}

#[test]
fn transfer_ship_from_storage_to_faction_uses_faction_garage_deposit() {
    let state = station_market_state();
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("ship".to_string()),
            ActionArg::ShipId("ship_abc123".to_string()),
            ActionArg::Any("storage".to_string()),
            ActionArg::Any("faction".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
    let payload = payload.expect("payload");
    assert_eq!(payload["target"], Value::String("faction".to_string()));
    assert!(payload.get("source").is_none());
    assert_eq!(payload["item_id"], Value::String("ship_abc123".to_string()));
    assert!(payload.get("quantity").is_none());
}

#[test]
fn transfer_ship_to_player_does_not_autodock_when_undocked() {
    let mut state = docked_state("sol", "station_1");
    state.docked = false;
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("ship".to_string()),
            ActionArg::ShipId("ship_abc123".to_string()),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Rowan Pike".to_string()),
        ],
    );
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
}

#[test]
fn transfer_ship_to_player_can_run_in_transit() {
    let mut state = docked_state("sol", "station_1");
    state.in_transit = true;
    let mut p = planner(
        "transfer",
        vec![
            ActionArg::Any("ship".to_string()),
            ActionArg::ShipId("ship_abc123".to_string()),
            ActionArg::Any("cargo".to_string()),
            ActionArg::Any("player:Rowan Pike".to_string()),
        ],
    );
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_storage/deposit");
}

#[test]
fn buy_cancels_crossing_orders_then_retries() {
    let mut state = station_market_state();
    let market = crate::engine::MarketData {
        sell_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 10,
                quantity: 50,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![ActionArg::ItemId("iron".to_string()), ActionArg::Integer(5)],
    );
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_buy_order");

    let crossing = serde_json::json!({
        "error": { "code": "crossing_order", "order_id": "ord_1" }
    });
    let (action, payload) = expect_api_call(
        p.next(&state, Some(ApiOutcome::Success(crossing)))
            .expect("plan"),
    );
    assert_eq!(action, "spacemolt_market/cancel_order");
    assert_eq!(
        payload.expect("payload")["order_id"],
        Value::String("ord_1".to_string())
    );

    let op = p
        .next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
        .expect("plan");
    assert!(matches!(op, RuntimeOperation::RefreshState));

    let (action, _) = expect_api_call(
        p.next(&state, Some(ApiOutcome::Success(Value::Null)))
            .expect("plan"),
    );
    assert_eq!(action, "spacemolt_market/create_buy_order");

    let done = serde_json::json!({ "result": { "message": "Order placed." } });
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(done)))
            .expect("plan"),
    );
    assert!(result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Order placed."));
}

#[test]
fn buy_crossing_failure_withdraws_returned_storage_before_rebuying_remainder() {
    let mut state = station_market_state();
    state.own_sell_orders = Arc::new(vec![open_order("ord_1", "iron", "sell", 10, 3)]);
    let market = crate::engine::MarketData {
        sell_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 10,
                quantity: 50,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![ActionArg::ItemId("iron".to_string()), ActionArg::Integer(5)],
    );
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_buy_order");

    let mut crossing = spacemolt_lib_rs::SpacemoltError::new(
        "crossing_order",
        "crossing_order: cancel ord_1 first",
    );
    crossing.details = Some(serde_json::json!({ "order_ids": ["ord_1"] }));
    let failure = ApiOutcome::Failure(OperationFailure::Client(
        spacemolt_lib_rs::ClientError::Server(crossing),
    ));
    let (action, payload) = expect_api_call(p.next(&state, Some(failure)).expect("plan"));
    assert_eq!(action, "spacemolt_market/cancel_order");
    assert_eq!(
        payload.expect("payload")["order_id"],
        Value::String("ord_1".to_string())
    );

    let op = p
        .next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
        .expect("plan");
    assert!(matches!(op, RuntimeOperation::RefreshState));

    let mut fresh = state.clone();
    fresh.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("iron".to_string(), 3)]),
    )]));
    let (action, payload) = expect_api_call(
        p.next(&fresh, Some(ApiOutcome::Success(Value::Null)))
            .expect("plan"),
    );
    assert_eq!(action, "spacemolt_storage/withdraw");
    let payload = payload.expect("payload");
    assert_eq!(payload["target"], Value::String("self".to_string()));
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(3.into()));

    let (action, payload) = expect_api_call(
        p.next(
            &fresh,
            Some(ApiOutcome::Success(serde_json::json!({
                "result": { "message": "Withdrew 3 iron." }
            }))),
        )
        .expect("plan"),
    );
    assert_eq!(action, "spacemolt_market/create_buy_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(2.into()));
    assert_eq!(payload["price_each"], Value::Number(10.into()));
}

#[test]
fn buy_crossing_with_returned_storage_can_satisfy_entire_cargo_buy() {
    let mut state = station_market_state();
    let market = crate::engine::MarketData {
        sell_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 10,
                quantity: 50,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![ActionArg::ItemId("iron".to_string()), ActionArg::Integer(5)],
    );
    let _ = expect_api_call(p.next(&state, None).expect("plan"));
    let crossing = serde_json::json!({
        "error": { "code": "crossing_order", "order_id": "ord_1" }
    });
    let _ = expect_api_call(
        p.next(&state, Some(ApiOutcome::Success(crossing)))
            .expect("plan"),
    );
    let op = p
        .next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
        .expect("plan");
    assert!(matches!(op, RuntimeOperation::RefreshState));

    let mut fresh = state.clone();
    fresh.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("iron".to_string(), 5)]),
    )]));
    let (action, payload) = expect_api_call(
        p.next(&fresh, Some(ApiOutcome::Success(Value::Null)))
            .expect("plan"),
    );
    assert_eq!(action, "spacemolt_storage/withdraw");
    let payload = payload.expect("payload");
    assert_eq!(payload["target"], Value::String("self".to_string()));
    assert_eq!(payload["quantity"], Value::Number(5.into()));

    let result = expect_complete(
        p.next(
            &fresh,
            Some(ApiOutcome::Success(serde_json::json!({
                "result": { "message": "Withdrew 5 iron." }
            }))),
        )
        .expect("plan"),
    );
    assert!(result.completed);
    assert_eq!(result.result_message.as_deref(), Some("Withdrew 5 iron."));
}

#[test]
fn sell_sweeps_priced_cargo_stacks() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([
        ("iron".to_string(), 3),
        ("junk".to_string(), 9),
    ]));
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 10,
                quantity: 3,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner("sell", vec![]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["price_each"], Value::Number(10.into()));

    // junk has no market data, so the sweep ends after iron.
    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
            .expect("plan"),
    );
    assert!(result.completed);
    assert_eq!(
        result.result_message.as_deref(),
        Some("Finished selling cargo/storage (1 item types).")
    );
}

#[test]
fn buy_order_quantity_is_not_limited_by_cargo_space() {
    let mut state = station_market_state();
    state.cargo_used = state.cargo_capacity;
    let market = crate::engine::MarketData {
        sell_orders: HashMap::from([(
            "carbon_ore".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 1,
                quantity: 100_000,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![
            ActionArg::ItemId("carbon_ore".to_string()),
            ActionArg::Integer(100_000),
            ActionArg::Integer(1),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_buy_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("carbon_ore".to_string()));
    assert_eq!(payload["quantity"], Value::Number(100_000.into()));
    assert_eq!(payload["price_each"], Value::Number(1.into()));
}

#[test]
fn buy_with_max_price_caps_price_and_trims_quantity() {
    let mut state = station_market_state();
    // Cheap asks are nearly gone; the bulk of the book sits at 44.
    let market = crate::engine::MarketData {
        sell_orders: HashMap::from([(
            "iron".to_string(),
            vec![
                crate::engine::MarketOrder {
                    price_each: 7,
                    quantity: 8,
                    source: None,
                    my_quantity: None,
                },
                crate::engine::MarketOrder {
                    price_each: 44,
                    quantity: 50,
                    source: None,
                    my_quantity: None,
                },
            ],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(58),
            ActionArg::Integer(10),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_buy_order");
    let payload = payload.expect("payload");
    // Only the 8 units at or below 10 qualify, and the limit price is the cap.
    assert_eq!(payload["quantity"], Value::Number(8.into()));
    assert_eq!(payload["price_each"], Value::Number(10.into()));
}

#[test]
fn buy_prefers_lowest_ask_over_best_bid() {
    let mut state = station_market_state();
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "targeting_computer".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 25,
                quantity: 1200,
                source: None,
                my_quantity: None,
            }],
        )]),
        sell_orders: HashMap::from([(
            "targeting_computer".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 37,
                quantity: 9,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![
            ActionArg::ItemId("targeting_computer".to_string()),
            ActionArg::Integer(9),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_buy_order");
    let payload = payload.expect("payload");
    assert_eq!(
        payload["item_id"],
        Value::String("targeting_computer".to_string())
    );
    assert_eq!(payload["quantity"], Value::Number(9.into()));
    assert_eq!(payload["price_each"], Value::Number(37.into()));
}

#[test]
fn buy_with_max_price_below_all_asks_buys_nothing() {
    let mut state = station_market_state();
    let market = crate::engine::MarketData {
        sell_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 44,
                quantity: 50,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(58),
            ActionArg::Integer(10),
        ],
    );
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert!(result.completed);
    assert_eq!(
        result.result_message.as_deref(),
        Some("No iron sell orders at or below 10/unit.")
    );
}

#[test]
fn buy_order_mode_places_full_quantity_below_all_asks() {
    let mut state = station_market_state();
    let market = crate::engine::MarketData {
        sell_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 44,
                quantity: 50,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "buy",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(58),
            ActionArg::Integer(10),
            ActionArg::Any("order".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_buy_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(58.into()));
    assert_eq!(payload["price_each"], Value::Number(10.into()));
}

#[test]
fn buy_order_mode_does_not_require_market_data() {
    let state = station_market_state();

    let mut p = planner(
        "buy",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(58),
            ActionArg::Integer(10),
            ActionArg::Any("order".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_buy_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(58.into()));
    assert_eq!(payload["price_each"], Value::Number(10.into()));
}

#[test]
fn sell_with_min_price_floors_price_and_trims_quantity() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 10)]));
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![
                crate::engine::MarketOrder {
                    price_each: 50,
                    quantity: 3,
                    source: None,
                    my_quantity: None,
                },
                crate::engine::MarketOrder {
                    price_each: 20,
                    quantity: 5,
                    source: None,
                    my_quantity: None,
                },
            ],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "sell",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(10),
            ActionArg::Integer(40),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    // Only the 3 units bid at or above 40 qualify; price floors at 40.
    assert_eq!(payload["quantity"], Value::Number(3.into()));
    assert_eq!(payload["price_each"], Value::Number(40.into()));
}

#[test]
fn sell_order_mode_places_full_quantity_above_available_bids() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 10)]));
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 50,
                quantity: 3,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "sell",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(10),
            ActionArg::Integer(40),
            ActionArg::Any("order".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(10.into()));
    assert_eq!(payload["price_each"], Value::Number(40.into()));
}

#[test]
fn sell_order_mode_does_not_require_market_data() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 10)]));

    let mut p = planner(
        "sell",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(10),
            ActionArg::Integer(40),
            ActionArg::Any("resting".to_string()),
        ],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(10.into()));
    assert_eq!(payload["price_each"], Value::Number(40.into()));
}

#[test]
fn sell_with_quantity_cap_limits_single_item_sale() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 10)]));
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 50,
                quantity: 10,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "sell",
        vec![ActionArg::ItemId("iron".to_string()), ActionArg::Integer(4)],
    );
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(4.into()));
    assert_eq!(payload["price_each"], Value::Number(50.into()));
}

#[test]
fn sell_retries_at_authoritative_available_quantity() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("shield_emitter".to_string(), 63)]));
    state.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("shield_emitter".to_string(), 1)]),
    )]));
    state.market = Arc::new(crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "shield_emitter".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 7_238,
                quantity: 100,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    });

    let mut p = planner(
        "sell",
        vec![
            ActionArg::ItemId("shield_emitter".to_string()),
            ActionArg::Integer(68),
            ActionArg::Integer(7_238),
        ],
    );
    let (_, first_payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(first_payload.expect("payload")["quantity"], 64);

    let failure = ApiOutcome::Failure(OperationFailure::Client(
        spacemolt_lib_rs::ClientError::Server(spacemolt_lib_rs::SpacemoltError::new(
            "insufficient_items",
            "You have 63 x Shield Emitter available (63 cargo, 0 storage). Need 64.",
        )),
    ));
    let (action, retry_payload) = expect_api_call(p.next(&state, Some(failure)).expect("retry"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    assert_eq!(retry_payload.expect("payload")["quantity"], 63);
}

#[test]
fn sell_with_arg_includes_current_storage_quantity() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 3)]));
    state.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("iron".to_string(), 7)]),
    )]));
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 50,
                quantity: 10,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner("sell", vec![ActionArg::ItemId("iron".to_string())]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(10.into()));
    assert_eq!(payload["price_each"], Value::Number(50.into()));
}

#[test]
fn sell_without_arg_sweeps_storage_only_items() {
    let mut state = station_market_state();
    state.storage = Arc::new(HashMap::from([(
        "poi_home".to_string(),
        HashMap::from([("iron".to_string(), 7), ("junk".to_string(), 4)]),
    )]));
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 50,
                quantity: 7,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner("sell", vec![]);
    let (action, payload) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/create_sell_order");
    let payload = payload.expect("payload");
    assert_eq!(payload["item_id"], Value::String("iron".to_string()));
    assert_eq!(payload["quantity"], Value::Number(7.into()));
    assert_eq!(payload["price_each"], Value::Number(50.into()));
}

#[test]
fn sell_with_min_price_no_qualifying_bid_sells_nothing() {
    let mut state = station_market_state();
    state.cargo = Arc::new(HashMap::from([("iron".to_string(), 10)]));
    let market = crate::engine::MarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![crate::engine::MarketOrder {
                price_each: 20,
                quantity: 5,
                source: None,
                my_quantity: None,
            }],
        )]),
        ..Default::default()
    };
    state.market = Arc::new(market);

    let mut p = planner(
        "sell",
        vec![
            ActionArg::ItemId("iron".to_string()),
            ActionArg::Integer(10),
            ActionArg::Integer(40),
        ],
    );
    let result = expect_complete(p.next(&state, None).expect("plan"));
    assert!(result.completed);
    assert_eq!(
        result.result_message.as_deref(),
        Some("No cargo with buy orders at or above 40/unit.")
    );
}

#[test]
fn cancel_buy_sweeps_open_orders_and_reports_errors() {
    let mut state = station_market_state();
    state.own_buy_orders = Arc::new(vec![
        open_order("ord_1", "iron", "buy", 1, 1),
        open_order("ord_2", "iron", "buy", 1, 1),
    ]);
    let mut p = planner("cancel_buy", vec![ActionArg::ItemId("iron".to_string())]);
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt_market/cancel_order");

    let failure = serde_json::json!({ "error": "denied", "result": { "message": "Too late." } });
    let (action, _) = expect_api_call(
        p.next(&state, Some(ApiOutcome::Success(failure)))
            .expect("plan"),
    );
    assert_eq!(action, "spacemolt_market/cancel_order");

    let result = expect_complete(
        p.next(&state, Some(ApiOutcome::Success(serde_json::json!({}))))
            .expect("plan"),
    );
    assert_eq!(
        result.result_message.as_deref(),
        Some("Canceled 1/2 order(s) for iron. Errors: Too late.")
    );
}

#[test]
fn market_command_undocked_positions_first() {
    let mut state = station_market_state();
    state.docked = false;
    let mut p = planner("buy", vec![ActionArg::ItemId("iron".to_string())]);
    let (action, _) = expect_api_call(p.next(&state, None).expect("plan"));
    assert_eq!(action, "spacemolt/dock");
}

#[test]
fn dock_required_passthrough_commands_dock_before_dispatch() {
    let mut state = docked_state("sol", "station_1");
    state.docked = false;

    for action in [
        "accept_mission",
        "decline_mission",
        "repair_module",
        "recycle",
        "load_passenger",
        "unload_passenger",
        "craft",
        "facility_build",
        "faction_facility_build",
        "facility_upgrade",
        "faction_facility_upgrade",
        "buy_ship",
        "buy_listed_ship",
        "switch_ship",
        "commission_ship",
        "list_ship_for_sale",
        "cancel_order",
        "modify_order",
        "facility_dismantle",
        "faction_facility_dismantle",
        "facility_set_access",
        "facility_set_output_price",
        "facility_set_name",
    ] {
        let mut planner = planner(action, vec![]);
        let (lowered, payload) = expect_api_call(planner.next(&state, None).expect(action));
        assert_eq!(lowered, "spacemolt/dock", "{action}");
        assert_eq!(payload, None, "{action}");
    }
}

#[test]
fn sell_targets_without_arg_include_all_positive_cargo_and_storage() {
    let state = PlanningState {
        current_poi: Some("station".to_string()),
        cargo: std::sync::Arc::new(std::collections::HashMap::from([
            ("iron".to_string(), 3),
            ("water".to_string(), 0),
            ("fuel".to_string(), 2),
        ])),
        storage: std::sync::Arc::new(std::collections::HashMap::from([(
            "station".to_string(),
            std::collections::HashMap::from([("iron".to_string(), 4), ("copper".to_string(), 6)]),
        )])),
        market: std::sync::Arc::new(crate::engine::MarketData {
            buy_orders: std::collections::HashMap::from([
                (
                    "iron".to_string(),
                    vec![crate::engine::MarketOrder {
                        price_each: 10,
                        quantity: 3,
                        source: None,
                        my_quantity: None,
                    }],
                ),
                (
                    "fuel".to_string(),
                    vec![crate::engine::MarketOrder {
                        price_each: 5,
                        quantity: 2,
                        source: None,
                        my_quantity: None,
                    }],
                ),
                (
                    "copper".to_string(),
                    vec![crate::engine::MarketOrder {
                        price_each: 8,
                        quantity: 6,
                        source: None,
                        my_quantity: None,
                    }],
                ),
            ]),
            ..Default::default()
        }),
        ..PlanningState::default()
    };
    let targets = sell_targets(&state, None);
    assert_eq!(targets.len(), 3);
    assert!(targets.contains(&("iron".to_string(), 7)));
    assert!(targets.contains(&("fuel".to_string(), 2)));
    assert!(targets.contains(&("copper".to_string(), 6)));
}

#[test]
fn sell_targets_with_arg_use_single_stack_quantity() {
    let state = PlanningState {
        cargo: std::sync::Arc::new(std::collections::HashMap::from([("iron".to_string(), 7)])),
        market: std::sync::Arc::new(crate::engine::MarketData {
            buy_orders: std::collections::HashMap::from([(
                "iron".to_string(),
                vec![crate::engine::MarketOrder {
                    price_each: 10,
                    quantity: 7,
                    source: None,
                    my_quantity: None,
                }],
            )]),
            ..Default::default()
        }),
        ..PlanningState::default()
    };
    let targets = sell_targets(&state, Some("iron"));
    assert_eq!(targets, vec![("iron".to_string(), 7)]);
}
