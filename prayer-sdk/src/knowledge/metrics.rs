use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct WorldKnowledgeCounts {
    pub galaxy_systems: usize,
    pub galaxy_pois: usize,
    pub galaxy_items: usize,
    pub galaxy_ships: usize,
    pub galaxy_recipes: usize,
    pub galaxy_facilities: usize,
    pub galaxy_skills: usize,
    pub galaxy_system_connections: usize,
    pub galaxy_poi_resources: usize,
    pub known_station_markets: usize,
    pub station_market_sell_item_keys: usize,
    pub station_market_sell_orders: usize,
    pub station_market_buy_item_keys: usize,
    pub station_market_buy_orders: usize,
    pub station_passenger_boards: usize,
    pub station_passenger_waiting: usize,
    pub station_passenger_aboard: usize,
    pub salvage_pois: usize,
    pub salvage_lootables: usize,
    pub storage_players: usize,
    pub storage_poi_buckets: usize,
    pub storage_item_stacks: usize,
    pub faction_storage_factions: usize,
    pub faction_storage_poi_buckets: usize,
    pub faction_storage_item_stacks: usize,
    pub faction_garages: usize,
    pub faction_garage_ships: usize,
    pub virtual_orders: usize,
    pub virtual_craft_orders: usize,
    pub facilities_pois: usize,
    pub agent_sightings: usize,
    pub system_agent_systems: usize,
    pub system_agent_sightings: usize,
    pub wildlife_pois: usize,
    pub wildlife_creatures: usize,
}

impl WorldKnowledgeCounts {
    pub fn from_knowledge(knowledge: &WorldState) -> Self {
        let station_market_sell_item_keys = knowledge
            .station_markets
            .values()
            .map(|market| market.sell_orders.len())
            .sum();
        let station_market_sell_orders = knowledge
            .station_markets
            .values()
            .flat_map(|market| market.sell_orders.values())
            .map(Vec::len)
            .sum();
        let station_market_buy_item_keys = knowledge
            .station_markets
            .values()
            .map(|market| market.buy_orders.len())
            .sum();
        let station_market_buy_orders = knowledge
            .station_markets
            .values()
            .flat_map(|market| market.buy_orders.values())
            .map(Vec::len)
            .sum();
        let station_passenger_waiting = knowledge
            .station_passengers
            .values()
            .map(|board| board.waiting.len())
            .sum();
        let station_passenger_aboard = knowledge
            .station_passengers
            .values()
            .map(|board| board.aboard.len())
            .sum();
        let salvage_lootables = knowledge
            .salvage_by_poi
            .values()
            .map(|salvage| {
                salvage.visible_lootables.len()
                    + salvage
                        .lootables_by_poi
                        .values()
                        .map(Vec::len)
                        .sum::<usize>()
            })
            .sum();
        let storage_poi_buckets = knowledge.storage_by_player.values().map(HashMap::len).sum();
        let storage_item_stacks = knowledge
            .storage_by_player
            .values()
            .flat_map(HashMap::values)
            .map(HashMap::len)
            .sum();
        let faction_storage_poi_buckets = knowledge
            .faction_storage_by_faction_poi
            .values()
            .map(HashMap::len)
            .sum();
        let faction_storage_item_stacks = knowledge
            .faction_storage_by_faction_poi
            .values()
            .flat_map(HashMap::values)
            .map(HashMap::len)
            .sum();
        let faction_garage_ships = knowledge
            .faction_garage_by_faction
            .values()
            .map(|garage| garage.ships.len())
            .sum();
        let system_agent_sightings = knowledge
            .system_agents_by_system
            .values()
            .map(Vec::len)
            .sum();
        let wildlife_creatures = knowledge
            .wildlife_by_poi
            .values()
            .map(|snapshot| snapshot.creatures.len())
            .sum();

        Self {
            galaxy_systems: knowledge.galaxy.system_records.len(),
            galaxy_pois: knowledge.galaxy.poi_records.len(),
            galaxy_items: knowledge.catalog.items.len(),
            galaxy_ships: knowledge.catalog.ships.len(),
            galaxy_recipes: knowledge.catalog.recipes.len(),
            galaxy_facilities: knowledge.catalog.facilities.len(),
            galaxy_skills: knowledge.catalog.skills.len(),
            galaxy_system_connections: knowledge
                .galaxy
                .system_records
                .values()
                .filter(|system| !system.connections.is_empty())
                .count(),
            galaxy_poi_resources: knowledge
                .galaxy
                .poi_records
                .values()
                .filter(|poi| !poi.resources.is_empty())
                .count(),
            known_station_markets: knowledge.station_markets.len(),
            station_market_sell_item_keys,
            station_market_sell_orders,
            station_market_buy_item_keys,
            station_market_buy_orders,
            station_passenger_boards: knowledge.station_passengers.len(),
            station_passenger_waiting,
            station_passenger_aboard,
            salvage_pois: knowledge.salvage_by_poi.len(),
            salvage_lootables,
            storage_players: knowledge.storage_by_player.len(),
            storage_poi_buckets,
            storage_item_stacks,
            faction_storage_factions: knowledge.faction_storage_by_faction_poi.len(),
            faction_storage_poi_buckets,
            faction_storage_item_stacks,
            faction_garages: knowledge.faction_garage_by_faction.len(),
            faction_garage_ships,
            virtual_orders: knowledge.virtual_orders.len(),
            virtual_craft_orders: knowledge.virtual_craft_orders.len(),
            facilities_pois: knowledge.facilities_by_poi.len(),
            agent_sightings: knowledge.agent_sightings.len(),
            system_agent_systems: knowledge.system_agents_by_system.len(),
            system_agent_sightings,
            wildlife_pois: knowledge.wildlife_by_poi.len(),
            wildlife_creatures,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorldKnowledgeByteBreakdown {
    pub catalog_bytes: usize,
    pub galaxy_bytes: usize,
    pub shipyard_listing_ids_bytes: usize,
    pub station_markets_bytes: usize,
    pub station_passengers_bytes: usize,
    pub salvage_by_poi_bytes: usize,
    pub storage_by_player_bytes: usize,
    pub faction_storage_by_faction_poi_bytes: usize,
    pub faction_garage_by_faction_bytes: usize,
    pub virtual_orders_bytes: usize,
    pub virtual_craft_orders_bytes: usize,
    pub facilities_by_poi_bytes: usize,
    pub agent_sightings_bytes: usize,
    pub system_agents_by_system_bytes: usize,
    pub wildlife_by_poi_bytes: usize,
    pub managed_players_bytes: usize,
}

impl WorldKnowledgeByteBreakdown {
    pub fn from_knowledge(knowledge: &WorldState) -> Self {
        Self {
            catalog_bytes: serialized_len(&knowledge.catalog),
            galaxy_bytes: serialized_len(&knowledge.galaxy),
            shipyard_listing_ids_bytes: serialized_len(&knowledge.shipyard_listing_ids),
            station_markets_bytes: serialized_len(&knowledge.station_markets),
            station_passengers_bytes: serialized_len(&knowledge.station_passengers),
            salvage_by_poi_bytes: serialized_len(&knowledge.salvage_by_poi),
            storage_by_player_bytes: serialized_len(&knowledge.storage_by_player),
            faction_storage_by_faction_poi_bytes: serialized_len(
                &knowledge.faction_storage_by_faction_poi,
            ),
            faction_garage_by_faction_bytes: serialized_len(&knowledge.faction_garage_by_faction),
            virtual_orders_bytes: serialized_len(&knowledge.virtual_orders),
            virtual_craft_orders_bytes: serialized_len(&knowledge.virtual_craft_orders),
            facilities_by_poi_bytes: serialized_len(&knowledge.facilities_by_poi),
            agent_sightings_bytes: serialized_len(&knowledge.agent_sightings),
            system_agents_by_system_bytes: serialized_len(&knowledge.system_agents_by_system),
            wildlife_by_poi_bytes: serialized_len(&knowledge.wildlife_by_poi),
            managed_players_bytes: serialized_len(&knowledge.managed_players),
        }
    }
}

pub fn serialized_len<T: serde::Serialize + ?Sized>(value: &T) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_and_discovered_world_payloads_are_measured_independently() {
        let mut knowledge = WorldState::default();
        Arc::make_mut(&mut knowledge.catalog).version = Some("0.377.0".to_string());
        Arc::make_mut(&mut knowledge.galaxy).system_records = (0..100)
            .map(|index| {
                let id = format!("system_{index:03}");
                (
                    id.clone(),
                    prayer_state::SystemKnowledge {
                        id,
                        name: Some(format!("System {index}")),
                        ..Default::default()
                    },
                )
            })
            .collect();

        let bytes = WorldKnowledgeByteBreakdown::from_knowledge(&knowledge);
        println!(
            "catalog_bytes={} discovered_world_bytes={}",
            bytes.catalog_bytes, bytes.galaxy_bytes
        );
        assert!(bytes.catalog_bytes > 0);
        assert!(bytes.galaxy_bytes > bytes.catalog_bytes);

        let catalog_json = serde_json::to_value(&knowledge.catalog).expect("catalog JSON");
        let galaxy_json = serde_json::to_value(&knowledge.galaxy).expect("galaxy JSON");
        assert!(catalog_json.get("items").is_some());
        assert!(galaxy_json.get("items").is_none());
        assert!(catalog_json.get("systems").is_none());
        assert!(galaxy_json.get("systems").is_none());
        assert!(galaxy_json.get("system_records").is_some());
    }
}
