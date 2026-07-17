//! Shared runtime game-state models.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use serde::{de, Deserialize, Deserializer, Serialize};

use crate::graph::RouteTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CraftJobStatus {
    Optimistic,
    Active,
    Completed,
    Cancelled,
    Failed,
    Lost,
}

/// Canonical cached chat record independent of the upstream transport schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChatMessageData {
    pub id: String,
    pub channel: String,
    pub sender_id: String,
    pub sender: String,
    pub content: String,
    pub timestamp_utc: String,
    pub system_id: Option<String>,
    pub poi_id: Option<String>,
    pub faction_id: Option<String>,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    pub empire_official: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FactionMemberData {
    pub player_id: String,
    pub username: String,
    pub role: String,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FactionRoleData {
    pub name: String,
    pub priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FactionSnapshotData {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub leader_id: String,
    pub leader_username: String,
    pub member_count: i64,
    pub treasury: Option<i64>,
    pub is_member: bool,
    pub description: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub members: Vec<FactionMemberData>,
    pub roles: Vec<FactionRoleData>,
}

/// Lazily built all-pairs routing table for one `GalaxyData` instance.
///
/// Purely derived from canonical system records, so it is
/// excluded from serialization and equality. Immutable galaxy snapshots share
/// the table; mutation paths must replace the cache before changing either
/// routing input.
#[derive(Default)]
pub struct RouteCache(Arc<OnceLock<RouteTable>>);

impl Clone for RouteCache {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl PartialEq for RouteCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for RouteCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.get().is_some() {
            "RouteCache(built)"
        } else {
            "RouteCache(empty)"
        })
    }
}

/// Last observed resource state at a point of interest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoiResourceData {
    /// Resource id.
    pub resource_id: String,
    /// Display name returned by SpaceMolt.
    pub name: String,
    /// Human-readable richness label, when provided.
    pub richness_text: String,
    /// Numeric richness score, when provided.
    pub richness: Option<i64>,
    /// Remaining amount, when provided.
    pub remaining: Option<i64>,
    /// Human-readable remaining amount, when provided.
    pub remaining_display: String,
}

/// Last observed point-of-interest metadata from `get_system` / `get_poi`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PoiInfoData {
    /// POI id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// System id.
    pub system_id: String,
    /// POI type.
    pub poi_type: String,
    /// Type-specific class.
    pub class_name: String,
    /// Description.
    pub description: String,
    /// Hidden flag.
    pub hidden: bool,
    /// X coordinate within the system.
    pub x: Option<f64>,
    /// Y coordinate within the system.
    pub y: Option<f64>,
    /// Whether this POI has a base.
    pub has_base: bool,
    /// Base id.
    pub base_id: Option<String>,
    /// Base name.
    pub base_name: Option<String>,
    /// Online player count.
    pub online: Option<i64>,
    /// Public fuel reserve.
    pub fuel_reserve: Option<i64>,
    /// Public fuel capacity.
    pub fuel_capacity: Option<i64>,
    /// Current refuel price.
    pub fuel_price: Option<i64>,
    /// Faction private fuel reserve.
    pub faction_fuel_reserve: Option<i64>,
    /// Faction private fuel capacity.
    pub faction_fuel_capacity: Option<i64>,
}

/// Immutable SpaceMolt definitions, separated from discovered world facts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogData {
    /// Last observed SpaceMolt catalog version.
    pub version: Option<String>,
    /// Item definitions keyed by id.
    pub items: HashMap<String, spacemolt_lib_rs::schema::CatalogDumpItemsItem>,
    /// Ship definitions keyed by id.
    pub ships: HashMap<String, spacemolt_lib_rs::schema::ShipClass>,
    /// Recipe definitions keyed by id.
    pub recipes: HashMap<String, spacemolt_lib_rs::schema::Recipe>,
    /// Facility definitions keyed by id.
    pub facilities: HashMap<String, spacemolt_lib_rs::schema::FacilityDefinition>,
    /// Skill definitions keyed by id.
    pub skills: HashMap<String, spacemolt_lib_rs::schema::SkillDefinition>,
}

impl CatalogData {
    pub fn item_ids(&self) -> impl Iterator<Item = &str> {
        self.items.keys().map(String::as_str)
    }

    pub fn ship_ids(&self) -> impl Iterator<Item = &str> {
        self.ships.keys().map(String::as_str)
    }
}

/// Snapshot of discovered galaxy entities used for navigation and analysis.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemKnowledge {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<(f64, f64)>,
    #[serde(default)]
    pub connections: Vec<String>,
    #[serde(default)]
    pub connections_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empire: Option<String>,
    #[serde(default)]
    pub is_stronghold: bool,
    /// Whether the observation explicitly supplied `is_stronghold`.
    #[serde(default)]
    pub stronghold_observed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poi_count: Option<usize>,
    #[serde(default)]
    pub pois_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_entered_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_entered_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scanned_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_surveyed_unix: Option<i64>,
    #[serde(default)]
    pub faint_signatures: Vec<serde_json::Value>,
    #[serde(default)]
    pub wildlife: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloom_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloom_intensity: Option<f64>,
    /// Whether survey-derived fields were observed, including empty/absent values.
    #[serde(default)]
    pub survey_complete: bool,
    #[serde(default)]
    pub observed_at_unix: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PoiKnowledge {
    pub id: String,
    pub system_id: String,
    #[serde(default)]
    pub info: PoiInfoData,
    /// Whether `info` is an authoritative snapshot rather than a partial row.
    #[serde(default)]
    pub info_complete: bool,
    #[serde(default)]
    pub resources: Vec<PoiResourceData>,
    #[serde(default)]
    pub resources_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_discovered_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_observed_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_visited_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_visited_unix: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GalaxyData {
    /// Canonical durable system facts keyed by stable SpaceMolt id.
    #[serde(default)]
    pub system_records: HashMap<String, SystemKnowledge>,
    /// Canonical durable POI facts keyed by stable SpaceMolt id.
    #[serde(default)]
    pub poi_records: HashMap<String, PoiKnowledge>,
    /// Last observed facility instances keyed by station/POI id.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub facilities_by_poi: HashMap<String, PoiFacilitiesSnapshot>,
    /// Cached all-pairs routing table derived from canonical system records.
    #[serde(skip)]
    pub routes: RouteCache,
}

impl GalaxyData {
    pub fn poi_id_for_base(&self, base_id: &str) -> Option<&str> {
        self.poi_records
            .values()
            .find(|poi| poi.info.base_id.as_deref() == Some(base_id))
            .map(|poi| poi.id.as_str())
    }

    pub fn base_id_for_poi(&self, poi_id: &str) -> Option<&str> {
        self.poi_records.get(poi_id)?.info.base_id.as_deref()
    }

    pub fn is_station_poi(&self, poi_id: &str) -> bool {
        self.poi_records
            .get(poi_id)
            .is_some_and(|poi| poi.info.poi_type.eq_ignore_ascii_case("station"))
    }
}

/// Cached facility instances observed at one POI.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PoiFacilitiesSnapshot {
    /// Unix timestamp of the upstream observation.
    #[serde(default)]
    pub observed_at_unix: i64,
    /// Personal, station, and public facilities returned for this POI.
    #[serde(default)]
    pub current: Option<spacemolt_lib_rs::schema::FacilityResponse>,
    /// Faction facilities returned for this POI.
    #[serde(default)]
    pub faction_current: Option<spacemolt_lib_rs::schema::FacilityResponse>,
}

/// Accumulation metadata around one canonical `get_system_agents` contact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AgentSightingData {
    /// Canonical contact facts returned by `get_system_agents`.
    pub contact: spacemolt_lib_rs::schema::NearbyPlayer,
    /// System the player was last seen in.
    pub last_seen_system: String,
    /// Unix seconds of the first recorded sighting.
    pub first_seen_unix: i64,
    /// Unix seconds of the most recent sighting.
    pub last_seen_unix: i64,
    /// Number of recorded sightings (distinct merge events, not raw polls).
    pub times_seen: i64,
}

impl AgentSightingData {
    /// Stable identity for accumulation: player id when known, otherwise
    /// username.
    pub fn sighting_key(&self) -> &str {
        self.contact
            .player_id
            .as_deref()
            .filter(|value| !value.is_empty())
            .or(self.contact.username.as_deref())
            .unwrap_or_default()
    }
}

impl Default for AgentSightingData {
    fn default() -> Self {
        Self {
            contact: spacemolt_lib_rs::schema::NearbyPlayer {
                clan_tag: None,
                docked: None,
                faction_id: None,
                faction_tag: None,
                in_combat: Some(false),
                offline: None,
                player_id: None,
                primary_color: None,
                secondary_color: None,
                ship_class: None,
                ship_name: None,
                status_message: None,
                username: None,
            },
            last_seen_system: String::new(),
            first_seen_unix: 0,
            last_seen_unix: 0,
            times_seen: 0,
        }
    }
}

/// One visible wildlife creature from `get_nearby.creatures`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WildlifeCreatureData {
    /// Canonical current creature facts from `get_nearby`.
    pub creature: spacemolt_lib_rs::schema::CreatureInfo,
    pub system_id: String,
    pub poi_id: String,
    pub observed_at_unix: i64,
}

impl Default for WildlifeCreatureData {
    fn default() -> Self {
        Self {
            creature: spacemolt_lib_rs::schema::CreatureInfo {
                creature_id: String::new(),
                hull: 0,
                in_combat: false,
                max_hull: 0,
                name: String::new(),
                role: String::new(),
                species: String::new(),
            },
            system_id: String::new(),
            poi_id: String::new(),
            observed_at_unix: 0,
        }
    }
}

/// Last observed wildlife snapshot for one POI.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WildlifePoiSnapshotData {
    pub system_id: String,
    pub poi_id: String,
    pub creature_count: i64,
    pub observed_at_unix: i64,
    pub creatures: Vec<WildlifeCreatureData>,
}

/// Canonical generated market depth row.
pub type MarketOrder = spacemolt_lib_rs::data::MarketOrder;

/// Last observed market order books for a single station.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StationMarketData {
    /// Buy orders by item id.
    pub buy_orders: HashMap<String, Vec<MarketOrder>>,
    /// Sell orders by item id.
    pub sell_orders: HashMap<String, Vec<MarketOrder>>,
    /// Unix seconds when this snapshot was observed; `None` on data
    /// recorded before timestamps existed.
    #[serde(default)]
    pub observed_at_unix: Option<i64>,
    /// SpaceMolt market cursor from the last successful `view_market`
    /// response for this station.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_tick: Option<i64>,
}

/// Snapshot of market entities used for analyzer identity resolution.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MarketData {
    /// Known shipyard listing ids.
    pub shipyard_listings: Vec<String>,
    /// Buy orders by item id at the current station.
    pub buy_orders: HashMap<String, Vec<MarketOrder>>,
    /// Sell orders by item id at the current station.
    pub sell_orders: HashMap<String, Vec<MarketOrder>>,
    /// Last observed market snapshot per station POI.
    #[serde(default)]
    pub station_markets: HashMap<String, StationMarketData>,
}

/// One visible lootable object at a POI: wreck, pirate wreck, jettison
/// container, or any future floating inventory shape.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SpaceLootInfo {
    /// Lootable id.
    pub id: String,
    /// API object type.
    pub kind: String,
    /// POI where the lootable is anchored.
    pub poi_id: String,
    /// System containing the POI.
    pub system_id: String,
    /// Visible cargo stacks.
    pub cargo: Vec<spacemolt_lib_rs::data::WreckCargoItem>,
    /// Visible modules.
    pub modules: Vec<spacemolt_lib_rs::data::WreckModule>,
    /// Salvage value, when provided.
    pub salvage_value: Option<i64>,
    /// Creation timestamp, when provided.
    pub created_at: Option<String>,
    /// Expiry timestamp, when provided.
    pub expires_at: Option<String>,
    /// Expiry tick, when provided.
    pub expire_tick: Option<i64>,
    /// Ship class, for wreck-like objects.
    pub ship_class: Option<String>,
    /// Ship name, for wreck-like objects.
    pub ship_name: Option<String>,
    /// Victim/player display fields, when provided.
    pub victim_name: Option<String>,
    /// Killer display fields, when provided.
    pub killer_name: Option<String>,
}

/// Current and remembered salvage knowledge.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SalvageData {
    /// Lootables visible at the current/last observed POI.
    pub visible_lootables: Vec<SpaceLootInfo>,
    /// Last observed lootables by POI id.
    #[serde(default)]
    pub lootables_by_poi: HashMap<String, Vec<SpaceLootInfo>>,
    /// POI covered by `visible_lootables`.
    pub last_seen_poi: Option<String>,
    /// System covered by `visible_lootables`.
    pub last_seen_system: Option<String>,
    /// Unix seconds when this salvage snapshot was observed.
    pub observed_at_unix: Option<i64>,
}

/// Cross-station price aggregates derived from the rebuilt station market layer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlobalPriceAggregates {
    /// Median across stations of each station's best (highest) buy price.
    pub median_buy_prices: HashMap<String, f64>,
    /// Median across stations of each station's best (lowest) sell price.
    pub median_sell_prices: HashMap<String, f64>,
    /// Depth-weighted midpoint of live ask prices.
    pub weighted_mid_prices: HashMap<String, f64>,
}

impl MarketData {
    /// Aggregate the remembered per-station snapshots into global per-item
    /// prices. Uses each station's best-of-book (not the full depth) so
    /// deep-book junk orders can't skew the buy/sell medians. The global
    /// replacement price uses the midpoint of live ask depth: each unit listed
    /// for sale counts as one entry at its order price. Buy-only books are not
    /// treated as finite replacement prices.
    pub fn global_price_aggregates(&self) -> GlobalPriceAggregates {
        let mut best_bids: HashMap<&str, Vec<f64>> = HashMap::new();
        let mut best_asks: HashMap<&str, Vec<f64>> = HashMap::new();
        let mut ask_depth_prices: HashMap<&str, Vec<(f64, i64)>> = HashMap::new();

        // Sorted iteration keeps float accumulation deterministic.
        let mut stations: Vec<_> = self.station_markets.iter().collect();
        stations.sort_by(|a, b| a.0.cmp(b.0));

        for (_, snapshot) in stations {
            let mut item_ids: Vec<&str> = snapshot
                .buy_orders
                .keys()
                .chain(snapshot.sell_orders.keys())
                .map(String::as_str)
                .collect();
            item_ids.sort_unstable();
            item_ids.dedup();

            for item_id in item_ids {
                let bids = live_orders(&snapshot.buy_orders, item_id);
                let asks = live_orders(&snapshot.sell_orders, item_id);
                let best_bid = bids.iter().map(|o| o.price_each).max();
                let best_ask = asks.iter().map(|o| o.price_each).min();
                if let Some(bid) = best_bid {
                    best_bids.entry(item_id).or_default().push(bid as f64);
                }
                if let Some(ask) = best_ask {
                    best_asks.entry(item_id).or_default().push(ask as f64);
                }

                for ask in asks {
                    if ask.price_each > 0 {
                        ask_depth_prices
                            .entry(item_id)
                            .or_default()
                            .push((ask.price_each as f64, ask.quantity));
                    }
                }
            }
        }

        GlobalPriceAggregates {
            median_buy_prices: medians_by_item(best_bids),
            median_sell_prices: medians_by_item(best_asks),
            weighted_mid_prices: weighted_medians_by_item(ask_depth_prices),
        }
    }
}

/// Orders for `item_id` that still have stock; filled/stale entries carry no
/// price information.
fn live_orders<'a>(
    orders: &'a HashMap<String, Vec<MarketOrder>>,
    item_id: &str,
) -> Vec<&'a MarketOrder> {
    orders
        .get(item_id)
        .map(|entries| entries.iter().filter(|o| o.quantity > 0).collect())
        .unwrap_or_default()
}

fn medians_by_item(prices: HashMap<&str, Vec<f64>>) -> HashMap<String, f64> {
    prices
        .into_iter()
        .map(|(item_id, mut values)| (item_id.to_string(), median(&mut values)))
        .collect()
}

fn weighted_medians_by_item(prices: HashMap<&str, Vec<(f64, i64)>>) -> HashMap<String, f64> {
    prices
        .into_iter()
        .filter_map(|(item_id, mut values)| {
            weighted_median(&mut values).map(|median| (item_id.to_string(), median))
        })
        .collect()
}

/// Median of a non-empty list; the even case averages the middle pair.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn weighted_median(values: &mut [(f64, i64)]) -> Option<f64> {
    values.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let total_weight = values
        .iter()
        .filter(|(_, weight)| *weight > 0)
        .map(|(_, weight)| *weight)
        .sum::<i64>();
    if total_weight <= 0 {
        return None;
    }

    let lower_rank = (total_weight + 1) / 2;
    let upper_rank = (total_weight + 2) / 2;
    let lower = weighted_rank_value(values, lower_rank)?;
    let upper = weighted_rank_value(values, upper_rank)?;
    Some((lower + upper) / 2.0)
}

fn weighted_rank_value(values: &[(f64, i64)], rank: i64) -> Option<f64> {
    let mut cumulative = 0_i64;
    for (value, weight) in values {
        if *weight <= 0 {
            continue;
        }
        cumulative = cumulative.saturating_add(*weight);
        if cumulative >= rank {
            return Some(*value);
        }
    }
    None
}

/// Snapshot of mission entities used for analyzer identity resolution.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MissionData {
    /// Active mission ids.
    pub active: Vec<String>,
    /// Available mission ids.
    pub available: Vec<String>,
    /// Active mission details keyed in-list order.
    pub active_details: Vec<spacemolt_lib_rs::schema::V2GameStateMissionsActiveItem>,
    /// Available mission details keyed in-list order.
    pub available_details: Vec<spacemolt_lib_rs::schema::MissionInfo>,
}

impl GalaxyData {
    fn route_table(&self) -> &RouteTable {
        self.routes.0.get_or_init(|| {
            let connections = self
                .system_records
                .iter()
                .map(|(id, system)| (id.clone(), system.connections.clone()))
                .collect::<HashMap<_, _>>();
            let strongholds = self
                .system_records
                .iter()
                .filter_map(|(id, system)| system.is_stronghold.then_some(id.clone()))
                .collect::<HashSet<_>>();
            let penalized_systems = RouteTable::systems_within_hops(&connections, &strongholds, 3);
            RouteTable::build_with_penalties(&connections, &penalized_systems)
        })
    }

    /// Build the all-pairs routing table now instead of on first query.
    pub fn precompute_routes(&self) {
        self.route_table();
    }

    /// Drop the derived route table before mutating routing inputs.
    pub fn invalidate_routes(&mut self) {
        self.routes = RouteCache::default();
    }

    /// Compute the shortest path from `start` to `target`.
    /// Returns hop sequence excluding `start`, including `target`.
    pub fn shortest_path_hops(&self, start: &str, target: &str) -> Option<Vec<String>> {
        self.route_table().path_hops(start, target)
    }

    /// Compute a cached route, optionally applying stronghold safety penalties.
    pub fn route_hops(&self, start: &str, target: &str, safe: bool) -> Option<Vec<String>> {
        if safe {
            self.route_table().path_hops(start, target)
        } else {
            self.route_table().naked_path_hops(start, target)
        }
    }

    /// Return the first hop from `start` toward `target` if reachable.
    pub fn next_hop_toward(&self, start: &str, target: &str) -> Option<String> {
        self.route_table().next_hop_toward(start, target)
    }

    /// Return hop-count distance between `start` and `target` if reachable.
    pub fn hop_distance(&self, start: &str, target: &str) -> Option<usize> {
        self.route_table().hop_distance(start, target)
    }

    /// Return weighted route cost between `start` and `target` if reachable.
    ///
    /// Systems within three naked jumps of a known stronghold cost two jumps
    /// to enter, so this is the distance bots should use for route scoring.
    pub fn path_cost(&self, start: &str, target: &str) -> Option<usize> {
        self.route_table().path_cost(start, target)
    }
}

/// Generated active-ship facts from the v2 game-state response.
pub type ShipState = spacemolt_lib_rs::schema::V2GameStateShip;

/// Canonical active ship commission returned by `commission_status`.
pub type ActiveCommissionInfo = spacemolt_lib_rs::schema::CommissionEntry;

/// One ship parked in the player's faction garage and claimable via switch_ship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactionGarageShipObservation {
    /// Garage station/base id, when provided.
    pub base_id: String,
    /// Garage station/base name, when provided.
    pub base_name: String,
    /// Garage station system name, when provided.
    pub system_name: String,
    /// Canonical SpaceMolt garage ship value.
    pub ship: spacemolt_lib_rs::schema::GaragedShipEntry,
}

/// Faction ship garage capacity and contents from `list_ships`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FactionGarageInfo {
    /// Occupied garage slots, when provided.
    pub used: Option<i64>,
    /// Total garage slots, when provided.
    pub capacity: Option<i64>,
    /// Ships parked in the faction garage.
    #[serde(default, skip_serializing_if = "arc_vec_is_empty")]
    pub ships: Arc<Vec<FactionGarageShipObservation>>,
}

/// Derived current/maximum occupancy view parsed from SpaceMolt's berth text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct PassengerBerthView {
    /// Occupied berths.
    pub current: i64,
    /// Total berths.
    pub max: i64,
}

impl std::str::FromStr for PassengerBerthView {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(parse_passenger_berth_count(value))
    }
}

impl<'de> Deserialize<'de> for PassengerBerthView {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = PassengerBerthView;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a berth count object or current/max display string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(parse_passenger_berth_count(value))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(PassengerBerthView::default())
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(PassengerBerthView::default())
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut current = None;
                let mut max = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "current" => current = Some(map.next_value::<i64>()?),
                        "max" => max = Some(map.next_value::<i64>()?),
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(PassengerBerthView {
                    current: current.unwrap_or_default(),
                    max: max.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

fn parse_passenger_berth_count(value: &str) -> PassengerBerthView {
    let value = value.trim();
    if let Some(total) = value
        .strip_suffix("total")
        .map(str::trim)
        .and_then(|total| total.parse::<i64>().ok())
    {
        return PassengerBerthView {
            current: 0,
            max: total.max(0),
        };
    }
    let (current, max) = value
        .split_once('/')
        .map(|(current, max)| (current.trim(), max.trim()))
        .unwrap_or(("", value));
    PassengerBerthView {
        current: current.parse::<i64>().unwrap_or_default().max(0),
        max: max.parse::<i64>().unwrap_or_default().max(0),
    }
}

/// Passenger berth capacity and current/waiting passenger snapshots.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PassengerState {
    /// Count of passengers currently aboard, when observed.
    pub aboard_count: Option<i64>,
    /// Economy berth occupancy from `list_passengers`.
    pub economy_berths: PassengerBerthView,
    /// Unchanged SpaceMolt berth field.
    pub economy_berths_raw: String,
    /// Business berth occupancy from `list_passengers`.
    pub business_berths: PassengerBerthView,
    /// Unchanged SpaceMolt berth field.
    pub business_berths_raw: String,
    /// First-class berth occupancy from `list_passengers`.
    pub first_berths: PassengerBerthView,
    /// Unchanged SpaceMolt berth field.
    pub first_berths_raw: String,
    /// Passengers currently aboard.
    #[serde(default, skip_serializing_if = "arc_vec_is_empty")]
    pub aboard: Arc<Vec<spacemolt_lib_rs::schema::PassengerView>>,
    /// Station id for the waiting-passenger board.
    pub station: String,
    /// Count of waiting passengers at `station`, when observed.
    pub waiting_count: Option<i64>,
    /// Passengers waiting at `station`.
    #[serde(default, skip_serializing_if = "arc_vec_is_empty")]
    pub waiting: Arc<Vec<spacemolt_lib_rs::schema::WaitingPassengerView>>,
}

/// Stable, transport-neutral identity for one fleet bot.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct BotId(String);

impl BotId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BotId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for BotId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for BotId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Connection status attached to a cached bot snapshot.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub enum BotConnectionState {
    Connected,
    #[default]
    Disconnected,
}

/// Actor-owned passenger facts. Station passenger boards are world state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ActorPassengerState {
    pub aboard_count: Option<i64>,
    pub economy_berths: PassengerBerthView,
    pub economy_berths_raw: String,
    pub business_berths: PassengerBerthView,
    pub business_berths_raw: String,
    pub first_berths: PassengerBerthView,
    pub first_berths_raw: String,
    pub aboard: Arc<Vec<spacemolt_lib_rs::schema::PassengerView>>,
}

impl From<&PassengerState> for ActorPassengerState {
    fn from(passengers: &PassengerState) -> Self {
        Self {
            aboard_count: passengers.aboard_count,
            economy_berths: passengers.economy_berths,
            economy_berths_raw: passengers.economy_berths_raw.clone(),
            business_berths: passengers.business_berths,
            business_berths_raw: passengers.business_berths_raw.clone(),
            first_berths: passengers.first_berths,
            first_berths_raw: passengers.first_berths_raw.clone(),
            aboard: Arc::clone(&passengers.aboard),
        }
    }
}

impl ActorPassengerState {
    pub fn to_passenger_state(&self) -> PassengerState {
        PassengerState {
            aboard_count: self.aboard_count,
            economy_berths: self.economy_berths,
            economy_berths_raw: self.economy_berths_raw.clone(),
            business_berths: self.business_berths,
            business_berths_raw: self.business_berths_raw.clone(),
            first_berths: self.first_berths,
            first_berths_raw: self.first_berths_raw.clone(),
            aboard: Arc::clone(&self.aboard),
            ..PassengerState::default()
        }
    }
}

/// Canonical facts unique to one SpaceMolt player.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CraftingQueueProjection {
    pub job_id: Option<String>,
    pub reservation_id: Option<String>,
    pub raw_text: Option<String>,
    pub source: Option<String>,
    pub item_id: Option<String>,
    pub quantity: Option<i64>,
    pub recipe_id: Option<String>,
    pub crafts: Option<i64>,
    pub order_id: Option<String>,
    pub station_id: Option<String>,
    pub status: Option<CraftJobStatus>,
    pub facility_id: Option<String>,
    pub preset: Option<String>,
}

/// Canonical facts unique to one SpaceMolt player.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BotState {
    pub fuel_pct: i64,
    pub fuel: i64,
    pub max_fuel: i64,
    pub cargo_pct: i64,
    pub cargo_used: i64,
    pub cargo_capacity: i64,
    /// Normalized quantity index derived from `cargo_items` for planners.
    pub cargo: Arc<HashMap<String, i64>>,
    /// Complete generated cargo facts from the v2 game-state response.
    pub cargo_items: Arc<Vec<spacemolt_lib_rs::schema::V2GameStateCargoItem>>,
    pub mission_complete: Arc<HashMap<String, bool>>,
    pub missions: Arc<MissionData>,
    pub owned_ship_details: Arc<Vec<spacemolt_lib_rs::schema::OwnedShipInfo>>,
    pub active_commissions: Arc<Vec<ActiveCommissionInfo>>,
    pub installed_modules: Arc<Vec<String>>,
    pub own_buy_orders: Arc<Vec<spacemolt_lib_rs::schema::ExchangeOrder>>,
    pub own_sell_orders: Arc<Vec<spacemolt_lib_rs::schema::ExchangeOrder>>,
    /// Prayer compatibility projection for optimistic craft enqueue state.
    pub crafting_queue: Arc<Vec<CraftingQueueProjection>>,
    pub last_mined: Arc<HashMap<String, i64>>,
    pub last_stored: Arc<HashMap<String, i64>>,
    pub script_mined_by_item: Arc<HashMap<String, i64>>,
    pub script_stored_by_item: Arc<HashMap<String, i64>>,
    /// Complete generated player facts from the v2 game-state response.
    pub player: spacemolt_lib_rs::schema::V2GameStatePlayer,
    pub ship: ShipState,
    /// Complete generated location facts from the v2 game-state response.
    pub location: spacemolt_lib_rs::schema::V2GameStateLocation,
    /// Typed observation-subscription contacts layered over status location.
    pub observation_nearby: Arc<HashMap<String, spacemolt_lib_rs::state::ObservedPlayer>>,
    pub in_battle: bool,
    pub combat_stance: Option<String>,
    pub combat_target: Option<String>,
    pub skills: Arc<HashMap<String, spacemolt_lib_rs::schema::V2GameStateSkillsValue>>,
    pub modules: Arc<Vec<spacemolt_lib_rs::schema::V2GameStateModulesItem>>,
    pub passengers: ActorPassengerState,
}

impl BotState {
    pub fn owned_ship_ids(&self) -> impl Iterator<Item = &str> {
        self.owned_ship_details
            .iter()
            .map(|ship| ship.ship_id.as_str())
    }

    pub fn owns_ship(&self, ship_id: &str) -> bool {
        self.owned_ship_ids().any(|owned| owned == ship_id)
    }

    pub fn effective_system_id(&self) -> Option<&str> {
        self.location
            .system_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn effective_poi_id(&self) -> Option<&str> {
        self.location
            .poi_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| self.effective_system_id())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FleetEntry {
    pub id: BotId,
    /// Stable display username, falling back to Prayer's durable bot label
    /// when the latest observed player payload is incomplete.
    #[serde(default)]
    pub username: Option<String>,
    pub state: Arc<BotState>,
    pub version: u64,
    pub observed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub connection: BotConnectionState,
    /// Canonical script lifecycle projected by the owning runtime service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<FleetScriptExecution>")]
    pub script_execution: Option<serde_json::Value>,
    /// Canonical active navigation route projected by the owning runtime service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<FleetActiveRoute>")]
    pub active_route: Option<serde_json::Value>,
    #[serde(default)]
    pub in_transit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transit_dest_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transit_dest_poi: Option<String>,
}

/// Public wire shape for the canonical script projection stored in a fleet entry.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetScriptExecution {
    pub id: String,
    pub run_id: Option<prayer_actions::RunId>,
    pub script: Option<String>,
    pub frame_kind: Option<String>,
    pub frame_name: Option<String>,
    pub state: String,
    pub current_line: Option<usize>,
    pub last_line: Option<usize>,
    pub outcome: Option<FleetScriptOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FleetScriptOutcome {
    Success { message: Option<String> },
    Error { kind: String, message: String },
}

/// Public wire shape for the active route projection stored in a fleet entry.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetActiveRoute {
    pub target: String,
    pub target_system: String,
    pub target_poi: Option<String>,
    pub hops: Vec<String>,
    pub total_jumps: usize,
    pub estimated_fuel_use: i64,
}

#[derive(Debug, Clone, Default)]
pub struct FleetState {
    pub bots: HashMap<BotId, Arc<BotState>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FleetSnapshot {
    pub bots: HashMap<BotId, FleetEntry>,
}

fn arc_vec_is_empty<T>(value: &Arc<Vec<T>>) -> bool {
    value.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_poi_falls_back_to_system() {
        let state = BotState {
            location: spacemolt_lib_rs::schema::V2GameStateLocation {
                system_id: Some("sol".to_string()),
                ..Default::default()
            },
            ..BotState::default()
        };

        assert_eq!(state.effective_poi_id(), Some("sol"));
    }

    #[test]
    fn passenger_berth_count_accepts_total_display_string() {
        let count: PassengerBerthView =
            serde_json::from_value(serde_json::json!("3 total")).expect("passenger berth count");

        assert_eq!(count, PassengerBerthView { current: 0, max: 3 });
    }

    fn orders(entries: &[(i64, i64)]) -> Vec<MarketOrder> {
        entries
            .iter()
            .map(|&(price_each, quantity)| MarketOrder {
                price_each,
                quantity,
                source: None,
                my_quantity: None,
            })
            .collect()
    }

    fn station(buys: &[(i64, i64)], sells: &[(i64, i64)]) -> StationMarketData {
        let by_item = |entries: &[(i64, i64)]| {
            if entries.is_empty() {
                HashMap::new()
            } else {
                HashMap::from([("iron".to_string(), orders(entries))])
            }
        };
        StationMarketData {
            buy_orders: by_item(buys),
            sell_orders: by_item(sells),
            ..StationMarketData::default()
        }
    }

    fn market_with(stations: Vec<(&str, StationMarketData)>) -> MarketData {
        MarketData {
            station_markets: stations
                .into_iter()
                .map(|(id, snapshot)| (id.to_string(), snapshot))
                .collect(),
            ..MarketData::default()
        }
    }

    #[test]
    fn aggregates_take_best_of_book_then_median_across_stations() {
        // Station a's lowball 1-credit bid loses to its 10-credit best bid;
        // the median over best bids [10, 12, 40] ignores outlier station c.
        let market = market_with(vec![
            ("a", station(&[(10, 5), (1, 999)], &[(14, 5), (200, 1)])),
            ("b", station(&[(12, 5)], &[(13, 5)])),
            ("c", station(&[(40, 5)], &[(41, 5)])),
        ]);

        let aggregates = market.global_price_aggregates();

        assert_eq!(aggregates.median_buy_prices["iron"], 12.0);
        assert_eq!(aggregates.median_sell_prices["iron"], 14.0);
    }

    #[test]
    fn median_averages_middle_pair_for_even_station_count() {
        let market = market_with(vec![
            ("a", station(&[(10, 5)], &[])),
            ("b", station(&[(13, 5)], &[])),
        ]);

        let aggregates = market.global_price_aggregates();

        assert_eq!(aggregates.median_buy_prices["iron"], 11.5);
        assert!(aggregates.median_sell_prices.is_empty());
    }

    #[test]
    fn global_price_uses_ask_depth_midpoint() {
        let market = market_with(vec![("a", station(&[(1, 20)], &[(10, 1), (20, 3)]))]);

        let aggregates = market.global_price_aggregates();

        assert_eq!(aggregates.weighted_mid_prices["iron"], 20.0);
    }

    #[test]
    fn global_price_uses_ask_depth_midpoint_to_resist_thin_outliers() {
        let market = market_with(vec![
            ("a", station(&[(10, 20)], &[(1, 1)])),
            ("b", station(&[(11, 10)], &[(14, 10)])),
            ("c", station(&[(12, 10)], &[(15, 10)])),
        ]);

        let aggregates = market.global_price_aggregates();

        assert_eq!(aggregates.weighted_mid_prices["iron"], 14.0);
    }

    #[test]
    fn global_price_falls_back_to_ask_only_books_without_two_sided_signal() {
        let market = market_with(vec![
            ("a", station(&[], &[(20, 10)])),
            ("b", station(&[], &[(22, 10)])),
            ("c", station(&[], &[(200, 1)])),
        ]);

        let aggregates = market.global_price_aggregates();

        assert_eq!(aggregates.weighted_mid_prices["iron"], 22.0);
    }

    #[test]
    fn global_price_does_not_use_buy_only_books_as_replacement_cost() {
        let market = market_with(vec![
            ("a", station(&[(10, 5)], &[])),
            ("b", station(&[(13, 5)], &[])),
        ]);

        let aggregates = market.global_price_aggregates();

        assert_eq!(aggregates.median_buy_prices["iron"], 11.5);
        assert!(!aggregates.weighted_mid_prices.contains_key("iron"));
    }

    #[test]
    fn aggregates_ignore_orders_without_stock() {
        let market = market_with(vec![("a", station(&[(10, 0)], &[(99, 5), (14, 0)]))]);

        let aggregates = market.global_price_aggregates();

        assert!(aggregates.median_buy_prices.is_empty());
        assert_eq!(aggregates.median_sell_prices["iron"], 99.0);
        assert_eq!(aggregates.weighted_mid_prices["iron"], 99.0);
    }
}
