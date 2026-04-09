//! Runtime transport boundary and SpaceMolt adapter stubs.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::engine::{
    CatalogEntryData, CommandArg, EngineCommand, EngineError, EngineExecutionResult, GalaxyData,
    GameState, MarketData, MarketOrderInfo, MissionData, OpenOrderInfo, ShipState,
};
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

mod orchestrator;

/// Transport-level errors.
#[derive(Debug, Error)]
pub enum TransportError {
    /// Network-level failure.
    #[error("network failure: {0}")]
    Network(String),
    /// Non-success API response.
    #[error("api failure ({status}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Server message.
        message: String,
    },
    /// Command was not supported.
    #[error("unsupported command '{0}'")]
    UnsupportedCommand(String),
}

/// Runtime command handler abstraction behind transport boundary.
#[async_trait]
pub trait RuntimeTransport: Send + Sync {
    /// Execute one command against remote runtime target.
    async fn execute(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError>;

    /// Execute a raw API action with an optional JSON payload.
    async fn execute_passthrough(
        &self,
        action: &str,
        payload: Option<Value>,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError>;

    /// Fetch latest external game state snapshot.
    async fn fetch_state(&self) -> Result<GameState, TransportError>;
}

/// Minimal SpaceMolt HTTP transport implementation.
pub struct SpaceMoltTransport {
    client: reqwest::Client,
    base_url: String,
    session_id: String,
    catalog_cache: Mutex<Option<CatalogCacheEntry>>,
}

#[derive(Debug, Clone, Default)]
struct CatalogCacheEntry {
    version: Option<String>,
    item_ids: Vec<String>,
    ship_ids: Vec<String>,
    recipe_ids: Vec<String>,
    item_entries: HashMap<String, CatalogEntryData>,
    ship_entries: HashMap<String, CatalogEntryData>,
    recipe_entries: HashMap<String, CatalogEntryData>,
}

impl SpaceMoltTransport {
    /// Build SpaceMolt transport.
    pub fn new(base_url: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            session_id: session_id.into(),
            catalog_cache: Mutex::new(None),
        }
    }

    /// Seed local catalog cache from previously persisted knowledge.
    pub fn seed_catalog_cache(&self, version: Option<String>, galaxy: &GalaxyData) {
        let item_ids = if galaxy.item_ids.is_empty() {
            galaxy.item_catalog_entries.keys().cloned().collect()
        } else {
            galaxy.item_ids.clone()
        };
        let ship_ids = if galaxy.ship_ids.is_empty() {
            galaxy.ship_catalog_entries.keys().cloned().collect()
        } else {
            galaxy.ship_ids.clone()
        };
        let recipe_ids = if galaxy.recipe_ids.is_empty() {
            galaxy.recipe_catalog_entries.keys().cloned().collect()
        } else {
            galaxy.recipe_ids.clone()
        };
        if item_ids.is_empty() && ship_ids.is_empty() && recipe_ids.is_empty() {
            return;
        }
        let mut cache = self.catalog_cache_guard();
        *cache = Some(CatalogCacheEntry {
            version,
            item_ids,
            ship_ids,
            recipe_ids,
            item_entries: galaxy.item_catalog_entries.clone(),
            ship_entries: galaxy.ship_catalog_entries.clone(),
            recipe_entries: galaxy.recipe_catalog_entries.clone(),
        });
    }

    fn catalog_cache_guard(&self) -> MutexGuard<'_, Option<CatalogCacheEntry>> {
        match self.catalog_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[async_trait]
impl RuntimeTransport for SpaceMoltTransport {
    async fn execute(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        if command.action.eq_ignore_ascii_case("halt") {
            return Ok(EngineExecutionResult {
                result_message: Some("Script halted.".to_string()),
                completed: true,
                halt_script: true,
            });
        }

        if let Some(result) = self.execute_high_level(command, runtime_state).await? {
            return Ok(result);
        }

        let spec = map_command_spec(&command.action)?;
        let payload = args_to_payload(&command.args, spec.payload_keys)?;
        let value = self.execute_api(spec.api_action, Some(payload)).await?;
        let result_message = value
            .get("result")
            .and_then(|v| {
                v.get("message")
                    .or_else(|| v.get("error"))
                    .or_else(|| v.get("status"))
            })
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let halt_script = command.action.eq_ignore_ascii_case("halt");
        Ok(EngineExecutionResult {
            result_message,
            completed: true,
            halt_script,
        })
    }

    async fn execute_passthrough(
        &self,
        action: &str,
        payload: Option<Value>,
        _runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        if action.eq_ignore_ascii_case("halt") {
            return Ok(EngineExecutionResult {
                result_message: Some("Script halted.".to_string()),
                completed: true,
                halt_script: true,
            });
        }

        let value = self.execute_api(action, payload).await?;
        let result_message = value
            .get("result")
            .and_then(|v| {
                v.get("message")
                    .or_else(|| v.get("error"))
                    .or_else(|| v.get("status"))
            })
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        Ok(EngineExecutionResult {
            result_message,
            completed: true,
            halt_script: false,
        })
    }

    async fn fetch_state(&self) -> Result<GameState, TransportError> {
        let status = self.execute_api("get_status", None).await?;
        let mut state = map_status_to_game_state(&status);

        if let Ok(system) = self.execute_api("get_system", None).await {
            enrich_from_get_system(&mut state, &system);
        }
        if let Ok(poi) = self.execute_api("get_poi", None).await {
            enrich_from_get_poi(&mut state, &poi);
        }
        if let Ok(map) = self.execute_api("get_map", None).await {
            enrich_from_get_map(&mut state, &map);
        }
        if let Ok(active_missions) = self.execute_api("get_active_missions", None).await {
            enrich_from_get_active_missions(&mut state, &active_missions);
        }
        self.refresh_catalog_cache_if_needed().await;
        self.enrich_catalog_ids_from_cache(&mut state);

        if state.docked {
            if let Ok(available_missions) = self.execute_api("get_missions", None).await {
                enrich_from_get_missions(&mut state, &available_missions);
            }
            if let Some(station_id) = state.current_poi.clone() {
                if let Ok(storage) = self
                    .execute_api(
                        "view_storage",
                        Some(serde_json::json!({ "station_id": station_id })),
                    )
                    .await
                {
                    enrich_from_view_storage(&mut state, &storage, &station_id);
                }
                if let Ok(market) = self.execute_api("view_market", None).await {
                    enrich_from_view_market(&mut state, &market);
                }
                if let Ok(orders) = self
                    .execute_api(
                        "view_orders",
                        Some(serde_json::json!({ "station_id": station_id })),
                    )
                    .await
                {
                    enrich_from_view_orders(&mut state, &orders);
                }
            }
        }

        Ok(state)
    }
}

impl SpaceMoltTransport {
    pub(super) async fn execute_api(
        &self,
        api_action: &str,
        payload: Option<Value>,
    ) -> Result<Value, TransportError> {
        let url = format!(
            "{}/api/v1/{api_action}",
            self.base_url.trim_end_matches('/')
        );
        let mut req = self
            .client
            .post(url)
            .header("X-Session-Id", &self.session_id);
        if let Some(p) = payload {
            req = req.json(&p);
        }
        let response = req
            .send()
            .await
            .map_err(|e| TransportError::Network(e.to_string()))?;
        if !response.status().is_success() {
            return Err(map_failed_response(
                response.status(),
                response.text().await.ok(),
            ));
        }
        response
            .json::<Value>()
            .await
            .map_err(|e| TransportError::Network(e.to_string()))
    }

    async fn fetch_catalog_entries(
        &self,
        catalog_type: &str,
    ) -> Result<HashMap<String, CatalogEntryData>, TransportError> {
        let mut out = HashMap::new();
        let mut page: i64 = 1;
        loop {
            let value = self
                .execute_api(
                    "catalog",
                    Some(serde_json::json!({
                        "type": catalog_type,
                        "page": page,
                        "page_size": 50
                    })),
                )
                .await?;
            let root = value.get("result").unwrap_or(&value);
            let Some(items) = root.get("items").and_then(Value::as_array) else {
                break;
            };
            for item in items {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    continue;
                };
                if id.trim().is_empty() {
                    continue;
                }
                out.insert(
                    id.to_string(),
                    CatalogEntryData {
                        id: id.to_string(),
                        raw: item.clone(),
                    },
                );
            }
            let total_pages = root
                .get("total_pages")
                .and_then(Value::as_i64)
                .unwrap_or(page);
            if page >= total_pages || items.is_empty() {
                break;
            }
            page += 1;
            if page > 1_000 {
                break;
            }
        }
        Ok(out)
    }

    async fn current_server_version(&self) -> Result<Option<String>, TransportError> {
        let value = self
            .execute_api(
                "get_version",
                Some(serde_json::json!({
                    "count": 1,
                    "page": 1
                })),
            )
            .await?;
        let root = value.get("result").unwrap_or(&value);
        Ok(root
            .get("version")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned))
    }

    async fn refresh_catalog_cache_if_needed(&self) {
        let version = self.current_server_version().await.ok().flatten();

        let needs_refresh = {
            let cached_entry = self.catalog_cache_guard().clone();
            match cached_entry {
                Some(entry) => match version.as_deref() {
                    Some(v) => entry.version.as_deref() != Some(v),
                    None => {
                        entry.item_ids.is_empty()
                            && entry.ship_ids.is_empty()
                            && entry.recipe_ids.is_empty()
                    }
                },
                None => true,
            }
        };

        if !needs_refresh {
            return;
        }

        let item_entries = self
            .fetch_catalog_entries("items")
            .await
            .unwrap_or_default();
        let ship_entries = self
            .fetch_catalog_entries("ships")
            .await
            .unwrap_or_default();
        let recipe_entries = self
            .fetch_catalog_entries("recipes")
            .await
            .unwrap_or_default();
        let item_ids = item_entries.keys().cloned().collect::<Vec<_>>();
        let ship_ids = ship_entries.keys().cloned().collect::<Vec<_>>();
        let recipe_ids = recipe_entries.keys().cloned().collect::<Vec<_>>();

        let mut cache = self.catalog_cache_guard();
        *cache = Some(CatalogCacheEntry {
            version,
            item_ids,
            ship_ids,
            recipe_ids,
            item_entries,
            ship_entries,
            recipe_entries,
        });
    }

    fn enrich_catalog_ids_from_cache(&self, state: &mut GameState) {
        let entry = self.catalog_cache_guard().clone();
        let Some(entry) = entry else {
            return;
        };

        let mut galaxy = state.galaxy.as_ref().clone();
        if !entry.item_ids.is_empty() {
            galaxy.item_ids = entry.item_ids;
        }
        if !entry.ship_ids.is_empty() {
            galaxy.ship_ids = entry.ship_ids;
        }
        if !entry.recipe_ids.is_empty() {
            galaxy.recipe_ids = entry.recipe_ids;
        }
        if !entry.item_entries.is_empty() {
            galaxy.item_catalog_entries = entry.item_entries;
        }
        if !entry.ship_entries.is_empty() {
            galaxy.ship_catalog_entries = entry.ship_entries;
        }
        if !entry.recipe_entries.is_empty() {
            galaxy.recipe_catalog_entries = entry.recipe_entries;
        }
        galaxy.catalog_version = entry.version;
        state.galaxy = Arc::new(galaxy);
    }
}

/// In-memory mock transport for testing/integration harnesses.
#[derive(Default)]
pub struct MockTransport {
    /// Static responses by action name.
    pub responses: HashMap<String, EngineExecutionResult>,
    /// Static state payload.
    pub state: GameState,
}

#[async_trait]
impl RuntimeTransport for MockTransport {
    async fn execute(
        &self,
        command: &EngineCommand,
        _runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        if let Some(response) = self.responses.get(&command.action) {
            return Ok(response.clone());
        }

        Ok(EngineExecutionResult {
            result_message: Some("ok".to_string()),
            completed: true,
            halt_script: command.action.eq_ignore_ascii_case("halt"),
        })
    }

    async fn fetch_state(&self) -> Result<GameState, TransportError> {
        Ok(self.state.clone())
    }

    async fn execute_passthrough(
        &self,
        action: &str,
        _payload: Option<Value>,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let command = EngineCommand {
            action: action.to_string(),
            args: Vec::new(),
            source_line: None,
        };
        self.execute(&command, runtime_state).await
    }
}

fn map_failed_response(status: StatusCode, body: Option<String>) -> TransportError {
    TransportError::Api {
        status: status.as_u16(),
        message: body
            .as_deref()
            .unwrap_or("remote api request failed")
            .to_string(),
    }
}

fn args_to_payload(
    args: &[CommandArg],
    keys: &'static [&'static str],
) -> Result<Value, TransportError> {
    if args.is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    if args.len() > keys.len() {
        return Err(TransportError::UnsupportedCommand(format!(
            "payload expected at most {} args, got {}",
            keys.len(),
            args.len()
        )));
    }

    let mut map = serde_json::Map::new();
    for (idx, arg) in args.iter().enumerate() {
        map.insert(keys[idx].to_string(), command_arg_to_json(arg));
    }
    Ok(Value::Object(map))
}

#[derive(Debug, Clone, Copy)]
struct CommandSpecMap {
    api_action: &'static str,
    payload_keys: &'static [&'static str],
}

fn map_command_spec(action: &str) -> Result<CommandSpecMap, TransportError> {
    match action.to_ascii_lowercase().as_str() {
        // Direct C# command->API mappings
        "survey" => Ok(CommandSpecMap {
            api_action: "survey_system",
            payload_keys: &[],
        }),
        "repair" => Ok(CommandSpecMap {
            api_action: "repair",
            payload_keys: &[],
        }),
        "self_destruct" => Ok(CommandSpecMap {
            api_action: "self_destruct",
            payload_keys: &[],
        }),
        "accept_mission" => Ok(CommandSpecMap {
            api_action: "accept_mission",
            payload_keys: &["mission_id"],
        }),
        "abandon_mission" => Ok(CommandSpecMap {
            api_action: "abandon_mission",
            payload_keys: &["mission_id"],
        }),
        "decline_mission" => Ok(CommandSpecMap {
            api_action: "decline_mission",
            payload_keys: &["template_id"],
        }),
        "complete_mission" => Ok(CommandSpecMap {
            api_action: "complete_mission",
            payload_keys: &["mission_id"],
        }),
        "switch_ship" => Ok(CommandSpecMap {
            api_action: "switch_ship",
            payload_keys: &["ship_id"],
        }),
        "install_mod" => Ok(CommandSpecMap {
            api_action: "install_mod",
            payload_keys: &["module_id"],
        }),
        "uninstall_mod" => Ok(CommandSpecMap {
            api_action: "uninstall_mod",
            payload_keys: &["module_id"],
        }),
        "buy_ship" => Ok(CommandSpecMap {
            api_action: "buy_listed_ship",
            payload_keys: &["listing_id"],
        }),
        "buy_listed_ship" => Ok(CommandSpecMap {
            api_action: "buy_listed_ship",
            payload_keys: &["listing_id"],
        }),
        "commission_ship" => Ok(CommandSpecMap {
            api_action: "commission_ship",
            payload_keys: &["ship_class"],
        }),
        "sell_ship" => Ok(CommandSpecMap {
            api_action: "sell_ship",
            payload_keys: &["ship_id"],
        }),
        "list_ship_for_sale" => Ok(CommandSpecMap {
            api_action: "list_ship_for_sale",
            payload_keys: &["ship_id", "price"],
        }),
        "craft" => Ok(CommandSpecMap {
            api_action: "craft",
            payload_keys: &["recipe_id", "quantity"],
        }),
        // C# implements these as richer multi-step/high-level operations.
        "go" | "mine" | "explore" | "buy" | "sell" | "cancel_buy" | "cancel_sell" | "retrieve"
        | "stash" | "wait" | "set_home" | "refuel" | "jettison" | "dock" => {
            Err(TransportError::UnsupportedCommand(format!(
            "{action} requires command-engine orchestration (C#-style multi-step/high-level behavior)"
        )))
        }
        _ => Err(TransportError::UnsupportedCommand(action.to_string())),
    }
}

fn command_arg_to_json(arg: &CommandArg) -> Value {
    match arg {
        CommandArg::Integer(v) => Value::Number((*v).into()),
        _ => Value::String(arg.as_text()),
    }
}

fn map_status_to_game_state(value: &Value) -> GameState {
    let result = value.get("result").unwrap_or(value);
    let player = result.get("player").unwrap_or(result);
    let ship = result.get("ship").unwrap_or(result);

    let credits = player
        .get("credits")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let system = player
        .get("system_id")
        .or_else(|| player.get("current_system"))
        .or_else(|| player.get("system"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let home_base = player
        .get("home_base")
        .or_else(|| player.get("home_base_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let nearest_station = player
        .get("nearest_station")
        .or_else(|| player.get("nearest_station_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let current_poi = player
        .get("current_poi_id")
        .or_else(|| player.get("current_poi"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let docked = player
        .get("docked")
        .and_then(Value::as_bool)
        .or_else(|| {
            player
                .get("docked_at_base")
                .and_then(Value::as_str)
                .map(|_| true)
        })
        .unwrap_or(false);

    let fuel = ship.get("fuel").and_then(Value::as_i64).unwrap_or_default();
    let max_fuel = ship
        .get("max_fuel")
        .and_then(Value::as_i64)
        .unwrap_or(100)
        .max(1);
    let cargo_used = ship
        .get("cargo_used")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let cargo_capacity = ship
        .get("cargo_capacity")
        .and_then(Value::as_i64)
        .unwrap_or(100)
        .max(1);
    let fuel_pct = ((fuel * 100) / max_fuel).clamp(0, 100);
    let cargo_pct = ((cargo_used * 100) / cargo_capacity).clamp(0, 100);

    let mut cargo = HashMap::new();
    match ship.get("cargo") {
        Some(Value::Object(cargo_obj)) => {
            for (item, qty) in cargo_obj {
                if let Some(v) = qty.as_i64() {
                    cargo.insert(item.clone(), v);
                } else if let Some(v) = qty.get("quantity").and_then(Value::as_i64) {
                    cargo.insert(item.clone(), v);
                }
            }
        }
        Some(Value::Array(cargo_entries)) => {
            for entry in cargo_entries {
                let Some(item_id) = entry.get("item_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(quantity) = entry.get("quantity").and_then(Value::as_i64) else {
                    continue;
                };
                cargo.insert(item_id.to_string(), quantity);
            }
        }
        _ => {}
    }

    let systems = extract_ids(result.get("systems"));
    let pois = extract_ids(result.get("pois"));
    let system_connections = extract_system_connections(result.get("systems"));
    let system_coordinates = extract_system_coordinates(result.get("systems"));
    let (poi_system, poi_base_to_id, poi_type_by_id) =
        extract_poi_lookups(result.get("systems"), result.get("pois"));
    let pois_by_resource = extract_pois_by_resource(result);
    let (explored_systems, visited_pois, surveyed_systems) = extract_exploration_sets(result);
    let (dockable_pois_by_system, station_pois_by_system) =
        extract_dockable_and_station_pois(result.get("systems"));
    let item_ids = extract_ids(result.get("items"))
        .into_iter()
        .collect::<Vec<_>>();
    let ship_ids = extract_ids(result.get("ships"));
    let recipe_ids = extract_ids(result.get("available_recipes"));
    let shipyard_listings = extract_ids(result.get("shipyard_listings"));
    let active_missions = extract_ids(result.get("active_missions"));
    let available_missions = extract_ids(result.get("available_missions"));
    let owned_ships = extract_ids(result.get("owned_ships"));
    let installed_modules = extract_ids(
        ship.get("installed_modules")
            .or_else(|| ship.get("modules")),
    );
    let own_buy_orders = extract_open_orders(result.get("own_buy_orders"));
    let own_sell_orders = extract_open_orders(result.get("own_sell_orders"));

    let ship_state = ShipState {
        name: ship
            .get("name")
            .or_else(|| ship.get("ship_name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        class_id: ship
            .get("class_id")
            .or_else(|| ship.get("classId"))
            .or_else(|| ship.get("class"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        armor: ship
            .get("armor")
            .or_else(|| ship.get("current_armor"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        speed: ship.get("speed").and_then(Value::as_i64).unwrap_or(0),
        hull: ship
            .get("hull")
            .or_else(|| ship.get("current_hull"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        max_hull: ship
            .get("max_hull")
            .or_else(|| ship.get("maxHull"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        shield: ship
            .get("shield")
            .or_else(|| ship.get("current_shield"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        max_shield: ship
            .get("max_shield")
            .or_else(|| ship.get("maxShield"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        cpu_used: ship
            .get("cpu_used")
            .or_else(|| ship.get("cpuUsed"))
            .or_else(|| ship.get("cpu"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        cpu_capacity: ship
            .get("cpu_capacity")
            .or_else(|| ship.get("cpuCapacity"))
            .or_else(|| ship.get("max_cpu"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        power_used: ship
            .get("power_used")
            .or_else(|| ship.get("powerUsed"))
            .or_else(|| ship.get("power"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        power_capacity: ship
            .get("power_capacity")
            .or_else(|| ship.get("powerCapacity"))
            .or_else(|| ship.get("max_power"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
    };

    GameState {
        system,
        home_base,
        nearest_station,
        current_poi,
        docked,
        credits,
        fuel_pct,
        cargo_pct,
        cargo_used,
        cargo_capacity,
        cargo: Arc::new(cargo),
        galaxy: Arc::new(GalaxyData {
            systems,
            pois,
            item_ids,
            ship_ids,
            recipe_ids,
            item_catalog_entries: HashMap::new(),
            ship_catalog_entries: HashMap::new(),
            recipe_catalog_entries: HashMap::new(),
            catalog_version: None,
            system_connections,
            system_coordinates,
            poi_system,
            poi_base_to_id,
            poi_type_by_id,
            pois_by_resource,
            explored_systems,
            visited_pois,
            surveyed_systems,
            dockable_pois_by_system,
            station_pois_by_system,
        }),
        market: Arc::new(MarketData {
            shipyard_listings,
            buy_orders: HashMap::new(),
            sell_orders: HashMap::new(),
        }),
        missions: Arc::new(MissionData {
            active: active_missions,
            available: available_missions,
        }),
        owned_ships: Arc::new(owned_ships),
        installed_modules: Arc::new(installed_modules),
        own_buy_orders: Arc::new(own_buy_orders),
        own_sell_orders: Arc::new(own_sell_orders),
        ship: ship_state,
        ..GameState::default()
    }
}

fn enrich_from_get_system(state: &mut GameState, value: &Value) {
    let root = value.get("result").unwrap_or(value);
    if let Some(from_system) = root.get("from_system").and_then(Value::as_str) {
        state.system = Some(from_system.to_string());
    }
    let system_obj = root.get("system").unwrap_or(root);

    if let Some(system_id) = system_obj.get("id").and_then(Value::as_str) {
        state.system = Some(system_id.to_string());
    }

    let mut galaxy = state.galaxy.as_ref().clone();

    let current_system = state.system.clone().unwrap_or_default();
    if let Some(connections) = system_obj.get("connections").and_then(Value::as_array) {
        let neighbors = connections
            .iter()
            .filter_map(|entry| {
                entry
                    .get("system_id")
                    .or_else(|| entry.get("id"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>();
        if !current_system.is_empty() {
            galaxy
                .system_connections
                .insert(current_system.clone(), neighbors);
        }
    }
    if !current_system.is_empty() {
        if let (Some(x), Some(y)) = (
            number_as_f64(system_obj.get("x")),
            number_as_f64(system_obj.get("y")),
        ) {
            galaxy
                .system_coordinates
                .insert(current_system.clone(), (x, y));
        }
    }

    if let Some(pois) = system_obj.get("pois").and_then(Value::as_array) {
        let mut dockable = Vec::new();
        let mut station = Vec::new();
        for poi in pois {
            let poi_id = poi
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| poi.as_str());
            if let Some(poi_id) = poi_id {
                if !galaxy.pois.iter().any(|p| p == poi_id) {
                    galaxy.pois.push(poi_id.to_string());
                }
                if !current_system.is_empty() {
                    galaxy
                        .poi_system
                        .insert(poi_id.to_string(), current_system.clone());
                }
                if let Some(base_id) = poi
                    .get("base_id")
                    .or_else(|| poi.get("baseId"))
                    .and_then(Value::as_str)
                {
                    galaxy
                        .poi_base_to_id
                        .insert(base_id.to_string(), poi_id.to_string());
                }
                if let Some(poi_type) = poi.get("type").and_then(Value::as_str) {
                    galaxy
                        .poi_type_by_id
                        .insert(poi_id.to_string(), poi_type.to_string());
                }
                let has_base = poi
                    .get("has_base")
                    .or_else(|| poi.get("hasBase"))
                    .and_then(Value::as_bool)
                    .unwrap_or_else(|| {
                        poi.get("base_id")
                            .or_else(|| poi.get("baseId"))
                            .and_then(Value::as_str)
                            .is_some()
                    });
                if has_base {
                    dockable.push(poi_id.to_string());
                    let poi_type = poi.get("type").and_then(Value::as_str).unwrap_or_default();
                    if poi_type.eq_ignore_ascii_case("station") {
                        station.push(poi_id.to_string());
                    }
                }
            }
        }
        if !current_system.is_empty() {
            galaxy
                .dockable_pois_by_system
                .insert(current_system.clone(), dockable);
            galaxy
                .station_pois_by_system
                .insert(current_system.clone(), station);
        }
    }

    if let Some(poi_obj) = root.get("poi").and_then(Value::as_object) {
        if let Some(poi_id) = poi_obj.get("id").and_then(Value::as_str) {
            state.current_poi = Some(poi_id.to_string());
            if !current_system.is_empty() {
                galaxy
                    .poi_system
                    .insert(poi_id.to_string(), current_system.clone());
            }
            if let Some(base_id) = poi_obj
                .get("base_id")
                .or_else(|| poi_obj.get("baseId"))
                .and_then(Value::as_str)
            {
                galaxy
                    .poi_base_to_id
                    .insert(base_id.to_string(), poi_id.to_string());
            }
        }
    }

    state.galaxy = Arc::new(galaxy);
}

fn enrich_from_get_poi(state: &mut GameState, value: &Value) {
    let root = value.get("result").unwrap_or(value);
    let poi_obj = root.get("poi").unwrap_or(root);
    let Some(poi_id) = poi_obj.get("id").and_then(Value::as_str) else {
        return;
    };

    let mut galaxy = state.galaxy.as_ref().clone();
    push_unique(&mut galaxy.pois, poi_id.to_string());

    if let Some(system_id) = poi_obj
        .get("system_id")
        .or_else(|| poi_obj.get("systemId"))
        .and_then(Value::as_str)
    {
        galaxy
            .poi_system
            .insert(poi_id.to_string(), system_id.to_string());
        push_unique(&mut galaxy.systems, system_id.to_string());
        if state.system.is_none() {
            state.system = Some(system_id.to_string());
        }
    }

    if let Some(base_id) = poi_obj
        .get("base_id")
        .or_else(|| poi_obj.get("baseId"))
        .and_then(Value::as_str)
    {
        galaxy
            .poi_base_to_id
            .insert(base_id.to_string(), poi_id.to_string());
    }

    if let Some(poi_type) = poi_obj.get("type").and_then(Value::as_str) {
        galaxy
            .poi_type_by_id
            .insert(poi_id.to_string(), poi_type.to_string());
    }

    if state.current_poi.is_none() {
        state.current_poi = Some(poi_id.to_string());
    }

    let resources = root
        .get("resources")
        .or_else(|| poi_obj.get("resources"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for resource in resources {
        let Some(resource_id) = resource
            .get("resource_id")
            .or_else(|| resource.get("resourceId"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if resource_id.trim().is_empty() {
            continue;
        }
        push_map_unique(
            &mut galaxy.pois_by_resource,
            resource_id,
            poi_id.to_string(),
        );
    }

    state.galaxy = Arc::new(galaxy);
}

fn enrich_from_get_map(state: &mut GameState, value: &Value) {
    let root = value.get("result").unwrap_or(value);
    let systems = root
        .get("systems")
        .or_else(|| root.get("map").and_then(|m| m.get("systems")));
    let known_pois = root
        .get("known_pois")
        .or_else(|| root.get("knownPois"))
        .or_else(|| root.get("map").and_then(|m| m.get("known_pois")))
        .or_else(|| root.get("map").and_then(|m| m.get("knownPois")));

    let mut galaxy = state.galaxy.as_ref().clone();

    if let Some(Value::Array(system_entries)) = systems {
        for entry in system_entries {
            let Value::Object(system_obj) = entry else {
                continue;
            };

            let Some(system_id) = value_as_str(system_obj, &["id", "system_id", "Id"]) else {
                continue;
            };
            push_unique(&mut galaxy.systems, system_id.to_string());

            if let (Some(x), Some(y)) = (
                number_as_f64(
                    system_obj
                        .get("x")
                        .or_else(|| system_obj.get("X"))
                        .or_else(|| system_obj.get("position").and_then(|p| p.get("x")))
                        .or_else(|| system_obj.get("Position").and_then(|p| p.get("X"))),
                ),
                number_as_f64(
                    system_obj
                        .get("y")
                        .or_else(|| system_obj.get("Y"))
                        .or_else(|| system_obj.get("position").and_then(|p| p.get("y")))
                        .or_else(|| system_obj.get("Position").and_then(|p| p.get("Y"))),
                ),
            ) {
                galaxy
                    .system_coordinates
                    .insert(system_id.to_string(), (x, y));
            }

            if let Some(Value::Array(connections)) = system_obj
                .get("connections")
                .or_else(|| system_obj.get("Connections"))
            {
                let neighbors = connections
                    .iter()
                    .filter_map(|c| match c {
                        Value::String(s) => Some(s.clone()),
                        Value::Object(obj) => {
                            value_as_str(obj, &["system_id", "id", "Id"]).map(ToOwned::to_owned)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if !neighbors.is_empty() {
                    galaxy
                        .system_connections
                        .insert(system_id.to_string(), neighbors);
                }
            }

            if let Some(Value::Array(pois)) =
                system_obj.get("pois").or_else(|| system_obj.get("Pois"))
            {
                let mut dockable = Vec::new();
                let mut station = Vec::new();
                for poi in pois {
                    let Value::Object(poi_obj) = poi else {
                        continue;
                    };
                    let Some(poi_id) = value_as_str(poi_obj, &["id", "poi_id", "Id"]) else {
                        continue;
                    };
                    push_unique(&mut galaxy.pois, poi_id.to_string());
                    galaxy
                        .poi_system
                        .insert(poi_id.to_string(), system_id.to_string());
                    if let Some(base_id) = value_as_str(poi_obj, &["base_id", "baseId", "BaseId"]) {
                        galaxy
                            .poi_base_to_id
                            .insert(base_id.to_string(), poi_id.to_string());
                    }
                    if let Some(poi_type) = value_as_str(poi_obj, &["type", "Type"]) {
                        galaxy
                            .poi_type_by_id
                            .insert(poi_id.to_string(), poi_type.to_string());
                    }
                    let has_base = poi_obj
                        .get("has_base")
                        .or_else(|| poi_obj.get("hasBase"))
                        .or_else(|| poi_obj.get("HasBase"))
                        .and_then(Value::as_bool)
                        .unwrap_or_else(|| {
                            value_as_str(poi_obj, &["base_id", "baseId", "BaseId"]).is_some()
                        });
                    if has_base {
                        dockable.push(poi_id.to_string());
                        if value_as_str(poi_obj, &["type", "Type"])
                            .is_some_and(|t| t.eq_ignore_ascii_case("station"))
                        {
                            station.push(poi_id.to_string());
                        }
                    }
                }
                if !dockable.is_empty() {
                    galaxy
                        .dockable_pois_by_system
                        .insert(system_id.to_string(), dockable);
                }
                if !station.is_empty() {
                    galaxy
                        .station_pois_by_system
                        .insert(system_id.to_string(), station);
                }
            }
        }
    }

    if let Some(Value::Array(pois)) = known_pois {
        for poi in pois {
            let Value::Object(poi_obj) = poi else {
                continue;
            };
            let Some(poi_id) = value_as_str(poi_obj, &["id", "poi_id", "Id"]) else {
                continue;
            };
            let Some(system_id) = value_as_str(poi_obj, &["system_id", "systemId", "SystemId"])
            else {
                continue;
            };
            push_unique(&mut galaxy.systems, system_id.to_string());
            push_unique(&mut galaxy.pois, poi_id.to_string());
            galaxy
                .poi_system
                .insert(poi_id.to_string(), system_id.to_string());
            if let Some(base_id) = value_as_str(poi_obj, &["base_id", "baseId", "BaseId"]) {
                galaxy
                    .poi_base_to_id
                    .insert(base_id.to_string(), poi_id.to_string());
            }
            if let Some(poi_type) = value_as_str(poi_obj, &["type", "Type"]) {
                galaxy
                    .poi_type_by_id
                    .insert(poi_id.to_string(), poi_type.to_string());
            }
            let has_base = poi_obj
                .get("has_base")
                .or_else(|| poi_obj.get("hasBase"))
                .or_else(|| poi_obj.get("HasBase"))
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    value_as_str(poi_obj, &["base_id", "baseId", "BaseId"]).is_some()
                });
            if has_base {
                push_map_unique(
                    &mut galaxy.dockable_pois_by_system,
                    system_id,
                    poi_id.to_string(),
                );
                if value_as_str(poi_obj, &["type", "Type"])
                    .is_some_and(|t| t.eq_ignore_ascii_case("station"))
                {
                    push_map_unique(
                        &mut galaxy.station_pois_by_system,
                        system_id,
                        poi_id.to_string(),
                    );
                }
            }
        }
    }

    state.galaxy = Arc::new(galaxy);
}

fn enrich_from_view_storage(state: &mut GameState, value: &Value, station_id: &str) {
    let root = value.get("result").unwrap_or(value);
    let mut by_item = HashMap::new();
    if let Some(items) = root.get("items").and_then(Value::as_array) {
        for item in items {
            let item_id = item.get("item_id").and_then(Value::as_str);
            let qty = item.get("quantity").and_then(Value::as_i64);
            if let (Some(item_id), Some(qty)) = (item_id, qty) {
                by_item.insert(item_id.to_string(), qty);
            }
        }
    }
    if by_item.is_empty() {
        return;
    }
    let mut stash = state.stash.as_ref().clone();
    stash.insert(station_id.to_string(), by_item);
    state.stash = Arc::new(stash);
}

fn enrich_from_view_market(state: &mut GameState, value: &Value) {
    let root = value.get("result").unwrap_or(value);
    let Some(items) = root.get("items").and_then(Value::as_array) else {
        return;
    };

    let mut market = state.market.as_ref().clone();
    for item in items {
        let Some(item_id) = item.get("item_id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(sells) = item.get("sell_orders").and_then(Value::as_array) {
            let parsed = parse_market_order_array(sells);
            if !parsed.is_empty() {
                market.sell_orders.insert(item_id.to_string(), parsed);
            }
        }
        if let Some(buys) = item.get("buy_orders").and_then(Value::as_array) {
            let parsed = parse_market_order_array(buys);
            if !parsed.is_empty() {
                market.buy_orders.insert(item_id.to_string(), parsed);
            }
        }
    }
    state.market = Arc::new(market);
}

fn enrich_from_view_orders(state: &mut GameState, value: &Value) {
    let root = value.get("result").unwrap_or(value);
    if let Some(orders) = root.get("orders").and_then(Value::as_array) {
        let mut buy = Vec::new();
        let mut sell = Vec::new();
        for order in orders {
            let Some(parsed) = extract_open_order(order) else {
                continue;
            };
            let side = order
                .get("side")
                .or_else(|| order.get("order_type"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if side == "buy" {
                buy.push(parsed);
            } else if side == "sell" {
                sell.push(parsed);
            }
        }
        state.own_buy_orders = Arc::new(buy);
        state.own_sell_orders = Arc::new(sell);
        return;
    }

    let buy = root.get("buy_orders").or_else(|| root.get("buyOrders"));
    let sell = root.get("sell_orders").or_else(|| root.get("sellOrders"));
    if let Some(buy) = buy {
        state.own_buy_orders = Arc::new(extract_open_orders(Some(buy)));
    }
    if let Some(sell) = sell {
        state.own_sell_orders = Arc::new(extract_open_orders(Some(sell)));
    }
}

fn enrich_from_get_active_missions(state: &mut GameState, value: &Value) {
    let root = value.get("result").unwrap_or(value);
    let ids = extract_mission_ids(
        root.get("missions")
            .or_else(|| root.get("active_missions"))
            .or_else(|| root.get("activeMissions")),
    );
    if ids.is_empty() {
        return;
    }

    let mut missions = state.missions.as_ref().clone();
    missions.active = ids;
    state.missions = Arc::new(missions);
}

fn enrich_from_get_missions(state: &mut GameState, value: &Value) {
    let root = value.get("result").unwrap_or(value);
    let ids = extract_mission_ids(
        root.get("missions")
            .or_else(|| root.get("available_missions"))
            .or_else(|| root.get("availableMissions")),
    );
    if ids.is_empty() {
        return;
    }

    let mut missions = state.missions.as_ref().clone();
    missions.available = ids;
    state.missions = Arc::new(missions);
}

fn parse_market_order_array(entries: &[Value]) -> Vec<MarketOrderInfo> {
    entries
        .iter()
        .filter_map(|entry| {
            let price = entry
                .get("price_each")
                .or_else(|| entry.get("priceEach"))
                .and_then(Value::as_f64)
                .map(|p| p.floor() as i64)
                .unwrap_or_default()
                .max(1);
            let quantity = entry
                .get("quantity")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            if quantity <= 0 {
                return None;
            }
            Some(MarketOrderInfo {
                price_each: price,
                quantity,
            })
        })
        .collect()
}

fn extract_ids(value: Option<&Value>) -> Vec<String> {
    extract_named_ids(value, &["id"])
}

fn extract_mission_ids(value: Option<&Value>) -> Vec<String> {
    extract_named_ids(value, &["mission_id", "missionId", "id"])
}

fn extract_named_ids(value: Option<&Value>, keys: &[&str]) -> Vec<String> {
    let Some(Value::Array(entries)) = value else {
        return Vec::new();
    };

    entries
        .iter()
        .filter_map(|entry| match entry {
            Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
            Value::Object(map) => keys.iter().find_map(|key| {
                map.get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToOwned::to_owned)
            }),
            _ => None,
        })
        .collect()
}

fn extract_open_orders(value: Option<&Value>) -> Vec<OpenOrderInfo> {
    let Some(Value::Array(entries)) = value else {
        return Vec::new();
    };

    entries.iter().filter_map(extract_open_order).collect()
}

fn extract_open_order(entry: &Value) -> Option<OpenOrderInfo> {
    let Value::Object(map) = entry else {
        return None;
    };

    let order_id = map
        .get("order_id")
        .or_else(|| map.get("orderId"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if order_id.is_empty() {
        return None;
    }

    let item_id = map
        .get("item_id")
        .or_else(|| map.get("itemId"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if item_id.is_empty() {
        return None;
    }

    let price_each = map
        .get("price_each")
        .or_else(|| map.get("priceEach"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let quantity = map
        .get("quantity")
        .and_then(Value::as_i64)
        .unwrap_or_default();

    Some(OpenOrderInfo {
        order_id,
        item_id,
        price_each,
        quantity,
    })
}

fn extract_system_connections(value: Option<&Value>) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    let Some(Value::Array(entries)) = value else {
        return out;
    };

    for entry in entries {
        let Value::Object(system_obj) = entry else {
            continue;
        };
        let Some(system_id) = system_obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        let connections = system_obj
            .get("connections")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out.insert(system_id.to_string(), connections);
    }

    out
}

fn extract_system_coordinates(value: Option<&Value>) -> HashMap<String, (f64, f64)> {
    let mut out = HashMap::new();
    let Some(Value::Array(entries)) = value else {
        return out;
    };

    for entry in entries {
        let Value::Object(system_obj) = entry else {
            continue;
        };
        let Some(system_id) = system_obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(x) = number_as_f64(system_obj.get("x")) else {
            continue;
        };
        let Some(y) = number_as_f64(system_obj.get("y")) else {
            continue;
        };
        out.insert(system_id.to_string(), (x, y));
    }

    out
}

fn number_as_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
}

fn value_as_str<'a>(object: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| object.get(*k).and_then(Value::as_str))
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|v| v == &value) {
        values.push(value);
    }
}

fn push_map_unique(map: &mut HashMap<String, Vec<String>>, key: &str, value: String) {
    let values = map.entry(key.to_string()).or_default();
    if !values.iter().any(|v| v == &value) {
        values.push(value);
    }
}

fn extract_poi_lookups(
    systems: Option<&Value>,
    pois: Option<&Value>,
) -> (
    HashMap<String, String>,
    HashMap<String, String>,
    HashMap<String, String>,
) {
    let mut poi_system = HashMap::new();
    let mut poi_base_to_id = HashMap::new();
    let mut poi_type_by_id = HashMap::new();

    if let Some(Value::Array(system_entries)) = systems {
        for entry in system_entries {
            let Value::Object(system_obj) = entry else {
                continue;
            };
            let Some(system_id) = system_obj.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(Value::Array(poi_entries)) = system_obj.get("pois") else {
                continue;
            };
            for poi in poi_entries {
                match poi {
                    Value::String(poi_id) if !poi_id.trim().is_empty() => {
                        poi_system.insert(poi_id.clone(), system_id.to_string());
                    }
                    Value::Object(poi_obj) => {
                        if let Some(poi_id) = poi_obj.get("id").and_then(Value::as_str) {
                            poi_system.insert(poi_id.to_string(), system_id.to_string());
                            if let Some(base_id) = poi_obj
                                .get("base_id")
                                .or_else(|| poi_obj.get("baseId"))
                                .and_then(Value::as_str)
                            {
                                poi_base_to_id.insert(base_id.to_string(), poi_id.to_string());
                            }
                            if let Some(poi_type) = poi_obj.get("type").and_then(Value::as_str) {
                                poi_type_by_id.insert(poi_id.to_string(), poi_type.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(Value::Array(poi_entries)) = pois {
        for entry in poi_entries {
            let Value::Object(poi_obj) = entry else {
                continue;
            };
            let Some(poi_id) = poi_obj.get("id").and_then(Value::as_str) else {
                continue;
            };
            let system_id = poi_obj
                .get("system_id")
                .or_else(|| poi_obj.get("systemId"))
                .and_then(Value::as_str);
            if let Some(system_id) = system_id {
                poi_system.insert(poi_id.to_string(), system_id.to_string());
            }
            if let Some(base_id) = poi_obj
                .get("base_id")
                .or_else(|| poi_obj.get("baseId"))
                .and_then(Value::as_str)
            {
                poi_base_to_id.insert(base_id.to_string(), poi_id.to_string());
            }
            if let Some(poi_type) = poi_obj
                .get("type")
                .or_else(|| poi_obj.get("poi_type"))
                .or_else(|| poi_obj.get("poiType"))
                .and_then(Value::as_str)
            {
                poi_type_by_id.insert(poi_id.to_string(), poi_type.to_string());
            }
        }
    }

    (poi_system, poi_base_to_id, poi_type_by_id)
}

fn extract_pois_by_resource(result: &Value) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    let resources = result
        .get("resources")
        .or_else(|| result.get("galaxy").and_then(|g| g.get("resources")));
    let Some(resources) = resources else {
        return out;
    };
    let Some(map) = resources
        .get("pois_by_resource")
        .or_else(|| resources.get("poisByResource"))
        .and_then(Value::as_object)
    else {
        return out;
    };

    for (resource_id, poi_list) in map {
        let Some(poi_arr) = poi_list.as_array() else {
            continue;
        };
        let pois = poi_arr
            .iter()
            .filter_map(|v| v.as_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>();
        out.insert(resource_id.clone(), pois);
    }

    out
}

fn extract_exploration_sets(result: &Value) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let exploration = result
        .get("exploration")
        .or_else(|| result.get("galaxy").and_then(|g| g.get("exploration")));

    let explored_systems = extract_string_set(exploration.and_then(|e| {
        e.get("explored_systems")
            .or_else(|| e.get("exploredSystems"))
    }));
    let visited_pois = extract_string_set(
        exploration.and_then(|e| e.get("visited_pois").or_else(|| e.get("visitedPois"))),
    );
    let surveyed_systems = extract_string_set(exploration.and_then(|e| {
        e.get("surveyed_systems")
            .or_else(|| e.get("surveyedSystems"))
    }));

    (explored_systems, visited_pois, surveyed_systems)
}

fn extract_string_set(value: Option<&Value>) -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(Value::Array(items)) = value else {
        return out;
    };
    for item in items {
        if let Some(id) = item.as_str() {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                out.insert(trimmed.to_string());
            }
        }
    }
    out
}

fn extract_dockable_and_station_pois(
    systems: Option<&Value>,
) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    let mut dockable = HashMap::new();
    let mut station = HashMap::new();
    let Some(Value::Array(entries)) = systems else {
        return (dockable, station);
    };

    for entry in entries {
        let Value::Object(system_obj) = entry else {
            continue;
        };
        let Some(system_id) = system_obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(Value::Array(pois)) = system_obj.get("pois") else {
            continue;
        };
        let mut dockable_ids = Vec::new();
        let mut station_ids = Vec::new();
        for poi in pois {
            let Value::Object(poi_obj) = poi else {
                continue;
            };
            let Some(poi_id) = poi_obj.get("id").and_then(Value::as_str) else {
                continue;
            };
            let has_base = poi_obj
                .get("has_base")
                .or_else(|| poi_obj.get("hasBase"))
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    poi_obj
                        .get("base_id")
                        .or_else(|| poi_obj.get("baseId"))
                        .and_then(Value::as_str)
                        .is_some()
                });
            if !has_base {
                continue;
            }
            dockable_ids.push(poi_id.to_string());
            let poi_type = poi_obj
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if poi_type.eq_ignore_ascii_case("station") {
                station_ids.push(poi_id.to_string());
            }
        }
        dockable.insert(system_id.to_string(), dockable_ids);
        station.insert(system_id.to_string(), station_ids);
    }

    (dockable, station)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_map(value: &Value) -> &serde_json::Map<String, Value> {
        value.as_object().expect("expected object")
    }

    #[test]
    fn args_to_payload_uses_named_command_schema() {
        let payload = args_to_payload(&[CommandArg::MissionId("m_1".to_string())], &["mission_id"])
            .expect("payload");
        let map = object_map(&payload);
        assert_eq!(
            map.get("mission_id"),
            Some(&Value::String("m_1".to_string()))
        );
    }

    #[test]
    fn args_to_payload_encodes_integers_as_numbers() {
        let payload = args_to_payload(
            &[
                CommandArg::ShipId("ship_1".to_string()),
                CommandArg::Integer(1200),
            ],
            &["ship_id", "price"],
        )
        .expect("payload");
        let map = object_map(&payload);
        assert_eq!(
            map.get("ship_id"),
            Some(&Value::String("ship_1".to_string()))
        );
        assert_eq!(
            map.get("price"),
            Some(&Value::Number(serde_json::Number::from(1200)))
        );
    }

    #[test]
    fn map_command_spec_uses_csharp_api_name_for_survey() {
        let spec = map_command_spec("survey").expect("spec");
        assert_eq!(spec.api_action, "survey_system");
    }

    #[test]
    fn map_command_spec_maps_buy_ship_to_buy_listed_ship() {
        let spec = map_command_spec("buy_ship").expect("spec");
        assert_eq!(spec.api_action, "buy_listed_ship");
        assert_eq!(spec.payload_keys, &["listing_id"]);
    }

    #[test]
    fn map_command_spec_rejects_high_level_go() {
        let err = map_command_spec("go").expect_err("expected unsupported");
        assert!(err
            .to_string()
            .contains("requires command-engine orchestration"));
    }

    #[test]
    fn map_command_spec_rejects_high_level_refuel() {
        let err = map_command_spec("refuel").expect_err("expected unsupported");
        assert!(err
            .to_string()
            .contains("requires command-engine orchestration"));
    }

    #[test]
    fn map_status_to_game_state_extracts_open_orders() {
        let status = serde_json::json!({
            "result": {
                "own_buy_orders": [
                    { "order_id": "ob_1", "item_id": "iron", "price_each": 12.0, "quantity": 4 }
                ],
                "own_sell_orders": [
                    { "orderId": "os_1", "itemId": "water", "priceEach": 3.0, "quantity": 2 }
                ]
            }
        });

        let state = map_status_to_game_state(&status);
        assert_eq!(state.own_buy_orders.len(), 1);
        assert_eq!(state.own_buy_orders[0].order_id, "ob_1");
        assert_eq!(state.own_buy_orders[0].item_id, "iron");
        assert_eq!(state.own_sell_orders.len(), 1);
        assert_eq!(state.own_sell_orders[0].order_id, "os_1");
        assert_eq!(state.own_sell_orders[0].item_id, "water");
    }

    #[test]
    fn map_status_to_game_state_extracts_cargo_array() {
        let status = serde_json::json!({
            "result": {
                "ship": {
                    "cargo": [
                        { "item_id": "iron", "quantity": 3 },
                        { "item_id": "water", "quantity": 1 }
                    ]
                }
            }
        });

        let state = map_status_to_game_state(&status);
        assert_eq!(state.cargo.get("iron"), Some(&3));
        assert_eq!(state.cargo.get("water"), Some(&1));
    }

    #[test]
    fn enrich_from_get_active_missions_uses_result_missions_array() {
        let mut state = GameState::default();
        let payload = serde_json::json!({
            "result": {
                "missions": [
                    { "mission_id": "m_active_1" }
                ]
            }
        });

        enrich_from_get_active_missions(&mut state, &payload);
        assert_eq!(state.missions.active, vec!["m_active_1".to_string()]);
    }

    #[test]
    fn enrich_from_get_missions_uses_result_missions_array() {
        let mut state = GameState::default();
        let payload = serde_json::json!({
            "result": {
                "missions": [
                    { "mission_id": "m_avail_1" }
                ]
            }
        });

        enrich_from_get_missions(&mut state, &payload);
        assert_eq!(state.missions.available, vec!["m_avail_1".to_string()]);
    }

    #[test]
    fn extract_mission_ids_accepts_camel_case_and_string_entries() {
        let value = serde_json::json!([
            { "missionId": "m_1" },
            "m_2"
        ]);

        assert_eq!(
            extract_mission_ids(Some(&value)),
            vec!["m_1".to_string(), "m_2".to_string()]
        );
    }

    #[test]
    fn map_status_to_game_state_reads_current_system() {
        let status = serde_json::json!({
            "result": {
                "player": {
                    "current_system": "sol"
                }
            }
        });

        let state = map_status_to_game_state(&status);
        assert_eq!(state.system.as_deref(), Some("sol"));
    }

    #[test]
    fn enrich_from_view_orders_parses_orders_array_by_side() {
        let mut state = GameState::default();
        let orders = serde_json::json!({
            "result": {
                "orders": [
                    { "order_id": "ob_1", "item_id": "iron", "price_each": 12, "quantity": 4, "side": "buy" },
                    { "order_id": "os_1", "item_id": "water", "price_each": 3, "quantity": 2, "side": "sell" }
                ]
            }
        });

        enrich_from_view_orders(&mut state, &orders);
        assert_eq!(state.own_buy_orders.len(), 1);
        assert_eq!(state.own_buy_orders[0].order_id, "ob_1");
        assert_eq!(state.own_sell_orders.len(), 1);
        assert_eq!(state.own_sell_orders[0].order_id, "os_1");
    }

    #[test]
    fn enrich_from_get_system_uses_from_system_when_in_transit() {
        let mut state = GameState::default();
        let transit = serde_json::json!({
            "result": {
                "action": "get_system",
                "in_transit": true,
                "transit_type": "jump",
                "from_system": "sol",
                "to_system": "alpha",
                "ticks_remaining": 2,
                "message": "In transit"
            }
        });

        enrich_from_get_system(&mut state, &transit);
        assert_eq!(state.system.as_deref(), Some("sol"));
    }

    #[test]
    fn enrich_from_get_poi_populates_resources_for_current_poi() {
        let mut state = GameState {
            current_poi: Some("main_belt".to_string()),
            ..GameState::default()
        };
        let poi = serde_json::json!({
            "result": {
                "poi": {
                    "id": "main_belt",
                    "system_id": "sol",
                    "type": "asteroid_belt"
                },
                "resources": [
                    { "resource_id": "iron", "richness": 3, "remaining": 5000 },
                    { "resource_id": "water_ice", "richness": 2, "remaining": 2500 }
                ]
            }
        });

        enrich_from_get_poi(&mut state, &poi);

        assert_eq!(
            state.galaxy.pois_by_resource.get("iron"),
            Some(&vec!["main_belt".to_string()])
        );
        assert_eq!(
            state.galaxy.pois_by_resource.get("water_ice"),
            Some(&vec!["main_belt".to_string()])
        );
        assert_eq!(
            state.galaxy.poi_system.get("main_belt").map(String::as_str),
            Some("sol")
        );
    }

    #[test]
    fn enrich_from_get_map_populates_system_and_poi_lookup_data() {
        let mut state = GameState::default();
        let map = serde_json::json!({
            "result": {
                "map": {
                    "systems": [
                        {
                            "system_id": "sol",
                            "x": 1.5,
                            "y": 2.5,
                            "connections": [{ "system_id": "alpha" }],
                            "pois": [
                                {
                                    "id": "poi_sol_station",
                                    "type": "station",
                                    "base_id": "base_sol_station"
                                }
                            ]
                        }
                    ]
                }
            }
        });

        enrich_from_get_map(&mut state, &map);

        assert!(state.galaxy.systems.iter().any(|s| s == "sol"));
        assert_eq!(
            state.galaxy.system_coordinates.get("sol"),
            Some(&(1.5_f64, 2.5_f64))
        );
        assert_eq!(
            state.galaxy.system_connections.get("sol"),
            Some(&vec!["alpha".to_string()])
        );
        assert_eq!(
            state.galaxy.poi_system.get("poi_sol_station"),
            Some(&"sol".to_string())
        );
        assert_eq!(
            state.galaxy.poi_base_to_id.get("base_sol_station"),
            Some(&"poi_sol_station".to_string())
        );
    }

    #[test]
    fn enrich_from_get_map_maps_known_poi_base_ids() {
        let mut state = GameState::default();
        let map = serde_json::json!({
            "result": {
                "known_pois": [
                    {
                        "id": "poi_known_station",
                        "system_id": "beta",
                        "base_id": "base_known_station",
                        "type": "station"
                    }
                ]
            }
        });

        enrich_from_get_map(&mut state, &map);

        assert!(state.galaxy.systems.iter().any(|s| s == "beta"));
        assert_eq!(
            state.galaxy.poi_system.get("poi_known_station"),
            Some(&"beta".to_string())
        );
        assert_eq!(
            state.galaxy.poi_base_to_id.get("base_known_station"),
            Some(&"poi_known_station".to_string())
        );
    }

    #[test]
    fn orchestrator_does_not_call_status_api_directly() {
        let source = include_str!("transport/orchestrator.rs");
        assert!(!source.contains("get_status"));
        assert!(!source.contains("fetch_state("));
    }
}
