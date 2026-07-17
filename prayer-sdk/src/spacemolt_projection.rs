use std::collections::HashMap;
use std::sync::Arc;

use prayer_runtime::engine::{
    FactionGarageInfo, FactionGarageShipObservation, GalaxyData, MarketOrder, PoiInfoData,
    StationMarketData,
};
use prayer_runtime::snapshot::{BotObservation, StateObservation, WorldObservation};
use prayer_state::{BotState, PassengerBerthView};

/// Prayer-side command envelope. Keeping the protocol result alive here lets
/// observation projection see typed query content and mutation details before
/// PrayerLang receives the historical flattened JSON payload.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutedSpacemoltCommand {
    pub tool: String,
    pub action: String,
    pub result: spacemolt_lib_rs::CommandResult,
}

impl ExecutedSpacemoltCommand {
    pub fn runtime_value(self) -> serde_json::Value {
        self.result.into_value()
    }

    fn observation_value(&self) -> &serde_json::Value {
        match &self.result {
            spacemolt_lib_rs::CommandResult::Query(result) => {
                result.structured_content.as_ref().unwrap_or(&result.result)
            }
            spacemolt_lib_rs::CommandResult::Mutation(result) => &result.delta,
        }
    }
}

/// The single command-observation adapter facade. Action dispatch remains in
/// one place and each projector validates against the generated response DTO
/// before translating it into Prayer's observation model.
pub fn project_executed_command(
    command: &ExecutedSpacemoltCommand,
) -> Result<Vec<StateObservation>, serde_json::Error> {
    if command.tool != "spacemolt" {
        return Ok(Vec::new());
    }
    let value = command.observation_value();
    let galaxy = match command.action.as_str() {
        "get_system" => {
            let response = serde_json::from_value(value.clone())?;
            project_get_system_galaxy(response)
        }
        "get_map" => {
            let response = serde_json::from_value(value.clone())?;
            project_get_map_galaxy(response)
        }
        "get_poi" => {
            let response = serde_json::from_value(value.clone())?;
            project_get_poi_galaxy(response)
        }
        "survey_system" => {
            let response = serde_json::from_value(mutation_details(value).clone())?;
            project_survey_system_galaxy(response)
        }
        _ => None,
    };
    Ok(galaxy
        .map(|galaxy| StateObservation {
            world: WorldObservation {
                galaxy: Arc::new(galaxy),
                ..WorldObservation::default()
            },
            ..StateObservation::default()
        })
        .into_iter()
        .collect())
}

fn mutation_details(value: &serde_json::Value) -> &serde_json::Value {
    value.get("details").unwrap_or(value)
}

/// Project location-bearing status snapshots and subscription updates through
/// the same observation shape used by command results.
pub fn project_location_update(
    value: &serde_json::Value,
    observed_at_unix: i64,
) -> Option<StateObservation> {
    let location = value.get("location").unwrap_or(value);
    let system_id = location.get("system_id")?.as_str()?;
    let poi_id = location.get("poi_id").and_then(|value| value.as_str());
    let mut galaxy = GalaxyData::default();
    galaxy.system_records.insert(
        system_id.to_string(),
        prayer_state::SystemKnowledge {
            id: system_id.to_string(),
            first_entered_unix: Some(observed_at_unix),
            last_entered_unix: Some(observed_at_unix),
            observed_at_unix,
            ..Default::default()
        },
    );
    if let Some(poi_id) = poi_id.filter(|_| {
        !location
            .get("in_transit")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }) {
        galaxy.poi_records.insert(
            poi_id.to_string(),
            prayer_state::PoiKnowledge {
                id: poi_id.to_string(),
                system_id: system_id.to_string(),
                info: PoiInfoData {
                    id: poi_id.to_string(),
                    system_id: system_id.to_string(),
                    ..Default::default()
                },
                resources: resources_from_json(location.get("resources")),
                first_discovered_unix: Some(observed_at_unix),
                last_observed_unix: Some(observed_at_unix),
                first_visited_unix: Some(observed_at_unix),
                last_visited_unix: Some(observed_at_unix),
                ..Default::default()
            },
        );
    }
    Some(StateObservation {
        status_system: Some(system_id.to_string()),
        status_poi: poi_id.map(str::to_string),
        world: WorldObservation {
            galaxy: Arc::new(galaxy),
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Debug, Clone, Default)]
pub struct ProjectedState {
    pub bot: BotState,
    pub world: WorldObservation,
    pub market_base_id: Option<String>,
}

impl ProjectedState {
    pub fn into_observation(self) -> StateObservation {
        StateObservation {
            status_system: self.bot.location.system_id.clone(),
            status_poi: self.bot.location.poi_id.clone(),
            bot: BotObservation { state: self.bot },
            world: self.world,
            ..StateObservation::default()
        }
    }
}

pub fn project_account_state(state: &spacemolt_lib_rs::state::StateCache) -> ProjectedState {
    let player = state.player().ok().flatten().unwrap_or_default();
    let ship = state.ship().ok().flatten().unwrap_or_default();
    let location = state.location().ok().flatten().unwrap_or_default();
    let cargo_items = state.cargo().ok().flatten().unwrap_or_default();
    let skills = state.skills().ok().flatten().unwrap_or_default();
    let modules = state.modules().ok().flatten().unwrap_or_default();
    let installed_modules = modules
        .iter()
        .filter_map(|module| module.type_id.clone())
        .collect::<Vec<_>>();
    let missions = state.missions().ok().flatten().unwrap_or_default();

    let fuel = ship.fuel.unwrap_or_default();
    let max_fuel = ship.max_fuel.unwrap_or(100).max(1);
    let cargo_used = ship.cargo_used.unwrap_or_default();
    let cargo_capacity = ship.cargo_capacity.unwrap_or(100).max(1);
    let mut cargo = HashMap::new();
    if cargo_used > 0 {
        for item in &cargo_items {
            if let (Some(item_id), Some(quantity)) = (&item.item_id, item.quantity) {
                cargo.insert(item_id.clone(), quantity);
            }
        }
    }

    let market_base_id = location.docked_at.clone().or_else(|| {
        location
            .poi_type
            .as_deref()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("station"))
            .then(|| location.poi_id.clone())
            .flatten()
    });
    let mut projected = ProjectedState {
        bot: BotState {
            fuel,
            max_fuel,
            fuel_pct: ((fuel * 100) / max_fuel).clamp(0, 100),
            cargo_used,
            cargo_capacity,
            cargo_pct: ((cargo_used * 100) / cargo_capacity).clamp(0, 100),
            cargo: Arc::new(cargo),
            cargo_items: Arc::new(cargo_items),
            player,
            ship,
            location,
            skills: Arc::new(skills),
            modules: Arc::new(modules),
            installed_modules: Arc::new(installed_modules),
            missions: Arc::new(prayer_state::MissionData {
                active: missions
                    .active
                    .iter()
                    .filter_map(|mission| mission.mission_id.clone())
                    .collect(),
                active_details: missions.active,
                ..prayer_state::MissionData::default()
            }),
            ..BotState::default()
        },
        market_base_id,
        ..ProjectedState::default()
    };
    project_current_location_into_galaxy(&mut projected);
    projected
}

/// Project the durable galaxy facts returned by `spacemolt/get_system`.
/// Query results are not game-state sections, so callers must merge this
/// observation when the response arrives rather than waiting for StateCache.
pub fn project_get_system_galaxy(
    response: spacemolt_lib_rs::schema::GetSystemResponse,
) -> Option<GalaxyData> {
    let value = serde_json::to_value(response).ok()?;
    project_get_system_json(&value)
}

fn project_get_system_json(value: &serde_json::Value) -> Option<GalaxyData> {
    let payload = value.get("result").unwrap_or(value);
    let system = payload.get("system")?.as_object()?;
    let system_id = system.get("id")?.as_str()?.trim();
    if system_id.is_empty() {
        return None;
    }
    let observed_at = observation_unix();
    let connections = system
        .get("connections")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("system_id").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let poi_values = system
        .get("pois")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut galaxy = GalaxyData::default();
    galaxy.system_records.insert(
        system_id.to_string(),
        prayer_state::SystemKnowledge {
            id: system_id.to_string(),
            name: system
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            connections: connections.clone(),
            connections_complete: true,
            empire: system
                .get("empire")
                .and_then(serde_json::Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            is_stronghold: system
                .get("is_stronghold")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            stronghold_observed: system.get("is_stronghold").is_some(),
            poi_count: Some(poi_values.len()),
            pois_complete: true,
            last_scanned_unix: Some(observed_at),
            observed_at_unix: observed_at,
            ..Default::default()
        },
    );
    for neighbor in connections {
        galaxy
            .system_records
            .entry(neighbor.clone())
            .or_insert_with(|| prayer_state::SystemKnowledge {
                id: neighbor,
                ..Default::default()
            });
    }
    for poi in &poi_values {
        if let Some(record) = poi_record_from_json(system_id, poi, observed_at, false) {
            galaxy.poi_records.insert(record.id.clone(), record);
        }
    }
    Some(galaxy)
}

pub fn project_get_map_galaxy(
    response: spacemolt_lib_rs::schema::GetMapResponse,
) -> Option<GalaxyData> {
    let systems = response.systems;
    let observed_at = observation_unix();
    let mut galaxy = GalaxyData::default();
    for system in systems {
        let id = system.system_id.trim();
        if id.is_empty() {
            continue;
        }
        let coordinates = Some((system.position.x, system.position.y));
        galaxy.system_records.insert(
            id.to_string(),
            prayer_state::SystemKnowledge {
                id: id.to_string(),
                name: Some(system.name),
                coordinates,
                connections: system.connections,
                connections_complete: true,
                empire: system.empire.filter(|v| !v.is_empty()),
                is_stronghold: system.is_stronghold.unwrap_or(false),
                stronghold_observed: system.is_stronghold.is_some(),
                poi_count: usize::try_from(system.poi_count).ok(),
                first_entered_unix: system.visited.then_some(observed_at),
                last_entered_unix: system.visited.then_some(observed_at),
                observed_at_unix: observed_at,
                ..Default::default()
            },
        );
    }
    Some(galaxy)
}

pub fn project_get_poi_galaxy(
    response: spacemolt_lib_rs::schema::GetPoiResponse,
) -> Option<GalaxyData> {
    let value = serde_json::to_value(response).ok()?;
    project_get_poi_json(&value)
}

fn project_get_poi_json(value: &serde_json::Value) -> Option<GalaxyData> {
    let payload = value.get("result").unwrap_or(value);
    let poi = payload.get("poi").unwrap_or(payload);
    let system_id = poi
        .get("system_id")
        .or_else(|| payload.get("system_id"))?
        .as_str()?;
    let observed_at = observation_unix();
    let mut record = poi_record_from_json(system_id, poi, observed_at, false)?;
    let resources = payload.get("resources").or_else(|| poi.get("resources"));
    record.resources = resources_from_json(resources);
    record.resources_complete = true;
    let mut galaxy = GalaxyData::default();
    galaxy.system_records.insert(
        system_id.to_string(),
        prayer_state::SystemKnowledge {
            id: system_id.to_string(),
            observed_at_unix: observed_at,
            ..Default::default()
        },
    );
    galaxy.poi_records.insert(record.id.clone(), record);
    Some(galaxy)
}

pub fn project_survey_system_galaxy(
    response: spacemolt_lib_rs::schema::SurveySystemResponse,
) -> Option<GalaxyData> {
    let value = serde_json::to_value(response).ok()?;
    let details = &value;
    let system_id = details.get("system_id")?.as_str()?;
    let observed_at = observation_unix();
    let mut galaxy = GalaxyData::default();
    galaxy.system_records.insert(
        system_id.to_string(),
        prayer_state::SystemKnowledge {
            id: system_id.to_string(),
            name: details
                .get("system_name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            last_surveyed_unix: Some(observed_at),
            bloom_status: details
                .get("bloom_status")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            bloom_intensity: details
                .get("bloom_intensity")
                .and_then(serde_json::Value::as_f64),
            faint_signatures: details
                .get("faint_signatures")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default(),
            wildlife: details
                .get("wildlife")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default(),
            survey_complete: true,
            observed_at_unix: observed_at,
            ..Default::default()
        },
    );
    for key in ["newly_revealed", "already_revealed"] {
        for poi in details
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(mut record) = poi_record_from_json(system_id, poi, observed_at, false) {
                record.resources = resources_from_json(poi.get("resources"));
                record.resources_complete = true;
                galaxy.poi_records.insert(record.id.clone(), record);
            }
        }
    }
    Some(galaxy)
}

fn observation_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn resources_from_json(
    resources: Option<&serde_json::Value>,
) -> Vec<prayer_runtime::engine::PoiResourceData> {
    resources
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|resource| {
            Some(prayer_runtime::engine::PoiResourceData {
                resource_id: resource
                    .get("resource_id")
                    .or_else(|| resource.get("item_id"))?
                    .as_str()?
                    .to_string(),
                name: resource
                    .get("name")
                    .or_else(|| resource.get("item_name"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                richness_text: resource
                    .get("richness")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                richness: resource.get("richness").and_then(serde_json::Value::as_i64),
                remaining: resource
                    .get("remaining")
                    .and_then(serde_json::Value::as_i64),
                remaining_display: resource
                    .get("remaining_display")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect()
}

fn poi_record_from_json(
    system_id: &str,
    poi: &serde_json::Value,
    observed_at: i64,
    visited: bool,
) -> Option<prayer_state::PoiKnowledge> {
    let id = poi.get("id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let position = poi.get("position");
    let base_id = poi
        .get("base_id")
        .and_then(serde_json::Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let info = PoiInfoData {
        id: id.to_string(),
        name: poi
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(id)
            .to_string(),
        system_id: system_id.to_string(),
        poi_type: poi
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        class_name: poi
            .get("class")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: poi
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        hidden: poi
            .get("hidden")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        x: position
            .and_then(|value| value.get("x"))
            .and_then(serde_json::Value::as_f64),
        y: position
            .and_then(|value| value.get("y"))
            .and_then(serde_json::Value::as_f64),
        has_base: poi
            .get("has_base")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(base_id.is_some()),
        base_id,
        base_name: poi
            .get("base_name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        online: poi.get("online").and_then(serde_json::Value::as_i64),
        fuel_reserve: poi.get("fuel_reserve").and_then(serde_json::Value::as_i64),
        fuel_capacity: poi.get("fuel_capacity").and_then(serde_json::Value::as_i64),
        fuel_price: poi.get("fuel_price").and_then(serde_json::Value::as_i64),
        faction_fuel_reserve: poi
            .get("faction_fuel_reserve")
            .and_then(serde_json::Value::as_i64),
        faction_fuel_capacity: poi
            .get("faction_fuel_capacity")
            .and_then(serde_json::Value::as_i64),
    };
    Some(prayer_state::PoiKnowledge {
        id: id.to_string(),
        system_id: system_id.to_string(),
        info,
        info_complete: true,
        resources: resources_from_json(poi.get("resources")),
        first_discovered_unix: Some(observed_at),
        last_observed_unix: Some(observed_at),
        first_visited_unix: visited.then_some(observed_at),
        last_visited_unix: visited.then_some(observed_at),
        ..Default::default()
    })
}

fn project_current_location_into_galaxy(state: &mut ProjectedState) {
    let Some(system_id) = state
        .bot
        .location
        .system_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return;
    };
    let observed_at = observation_unix();
    let location_resources = serde_json::to_value(&state.bot.location)
        .ok()
        .map(|location| resources_from_json(location.get("resources")))
        .unwrap_or_default();
    let mut galaxy = state.world.galaxy.as_ref().clone();
    let system = galaxy
        .system_records
        .entry(system_id.to_string())
        .or_default();
    if system.id.is_empty() {
        system.id = system_id.to_string();
    }
    system.first_entered_unix = Some(
        system
            .first_entered_unix
            .unwrap_or(observed_at)
            .min(observed_at),
    );
    system.last_entered_unix = Some(
        system
            .last_entered_unix
            .unwrap_or(observed_at)
            .max(observed_at),
    );
    system.observed_at_unix = system.observed_at_unix.max(observed_at);
    if !state.bot.location.in_transit.unwrap_or(false) {
        if let Some(poi_id) = state
            .bot
            .location
            .poi_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            let poi = galaxy.poi_records.entry(poi_id.to_string()).or_default();
            if poi.id.is_empty() {
                poi.id = poi_id.to_string();
                poi.system_id = system_id.to_string();
                poi.info.id = poi_id.to_string();
                poi.info.system_id = system_id.to_string();
            }
            if !location_resources.is_empty() {
                poi.resources = location_resources;
            }
            poi.first_discovered_unix = Some(
                poi.first_discovered_unix
                    .unwrap_or(observed_at)
                    .min(observed_at),
            );
            poi.last_observed_unix = Some(
                poi.last_observed_unix
                    .unwrap_or(observed_at)
                    .max(observed_at),
            );
            poi.first_visited_unix = Some(
                poi.first_visited_unix
                    .unwrap_or(observed_at)
                    .min(observed_at),
            );
            poi.last_visited_unix = Some(
                poi.last_visited_unix
                    .unwrap_or(observed_at)
                    .max(observed_at),
            );
            if let Some(base_id) = state
                .bot
                .location
                .docked_at
                .as_deref()
                .filter(|v| !v.is_empty())
            {
                poi.info.has_base = true;
                poi.info.base_id = Some(base_id.to_string());
                if poi.info.poi_type.is_empty() {
                    poi.info.poi_type = "station".to_string();
                }
            }
        }
    }
    galaxy.invalidate_routes();
    state.world.galaxy = Arc::new(galaxy);
}

pub fn project_aboard_passengers(
    state: &mut ProjectedState,
    response: spacemolt_lib_rs::commands::ListPassengersResponse,
) {
    state.bot.passengers.aboard_count = Some(response.count);
    let berth_text = |berth: &spacemolt_lib_rs::schema::BerthCount| {
        format!("{}/{}", berth.total.saturating_sub(berth.free), berth.total)
    };
    if let Some(berths) = &response.berths {
        state.bot.passengers.economy_berths_raw = berth_text(&berths.economy);
        state.bot.passengers.business_berths_raw = berth_text(&berths.business);
        state.bot.passengers.first_berths_raw = berth_text(&berths.first);
        state.bot.passengers.economy_berths =
            parse_berths(&state.bot.passengers.economy_berths_raw);
        state.bot.passengers.business_berths =
            parse_berths(&state.bot.passengers.business_berths_raw);
        state.bot.passengers.first_berths = parse_berths(&state.bot.passengers.first_berths_raw);
    }
    state.bot.passengers.aboard = Arc::new(response.passengers);
}

pub fn project_station_passengers(
    state: &mut ProjectedState,
    response: spacemolt_lib_rs::commands::StationPassengersResponse,
) {
    state.world.passengers.station = response.station;
    state.world.passengers.waiting_count = Some(response.count);
    state.world.passengers.waiting = Arc::new(response.waiting);
}

fn parse_berths(value: &str) -> PassengerBerthView {
    value.parse().unwrap_or_default()
}

pub fn project_faction_garages(
    response: spacemolt_lib_rs::commands::FactionGaragesResponse,
) -> FactionGarageInfo {
    let mut capacity = 0i64;
    let mut ships = Vec::new();
    for station in response.stations {
        capacity = capacity.saturating_add(station.capacity);
        for ship in station.ships {
            ships.push(FactionGarageShipObservation {
                base_id: station.base_id.clone(),
                base_name: station.base_name.clone().unwrap_or_default(),
                system_name: station.system_name.clone().unwrap_or_default(),
                ship,
            });
        }
    }
    ships.sort_by(|a, b| {
        a.base_id
            .cmp(&b.base_id)
            .then_with(|| a.ship.class_name.cmp(&b.ship.class_name))
            .then_with(|| a.ship.class_id.cmp(&b.ship.class_id))
            .then_with(|| a.ship.ship_id.cmp(&b.ship.ship_id))
    });
    FactionGarageInfo {
        used: Some(response.total_ships),
        capacity: Some(capacity),
        ships: Arc::new(ships),
    }
}

pub fn project_market_book_from_client(
    state: &mut ProjectedState,
    book: &spacemolt_lib_rs::state::MarketBook,
) {
    if book.base_id.trim().is_empty() {
        return;
    }

    let mut station = StationMarketData {
        observed_at_unix: Some(chrono::Utc::now().timestamp()),
        current_tick: Some(book.tick),
        ..StationMarketData::default()
    };

    for item in book.items.values() {
        project_market_item(&mut station, item);
    }

    // Shared market snapshots are keyed by the POI id (the actor's canonical
    // location) so consumer lookups by `current_poi` resolve; the raw base id
    // is only the client-side market-cache key. The book always describes the
    // station the actor is currently docked at.
    let station_key = state
        .bot
        .location
        .poi_id
        .clone()
        .unwrap_or_else(|| book.base_id.clone());
    let is_current_station = state.market_base_id.as_deref() == Some(book.base_id.as_str());
    let mut market = state.world.market.as_ref().clone();
    market.station_markets.insert(station_key, station.clone());
    if is_current_station {
        market.buy_orders = station.buy_orders.clone();
        market.sell_orders = station.sell_orders.clone();
    }
    state.world.market = Arc::new(market);
}

fn project_market_item(
    station: &mut StationMarketData,
    item: &spacemolt_lib_rs::state::MarketItem,
) {
    use spacemolt_lib_rs::state::MarketItem;

    let (item_id, buy_orders, sell_orders) = match item {
        MarketItem::Snapshot(item) => (
            &item.item_id,
            item.buy_orders
                .iter()
                .map(|order| MarketOrder {
                    price_each: order.price_each,
                    quantity: order.quantity,
                    source: order.source.clone(),
                    my_quantity: None,
                })
                .collect::<Vec<_>>(),
            item.sell_orders
                .iter()
                .map(|order| MarketOrder {
                    price_each: order.price_each,
                    quantity: order.quantity,
                    source: order.source.clone(),
                    my_quantity: None,
                })
                .collect::<Vec<_>>(),
        ),
        MarketItem::Update(item) => (
            &item.item_id,
            item.buy_orders
                .iter()
                .map(|order| MarketOrder {
                    price_each: order.price_each,
                    quantity: order.quantity,
                    source: order.source.clone(),
                    my_quantity: None,
                })
                .collect::<Vec<_>>(),
            item.sell_orders
                .iter()
                .map(|order| MarketOrder {
                    price_each: order.price_each,
                    quantity: order.quantity,
                    source: order.source.clone(),
                    my_quantity: None,
                })
                .collect::<Vec<_>>(),
        ),
    };
    if !buy_orders.is_empty() {
        station.buy_orders.insert(item_id.clone(), buy_orders);
    }
    if !sell_orders.is_empty() {
        station.sell_orders.insert(item_id.clone(), sell_orders);
    }
}

pub fn project_observation_view_from_client(
    state: &mut ProjectedState,
    view: &spacemolt_lib_rs::state::ObservationView,
) {
    if let Some(poi_id) = view
        .poi_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        state.bot.location.poi_id = Some(poi_id.to_string());
    }
    state.bot.observation_nearby = Arc::new(view.nearby.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use serde_json::json;
    use spacemolt_lib_rs::state::{MarketBook, ObservationView, ObservedPlayer};

    #[test]
    fn projects_core_status_from_account_sections() {
        let mut cache = spacemolt_lib_rs::state::StateCache::default();
        cache.seed(&json!({
            "player": {
                "id": "player_1",
                "username": "Scout",
                "credits": 1234,
                "home_base": "earth_station"
            },
            "ship": {
                "id": "ship_1",
                "fuel": 80,
                "max_fuel": 100,
                "cargo_used": 4,
                "cargo_capacity": 20,
                "hull": 90,
                "max_hull": 100
            },
            "location": {
                "system_id": "sol",
                "poi_id": "earth_station",
                "docked_at": "earth_station",
                "poi_type": "station",
                "resources": [
                    {"item_id": "iron_ore", "item_name": "Iron Ore"},
                    {"item_id": "copper_ore", "item_name": "Copper Ore"}
                ]
            },
            "cargo": [{ "item_id": "iron_ore", "quantity": 7 }]
        }));
        let state = project_account_state(&cache);

        assert_eq!(state.bot.player.username.as_deref(), Some("Scout"));
        assert_eq!(state.bot.player.id.as_deref(), Some("player_1"));
        assert_eq!(state.bot.player.credits, Some(1234));
        assert_eq!(state.bot.location.system_id.as_deref(), Some("sol"));
        assert_eq!(state.bot.location.poi_id.as_deref(), Some("earth_station"));
        assert!(state.bot.location.docked_at.is_some());
        assert_eq!(state.bot.fuel, 80);
        assert_eq!(state.bot.max_fuel, 100);
        assert_eq!(state.bot.cargo.get("iron_ore"), Some(&7));
        let location_poi = state
            .world
            .galaxy
            .poi_records
            .get("earth_station")
            .expect("location POI");
        assert_eq!(location_poi.system_id, "sol");
        assert_eq!(location_poi.info.base_id.as_deref(), Some("earth_station"));
        assert_eq!(location_poi.info.poi_type, "station");
        assert_eq!(location_poi.resources.len(), 2);
        assert_eq!(location_poi.resources[0].resource_id, "iron_ore");
        assert_eq!(location_poi.resources[1].resource_id, "copper_ore");
    }

    #[test]
    fn projects_skills_and_modules_as_generated_types() {
        let mut cache = spacemolt_lib_rs::state::StateCache::default();
        cache.seed(&json!({
            "player": { "id": "player_1", "username": "Scout" },
            "skills": {
                "mining": { "name": "Mining", "level": 4, "xp": 120, "nextLevelXp": 200 }
            },
            "modules": [
                { "module_id": "mod_1", "type_id": "afterburner" }
            ]
        }));
        let state = project_account_state(&cache);

        assert_eq!(
            state.bot.skills.get("mining").and_then(|skill| skill.level),
            Some(4)
        );
        assert_eq!(state.bot.modules.len(), 1);
        assert_eq!(state.bot.modules[0].module_id.as_deref(), Some("mod_1"));
        assert_eq!(state.bot.installed_modules.as_ref(), &["afterburner"]);
    }

    #[test]
    fn projects_aboard_and_waiting_passengers() {
        let mut state = ProjectedState::default();
        let aboard = serde_json::from_value(serde_json::json!({
            "count": 1,
            "berths": {
                "economy": { "free": 3, "total": 4 },
                "business": { "free": 2, "total": 2 },
                "first": { "free": 1, "total": 1 }
            },
            "passengers": [{
                "base_fare": 120,
                "bio": "Explorer",
                "citizen_id": "citizen_1",
                "class": "economy",
                "destination": "mars_station",
                "destination_name": "Mars Station",
                "destination_system": "sol",
                "name": "Ada",
                "speed_bonus": 20,
                "ticks_remaining": 30
            }]
        }))
        .expect("aboard response");
        project_aboard_passengers(&mut state, aboard);

        let waiting = serde_json::from_value(serde_json::json!({
            "count": 1,
            "demand_level": "normal",
            "fare_surge": 1.0,
            "market_conditions": "Stable",
            "station": "earth_station",
            "waiting": [{
                "bio": "Engineer",
                "citizen_id": "citizen_2",
                "citizenship": "core",
                "class": "business",
                "destination": "mars_station",
                "destination_name": "Mars Station",
                "destination_system": "sol",
                "estimated_fare": 240,
                "name": "Grace"
            }]
        }))
        .expect("station response");
        project_station_passengers(&mut state, waiting);

        assert_eq!(state.bot.passengers.aboard_count, Some(1));
        assert_eq!(state.bot.passengers.economy_berths.current, 1);
        assert_eq!(state.bot.passengers.economy_berths.max, 4);
        assert_eq!(state.bot.passengers.business_berths.max, 2);
        assert_eq!(state.bot.passengers.aboard[0].name, "Ada");
        assert_eq!(state.world.passengers.station, "earth_station");
        assert_eq!(state.world.passengers.waiting_count, Some(1));
        assert_eq!(state.world.passengers.waiting[0].estimated_fare, Some(240));
    }

    #[test]
    fn successful_empty_passenger_responses_clear_stale_rows() {
        let mut state = ProjectedState::default();
        state.bot.passengers.aboard = Arc::new(vec![serde_json::from_value(serde_json::json!({
            "base_fare": 0, "bio": "", "citizen_id": "stale-aboard",
            "class": "economy", "destination": "", "destination_name": "",
            "name": "stale aboard", "ticks_remaining": 0
        }))
        .expect("aboard passenger fixture")]);
        state.world.passengers.waiting =
            Arc::new(vec![serde_json::from_value(serde_json::json!({
                "bio": "", "citizen_id": "stale-waiting", "citizenship": "",
                "class": "economy", "destination": "", "destination_name": "",
                "name": "stale waiting"
            }))
            .expect("waiting passenger fixture")]);
        let aboard = serde_json::from_value(serde_json::json!({
            "count": 0,
            "berths": {
                "economy": { "free": 2, "total": 2 },
                "business": { "free": 0, "total": 0 },
                "first": { "free": 0, "total": 0 }
            },
            "passengers": []
        }))
        .expect("aboard response");
        project_aboard_passengers(&mut state, aboard);
        let waiting = serde_json::from_value(serde_json::json!({
            "count": 0,
            "demand_level": "low",
            "fare_surge": 0.8,
            "market_conditions": "Well served",
            "station": "earth_station",
            "waiting": []
        }))
        .expect("station response");
        project_station_passengers(&mut state, waiting);

        assert!(state.bot.passengers.aboard.is_empty());
        assert!(state.world.passengers.waiting.is_empty());
        assert_eq!(state.bot.passengers.aboard_count, Some(0));
        assert_eq!(state.world.passengers.waiting_count, Some(0));
    }

    #[test]
    fn treats_zero_cargo_used_as_empty_cargo() {
        let mut cache = spacemolt_lib_rs::state::StateCache::default();
        cache.seed(&json!({
            "ship": { "cargo_used": 0, "cargo_capacity": 20 },
            "cargo": [{ "item_id": "iron_ore", "quantity": 7 }]
        }));

        let state = project_account_state(&cache);

        assert!(state.bot.cargo.is_empty());
    }

    #[test]
    fn projects_faction_identity_from_player_section() {
        let mut cache = spacemolt_lib_rs::state::StateCache::default();
        cache.seed(&serde_json::json!({
            "player": {
                "id": "player_1",
                "username": "Scout",
                "faction_id": "fac_traders",
                "clan_tag": "TRD"
            }
        }));
        let state = project_account_state(&cache);

        assert_eq!(state.bot.player.faction_id.as_deref(), Some("fac_traders"));
        assert_eq!(state.bot.player.clan_tag.as_deref(), Some("TRD"));
    }

    #[test]
    fn projects_market_book_from_client_market_cache_shape() {
        let mut state = ProjectedState {
            bot: BotState {
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: None,
                    poi_id: Some("earth_station_poi".to_string()),
                    docked_at: (true).then(|| "docked".to_string()),
                    ..Default::default()
                },
                ..BotState::default()
            },
            market_base_id: Some("earth_station".to_string()),
            ..ProjectedState::default()
        };
        project_market_book_from_client(
            &mut state,
            &MarketBook {
                base_id: "earth_station".to_string(),
                base_name: None,
                tick: 42,
                items: HashMap::from([(
                    "iron_ore".to_string(),
                    spacemolt_lib_rs::state::MarketItem::Snapshot(
                        serde_json::from_value(serde_json::json!({
                            "item_id": "iron_ore",
                            "buy_orders": [{ "price_each": 8, "quantity": 10 }],
                            "sell_orders": [{ "price_each": 12, "quantity": 4 }]
                        }))
                        .expect("valid market item"),
                    ),
                )]),
            },
        );

        assert!(state
            .world
            .market
            .station_markets
            .contains_key("earth_station_poi"));
        assert!(state.world.market.buy_orders.contains_key("iron_ore"));
        assert!(state.world.market.sell_orders.contains_key("iron_ore"));
    }

    #[test]
    fn projects_observation_presence_into_nearby_state() {
        let mut state = ProjectedState::default();
        project_observation_view_from_client(
            &mut state,
            &ObservationView {
                poi_id: Some("earth_station".to_string()),
                system_id: Some("sol".to_string()),
                tick: 42,
                nearby: HashMap::from([(
                    "player_2".to_string(),
                    ObservedPlayer::NearbyUpdate(
                        serde_json::from_value(serde_json::json!({
                            "player_id": "player_2",
                            "username": "Neighbor",
                            "in_combat": false
                        }))
                        .expect("valid nearby player"),
                    ),
                )]),
                system: HashMap::new(),
                cloaked: HashMap::new(),
                unknown_signature: false,
                active_scan: false,
            },
        );

        assert_eq!(state.bot.location.poi_id.as_deref(), Some("earth_station"));
        assert!(serde_json::to_string(state.bot.observation_nearby.as_ref())
            .expect("typed observation contacts")
            .contains("Neighbor"));
    }

    #[test]
    fn projects_every_get_system_poi_into_durable_galaxy_knowledge() {
        let galaxy = project_get_system_json(&serde_json::json!({
            "action": "get_system",
            "system": {
                "id": "alpha",
                "name": "Alpha Centauri",
                "empire": "solarian",
                "connections": [{ "system_id": "sol", "name": "Sol", "distance": 1 }],
                "pois": [
                    { "id": "alpha_star", "name": "Alpha Centauri A", "type": "star", "position": { "x": 10.0, "y": 20.0 }, "has_base": false, "online": 0, "fuel_reserve": 0 },
                    { "id": "alpha_prime", "name": "Alpha Prime", "type": "planet", "position": { "x": 30.0, "y": 40.0 }, "has_base": false, "online": 2, "fuel_reserve": 0 },
                    { "id": "alpha_station", "name": "Centauri Station", "type": "station", "position": { "x": 50.0, "y": 60.0 }, "has_base": true, "base_id": "base_alpha", "base_name": "Centauri Base", "online": 4, "fuel_reserve": 100 }
                ]
            }
        }))
        .expect("get_system galaxy projection");

        assert_eq!(galaxy.poi_records.len(), 3);
        assert_eq!(
            galaxy
                .poi_records
                .get("alpha_prime")
                .map(|poi| poi.system_id.as_str()),
            Some("alpha")
        );
        let planet = galaxy
            .poi_records
            .get("alpha_prime")
            .expect("planet metadata");
        assert_eq!(planet.info.name, "Alpha Prime");
        assert_eq!(planet.info.poi_type, "planet");
        assert_eq!((planet.info.x, planet.info.y), (Some(30.0), Some(40.0)));
        assert_eq!(
            galaxy
                .poi_records
                .values()
                .filter(|poi| poi.system_id == "alpha" && poi.info.poi_type == "station")
                .count(),
            1
        );
        assert_eq!(galaxy.poi_id_for_base("base_alpha"), Some("alpha_station"));
    }

    fn query_fixture(action: &str, text: &str) -> ExecutedSpacemoltCommand {
        let value: serde_json::Value = serde_json::from_str(text).expect("golden fixture JSON");
        ExecutedSpacemoltCommand {
            tool: "spacemolt".into(),
            action: action.into(),
            result: spacemolt_lib_rs::CommandResult::Query(
                spacemolt_lib_rs::protocol::QueryResult {
                    result: serde_json::Value::Null,
                    structured_content: Some(value),
                },
            ),
        }
    }

    #[test]
    fn golden_query_and_mutation_fixtures_reach_canonical_records() {
        let map = project_executed_command(&query_fixture(
            "get_map",
            include_str!("../testdata/observations/get_map.json"),
        ))
        .expect("typed map");
        assert_eq!(
            map[0].world.galaxy.system_records["sol"].name.as_deref(),
            Some("Sol")
        );

        let system = project_executed_command(&query_fixture(
            "get_system",
            include_str!("../testdata/observations/get_system.json"),
        ))
        .expect("typed system");
        assert_eq!(system[0].world.galaxy.poi_records.len(), 5);
        assert_eq!(
            system[0].world.galaxy.poi_records["earth"].info.name,
            "Earth Station"
        );

        let poi = project_executed_command(&query_fixture(
            "get_poi",
            include_str!("../testdata/observations/get_poi.json"),
        ))
        .expect("typed POI");
        assert_eq!(
            poi[0].world.galaxy.poi_records["belt_1"].resources[0].resource_id,
            "iron_ore"
        );

        let details: serde_json::Value =
            serde_json::from_str(include_str!("../testdata/observations/survey_system.json"))
                .unwrap();
        let survey = ExecutedSpacemoltCommand {
            tool: "spacemolt".into(),
            action: "survey_system".into(),
            result: spacemolt_lib_rs::CommandResult::Mutation(
                spacemolt_lib_rs::protocol::MutationResult {
                    command: "survey_system".into(),
                    tick: 1,
                    delta: serde_json::json!({"details": details}),
                    auto_docked: false,
                    auto_undocked: false,
                },
            ),
        };
        let survey = project_executed_command(&survey).expect("typed survey");
        let galaxy = survey[0].world.galaxy.as_ref();
        assert_eq!(
            galaxy.poi_records["deep"].resources[0].resource_id,
            "iridium"
        );
        assert_eq!(
            galaxy.poi_records["belt_1"].resources[0].resource_id,
            "iron_ore"
        );
        assert_eq!(
            galaxy.system_records["sol"].bloom_status.as_deref(),
            Some("active")
        );
        assert_eq!(galaxy.system_records["sol"].faint_signatures.len(), 1);
        assert_eq!(galaxy.system_records["sol"].wildlife.len(), 1);
    }

    #[test]
    fn golden_status_and_subscription_updates_share_location_projection() {
        for fixture in [
            include_str!("../testdata/observations/status.json"),
            include_str!("../testdata/observations/subscription.json"),
        ] {
            let value = serde_json::from_str(fixture).unwrap();
            let observation = project_location_update(&value, 100).expect("location observation");
            assert_eq!(observation.status_system.as_deref(), Some("sol"));
            assert_eq!(
                observation.world.galaxy.poi_records["earth"].first_visited_unix,
                Some(100)
            );
        }
    }

    #[test]
    fn location_updates_project_resources_into_the_canonical_poi() {
        let value = serde_json::json!({
            "location": {
                "system_id": "sol",
                "poi_id": "belt_1",
                "in_transit": false,
                "resources": [
                    {"resource_id": "iron_ore", "name": "Iron Ore", "richness": 8},
                    {"resource_id": "copper_ore", "name": "Copper Ore"}
                ]
            }
        });

        let observation = project_location_update(&value, 100).expect("location observation");
        let resources = &observation.world.galaxy.poi_records["belt_1"].resources;
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].resource_id, "iron_ore");
        assert_eq!(resources[1].resource_id, "copper_ore");
    }
}
