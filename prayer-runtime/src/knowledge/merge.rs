use std::collections::HashMap;

use crate::engine::{AgentSightingData, StationMarketData};
use crate::snapshot::WRECKS_REFRESH_TTL;
use crate::WildlifePoiSnapshotData;

use super::{WorldRuntimeMetadata, WorldState};

const AGENT_SIGHTING_RESTAMP_SECS: i64 = 300;

pub fn merge_agent_sighting(
    sightings: &mut HashMap<String, AgentSightingData>,
    fresh: &AgentSightingData,
) {
    let key = fresh.sighting_key().to_string();
    match sightings.get_mut(&key) {
        Some(existing) => {
            let mut updated = fresh.clone();
            updated.first_seen_unix = existing.first_seen_unix.min(fresh.first_seen_unix);
            updated.times_seen = existing.times_seen.saturating_add(1);
            if updated.contact.player_id.is_none() {
                updated.contact.player_id = existing.contact.player_id.clone();
            }
            if updated.contact.username.is_none() {
                updated.contact.username = existing.contact.username.clone();
            }
            let observable_changed = {
                let mut probe = updated.clone();
                probe.first_seen_unix = existing.first_seen_unix;
                probe.last_seen_unix = existing.last_seen_unix;
                probe.times_seen = existing.times_seen;
                probe != *existing
            };
            let stale = fresh.last_seen_unix.saturating_sub(existing.last_seen_unix)
                >= AGENT_SIGHTING_RESTAMP_SECS;
            if observable_changed || stale {
                *existing = updated;
            }
        }
        None => {
            sightings.insert(key, fresh.clone());
        }
    }
}

pub fn world_knowledge_runtime_eq<V, C>(a: &WorldState<V, C>, b: &WorldState<V, C>) -> bool {
    world_knowledge_persisted_eq(a, b)
        && station_market_snapshots_eq(&a.station_markets, &b.station_markets)
        && a.chat_messages_by_session == b.chat_messages_by_session
        && a.faction_by_session == b.faction_by_session
}

pub fn station_market_snapshots_eq(
    a: &HashMap<String, StationMarketData>,
    b: &HashMap<String, StationMarketData>,
) -> bool {
    a.len() == b.len()
        && a.iter().all(|(station_id, left)| {
            b.get(station_id)
                .is_some_and(|right| station_market_snapshot_eq(left, right))
        })
}

pub fn station_market_snapshot_eq(left: &StationMarketData, right: &StationMarketData) -> bool {
    left.buy_orders == right.buy_orders
        && left.sell_orders == right.sell_orders
        && left.current_tick == right.current_tick
}

pub fn world_knowledge_persisted_eq<V, C>(a: &WorldState<V, C>, b: &WorldState<V, C>) -> bool {
    a.galaxy == b.galaxy
        && a.shipyard_listing_ids == b.shipyard_listing_ids
        && a.storage_by_player == b.storage_by_player
        && a.faction_storage_by_faction_poi == b.faction_storage_by_faction_poi
        && a.facilities_by_poi == b.facilities_by_poi
        && a.owned_facilities_by_player == b.owned_facilities_by_player
        && a.owned_facilities_by_faction == b.owned_facilities_by_faction
        && a.agent_sightings == b.agent_sightings
        && system_agents_snapshots_eq(&a.system_agents_by_system, &b.system_agents_by_system)
        && wildlife_snapshots_eq(&a.wildlife_by_poi, &b.wildlife_by_poi)
}

pub fn system_agents_snapshots_eq(
    a: &HashMap<String, Vec<AgentSightingData>>,
    b: &HashMap<String, Vec<AgentSightingData>>,
) -> bool {
    a.len() == b.len()
        && a.iter().all(|(system, agents)| {
            b.get(system).is_some_and(|other| {
                agents.len() == other.len()
                    && agents
                        .iter()
                        .zip(other)
                        .all(|(left, right)| agent_current_snapshot_eq(left, right))
            })
        })
}

pub fn agent_current_snapshot_eq(a: &AgentSightingData, b: &AgentSightingData) -> bool {
    a.contact.player_id == b.contact.player_id
        && a.contact.username == b.contact.username
        && a.contact.faction_id == b.contact.faction_id
        && a.contact.faction_tag == b.contact.faction_tag
        && a.contact.clan_tag == b.contact.clan_tag
        && a.contact.ship_class == b.contact.ship_class
        && a.contact.ship_name == b.contact.ship_name
        && a.contact.status_message == b.contact.status_message
        && a.contact.primary_color == b.contact.primary_color
        && a.contact.secondary_color == b.contact.secondary_color
        && a.contact.in_combat == b.contact.in_combat
        && a.contact.offline == b.contact.offline
        && a.last_seen_system == b.last_seen_system
}

pub fn wildlife_snapshots_eq(
    a: &HashMap<String, WildlifePoiSnapshotData>,
    b: &HashMap<String, WildlifePoiSnapshotData>,
) -> bool {
    a.len() == b.len()
        && a.iter().all(|(poi_id, snapshot)| {
            b.get(poi_id)
                .is_some_and(|other| wildlife_snapshot_eq(snapshot, other))
        })
}

pub fn wildlife_snapshot_eq(
    left: &WildlifePoiSnapshotData,
    right: &WildlifePoiSnapshotData,
) -> bool {
    left.system_id == right.system_id
        && left.poi_id == right.poi_id
        && left.creature_count == right.creature_count
        && left.creatures.len() == right.creatures.len()
        && left.creatures.iter().zip(&right.creatures).all(|(a, b)| {
            a.creature == b.creature && a.system_id == b.system_id && a.poi_id == b.poi_id
        })
}

pub fn salvage_snapshot_fresh(metadata: &WorldRuntimeMetadata, poi_id: &str) -> bool {
    metadata
        .wrecks_fetched_at_by_poi
        .get(poi_id)
        .is_some_and(|fetched_at| fetched_at.elapsed() < WRECKS_REFRESH_TTL)
}
