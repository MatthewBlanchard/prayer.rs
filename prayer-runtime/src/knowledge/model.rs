//! Compatibility re-exports for canonical shared world snapshots.

pub(crate) use prayer_state::WorldState;

use std::collections::HashMap;
use std::time::Instant;

/// Process-local refresh and invalidation clocks kept outside portable state.
#[derive(Debug, Clone, Default)]
pub struct WorldRuntimeMetadata {
    pub storage_fetched_at_by_key: HashMap<String, Instant>,
    pub map_fetched_at: Option<Instant>,
    pub agents_fetched_at_by_system: HashMap<String, Instant>,
    pub nearby_fetched_at_by_poi: HashMap<String, Instant>,
    pub wrecks_fetched_at_by_poi: HashMap<String, Instant>,
    pub faction_storage_fetched_at_by_key: HashMap<String, Instant>,
    pub faction_garage_fetched_at_by_key: HashMap<String, Instant>,
    pub faction_treasury_fetched_at_by_key: HashMap<String, Instant>,
    pub station_passengers_fetched_at_by_station: HashMap<String, Instant>,
    pub chat_fetched_at_by_session: HashMap<String, Instant>,
    pub faction_fetched_at_by_session: HashMap<String, Instant>,
}
