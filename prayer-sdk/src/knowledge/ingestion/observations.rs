//! Observation ingestion, merge semantics, and change detection.

use super::super::*;

pub fn merge_knowledge_state_with_metadata(
    knowledge: &mut WorldState,
    metadata: &mut prayer_runtime::knowledge::WorldRuntimeMetadata,
    observation: &StateObservation,
) -> bool {
    let fetched = &observation.bot.state;
    let world = &observation.world;
    let fetched_galaxy = world.galaxy.as_ref();
    let mut versioned_changed = false;

    if let Some(catalog) = &observation.catalog {
        if knowledge.catalog.as_ref() != catalog {
            info!(
                catalog_version = catalog.version.as_deref().unwrap_or("(none)"),
                fetched_items = catalog.items.len(),
                fetched_ships = catalog.ships.len(),
                fetched_recipes = catalog.recipes.len(),
                fetched_facilities = catalog.facilities.len(),
                fetched_skills = catalog.skills.len(),
                "replacing canonical catalog snapshot"
            );
            knowledge.catalog = Arc::new(catalog.clone());
            versioned_changed = true;
        }
    }

    if galaxy_observation_changes(knowledge.galaxy.as_ref(), observation) {
        let mut galaxy = knowledge.galaxy.as_ref().clone();
        for (system_id, incoming) in &fetched_galaxy.system_records {
            if incoming.pois_complete {
                let current_stamp = galaxy
                    .system_records
                    .get(system_id)
                    .and_then(|system| system.last_scanned_unix)
                    .unwrap_or_default();
                let incoming_stamp = incoming
                    .last_scanned_unix
                    .unwrap_or(incoming.observed_at_unix);
                if incoming_stamp >= current_stamp {
                    let incoming_ids = fetched_galaxy
                        .poi_records
                        .values()
                        .filter(|poi| poi.system_id == *system_id)
                        .map(|poi| poi.id.as_str())
                        .collect::<HashSet<_>>();
                    galaxy.poi_records.retain(|id, poi| {
                        poi.system_id != *system_id || incoming_ids.contains(id.as_str())
                    });
                }
            }
        }

        for (id, incoming) in &fetched_galaxy.system_records {
            merge_system_record(
                galaxy.system_records.entry(id.clone()).or_default(),
                incoming,
            );
        }
        for (id, incoming) in &fetched_galaxy.poi_records {
            merge_poi_record(galaxy.poi_records.entry(id.clone()).or_default(), incoming);
        }

        if galaxy != *knowledge.galaxy {
            knowledge.galaxy = Arc::new(galaxy);
            versioned_changed = true;
        }
    }
    if observation.map_fetched {
        metadata.map_fetched_at = Some(Instant::now());
    }
    if observation.agents_fetched {
        if let Some(system_id) = observation
            .agents
            .as_ref()
            .map(|agents| agents.system_id.as_str())
            .or(observation.status_system.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let system_id = canonical_system_id_for_knowledge(knowledge, system_id);
            metadata
                .agents_fetched_at_by_system
                .insert(system_id, Instant::now());
        }
    }
    if observation.nearby_fetched {
        if let Some(poi_id) = observation
            .wildlife
            .as_ref()
            .map(|wildlife| wildlife.snapshot.poi_id.as_str())
            .or(observation.status_poi.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            metadata
                .nearby_fetched_at_by_poi
                .insert(poi_id.to_string(), Instant::now());
        }
    }
    if observation.wrecks_fetched {
        if let Some(poi_id) = world
            .salvage
            .last_seen_poi
            .as_deref()
            .or(observation.status_poi.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            metadata
                .wrecks_fetched_at_by_poi
                .insert(poi_id.to_string(), Instant::now());
        }
    }
    if observation.docked_faction_storage_fetched {
        if let (Some(faction_id), Some(station_id)) = (
            faction_storage_key_for_actor(fetched),
            fetched
                .location
                .poi_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        ) {
            metadata.faction_storage_fetched_at_by_key.insert(
                faction_station_storage_key(faction_id, station_id),
                Instant::now(),
            );
        }
    }
    if observation.docked_storage_fetched {
        if let Some(player_id) = storage_player_key_for_actor(fetched) {
            let observed_at = Instant::now();
            for station_id in world
                .storage
                .keys()
                .map(String::as_str)
                .chain(fetched.location.poi_id.as_deref().into_iter())
            {
                let station_id = station_id.trim();
                if !station_id.is_empty() {
                    metadata.storage_fetched_at_by_key.insert(
                        player_station_storage_key(player_id, station_id),
                        observed_at,
                    );
                }
            }
        }
    }
    if world
        .market
        .shipyard_listings
        .iter()
        .any(|id| !knowledge.shipyard_listing_ids.contains(id))
    {
        versioned_changed = true;
    }
    merge_unique_strings(
        &mut knowledge.shipyard_listing_ids,
        &world.market.shipyard_listings,
    );
    // Market memory is global within this process only: the latest snapshot
    // per station wins, but station order books are intentionally not
    // persisted across restarts.
    for (station_id, snapshot) in &world.market.station_markets {
        if !knowledge
            .station_markets
            .get(station_id)
            .is_some_and(|known| {
                prayer_runtime::knowledge::station_market_snapshot_eq(known, snapshot)
            })
        {
            versioned_changed = true;
        }
        knowledge
            .station_markets
            .insert(station_id.clone(), snapshot.clone());
    }
    if observation.docked_passengers_fetched {
        let station_id = fetched
            .location
            .poi_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| world.passengers.station.as_str())
            .trim();
        let passenger_station = world.passengers.station.trim();
        if !station_id.is_empty() {
            let mut station_passengers = prayer_state::PassengerState::default();
            station_passengers.station = station_id.to_string();
            station_passengers.waiting_count = world.passengers.waiting_count;
            station_passengers.waiting = Arc::clone(&world.passengers.waiting);
            knowledge
                .station_passengers
                .insert(station_id.to_string(), station_passengers);
            metadata
                .station_passengers_fetched_at_by_station
                .insert(station_id.to_string(), Instant::now());
            info!(
                station_id,
                passenger_station,
                waiting_count = world.passengers.waiting_count,
                waiting_len = world.passengers.waiting.len(),
                "merged station passenger board into knowledge"
            );
        } else {
            info!(
                current_poi = fetched.location.poi_id.as_deref().unwrap_or("(none)"),
                passenger_station = %world.passengers.station,
                "skipped station passenger board merge without station id"
            );
        }
    }
    if let Some(poi_id) = world
        .salvage
        .last_seen_poi
        .as_deref()
        .filter(|v| !v.trim().is_empty())
    {
        knowledge
            .salvage_by_poi
            .insert(poi_id.to_string(), world.salvage.as_ref().clone());
    }
    if !world.storage.is_empty() {
        if let Some(player_key) = storage_player_key_for_actor(fetched) {
            let storage = knowledge
                .storage_by_player
                .entry(player_key.to_string())
                .or_default();
            for (station_id, items) in world.storage.iter() {
                if storage.get(station_id) != Some(items) {
                    versioned_changed = true;
                }
                storage.insert(station_id.clone(), items.clone());
            }
        }
    }
    if observation.docked_faction_storage_fetched {
        if let Some(faction_key) = faction_storage_key_for_actor(fetched) {
            if let Some(poi_id) = fetched
                .location
                .poi_id
                .as_deref()
                .map(str::trim)
                .filter(|poi| !poi.is_empty())
            {
                if knowledge
                    .faction_storage_by_faction_poi
                    .get(faction_key)
                    .and_then(|stations| stations.get(poi_id))
                    != Some(world.faction_storage.as_ref())
                {
                    versioned_changed = true;
                }
                knowledge
                    .faction_storage_by_faction_poi
                    .entry(faction_key.to_string())
                    .or_default()
                    .insert(poi_id.to_string(), world.faction_storage.as_ref().clone());
            }
        }
    }
    if !world.faction_garage.ships.is_empty() {
        if let Some(faction_key) = faction_garage_key_for_actor(fetched) {
            if faction_garage_has_locations(&world.faction_garage) {
                knowledge
                    .faction_garage_by_faction
                    .insert(faction_key.to_string(), world.faction_garage.clone());
            }
        }
    }
    if let Some(agents) = &observation.agents {
        let canonical_system = canonical_system_id_for_knowledge(knowledge, &agents.system_id);
        let mut current_agents = Vec::new();
        for sighting in &agents.agents {
            let mut sighting = sighting.clone();
            sighting.last_seen_system = canonical_system.clone();
            let key = sighting.sighting_key().to_string();
            let before = knowledge.agent_sightings.get(&key).cloned();
            merge_agent_sighting(&mut knowledge.agent_sightings, &sighting);
            let after = knowledge.agent_sightings.get(&key);
            if before.as_ref() != after {
                versioned_changed = true;
            }
            current_agents.push(sighting);
        }
        current_agents.sort_by(|a, b| {
            a.sighting_key()
                .cmp(b.sighting_key())
                .then_with(|| a.contact.username.cmp(&b.contact.username))
        });
        if !knowledge
            .system_agents_by_system
            .get(&canonical_system)
            .is_some_and(|known| {
                known.len() == current_agents.len()
                    && known.iter().zip(&current_agents).all(|(left, right)| {
                        prayer_runtime::knowledge::agent_current_snapshot_eq(left, right)
                    })
            })
        {
            versioned_changed = true;
        }
        knowledge
            .system_agents_by_system
            .insert(canonical_system, current_agents);
    }
    if let Some(wildlife) = &observation.wildlife {
        if !wildlife.snapshot.poi_id.trim().is_empty() {
            if !knowledge
                .wildlife_by_poi
                .get(&wildlife.snapshot.poi_id)
                .is_some_and(|known| {
                    prayer_runtime::knowledge::wildlife_snapshot_eq(known, &wildlife.snapshot)
                })
            {
                versioned_changed = true;
            }
            knowledge
                .wildlife_by_poi
                .insert(wildlife.snapshot.poi_id.clone(), wildlife.snapshot.clone());
        }
    }
    versioned_changed
}

fn galaxy_observation_changes(current: &GalaxyData, observation: &StateObservation) -> bool {
    let incoming = observation.world.galaxy.as_ref();
    if canonical_records_change(current, incoming) {
        return true;
    }
    incoming.system_records.iter().any(|(system_id, system)| {
        if !system.pois_complete {
            return false;
        }
        let current_stamp = current
            .system_records
            .get(system_id)
            .and_then(|known| known.last_scanned_unix)
            .unwrap_or_default();
        let incoming_stamp = system.last_scanned_unix.unwrap_or(system.observed_at_unix);
        if incoming_stamp < current_stamp {
            return false;
        }
        let current_ids = current
            .poi_records
            .values()
            .filter(|poi| poi.system_id == *system_id)
            .map(|poi| poi.id.as_str())
            .collect::<HashSet<_>>();
        let incoming_ids = incoming
            .poi_records
            .values()
            .filter(|poi| poi.system_id == *system_id)
            .map(|poi| poi.id.as_str())
            .collect::<HashSet<_>>();
        current_ids != incoming_ids
    })
}

fn canonical_records_change(current: &GalaxyData, incoming: &GalaxyData) -> bool {
    let mut probe = current.clone();
    for (id, value) in &incoming.system_records {
        merge_system_record(probe.system_records.entry(id.clone()).or_default(), value);
    }
    for (id, value) in &incoming.poi_records {
        merge_poi_record(probe.poi_records.entry(id.clone()).or_default(), value);
    }
    probe.system_records != current.system_records || probe.poi_records != current.poi_records
}

fn merge_system_record(
    current: &mut prayer_state::SystemKnowledge,
    incoming: &prayer_state::SystemKnowledge,
) {
    if current.id.is_empty() {
        *current = incoming.clone();
        return;
    }
    if current.id != incoming.id {
        warn!(current_id = %current.id, incoming_id = %incoming.id, "rejected conflicting system observation identity");
        return;
    }
    let newer = incoming.observed_at_unix >= current.observed_at_unix;
    let facts_differ = incoming
        .name
        .as_ref()
        .is_some_and(|v| current.name.as_ref() != Some(v))
        || incoming
            .coordinates
            .is_some_and(|v| current.coordinates != Some(v))
        || ((incoming.connections_complete || !incoming.connections.is_empty())
            && current.connections != incoming.connections)
        || incoming
            .empire
            .as_ref()
            .is_some_and(|v| current.empire.as_ref() != Some(v))
        || (incoming.stronghold_observed
            && (!current.stronghold_observed || incoming.is_stronghold != current.is_stronghold))
        || (incoming.pois_complete
            && (current.poi_count != incoming.poi_count || !current.pois_complete))
        || (incoming.survey_complete
            && (!current.survey_complete
                || current.bloom_status != incoming.bloom_status
                || current.bloom_intensity != incoming.bloom_intensity
                || current.faint_signatures != incoming.faint_signatures
                || current.wildlife != incoming.wildlife));
    if newer && facts_differ {
        if incoming.name.is_some() {
            current.name = incoming.name.clone();
        }
        if incoming.coordinates.is_some() {
            current.coordinates = incoming.coordinates;
        }
        if incoming.connections_complete {
            current.connections = incoming.connections.clone();
            current.connections_complete = true;
        } else {
            for connection in &incoming.connections {
                if !current.connections.contains(connection) {
                    current.connections.push(connection.clone());
                }
            }
        }
        if incoming.empire.is_some() {
            current.empire = incoming.empire.clone();
        }
        if incoming.stronghold_observed {
            current.is_stronghold = incoming.is_stronghold;
            current.stronghold_observed = true;
        }
        if incoming.pois_complete {
            current.poi_count = incoming.poi_count;
            current.pois_complete = true;
        }
        if incoming.survey_complete {
            current.bloom_status = incoming.bloom_status.clone();
            current.bloom_intensity = incoming.bloom_intensity;
            current.faint_signatures = incoming.faint_signatures.clone();
            current.wildlife = incoming.wildlife.clone();
            current.survey_complete = true;
        }
        current.observed_at_unix = incoming.observed_at_unix;
    }
    current.first_entered_unix = earliest(current.first_entered_unix, incoming.first_entered_unix);
    current.last_entered_unix = latest(current.last_entered_unix, incoming.last_entered_unix);
    current.last_scanned_unix = latest(current.last_scanned_unix, incoming.last_scanned_unix);
    current.last_surveyed_unix = latest(current.last_surveyed_unix, incoming.last_surveyed_unix);
}

fn merge_poi_record(
    current: &mut prayer_state::PoiKnowledge,
    incoming: &prayer_state::PoiKnowledge,
) {
    if current.id.is_empty() {
        *current = incoming.clone();
        return;
    }
    if current.id != incoming.id || current.system_id != incoming.system_id {
        warn!(current_id = %current.id, incoming_id = %incoming.id, current_system = %current.system_id, incoming_system = %incoming.system_id, "rejected conflicting POI observation identity");
        return;
    }
    let newer = incoming.last_observed_unix >= current.last_observed_unix;
    if newer
        && (incoming.info != current.info
            || (incoming.info_complete && !current.info_complete)
            || ((incoming.resources_complete || !incoming.resources.is_empty())
                && (incoming.resources != current.resources
                    || (incoming.resources_complete && !current.resources_complete))))
    {
        if incoming.info_complete {
            current.info = incoming.info.clone();
            current.info_complete = true;
        } else {
            merge_partial_poi_info(&mut current.info, &incoming.info);
        }
        if incoming.resources_complete {
            current.resources = incoming.resources.clone();
            current.resources_complete = true;
        } else {
            for resource in &incoming.resources {
                if let Some(existing) = current
                    .resources
                    .iter_mut()
                    .find(|existing| existing.resource_id == resource.resource_id)
                {
                    merge_partial_resource(existing, resource);
                } else {
                    current.resources.push(resource.clone());
                }
            }
        }
        current.last_observed_unix =
            latest(current.last_observed_unix, incoming.last_observed_unix);
    }
    current.first_discovered_unix = earliest(
        current.first_discovered_unix,
        incoming.first_discovered_unix,
    );
    current.first_visited_unix = earliest(current.first_visited_unix, incoming.first_visited_unix);
    current.last_visited_unix = latest(current.last_visited_unix, incoming.last_visited_unix);
}

fn merge_partial_poi_info(
    current: &mut prayer_state::PoiInfoData,
    incoming: &prayer_state::PoiInfoData,
) {
    if !incoming.id.is_empty() {
        current.id = incoming.id.clone();
    }
    if !incoming.system_id.is_empty() {
        current.system_id = incoming.system_id.clone();
    }
    if !incoming.name.is_empty() {
        current.name = incoming.name.clone();
    }
    if !incoming.poi_type.is_empty() {
        current.poi_type = incoming.poi_type.clone();
    }
    if !incoming.class_name.is_empty() {
        current.class_name = incoming.class_name.clone();
    }
    if !incoming.description.is_empty() {
        current.description = incoming.description.clone();
    }
    if incoming.x.is_some() {
        current.x = incoming.x;
    }
    if incoming.y.is_some() {
        current.y = incoming.y;
    }
    if incoming.base_id.is_some() {
        current.base_id = incoming.base_id.clone();
    }
    if incoming.base_name.is_some() {
        current.base_name = incoming.base_name.clone();
    }
    if incoming.online.is_some() {
        current.online = incoming.online;
    }
    if incoming.fuel_reserve.is_some() {
        current.fuel_reserve = incoming.fuel_reserve;
    }
    if incoming.fuel_capacity.is_some() {
        current.fuel_capacity = incoming.fuel_capacity;
    }
    if incoming.fuel_price.is_some() {
        current.fuel_price = incoming.fuel_price;
    }
    if incoming.faction_fuel_reserve.is_some() {
        current.faction_fuel_reserve = incoming.faction_fuel_reserve;
    }
    if incoming.faction_fuel_capacity.is_some() {
        current.faction_fuel_capacity = incoming.faction_fuel_capacity;
    }
    // True is informative in a partial observation; false/absence is not.
    current.hidden |= incoming.hidden;
    current.has_base |= incoming.has_base;
}

fn merge_partial_resource(
    current: &mut prayer_state::PoiResourceData,
    incoming: &prayer_state::PoiResourceData,
) {
    if !incoming.name.is_empty() {
        current.name = incoming.name.clone();
    }
    if !incoming.richness_text.is_empty() {
        current.richness_text = incoming.richness_text.clone();
    }
    if incoming.richness.is_some() {
        current.richness = incoming.richness;
    }
    if incoming.remaining.is_some() {
        current.remaining = incoming.remaining;
    }
    if !incoming.remaining_display.is_empty() {
        current.remaining_display = incoming.remaining_display.clone();
    }
}

fn earliest(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn latest(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod canonical_merge_tests {
    use super::*;
    use prayer_runtime::snapshot::WorldObservation;

    fn observation(
        system: prayer_state::SystemKnowledge,
        poi: prayer_state::PoiKnowledge,
    ) -> StateObservation {
        StateObservation {
            world: WorldObservation {
                galaxy: Arc::new(GalaxyData {
                    system_records: HashMap::from([(system.id.clone(), system)]),
                    poi_records: HashMap::from([(poi.id.clone(), poi)]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn stale_and_missing_fields_cannot_overwrite_newer_canonical_facts() {
        let mut knowledge = WorldState::default();
        let fresh = observation(
            prayer_state::SystemKnowledge {
                id: "sol".into(),
                name: Some("Sol".into()),
                empire: Some("solarian".into()),
                observed_at_unix: 200,
                ..Default::default()
            },
            prayer_state::PoiKnowledge {
                id: "earth".into(),
                system_id: "sol".into(),
                info: prayer_state::PoiInfoData {
                    id: "earth".into(),
                    system_id: "sol".into(),
                    name: "Earth".into(),
                    ..Default::default()
                },
                last_observed_unix: Some(200),
                ..Default::default()
            },
        );
        assert!(merge_knowledge_state_if_changed(&mut knowledge, &fresh));
        let stale = observation(
            prayer_state::SystemKnowledge {
                id: "sol".into(),
                name: Some("Old Sol".into()),
                observed_at_unix: 100,
                ..Default::default()
            },
            prayer_state::PoiKnowledge {
                id: "earth".into(),
                system_id: "sol".into(),
                info: prayer_state::PoiInfoData {
                    id: "earth".into(),
                    system_id: "sol".into(),
                    name: "Old Earth".into(),
                    ..Default::default()
                },
                last_observed_unix: Some(100),
                ..Default::default()
            },
        );
        assert!(!merge_knowledge_state_if_changed(&mut knowledge, &stale));
        assert_eq!(
            knowledge.galaxy.system_records["sol"].name.as_deref(),
            Some("Sol")
        );
        assert_eq!(
            knowledge.galaxy.system_records["sol"].empire.as_deref(),
            Some("solarian")
        );
        assert_eq!(knowledge.galaxy.poi_records["earth"].info.name, "Earth");
    }

    #[test]
    fn replaying_identical_observation_does_not_bump_version() {
        let mut knowledge = WorldState::default();
        let value = observation(
            prayer_state::SystemKnowledge {
                id: "sol".into(),
                name: Some("Sol".into()),
                observed_at_unix: 200,
                ..Default::default()
            },
            prayer_state::PoiKnowledge {
                id: "earth".into(),
                system_id: "sol".into(),
                last_observed_unix: Some(200),
                ..Default::default()
            },
        );
        assert!(merge_knowledge_state_if_changed(&mut knowledge, &value));
        let version = knowledge.knowledge_version;
        assert!(!merge_knowledge_state_if_changed(&mut knowledge, &value));
        assert_eq!(knowledge.knowledge_version, version);
    }

    #[test]
    fn newer_identity_only_poi_preserves_complete_metadata() {
        let mut current = prayer_state::PoiKnowledge {
            id: "station".into(),
            system_id: "sol".into(),
            info_complete: true,
            info: prayer_state::PoiInfoData {
                id: "station".into(),
                system_id: "sol".into(),
                name: "Sol Station".into(),
                poi_type: "station".into(),
                has_base: true,
                base_id: Some("base".into()),
                ..Default::default()
            },
            last_observed_unix: Some(10),
            ..Default::default()
        };
        let partial = prayer_state::PoiKnowledge {
            id: "station".into(),
            system_id: "sol".into(),
            info: prayer_state::PoiInfoData {
                id: "station".into(),
                system_id: "sol".into(),
                ..Default::default()
            },
            last_observed_unix: Some(20),
            ..Default::default()
        };
        merge_poi_record(&mut current, &partial);
        assert_eq!(current.info.name, "Sol Station");
        assert_eq!(current.info.poi_type, "station");
        assert!(current.info.has_base);
        assert_eq!(current.info.base_id.as_deref(), Some("base"));
    }

    #[test]
    fn newer_partial_system_preserves_observed_stronghold() {
        let mut current = prayer_state::SystemKnowledge {
            id: "sol".into(),
            is_stronghold: true,
            stronghold_observed: true,
            observed_at_unix: 10,
            ..Default::default()
        };
        let partial = prayer_state::SystemKnowledge {
            id: "sol".into(),
            observed_at_unix: 20,
            ..Default::default()
        };
        merge_system_record(&mut current, &partial);
        assert!(current.is_stronghold);
    }

    #[test]
    fn newer_partial_resource_preserves_unobserved_details() {
        let mut current = prayer_state::PoiKnowledge {
            id: "belt".into(),
            system_id: "sol".into(),
            resources: vec![prayer_state::PoiResourceData {
                resource_id: "iron".into(),
                name: "Iron".into(),
                richness_text: "Rich".into(),
                richness: Some(8),
                remaining: Some(100),
                remaining_display: "100".into(),
            }],
            last_observed_unix: Some(10),
            ..Default::default()
        };
        let partial = prayer_state::PoiKnowledge {
            id: "belt".into(),
            system_id: "sol".into(),
            resources: vec![prayer_state::PoiResourceData {
                resource_id: "iron".into(),
                remaining: Some(90),
                ..Default::default()
            }],
            last_observed_unix: Some(20),
            ..Default::default()
        };
        merge_poi_record(&mut current, &partial);
        let resource = &current.resources[0];
        assert_eq!(resource.name, "Iron");
        assert_eq!(resource.richness, Some(8));
        assert_eq!(resource.remaining, Some(90));
    }

    #[test]
    fn authoritative_empty_survey_clears_stale_values() {
        let mut current = prayer_state::SystemKnowledge {
            id: "sol".into(),
            bloom_status: Some("active".into()),
            bloom_intensity: Some(2.0),
            faint_signatures: vec![serde_json::json!({"kind":"ore"})],
            wildlife: vec![serde_json::json!({"kind":"bird"})],
            survey_complete: true,
            observed_at_unix: 10,
            ..Default::default()
        };
        let empty = prayer_state::SystemKnowledge {
            id: "sol".into(),
            survey_complete: true,
            observed_at_unix: 20,
            ..Default::default()
        };
        merge_system_record(&mut current, &empty);
        assert!(current.bloom_status.is_none());
        assert!(current.bloom_intensity.is_none());
        assert!(current.faint_signatures.is_empty());
        assert!(current.wildlife.is_empty());
    }
}

fn map_vec_unique_changes(
    current: &HashMap<String, Vec<String>>,
    incoming: &HashMap<String, Vec<String>>,
    excluded_values: &[&str],
) -> bool {
    incoming.iter().any(|(key, values)| {
        values.iter().any(|value| {
            !excluded_values.iter().any(|excluded| value == excluded)
                && !current
                    .get(key)
                    .is_some_and(|known| known.iter().any(|item| item == value))
        })
    })
}

pub fn canonical_system_id_for_knowledge(knowledge: &WorldState, system_id: &str) -> String {
    let system_id = system_id.trim();
    if system_id == MOBILE_BASE_POI_ID {
        return knowledge
            .galaxy
            .poi_records
            .get(MOBILE_BASE_POI_ID)
            .map(|poi| poi.system_id.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(system_id)
            .to_string();
    }
    system_id.to_string()
}

/// Fold one fresh sighting into the accumulated map. Identity fields and the
/// first-seen stamp survive from the previous record; everything observable
/// (faction, ship, status, location, flags) takes the latest value. An
/// otherwise-unchanged sighting is re-stamped at most every
/// AGENT_SIGHTING_RESTAMP_SECS so a bot idling next to another player doesn't
/// rewrite the knowledge cache on every refresh.
pub use prayer_runtime::knowledge::merge_agent_sighting;
pub fn merge_knowledge_state_if_changed_with_metadata(
    knowledge: &mut WorldState,
    metadata: &mut prayer_runtime::knowledge::WorldRuntimeMetadata,
    observation: &StateObservation,
) -> bool {
    let changed = merge_knowledge_state_with_metadata(knowledge, metadata, observation);
    if changed {
        knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
    }
    changed
}

#[cfg(test)]
pub fn merge_knowledge_state(knowledge: &mut WorldState, observation: &StateObservation) -> bool {
    merge_knowledge_state_with_metadata(knowledge, &mut Default::default(), observation)
}

#[cfg(test)]
pub fn merge_knowledge_state_if_changed(
    knowledge: &mut WorldState,
    observation: &StateObservation,
) -> bool {
    merge_knowledge_state_if_changed_with_metadata(knowledge, &mut Default::default(), observation)
}

pub use prayer_runtime::knowledge::salvage_snapshot_fresh;
#[cfg(test)]
pub use prayer_runtime::knowledge::world_knowledge_persisted_eq;
