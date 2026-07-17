//! Snapshot observation contracts.

use std::collections::HashMap;

use std::sync::Arc;

use crate::engine::{
    AgentSightingData, CatalogData, FactionGarageInfo, GalaxyData, MarketData, SalvageData,
};
use crate::state::{BotState, PassengerState, WildlifePoiSnapshotData};

/// How long a POI wreck/container scan stays fresh before re-fetching.
/// This is tracked by the caller's shared knowledge store so sessions parked
/// at the same POI can reuse a recent salvage scan.
pub const WRECKS_REFRESH_TTL: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Clone, Default)]
pub struct BotObservation {
    pub state: BotState,
}

/// Shared facts observed by a single fetch. These are deltas, not a complete
/// replacement for canonical world state.
#[derive(Debug, Clone, Default)]
pub struct WorldObservation {
    pub catalog: Arc<CatalogData>,
    pub galaxy: Arc<GalaxyData>,
    pub market: Arc<MarketData>,
    pub storage: Arc<HashMap<String, HashMap<String, i64>>>,
    pub faction_storage: Arc<HashMap<String, i64>>,
    pub faction_garage: FactionGarageInfo,
    pub passengers: PassengerState,
    pub salvage: Arc<SalvageData>,
}

/// One observed snapshot, explicitly separated into bot and world deltas.
#[derive(Debug, Clone, Default)]
pub struct StateObservation {
    pub bot: BotObservation,
    pub world: WorldObservation,
    /// Position reported for this observation before enrichment (which may
    /// rewrite `live.system` mid-transit).
    pub status_system: Option<String>,
    /// See [`Self::status_system`].
    pub status_poi: Option<String>,
    /// Whether system details were refreshed in this observation.
    pub system_fetched: bool,
    /// Whether POI details were refreshed in this observation.
    pub poi_fetched: bool,
    /// Whether map knowledge was refreshed in this observation.
    pub map_fetched: bool,
    /// Full catalog snapshot, present only when the catalog was (re)fetched.
    pub catalog: Option<CatalogData>,
    /// Other players seen in this observation, present only when refreshed.
    pub agents: Option<AgentsObservation>,
    /// Whether other-agent presence was refreshed in this observation.
    pub agents_fetched: bool,
    /// Wildlife seen in this observation, present only when refreshed.
    pub wildlife: Option<WildlifeObservation>,
    /// Whether nearby wildlife was refreshed in this observation.
    pub nearby_fetched: bool,
    /// Whether salvageable wreck/container data was refreshed in this observation.
    pub wrecks_fetched: bool,
    /// Whether active missions were refreshed in this observation.
    pub missions_fetched: bool,
    /// Whether owned ships were refreshed in this observation.
    pub ships_fetched: bool,
    /// Whether commission status was refreshed in this observation.
    pub commission_status_fetched: bool,
    /// Whether the mission board was refreshed for the current docked station.
    pub docked_missions_fetched: bool,
    /// Whether personal storage was refreshed for the current docked station.
    pub docked_storage_fetched: bool,
    /// Whether faction storage was refreshed at the current docked station.
    pub docked_faction_storage_fetched: bool,
    /// Whether the crafting queue was refreshed at the current docked station.
    pub docked_crafting_queue_fetched: bool,
    /// Whether personal passengers were refreshed in this observation.
    pub passengers_fetched: bool,
    /// Whether the passenger board was refreshed at the current docked station.
    pub docked_passengers_fetched: bool,
}

/// One system-agent observation: who was in the system and when.
#[derive(Debug, Clone, Default)]
pub struct AgentsObservation {
    /// System the agents were observed in.
    pub system_id: String,
    /// Unix seconds when the observation was made.
    pub observed_at_unix: i64,
    /// Agents present, with sighting stamps already applied.
    pub agents: Vec<AgentSightingData>,
}

/// One wildlife observation visible at the current POI.
#[derive(Debug, Clone, Default)]
pub struct WildlifeObservation {
    pub snapshot: WildlifePoiSnapshotData,
}
