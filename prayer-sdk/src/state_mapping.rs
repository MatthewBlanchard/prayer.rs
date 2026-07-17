use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use prayer_runtime::engine::{
    ActiveCommissionInfo, AgentSightingData, CatalogData, FactionGarageShipObservation, GalaxyData,
    MarketData, MarketOrder,
};
use prayer_state::{WildlifeCreatureData, WildlifePoiSnapshotData};
use serde_json::Value;

use prayer_runtime::read_context::WorldReadState;
use prayer_state::BotState;

use crate::{
    RuntimeArbitradePackageDto, RuntimeArbitradePackageMemberDto, RuntimeArbitrageAcquireFromDto,
    RuntimeArbitrageDealDto, RuntimeArbitrageDisposeToDto, RuntimeCatalogueDto,
    RuntimeCommanderGalaxyStateDto, RuntimeFactionGarageDto, RuntimeFactionGarageShipProjectionDto,
    RuntimeGalaxyCatalogDto, RuntimeGalaxyKnownPoiInfoDto, RuntimeGalaxyMapSnapshotDto,
    RuntimeGalaxyMarketDto, RuntimeGalaxyPoiInfoDto, RuntimeGalaxyResourcesDto,
    RuntimeGalaxySystemInfoDto, RuntimeGalaxyWildlifeDto, RuntimeGameChatMessageDto,
    RuntimeGameNotificationDto, RuntimeItemQuantityProjectionDto, RuntimeLocationDto,
    RuntimeLogisticsEndpointDto, RuntimeLogisticsItemDto, RuntimeLogisticsPackageDto,
    RuntimeMarketQueryOrderProjectionDto, RuntimeMarketQueryRequest, RuntimeMarketQueryResponse,
    RuntimeMarketStateDto, RuntimeOwnedShipProjectionDto, RuntimePassengerBerthUsageDto,
    RuntimePassengerBerthViewDto, RuntimePassengerFareDealDto, RuntimePassengerStateDto,
    RuntimePlayerShipProjectionDto, RuntimePoiInfoDto, RuntimePoiResourceInfoDto,
    RuntimeResourceDestinationPoiDto, RuntimeResourceSourcePoiDto, RuntimeResourceSourcesResponse,
    RuntimeSalvageStateDto, RuntimeShipyardListingEntryDto, RuntimeShipyardShowroomEntryDto,
    RuntimeSpaceLootInfoDto, RuntimeStationContextDto, RuntimeWildlifeCreatureDto,
    RuntimeWildlifePoiDto, RuntimeWildlifeSpeciesDto, RuntimeWildlifeSystemDto, SdkError,
    SocialBotDto, SocialResponse,
};

pub fn map_shared_runtime_world_state(
    catalog: &CatalogData,
    galaxy: &GalaxyData,
    wildlife_by_poi: &HashMap<String, WildlifePoiSnapshotData>,
) -> Result<RuntimeCommanderGalaxyStateDto, SdkError> {
    Ok(RuntimeCommanderGalaxyStateDto {
        map: map_shared_galaxy_map(galaxy)?,
        catalog: map_galaxy_catalog(catalog),
        resources: map_galaxy_resources(galaxy, None, &HashMap::new())?,
        wildlife: map_shared_galaxy_wildlife(wildlife_by_poi),
        updated_at_utc: Utc::now(),
    })
}

pub fn map_commander_session_state(
    actor: &BotState,
    world: &WorldReadState,
) -> Result<Value, SdkError> {
    let system = required_non_empty_state_field("system", projection_system_id_actor(actor))?;
    let current_poi_id =
        required_non_empty_state_field("current_poi", projection_poi_id_actor(actor))?;
    let home_base = actor.player.home_base.clone().unwrap_or_default();
    let home_poi = actor
        .player
        .home_poi
        .clone()
        .unwrap_or_else(|| home_base.clone());
    let ship = map_player_ship_actor(actor)?;
    let current_poi = map_poi_info_scoped(actor, &world.galaxy, &current_poi_id, &system)
        .unwrap_or_else(|| RuntimePoiInfoDto {
            id: current_poi_id.clone(),
            system_id: system.clone(),
            name: current_poi_id.clone(),
            r#type: effective_poi_type_actor(actor, &world.galaxy, &current_poi_id)
                .unwrap_or_default()
                .to_string(),
            class_name: String::new(),
            description: String::new(),
            hidden: false,
            x: None,
            y: None,
            has_base: false,
            base_id: None,
            base_name: None,
            online: 0,
            fuel_reserve: None,
            fuel_capacity: None,
            fuel_price: None,
            faction_fuel_reserve: None,
            faction_fuel_capacity: None,
            resources: map_poi_resources_from_galaxy(&world.galaxy, &current_poi_id),
        });

    let mut value = serde_json::Map::new();
    macro_rules! put {
        ($key:literal, $expr:expr) => {
            value.insert(
                $key.to_string(),
                serde_json::to_value($expr).map_err(|err| {
                    SdkError::InvalidRuntimeState(format!(
                        "serialize commander session field {}: {err}",
                        $key
                    ))
                })?,
            );
        };
    }
    put!("system", system);
    put!("currentPoi", current_poi.clone());
    put!("pois", vec![current_poi]);
    put!("systems", Vec::<String>::new());
    put!("storageByPoi", world.storage.as_ref().clone());
    put!("factionStorage", world.faction_storage.as_deref());
    put!("economyDeals", Vec::<RuntimeArbitrageDealDto>::new());
    put!("ownBuyOrders", actor.own_buy_orders.as_ref());
    put!("ownSellOrders", actor.own_sell_orders.as_ref());
    put!("craftingQueue", actor.crafting_queue.as_ref().clone());
    put!("ship", ship);
    put!("cargo", actor.cargo.as_ref().clone());
    put!("credits", actor.player.credits.unwrap_or_default());
    put!("docked", actor.location.docked_at.is_some());
    put!("homeBase", home_base);
    put!("homePoi", home_poi);
    put!("inTransit", actor.location.in_transit.unwrap_or(false));
    put!("transitType", actor.location.transit_type.clone());
    put!(
        "transitDestSystem",
        actor.location.transit_dest_system_id.clone()
    );
    put!("transitDestPoi", actor.location.transit_dest_poi_id.clone());
    put!("location", map_location_scoped(actor, world));
    put!(
        "username",
        actor.player.username.clone().unwrap_or_default()
    );
    put!("playerId", actor.player.id.clone());
    put!("empire", actor.player.empire.clone().unwrap_or_default());
    put!("clanTag", actor.player.clan_tag.clone());
    put!("statusMessage", actor.player.status_message.clone());
    put!("primaryColor", actor.player.primary_color.clone());
    put!("secondaryColor", actor.player.secondary_color.clone());
    put!("isCloaked", actor.player.is_cloaked);
    put!("towingWreckId", actor.player.towing_wreck_id.clone());
    put!("salvage", map_salvage_value(&world.salvage));
    put!("standings", actor.player.standings.clone());
    put!("playerStats", actor.player.stats.clone());
    put!(
        "factionId",
        actor.player.faction_id.clone().unwrap_or_default()
    );
    put!(
        "factionRank",
        actor.player.faction_rank.clone().unwrap_or_default()
    );
    put!("skills", actor.skills.as_ref().clone());
    put!("modules", actor.modules.as_ref().clone());
    put!("shipyardShowroom", map_shipyard_showroom(&world.catalog));
    put!(
        "shipyardListings",
        Vec::<RuntimeShipyardListingEntryDto>::new()
    );
    put!("shipCatalogue", empty_runtime_catalogue());
    put!("ownedShips", map_actor_owned_ships(actor));
    put!(
        "factionGarage",
        map_faction_garage_value(&world.faction_garage)
    );
    put!("passengers", map_scoped_passengers(actor, world));
    put!(
        "availableRecipes",
        Vec::<spacemolt_lib_rs::schema::CatalogDumpItemsItem>::new()
    );
    put!("activeMissions", map_actor_active_missions(actor));
    put!("availableMissions", map_actor_available_missions(actor));
    put!("notifications", Vec::<RuntimeGameNotificationDto>::new());
    put!("chatMessages", Vec::<RuntimeGameChatMessageDto>::new());
    let station_market = actor
        .location
        .poi_id
        .as_deref()
        .and_then(|poi| world.market.station_markets.get(poi))
        .or_else(|| {
            world
                .nearest_station
                .as_deref()
                .and_then(|station| world.market.station_markets.get(station))
        });
    let current_market = map_focused_current_market(actor, world, station_market);
    put!("currentMarket", current_market.clone());
    put!(
        "station",
        map_commander_station_context(actor, &world.catalog, current_market)
    );
    Ok(Value::Object(value))
}

fn empty_runtime_catalogue() -> RuntimeCatalogueDto {
    RuntimeCatalogueDto {
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
    }
}

fn projection_system_id_actor(state: &BotState) -> Option<&str> {
    state.effective_system_id().or_else(|| {
        state
            .location
            .in_transit
            .unwrap_or(false)
            .then_some(())
            .and_then(|_| state.location.transit_dest_system_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn projection_poi_id_actor(state: &BotState) -> Option<&str> {
    state.effective_poi_id().or_else(|| {
        state
            .location
            .in_transit
            .unwrap_or(false)
            .then_some(())
            .and_then(|_| state.location.transit_dest_poi_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn effective_poi_type_actor<'a>(
    actor: &'a BotState,
    galaxy: &'a GalaxyData,
    poi_id: &str,
) -> Option<&'a str> {
    galaxy
        .poi_records
        .get(poi_id)
        .map(|poi| poi.info.poi_type.as_str())
        .or_else(|| {
            let system = actor.effective_system_id()?;
            let current = actor.effective_poi_id()?;
            (!actor.location.docked_at.is_some() && poi_id == current && current == system)
                .then_some("space")
        })
}

fn map_poi_resources_from_galaxy(
    galaxy: &GalaxyData,
    poi_id: &str,
) -> Vec<RuntimePoiResourceInfoDto> {
    galaxy
        .poi_records
        .get(poi_id)
        .into_iter()
        .flat_map(|poi| &poi.resources)
        .map(|resource| RuntimePoiResourceInfoDto {
            resource_id: resource.resource_id.clone(),
            name: resource.name.clone(),
            richness_text: resource.richness_text.clone(),
            richness: resource.richness,
            remaining: resource.remaining,
            remaining_display: resource.remaining_display.clone(),
        })
        .collect()
}

fn map_poi_resource(
    resource: &prayer_runtime::engine::PoiResourceData,
) -> RuntimePoiResourceInfoDto {
    RuntimePoiResourceInfoDto {
        resource_id: resource.resource_id.clone(),
        name: resource.name.clone(),
        richness_text: resource.richness_text.clone(),
        richness: resource.richness,
        remaining: resource.remaining,
        remaining_display: resource.remaining_display.clone(),
    }
}

fn map_poi_info_scoped(
    actor: &BotState,
    galaxy: &GalaxyData,
    poi_id: &str,
    fallback_system: &str,
) -> Option<RuntimePoiInfoDto> {
    let info = &galaxy.poi_records.get(poi_id)?.info;
    let base_id = info.base_id.clone();
    Some(RuntimePoiInfoDto {
        id: poi_id.to_string(),
        system_id: if info.system_id.is_empty() {
            fallback_system.to_string()
        } else {
            info.system_id.clone()
        },
        name: if info.name.is_empty() {
            poi_id.to_string()
        } else {
            info.name.clone()
        },
        r#type: if info.poi_type.is_empty() {
            effective_poi_type_actor(actor, galaxy, poi_id)
                .unwrap_or_default()
                .to_string()
        } else {
            info.poi_type.clone()
        },
        class_name: info.class_name.clone(),
        description: info.description.clone(),
        hidden: info.hidden,
        x: info.x,
        y: info.y,
        has_base: info.has_base || base_id.is_some(),
        base_id: base_id.clone(),
        base_name: info.base_name.clone().or(base_id),
        online: info.online.unwrap_or(0),
        fuel_reserve: info.fuel_reserve,
        fuel_capacity: info.fuel_capacity,
        fuel_price: info.fuel_price,
        faction_fuel_reserve: info.faction_fuel_reserve,
        faction_fuel_capacity: info.faction_fuel_capacity,
        resources: map_poi_resources_from_galaxy(galaxy, poi_id),
    })
}

fn map_player_ship_actor(state: &BotState) -> Result<RuntimePlayerShipProjectionDto, SdkError> {
    let system = required_non_empty_state_field("system", projection_system_id_actor(state))?;
    let fuel = if state.max_fuel > 0 {
        state.fuel
    } else {
        state.fuel_pct
    };
    let max_fuel = if state.max_fuel > 0 {
        state.max_fuel
    } else {
        100
    };
    Ok(RuntimePlayerShipProjectionDto {
        name: state.ship.name.clone().unwrap_or_default(),
        class_id: state.ship.class_id.clone().unwrap_or_default(),
        system_id: system,
        armor: state.ship.armor.unwrap_or_default(),
        speed: state.ship.speed.unwrap_or_default(),
        cpu_used: state.ship.cpu_used.unwrap_or_default(),
        cpu_capacity: state.ship.cpu_capacity.unwrap_or_default(),
        power_used: state.ship.power_used.unwrap_or_default(),
        power_capacity: state.ship.power_capacity.unwrap_or_default(),
        module_count: state.modules.len() as i64,
        fuel,
        max_fuel,
        fuel_percent: state.fuel_pct,
        hull: state.ship.hull.unwrap_or_default(),
        max_hull: state.ship.max_hull.unwrap_or_default(),
        shield: state.ship.shield.unwrap_or_default(),
        max_shield: state.ship.max_shield.unwrap_or_default(),
        cargo_used: state.cargo_used,
        cargo_capacity: state.cargo_capacity,
    })
}

fn map_location_scoped(actor: &BotState, world: &WorldReadState) -> RuntimeLocationDto {
    RuntimeLocationDto {
        spacemolt: actor.location.clone(),
        nearby_creature_count: world.nearby_creature_count,
        nearby_creatures: actor
            .location
            .poi_id
            .as_deref()
            .and_then(|poi| world.wildlife_by_poi.get(poi))
            .map(|snapshot| {
                snapshot
                    .creatures
                    .iter()
                    .map(map_wildlife_creature)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn map_focused_current_market(
    actor: &BotState,
    world: &WorldReadState,
    snapshot: Option<&prayer_state::StationMarketData>,
) -> Option<RuntimeMarketStateDto> {
    if !actor.location.docked_at.is_some() {
        return None;
    }
    let poi_id = actor.location.poi_id.clone();
    let station_id = world.nearest_station.clone().or_else(|| poi_id.clone())?;
    Some(RuntimeMarketStateDto {
        station_id,
        station_name: poi_id
            .as_ref()
            .and_then(|id| world.galaxy.poi_records.get(id))
            .map(|poi| poi.info.name.trim())
            .filter(|name| !name.is_empty())
            .map(str::to_string),
        poi_id,
        sell_orders: map_market_orders(&world.market.sell_orders),
        buy_orders: map_market_orders(&world.market.buy_orders),
        observed_at_unix: snapshot.and_then(|value| value.observed_at_unix),
        current_tick: snapshot.and_then(|value| value.current_tick),
    })
}

fn map_commander_station_context(
    actor: &BotState,
    catalog: &CatalogData,
    market: Option<RuntimeMarketStateDto>,
) -> Option<RuntimeStationContextDto> {
    if !actor.location.docked_at.is_some() {
        return None;
    }
    let station_id = projection_poi_id_actor(actor)?.to_string();
    Some(RuntimeStationContextDto {
        station_name: station_id.clone(),
        station_id,
        market,
        shipyard_showroom: map_shipyard_showroom(catalog),
        shipyard_listings: Vec::new(),
        craftable: Vec::new(),
        crafting_queue: actor.crafting_queue.as_ref().clone(),
    })
}

fn map_scoped_passengers(actor: &BotState, world: &WorldReadState) -> RuntimePassengerStateDto {
    RuntimePassengerStateDto {
        aboard_count: actor.passengers.aboard_count,
        economy_berths: RuntimePassengerBerthViewDto {
            current: actor.passengers.economy_berths.current,
            max: actor.passengers.economy_berths.max,
        },
        business_berths: RuntimePassengerBerthViewDto {
            current: actor.passengers.business_berths.current,
            max: actor.passengers.business_berths.max,
        },
        first_berths: RuntimePassengerBerthViewDto {
            current: actor.passengers.first_berths.current,
            max: actor.passengers.first_berths.max,
        },
        aboard: actor.passengers.aboard.as_ref().clone(),
        station: world.station_passengers.station.clone(),
        waiting_count: world.station_passengers.waiting_count,
        waiting: world.station_passengers.waiting.as_ref().clone(),
    }
}

fn map_salvage_value(salvage: &prayer_state::SalvageData) -> RuntimeSalvageStateDto {
    let mut lootables_by_poi = salvage
        .lootables_by_poi
        .iter()
        .map(|(poi, lootables)| {
            (
                poi.clone(),
                lootables
                    .iter()
                    .map(map_space_loot_info)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    for lootables in lootables_by_poi.values_mut() {
        lootables.sort_by(|a, b| a.id.cmp(&b.id));
    }
    let mut visible_lootables = salvage
        .visible_lootables
        .iter()
        .map(map_space_loot_info)
        .collect::<Vec<_>>();
    visible_lootables.sort_by(|a, b| a.id.cmp(&b.id));
    RuntimeSalvageStateDto {
        visible_lootables,
        lootables_by_poi,
        last_seen_poi: salvage.last_seen_poi.clone(),
        last_seen_system: salvage.last_seen_system.clone(),
        observed_at_unix: salvage.observed_at_unix,
    }
}

fn map_shared_galaxy_wildlife(
    wildlife_by_poi: &HashMap<String, WildlifePoiSnapshotData>,
) -> RuntimeGalaxyWildlifeDto {
    let mut pois = wildlife_by_poi
        .values()
        .map(|snapshot| RuntimeWildlifePoiDto {
            system_id: snapshot.system_id.clone(),
            poi_id: snapshot.poi_id.clone(),
            creature_count: snapshot.creature_count,
            observed_at_unix: snapshot.observed_at_unix,
            creatures: snapshot
                .creatures
                .iter()
                .map(map_wildlife_creature)
                .collect(),
        })
        .collect::<Vec<_>>();
    pois.sort_by(|a, b| {
        a.system_id
            .cmp(&b.system_id)
            .then_with(|| a.poi_id.cmp(&b.poi_id))
    });

    let mut by_system: HashMap<String, Vec<&RuntimeWildlifePoiDto>> = HashMap::new();
    for poi in &pois {
        by_system
            .entry(poi.system_id.clone())
            .or_default()
            .push(poi);
    }

    let mut systems = by_system
        .into_iter()
        .map(|(system_id, snapshots)| {
            let mut species_counts: HashMap<(String, String, String), i64> = HashMap::new();
            let mut poi_ids = Vec::new();
            let mut creature_count = 0_i64;
            let mut observed_at_unix = 0_i64;
            for snapshot in snapshots {
                poi_ids.push(snapshot.poi_id.clone());
                creature_count = creature_count.saturating_add(snapshot.creature_count);
                observed_at_unix = observed_at_unix.max(snapshot.observed_at_unix);
                for creature in &snapshot.creatures {
                    *species_counts
                        .entry((
                            creature.species.clone(),
                            creature.name.clone(),
                            creature.role.clone(),
                        ))
                        .or_default() += 1;
                }
            }
            sort_dedup_strings(&mut poi_ids);
            let mut species = species_counts
                .into_iter()
                .map(|((species, name, role), count)| RuntimeWildlifeSpeciesDto {
                    species,
                    name,
                    role,
                    count,
                })
                .collect::<Vec<_>>();
            species.sort_by(|a, b| a.species.cmp(&b.species).then_with(|| a.name.cmp(&b.name)));
            RuntimeWildlifeSystemDto {
                system_id,
                creature_count,
                species,
                pois: poi_ids,
                observed_at_unix,
            }
        })
        .collect::<Vec<_>>();
    systems.sort_by(|a, b| a.system_id.cmp(&b.system_id));

    RuntimeGalaxyWildlifeDto { systems, pois }
}

fn map_wildlife_creature(creature: &WildlifeCreatureData) -> RuntimeWildlifeCreatureDto {
    RuntimeWildlifeCreatureDto {
        creature_id: creature.creature.creature_id.clone(),
        species: creature.creature.species.clone(),
        name: creature.creature.name.clone(),
        role: creature.creature.role.clone(),
        hull: creature.creature.hull,
        max_hull: creature.creature.max_hull,
        in_combat: creature.creature.in_combat,
        system_id: creature.system_id.clone(),
        poi_id: creature.poi_id.clone(),
        observed_at_unix: creature.observed_at_unix,
    }
}

/// Shared-world map projection with no actor fallback or compatibility state.
pub fn map_shared_galaxy_map(galaxy: &GalaxyData) -> Result<RuntimeGalaxyMapSnapshotDto, SdkError> {
    let mut known_pois = galaxy
        .poi_records
        .values()
        .map(|poi| {
            let info = &poi.info;
            RuntimeGalaxyKnownPoiInfoDto {
                id: poi.id.clone(),
                system_id: poi.system_id.clone(),
                name: if info.name.is_empty() {
                    poi.id.clone()
                } else {
                    info.name.clone()
                },
                r#type: info.poi_type.clone(),
                x: info.x,
                y: info.y,
                has_base: info.has_base || info.base_id.is_some(),
                base_id: info.base_id.clone(),
                base_name: info.base_name.clone(),
                resources: poi.resources.iter().map(map_poi_resource).collect(),
                first_discovered_unix: poi.first_discovered_unix,
                last_observed_unix: poi.last_observed_unix,
                first_visited_unix: poi.first_visited_unix,
                last_visited_unix: poi.last_visited_unix,
            }
        })
        .collect::<Vec<_>>();
    known_pois.sort_by(|a, b| a.id.cmp(&b.id));

    let mut systems = galaxy
        .system_records
        .values()
        .map(|system| {
            let (x, y) = system
                .coordinates
                .map(|(x, y)| (Some(x), Some(y)))
                .unwrap_or((None, None));
            let mut poi_ids = galaxy
                .poi_records
                .values()
                .filter_map(|poi| (poi.system_id == system.id).then_some(poi.id.clone()))
                .collect::<Vec<_>>();
            sort_dedup_strings(&mut poi_ids);
            RuntimeGalaxySystemInfoDto {
                id: system.id.clone(),
                name: system.name.clone(),
                empire: system.empire.clone().unwrap_or_default(),
                is_stronghold: system.is_stronghold,
                connections: system.connections.clone(),
                poi_count: system.poi_count,
                pois_complete: system.pois_complete,
                first_entered_unix: system.first_entered_unix,
                last_entered_unix: system.last_entered_unix,
                last_scanned_unix: system.last_scanned_unix,
                last_surveyed_unix: system.last_surveyed_unix,
                bloom_status: system.bloom_status.clone(),
                bloom_intensity: system.bloom_intensity,
                faint_signatures: system.faint_signatures.clone(),
                wildlife: system.wildlife.clone(),
                pois: poi_ids
                    .into_iter()
                    .map(|id| {
                        let info = galaxy.poi_records.get(&id).map(|poi| &poi.info);
                        RuntimeGalaxyPoiInfoDto {
                            x: info.and_then(|value| value.x),
                            y: info.and_then(|value| value.y),
                            id,
                        }
                    })
                    .collect(),
                x,
                y,
            }
        })
        .collect::<Vec<_>>();
    systems.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(RuntimeGalaxyMapSnapshotDto {
        systems,
        known_pois,
    })
}

pub fn map_galaxy_resources(
    galaxy: &GalaxyData,
    actor_system: Option<&str>,
    actor_cargo: &HashMap<String, i64>,
) -> Result<RuntimeGalaxyResourcesDto, SdkError> {
    let mut systems_by_resource = HashMap::<String, Vec<String>>::new();
    let mut pois_by_resource = HashMap::<String, Vec<String>>::new();
    for poi in galaxy.poi_records.values() {
        for resource in &poi.resources {
            push_unique_string(
                systems_by_resource
                    .entry(resource.resource_id.clone())
                    .or_default(),
                &poi.system_id,
            );
            push_unique_string(
                pois_by_resource
                    .entry(resource.resource_id.clone())
                    .or_default(),
                &poi.id,
            );
        }
    }
    if systems_by_resource.is_empty() {
        let mut known_systems = galaxy.system_records.keys().cloned().collect::<Vec<_>>();
        if let Some(system) = actor_system {
            push_unique_string(&mut known_systems, system);
        }
        for item in actor_cargo.keys() {
            systems_by_resource.insert(item.clone(), known_systems.clone());
        }
    }
    Ok(RuntimeGalaxyResourcesDto {
        systems_by_resource,
        pois_by_resource,
    })
}

pub fn map_shared_resource_sources(
    galaxy: &GalaxyData,
    requested: &str,
) -> Result<RuntimeResourceSourcesResponse, SdkError> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err(SdkError::BadRequest("resource_id is required".to_string()));
    }
    let mut known = BTreeSet::new();
    for resource in galaxy.poi_records.values().flat_map(|poi| &poi.resources) {
        if !resource.resource_id.trim().is_empty() {
            known.insert(resource.resource_id.clone());
        }
    }
    let wanted = normalize_lookup_token(requested);
    let resource_id = known
        .iter()
        .find(|id| normalize_lookup_token(id) == wanted)
        .cloned()
        .ok_or_else(|| {
            let suggestion = known
                .iter()
                .min_by_key(|id| levenshtein(&wanted, &normalize_lookup_token(id)))
                .cloned();
            SdkError::BadRequest(suggestion.map_or_else(
                || format!("unknown resource_id '{requested}'"),
                |id| format!("unknown resource_id '{requested}', did you mean '{id}'?"),
            ))
        })?;
    let source_ids = galaxy
        .poi_records
        .values()
        .filter(|poi| {
            poi.resources.iter().any(|value| {
                normalize_lookup_token(&value.resource_id) == normalize_lookup_token(&resource_id)
            })
        })
        .map(|poi| poi.id.clone())
        .collect::<BTreeSet<_>>();
    let mut sources = source_ids
        .into_iter()
        .filter_map(|poi_id| {
            let poi = galaxy.poi_records.get(&poi_id)?;
            let info = &poi.info;
            let system_id = poi.system_id.clone();
            let resource = poi
                .resources
                .iter()
                .find(|v| {
                    normalize_lookup_token(&v.resource_id) == normalize_lookup_token(&resource_id)
                })
                .map(|v| RuntimePoiResourceInfoDto {
                    resource_id: v.resource_id.clone(),
                    name: v.name.clone(),
                    richness_text: v.richness_text.clone(),
                    richness: v.richness,
                    remaining: v.remaining,
                    remaining_display: v.remaining_display.clone(),
                });
            let depleted = resource.as_ref().is_some_and(|v| {
                v.remaining == Some(0) || v.remaining_display.eq_ignore_ascii_case("depleted")
            });
            Some(RuntimeResourceSourcePoiDto {
                name: (!info.name.is_empty())
                    .then(|| info.name.clone())
                    .unwrap_or_else(|| poi_id.clone()),
                r#type: info.poi_type.clone(),
                has_base: info.has_base || info.base_id.is_some(),
                poi_id,
                system_id,
                jumps: None,
                depleted,
                resource,
            })
        })
        .collect::<Vec<_>>();
    sources.sort_by(|a, b| {
        a.depleted
            .cmp(&b.depleted)
            .then(a.system_id.cmp(&b.system_id))
            .then(a.poi_id.cmp(&b.poi_id))
    });
    let destination_ids = galaxy
        .poi_records
        .values()
        .filter(|poi| is_storage_destination_info(&poi.info))
        .map(|poi| poi.id.clone())
        .collect::<BTreeSet<_>>();
    let mut destinations = destination_ids
        .into_iter()
        .filter_map(|poi_id| {
            let poi = galaxy.poi_records.get(&poi_id)?;
            let info = &poi.info;
            let system_id = poi.system_id.clone();
            Some(RuntimeResourceDestinationPoiDto {
                name: (!info.name.is_empty())
                    .then(|| info.name.clone())
                    .unwrap_or_else(|| poi_id.clone()),
                r#type: info.poi_type.clone(),
                has_base: is_storage_destination_info(info),
                poi_id,
                system_id,
                jumps: None,
            })
        })
        .collect::<Vec<_>>();
    destinations.sort_by(|a, b| a.system_id.cmp(&b.system_id).then(a.poi_id.cmp(&b.poi_id)));
    Ok(RuntimeResourceSourcesResponse {
        resource_id,
        sources,
        destinations,
    })
}

pub fn map_actor_resource_sources(
    galaxy: &GalaxyData,
    actor_system: Option<&str>,
    home_base: Option<&str>,
    requested: &str,
) -> Result<RuntimeResourceSourcesResponse, SdkError> {
    let mut response = map_shared_resource_sources(galaxy, requested)?;
    let jumps = |target: &str| {
        actor_system.and_then(|current| {
            galaxy
                .hop_distance(current, target)
                .and_then(|distance| i32::try_from(distance).ok())
        })
    };
    for source in &mut response.sources {
        source.jumps = jumps(&source.system_id);
    }
    for destination in &mut response.destinations {
        destination.jumps = jumps(&destination.system_id);
    }
    if let Some(home) = home_base.map(str::trim).filter(|home| !home.is_empty()) {
        if let Some(index) = response
            .destinations
            .iter()
            .position(|row| row.poi_id == home)
        {
            let mut row = response.destinations.remove(index);
            row.name = format!("{} (home)", row.name);
            response.destinations.insert(0, row);
        }
    }
    response.sources.sort_by(|a, b| {
        a.depleted
            .cmp(&b.depleted)
            .then(a.jumps.is_none().cmp(&b.jumps.is_none()))
            .then(a.jumps.cmp(&b.jumps))
            .then(a.system_id.cmp(&b.system_id))
            .then(a.poi_id.cmp(&b.poi_id))
    });
    response.destinations.sort_by(|a, b| {
        (a.poi_id != home_base.unwrap_or_default())
            .cmp(&(b.poi_id != home_base.unwrap_or_default()))
            .then(a.jumps.is_none().cmp(&b.jumps.is_none()))
            .then(a.jumps.cmp(&b.jumps))
            .then(a.poi_id.cmp(&b.poi_id))
    });
    Ok(response)
}

fn is_storage_destination_info(info: &prayer_runtime::engine::PoiInfoData) -> bool {
    info.has_base || info.base_id.is_some() || info.poi_type.eq_ignore_ascii_case("station")
}

fn normalize_lookup_token(value: &str) -> String {
    value.trim().to_lowercase().replace([' ', '-'], "_")
}

fn levenshtein(a: &str, b: &str) -> usize {
    let mut costs: Vec<usize> = (0..=b.chars().count()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut last = i;
        costs[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let old = costs[j + 1];
            let substitute = last + usize::from(ca != cb);
            costs[j + 1] = (costs[j + 1] + 1).min(costs[j] + 1).min(substitute);
            last = old;
        }
    }
    costs.last().copied().unwrap_or(0)
}

/// Project accumulated agent sightings into the social response. Input is
/// expected pre-sorted (most recently seen first).
pub fn map_social_bots(sightings: Vec<AgentSightingData>) -> SocialResponse {
    let bots = sightings
        .into_iter()
        .map(|s| SocialBotDto {
            actor_kind: "player".to_string(),
            synthetic: false,
            player_id: s.contact.player_id,
            username: s.contact.username.unwrap_or_default(),
            faction_id: s.contact.faction_id,
            faction_tag: s.contact.faction_tag,
            clan_tag: s.contact.clan_tag,
            ship_class: s.contact.ship_class,
            ship_name: s.contact.ship_name,
            status_message: s.contact.status_message,
            primary_color: s.contact.primary_color,
            secondary_color: s.contact.secondary_color,
            in_combat: s.contact.in_combat.unwrap_or(false),
            offline: s.contact.offline.unwrap_or(false),
            last_seen_system: s.last_seen_system,
            first_seen_utc: utc_from_unix(s.first_seen_unix),
            last_seen_utc: utc_from_unix(s.last_seen_unix),
            times_seen: s.times_seen,
        })
        .collect();
    SocialResponse { bots, chat: None }
}

fn utc_from_unix(unix: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(unix, 0).unwrap_or_default()
}

pub fn map_galaxy_market(galaxy: &GalaxyData, market: &MarketData) -> RuntimeGalaxyMarketDto {
    let markets_by_station = market
        .station_markets
        .iter()
        .map(|(station_id, snapshot)| {
            let poi_id = galaxy
                .poi_id_for_base(station_id)
                .map(str::to_string)
                .unwrap_or_else(|| station_id.clone());
            let station_name = galaxy
                .poi_records
                .get(&poi_id)
                .map(|poi| poi.info.name.trim())
                .filter(|name| !name.is_empty())
                .map(str::to_string);
            (
                station_id.clone(),
                RuntimeMarketStateDto {
                    station_id: station_id.clone(),
                    poi_id: Some(poi_id),
                    station_name,
                    sell_orders: map_market_orders(&snapshot.sell_orders),
                    buy_orders: map_market_orders(&snapshot.buy_orders),
                    observed_at_unix: snapshot.observed_at_unix,
                    current_tick: snapshot.current_tick,
                },
            )
        })
        .collect();
    let aggregates = market.global_price_aggregates();
    RuntimeGalaxyMarketDto {
        markets_by_station,
        global_median_buy_prices: aggregates.median_buy_prices,
        global_median_sell_prices: aggregates.median_sell_prices,
        global_weighted_mid_prices: aggregates.weighted_mid_prices,
    }
}

pub fn map_market_query(
    catalog: &CatalogData,
    galaxy: &GalaxyData,
    market: &MarketData,
    actor_system: Option<&str>,
    request: RuntimeMarketQueryRequest,
) -> Result<RuntimeMarketQueryResponse, SdkError> {
    let sides = market_query_sides(request.side.as_deref())?;
    let sort = request
        .sort
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            if sides == [MarketQuerySide::Buy] {
                "price_desc"
            } else {
                "price_asc"
            }
        });
    validate_market_query_sort(sort)?;

    let limit = request.limit.unwrap_or(50).clamp(1, 200);
    let item_filter = normalized_optional_filter(request.item_id.as_deref());
    let station_filter = normalized_optional_filter(request.station.as_deref());
    let system_filter = normalized_optional_filter(request.system.as_deref());
    let own_only = request.own_only.unwrap_or(false);

    let mut orders = Vec::new();
    for (station_id, snapshot) in &market.station_markets {
        let station_info = galaxy.poi_records.get(station_id);
        let station_name = station_info
            .map(|poi| poi.info.name.as_str())
            .filter(|name| !name.trim().is_empty());
        let system_id = station_info
            .map(|poi| poi.system_id.as_str())
            .filter(|system_id| !system_id.trim().is_empty());

        if !market_station_matches(station_id, station_name, station_filter.as_deref()) {
            continue;
        }
        if !market_system_matches(system_id, system_filter.as_deref()) {
            continue;
        }

        for side in sides.iter().copied() {
            let side_orders = match side {
                MarketQuerySide::Sell => &snapshot.sell_orders,
                MarketQuerySide::Buy => &snapshot.buy_orders,
            };
            for (item_id, entries) in side_orders {
                let item_name = market_item_name(catalog, item_id);
                if !market_item_matches(item_id, item_name.as_deref(), item_filter.as_deref()) {
                    continue;
                }
                for order in entries {
                    let price_each = order.price_each as f64;
                    if request.min_price.is_some_and(|min| price_each < min) {
                        continue;
                    }
                    if request.max_price.is_some_and(|max| price_each > max) {
                        continue;
                    }
                    if request.min_quantity.is_some_and(|min| order.quantity < min) {
                        continue;
                    }
                    if own_only && order.my_quantity.unwrap_or(0) <= 0 {
                        continue;
                    }

                    let system_id = system_id.map(ToOwned::to_owned);
                    orders.push(RuntimeMarketQueryOrderProjectionDto {
                        station_id: station_id.clone(),
                        station_name: station_name.map(ToOwned::to_owned),
                        system_id: system_id.clone(),
                        jumps: actor_system.and_then(|current| {
                            system_id.as_deref().and_then(|target| {
                                galaxy
                                    .hop_distance(current, target)
                                    .and_then(|distance| i32::try_from(distance).ok())
                            })
                        }),
                        item_id: item_id.clone(),
                        item_name: item_name.clone(),
                        side: side.as_str().to_string(),
                        price_each,
                        quantity: order.quantity,
                        observed_at_unix: snapshot.observed_at_unix,
                        current_tick: snapshot.current_tick,
                        source: order.source.clone(),
                        my_quantity: order.my_quantity,
                    });
                }
            }
        }
    }

    sort_market_query_orders(&mut orders, sort);
    let total_matches = orders.len();
    orders.truncate(limit);
    let returned = orders.len();
    Ok(RuntimeMarketQueryResponse {
        orders,
        total_matches,
        returned,
        truncated: total_matches > returned,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketQuerySide {
    Sell,
    Buy,
}

impl MarketQuerySide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Sell => "sell",
            Self::Buy => "buy",
        }
    }
}

fn market_query_sides(side: Option<&str>) -> Result<Vec<MarketQuerySide>, SdkError> {
    match side.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(vec![MarketQuerySide::Sell, MarketQuerySide::Buy]),
        Some(side) if side.eq_ignore_ascii_case("both") || side.eq_ignore_ascii_case("all") => {
            Ok(vec![MarketQuerySide::Sell, MarketQuerySide::Buy])
        }
        Some(side)
            if side.eq_ignore_ascii_case("sell")
                || side.eq_ignore_ascii_case("ask")
                || side.eq_ignore_ascii_case("asks") =>
        {
            Ok(vec![MarketQuerySide::Sell])
        }
        Some(side)
            if side.eq_ignore_ascii_case("buy")
                || side.eq_ignore_ascii_case("bid")
                || side.eq_ignore_ascii_case("bids") =>
        {
            Ok(vec![MarketQuerySide::Buy])
        }
        Some(side) => Err(SdkError::BadRequest(format!(
            "unsupported market side '{side}'; use sell, buy, or both"
        ))),
    }
}

fn validate_market_query_sort(sort: &str) -> Result<(), SdkError> {
    match sort {
        "price_asc" | "price_desc" | "quantity_desc" | "jumps" | "station" | "item" => Ok(()),
        other => Err(SdkError::BadRequest(format!(
            "unsupported market sort '{other}'; use price_asc, price_desc, quantity_desc, jumps, station, or item"
        ))),
    }
}

fn normalized_optional_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_lookup_token)
}

fn market_station_matches(
    station_id: &str,
    station_name: Option<&str>,
    filter: Option<&str>,
) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    normalize_lookup_token(station_id).contains(filter)
        || station_name
            .map(normalize_lookup_token)
            .is_some_and(|name| name.contains(filter))
}

fn market_system_matches(system_id: Option<&str>, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    system_id
        .map(normalize_lookup_token)
        .is_some_and(|system_id| system_id == filter || system_id.contains(filter))
}

fn market_item_matches(item_id: &str, item_name: Option<&str>, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    normalize_lookup_token(item_id).contains(filter)
        || item_name
            .map(normalize_lookup_token)
            .is_some_and(|name| name.contains(filter))
}

fn market_item_name(catalog: &CatalogData, item_id: &str) -> Option<String> {
    catalog
        .items
        .get(item_id)
        .map(spacemolt_lib_rs::schema::CatalogDumpItemsItem::name)
        .filter(|name| !name.eq_ignore_ascii_case(item_id))
        .map(ToOwned::to_owned)
}

fn sort_market_query_orders(orders: &mut [RuntimeMarketQueryOrderProjectionDto], sort: &str) {
    match sort {
        "price_desc" => orders.sort_by(|a, b| {
            b.price_each
                .total_cmp(&a.price_each)
                .then_with(|| market_query_tiebreak(a, b))
        }),
        "quantity_desc" => orders.sort_by(|a, b| {
            b.quantity
                .cmp(&a.quantity)
                .then_with(|| market_query_tiebreak(a, b))
        }),
        "jumps" => orders.sort_by(|a, b| {
            a.jumps
                .unwrap_or(i32::MAX)
                .cmp(&b.jumps.unwrap_or(i32::MAX))
                .then_with(|| market_query_tiebreak(a, b))
        }),
        "station" => orders.sort_by(|a, b| {
            a.station_id
                .cmp(&b.station_id)
                .then_with(|| a.item_id.cmp(&b.item_id))
                .then_with(|| a.side.cmp(&b.side))
                .then_with(|| a.price_each.total_cmp(&b.price_each))
        }),
        "item" => orders.sort_by(|a, b| {
            a.item_id
                .cmp(&b.item_id)
                .then_with(|| a.side.cmp(&b.side))
                .then_with(|| a.price_each.total_cmp(&b.price_each))
                .then_with(|| a.station_id.cmp(&b.station_id))
        }),
        _ => orders.sort_by(|a, b| {
            a.price_each
                .total_cmp(&b.price_each)
                .then_with(|| market_query_tiebreak(a, b))
        }),
    }
}

fn market_query_tiebreak(
    a: &RuntimeMarketQueryOrderProjectionDto,
    b: &RuntimeMarketQueryOrderProjectionDto,
) -> std::cmp::Ordering {
    a.jumps
        .unwrap_or(i32::MAX)
        .cmp(&b.jumps.unwrap_or(i32::MAX))
        .then_with(|| a.station_id.cmp(&b.station_id))
        .then_with(|| a.item_id.cmp(&b.item_id))
        .then_with(|| a.side.cmp(&b.side))
}

fn map_market_orders(
    orders: &HashMap<String, Vec<MarketOrder>>,
) -> HashMap<String, Vec<MarketOrder>> {
    orders.clone()
}

pub fn map_arbitrage_deal(
    deal: prayer_runtime::economy::ArbitrageDeal,
    state: &prayer_runtime::economy::EconomyReadState,
) -> RuntimeArbitrageDealDto {
    let source_kind = deal.acquire_from.kind().to_string();
    let source_station_id = state
        .galaxy
        .poi_id_for_base(&deal.buy_station_id)
        .unwrap_or(&deal.buy_station_id);
    let source_owner = match source_kind.as_str() {
        "market" => Some("market"),
        "personal_storage" => state.player_id.as_deref().or(state.username.as_deref()),
        "virtual_faction" => state.faction_id.as_deref(),
        _ => None,
    };
    let acquire_from = RuntimeArbitrageAcquireFromDto {
        kind: source_kind.clone(),
        virtual_order_id: deal.acquire_from.virtual_order_id().map(str::to_string),
        claim_key: source_owner.map(|owner| {
            let claim_kind = match source_kind.as_str() {
                "virtual_faction" => "faction_storage",
                other => other,
            };
            format!("{claim_kind}|{owner}|{source_station_id}|{}", deal.item_id)
        }),
    };
    let dispose_to = RuntimeArbitrageDisposeToDto {
        kind: deal.dispose_to.kind().to_string(),
        virtual_order_id: deal.dispose_to.virtual_order_id().map(str::to_string),
    };
    RuntimeArbitrageDealDto {
        item_id: deal.item_id,
        buy_station_id: deal.buy_station_id,
        buy_system_id: deal.buy_system_id,
        acquire_from,
        buy_price: deal.buy_price,
        sell_station_id: deal.sell_station_id,
        sell_system_id: deal.sell_system_id,
        dispose_to,
        sell_price: deal.sell_price,
        profit_per_unit: deal.profit_per_unit,
        item_size: deal.item_size,
        quantity: deal.quantity,
        total_profit: deal.total_profit,
        capital_required: deal.capital_required,
        roi: deal.roi,
        gross_margin: deal.gross_margin,
        break_even_cover: deal.break_even_cover,
        risk_band: deal.risk_band.as_str().to_string(),
        jumps_to_buy: deal.jumps_to_buy as i64,
        jumps_buy_to_sell: deal.jumps_buy_to_sell as i64,
        data_age_seconds: deal.data_age_seconds,
        raw_score: deal.raw_score,
        score: deal.score,
    }
}

pub fn map_arbitrade_package(
    package: prayer_runtime::economy::ArbitradePackage,
    state: &prayer_runtime::economy::EconomyReadState,
) -> RuntimeArbitradePackageDto {
    let deals = package
        .deals
        .iter()
        .cloned()
        .map(|deal| map_arbitrage_deal(deal, state))
        .collect::<Vec<_>>();
    let passenger_fares = package
        .passenger_fares
        .iter()
        .cloned()
        .map(map_passenger_fare_deal)
        .collect::<Vec<_>>();
    let members = package
        .members
        .into_iter()
        .map(|member| match member {
            prayer_runtime::economy::ArbitradePackageMember::ItemDeal(deal) => {
                RuntimeArbitradePackageMemberDto::ItemDeal {
                    deal: map_arbitrage_deal(deal, state),
                }
            }
            prayer_runtime::economy::ArbitradePackageMember::PassengerFare(fare) => {
                RuntimeArbitradePackageMemberDto::PassengerFare {
                    fare: map_passenger_fare_deal(fare),
                }
            }
        })
        .collect();
    RuntimeArbitradePackageDto {
        buy_station_id: package.buy_station_id,
        buy_system_id: package.buy_system_id,
        sell_station_id: package.sell_station_id,
        sell_system_id: package.sell_system_id,
        deals,
        members,
        passenger_fares,
        cargo_used: package.cargo_used,
        cargo_capacity: package.cargo_capacity,
        capital_required: package.capital_required,
        total_profit: package.total_profit,
        passenger_revenue: package.passenger_revenue,
        berth_used: map_passenger_berth_usage(package.berth_used),
        berth_capacity: map_passenger_berth_usage(package.berth_capacity),
        roi: package.roi,
        gross_margin: package.gross_margin,
        break_even_cover: package.break_even_cover,
        risk_band: package.risk_band.as_str().to_string(),
        jumps_to_buy: package.jumps_to_buy as i64,
        jumps_buy_to_sell: package.jumps_buy_to_sell as i64,
        data_age_seconds: package.data_age_seconds,
        raw_score: package.raw_score,
        score: package.score,
        anchor_kind: package.anchor_kind.as_str().to_string(),
    }
}

fn map_passenger_berth_usage(
    usage: prayer_runtime::economy::PassengerBerthUsage,
) -> RuntimePassengerBerthUsageDto {
    RuntimePassengerBerthUsageDto {
        economy: usage.economy,
        business: usage.business,
        first: usage.first,
    }
}

fn map_passenger_fare_deal(
    fare: prayer_runtime::economy::PassengerFareDeal,
) -> RuntimePassengerFareDealDto {
    RuntimePassengerFareDealDto {
        citizen_id: fare.citizen_id,
        name: fare.name,
        class_name: fare.class_name,
        origin_station_id: fare.origin_station_id,
        destination_station_id: fare.destination_station_id,
        destination_system_id: fare.destination_system_id,
        estimated_fare: fare.estimated_fare,
        base_fare: fare.base_fare,
        speed_bonus: fare.speed_bonus,
        berth_units: fare.berth_units,
        total_jumps: fare.total_jumps as i64,
        fare_per_jump: fare.fare_per_jump,
        score: fare.score,
        risk_band: fare.risk_band.to_string(),
    }
}

pub fn map_logistics_package(
    package: prayer_runtime::economy::LogisticsPackage,
) -> RuntimeLogisticsPackageDto {
    let source_station_id = package.source_station_id;
    let items = package
        .items
        .into_iter()
        .map(|item| map_logistics_item(item, &source_station_id))
        .collect();
    RuntimeLogisticsPackageDto {
        source_station_id,
        source_system_id: package.source_system_id,
        destination_station_id: package.destination_station_id,
        destination_system_id: package.destination_system_id,
        items,
        cargo_used: package.cargo_used,
        cargo_capacity: package.cargo_capacity,
        jumps_to_source: package.jumps_to_source as i64,
        jumps_source_to_destination: package.jumps_source_to_destination as i64,
        total_jumps: package.total_jumps as i64,
        score: package.score,
    }
}

fn map_logistics_item(
    item: prayer_runtime::economy::LogisticsItem,
    source_station_id: &str,
) -> RuntimeLogisticsItemDto {
    let source_kind = item.source.kind().to_string();
    let source_claim_key = (source_kind == "market")
        .then(|| format!("market|market|{source_station_id}|{}", item.item_id));
    RuntimeLogisticsItemDto {
        item_id: item.item_id,
        quantity: item.quantity,
        item_size: item.item_size,
        source_price: item.source_price,
        destination_price: item.destination_price,
        source: RuntimeLogisticsEndpointDto {
            kind: source_kind,
            virtual_order_id: item.source.virtual_order_id().map(str::to_string),
            claim_key: source_claim_key,
        },
        destination: RuntimeLogisticsEndpointDto {
            kind: item.destination.kind().to_string(),
            virtual_order_id: item.destination.virtual_order_id().map(str::to_string),
            claim_key: None,
        },
        priority: item.priority,
        value_per_unit: item.value_per_unit,
        route_value: item.route_value,
        score: item.score,
    }
}

/// Market order books for the docked station, when known.
pub fn map_galaxy_catalog(catalog: &CatalogData) -> RuntimeGalaxyCatalogDto {
    RuntimeGalaxyCatalogDto {
        items_by_id: catalog.items.clone(),
        ships_by_id: catalog.ships.clone(),
        recipes_by_id: catalog.recipes.clone(),
        facilities_by_id: catalog.facilities.clone(),
        skills_by_id: catalog.skills.clone(),
    }
}

pub fn map_focused_station_context(
    actor: &BotState,
    catalog: &CatalogData,
    galaxy: &GalaxyData,
    station_market: Option<&prayer_state::StationMarketData>,
) -> Option<RuntimeStationContextDto> {
    if !actor.location.docked_at.is_some() {
        return None;
    }
    let station_id = actor
        .location
        .poi_id
        .clone()
        .or_else(|| actor.location.system_id.clone())?;
    let market = station_market.map(|snapshot| RuntimeMarketStateDto {
        station_id: station_id.clone(),
        station_name: galaxy
            .poi_records
            .get(&station_id)
            .map(|poi| poi.info.name.clone())
            .filter(|name| !name.trim().is_empty()),
        poi_id: Some(station_id.clone()),
        sell_orders: map_market_orders(&snapshot.sell_orders),
        buy_orders: map_market_orders(&snapshot.buy_orders),
        observed_at_unix: snapshot.observed_at_unix,
        current_tick: snapshot.current_tick,
    });
    Some(RuntimeStationContextDto {
        station_name: galaxy
            .poi_records
            .get(&station_id)
            .map(|poi| poi.info.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| station_id.clone()),
        station_id,
        market,
        shipyard_showroom: map_shipyard_showroom(catalog),
        shipyard_listings: Vec::new(),
        craftable: Vec::new(),
        crafting_queue: actor.crafting_queue.as_ref().clone(),
    })
}

pub fn map_actor_active_missions(
    state: &BotState,
) -> Vec<spacemolt_lib_rs::schema::V2GameStateMissionsActiveItem> {
    state.missions.active_details.clone()
}

pub fn map_actor_available_missions(
    state: &BotState,
) -> Vec<spacemolt_lib_rs::schema::MissionInfo> {
    state.missions.available_details.clone()
}

fn required_non_empty_state_field(field: &str, value: Option<&str>) -> Result<String, SdkError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Ok(v.to_string()),
        None => Err(SdkError::InvalidRuntimeState(format!(
            "missing required runtime state field '{field}'"
        ))),
    }
}

/// Trim a runtime state field, returning an empty string when it is absent or
/// blank. Used by galaxy-wide projections that tolerate a missing position.
fn push_unique_string(values: &mut Vec<String>, candidate: &str) {
    if !candidate.is_empty() && !values.iter().any(|value| value == candidate) {
        values.push(candidate.to_string());
    }
}

fn sort_dedup_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

pub fn map_actor_owned_ships(actor: &BotState) -> Vec<RuntimeOwnedShipProjectionDto> {
    map_owned_ship_collections(&actor.owned_ship_details)
}

fn map_owned_ship_collections(
    details: &[spacemolt_lib_rs::schema::OwnedShipInfo],
) -> Vec<RuntimeOwnedShipProjectionDto> {
    details.iter().map(map_owned_ship_detail).collect()
}

pub fn map_actor_active_commissions(actor: &BotState) -> Vec<ActiveCommissionInfo> {
    actor.active_commissions.as_ref().clone()
}

fn map_owned_ship_detail(
    ship: &spacemolt_lib_rs::schema::OwnedShipInfo,
) -> RuntimeOwnedShipProjectionDto {
    RuntimeOwnedShipProjectionDto {
        owner_handle: String::new(),
        owner_kind: String::new(),
        owner_id: String::new(),
        owner_name: String::new(),
        faction_id: String::new(),
        faction_tag: String::new(),
        ship: ship.clone(),
    }
}

pub fn map_faction_garage_value(
    garage: &prayer_state::FactionGarageInfo,
) -> RuntimeFactionGarageDto {
    RuntimeFactionGarageDto {
        used: garage.used,
        capacity: garage.capacity,
        ships: garage.ships.iter().map(map_faction_garage_ship).collect(),
    }
}

fn map_faction_garage_ship(
    ship: &FactionGarageShipObservation,
) -> RuntimeFactionGarageShipProjectionDto {
    RuntimeFactionGarageShipProjectionDto {
        owner_handle: String::new(),
        base_id: ship.base_id.clone(),
        base_name: ship.base_name.clone(),
        system_name: ship.system_name.clone(),
        faction_id: String::new(),
        faction_tag: String::new(),
        ship: ship.ship.clone(),
    }
}

fn map_item_stacks(
    items: &HashMap<String, i64>,
) -> HashMap<String, RuntimeItemQuantityProjectionDto> {
    items
        .iter()
        .map(|(id, quantity)| {
            (
                id.clone(),
                RuntimeItemQuantityProjectionDto {
                    item_id: id.clone(),
                    quantity: *quantity,
                },
            )
        })
        .collect()
}

pub fn map_item_quantities(
    items: &HashMap<String, i64>,
) -> HashMap<String, RuntimeItemQuantityProjectionDto> {
    map_item_stacks(items)
}

fn map_space_loot_info(
    lootable: &prayer_runtime::engine::SpaceLootInfo,
) -> RuntimeSpaceLootInfoDto {
    RuntimeSpaceLootInfoDto {
        id: lootable.id.clone(),
        kind: lootable.kind.clone(),
        poi_id: lootable.poi_id.clone(),
        system_id: lootable.system_id.clone(),
        cargo: lootable.cargo.clone(),
        modules: lootable.modules.clone(),
        salvage_value: lootable.salvage_value,
        created_at: lootable.created_at.clone(),
        expires_at: lootable.expires_at.clone(),
        expire_tick: lootable.expire_tick,
        ship_class: lootable.ship_class.clone(),
        ship_name: lootable.ship_name.clone(),
        victim_name: lootable.victim_name.clone(),
        killer_name: lootable.killer_name.clone(),
    }
}

fn map_shipyard_showroom(catalog: &CatalogData) -> Vec<RuntimeShipyardShowroomEntryDto> {
    let mut ships = if !catalog.ships.is_empty() {
        catalog
            .ships
            .iter()
            .filter_map(|(id, entry)| make_shipyard_showroom_entry(id, Some(entry)))
            .collect::<Vec<_>>()
    } else {
        catalog
            .ships
            .keys()
            .filter_map(|id| make_shipyard_showroom_entry(id, None))
            .collect::<Vec<_>>()
    };
    ships.sort_by(|a, b| {
        a.tier
            .unwrap_or(i64::MAX)
            .cmp(&b.tier.unwrap_or(i64::MAX))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.ship_class_id.cmp(&b.ship_class_id))
    });
    ships
}

fn make_shipyard_showroom_entry(
    id: &str,
    source: Option<&spacemolt_lib_rs::schema::ShipClass>,
) -> Option<RuntimeShipyardShowroomEntryDto> {
    if source.and_then(|ship| ship.hidden).unwrap_or(false) {
        return None;
    }
    Some(RuntimeShipyardShowroomEntryDto {
        ship_class_id: id.to_string(),
        ship_id: None,
        name: source
            .map(|ship| ship.name.clone())
            .unwrap_or_else(|| id.to_string()),
        category: source
            .map(|ship| ship.category.clone().unwrap_or_else(|| ship.class.clone()))
            .unwrap_or_default(),
        tier: source.and_then(|ship| ship.shipyard_tier),
        scale: source.and_then(|ship| ship.scale),
        hull: source.and_then(|ship| ship.base_hull),
        shield: source.and_then(|ship| ship.base_shield),
        cargo: source.and_then(|ship| ship.cargo_capacity),
        speed: source.and_then(|ship| ship.base_speed),
        price: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commander_projection_requires_actor_position() {
        let error = map_commander_session_state(&BotState::default(), &WorldReadState::default())
            .expect_err("missing actor position must fail");
        assert!(error.to_string().contains("system"));
    }

    #[test]
    fn galaxy_map_exposes_survey_observations() {
        let mut galaxy = GalaxyData::default();
        galaxy.system_records.insert(
            "sol".into(),
            prayer_state::SystemKnowledge {
                id: "sol".into(),
                faint_signatures: vec![
                    serde_json::json!({"type": "mineral", "difficulty": "hard"}),
                ],
                wildlife: vec![serde_json::json!({"species": "void_ray", "estimate": 2})],
                ..Default::default()
            },
        );

        let map = map_shared_galaxy_map(&galaxy).expect("galaxy map");
        assert_eq!(map.systems[0].faint_signatures[0]["type"], "mineral");
        assert_eq!(map.systems[0].wildlife[0]["species"], "void_ray");
    }
}
