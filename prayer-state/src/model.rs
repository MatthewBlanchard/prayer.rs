use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::PoiFacilitiesSnapshot;
use crate::{
    AgentSightingData, CatalogData, FactionGarageInfo, GalaxyData, SalvageData, StationMarketData,
};
use crate::{ChatMessageData, FactionSnapshotData, PassengerState, WildlifePoiSnapshotData};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FactionTreasuryInfo {
    #[serde(default)]
    pub faction_name: String,
    #[serde(default)]
    pub treasury: i64,
}

/// Canonical shared state for the fleet.
///
/// Application-coordinated virtual market and craft orders remain generic so
/// the runtime model stays independent of SDK and HTTP types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "VirtualOrder: Serialize, VirtualCraftOrder: Serialize",
    deserialize = "VirtualOrder: Deserialize<'de>, VirtualCraftOrder: Deserialize<'de>"
))]
pub struct WorldState<VirtualOrder, VirtualCraftOrder> {
    #[serde(default)]
    pub knowledge_version: u64,
    #[serde(default)]
    pub catalog: Arc<CatalogData>,
    pub galaxy: Arc<GalaxyData>,
    pub shipyard_listing_ids: Vec<String>,
    #[serde(skip, default)]
    pub station_markets: HashMap<String, StationMarketData>,
    #[serde(skip, default)]
    pub station_passengers: HashMap<String, PassengerState>,
    #[serde(skip, default)]
    pub salvage_by_poi: HashMap<String, SalvageData>,
    #[serde(default)]
    pub storage_by_player: HashMap<String, HashMap<String, HashMap<String, i64>>>,
    #[serde(default)]
    pub faction_storage_by_faction_poi: HashMap<String, HashMap<String, HashMap<String, i64>>>,
    #[serde(default)]
    pub faction_garage_by_faction: HashMap<String, FactionGarageInfo>,
    #[serde(skip, default)]
    pub faction_treasury_by_faction: HashMap<String, FactionTreasuryInfo>,
    #[serde(default)]
    pub virtual_orders: Vec<VirtualOrder>,
    #[serde(default)]
    pub virtual_craft_orders: Vec<VirtualCraftOrder>,
    #[serde(default)]
    pub facilities_by_poi: HashMap<String, PoiFacilitiesSnapshot>,
    /// Last observed personally-owned facility response keyed by player id.
    #[serde(default)]
    pub owned_facilities_by_player: HashMap<String, spacemolt_lib_rs::schema::FacilityResponse>,
    /// Last observed faction-owned facility response keyed by faction id.
    #[serde(default)]
    pub owned_facilities_by_faction: HashMap<String, spacemolt_lib_rs::schema::FacilityResponse>,
    #[serde(default)]
    pub agent_sightings: HashMap<String, AgentSightingData>,
    /// Recent public chat history keyed by managed session id.
    #[serde(skip, default)]
    pub chat_messages_by_session: HashMap<String, Vec<ChatMessageData>>,
    #[serde(skip, default)]
    pub faction_by_session: HashMap<String, FactionSnapshotData>,
    #[serde(default)]
    pub system_agents_by_system: HashMap<String, Vec<AgentSightingData>>,
    #[serde(default)]
    pub wildlife_by_poi: HashMap<String, WildlifePoiSnapshotData>,
    #[serde(skip, default)]
    pub managed_players: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WorldSnapshot<VirtualOrder, VirtualCraftOrder> {
    pub state: Arc<WorldState<VirtualOrder, VirtualCraftOrder>>,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct StateSnapshot<VirtualOrder, VirtualCraftOrder> {
    pub fleet: crate::FleetSnapshot,
    pub world: WorldSnapshot<VirtualOrder, VirtualCraftOrder>,
    pub version: u64,
}

/// Borrowed read lens over one canonical shared-world snapshot.
pub struct WorldLens<'a, VirtualOrder, VirtualCraftOrder> {
    knowledge: &'a WorldState<VirtualOrder, VirtualCraftOrder>,
}

impl<'a, VirtualOrder, VirtualCraftOrder> WorldLens<'a, VirtualOrder, VirtualCraftOrder> {
    pub fn new(knowledge: &'a WorldState<VirtualOrder, VirtualCraftOrder>) -> Self {
        Self { knowledge }
    }

    pub fn galaxy(&self) -> &'a GalaxyData {
        &self.knowledge.galaxy
    }

    pub fn catalog(&self) -> &'a CatalogData {
        &self.knowledge.catalog
    }

    pub fn knowledge(&self) -> &'a WorldState<VirtualOrder, VirtualCraftOrder> {
        self.knowledge
    }
}

impl<VirtualOrder, VirtualCraftOrder> Default for WorldState<VirtualOrder, VirtualCraftOrder> {
    fn default() -> Self {
        Self {
            knowledge_version: 0,
            catalog: Arc::default(),
            galaxy: Arc::default(),
            shipyard_listing_ids: Vec::new(),
            station_markets: HashMap::new(),
            station_passengers: HashMap::new(),
            salvage_by_poi: HashMap::new(),
            storage_by_player: HashMap::new(),
            faction_storage_by_faction_poi: HashMap::new(),
            faction_garage_by_faction: HashMap::new(),
            faction_treasury_by_faction: HashMap::new(),
            virtual_orders: Vec::new(),
            virtual_craft_orders: Vec::new(),
            facilities_by_poi: HashMap::new(),
            owned_facilities_by_player: HashMap::new(),
            owned_facilities_by_faction: HashMap::new(),
            agent_sightings: HashMap::new(),
            chat_messages_by_session: HashMap::new(),
            faction_by_session: HashMap::new(),
            system_agents_by_system: HashMap::new(),
            wildlife_by_poi: HashMap::new(),
            managed_players: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_order_ledgers_preserve_their_persisted_field_names() {
        type Knowledge = WorldState<String, String>;
        let value = serde_json::to_value(Knowledge::default()).expect("serialize knowledge");
        assert_eq!(value.get("virtual_orders"), Some(&serde_json::json!([])));
        assert_eq!(
            value.get("virtual_craft_orders"),
            Some(&serde_json::json!([]))
        );
        assert!(value.get("craft_jobs").is_none());
        assert!(value.get("station_markets").is_none());
        assert!(value.get("map_fetched_at").is_none());
    }
}
