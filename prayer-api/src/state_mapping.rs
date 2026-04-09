use std::collections::HashMap;

use chrono::Utc;
use prayer_runtime::engine::{CatalogEntryData, GameState, MissionInfoData};
use serde_json::Value;

use crate::{
    ApiError,
    RuntimeCatalogueDto, RuntimeCatalogueEntryDto, RuntimeGalaxyCatalogDto,
    RuntimeGalaxyExplorationDto, RuntimeGalaxyKnownPoiInfoDto, RuntimeGalaxyMapSnapshotDto,
    RuntimeGalaxyMarketDto, RuntimeGalaxyPoiInfoDto, RuntimeGalaxyResourcesDto,
    RuntimeGalaxyStateDto, RuntimeGalaxySystemInfoDto, RuntimeGameStateDto,
    RuntimeItemCatalogueEntryDto, RuntimeItemStackDto, RuntimeMarketStateDto,
    RuntimeMissionInfoDto, RuntimeOpenOrderInfoDto, RuntimePlayerShipDto, RuntimePoiInfoDto,
    RuntimeRecipeEntryDto, RuntimeRecipeIngredientEntryDto, RuntimeShipCatalogueEntryDto,
    RuntimeStationContextDto,
};

pub(crate) fn map_runtime_state(state: &GameState) -> Result<RuntimeGameStateDto, ApiError> {
    let system = required_non_empty_state_field("system", state.system.as_deref())?;
    let home_base = state.home_base.clone().unwrap_or_default();
    let docked = state.docked;
    let current_poi_id = state
        .current_poi
        .as_ref()
        .filter(|poi| !poi.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| system.clone());
    let storage_source = state
        .current_poi
        .as_ref()
        .and_then(|poi| state.stash.get(poi))
        .or_else(|| {
            state
                .home_base
                .as_ref()
                .and_then(|base| state.stash.get(base))
        })
        .cloned()
        .unwrap_or_default();
    let storage_items = map_item_stacks(&storage_source);
    let cargo = map_item_stacks(state.cargo.as_ref());
    let poi_base_by_id = reverse_base_lookup(&state.galaxy.poi_base_to_id);

    let mut known_systems = state.galaxy.systems.clone();
    push_unique_string(&mut known_systems, &system);

    let mut known_poi_ids = state.galaxy.pois.clone();
    push_unique_string(&mut known_poi_ids, &current_poi_id);

    let mut poi_ids_by_system: HashMap<String, Vec<String>> = HashMap::new();
    for (poi_id, system_id) in state.galaxy.poi_system.iter() {
        if !poi_id.is_empty() && !system_id.is_empty() {
            poi_ids_by_system
                .entry(system_id.clone())
                .or_default()
                .push(poi_id.clone());
        }
    }
    if !current_poi_id.is_empty() && !system.is_empty() {
        poi_ids_by_system
            .entry(system.clone())
            .or_default()
            .push(current_poi_id.clone());
    }
    for poi_ids in poi_ids_by_system.values_mut() {
        sort_dedup_strings(poi_ids);
    }

    let known_pois: Vec<RuntimeGalaxyKnownPoiInfoDto> = known_poi_ids
        .iter()
        .map(|poi_id| {
            let poi_system = state
                .galaxy
                .poi_system
                .get(poi_id)
                .cloned()
                .or_else(|| {
                    if *poi_id == current_poi_id {
                        Some(system.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            let poi_type = state
                .galaxy
                .poi_type_by_id
                .get(poi_id)
                .cloned()
                .unwrap_or_else(|| {
                    if !docked && *poi_id == current_poi_id && *poi_id == system && !system.is_empty()
                    {
                        "space".to_string()
                    } else {
                        String::new()
                    }
                });
            let base_id = poi_base_by_id.get(poi_id).cloned();
            RuntimeGalaxyKnownPoiInfoDto {
                id: poi_id.clone(),
                system_id: poi_system,
                name: poi_id.clone(),
                r#type: poi_type,
                x: None,
                y: None,
                has_base: base_id.is_some(),
                base_id: base_id.clone(),
                base_name: base_id,
                last_seen_utc: Utc::now(),
            }
        })
        .collect();

    let systems: Vec<RuntimeGalaxySystemInfoDto> = known_systems
        .iter()
        .map(|system_id| {
            let (x, y) = state
                .galaxy
                .system_coordinates
                .get(system_id)
                .copied()
                .map(|(sx, sy)| (Some(sx), Some(sy)))
                .unwrap_or((None, None));
            let pois = poi_ids_by_system
                .get(system_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|poi_id| RuntimeGalaxyPoiInfoDto {
                    id: poi_id,
                    x: None,
                    y: None,
                })
                .collect();
            RuntimeGalaxySystemInfoDto {
                id: system_id.clone(),
                empire: String::new(),
                x,
                y,
                connections: state
                    .galaxy
                    .system_connections
                    .get(system_id)
                    .cloned()
                    .unwrap_or_default(),
                pois,
            }
        })
        .collect();

    let current_poi_type = known_pois
        .iter()
        .find(|poi| poi.id == current_poi_id)
        .map(|poi| poi.r#type.clone())
        .unwrap_or_default();
    let current_poi_base = known_pois
        .iter()
        .find(|poi| poi.id == current_poi_id)
        .and_then(|poi| poi.base_id.clone());

    let ship = RuntimePlayerShipDto {
        name: state.ship.name.clone(),
        class_id: state.ship.class_id.clone(),
        system_id: system.clone(),
        armor: state.ship.armor,
        speed: state.ship.speed,
        cpu_used: state.ship.cpu_used,
        cpu_capacity: state.ship.cpu_capacity,
        power_used: state.ship.power_used,
        power_capacity: state.ship.power_capacity,
        module_count: state.installed_modules.len() as i64,
        fuel: state.fuel_pct,
        max_fuel: 100,
        fuel_percent: state.fuel_pct,
        hull: state.ship.hull,
        max_hull: state.ship.max_hull,
        shield: state.ship.shield,
        max_shield: state.ship.max_shield,
        cargo_used: state.cargo_used,
        cargo_capacity: state.cargo_capacity,
        cargo,
    };
    let current_poi = RuntimePoiInfoDto {
        id: current_poi_id.clone(),
        system_id: system.clone(),
        name: current_poi_id.clone(),
        r#type: current_poi_type,
        description: String::new(),
        hidden: false,
        x: None,
        y: None,
        has_base: current_poi_base.is_some(),
        base_id: current_poi_base.clone(),
        base_name: current_poi_base,
        online: 0,
        resources: Vec::new(),
    };
    let mut systems_by_resource = map_systems_by_resource(
        &state.galaxy.pois_by_resource,
        &state.galaxy.poi_system,
    );
    if systems_by_resource.is_empty() {
        for item in state.cargo.keys() {
            systems_by_resource.insert(item.clone(), known_systems.clone());
        }
    }
    let mut explored_systems: Vec<String> = state.galaxy.explored_systems.iter().cloned().collect();
    push_unique_string(&mut explored_systems, &system);
    let mut visited_pois: Vec<String> = state.galaxy.visited_pois.iter().cloned().collect();
    push_unique_string(&mut visited_pois, &current_poi_id);
    let mut surveyed_systems: Vec<String> = state.galaxy.surveyed_systems.iter().cloned().collect();
    sort_dedup_strings(&mut explored_systems);
    sort_dedup_strings(&mut visited_pois);
    sort_dedup_strings(&mut surveyed_systems);
    let mut pois = known_pois
        .iter()
        .map(|poi| RuntimePoiInfoDto {
            id: poi.id.clone(),
            system_id: poi.system_id.clone(),
            name: poi.name.clone(),
            r#type: poi.r#type.clone(),
            description: String::new(),
            hidden: false,
            x: poi.x,
            y: poi.y,
            has_base: poi.has_base,
            base_id: poi.base_id.clone(),
            base_name: poi.base_name.clone(),
            online: 0,
            resources: Vec::new(),
        })
        .collect::<Vec<_>>();
    if !current_poi.id.is_empty() && !pois.iter().any(|poi| poi.id == current_poi.id) {
        pois.push(current_poi.clone());
    }
    let station = if docked {
        Some(RuntimeStationContextDto {
            station_id: current_poi_id.clone(),
            station_name: current_poi_id.clone(),
            storage_credits: 0,
            storage_items: storage_items.clone(),
            market: None,
            shipyard_showroom: Vec::new(),
            shipyard_listings: Vec::new(),
            craftable: Vec::new(),
        })
    } else {
        None
    };
    Ok(RuntimeGameStateDto {
        system: system.clone(),
        current_poi: current_poi.clone(),
        pois,
        systems: known_systems.clone(),
        galaxy: RuntimeGalaxyStateDto {
            map: RuntimeGalaxyMapSnapshotDto {
                systems,
                known_pois,
            },
            market: RuntimeGalaxyMarketDto {
                markets_by_station: HashMap::new(),
                global_median_buy_prices: HashMap::new(),
                global_median_sell_prices: HashMap::new(),
                global_weighted_mid_prices: HashMap::new(),
            },
            catalog: RuntimeGalaxyCatalogDto {
                items_by_id: map_item_catalog_entries(state),
                ships_by_id: map_ship_catalog_entries(state),
                recipes_by_id: map_recipe_catalog_entries(state),
            },
            resources: RuntimeGalaxyResourcesDto {
                systems_by_resource: systems_by_resource.clone(),
                pois_by_resource: state.galaxy.pois_by_resource.clone(),
            },
            exploration: RuntimeGalaxyExplorationDto {
                explored_systems,
                visited_pois,
                surveyed_systems,
                mining_checked_pois_by_resource: HashMap::new(),
                mining_explored_systems_by_resource: HashMap::new(),
            },
            updated_at_utc: Utc::now(),
        },
        storage_credits: 0,
        storage_items,
        economy_deals: Vec::new(),
        own_buy_orders: map_open_orders(state.own_buy_orders.as_ref()),
        own_sell_orders: map_open_orders(state.own_sell_orders.as_ref()),
        ship,
        credits: state.credits,
        docked,
        home_base,
        shipyard_showroom: Vec::new(),
        shipyard_listings: Vec::new(),
        ship_catalogue: RuntimeCatalogueDto {
            r#type: String::new(),
            category: None,
            id: None,
            page: None,
            page_size: None,
            total_pages: None,
            total_items: None,
            total: None,
            message: String::new(),
            items: Vec::new(),
            entries: Vec::new(),
            ships: Vec::new(),
        },
        owned_ships: Vec::new(),
        available_recipes: Vec::new(),
        skills: HashMap::new(),
        active_missions: map_missions(
            &state.missions.active,
            &state.missions.active_details,
            state,
            true,
        ),
        available_missions: map_missions(
            &state.missions.available,
            &state.missions.available_details,
            state,
            false,
        ),
        notifications: Vec::new(),
        chat_messages: Vec::new(),
        current_market: None::<RuntimeMarketStateDto>,
        station,
    })
}

fn map_missions(
    ids: &[String],
    details: &[MissionInfoData],
    state: &GameState,
    active: bool,
) -> Vec<RuntimeMissionInfoDto> {
    let details_by_id = details
        .iter()
        .filter(|detail| !detail.mission_id.trim().is_empty())
        .map(|detail| (detail.mission_id.clone(), detail))
        .collect::<HashMap<_, _>>();

    ids.iter()
        .filter(|id| !id.trim().is_empty())
        .map(|id| {
            let detail = details_by_id.get(id).copied();
            RuntimeMissionInfoDto {
                id: detail
                    .map(|m| m.id.clone())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| id.clone()),
                mission_id: id.clone(),
                template_id: detail
                    .map(|m| m.template_id.clone())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| id.clone()),
                title: detail
                    .map(|m| m.title.clone())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| id.clone()),
                r#type: detail.map(|m| m.mission_type.clone()).unwrap_or_default(),
                description: detail.map(|m| m.description.clone()).unwrap_or_default(),
                progress_text: detail.map(|m| m.progress_text.clone()).unwrap_or_default(),
                completed: detail.map(|m| m.completed).unwrap_or(false) || {
                    if active {
                        *state.mission_complete.get(id).unwrap_or(&false)
                    } else {
                        false
                    }
                },
                difficulty: detail.and_then(|m| m.difficulty),
                expires_in_ticks: detail.and_then(|m| m.expires_in_ticks),
                accepted_at: detail.map(|m| m.accepted_at.clone()).unwrap_or_default(),
                issuing_base: detail.map(|m| m.issuing_base.clone()).unwrap_or_default(),
                issuing_base_id: detail.map(|m| m.issuing_base_id.clone()).unwrap_or_default(),
                giver_name: detail.map(|m| m.giver_name.clone()).unwrap_or_default(),
                giver_title: detail.map(|m| m.giver_title.clone()).unwrap_or_default(),
                repeatable: detail.and_then(|m| m.repeatable),
                faction_id: detail.map(|m| m.faction_id.clone()).unwrap_or_default(),
                faction_name: detail.map(|m| m.faction_name.clone()).unwrap_or_default(),
                chain_next: detail.map(|m| m.chain_next.clone()).unwrap_or_default(),
                objectives_summary: detail
                    .map(|m| m.objectives_summary.clone())
                    .unwrap_or_default(),
                progress_summary: detail
                    .map(|m| m.progress_summary.clone())
                    .unwrap_or_default(),
                requirements_summary: detail
                    .map(|m| m.requirements_summary.clone())
                    .unwrap_or_default(),
                rewards_summary: detail.map(|m| m.rewards_summary.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

fn required_non_empty_state_field(field: &str, value: Option<&str>) -> Result<String, ApiError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Ok(v.to_string()),
        None => Err(ApiError::InvalidRuntimeState(format!(
            "missing required runtime state field '{field}'"
        ))),
    }
}

fn push_unique_string(values: &mut Vec<String>, candidate: &str) {
    if !candidate.is_empty() && !values.iter().any(|value| value == candidate) {
        values.push(candidate.to_string());
    }
}

fn sort_dedup_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn reverse_base_lookup(base_to_poi: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (base_id, poi_id) in base_to_poi {
        if !base_id.is_empty() && !poi_id.is_empty() {
            out.entry(poi_id.clone()).or_insert_with(|| base_id.clone());
        }
    }
    out
}

fn map_systems_by_resource(
    pois_by_resource: &HashMap<String, Vec<String>>,
    poi_system: &HashMap<String, String>,
) -> HashMap<String, Vec<String>> {
    let mut systems_by_resource = HashMap::new();
    for (resource_id, poi_ids) in pois_by_resource {
        let mut systems = poi_ids
            .iter()
            .filter_map(|poi_id| poi_system.get(poi_id).cloned())
            .collect::<Vec<_>>();
        sort_dedup_strings(&mut systems);
        systems_by_resource.insert(resource_id.clone(), systems);
    }
    systems_by_resource
}

fn map_open_orders(
    orders: &[prayer_runtime::engine::OpenOrderInfo],
) -> Vec<RuntimeOpenOrderInfoDto> {
    orders
        .iter()
        .map(|o| RuntimeOpenOrderInfoDto {
            order_id: o.order_id.clone(),
            item_id: o.item_id.clone(),
            price_each: o.price_each,
            quantity: o.quantity,
        })
        .collect()
}

fn map_item_stacks(items: &HashMap<String, i64>) -> HashMap<String, RuntimeItemStackDto> {
    items
        .iter()
        .map(|(id, quantity)| {
            (
                id.clone(),
                RuntimeItemStackDto {
                    item_id: id.clone(),
                    quantity: *quantity,
                },
            )
        })
        .collect()
}

fn map_item_catalog_entries(state: &GameState) -> HashMap<String, RuntimeItemCatalogueEntryDto> {
    if !state.galaxy.item_catalog_entries.is_empty() {
        return state
            .galaxy
            .item_catalog_entries
            .iter()
            .map(|(id, entry)| {
                (
                    id.clone(),
                    RuntimeItemCatalogueEntryDto {
                        entry: make_catalog_entry(id, Some(entry)),
                    },
                )
            })
            .collect();
    }
    state
        .galaxy
        .item_ids
        .iter()
        .map(|id| {
            (
                id.clone(),
                RuntimeItemCatalogueEntryDto {
                    entry: make_catalog_entry(id, None),
                },
            )
        })
        .collect()
}

fn map_ship_catalog_entries(state: &GameState) -> HashMap<String, RuntimeShipCatalogueEntryDto> {
    if !state.galaxy.ship_catalog_entries.is_empty() {
        return state
            .galaxy
            .ship_catalog_entries
            .iter()
            .map(|(id, entry)| {
                (
                    id.clone(),
                    RuntimeShipCatalogueEntryDto {
                        entry: make_catalog_entry(id, Some(entry)),
                    },
                )
            })
            .collect();
    }
    state
        .galaxy
        .ship_ids
        .iter()
        .map(|id| {
            (
                id.clone(),
                RuntimeShipCatalogueEntryDto {
                    entry: make_catalog_entry(id, None),
                },
            )
        })
        .collect()
}

fn map_recipe_catalog_entries(state: &GameState) -> HashMap<String, RuntimeRecipeEntryDto> {
    if !state.galaxy.recipe_catalog_entries.is_empty() {
        return state
            .galaxy
            .recipe_catalog_entries
            .iter()
            .map(|(id, entry)| (id.clone(), make_recipe_entry(id, Some(entry))))
            .collect();
    }
    state
        .galaxy
        .recipe_ids
        .iter()
        .map(|id| (id.clone(), make_recipe_entry(id, None)))
        .collect()
}

fn value_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| value.get(*k).and_then(Value::as_str))
        .filter(|v| !v.trim().is_empty())
}

fn value_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|k| value.get(*k).and_then(Value::as_i64))
}

fn value_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|k| value.get(*k).and_then(Value::as_f64))
}

fn value_map_i64(value: Option<&Value>) -> Option<HashMap<String, i64>> {
    let Value::Object(obj) = value? else {
        return None;
    };
    let mapped = obj
        .iter()
        .filter_map(|(k, v)| v.as_i64().map(|n| (k.clone(), n)))
        .collect::<HashMap<_, _>>();
    if mapped.is_empty() {
        None
    } else {
        Some(mapped)
    }
}

fn map_ingredient_entries(value: Option<&Value>) -> Vec<RuntimeRecipeIngredientEntryDto> {
    let Some(Value::Array(entries)) = value else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let id = value_str(entry, &["id", "item_id", "item"])
                .map(ToOwned::to_owned)
                .unwrap_or_default();
            if id.is_empty() {
                return None;
            }
            let name = value_str(entry, &["name", "item"])
                .unwrap_or(&id)
                .to_string();
            Some(RuntimeRecipeIngredientEntryDto {
                item_id: value_str(entry, &["item_id", "item", "id"])
                    .unwrap_or(&id)
                    .to_string(),
                item: value_str(entry, &["item", "item_id", "id"])
                    .unwrap_or(&id)
                    .to_string(),
                id: id.clone(),
                name,
                quantity: value_i64(entry, &["quantity"]),
                amount: value_i64(entry, &["amount"]),
                count: value_i64(entry, &["count"]),
            })
        })
        .collect()
}

fn make_catalog_entry(id: &str, source: Option<&CatalogEntryData>) -> RuntimeCatalogueEntryDto {
    let raw = source.map(|entry| &entry.raw);
    RuntimeCatalogueEntryDto {
        id: id.to_string(),
        name: raw
            .and_then(|v| value_str(v, &["name", "id"]))
            .unwrap_or(id)
            .to_string(),
        class_id: raw
            .and_then(|v| value_str(v, &["class_id", "classId"]))
            .unwrap_or_default()
            .to_string(),
        class_name: raw
            .and_then(|v| value_str(v, &["class", "class_name", "className"]))
            .unwrap_or_default()
            .to_string(),
        category: raw
            .and_then(|v| value_str(v, &["category"]))
            .unwrap_or_default()
            .to_string(),
        type_name: raw
            .and_then(|v| value_str(v, &["type"]))
            .unwrap_or_default()
            .to_string(),
        tier: raw.and_then(|v| value_i64(v, &["tier"])),
        scale: raw.and_then(|v| value_i64(v, &["scale"])),
        hull: raw.and_then(|v| value_i64(v, &["hull"])),
        base_hull: raw.and_then(|v| value_i64(v, &["base_hull", "baseHull"])),
        shield: raw.and_then(|v| value_i64(v, &["shield"])),
        base_shield: raw.and_then(|v| value_i64(v, &["base_shield", "baseShield"])),
        cargo: raw.and_then(|v| value_i64(v, &["cargo"])),
        cargo_capacity: raw.and_then(|v| value_i64(v, &["cargo_capacity", "cargoCapacity"])),
        speed: raw.and_then(|v| value_i64(v, &["speed"])),
        base_speed: raw.and_then(|v| value_i64(v, &["base_speed", "baseSpeed"])),
        price: raw.and_then(|v| value_f64(v, &["price"])),
        materials: raw.and_then(|v| value_map_i64(v.get("materials"))),
        ingredients: raw
            .map(|v| map_ingredient_entries(v.get("ingredients")))
            .unwrap_or_default(),
        inputs: raw
            .map(|v| map_ingredient_entries(v.get("inputs")))
            .unwrap_or_default(),
        outputs: raw
            .map(|v| map_ingredient_entries(v.get("outputs")))
            .unwrap_or_default(),
        required_skills: raw.and_then(|v| value_map_i64(v.get("required_skills"))),
    }
}

fn make_recipe_entry(id: &str, source: Option<&CatalogEntryData>) -> RuntimeRecipeEntryDto {
    let raw = source.map(|entry| &entry.raw);
    RuntimeRecipeEntryDto {
        id: id.to_string(),
        name: raw
            .and_then(|v| value_str(v, &["name", "id"]))
            .unwrap_or(id)
            .to_string(),
        inputs: raw
            .map(|v| map_ingredient_entries(v.get("inputs")))
            .unwrap_or_default(),
        outputs: raw
            .map(|v| map_ingredient_entries(v.get("outputs")))
            .unwrap_or_default(),
        required_skills: raw.and_then(|v| value_map_i64(v.get("required_skills"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prayer_runtime::engine::GameState;

    #[test]
    fn map_runtime_state_undocked_uses_system_as_poi_id() {
        let state = GameState {
            system: Some("sol".to_string()),
            docked: false,
            ..GameState::default()
        };
        let dto = map_runtime_state(&state).expect("state should map");
        assert_eq!(dto.system, "sol");
        assert_eq!(dto.current_poi.id, "sol");
        assert_eq!(dto.current_poi.r#type, "space");
    }

    #[test]
    fn map_runtime_state_docked_uses_current_poi_over_system() {
        let state = GameState {
            system: Some("sol".to_string()),
            current_poi: Some("sol_station".to_string()),
            docked: true,
            ..GameState::default()
        };
        let dto = map_runtime_state(&state).expect("state should map");
        assert_eq!(dto.current_poi.id, "sol_station");
        assert_eq!(dto.current_poi.r#type, "");
        assert!(dto.docked);
    }

    #[test]
    fn map_runtime_state_docked_without_current_poi_falls_back_to_system() {
        let state = GameState {
            system: Some("sol".to_string()),
            home_base: Some("earth_base".to_string()),
            current_poi: None,
            docked: true,
            ..GameState::default()
        };
        let dto = map_runtime_state(&state).expect("state should map");
        assert_eq!(dto.current_poi.id, "sol");
    }

    #[test]
    fn map_runtime_state_storage_populated_from_stash_at_current_poi() {
        let state = GameState {
            system: Some("sol".to_string()),
            current_poi: Some("sol_station".to_string()),
            home_base: Some("earth_base".to_string()),
            docked: true,
            stash: std::sync::Arc::new(std::collections::HashMap::from([(
                "sol_station".to_string(),
                std::collections::HashMap::from([("iron_ore".to_string(), 42i64)]),
            )])),
            ..GameState::default()
        };

        let dto = map_runtime_state(&state).expect("state should map");
        assert!(dto.storage_items.contains_key("iron_ore"));
        assert_eq!(dto.storage_items["iron_ore"].quantity, 42);
    }

    #[test]
    fn map_runtime_state_storage_falls_back_to_home_base_when_current_poi_missing() {
        let state = GameState {
            system: Some("sol".to_string()),
            current_poi: Some("sol_station".to_string()),
            home_base: Some("earth_base".to_string()),
            docked: true,
            stash: std::sync::Arc::new(std::collections::HashMap::from([(
                "earth_base".to_string(),
                std::collections::HashMap::from([("iron_ore".to_string(), 42i64)]),
            )])),
            ..GameState::default()
        };

        let dto = map_runtime_state(&state).expect("state should map");
        assert!(dto.storage_items.contains_key("iron_ore"));
        assert_eq!(dto.storage_items["iron_ore"].quantity, 42);
    }

    #[test]
    fn map_runtime_state_storage_empty_when_no_home_base() {
        let state = GameState {
            system: Some("sol".to_string()),
            home_base: None,
            ..GameState::default()
        };
        let dto = map_runtime_state(&state).expect("state should map");
        assert!(dto.storage_items.is_empty());
    }

    #[test]
    fn map_runtime_state_station_context_present_when_docked() {
        let state = GameState {
            system: Some("sol".to_string()),
            current_poi: Some("sol_station".to_string()),
            docked: true,
            ..GameState::default()
        };
        let dto = map_runtime_state(&state).expect("state should map");
        assert!(dto.station.is_some());
    }

    #[test]
    fn map_runtime_state_station_context_absent_when_undocked() {
        let state = GameState {
            system: Some("sol".to_string()),
            docked: false,
            ..GameState::default()
        };
        let dto = map_runtime_state(&state).expect("state should map");
        assert!(dto.station.is_none());
    }

    #[test]
    fn map_runtime_state_cargo_keys_appear_in_systems_by_resource() {
        let state = GameState {
            cargo: std::sync::Arc::new(std::collections::HashMap::from([
                ("iron_ore".to_string(), 3i64),
                ("water".to_string(), 1i64),
            ])),
            system: Some("sol".to_string()),
            ..GameState::default()
        };

        let dto = map_runtime_state(&state).expect("state should map");
        assert!(dto
            .galaxy
            .resources
            .systems_by_resource
            .contains_key("iron_ore"));
        assert!(dto
            .galaxy
            .resources
            .systems_by_resource
            .contains_key("water"));
    }

    #[test]
    fn map_runtime_state_credits_and_fuel_forwarded() {
        let state = GameState {
            credits: 9999,
            fuel_pct: 42,
            system: Some("sol".to_string()),
            ..GameState::default()
        };
        let dto = map_runtime_state(&state).expect("state should map");
        assert_eq!(dto.credits, 9999);
        assert_eq!(dto.ship.fuel, 42);
        assert_eq!(dto.ship.fuel_percent, 42);
    }

    #[test]
    fn map_runtime_state_open_orders_forwarded() {
        let state = GameState {
            own_buy_orders: std::sync::Arc::new(vec![prayer_runtime::engine::OpenOrderInfo {
                order_id: "ob_1".to_string(),
                item_id: "iron".to_string(),
                price_each: 10.0,
                quantity: 5,
            }]),
            own_sell_orders: std::sync::Arc::new(vec![]),
            system: Some("sol".to_string()),
            ..GameState::default()
        };

        let dto = map_runtime_state(&state).expect("state should map");
        assert_eq!(dto.own_buy_orders.len(), 1);
        assert_eq!(dto.own_buy_orders[0].order_id, "ob_1");
        assert!(dto.own_sell_orders.is_empty());
    }

    #[test]
    fn map_runtime_state_missions_forwarded_to_dto() {
        let state = GameState {
            mission_complete: std::sync::Arc::new(std::collections::HashMap::from([(
                "active_1".to_string(),
                true,
            )])),
            missions: std::sync::Arc::new(prayer_runtime::engine::MissionData {
                active: vec!["active_1".to_string()],
                available: vec!["avail_1".to_string()],
                active_details: vec![prayer_runtime::engine::MissionInfoData {
                    mission_id: "active_1".to_string(),
                    title: "Welcome to Sol Central".to_string(),
                    mission_type: "tutorial".to_string(),
                    objectives_summary: "Dock at Sol Central".to_string(),
                    ..prayer_runtime::engine::MissionInfoData::default()
                }],
                available_details: vec![prayer_runtime::engine::MissionInfoData {
                    mission_id: "avail_1".to_string(),
                    title: "First Haul".to_string(),
                    description: "Deliver ore".to_string(),
                    rewards_summary: "{\"credits\":500}".to_string(),
                    ..prayer_runtime::engine::MissionInfoData::default()
                }],
                ..prayer_runtime::engine::MissionData::default()
            }),
            system: Some("sol".to_string()),
            ..GameState::default()
        };

        let dto = map_runtime_state(&state).expect("state should map");
        assert_eq!(dto.active_missions.len(), 1);
        assert_eq!(dto.available_missions.len(), 1);
        assert_eq!(dto.active_missions[0].mission_id, "active_1");
        assert_eq!(dto.available_missions[0].mission_id, "avail_1");
        assert!(dto.active_missions[0].completed);
        assert!(!dto.available_missions[0].completed);
        assert_eq!(dto.active_missions[0].title, "Welcome to Sol Central");
        assert_eq!(dto.active_missions[0].objectives_summary, "Dock at Sol Central");
        assert_eq!(dto.available_missions[0].description, "Deliver ore");
    }

    #[test]
    fn map_runtime_state_missions_filters_blank_ids() {
        let state = GameState {
            missions: std::sync::Arc::new(prayer_runtime::engine::MissionData {
                active: vec!["".to_string(), "  ".to_string()],
                available: vec!["ok_1".to_string()],
                ..prayer_runtime::engine::MissionData::default()
            }),
            system: Some("sol".to_string()),
            ..GameState::default()
        };

        let dto = map_runtime_state(&state).expect("state should map");
        assert!(dto.active_missions.is_empty());
        assert_eq!(dto.available_missions.len(), 1);
        assert_eq!(dto.available_missions[0].mission_id, "ok_1");
    }

    #[test]
    fn map_runtime_state_errors_when_system_missing() {
        let state = GameState::default();
        let err = map_runtime_state(&state).expect_err("missing system should error");
        assert!(matches!(err, ApiError::InvalidRuntimeState(_)));
    }
}
