use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Barrier, Mutex as StdMutex};
use std::time::{Duration, Instant};

use crate::state_mapping::map_commander_session_state;
use crate::RuntimeVirtualOrderUseDto;
use chrono::Utc;
use prayer_runtime::engine::{
    AgentSightingData, GalaxyData, MarketData, MarketOrder, MissionData, PoiInfoData,
    RuntimeEngine, SalvageData, SpaceLootInfo, StationMarketData,
};
use prayer_runtime::operation_failure::OperationFailure;
use prayer_runtime::snapshot::{AgentsObservation, StateObservation, WRECKS_REFRESH_TTL};
use prayer_runtime::{BotState, CatalogData};
use serde_json::Value;
use spacemolt_lib_rs::errors::ConnectionClosedError;
use spacemolt_lib_rs::protocol::{InboundFrame, RawFrame};
use spacemolt_lib_rs::transport::socket::{
    BoxedConnect, SocketCallbacks, SocketFactory, SocketHandle,
};
use uuid::Uuid;

fn test_catalog_item(
    id: &str,
    name: &str,
    size: i64,
) -> spacemolt_lib_rs::schema::CatalogDumpItemsItem {
    spacemolt_lib_rs::schema::CatalogDumpItemsItem::Item(spacemolt_lib_rs::schema::Item {
        base_value: 0,
        category: "test".to_string(),
        description: name.to_string(),
        effect: None,
        extracted_by: None,
        food_type: None,
        hazardous: None,
        hidden: None,
        id: id.to_string(),
        name: name.to_string(),
        quest_item: None,
        rarity: None,
        region_lock: Vec::new(),
        size,
        stackable: true,
        tradeable: true,
    })
}

fn test_facility(id: &str, name: &str) -> spacemolt_lib_rs::schema::FacilityDefinition {
    serde_json::from_value(serde_json::json!({
        "id": id, "name": name, "description": name, "category": "test",
        "level": 0, "always_on": false, "build_cost": 0, "build_time": 0,
        "labor_cost": 0
    }))
    .expect("valid facility definition")
}

fn test_observation(
    bot: BotState,
    world: prayer_runtime::snapshot::WorldObservation,
) -> StateObservation {
    StateObservation {
        status_system: bot.location.system_id.clone(),
        status_poi: bot.location.poi_id.clone(),
        bot: prayer_runtime::snapshot::BotObservation { state: bot },
        world,
        ..StateObservation::default()
    }
}

fn catalog_root_type_schema_from_value(value: &Value) -> Value {
    let root_keys = value
        .as_object()
        .map(|object| object.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    let mut root_counts = serde_json::Map::new();
    let mut schemas = serde_json::Map::new();
    for root in CANONICAL_CATALOG_ROOTS {
        let entries = value
            .get(*root)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        root_counts.insert(root.to_string(), Value::from(entries.len()));
        schemas.insert(
            root.to_string(),
            Value::Object(catalog_entry_schema(&entries)),
        );
    }

    serde_json::json!({
        "source_version": value.get("version").and_then(Value::as_str).unwrap_or(""),
        "root_counts": root_counts,
        "root_keys": root_keys,
        "schemas": schemas,
    })
}

fn catalog_entry_schema(entries: &[Value]) -> serde_json::Map<String, Value> {
    let mut fields: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in entries {
        let Some(object) = entry.as_object() else {
            continue;
        };
        for (key, value) in object {
            fields
                .entry(key.clone())
                .or_default()
                .insert(catalog_schema_type(value));
        }
    }
    fields
        .into_iter()
        .map(|(key, types)| {
            (
                key,
                Value::Array(types.into_iter().map(Value::String).collect()),
            )
        })
        .collect()
}

fn catalog_schema_type(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let inner = items
                .iter()
                .map(|item| match item {
                    Value::Array(_) => "array",
                    Value::Object(_) => "object",
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("|");
            format!("array<{inner}>")
        }
        Value::Object(_) => "object".to_string(),
        Value::Null => "null".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::String(_) => "string".to_string(),
    }
}

use super::{
    apply_live_state, apply_live_state_inner, apply_mobile_capital_location,
    canonical_catalog_from_cache, commission_related_api_action, craft_enqueue_response_text,
    crafting_queue_related_api_action, execution_state, execution_state_with_metadata,
    facility_snapshot_fresh, facility_types_from_catalog, market_related_api_action,
    merge_knowledge_state, merge_knowledge_state_if_changed,
    merge_knowledge_state_if_changed_with_metadata, mission_related_api_action,
    parse_runtime_sessions_value, player_station_storage_key, preserve_craft_enqueue_as_queue,
    replace_pending_knowledge_snapshot, should_refresh_owned_ships, storage_market_prices,
    switch_ship_already_active_error, switch_ship_related_api_action,
    unload_all_no_passengers_error, world_knowledge_persisted_eq, world_read_state,
    KnowledgePersistence, KnowledgePersistenceRequest, PersistedWorldKnowledgeV4,
    PersistenceTelemetry, PoiFacilitiesSnapshot, RuntimeService, RuntimeVirtualCraftOrderDto,
    RuntimeVirtualMarketOrderDto, SessionHandle, WorldState, CANONICAL_CATALOG_ROOTS,
    FACILITY_POI_SNAPSHOT_TTL_SECS, KNOWLEDGE_SCHEMA_VERSION, MOBILE_BASE_POI_ID,
    SESSION_SCHEMA_VERSION,
};

#[derive(Clone, Default)]
struct TestSocketFactory {
    sockets: Arc<StdMutex<Vec<TestSocket>>>,
}

impl TestSocketFactory {
    fn latest(&self) -> TestSocket {
        self.sockets
            .lock()
            .expect("sockets")
            .last()
            .expect("socket")
            .clone()
    }
}

impl SocketFactory for TestSocketFactory {
    fn connect(&self, url: String, callbacks: SocketCallbacks) -> BoxedConnect {
        let socket = TestSocket::new(url, callbacks);
        self.sockets.lock().expect("sockets").push(socket.clone());
        Box::pin(async move { Ok(Arc::new(socket) as Arc<dyn SocketHandle>) })
    }
}

#[derive(Clone)]
struct TestSocket {
    inner: Arc<TestSocketInner>,
}

struct TestSocketInner {
    _url: String,
    sent: StdMutex<Vec<InboundFrame>>,
    callbacks: SocketCallbacks,
    closed: StdMutex<bool>,
}

impl TestSocket {
    fn new(url: String, callbacks: SocketCallbacks) -> Self {
        Self {
            inner: Arc::new(TestSocketInner {
                _url: url,
                sent: StdMutex::new(Vec::new()),
                callbacks,
                closed: StdMutex::new(false),
            }),
        }
    }

    fn server_send(&self, frame: RawFrame) {
        self.inner.callbacks.frame(frame);
    }

    fn sent(&self) -> Vec<InboundFrame> {
        self.inner.sent.lock().expect("sent").clone()
    }

    async fn wait_for_action(&self, action: &str) -> InboundFrame {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(frame) = self.sent().into_iter().find(|frame| frame.action == action) {
                    return frame;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {action}"))
    }
}

impl SocketHandle for TestSocket {
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
        let mut closed = self.inner.closed.lock().expect("closed");
        *closed = true;
    }
}

fn test_welcome_payload() -> serde_json::Value {
    serde_json::json!({
        "version": "test",
        "release_date": "2026-07-09",
        "release_notes": [],
        "tick_rate": 1,
        "current_tick": 1,
        "server_time": 1,
        "game_info": "test",
        "website": "https://example.test",
        "help_text": "test",
        "terms": "test"
    })
}

async fn seeded_test_account(state: serde_json::Value) -> spacemolt_lib_rs::Account {
    seeded_test_account_with_id(None, state).await
}

async fn seeded_test_account_with_id(
    id: Option<String>,
    state: serde_json::Value,
) -> spacemolt_lib_rs::Account {
    let (account, _) = seeded_test_account_with_socket(id, state).await;
    account
}

async fn seeded_test_account_with_socket(
    id: Option<String>,
    state: serde_json::Value,
) -> (spacemolt_lib_rs::Account, TestSocket) {
    let factory = Arc::new(TestSocketFactory::default());
    let account = spacemolt_lib_rs::Account::with_socket_factory(
        spacemolt_lib_rs::AccountOptions {
            url: "ws://mock/ws/v2".to_string(),
            id,
            seed_state: true,
            query_timeout_ms: 250,
            mutation_timeout_ms: 250,
            fast_mutation_timeout_ms: 250,
            ..spacemolt_lib_rs::AccountOptions::default()
        },
        factory.clone(),
    );
    account.connect().await.expect("connect");
    let socket = factory.latest();
    socket.server_send(spacemolt_lib_rs::RawFrame {
        kind: "welcome".to_string(),
        request_id: None,
        payload: Some(test_welcome_payload()),
    });
    account.wait_for_welcome().await.expect("welcome");
    socket.server_send(spacemolt_lib_rs::RawFrame {
        kind: "logged_in".to_string(),
        request_id: None,
        payload: Some(state),
    });
    (account, socket)
}

#[tokio::test]
async fn new_session_starts_without_spacemolt_account() {
    let service = RuntimeService::new();
    let id = service.create_session();
    let session = service.get_session(id).await.expect("session");
    let session = session.lock().await;

    assert!(session.spacemolt_account.is_none());
}

#[tokio::test]
async fn install_connected_owned_accounts_creates_runtime_sessions() {
    let service = RuntimeService::new();
    let account = spacemolt_lib_rs::Account::new(spacemolt_lib_rs::AccountOptions {
        id: Some("Scout".to_string()),
        seed_state: false,
        ..spacemolt_lib_rs::AccountOptions::default()
    });

    service
        .install_connected_owned_spacemolt_accounts(
            vec![account],
            "https://game.spacemolt.com".to_string(),
        )
        .await
        .expect("install accounts");

    let sessions = service.list_sessions().await;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].label, "Scout");
    let session = service
        .get_session(Uuid::parse_str(&sessions[0].id).expect("id"))
        .await
        .expect("session");
    let session = session.lock().await;
    assert!(session.spacemolt_account.is_some());
    assert_eq!(session.spacemolt_account_selector.as_deref(), Some("Scout"));
    assert_eq!(
        session.spacemolt_base_url.as_deref(),
        Some("https://game.spacemolt.com")
    );
}

#[tokio::test]
async fn attached_account_is_roster_visible_before_refresh() {
    let service = RuntimeService::new();
    let account = spacemolt_lib_rs::Account::new(spacemolt_lib_rs::AccountOptions {
        id: Some("Immediate Scout".to_string()),
        seed_state: false,
        ..spacemolt_lib_rs::AccountOptions::default()
    });

    let (created, installed) = service
        .attach_connected_owned_spacemolt_accounts(
            vec![account],
            "https://game.spacemolt.com".to_string(),
        )
        .await
        .expect("attach account");
    let roster = service.commander_roster_snapshot().await;

    assert_eq!(created, 1);
    assert_eq!(installed.len(), 1);
    assert_eq!(roster["sessions"].as_array().expect("sessions").len(), 1);
    assert_eq!(roster["sessions"][0]["playerName"], "Immediate Scout");
    let session = service.get_session(installed[0]).await.expect("session");
    assert!(
        !session.lock().await.has_state,
        "refresh must remain queued work"
    );
}

#[tokio::test]
async fn install_connected_owned_accounts_attaches_to_existing_runtime_session() {
    let service = RuntimeService::new();
    let id = service
        .create_session_with_label(Some("Scout".to_string()))
        .expect("session");
    let account = seeded_test_account_with_id(
        Some("Scout".to_string()),
        serde_json::json!({
            "player": { "id": "player_1", "username": "Scout", "credits": 44 },
            "ship": { "fuel": 90, "max_fuel": 100 },
            "location": {
                "system_id": "sol",
                "poi_id": "earth_station",
                "docked_at": "earth_station"
            }
        }),
    )
    .await;

    let created = service
        .install_connected_owned_spacemolt_accounts(
            vec![account],
            "https://game.spacemolt.com".to_string(),
        )
        .await
        .expect("install accounts");

    assert_eq!(created, 0);
    let session = service.get_session(id).await.expect("session");
    let session = session.lock().await;
    assert!(session.spacemolt_account.is_some());
    assert_eq!(session.spacemolt_account_selector.as_deref(), Some("Scout"));
    assert_eq!(
        session.spacemolt_base_url.as_deref(),
        Some("https://game.spacemolt.com")
    );
    drop(session);

    let snapshot = service
        .engine_snapshot_response(id)
        .await
        .expect("snapshot");
    assert_eq!(snapshot.username.as_deref(), Some("Scout"));
    assert_eq!(snapshot.latest_system.as_deref(), Some("sol"));
    assert_eq!(snapshot.latest_poi.as_deref(), Some("earth_station"));
}

#[tokio::test]
async fn install_connected_owned_accounts_matches_existing_session_by_username() {
    let service = RuntimeService::new();
    let id = service
        .create_session_with_label(Some("Scout".to_string()))
        .expect("session");
    let account = seeded_test_account_with_id(
        Some("player_1".to_string()),
        serde_json::json!({
            "player": { "id": "player_1", "username": "Scout", "credits": 44 },
            "ship": { "fuel": 90, "max_fuel": 100 },
            "location": {
                "system_id": "sol",
                "poi_id": "earth_station",
                "docked_at": "earth_station"
            }
        }),
    )
    .await;

    let created = service
        .install_connected_owned_spacemolt_accounts(
            vec![account],
            "https://game.spacemolt.com".to_string(),
        )
        .await
        .expect("install accounts");

    assert_eq!(created, 0);
    assert_eq!(service.list_sessions().await.len(), 1);
    let session = service.get_session(id).await.expect("session");
    let session = session.lock().await;
    assert!(session.spacemolt_account.is_some());
    assert_eq!(
        session.spacemolt_account_selector.as_deref(),
        Some("player_1")
    );
    drop(session);

    let snapshot = service
        .engine_snapshot_response(id)
        .await
        .expect("snapshot");
    assert_eq!(snapshot.username.as_deref(), Some("Scout"));
    assert_eq!(snapshot.latest_system.as_deref(), Some("sol"));
    assert_eq!(snapshot.latest_poi.as_deref(), Some("earth_station"));
}

#[tokio::test]
async fn session_account_access_requires_connected_account() {
    let service = RuntimeService::new();
    let id = service.create_session();

    let err = service
        .spacemolt_account(id)
        .await
        .expect_err("missing account should fail");

    assert!(err
        .to_string()
        .contains("SpaceMolt account is not connected"));
}

#[tokio::test]
async fn refresh_state_uses_account_cache_when_account_is_connected() {
    let service = RuntimeService::new();
    let id = service.create_session();
    let account = seeded_test_account(serde_json::json!({
        "player": { "id": "player_1", "username": "Scout", "credits": 44 },
        "ship": { "fuel": 90, "max_fuel": 100 },
        "location": {
            "system_id": "sol",
            "poi_id": "earth_station",
            "docked_at": "earth_station"
        }
    }))
    .await;

    {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        session.spacemolt_account = Some(account);
    }

    service.refresh_state(id).await.expect("refresh");
    let snapshot = service
        .engine_snapshot_response(id)
        .await
        .expect("snapshot");
    assert_eq!(snapshot.username.as_deref(), Some("Scout"));
    assert_eq!(snapshot.latest_system.as_deref(), Some("sol"));
}

#[tokio::test]
async fn initial_refresh_hydrates_owned_ships_from_list_ships() {
    let service = Arc::new(RuntimeService::new());
    let id = service.create_session();
    let (account, socket) = seeded_test_account_with_socket(
        Some("player_1".to_string()),
        serde_json::json!({
            "player": { "id": "player_1", "username": "Scout", "credits": 44 },
            "ship": { "id": "ship_1", "fuel": 90, "max_fuel": 100 },
            "location": { "system_id": "sol", "poi_id": "deep_space" }
        }),
    )
    .await;
    {
        let session = service.get_session(id).await.expect("session");
        session.lock().await.spacemolt_account = Some(account);
    }

    let refresh_service = Arc::clone(&service);
    let refresh = tokio::spawn(async move { refresh_service.refresh_state(id).await });
    let request = socket.wait_for_action("list_ships").await;
    socket.server_send(spacemolt_lib_rs::RawFrame {
        kind: "result".to_string(),
        request_id: request.request_id,
        payload: Some(serde_json::json!({
            "result": "ok",
            "structuredContent": {
                "active_ship_class": "runner",
                "active_ship_id": "ship_1",
                "count": 2,
                "ships": [
                    { "ship_id": "ship_1", "class_id": "runner", "is_active": true },
                    { "ship_id": "ship_2", "class_id": "hauler", "is_active": false }
                ]
            }
        })),
    });
    refresh.await.expect("refresh task").expect("refresh");

    let session = service.get_session(id).await.expect("session");
    let session = session.lock().await;
    assert_eq!(session.actor.observed.owned_ship_details.len(), 2);
    assert_eq!(
        session.actor.observed.owned_ship_details[0].ship_id,
        "ship_1"
    );
    assert_eq!(
        session.actor.observed.owned_ship_details[1].ship_id,
        "ship_2"
    );
}

#[tokio::test]
async fn session_summary_includes_latest_location_after_refresh() {
    let service = RuntimeService::new();
    let id = service.create_session();
    let account = seeded_test_account(serde_json::json!({
        "player": { "id": "player_1", "username": "Scout", "credits": 44 },
        "ship": { "fuel": 90, "max_fuel": 100 },
        "location": {
            "system_id": "sol",
            "poi_id": "earth_station",
            "docked_at": "earth_station"
        }
    }))
    .await;

    {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        session.spacemolt_account = Some(account);
    }

    service.refresh_state(id).await.expect("refresh");
    let cached = service
        .session_summary_cache
        .lock()
        .get(&id)
        .cloned()
        .expect("cached summary");
    assert_eq!(cached.latest_system.as_deref(), Some("sol"));
    assert_eq!(cached.latest_poi.as_deref(), Some("earth_station"));

    let summaries = service.list_sessions().await;
    let summary = summaries
        .iter()
        .find(|row| row.id == id.to_string())
        .expect("summary");
    let value = serde_json::to_value(summary).expect("summary json");

    assert_eq!(value["latestSystem"], "sol");
    assert_eq!(value["latestPoi"], "earth_station");
}

#[test]
fn persisted_session_stores_spacemolt_selector_without_credentials() {
    let id = Uuid::new_v4();
    let value = serde_json::json!({
        "session_schema_version": SESSION_SCHEMA_VERSION,
        "sessions": [{
            "id": id,
            "label": "Scout",
            "created_utc": "2026-07-09T00:00:00Z",
            "last_updated_utc": "2026-07-09T00:00:00Z",
            "execution": RuntimeEngine::default().execution_checkpoint().expect("checkpoint"),
            "spacemolt_account_selector": "scout",
            "spacemolt_base_url": "https://game.spacemolt.com"
        }]
    });

    let records = parse_runtime_sessions_value(value).expect("records");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].spacemolt_account_selector.as_deref(),
        Some("scout")
    );
    assert_eq!(
        records[0].spacemolt_base_url.as_deref(),
        Some("https://game.spacemolt.com")
    );
}

#[test]
fn storage_market_prices_use_median_buy_sell_and_credit_fallback() {
    let market = MarketData {
        station_markets: HashMap::from([
            (
                "station_a".to_string(),
                StationMarketData {
                    buy_orders: HashMap::from([(
                        "iron".to_string(),
                        vec![MarketOrder {
                            price_each: 10,
                            quantity: 100,
                            source: None,
                            my_quantity: None,
                        }],
                    )]),
                    sell_orders: HashMap::from([(
                        "iron".to_string(),
                        vec![MarketOrder {
                            price_each: 14,
                            quantity: 100,
                            source: None,
                            my_quantity: None,
                        }],
                    )]),
                    ..StationMarketData::default()
                },
            ),
            (
                "station_b".to_string(),
                StationMarketData {
                    buy_orders: HashMap::from([(
                        "iron".to_string(),
                        vec![MarketOrder {
                            price_each: 20,
                            quantity: 1,
                            source: None,
                            my_quantity: None,
                        }],
                    )]),
                    ..StationMarketData::default()
                },
            ),
        ]),
        ..MarketData::default()
    };

    let prices = storage_market_prices(&market);

    assert_eq!(prices["iron"].median_buy_price, Some(15.0));
    assert_eq!(prices["iron"].median_sell_price, Some(14.0));
    assert_eq!(prices["credits"].median_buy_price, Some(1.0));
    assert_eq!(prices["credits"].median_sell_price, Some(1.0));
}

#[test]
fn storage_market_prices_include_buy_only_markets() {
    let market = MarketData {
        station_markets: HashMap::from([(
            "station_a".to_string(),
            StationMarketData {
                buy_orders: HashMap::from([(
                    "iron".to_string(),
                    vec![MarketOrder {
                        price_each: 10,
                        quantity: 100,
                        source: None,
                        my_quantity: None,
                    }],
                )]),
                ..StationMarketData::default()
            },
        )]),
        ..MarketData::default()
    };

    let prices = storage_market_prices(&market);

    assert_eq!(prices["iron"].median_buy_price, Some(10.0));
    assert_eq!(prices["iron"].median_sell_price, None);
    assert_eq!(prices["credits"].median_buy_price, Some(1.0));
    assert_eq!(prices["credits"].median_sell_price, Some(1.0));
}

#[test]
fn startup_restore_has_no_prayer_side_sleep() {
    let source = include_str!("../../service.rs");
    let env_knob = [
        "PRAYER",
        "_STARTUP",
        "_SESSION",
        "_HYDRATION",
        "_DELAY",
        "_MS",
    ]
    .concat();
    let helper = ["startup", "_session", "_hydration", "_delay"].concat();

    assert!(!source.contains(&env_knob));
    assert!(!source.contains(&helper));
}

#[test]
fn mobile_capital_agent_sightings_use_host_system() {
    let mut knowledge = WorldState::default();
    let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
    let mut galaxy = GalaxyData::default();
    assert!(apply_mobile_capital_location(&mut galaxy, "frontier_alpha"));
    knowledge.galaxy = Arc::new(galaxy);

    assert!(merge_knowledge_state_if_changed_with_metadata(
        &mut knowledge,
        &mut metadata,
        &StateObservation {
            agents: Some(AgentsObservation {
                system_id: MOBILE_BASE_POI_ID.to_string(),
                observed_at_unix: 1_000,
                agents: vec![sighting("MarketBot", MOBILE_BASE_POI_ID, 1_000)],
            }),
            agents_fetched: true,
            status_system: Some("frontier_alpha".to_string()),
            ..StateObservation::default()
        },
    ));

    assert_eq!(
        knowledge.agent_sightings["p_MarketBot"].last_seen_system,
        "frontier_alpha"
    );
    assert!(metadata
        .agents_fetched_at_by_system
        .contains_key("frontier_alpha"));
    assert!(!metadata
        .agents_fetched_at_by_system
        .contains_key(MOBILE_BASE_POI_ID));
}

#[test]
fn refreshed_catalog_replaces_stale_item_ids() {
    let mut knowledge = WorldState {
        catalog: Arc::new(CatalogData {
            items: HashMap::from([(
                "copper_piping".to_string(),
                test_catalog_item("copper_piping", "Copper Piping", 1),
            )]),
            version: Some("0.376.0".to_string()),
            ..CatalogData::default()
        }),
        ..WorldState::default()
    };
    let observation = StateObservation {
        catalog: Some(CatalogData {
            version: Some("0.377.0".to_string()),
            items: HashMap::from([(
                "copper_wiring".to_string(),
                test_catalog_item("copper_wiring", "Copper Wiring", 1),
            )]),
            ..CatalogData::default()
        }),
        ..StateObservation::default()
    };

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &observation
    ));

    let catalog = knowledge.catalog.as_ref();
    assert!(catalog.items.contains_key("copper_wiring"));
    assert!(!catalog.items.contains_key("copper_piping"));
    assert_eq!(catalog.version.as_deref(), Some("0.377.0"));
}

#[test]
fn facility_snapshot_freshness_uses_twenty_four_hour_ttl() {
    let now = Utc::now().timestamp();
    assert!(facility_snapshot_fresh(
        &PoiFacilitiesSnapshot {
            observed_at_unix: now - FACILITY_POI_SNAPSHOT_TTL_SECS + 1,
            current: None,
            faction_current: None,
        },
        now
    ));
    assert!(!facility_snapshot_fresh(
        &PoiFacilitiesSnapshot {
            observed_at_unix: now - FACILITY_POI_SNAPSHOT_TTL_SECS,
            current: None,
            faction_current: None,
        },
        now
    ));
}

#[test]
fn facility_snapshots_are_persisted_world_knowledge() {
    let mut with_facilities = WorldState::default();
    with_facilities.facilities_by_poi.insert(
        "poi_station_a".to_string(),
        PoiFacilitiesSnapshot {
            observed_at_unix: 1_000,
            current: None,
            faction_current: None,
        },
    );

    assert!(!world_knowledge_persisted_eq(
        &WorldState::default(),
        &with_facilities
    ));
}

#[test]
fn knowledge_persistence_pending_slot_keeps_only_newest_version() {
    let request = |knowledge_version| KnowledgePersistenceRequest {
        snapshot: Arc::new(WorldState {
            knowledge_version,
            ..WorldState::default()
        }),
        context: "test",
    };
    let mut pending = None;

    assert!(replace_pending_knowledge_snapshot(&mut pending, request(2)));
    assert!(!replace_pending_knowledge_snapshot(
        &mut pending,
        request(1)
    ));
    assert_eq!(
        pending
            .as_ref()
            .map(|request| request.snapshot.knowledge_version),
        Some(2)
    );

    assert!(replace_pending_knowledge_snapshot(&mut pending, request(3)));
    assert_eq!(
        pending
            .as_ref()
            .map(|request| request.snapshot.knowledge_version),
        Some(3)
    );
}

#[test]
fn knowledge_persistence_worker_never_replaces_newer_file_with_older_snapshot() {
    let dir = std::path::PathBuf::from("/tmp").join(format!(
        "prayerrs-knowledge-persistence-test-{}",
        Uuid::new_v4()
    ));
    let path = dir.join("knowledge-state.json");
    let persistence = KnowledgePersistence::start(
        path.clone(),
        Arc::new(PersistenceTelemetry::default()),
        None,
    );
    let snapshot = |knowledge_version| WorldState {
        knowledge_version,
        ..WorldState::default()
    };

    persistence.publish(snapshot(2), "newer test snapshot");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(bytes) = std::fs::read(&path) {
            let persisted: PersistedWorldKnowledgeV4 =
                serde_json::from_slice(&bytes).expect("persisted knowledge JSON");
            if persisted.state.knowledge_version == 2 {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for knowledge snapshot"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    persistence.publish(snapshot(1), "late older test snapshot");
    std::thread::sleep(Duration::from_millis(100));
    let persisted: PersistedWorldKnowledgeV4 =
        serde_json::from_slice(&std::fs::read(&path).expect("persisted knowledge file"))
            .expect("persisted knowledge JSON");
    assert_eq!(persisted.state.knowledge_version, 2);

    drop(persistence);
    std::fs::remove_dir_all(dir).expect("remove persistence test directory");
}

#[test]
fn facility_types_are_projected_from_shared_galaxy_catalog() {
    let catalog = CatalogData {
        facilities: HashMap::from([(
            "faction_storage".to_string(),
            spacemolt_lib_rs::schema::FacilityDefinition {
                category: "faction".to_string(),
                level: 1,
                build_cost: 5000,
                ..test_facility("faction_storage", "Faction Storage")
            },
        )]),
        ..CatalogData::default()
    };

    let types = facility_types_from_catalog(&catalog);
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].id, "faction_storage");
    assert_eq!(types[0].name, "Faction Storage");
    assert_eq!(types[0].level, 1);
    assert_eq!(types[0].build_cost, 5000);
}

#[test]
fn canonical_catalog_parser_keys_top_level_arrays_by_id() {
    let ship: spacemolt_lib_rs::schema::ShipClass = serde_json::from_value(serde_json::json!({
        "id": "scout", "name": "Scout", "description": "Scout", "class": "scout",
        "price": 0, "tier": 1, "scale": 1, "base_hull": 1, "base_shield": 1,
        "base_shield_recharge": 1, "base_armor": 1, "base_speed": 1, "base_fuel": 1,
        "cargo_capacity": 1, "cpu_capacity": 1, "power_capacity": 1,
        "weapon_slots": 0, "defense_slots": 0, "utility_slots": 0,
        "shipyard_tier": 1, "build_time": 1
    }))
    .expect("valid ship class");
    let recipe: spacemolt_lib_rs::schema::Recipe = serde_json::from_value(serde_json::json!({
        "id": "smelt_steel_plate", "name": "Smelt Steel Plate",
        "description": "Smelt Steel Plate", "category": "test", "inputs": [],
        "outputs": [], "crafting_time": 1
    }))
    .expect("valid recipe");
    let skill: spacemolt_lib_rs::schema::SkillDefinition =
        serde_json::from_value(serde_json::json!({
            "id": "mining", "name": "Mining", "description": "Mining",
            "category": "test", "max_level": 1, "xp_per_level": [1]
        }))
        .expect("valid skill");
    let value = serde_json::json!({
        "version": "0.450.2",
        "achievements": [],
        "faction_achievements": [],
        "hidden_achievement_count": 0,
        "hidden_faction_achievement_count": 0,
        "items": [test_catalog_item("steel_plate", "Steel Plate", 1)],
        "ships": [ship],
        "recipes": [recipe],
        "facilities": [test_facility("workshop", "Workshop")],
        "skills": [skill]
    });

    let cache = spacemolt_lib_rs::data::CatalogCache::new(value).expect("typed catalog cache");
    let catalog = canonical_catalog_from_cache(&cache).expect("canonical catalog");

    assert_eq!(catalog.version.as_deref(), Some("0.450.2"));
    assert_eq!(catalog.items["steel_plate"].name(), "Steel Plate");
    assert_eq!(catalog.ships["scout"].name, "Scout");
    assert_eq!(
        catalog.recipes["smelt_steel_plate"].name,
        "Smelt Steel Plate"
    );
    assert_eq!(catalog.facilities["workshop"].name, "Workshop");
    assert_eq!(catalog.skills["mining"].name, "Mining");
}

#[test]
fn catalog_root_schema_snapshot_covers_all_required_roots() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../prayer-api/testdata/catalog_schema/root_type_schemas.json"
    ))
    .expect("schema snapshot");
    let root_keys = schema["root_keys"]
        .as_array()
        .expect("root keys")
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        root_keys,
        BTreeSet::from([
            "facilities",
            "items",
            "recipes",
            "ships",
            "skills",
            "version"
        ])
    );
    for root in CANONICAL_CATALOG_ROOTS {
        assert!(
            schema["root_counts"][*root].as_u64().unwrap_or_default() > 0,
            "expected non-empty root count for {root}"
        );
        assert!(
            schema["schemas"][*root]["id"]
                .as_array()
                .is_some_and(|types: &Vec<Value>| types.iter().any(|value| value == "string")),
            "expected {root} schema to include string id"
        );
    }
}

#[test]
fn catalog_root_schema_snapshot_matches_source_when_provided() {
    let Some(path) = option_env!("PRAYER_CATALOG_SCHEMA_SOURCE").map(std::path::PathBuf::from)
    else {
        return;
    };
    let source_path = path;
    let source = std::fs::read_to_string(&source_path).expect("read catalog schema source");
    let catalog: Value = serde_json::from_str(&source).expect("parse catalog schema source");
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../prayer-api/testdata/catalog_schema/root_type_schemas.json"
    ))
    .expect("schema snapshot");

    assert_eq!(
        catalog_root_type_schema_from_value(&catalog),
        expected,
        "catalog schema source differed from checked-in snapshot: {}",
        source_path.display()
    );
}

#[test]
fn facility_types_read_build_material_arrays_from_catalog() {
    let catalog = CatalogData {
        facilities: HashMap::from([(
            "workshop".to_string(),
            spacemolt_lib_rs::schema::FacilityDefinition {
                category: "production".to_string(),
                build_materials: serde_json::from_value(serde_json::json!([
                    { "item_id": "steel_plate", "quantity": 20 },
                    { "item_id": "circuit_board", "quantity": 4 }
                ]))
                .expect("valid build materials"),
                ..test_facility("workshop", "Workshop")
            },
        )]),
        ..CatalogData::default()
    };

    let types = facility_types_from_catalog(&catalog);

    assert_eq!(types.len(), 1);
    assert!(types[0]
        .build_materials
        .iter()
        .any(|item| item.item_id == "steel_plate" && item.quantity == 20));
    assert!(types[0]
        .build_materials
        .iter()
        .any(|item| item.item_id == "circuit_board" && item.quantity == 4));
}

#[test]
fn map_freshness_does_not_bump_knowledge_version() {
    let observation = StateObservation {
        map_fetched: true,
        ..StateObservation::default()
    };
    let mut knowledge = WorldState::default();
    let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();

    assert!(!merge_knowledge_state_if_changed_with_metadata(
        &mut knowledge,
        &mut metadata,
        &observation
    ));
    assert_eq!(knowledge.knowledge_version, 0);
    assert!(metadata.map_fetched_at.is_some());
}

fn sighting(username: &str, system: &str, unix: i64) -> AgentSightingData {
    AgentSightingData {
        contact: spacemolt_lib_rs::schema::NearbyPlayer {
            player_id: Some(format!("p_{username}")),
            username: Some(username.to_string()),
            ..AgentSightingData::default().contact
        },
        last_seen_system: system.to_string(),
        first_seen_unix: unix,
        last_seen_unix: unix,
        times_seen: 1,
    }
}

fn agents_observation(agents: Vec<AgentSightingData>, unix: i64) -> StateObservation {
    let system_id = agents
        .first()
        .map(|a| a.last_seen_system.clone())
        .unwrap_or_default();
    StateObservation {
        agents: Some(AgentsObservation {
            system_id,
            observed_at_unix: unix,
            agents,
        }),
        ..StateObservation::default()
    }
}

#[test]
fn agent_sightings_accumulate_without_version_churn() {
    let mut knowledge = WorldState::default();

    // First sighting is recorded and bumps the knowledge version.
    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &agents_observation(vec![sighting("OreRustler", "sol", 1_000)], 1_000)
    ));
    assert_eq!(knowledge.agent_sightings["p_OreRustler"].times_seen, 1);

    // Same sighting shortly after: kept as-is, no version churn.
    assert!(!merge_knowledge_state_if_changed(
        &mut knowledge,
        &agents_observation(vec![sighting("OreRustler", "sol", 1_060)], 1_060)
    ));
    assert_eq!(
        knowledge.agent_sightings["p_OreRustler"].last_seen_unix,
        1_000
    );

    // Moved system: recorded immediately; identity history survives.
    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &agents_observation(vec![sighting("OreRustler", "alpha", 1_120)], 1_120)
    ));
    let stored = &knowledge.agent_sightings["p_OreRustler"];
    assert_eq!(stored.last_seen_system, "alpha");
    assert_eq!(stored.first_seen_unix, 1_000);
    assert_eq!(stored.last_seen_unix, 1_120);
    assert_eq!(stored.times_seen, 2);

    // Unchanged but past the restamp threshold: timestamp refreshed.
    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &agents_observation(vec![sighting("OreRustler", "alpha", 1_420)], 1_420)
    ));
    let stored = &knowledge.agent_sightings["p_OreRustler"];
    assert_eq!(stored.last_seen_unix, 1_420);
    assert_eq!(stored.times_seen, 3);
}

#[tokio::test]
async fn faction_garage_watchers_skip_non_selected_sessions() {
    let service = RuntimeService::default();
    let first = service
        .create_session_with_label(Some("Garage Alpha".to_string()))
        .expect("first session");
    let second = service
        .create_session_with_label(Some("Garage Beta".to_string()))
        .expect("second session");

    for id in [first, second] {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        session.bot_state_mut().player.faction_id = Some("fac_traders".to_string());
        session.bot_state_mut().player.faction_id = Some("fac_traders".to_string());
    }

    service.reconcile_refresh_watchers().await;
    let faction_garage_watcher = *service
        .faction_garage_watchers_by_key
        .lock()
        .get("fac_traders")
        .expect("faction garage watcher");
    assert!([first, second].contains(&faction_garage_watcher));
}

#[tokio::test]
async fn market_watchers_keep_one_session_per_station_until_it_leaves() {
    let service = RuntimeService::default();
    let first = service
        .create_session_with_label(Some("Market Alpha".to_string()))
        .expect("first session");
    let second = service
        .create_session_with_label(Some("Market Beta".to_string()))
        .expect("second session");

    for id in [first, second] {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        session.bot_state_mut().location.poi_id = Some("grand_exchange".to_string());
        session.bot_state_mut().location.docked_at = Some("grand_exchange".to_string());
        session.bot_state_mut().location.poi_id = Some("grand_exchange".to_string());
        session.bot_state_mut().location.docked_at = Some("grand_exchange".to_string());
    }

    service.reconcile_refresh_watchers().await;
    let watcher = *service
        .market_watchers
        .lock()
        .get("grand_exchange")
        .expect("market watcher");
    let non_watcher = if watcher == first { second } else { first };

    assert!([first, second].contains(&watcher));

    {
        let session = service.get_session(watcher).await.expect("watcher session");
        let mut session = session.lock().await;
        session.bot_state_mut().location.poi_id = None;
        session.bot_state_mut().location.docked_at = None;
        session.bot_state_mut().location.poi_id = None;
        session.bot_state_mut().location.docked_at = None;
    }

    service.reconcile_refresh_watchers().await;
    assert_eq!(
        service
            .market_watchers
            .lock()
            .get("grand_exchange")
            .copied(),
        Some(non_watcher)
    );
}

#[tokio::test]
async fn observation_watchers_keep_one_connected_session_per_poi() {
    let service = RuntimeService::default();
    let first = service
        .create_session_with_label(Some("Observation Alpha".to_string()))
        .expect("first session");
    let second = service
        .create_session_with_label(Some("Observation Beta".to_string()))
        .expect("second session");
    let third = service
        .create_session_with_label(Some("Observation Gamma".to_string()))
        .expect("third session");
    let state = |username: &str, poi_id: &str| {
        serde_json::json!({
            "player": { "id": format!("player_{username}"), "username": username },
            "location": { "system_id": "sol", "poi_id": poi_id }
        })
    };
    let (first_account, _first_socket) =
        seeded_test_account_with_socket(Some("alpha".to_string()), state("Alpha", "earth_orbit"))
            .await;
    let (second_account, _second_socket) =
        seeded_test_account_with_socket(Some("beta".to_string()), state("Beta", "earth_orbit"))
            .await;
    let (third_account, _third_socket) =
        seeded_test_account_with_socket(Some("gamma".to_string()), state("Gamma", "mars_orbit"))
            .await;
    service
        .get_session(first)
        .await
        .expect("first session")
        .lock()
        .await
        .spacemolt_account = Some(first_account);
    service
        .get_session(second)
        .await
        .expect("second session")
        .lock()
        .await
        .spacemolt_account = Some(second_account);
    service
        .get_session(third)
        .await
        .expect("third session")
        .lock()
        .await
        .spacemolt_account = Some(third_account);

    let owner = service
        .observation_subscription_owner_for_poi("earth_orbit")
        .await
        .expect("observation owner");
    assert_eq!(owner, first.min(second));
    assert_eq!(
        service
            .observation_subscription_owner_for_poi("mars_orbit")
            .await,
        Some(third)
    );

    service
        .get_session(owner)
        .await
        .expect("owner session")
        .lock()
        .await
        .spacemolt_account = None;
    let replacement = service
        .observation_subscription_owner_for_poi("earth_orbit")
        .await;
    assert_eq!(
        replacement,
        Some(if owner == first { second } else { first })
    );
}

#[tokio::test]
async fn market_subscription_refresh_uses_one_account_per_station() {
    let service = Arc::new(RuntimeService::default());
    let first = service
        .create_session_with_label(Some("Market Alpha".to_string()))
        .expect("first session");
    let second = service
        .create_session_with_label(Some("Market Beta".to_string()))
        .expect("second session");
    let state = |username: &str| {
        serde_json::json!({
            "player": { "id": format!("player_{username}"), "username": username, "credits": 44 },
            "ship": { "fuel": 90, "max_fuel": 100 },
            "location": {
                "system_id": "sol",
                "poi_id": "grand_exchange_poi",
                "docked_at": "grand_exchange"
            }
        })
    };
    let (first_account, first_socket) =
        seeded_test_account_with_socket(Some("alpha".to_string()), state("Alpha")).await;
    let (second_account, second_socket) =
        seeded_test_account_with_socket(Some("beta".to_string()), state("Beta")).await;
    {
        let session = service.get_session(first).await.expect("first session");
        session.lock().await.spacemolt_account = Some(first_account.clone());
    }
    {
        let session = service.get_session(second).await.expect("second session");
        session.lock().await.spacemolt_account = Some(second_account.clone());
    }

    let owner = first.min(second);
    let non_owner = if owner == first { second } else { first };
    let owner_socket = if owner == first {
        first_socket.clone()
    } else {
        second_socket.clone()
    };
    let non_owner_socket = if non_owner == first {
        first_socket.clone()
    } else {
        second_socket.clone()
    };
    let owner_account = if owner == first {
        first_account.clone()
    } else {
        second_account.clone()
    };
    let non_owner_account = if non_owner == first {
        first_account.clone()
    } else {
        second_account.clone()
    };

    let owner_service = Arc::clone(&service);
    let owner_refresh = tokio::spawn(async move { owner_service.refresh_state(owner).await });
    let request = owner_socket.wait_for_action("subscribe_market").await;
    owner_socket.server_send(spacemolt_lib_rs::RawFrame {
        kind: "result".to_string(),
        request_id: request.request_id,
        payload: Some(serde_json::json!({
            "result": "ok",
            "structuredContent": {
                "action": "subscribe_market",
                "base_id": "grand_exchange",
                "base_name": "Grand Exchange",
                "items": [
                    {
                        "item_id": "iron_ore",
                        "sell_orders": [{ "price_each": 12, "quantity": 3 }],
                        "buy_orders": [{ "price_each": 10, "quantity": 4 }]
                    }
                ]
            }
        })),
    });
    owner_refresh
        .await
        .expect("owner refresh task")
        .expect("owner refresh");
    assert!(owner_account.market_subscribed());

    service.reconcile_refresh_watchers().await;
    let non_owner_service = Arc::clone(&service);
    let non_owner_refresh =
        tokio::spawn(async move { non_owner_service.refresh_state(non_owner).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        non_owner_socket
            .sent()
            .iter()
            .filter(|frame| frame.action == "subscribe_market")
            .count(),
        0,
        "non-owner must not issue subscribe_market"
    );
    non_owner_refresh.abort();

    assert!(!non_owner_account.market_subscribed());
    assert!(service
        .knowledge_state
        .read()
        .station_markets
        .contains_key("grand_exchange_poi"));
}

#[tokio::test]
async fn market_watcher_reconcile_prunes_unoccupied_station_snapshots() {
    let service = RuntimeService::default();
    let session_id = service
        .create_session_with_label(Some("Market Alpha".to_string()))
        .expect("session");
    {
        let session = service.get_session(session_id).await.expect("session");
        let mut session = session.lock().await;
        session.bot_state_mut().location.poi_id = Some("grand_exchange".to_string());
        session.bot_state_mut().location.docked_at = Some("grand_exchange".to_string());
        session.bot_state_mut().location.poi_id = Some("grand_exchange".to_string());
        session.bot_state_mut().location.docked_at = Some("grand_exchange".to_string());
    }
    {
        let mut knowledge = service.knowledge_state.write();
        knowledge
            .station_markets
            .insert("grand_exchange".to_string(), station_snapshot_at(10, 1_000));
        knowledge
            .station_markets
            .insert("mobile_capital".to_string(), station_snapshot_at(20, 1_000));
    }

    service.reconcile_refresh_watchers().await;

    let knowledge = service.knowledge_state.read();
    assert!(knowledge.station_markets.contains_key("grand_exchange"));
    assert!(!knowledge.station_markets.contains_key("mobile_capital"));
}

#[tokio::test]
async fn refresh_reconcile_prunes_unoccupied_station_passenger_boards() {
    let service = RuntimeService::default();
    let session_id = service
        .create_session_with_label(Some("Passenger Alpha".to_string()))
        .expect("session");
    {
        let session = service.get_session(session_id).await.expect("session");
        let mut session = session.lock().await;
        session.bot_state_mut().location.poi_id = Some("grand_exchange".to_string());
        session.bot_state_mut().location.docked_at = Some("grand_exchange".to_string());
        session.bot_state_mut().location.poi_id = Some("grand_exchange".to_string());
        session.bot_state_mut().location.docked_at = Some("grand_exchange".to_string());
    }
    {
        let mut knowledge = service.knowledge_state.write();
        knowledge.station_passengers.insert(
            "grand_exchange".to_string(),
            passenger_board("grand_exchange", "Ada"),
        );
        knowledge.station_passengers.insert(
            "mobile_capital".to_string(),
            passenger_board("mobile_capital", "Grace"),
        );
        service
            .knowledge_metadata
            .write()
            .station_passengers_fetched_at_by_station
            .insert("mobile_capital".to_string(), Instant::now());
    }

    service.reconcile_refresh_watchers().await;

    let knowledge = service.knowledge_state.read();
    assert!(knowledge.station_passengers.contains_key("grand_exchange"));
    assert!(!knowledge.station_passengers.contains_key("mobile_capital"));
    assert!(!service
        .knowledge_metadata
        .read()
        .station_passengers_fetched_at_by_station
        .contains_key("mobile_capital"));
}

#[test]
fn command_action_classifiers_require_tool_and_action() {
    assert!(mission_related_api_action("spacemolt", "accept_mission"));
    assert!(mission_related_api_action("spacemolt", "complete_mission"));
    assert!(!mission_related_api_action(
        "spacemolt_market",
        "accept_mission"
    ));

    assert!(crafting_queue_related_api_action("spacemolt", "craft"));
    assert!(!crafting_queue_related_api_action(
        "spacemolt_storage",
        "craft"
    ));

    assert!(market_related_api_action(
        "spacemolt_market",
        "create_buy_order"
    ));
    assert!(market_related_api_action(
        "spacemolt_market",
        "create_sell_order"
    ));
    assert!(market_related_api_action(
        "spacemolt_market",
        "cancel_order"
    ));
    assert!(!market_related_api_action("spacemolt", "cancel_order"));

    assert!(commission_related_api_action(
        "spacemolt_ship",
        "commission_ship"
    ));
    assert!(commission_related_api_action(
        "spacemolt_ship",
        "cancel_commission"
    ));
    assert!(!commission_related_api_action(
        "spacemolt",
        "commission_ship"
    ));
}

#[test]
fn switch_ship_classifier_marks_switch_ship_only() {
    let switch_command = prayer_actions::ResolvedAction {
        action: "switch_ship".to_string(),
        args: vec![prayer_actions::ActionArg::ShipId("ship_abc123".to_string())],
        source_line: None,
    };
    assert!(switch_ship_related_api_action(
        &switch_command,
        "spacemolt_ship",
        "switch_ship"
    ));
    assert!(!switch_ship_related_api_action(
        &switch_command,
        "spacemolt_storage",
        "deposit"
    ));

    let transfer_command = prayer_actions::ResolvedAction {
        action: "transfer".to_string(),
        args: vec![prayer_actions::ActionArg::ShipId("ship_abc123".to_string())],
        source_line: None,
    };
    assert!(!switch_ship_related_api_action(
        &transfer_command,
        "spacemolt_ship",
        "switch_ship"
    ));
}

#[test]
fn switch_ship_already_active_error_matches_exact_api_rejection() {
    let error = OperationFailure::Client(spacemolt_lib_rs::ClientError::Server(
        spacemolt_lib_rs::SpacemoltError::new(
            "already_active",
            "That is already your active ship.",
        ),
    ));
    assert!(switch_ship_already_active_error(&error));

    let other_bad_request = OperationFailure::Client(spacemolt_lib_rs::ClientError::Server(
        spacemolt_lib_rs::SpacemoltError::new("not_here", "ship is not here"),
    ));
    assert!(!switch_ship_already_active_error(&other_bad_request));

    let text_only =
        OperationFailure::Policy("already_active: That is already your active ship.".to_string());
    assert!(!switch_ship_already_active_error(&text_only));
}

#[test]
fn unload_all_no_passengers_error_matches_exact_api_rejection() {
    let error = OperationFailure::Client(spacemolt_lib_rs::ClientError::Server(
        spacemolt_lib_rs::SpacemoltError::new(
            "no_passengers",
            "You have no passengers aboard to unload.",
        ),
    ));
    // Runtime lowering uses the upstream API's `id` field for the command's
    // passenger name (including the special `all` value).
    let payload = serde_json::json!({ "id": "all" });

    assert!(unload_all_no_passengers_error(
        "spacemolt",
        "unload_passenger",
        Some(&payload),
        &error
    ));

    let named_payload = serde_json::json!({ "id": "Ada" });
    assert!(!unload_all_no_passengers_error(
        "spacemolt",
        "unload_passenger",
        Some(&named_payload),
        &error
    ));
    assert!(!unload_all_no_passengers_error(
        "spacemolt",
        "load_passenger",
        Some(&payload),
        &error
    ));
}

#[test]
fn craft_enqueue_response_is_preserved_as_raw_queue_entry() {
    let response = serde_json::json!({
        "result": "Crafting queued: 1 run(s) of Refine Steel at Iron Refinery, making 2x Steel Plate (job 50209e8fa8068147fd28ca632328f913)."
    });
    let message = craft_enqueue_response_text(&response).expect("enqueue message");
    let mut state = prayer_state::BotState::default();

    preserve_craft_enqueue_as_queue(&mut state, &message);

    assert_eq!(state.crafting_queue.len(), 1);
    assert!(state.crafting_queue[0]
        .raw_text
        .as_deref()
        .is_some_and(|text| text.contains("50209e8fa8068147fd28ca632328f913")));
    assert_eq!(
        state.crafting_queue[0].source.as_deref(),
        Some("craft_enqueue")
    );
}

fn station_snapshot_at(price: i64, observed_at_unix: i64) -> StationMarketData {
    StationMarketData {
        buy_orders: HashMap::from([(
            "iron".to_string(),
            vec![MarketOrder {
                price_each: price,
                quantity: 10,
                source: None,
                my_quantity: None,
            }],
        )]),
        observed_at_unix: Some(observed_at_unix),
        ..StationMarketData::default()
    }
}

fn passenger_board(station_id: &str, passenger_name: &str) -> prayer_state::PassengerState {
    let mut passengers = prayer_state::PassengerState {
        station: station_id.to_string(),
        waiting_count: Some(1),
        waiting: Arc::new(vec![spacemolt_lib_rs::schema::WaitingPassengerView {
            bio: String::new(),
            citizen_id: format!("passenger-{passenger_name}"),
            citizenship: String::new(),
            class: "economy".to_string(),
            name: passenger_name.to_string(),
            destination: "vega_terminal".to_string(),
            destination_name: String::new(),
            destination_system: None,
            estimated_fare: None,
        }]),
        ..prayer_state::PassengerState::default()
    };
    passengers.aboard_count = Some(99);
    passengers
}

fn virtual_order(
    id: &str,
    side: &str,
    item_id: &str,
    station_id: &str,
    price_each: i64,
    quantity: i64,
) -> RuntimeVirtualMarketOrderDto {
    RuntimeVirtualMarketOrderDto {
        id: id.to_string(),
        status: "available".to_string(),
        side: side.to_string(),
        item_id: item_id.to_string(),
        station_id: station_id.to_string(),
        price_each,
        quantity,
        tipping_point: None,
        dumping: false,
        reserved: 0,
        reservation_id: None,
        filled: 0,
        enabled: true,
        internal_only: false,
        priority: 1.0,
        do_forever: false,
    }
}

fn virtual_order_knowledge(virtual_orders: Vec<RuntimeVirtualMarketOrderDto>) -> WorldState {
    WorldState {
        galaxy: Arc::new(GalaxyData {
            system_records: HashMap::from([
                (
                    "sol".into(),
                    prayer_state::SystemKnowledge {
                        id: "sol".into(),
                        connections: vec!["vega".into()],
                        ..Default::default()
                    },
                ),
                (
                    "vega".into(),
                    prayer_state::SystemKnowledge {
                        id: "vega".into(),
                        connections: vec!["sol".into()],
                        ..Default::default()
                    },
                ),
            ]),
            poi_records: HashMap::from([
                (
                    "station_sol".into(),
                    prayer_state::PoiKnowledge {
                        id: "station_sol".into(),
                        system_id: "sol".into(),
                        info: PoiInfoData {
                            poi_type: "station".into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
                (
                    "station_vega".into(),
                    prayer_state::PoiKnowledge {
                        id: "station_vega".into(),
                        system_id: "vega".into(),
                        info: PoiInfoData {
                            poi_type: "station".into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
            ]),
            ..GalaxyData::default()
        }),
        catalog: Arc::new(CatalogData {
            items: HashMap::from([("iron".to_string(), test_catalog_item("iron", "Iron", 1))]),
            ..CatalogData::default()
        }),
        virtual_orders,
        ..WorldState::default()
    }
}

fn old_station_snapshot(price: i64) -> StationMarketData {
    station_snapshot_at(price, Utc::now().timestamp().saturating_sub(3_600))
}

fn salvage_snapshot(lootable: SpaceLootInfo, observed_at_unix: i64) -> SalvageData {
    SalvageData {
        visible_lootables: vec![lootable],
        last_seen_poi: Some("poi_station_a".to_string()),
        last_seen_system: Some("sol".to_string()),
        observed_at_unix: Some(observed_at_unix),
        ..SalvageData::default()
    }
}

#[test]
fn market_knowledge_is_process_local_versioned_and_not_persisted() {
    let mut knowledge = WorldState::default();
    let observation = test_observation(
        BotState::default(),
        prayer_runtime::snapshot::WorldObservation {
            market: Arc::new(MarketData {
                station_markets: HashMap::from([(
                    "poi_station_a".to_string(),
                    station_snapshot_at(10, 1_000),
                )]),
                ..MarketData::default()
            }),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
    );

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &observation
    ));
    assert_eq!(knowledge.knowledge_version, 1);
    assert_eq!(
        knowledge.station_markets["poi_station_a"].buy_orders["iron"][0].price_each,
        10
    );

    let restamped = test_observation(
        BotState::default(),
        prayer_runtime::snapshot::WorldObservation {
            market: Arc::new(MarketData {
                station_markets: HashMap::from([(
                    "poi_station_a".to_string(),
                    station_snapshot_at(10, 1_060),
                )]),
                ..MarketData::default()
            }),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
    );

    assert!(!merge_knowledge_state_if_changed(
        &mut knowledge,
        &restamped
    ));
    assert_eq!(knowledge.knowledge_version, 1);
    assert_eq!(
        knowledge.station_markets["poi_station_a"].observed_at_unix,
        Some(1_060)
    );

    let persisted = PersistedWorldKnowledgeV4 {
        knowledge_schema_version: KNOWLEDGE_SCHEMA_VERSION,
        state: knowledge,
    };
    let serialized = serde_json::to_value(&persisted).expect("serialize persisted knowledge");
    assert!(serialized.pointer("/state/station_markets").is_none());
}

#[test]
fn changed_market_depth_bumps_knowledge_version() {
    let mut knowledge = WorldState::default();
    let observation = |price, observed_at_unix| {
        test_observation(
            BotState::default(),
            prayer_runtime::snapshot::WorldObservation {
                market: Arc::new(MarketData {
                    station_markets: HashMap::from([(
                        "poi_station_a".to_string(),
                        station_snapshot_at(price, observed_at_unix),
                    )]),
                    ..MarketData::default()
                }),
                ..prayer_runtime::snapshot::WorldObservation::default()
            },
        )
    };

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &observation(10, 1_000),
    ));
    assert_eq!(knowledge.knowledge_version, 1);

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &observation(11, 1_001),
    ));
    assert_eq!(knowledge.knowledge_version, 2);
}

#[test]
fn passenger_board_knowledge_is_process_local_and_not_persisted() {
    let mut knowledge = WorldState::default();
    let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
    let observation = StateObservation {
        docked_passengers_fetched: true,
        world: prayer_runtime::snapshot::WorldObservation {
            passengers: passenger_board("poi_station_a", "Ada"),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        ..StateObservation::default()
    };

    assert!(!merge_knowledge_state_if_changed_with_metadata(
        &mut knowledge,
        &mut metadata,
        &observation
    ));
    assert_eq!(knowledge.knowledge_version, 0);
    assert_eq!(
        knowledge.station_passengers["poi_station_a"].waiting[0].name,
        "Ada"
    );
    assert!(metadata
        .station_passengers_fetched_at_by_station
        .contains_key("poi_station_a"));

    let persisted = PersistedWorldKnowledgeV4 {
        knowledge_schema_version: KNOWLEDGE_SCHEMA_VERSION,
        state: knowledge,
    };
    let serialized = serde_json::to_value(&persisted).expect("serialize persisted knowledge");
    assert!(serialized.pointer("/state/station_passengers").is_none());
    assert!(serialized
        .pointer("/state/station_passengers_fetched_at_by_station")
        .is_none());
}

#[test]
fn passenger_board_knowledge_uses_current_poi_over_display_station_name() {
    let mut knowledge = WorldState::default();
    let observation = StateObservation {
        docked_passengers_fetched: true,
        status_poi: Some("grand_exchange".to_string()),
        bot: prayer_runtime::snapshot::BotObservation {
            state: BotState {
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: None,
                    poi_id: Some("grand_exchange".to_string()),
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
        },
        world: prayer_runtime::snapshot::WorldObservation {
            passengers: passenger_board("Grand Exchange Station", "Ada"),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        ..StateObservation::default()
    };

    merge_knowledge_state(&mut knowledge, &observation);

    assert!(knowledge.station_passengers.contains_key("grand_exchange"));
    assert!(!knowledge
        .station_passengers
        .contains_key("Grand Exchange Station"));
    let live = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("grand_exchange".to_string()),
            docked_at: None,
            ..Default::default()
        },
        ..BotState::default()
    };
    let composed = execution_state(&knowledge, &live);
    assert_eq!(composed.world.station_passengers.station, "grand_exchange");
    assert_eq!(composed.world.station_passengers.waiting[0].name, "Ada");
}

#[test]
fn salvage_knowledge_is_process_local_and_not_persisted() {
    let mut knowledge = WorldState::default();
    let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
    let lootable = SpaceLootInfo {
        id: "wreck_a".to_string(),
        poi_id: "poi_station_a".to_string(),
        ..SpaceLootInfo::default()
    };
    let observation = StateObservation {
        wrecks_fetched: true,
        world: prayer_runtime::snapshot::WorldObservation {
            salvage: Arc::new(salvage_snapshot(lootable, 1_000)),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        ..StateObservation::default()
    };

    assert!(!merge_knowledge_state_if_changed_with_metadata(
        &mut knowledge,
        &mut metadata,
        &observation
    ));
    assert_eq!(knowledge.knowledge_version, 0);
    assert_eq!(
        knowledge.salvage_by_poi["poi_station_a"].visible_lootables[0].id,
        "wreck_a"
    );
    assert!(metadata
        .wrecks_fetched_at_by_poi
        .contains_key("poi_station_a"));

    let persisted = PersistedWorldKnowledgeV4 {
        knowledge_schema_version: KNOWLEDGE_SCHEMA_VERSION,
        state: knowledge,
    };
    let serialized = serde_json::to_value(&persisted).expect("serialize persisted knowledge");
    assert!(serialized.pointer("/state/salvage_by_poi").is_none());
    assert!(serialized
        .pointer("/state/wrecks_fetched_at_by_poi")
        .is_none());
}

#[test]
fn storage_knowledge_persists_player_station_observations() {
    let mut knowledge = WorldState::default();
    let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
    let observation = StateObservation {
        status_poi: Some("poi_station_a".to_string()),
        bot: prayer_runtime::snapshot::BotObservation {
            state: BotState {
                player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                    username: Some("miner".to_string()),
                    ..Default::default()
                },
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: None,
                    poi_id: Some("poi_station_a".to_string()),
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
        },
        world: prayer_runtime::snapshot::WorldObservation {
            storage: Arc::new(HashMap::from([(
                "poi_station_a".to_string(),
                HashMap::from([("ore".to_string(), 5)]),
            )])),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        docked_storage_fetched: true,
        ..StateObservation::default()
    };

    assert!(merge_knowledge_state_if_changed_with_metadata(
        &mut knowledge,
        &mut metadata,
        &observation
    ));
    assert_eq!(
        knowledge.storage_by_player["miner"]["poi_station_a"]["ore"],
        5
    );
    assert!(metadata
        .storage_fetched_at_by_key
        .contains_key(&player_station_storage_key("miner", "poi_station_a")));

    let empty_snapshot = StateObservation {
        bot: prayer_runtime::snapshot::BotObservation {
            state: BotState {
                player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                    username: Some("miner".to_string()),
                    ..Default::default()
                },
                ..BotState::default()
            },
        },
        world: prayer_runtime::snapshot::WorldObservation {
            storage: Arc::new(HashMap::from([(
                "poi_station_a".to_string(),
                HashMap::new(),
            )])),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        ..StateObservation::default()
    };

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &empty_snapshot
    ));
    assert!(knowledge.storage_by_player["miner"]["poi_station_a"].is_empty());
}

#[test]
fn storage_knowledge_persists_empty_faction_station_observations() {
    let mut knowledge = WorldState::default();
    let observation = StateObservation {
        status_poi: Some("poi_station_a".to_string()),
        bot: prayer_runtime::snapshot::BotObservation {
            state: BotState {
                player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                    faction_id: Some("fac_traders".to_string()),
                    ..Default::default()
                },
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: None,
                    poi_id: Some("poi_station_a".to_string()),
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
        },
        world: prayer_runtime::snapshot::WorldObservation {
            faction_storage: Arc::new(HashMap::from([("ore".to_string(), 5)])),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        docked_faction_storage_fetched: true,
        ..StateObservation::default()
    };

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &observation
    ));
    assert_eq!(
        knowledge.faction_storage_by_faction_poi["fac_traders"]["poi_station_a"]["ore"],
        5
    );

    let empty_snapshot = StateObservation {
        status_poi: Some("poi_station_a".to_string()),
        bot: prayer_runtime::snapshot::BotObservation {
            state: BotState {
                player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                    faction_id: Some("fac_traders".to_string()),
                    ..Default::default()
                },
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: None,
                    poi_id: Some("poi_station_a".to_string()),
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
        },
        world: prayer_runtime::snapshot::WorldObservation {
            faction_storage: Arc::new(HashMap::new()),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        docked_faction_storage_fetched: true,
        ..StateObservation::default()
    };

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &empty_snapshot
    ));
    assert!(knowledge.faction_storage_by_faction_poi["fac_traders"]["poi_station_a"].is_empty());
}

#[test]
fn faction_storage_view_items_include_root_and_bucket_items() {
    let response: spacemolt_lib_rs::schema::StorageResponse =
        serde_json::from_value(serde_json::json!({
            "action": "view_faction_storage",
            "base_id": "station_alpha",
            "credits": 0,
            "faction_id": "fac_traders",
            "faction_name": "Traders",
            "faction_tag": "TRDR",
            "hint": "",
            "recent_activity": [],
            "items": [
                { "item_id": "iron_ore", "name": "Iron Ore", "quantity": 5, "size": 1 },
                { "item_id": "water", "name": "Water", "quantity": 3, "size": 1 }
            ],
            "buckets": [{
                "cap_per_item": 1000,
                "package_cap": 1000,
                "id": "crafting",
                "name": "Crafting",
                "items": [
                    { "item_id": "iron_ore", "name": "Iron Ore", "quantity": 7, "size": 1 },
                    { "item_id": "copper_ore", "name": "Copper Ore", "quantity": 11, "size": 1 }
                ]
            }]
        }))
        .expect("typed faction storage response");
    let items = super::faction_storage_items_from_view_response(&response);

    assert_eq!(items["iron_ore"], 12);
    assert_eq!(items["water"], 3);
    assert_eq!(items["copper_ore"], 11);
}

#[test]
fn faction_storage_refresh_uses_docked_station_id_and_poi_watcher_key() {
    let service = RuntimeService::default();
    let id = Uuid::new_v4();
    let state = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac_traders".to_string()),
            ..Default::default()
        },
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            poi_id: Some("poi_station_a".to_string()),
            docked_at: Some("base_station_a".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    service
        .faction_storage_watchers_by_key
        .lock()
        .insert("fac_traders@poi_station_a".to_string(), id);

    assert_eq!(
        service.faction_storage_refresh_target(id, &state),
        Some(("fac_traders".to_string(), "base_station_a".to_string()))
    );
}

#[test]
fn storage_knowledge_does_not_stamp_unfetched_faction_storage_onto_current_poi() {
    let mut knowledge = WorldState::default();
    let real_station_observation = StateObservation {
        status_poi: Some("unknown_edge_waystation".to_string()),
        bot: prayer_runtime::snapshot::BotObservation {
            state: BotState {
                player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                    faction_id: Some("fac_traders".to_string()),
                    ..Default::default()
                },
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: None,
                    poi_id: Some("unknown_edge_waystation".to_string()),
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
        },
        world: prayer_runtime::snapshot::WorldObservation {
            faction_storage: Arc::new(HashMap::from([("ore".to_string(), 5)])),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        docked_faction_storage_fetched: true,
        ..StateObservation::default()
    };

    assert!(merge_knowledge_state_if_changed(
        &mut knowledge,
        &real_station_observation
    ));

    let stale_projection_at_other_poi = StateObservation {
        status_poi: Some("alpha_extraction_zone".to_string()),
        bot: prayer_runtime::snapshot::BotObservation {
            state: BotState {
                player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                    faction_id: Some("fac_traders".to_string()),
                    ..Default::default()
                },
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: None,
                    poi_id: Some("alpha_extraction_zone".to_string()),
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
        },
        world: prayer_runtime::snapshot::WorldObservation {
            faction_storage: Arc::new(HashMap::from([("ore".to_string(), 5)])),
            ..prayer_runtime::snapshot::WorldObservation::default()
        },
        docked_faction_storage_fetched: false,
        ..StateObservation::default()
    };

    merge_knowledge_state_if_changed(&mut knowledge, &stale_projection_at_other_poi);
    assert!(knowledge.faction_storage_by_faction_poi["fac_traders"]
        .contains_key("unknown_edge_waystation"));
    assert!(!knowledge.faction_storage_by_faction_poi["fac_traders"]
        .contains_key("alpha_extraction_zone"));
}

#[tokio::test]
async fn commander_storage_snapshot_seeds_connected_account_id_for_personal_storage() {
    let service = RuntimeService::new();
    let id = service
        .create_session_with_label(Some("Scout".to_string()))
        .expect("session");
    let account = seeded_test_account_with_id(
        Some("player_1".to_string()),
        serde_json::json!({
            "player": { "username": "Scout", "credits": 44 },
            "location": {
                "system_id": "sol",
                "poi_id": "earth_station",
                "docked_at": "earth_station"
            }
        }),
    )
    .await;

    let created = service
        .install_connected_owned_spacemolt_accounts(
            vec![account],
            "https://game.spacemolt.com".to_string(),
        )
        .await
        .expect("install accounts");
    assert_eq!(created, 0);

    {
        let mut knowledge = service.knowledge_state.write();
        knowledge.storage_by_player = HashMap::from([(
            "player_1".to_string(),
            HashMap::from([(
                "poi_station_a".to_string(),
                HashMap::from([("ore".to_string(), 5)]),
            )]),
        )]);
    }

    let session = service.get_session(id).await.expect("session");
    let session = session.lock().await;
    assert_eq!(
        session.actor.observed.player.username.as_deref(),
        Some("Scout")
    );
    assert_eq!(session.actor.observed.player.id, None);
    drop(session);

    let snapshot = service
        .commander_storage_snapshot()
        .await
        .expect("storage snapshot");
    let row = snapshot
        .rows
        .iter()
        .find(|row| row.source_kind == "personal" && row.item_id == "ore")
        .expect("personal ore row");

    assert_eq!(row.quantity, 5);
    assert_eq!(row.owner_id.as_deref(), Some("player_1"));
    assert_eq!(row.owner_name, "Scout");
}

#[test]
fn execution_state_overlays_matching_player_storage_only() {
    let knowledge = WorldState {
        storage_by_player: HashMap::from([
            (
                "miner".to_string(),
                HashMap::from([(
                    "poi_station_a".to_string(),
                    HashMap::from([("ore".to_string(), 5)]),
                )]),
            ),
            (
                "other".to_string(),
                HashMap::from([(
                    "poi_station_b".to_string(),
                    HashMap::from([("ore".to_string(), 99)]),
                )]),
            ),
        ]),
        faction_storage_by_faction_poi: HashMap::from([(
            "miners_guild".to_string(),
            HashMap::from([(
                "poi_station_c".to_string(),
                HashMap::from([("fuel".to_string(), 7)]),
            )]),
        )]),
        ..WorldState::default()
    };
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            username: Some("miner".to_string()),
            faction_id: Some("miners_guild".to_string()),
            ..Default::default()
        },
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_c".to_string()),
            docked_at: None,
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    assert_eq!(composed.world.storage["poi_station_a"]["ore"], 5);
    assert!(!composed.world.storage.contains_key("poi_station_c"));
    assert!(!composed.world.storage.contains_key("poi_station_b"));
    assert_eq!(
        composed
            .world
            .faction_storage
            .as_ref()
            .expect("known faction storage")["fuel"],
        7
    );
}

#[test]
fn execution_state_distinguishes_missing_from_empty_faction_storage() {
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("miners_guild".to_string()),
            ..Default::default()
        },
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            poi_id: Some("poi_station_c".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let missing = execution_state(&WorldState::default(), &live);
    assert!(missing.world.faction_storage.is_none());

    let knowledge = WorldState {
        faction_storage_by_faction_poi: HashMap::from([(
            "miners_guild".to_string(),
            HashMap::from([("poi_station_c".to_string(), HashMap::new())]),
        )]),
        ..WorldState::default()
    };
    let empty = execution_state(&knowledge, &live);
    assert!(empty
        .world
        .faction_storage
        .as_ref()
        .is_some_and(|storage| storage.is_empty()));
}

#[test]
fn execution_state_checks_all_player_storage_identities() {
    let knowledge = WorldState {
        storage_by_player: HashMap::from([(
            "Scout".to_string(),
            HashMap::from([(
                "poi_station_a".to_string(),
                HashMap::from([("ore".to_string(), 5)]),
            )]),
        )]),
        ..WorldState::default()
    };
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            id: Some("player_1".to_string()),
            username: Some("Scout".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    assert_eq!(composed.world.storage["poi_station_a"]["ore"], 5);
}

#[test]
fn execution_state_overlays_shared_station_markets() {
    let knowledge = WorldState {
        station_markets: HashMap::from([
            ("poi_station_a".to_string(), station_snapshot_at(10, 1_000)),
            ("poi_station_b".to_string(), station_snapshot_at(20, 2_000)),
            ("stale_station".to_string(), old_station_snapshot(99)),
        ]),
        ..WorldState::default()
    };
    let live = BotState::default();

    let composed = execution_state(&knowledge, &live);

    let markets = &composed.world.market.station_markets;
    assert_eq!(
        markets["poi_station_a"].buy_orders["iron"][0].price_each,
        10
    );
    assert_eq!(
        markets["poi_station_b"].buy_orders["iron"][0].price_each,
        20
    );
    assert_eq!(
        markets["stale_station"].buy_orders["iron"][0].price_each,
        99
    );
}

#[test]
fn execution_state_overlays_current_station_passengers() {
    let knowledge = WorldState {
        station_passengers: HashMap::from([
            (
                "poi_station_a".to_string(),
                passenger_board("poi_station_a", "Ada"),
            ),
            (
                "poi_station_b".to_string(),
                passenger_board("poi_station_b", "Grace"),
            ),
        ]),
        ..WorldState::default()
    };
    let live = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_a".to_string()),
            docked_at: None,
            ..Default::default()
        },
        passengers: prayer_state::ActorPassengerState {
            aboard_count: Some(1),
            aboard: Arc::new(vec![spacemolt_lib_rs::schema::PassengerView {
                base_fare: 0,
                berth_class: None,
                bio: String::new(),
                citizen_id: "already-aboard".to_string(),
                class: "economy".to_string(),
                connecting: None,
                destination: String::new(),
                destination_name: String::new(),
                destination_system: None,
                name: "Already Aboard".to_string(),
                speed_bonus: None,
                ticks_remaining: 0,
            }]),
            ..prayer_state::ActorPassengerState::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    assert_eq!(composed.bot.passengers.aboard_count, Some(1));
    assert_eq!(composed.bot.passengers.aboard[0].name, "Already Aboard");
    assert_eq!(composed.world.station_passengers.station, "poi_station_a");
    assert_eq!(composed.world.station_passengers.waiting_count, Some(1));
    assert_eq!(composed.world.station_passengers.waiting[0].name, "Ada");
}

#[test]
fn execution_state_overlays_virtual_orders_into_station_books() {
    let knowledge = virtual_order_knowledge(vec![
        virtual_order("vf-sell", "sell", "iron", "station_sol", 7, 10),
        virtual_order("vf-buy", "buy", "iron", "station_vega", 25, 12),
    ]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    let sol = &composed.world.market.station_markets["station_sol"];
    assert_eq!(sol.sell_orders["iron"][0].quantity, 10);
    assert_eq!(
        sol.sell_orders["iron"][0].source.as_deref(),
        Some("virtual_faction:vf-sell")
    );
    let vega = &composed.world.market.station_markets["station_vega"];
    assert_eq!(vega.buy_orders["iron"][0].quantity, 12);
    assert_eq!(
        vega.buy_orders["iron"][0].source.as_deref(),
        Some("virtual_faction:vf-buy")
    );
}

#[test]
fn execution_state_ignores_unavailable_virtual_orders() {
    let mut disabled = virtual_order("disabled", "sell", "iron", "station_sol", 7, 10);
    disabled.enabled = false;
    let mut filled = virtual_order("filled", "sell", "iron", "station_sol", 7, 10);
    filled.filled = 10;
    let mut reserved = virtual_order("reserved", "buy", "iron", "station_vega", 25, 10);
    reserved.reserved = 10;
    let knowledge = virtual_order_knowledge(vec![disabled, filled, reserved]);

    let composed = execution_state(&knowledge, &BotState::default());

    assert!(composed.world.market.station_markets.is_empty());
}

#[test]
fn virtual_craft_orders_remain_user_authored_and_reservable() {
    let service = RuntimeService::default();
    service.replace_virtual_craft_orders(vec![RuntimeVirtualCraftOrderDto {
        id: "manual-craft-order".to_string(),
        status: "available".to_string(),
        action: "craft".to_string(),
        recipe_id: "smelt_plate".to_string(),
        item_id: "plate".to_string(),
        station_id: "station_sol".to_string(),
        quantity: 5,
        reserved: 0,
        reservation_id: None,
        filled: 0,
        enabled: true,
        priority: 1.0,
        facility_id: None,
        preset: None,
        squad_id: None,
        session_handles: Vec::new(),
        credit_floor: None,
        do_forever: false,
    }]);

    let reserved = service.reserve_virtual_craft_orders(vec![RuntimeVirtualOrderUseDto {
        order_id: "manual-craft-order".to_string(),
        quantity: 2,
    }]);
    assert_eq!((reserved[0].reserved, reserved[0].filled), (2, 0));

    let filled = service.fill_virtual_craft_order("manual-craft-order");
    assert_eq!((filled[0].reserved, filled[0].filled), (0, 2));
}

#[test]
fn execution_state_caps_virtual_sell_by_known_faction_storage() {
    let mut knowledge = virtual_order_knowledge(vec![virtual_order(
        "vf-sell",
        "sell",
        "iron",
        "station_sol",
        7,
        10,
    )]);
    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 4)]),
        )]),
    )]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    assert_eq!(
        composed.world.market.station_markets["station_sol"].sell_orders["iron"][0].quantity,
        4
    );
}

#[test]
fn execution_state_buy_until_exposes_shortfall() {
    let mut order = virtual_order("vf-buy-until", "buy_until", "iron", "station_sol", 25, 10);
    order.reserved = 2;
    let mut knowledge = virtual_order_knowledge(vec![order]);
    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 4)]),
        )]),
    )]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    let order = &composed.world.market.station_markets["station_sol"].buy_orders["iron"][0];
    assert_eq!(order.quantity, 4);
    assert_eq!(
        order.source.as_deref(),
        Some("virtual_faction:vf-buy-until")
    );
}

#[test]
fn execution_state_sell_until_exposes_excess() {
    let mut order = virtual_order("vf-sell-until", "sell_until", "iron", "station_sol", 7, 10);
    order.reserved = 3;
    let mut knowledge = virtual_order_knowledge(vec![order]);
    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 25)]),
        )]),
    )]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    let order = &composed.world.market.station_markets["station_sol"].sell_orders["iron"][0];
    assert_eq!(order.quantity, 12);
    assert_eq!(
        order.source.as_deref(),
        Some("virtual_faction:vf-sell-until")
    );
}

#[test]
fn execution_state_sell_until_tipping_point_waits_for_excess() {
    let mut order = virtual_order("vf-sell-until", "sell_until", "iron", "station_sol", 7, 10);
    order.tipping_point = Some(20);
    let mut knowledge = virtual_order_knowledge(vec![order]);
    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 25)]),
        )]),
    )]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    assert!(!composed
        .world
        .market
        .station_markets
        .contains_key("station_sol"));
}

#[test]
fn execution_state_sell_until_tipping_point_keeps_dumping_below_tip() {
    let mut order = virtual_order("vf-sell-until", "sell_until", "iron", "station_sol", 7, 10);
    order.tipping_point = Some(20);
    order.dumping = true;
    let mut knowledge = virtual_order_knowledge(vec![order]);
    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 25)]),
        )]),
    )]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    let order = &composed.world.market.station_markets["station_sol"].sell_orders["iron"][0];
    assert_eq!(order.quantity, 15);
    assert_eq!(
        order.source.as_deref(),
        Some("virtual_faction:vf-sell-until")
    );
}

#[test]
fn execution_state_sell_until_tipping_point_exposes_full_excess() {
    let mut order = virtual_order("vf-sell-until", "sell_until", "iron", "station_sol", 7, 10);
    order.tipping_point = Some(20);
    let mut knowledge = virtual_order_knowledge(vec![order]);
    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 30)]),
        )]),
    )]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    let order = &composed.world.market.station_markets["station_sol"].sell_orders["iron"][0];
    assert_eq!(order.quantity, 20);
    assert_eq!(
        order.source.as_deref(),
        Some("virtual_faction:vf-sell-until")
    );
}

#[test]
fn sell_until_tipping_point_dump_mode_reconciles_from_storage() {
    let mut order = virtual_order("vf-sell-until", "sell_until", "iron", "station_sol", 7, 10);
    order.tipping_point = Some(20);
    let mut knowledge = virtual_order_knowledge(vec![order]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 30)]),
        )]),
    )]);
    assert!(super::reconcile_virtual_order_dump_modes(
        &mut knowledge,
        &live
    ));
    assert!(knowledge.virtual_orders[0].dumping);

    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 25)]),
        )]),
    )]);
    assert!(!super::reconcile_virtual_order_dump_modes(
        &mut knowledge,
        &live
    ));
    assert!(knowledge.virtual_orders[0].dumping);

    knowledge.faction_storage_by_faction_poi = HashMap::from([(
        "fac".to_string(),
        HashMap::from([(
            "station_sol".to_string(),
            HashMap::from([("iron".to_string(), 10)]),
        )]),
    )]);
    assert!(super::reconcile_virtual_order_dump_modes(
        &mut knowledge,
        &live
    ));
    assert!(!knowledge.virtual_orders[0].dumping);
}

#[test]
fn filling_until_orders_releases_without_closing_target() {
    let service = RuntimeService::default();
    service.replace_virtual_orders(vec![virtual_order(
        "vf-buy-until",
        "buy_until",
        "iron",
        "station_sol",
        25,
        10,
    )]);
    service.reserve_virtual_orders_detailed(vec![RuntimeVirtualOrderUseDto {
        order_id: "vf-buy-until".to_string(),
        quantity: 4,
    }]);

    let orders = service.fill_virtual_order("vf-buy-until");

    assert_eq!(orders[0].reserved, 0);
    assert_eq!(orders[0].filled, 0);
}

#[test]
fn filling_fixed_virtual_order_deletes_when_settled() {
    let service = RuntimeService::default();
    service.replace_virtual_orders(vec![virtual_order(
        "vf-buy",
        "buy",
        "iron",
        "station_sol",
        25,
        4,
    )]);
    service.reserve_virtual_orders_detailed(vec![RuntimeVirtualOrderUseDto {
        order_id: "vf-buy".to_string(),
        quantity: 4,
    }]);

    let orders = service.fill_virtual_order("vf-buy");

    assert!(orders.is_empty());
    assert!(service.virtual_orders().is_empty());
}

#[test]
fn fully_reserved_fixed_virtual_order_is_not_deleted_before_fill() {
    let service = RuntimeService::default();
    service.replace_virtual_orders(vec![virtual_order(
        "vf-buy",
        "buy",
        "iron",
        "station_sol",
        25,
        4,
    )]);

    let (orders, _reservation_results) =
        service.reserve_virtual_orders_detailed(vec![RuntimeVirtualOrderUseDto {
            order_id: "vf-buy".to_string(),
            quantity: 4,
        }]);

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].reserved, 4);
    assert_eq!(orders[0].filled, 0);
}

#[test]
fn detailed_virtual_order_reservation_reports_short_second_claim() {
    let service = RuntimeService::default();
    service.replace_virtual_orders(vec![virtual_order(
        "vf-buy",
        "buy",
        "iron",
        "station_sol",
        25,
        4,
    )]);
    let (_orders, first_results) =
        service.reserve_virtual_orders_detailed(vec![RuntimeVirtualOrderUseDto {
            order_id: "vf-buy".to_string(),
            quantity: 4,
        }]);
    assert_eq!(first_results[0].requested, 4);
    assert_eq!(first_results[0].accepted, 4);

    let (orders, second_results) =
        service.reserve_virtual_orders_detailed(vec![RuntimeVirtualOrderUseDto {
            order_id: "vf-buy".to_string(),
            quantity: 4,
        }]);

    assert_eq!(orders[0].reserved, 4);
    assert_eq!(second_results[0].requested, 4);
    assert_eq!(second_results[0].accepted, 0);
    assert_eq!(second_results[0].reserved_before, 4);
    assert_eq!(second_results[0].reserved_after, 4);
}

#[test]
fn concurrent_virtual_order_reservations_have_one_winner_and_stable_identity() {
    let service = Arc::new(RuntimeService::default());
    service.replace_virtual_orders(vec![virtual_order(
        "vf-buy",
        "buy",
        "iron",
        "station_sol",
        25,
        4,
    )]);
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let service = Arc::clone(&service);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            service
                .reserve_virtual_orders_detailed(vec![RuntimeVirtualOrderUseDto {
                    order_id: "vf-buy".to_string(),
                    quantity: 4,
                }])
                .1
                .remove(0)
        }));
    }
    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("reservation thread"))
        .collect::<Vec<_>>();

    assert_eq!(
        results.iter().filter(|result| result.accepted == 4).count(),
        1
    );
    assert_eq!(
        results.iter().filter(|result| result.accepted == 0).count(),
        1
    );
    let order = &service.virtual_orders()[0];
    assert_eq!(order.reserved, 4);
    assert!(order.reservation_id.is_some());
    assert_eq!(
        results
            .iter()
            .find(|result| result.accepted == 4)
            .and_then(|result| result.reservation_id.as_ref()),
        order.reservation_id.as_ref()
    );
}

#[test]
fn virtual_market_overlay_preserves_endpoint_metadata() {
    let knowledge = virtual_order_knowledge(vec![
        virtual_order("vf-sell", "sell", "iron", "station_sol", 7, 10),
        virtual_order("vf-buy", "buy", "iron", "station_vega", 25, 12),
    ]);
    let live = BotState {
        player: spacemolt_lib_rs::schema::V2GameStatePlayer {
            faction_id: Some("fac".to_string()),
            credits: Some(10_000),
            ..Default::default()
        },
        cargo_capacity: 100,
        ..BotState::default()
    };
    let composed = execution_state(&knowledge, &live);

    let sell = &composed.world.market.station_markets["station_sol"].sell_orders["iron"][0];
    let buy = &composed.world.market.station_markets["station_vega"].buy_orders["iron"][0];
    assert_eq!(sell.source.as_deref(), Some("virtual_faction:vf-sell"));
    assert_eq!(buy.source.as_deref(), Some("virtual_faction:vf-buy"));
    assert_eq!(sell.quantity, 10);
    assert_eq!(buy.quantity, 12);
}

#[test]
fn reservation_reduces_visible_virtual_market_depth() {
    let mut order = virtual_order("vf-buy", "buy", "iron", "station_vega", 25, 12);
    order.reserved = 5;
    let knowledge = virtual_order_knowledge(vec![order]);

    let composed = execution_state(&knowledge, &BotState::default());

    assert_eq!(
        composed.world.market.station_markets["station_vega"].buy_orders["iron"][0].quantity,
        7
    );
}

#[test]
fn execution_state_rebuilds_market_and_salvage_from_knowledge() {
    let lootable = SpaceLootInfo {
        id: "wreck_a".to_string(),
        poi_id: "poi_station_a".to_string(),
        ..SpaceLootInfo::default()
    };
    let knowledge = WorldState {
        shipyard_listing_ids: vec!["ship_listing_a".to_string()],
        station_markets: HashMap::from([(
            "poi_station_a".to_string(),
            station_snapshot_at(10, 1_000),
        )]),
        salvage_by_poi: HashMap::from([(
            "poi_station_a".to_string(),
            salvage_snapshot(lootable.clone(), 1_000),
        )]),
        ..WorldState::default()
    };
    let metadata = prayer_runtime::knowledge::WorldRuntimeMetadata {
        wrecks_fetched_at_by_poi: HashMap::from([("poi_station_a".to_string(), Instant::now())]),
        ..Default::default()
    };
    let live = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_a".to_string()),
            docked_at: (true).then(|| "docked".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state_with_metadata(&knowledge, &metadata, &live);

    assert_eq!(
        composed.world.market.shipyard_listings,
        vec!["ship_listing_a".to_string()]
    );
    assert!(!composed
        .world
        .market
        .station_markets
        .contains_key("session_station"));
    assert_eq!(composed.world.market.buy_orders["iron"][0].price_each, 10);
    assert_eq!(composed.world.salvage.visible_lootables, vec![lootable]);
}

#[test]
fn execution_state_drops_stale_salvage_after_ttl() {
    let lootable = SpaceLootInfo {
        id: "wreck_a".to_string(),
        poi_id: "poi_station_a".to_string(),
        ..SpaceLootInfo::default()
    };
    let knowledge = WorldState {
        salvage_by_poi: HashMap::from([(
            "poi_station_a".to_string(),
            salvage_snapshot(lootable, 1_000),
        )]),
        ..WorldState::default()
    };
    let metadata = prayer_runtime::knowledge::WorldRuntimeMetadata {
        wrecks_fetched_at_by_poi: HashMap::from([(
            "poi_station_a".to_string(),
            Instant::now() - WRECKS_REFRESH_TTL - Duration::from_secs(1),
        )]),
        ..Default::default()
    };
    let live = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_a".to_string()),
            docked_at: None,
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state_with_metadata(&knowledge, &metadata, &live);

    assert!(composed.world.salvage.visible_lootables.is_empty());
    assert!(composed.world.salvage.lootables_by_poi.is_empty());
    assert!(composed.world.salvage.last_seen_poi.is_none());
    assert!(composed.world.salvage.observed_at_unix.is_none());
}

#[test]
fn execution_state_uses_remembered_current_station_books() {
    let knowledge = WorldState {
        station_markets: HashMap::from([("poi_station_a".to_string(), old_station_snapshot(99))]),
        ..WorldState::default()
    };
    let live = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_a".to_string()),
            docked_at: (true).then(|| "docked".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    let composed = execution_state(&knowledge, &live);

    assert_eq!(
        composed.world.market.station_markets["poi_station_a"].buy_orders["iron"][0].price_each,
        99
    );
    assert_eq!(composed.world.market.buy_orders["iron"][0].price_each, 99);
}

#[test]
fn bot_state_cannot_retain_shared_market_and_salvage() {
    let mut session = SessionHandle::new("test".to_string());
    let knowledge = WorldState {
        shipyard_listing_ids: vec!["ship_listing_a".to_string()],
        station_markets: HashMap::from([(
            "poi_station_a".to_string(),
            station_snapshot_at(10, 1_000),
        )]),
        salvage_by_poi: HashMap::from([(
            "poi_station_a".to_string(),
            salvage_snapshot(
                SpaceLootInfo {
                    id: "wreck_a".to_string(),
                    poi_id: "poi_station_a".to_string(),
                    ..SpaceLootInfo::default()
                },
                1_000,
            ),
        )]),
        ..WorldState::default()
    };
    let fetched = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_a".to_string()),
            docked_at: (true).then(|| "docked".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };

    apply_live_state(&mut session, fetched, &knowledge);

    let retained = serde_json::to_value(&session.actor.observed).expect("serialize actor");
    for forbidden in [
        "market",
        "salvage",
        "galaxy",
        "storage",
        "faction_storage",
        "faction_garage",
        "wildlife_by_poi",
        "system_agents",
        "managed_players",
    ] {
        assert!(
            retained.get(forbidden).is_none(),
            "actor retained {forbidden}"
        );
    }
    let metadata = prayer_runtime::knowledge::WorldRuntimeMetadata {
        wrecks_fetched_at_by_poi: HashMap::from([("poi_station_a".to_string(), Instant::now())]),
        ..Default::default()
    };
    let projected =
        execution_state_with_metadata(&knowledge, &metadata, session.actor.observed.as_ref());
    assert_eq!(projected.world.market.buy_orders["iron"][0].price_each, 10);
    assert_eq!(projected.world.salvage.visible_lootables[0].id, "wreck_a");
}

#[test]
fn actor_snapshot_is_at_least_half_smaller_than_shared_world_input() {
    let station_markets = (0..100)
        .map(|index| {
            (
                format!("station-{index}"),
                station_snapshot_at(index + 1, 1_000 + index),
            )
        })
        .collect::<HashMap<_, _>>();
    let world = WorldState {
        station_markets,
        wildlife_by_poi: (0..100)
            .map(|index| {
                (
                    format!("poi-{index}"),
                    prayer_state::WildlifePoiSnapshotData::default(),
                )
            })
            .collect(),
        managed_players: (0..100).map(|index| format!("bot-{index}")).collect(),
        ..WorldState::default()
    };
    let input_bytes = serde_json::to_vec(&world).expect("input json").len();
    let mut session = SessionHandle::new("memory-gate".to_string());
    apply_live_state(&mut session, BotState::default(), &world);
    let actor_bytes = serde_json::to_vec(&session.actor.observed)
        .expect("actor json")
        .len();
    assert!(
        actor_bytes.saturating_mul(2) <= input_bytes,
        "actor={actor_bytes} input={input_bytes}"
    );
}

#[test]
#[ignore = "memory scaling benchmark; run explicitly with --ignored --nocapture"]
fn benchmark_shared_world_at_1_10_50_100_sessions() {
    let knowledge = WorldState {
        station_markets: (0..500)
            .map(|index| {
                (
                    format!("station-{index}"),
                    station_snapshot_at(index + 1, 10_000 + index),
                )
            })
            .collect(),
        ..WorldState::default()
    };
    let shared_bytes = serde_json::to_vec(&knowledge.station_markets)
        .expect("knowledge json")
        .len();

    for count in [1usize, 10, 50, 100] {
        let started = Instant::now();
        let mut sessions = Vec::with_capacity(count);
        for index in 0..count {
            let mut session = SessionHandle::new(format!("bot-{index}"));
            apply_live_state(
                &mut session,
                BotState {
                    player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                        id: Some(format!("player-{index}")),
                        username: Some(format!("bot-{index}")),
                        ..Default::default()
                    },
                    location: spacemolt_lib_rs::schema::V2GameStateLocation {
                        system_id: Some("system-0".to_string()),
                        poi_id: Some(format!("station-{}", index % 500)),
                        docked_at: (true).then(|| "docked".to_string()),
                        ..Default::default()
                    },
                    ..BotState::default()
                },
                &knowledge,
            );
            sessions.push(session);
        }
        let actor_bytes = sessions
            .iter()
            .map(|session| {
                serde_json::to_vec(&session.actor.observed)
                    .expect("actor json")
                    .len()
            })
            .sum::<usize>();
        let projection_started = Instant::now();
        for session in &sessions {
            let world = world_read_state(&knowledge, &session.actor.observed);
            let projected = map_commander_session_state(&session.actor.observed, &world)
                .expect("commander projection");
            std::hint::black_box(projected);
        }
        eprintln!(
            "sessions={count} shared_bytes={shared_bytes} actor_bytes={actor_bytes} build_ms={} projection_ms={}",
            started.elapsed().as_millis(),
            projection_started.elapsed().as_millis(),
        );
    }
}

#[test]
fn apply_live_state_does_not_retain_shared_storage() {
    let mut session = SessionHandle::new("test".to_string());
    let knowledge = WorldState::default();

    // First fetch: docked at station A with ore in storage.
    let docked_at_a = BotState::default();
    apply_live_state(&mut session, docked_at_a, &knowledge);

    assert!(serde_json::to_value(&session.actor.observed)
        .unwrap()
        .get("storage")
        .is_none());

    // Later observations remain in shared knowledge, not on the actor.
    apply_live_state(&mut session, BotState::default(), &knowledge);
    assert!(serde_json::to_value(&session.actor.observed)
        .unwrap()
        .get("storage")
        .is_none());

    // Third fetch: station A observed again, now empty — overwritten.
    let emptied_at_a = BotState::default();
    apply_live_state(&mut session, emptied_at_a, &knowledge);
    assert!(serde_json::to_value(&session.actor.observed)
        .unwrap()
        .get("storage")
        .is_none());
}

#[test]
fn partial_side_fetch_does_not_clear_core_status() {
    let mut session = SessionHandle::new("test".to_string());
    let knowledge = WorldState::default();

    apply_live_state(
        &mut session,
        BotState {
            location: spacemolt_lib_rs::schema::V2GameStateLocation {
                system_id: Some("dheneb".to_string()),
                poi_id: Some("station_a".to_string()),
                docked_at: (true).then(|| "docked".to_string()),
                ..Default::default()
            },
            fuel_pct: 95,
            fuel: 95,
            max_fuel: 100,
            cargo_pct: 10,
            cargo_used: 7,
            cargo_capacity: 70,
            cargo: Arc::new(HashMap::from([("ore".to_string(), 7)])),
            player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                username: Some("Pike Market Bot 25".to_string()),
                id: Some("player_25".to_string()),
                credits: Some(42_000),
                ..Default::default()
            },
            ship: prayer_runtime::engine::ShipState {
                id: Some("ship_25".to_string()),
                class_id: Some("hauler".to_string()),
                ..prayer_runtime::engine::ShipState::default()
            },
            installed_modules: Arc::new(vec!["cargo_rack".to_string()]),
            ..BotState::default()
        },
        &knowledge,
    );

    apply_live_state_inner(
        &mut session,
        prayer_state::BotState {
            passengers: prayer_state::ActorPassengerState::from(&passenger_board(
                "station_a",
                "Ada",
            )),
            ..prayer_state::BotState::default()
        },
        &knowledge,
        false,
        false,
        false,
        true,
        true,
    );

    assert_eq!(
        session.actor.observed.location.system_id.as_deref(),
        Some("dheneb")
    );
    assert_eq!(
        session.actor.observed.location.poi_id.as_deref(),
        Some("station_a")
    );
    assert!(session.actor.observed.location.docked_at.is_some());
    assert_eq!(session.actor.observed.fuel, 95);
    assert_eq!(session.actor.observed.max_fuel, 100);
    assert_eq!(session.actor.observed.cargo_used, 7);
    assert_eq!(session.actor.observed.cargo_capacity, 70);
    assert_eq!(session.actor.observed.cargo["ore"], 7);
    assert_eq!(session.actor.observed.player.credits, Some(42_000));
    assert_eq!(session.actor.observed.ship.id.as_deref(), Some("ship_25"));
    assert_eq!(
        session.actor.observed.installed_modules.as_ref(),
        &vec!["cargo_rack".to_string()]
    );
    assert_eq!(
        session.actor.observed.location.system_id.as_deref(),
        Some("dheneb")
    );
}

#[test]
fn apply_live_state_carries_station_mission_board_forward() {
    let mut session = SessionHandle::new("test".to_string());
    let knowledge = WorldState::default();

    let docked_with_board = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_a".to_string()),
            docked_at: (true).then(|| "docked".to_string()),
            ..Default::default()
        },
        missions: Arc::new(MissionData {
            available: vec!["mission_1".to_string()],
            available_details: vec![serde_json::from_value(serde_json::json!({
                "mission_id": "mission_1",
                "title": "First Haul",
                "description": "",
                "difficulty": 1,
                "expires_in_ticks": 100,
                "type": "delivery",
                "rewards": { "credits": 0 }
            }))
            .expect("mission fixture")],
            ..MissionData::default()
        }),
        ..BotState::default()
    };
    apply_live_state(&mut session, docked_with_board, &knowledge);

    let status_only_same_station = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_a".to_string()),
            docked_at: (true).then(|| "docked".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };
    apply_live_state(&mut session, status_only_same_station, &knowledge);

    assert_eq!(
        session.actor.observed.missions.available,
        vec!["mission_1".to_string()]
    );
    assert_eq!(
        session.actor.observed.missions.available_details[0].title,
        "First Haul"
    );
    assert_eq!(session.actor.observed.missions.available.len(), 1);

    let status_only_other_station = BotState {
        location: spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: None,
            poi_id: Some("poi_station_b".to_string()),
            docked_at: (true).then(|| "docked".to_string()),
            ..Default::default()
        },
        ..BotState::default()
    };
    apply_live_state(&mut session, status_only_other_station, &knowledge);

    assert!(session.actor.observed.missions.available.is_empty());
    assert!(session.actor.observed.missions.available_details.is_empty());
}

#[test]
fn apply_live_state_carries_owned_ship_details_forward() {
    let mut session = SessionHandle::new("test".to_string());
    let knowledge = WorldState::default();
    let fetched_with_ships = BotState {
        owned_ship_details: Arc::new(vec![
            spacemolt_lib_rs::schema::OwnedShipInfo {
                cargo_used: None,
                ship_id: "ship_1".to_string(),
                class_id: "viper".to_string(),
                class_name: None,
                custom_name: None,
                fuel: None,
                hull: None,
                listing_base_id: None,
                listing_id: None,
                listing_price: None,
                location: Some("Active ship".to_string()),
                location_base_id: None,
                modules: None,
                is_active: true,
            },
            spacemolt_lib_rs::schema::OwnedShipInfo {
                cargo_used: None,
                ship_id: "ship_2".to_string(),
                class_id: "prayer".to_string(),
                class_name: None,
                custom_name: None,
                fuel: None,
                hull: None,
                listing_base_id: None,
                listing_id: None,
                listing_price: None,
                location: Some("Stored at Station".to_string()),
                location_base_id: None,
                modules: None,
                is_active: false,
            },
        ]),
        ..BotState::default()
    };
    apply_live_state(&mut session, fetched_with_ships, &knowledge);

    // Later status-only refreshes skip `list_ships`; keep the fleet view
    // instead of replacing it with the default empty list, but use
    // get_status' active ship id as canonical.
    let status_only = BotState {
        ship: prayer_runtime::engine::ShipState {
            id: Some("ship_2".to_string()),
            ..prayer_runtime::engine::ShipState::default()
        },
        ..BotState::default()
    };
    apply_live_state(&mut session, status_only, &knowledge);

    assert_eq!(
        session.actor.observed.owned_ship_ids().collect::<Vec<_>>(),
        vec!["ship_2", "ship_1"]
    );
    assert_eq!(
        session.actor.observed.owned_ship_details[0].class_id,
        "prayer"
    );
    assert!(session.actor.observed.owned_ship_details[0].is_active);
    assert!(!session.actor.observed.owned_ship_details[1].is_active);
    assert_eq!(session.actor.observed.owned_ship_details.len(), 2);
}

#[test]
fn owned_ships_refresh_initially_and_when_dock_changes() {
    assert!(should_refresh_owned_ships(false, false, None, None));
    assert!(should_refresh_owned_ships(true, false, None, None));
    assert!(should_refresh_owned_ships(true, true, None, Some("base_a")));
    assert!(should_refresh_owned_ships(
        true,
        true,
        Some("base_a"),
        Some("base_b")
    ));
    assert!(!should_refresh_owned_ships(
        true,
        true,
        Some("base_a"),
        Some("base_a")
    ));
    assert!(!should_refresh_owned_ships(
        true,
        true,
        Some("base_a"),
        None
    ));
}

#[tokio::test]
async fn state_snapshot_omits_shared_market_memory() {
    let service = RuntimeService::default();
    let id = service.create_session();

    {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        apply_live_state(
            &mut session,
            BotState {
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: Some("sol".to_string()),
                    poi_id: Some("poi_station_a".to_string()),
                    docked_at: (true).then(|| "docked".to_string()),
                    ..Default::default()
                },
                ..BotState::default()
            },
            &WorldState::default(),
        );
        session.touch_state();
    }

    {
        let mut knowledge = service.knowledge_state.write();
        knowledge
            .station_markets
            .insert("poi_station_a".to_string(), station_snapshot_at(10, 2_000));
    }

    let response = service
        .commander_state_snapshot()
        .await
        .expect("commander state snapshot");
    let world = response.world.expect("world");
    let galaxy = serde_json::to_value(&world.galaxy).expect("serialize galaxy");
    assert!(galaxy.get("market").is_none());
}

#[tokio::test]
async fn commander_state_snapshot_omits_only_sessions_without_required_position() {
    let service = RuntimeService::default();
    let valid_id = service
        .create_session_with_label(Some("Valid".to_string()))
        .expect("valid session");
    let invalid_id = service
        .create_session_with_label(Some("Invalid".to_string()))
        .expect("invalid session");

    {
        let session = service.get_session(valid_id).await.expect("valid session");
        let mut session = session.lock().await;
        apply_live_state(
            &mut session,
            BotState {
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: Some("sol".to_string()),
                    poi_id: Some("earth_station".to_string()),
                    docked_at: (true).then(|| "docked".to_string()),
                    ..Default::default()
                },
                ..BotState::default()
            },
            &WorldState::default(),
        );
        session.touch_state();
    }

    {
        let session = service
            .get_session(invalid_id)
            .await
            .expect("invalid session");
        let mut session = session.lock().await;
        apply_live_state(&mut session, BotState::default(), &WorldState::default());
        session.touch_state();
    }

    let response = service
        .commander_state_snapshot()
        .await
        .expect("commander state snapshot");

    let valid = response
        .sessions
        .iter()
        .find(|session| session.player_name == "Valid")
        .expect("valid session response");
    assert!(valid.state.is_some());

    let invalid = response
        .sessions
        .iter()
        .find(|session| session.player_name == "Invalid")
        .expect("invalid session response");
    assert!(invalid.state.is_none());
    assert!(response.world.is_some());
}

#[tokio::test]
async fn commander_session_delta_uses_global_sequence_and_emits_tombstones() {
    let service = RuntimeService::default();
    let gideon = service
        .create_session_with_label(Some("Gideon".to_string()))
        .expect("Gideon session");
    let mara = service
        .create_session_with_label(Some("Mara".to_string()))
        .expect("Mara session");
    let baseline = service
        .commander_session_state_delta(0)
        .await
        .expect("baseline");
    let baseline_version = baseline["stateVersion"].as_u64().expect("version");
    assert_eq!(baseline["sessions"].as_array().expect("sessions").len(), 2);

    {
        let session = service.get_session(gideon).await.expect("session");
        session.lock().await.touch_state();
    }
    service.note_session_changed(gideon);
    let delta = service
        .commander_session_state_delta(baseline_version)
        .await
        .expect("delta");
    let sessions = delta["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["playerName"], "Gideon");

    let before_remove = delta["stateVersion"].as_u64().expect("version");
    service.sessions.write().remove(&mara);
    service.note_session_removed(mara, "Mara".to_string());
    let removal = service
        .commander_session_state_delta(before_remove)
        .await
        .expect("removal delta");
    assert_eq!(
        removal["removedSessionHandles"],
        serde_json::json!(["Mara"])
    );
}

#[tokio::test]
async fn one_session_delta_scales_with_changed_session_not_fleet() {
    let service = RuntimeService::default();
    let mut ids = Vec::new();
    for index in 0..50 {
        ids.push(
            service
                .create_session_with_label(Some(format!("Fleet {index:02}")))
                .expect("session"),
        );
    }
    let baseline = service
        .commander_session_state_delta(0)
        .await
        .expect("baseline");
    let baseline_version = baseline["stateVersion"].as_u64().expect("version");
    let baseline_bytes = serde_json::to_vec(&baseline).expect("baseline bytes").len();

    let changed_id = ids[17];
    {
        let session = service.get_session(changed_id).await.expect("session");
        let mut session = session.lock().await;
        session.push_status("changed session only");
        session.touch_state();
    }
    service.note_session_changed(changed_id);
    let delta_started = Instant::now();
    let delta = service
        .commander_session_state_delta(baseline_version)
        .await
        .expect("delta");
    let delta_ms = delta_started.elapsed().as_millis();
    let delta_bytes = serde_json::to_vec(&delta).expect("delta bytes").len();

    assert_eq!(delta["sessions"].as_array().expect("sessions").len(), 1);
    assert!(delta.get("world").is_none());
    assert!(delta.get("social").is_none());
    assert!(delta_bytes < baseline_bytes / 10);
    assert!(delta_bytes < 500_000);
    assert!(delta_ms < 250, "delta composition took {delta_ms}ms");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn commander_knowledge_projection_does_not_reenter_knowledge_lock() {
    let service = Arc::new(RuntimeService::default());
    let reader = {
        let service = Arc::clone(&service);
        tokio::spawn(async move {
            for _ in 0..10 {
                service
                    .commander_knowledge_snapshot()
                    .await
                    .expect("knowledge snapshot");
                tokio::task::yield_now().await;
            }
        })
    };
    let writer = {
        let service = Arc::clone(&service);
        tokio::spawn(async move {
            for _ in 0..10 {
                let next = service
                    .knowledge_state
                    .read()
                    .knowledge_version
                    .saturating_add(1);
                service.knowledge_state.write().knowledge_version = next;
                tokio::task::yield_now().await;
            }
        })
    };

    tokio::time::timeout(Duration::from_secs(5), async {
        reader.await.expect("reader task");
        writer.await.expect("writer task");
    })
    .await
    .expect("knowledge projection deadlocked");
}

#[tokio::test]
async fn state_snapshot_serializes_shared_facility_catalog() {
    let service = RuntimeService::default();
    let id = service.create_session();

    {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        apply_live_state(
            &mut session,
            BotState {
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: Some("sol".to_string()),
                    poi_id: Some("poi_station_a".to_string()),
                    docked_at: (true).then(|| "docked".to_string()),
                    ..Default::default()
                },
                ..BotState::default()
            },
            &WorldState::default(),
        );
        session.touch_state();
    }

    {
        let mut knowledge = service.knowledge_state.write();
        knowledge.catalog = Arc::new(CatalogData {
            facilities: HashMap::from([(
                "faction_workshop".to_string(),
                spacemolt_lib_rs::schema::FacilityDefinition {
                    category: "faction".to_string(),
                    level: 1,
                    ..test_facility("faction_workshop", "Faction Workshop")
                },
            )]),
            ..CatalogData::default()
        });
    }

    let response = service
        .commander_state_snapshot()
        .await
        .expect("commander state snapshot");
    let world = response.world.expect("world");
    let workshop = world
        .galaxy
        .catalog
        .facilities_by_id
        .get("faction_workshop")
        .expect("facility catalog entry");

    assert_eq!(workshop.name, "Faction Workshop");
    assert_eq!(workshop.id, "faction_workshop");
    assert_eq!(workshop.level, 1);
}

#[tokio::test]
async fn script_runner_registry_rejects_duplicate_execute() {
    use crate::SdkError;

    let service = RuntimeService::new();
    let id = service.create_session();
    service
        .set_script(id, "go alpha;".to_string())
        .await
        .expect("set script");

    let run_guard = service
        .begin_script_run(id, "startup restore")
        .await
        .expect("begin runner");

    let err = service
        .execute_script(id)
        .await
        .expect_err("duplicate execute should fail");
    assert!(matches!(err, SdkError::BadRequest(_)));
    assert!(err.to_string().contains("startup restore"));

    drop(run_guard);
    let second_guard = service
        .begin_script_run(id, "api execute")
        .await
        .expect("runner can be reacquired after finish");
    drop(second_guard);
}

#[tokio::test]
async fn action_runner_registry_reuses_an_active_action_runner() {
    let service = Arc::new(RuntimeService::new());
    let id = service.create_session();
    let run_guard = service
        .begin_script_run(id, "sdk action run")
        .await
        .expect("begin action runner");

    service
        .ensure_action_runner(id, "sdk action run")
        .await
        .expect("active action runner should be reusable");

    assert!(service.script_run_info(id).await.is_some());
    drop(run_guard);
}

#[tokio::test]
async fn script_runner_guard_releases_on_drop() {
    let service = RuntimeService::new();
    let id = service.create_session();

    let run_guard = service
        .begin_script_run(id, "api execute")
        .await
        .expect("begin runner");
    assert!(service.script_run_info(id).await.is_some());

    drop(run_guard);
    assert!(service.script_run_info(id).await.is_none());
}

#[tokio::test]
async fn set_script_rejects_active_script_runner_without_replacing_script() {
    use crate::SdkError;

    let service = RuntimeService::new();
    let id = service.create_session();
    service
        .set_script(id, "go alpha;".to_string())
        .await
        .expect("set original script");
    let run_guard = service
        .begin_script_run(id, "startup restore")
        .await
        .expect("begin runner");

    let err = service
        .set_script(id, "go beta;".to_string())
        .await
        .expect_err("active runner should prevent replacing script");
    assert!(matches!(err, SdkError::BadRequest(_)));
    assert!(err.to_string().contains("startup restore"));

    let session = service.get_session(id).await.expect("session");
    let session = session.lock().await;
    assert_eq!(session.current_control_input.as_deref(), Some("go alpha;"));

    drop(session);
    drop(run_guard);
}

#[tokio::test]
async fn engine_snapshot_reports_active_script_runner() {
    let service = RuntimeService::new();
    let id = service.create_session();

    let run_guard = service
        .begin_script_run(id, "startup restore")
        .await
        .expect("begin runner");

    let snapshot = service
        .engine_snapshot_response(id)
        .await
        .expect("engine snapshot");

    assert!(snapshot.script_running);
    let runner = snapshot.script_runner.expect("runner info");
    assert_eq!(runner.origin, "startup restore");

    drop(run_guard);
}

#[tokio::test]
async fn engine_snapshot_does_not_report_checkpoint_as_running_without_runner() {
    let service = RuntimeService::new();
    let id = service.create_session();
    service
        .set_script(id, "go alpha;".to_string())
        .await
        .expect("set script");
    {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        session
            .engine
            .decide_next(prayer_runtime::read_context::ExecutionReadContext::default())
            .expect("decide next")
            .expect("command");
    }

    let snapshot = service
        .engine_snapshot_response(id)
        .await
        .expect("engine snapshot");

    assert!(!snapshot.script_running);
    assert!(snapshot.script_runner.is_none());
}

#[tokio::test]
async fn disconnected_restored_sessions_are_preserved() {
    let service = RuntimeService::new();
    let connected = service
        .create_session_with_label(Some("Connected".to_string()))
        .expect("connected session");
    let disconnected = service
        .create_session_with_label(Some("Ghost".to_string()))
        .expect("disconnected session");
    {
        let session = service.get_session(connected).await.expect("session");
        session.lock().await.spacemolt_account = Some(spacemolt_lib_rs::Account::new(
            spacemolt_lib_rs::AccountOptions {
                id: Some("Connected".to_string()),
                seed_state: false,
                ..spacemolt_lib_rs::AccountOptions::default()
            },
        ));
    }

    assert!(service.sessions.read().contains_key(&disconnected));
    assert!(service.get_session(connected).await.is_ok());
    assert!(service.get_session(disconnected).await.is_ok());
    assert!(service.session_labels.read().contains_key("Ghost"));
}

#[tokio::test]
async fn create_and_list_sessions() {
    let service = RuntimeService::new();
    let id1 = service.create_session();
    let id2 = service.create_session();
    let list = service.list_sessions().await;
    let ids: Vec<_> = list.iter().map(|s| s.id.clone()).collect();
    assert!(ids.contains(&id1.to_string()));
    assert!(ids.contains(&id2.to_string()));
}

#[tokio::test]
async fn session_summary_returns_not_found_for_unknown_id() {
    use crate::SdkError;
    let service = RuntimeService::new();
    let fake_id = uuid::Uuid::new_v4().to_string();
    let err = service
        .session_summary(&fake_id)
        .await
        .expect_err("expected not found");
    assert!(matches!(err, SdkError::SessionNotFound));
}

#[tokio::test]
async fn set_script_invalid_dsl_returns_error() {
    use crate::SdkError;
    let service = RuntimeService::new();
    let id = service.create_session();
    let err = service
        .set_script(id, "INVALID GARBAGE !!!".to_string())
        .await
        .expect_err("expected error");
    assert!(matches!(err, SdkError::Engine(_)));
}

#[tokio::test]
async fn halt_clears_execution_without_latching_the_scheduler() {
    let service = RuntimeService::new();
    let id = service.create_session();
    service
        .set_script(id, "go alpha;".to_string())
        .await
        .expect("set script");

    service.halt(id, None).await.expect("halt");
    let snapshot = service.snapshot(id).await.expect("snapshot");
    assert!(!snapshot.is_halted);
    let scheduler = service
        .scheduler_snapshot(id)
        .await
        .expect("scheduler snapshot");
    assert!(scheduler.claim.is_none());
    assert!(scheduler.running.is_none());
    assert!(scheduler.pending.is_empty());

    service
        .set_script(id, "go beta;".to_string())
        .await
        .expect("set replacement script without resume");
    let snapshot = service.snapshot(id).await.expect("replacement snapshot");
    assert!(!snapshot.is_halted);
}

#[tokio::test]
async fn checkpoint_roundtrip_restores_script() {
    let service = RuntimeService::new();
    let id = service.create_session();
    service
        .set_script(id, "go alpha;".to_string())
        .await
        .expect("set script");

    let cp = service.execution_checkpoint(id).await.expect("checkpoint");
    assert!(serde_json::to_string(&cp)
        .expect("json")
        .contains("go alpha;"));

    let id2 = service.create_session();
    service
        .restore_execution_checkpoint(id2, cp)
        .await
        .expect("restore");

    let snap = service.snapshot(id2).await.expect("snapshot");
    assert!(snap.script.contains("go alpha;"));
}

#[tokio::test]
async fn checkpoint_restore_preserves_explicit_poi() {
    let service = RuntimeService::new();
    let id = service.create_session();
    {
        let session = service.get_session(id).await.expect("session");
        let mut session = session.lock().await;
        apply_live_state(
            &mut session,
            BotState {
                fuel_pct: 100,
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: Some("alpha".to_string()),
                    poi_id: None,
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
            &WorldState::default(),
        );
    }
    service
        .set_script(id, "go alpha;".to_string())
        .await
        .expect("set script");
    let cp = service.execution_checkpoint(id).await.expect("checkpoint");

    let id2 = service.create_session();
    {
        let session = service.get_session(id2).await.expect("session");
        let mut session = session.lock().await;
        apply_live_state(
            &mut session,
            BotState {
                fuel_pct: 100,
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: Some("beta".to_string()),
                    poi_id: None,
                    docked_at: None,
                    ..Default::default()
                },
                ..BotState::default()
            },
            &WorldState::default(),
        );
    }
    service
        .restore_execution_checkpoint(id2, cp)
        .await
        .expect("restore");

    let session = service.get_session(id2).await.expect("session");
    let mut session = session.lock().await;
    let bot = Arc::clone(&session.actor.observed);
    let world = world_read_state(&WorldState::default(), &bot);
    let runtime = prayer_runtime::read_context::ExecutionRuntimeState::default();
    let cmd = session
        .engine
        .decide_next(prayer_runtime::read_context::ExecutionReadContext {
            bot: &bot,
            world: &world,
            runtime: &runtime,
        })
        .expect("decide")
        .expect("cmd");
    assert_eq!(cmd.args_as_strings(), vec!["alpha".to_string()]);
}

#[tokio::test]
async fn drain_events_clears_after_first_call() {
    let service = RuntimeService::new();
    let id = service.create_session();
    service
        .set_script(id, "go alpha;".to_string())
        .await
        .expect("set script");

    let events = service.drain_events(id).await.expect("drain");
    assert!(!events.is_empty());

    let events2 = service.drain_events(id).await.expect("drain 2");
    assert!(events2.is_empty());
}
