//! Shared runtime game-state models.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Canonical catalog entry payload from SpaceMolt.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntryData {
    /// Catalog entry id.
    pub id: String,
    /// Full entry payload as returned by SpaceMolt.
    pub raw: Value,
}

/// Snapshot of known galaxy entities used for analyzer identity resolution.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GalaxyData {
    /// Known system ids.
    pub systems: Vec<String>,
    /// Known poi ids.
    pub pois: Vec<String>,
    /// Item ids from catalog/cache.
    pub item_ids: Vec<String>,
    /// Ship ids from catalog/cache.
    pub ship_ids: Vec<String>,
    /// Recipe ids from catalog/cache.
    pub recipe_ids: Vec<String>,
    /// Item catalog entries keyed by id.
    pub item_catalog_entries: HashMap<String, CatalogEntryData>,
    /// Ship catalog entries keyed by id.
    pub ship_catalog_entries: HashMap<String, CatalogEntryData>,
    /// Recipe catalog entries keyed by id.
    pub recipe_catalog_entries: HashMap<String, CatalogEntryData>,
    /// Last seen SpaceMolt catalog version.
    pub catalog_version: Option<String>,
    /// System jump graph adjacency list.
    pub system_connections: HashMap<String, Vec<String>>,
    /// System id -> map coordinates `(x, y)` when known.
    pub system_coordinates: HashMap<String, (f64, f64)>,
    /// POI id -> system id.
    pub poi_system: HashMap<String, String>,
    /// Base id -> POI id lookup.
    pub poi_base_to_id: HashMap<String, String>,
    /// POI id -> POI type (e.g. `station`, `asteroid_field`).
    pub poi_type_by_id: HashMap<String, String>,
    /// Resource id -> known POI ids.
    pub pois_by_resource: HashMap<String, Vec<String>>,
    /// Explored system ids.
    pub explored_systems: HashSet<String>,
    /// Visited POI ids.
    pub visited_pois: HashSet<String>,
    /// Surveyed system ids.
    pub surveyed_systems: HashSet<String>,
    /// Dockable POI ids by system id.
    pub dockable_pois_by_system: HashMap<String, Vec<String>>,
    /// Station POI ids by system id.
    pub station_pois_by_system: HashMap<String, Vec<String>>,
}

/// Lightweight market order entry from station market snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOrderInfo {
    /// Price per item (integer credits).
    pub price_each: i64,
    /// Quantity available.
    pub quantity: i64,
}

/// Snapshot of market entities used for analyzer identity resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketData {
    /// Known shipyard listing ids.
    pub shipyard_listings: Vec<String>,
    /// Buy orders by item id.
    pub buy_orders: HashMap<String, Vec<MarketOrderInfo>>,
    /// Sell orders by item id.
    pub sell_orders: HashMap<String, Vec<MarketOrderInfo>>,
}

/// Snapshot of mission entities used for analyzer identity resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionData {
    /// Active mission ids.
    pub active: Vec<String>,
    /// Available mission ids.
    pub available: Vec<String>,
}

/// Open market order metadata from runtime snapshot.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenOrderInfo {
    /// Order id.
    pub order_id: String,
    /// Item id.
    pub item_id: String,
    /// Price per item.
    pub price_each: f64,
    /// Quantity.
    pub quantity: i64,
}

impl GalaxyData {
    /// Compute the shortest path from `start` to `target` using A*.
    /// Returns hop sequence excluding `start`, including `target`.
    pub fn astar_shortest_path_hops(&self, start: &str, target: &str) -> Option<Vec<String>> {
        crate::graph::astar_shortest_path_hops(
            &self.system_connections,
            &self.system_coordinates,
            start,
            target,
        )
    }

    /// Return the first hop from `start` toward `target` if reachable.
    pub fn next_hop_toward(&self, start: &str, target: &str) -> Option<String> {
        crate::graph::next_hop_toward(
            &self.system_connections,
            &self.system_coordinates,
            start,
            target,
        )
    }

    /// Return hop-count distance between `start` and `target` if reachable.
    pub fn hop_distance(&self, start: &str, target: &str) -> Option<usize> {
        crate::graph::hop_distance(
            &self.system_connections,
            &self.system_coordinates,
            start,
            target,
        )
    }
}

/// Active ship stats that come from the game API but aren't needed by engine predicates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipState {
    /// Ship display name.
    pub name: String,
    /// Ship class id.
    pub class_id: String,
    /// Current armor.
    pub armor: i64,
    /// Speed.
    pub speed: i64,
    /// Current hull.
    pub hull: i64,
    /// Max hull.
    pub max_hull: i64,
    /// Current shield.
    pub shield: i64,
    /// Max shield.
    pub max_shield: i64,
    /// CPU used.
    pub cpu_used: i64,
    /// CPU capacity.
    pub cpu_capacity: i64,
    /// Power used.
    pub power_used: i64,
    /// Power capacity.
    pub power_capacity: i64,
}

/// Game-state data used by predicates and macro resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameState {
    /// Current system id.
    pub system: Option<String>,
    /// Home base id.
    pub home_base: Option<String>,
    /// Nearest station id.
    pub nearest_station: Option<String>,
    /// Current POI id.
    pub current_poi: Option<String>,
    /// Whether the ship is currently docked.
    pub docked: bool,
    /// Credit count.
    pub credits: i64,
    /// Fuel percent.
    pub fuel_pct: i64,
    /// Cargo percent.
    pub cargo_pct: i64,
    /// Cargo units used.
    pub cargo_used: i64,
    /// Cargo capacity units.
    pub cargo_capacity: i64,
    /// Cargo quantities by item.
    pub cargo: Arc<HashMap<String, i64>>,
    /// Storage quantities by poi then item.
    pub stash: Arc<HashMap<String, HashMap<String, i64>>>,
    /// Mission completion map.
    pub mission_complete: Arc<HashMap<String, bool>>,
    /// Known galaxy entities.
    pub galaxy: Arc<GalaxyData>,
    /// Known market entities.
    pub market: Arc<MarketData>,
    /// Mission ids by status.
    pub missions: Arc<MissionData>,
    /// Owned ship ids.
    pub owned_ships: Arc<Vec<String>>,
    /// Installed module ids on active ship.
    pub installed_modules: Arc<Vec<String>>,
    /// Open buy orders owned by player.
    pub own_buy_orders: Arc<Vec<OpenOrderInfo>>,
    /// Open sell orders owned by player.
    pub own_sell_orders: Arc<Vec<OpenOrderInfo>>,
    /// Last mined deltas by item for counter accumulation.
    pub last_mined: Arc<HashMap<String, i64>>,
    /// Last stashed deltas by item for counter accumulation.
    pub last_stashed: Arc<HashMap<String, i64>>,
    /// Script/session mined totals by item.
    pub script_mined_by_item: Arc<HashMap<String, i64>>,
    /// Script/session stashed totals by item.
    pub script_stashed_by_item: Arc<HashMap<String, i64>>,
    /// Active ship stats.
    pub ship: ShipState,
}
