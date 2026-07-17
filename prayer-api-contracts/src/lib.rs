//! Shared HTTP request and response contracts for the Prayer API.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use prayer_runtime::{PersistedExecutionRun, RuntimeEvent};
use serde::{Deserialize, Serialize};

/// Canonical generated SpaceMolt facility response exposed without transport access.
pub type CanonicalFacilityResponse = spacemolt_lib_rs::schema::FacilityResponse;
use serde_json::Value;
use uuid::Uuid;

/// Session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// Session id.
    pub id: Uuid,
}

/// Request body for setting scripts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetScriptRequest {
    /// Raw DSL script.
    pub script: String,
}

/// Response body after setting scripts.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetScriptResponse {
    /// Normalized script.
    pub normalized_script: String,
}

/// Runtime host snapshot DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostSnapshotDto {
    /// Halt flag.
    pub is_halted: bool,
    /// True when the loaded script ran to natural completion.
    pub is_finished: bool,
    /// Active command flag.
    pub has_active_command: bool,
    /// Current script line.
    pub current_script_line: Option<usize>,
    /// Current script.
    pub current_script: Option<String>,
    /// Latest user-visible command result or halt message, if known.
    pub result_message: Option<String>,
    /// Active user-visible script frame.
    pub active_frame: Option<RuntimeActiveFrameDto>,
}

/// Active user-visible script frame.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeActiveFrameDto {
    /// User-facing frame kind, e.g. `main`, `override`, or `skill`.
    pub kind: String,
    /// User-facing frame name for named scopes.
    pub name: Option<String>,
    /// Runtime frame path.
    pub path: String,
    /// Canonical script text for this frame body.
    pub script: String,
    /// One-based active line within `script`.
    pub line: Option<usize>,
}

/// Active script runner ownership.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRunnerDto {
    /// Runner origin, such as startup restore or API execute.
    pub origin: String,
    /// Runner start timestamp.
    pub started_utc: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScriptExecutionDto {
    pub id: Uuid,
    #[serde(default)]
    pub run_id: Option<prayer_actions::RunId>,
    /// Original PrayerLang source submitted for this execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Active execution lane projected for clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_name: Option<String>,
    #[serde(flatten)]
    pub state: ScriptExecutionStateDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ScriptExecutionStateDto {
    Running {
        current_line: Option<usize>,
        last_line: Option<usize>,
        outcome: Option<ScriptOutcomeDto>,
    },
    Stopped {
        current_line: Option<usize>,
        last_line: Option<usize>,
        outcome: ScriptOutcomeDto,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ScriptOutcomeDto {
    Success {
        message: Option<String>,
    },
    Error {
        kind: ScriptErrorKindDto,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScriptErrorKindDto {
    Runtime,
    UserHalt,
    Cancelled,
    Replaced,
    Shutdown,
    RunnerExited,
    Internal,
}

/// Runtime snapshot response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshotResponse {
    /// Session id.
    pub session_id: String,
    /// Authenticated player username, when state has been loaded.
    pub username: Option<String>,
    /// Monotonic session state version.
    pub state_version: u64,
    /// Monotonic shared knowledge version used by this snapshot.
    pub knowledge_version: u64,
    /// Host snapshot.
    pub snapshot: RuntimeHostSnapshotDto,
    /// Authoritative scheduler and producer execution projection.
    pub execution: prayer_runtime::ExecutionSnapshot,
    /// Canonical script lifecycle and terminal outcome.
    pub script_execution: Option<ScriptExecutionDto>,
    /// Latest system.
    pub latest_system: Option<String>,
    /// Latest poi.
    pub latest_poi: Option<String>,
    /// Home base POI id.
    pub home_base: Option<String>,
    /// Home canonical POI id, when distinct from the base id.
    pub home_poi: Option<String>,
    /// Docked flag.
    pub docked: Option<bool>,
    /// Fuel.
    pub fuel: Option<i64>,
    /// Max fuel.
    pub max_fuel: Option<i64>,
    /// Fuel percent.
    pub fuel_percent: Option<i64>,
    /// Estimated fuel required for one inter-system jump.
    pub fuel_per_jump: Option<i64>,
    /// Current hull.
    pub hull: Option<i64>,
    /// Max hull.
    pub max_hull: Option<i64>,
    /// Cargo units used.
    pub cargo_used: Option<i64>,
    /// Cargo capacity.
    pub cargo_capacity: Option<i64>,
    /// Total passenger berths across all classes.
    pub passenger_berths: Option<i64>,
    /// Ship cargo, keyed by item id.
    pub cargo: HashMap<String, i64>,
    /// Credits.
    pub credits: Option<i64>,
    /// Last update timestamp.
    pub last_updated_utc: DateTime<Utc>,
    /// Script running flag.
    pub script_running: bool,
    /// Active script runner ownership, when a service runner is alive.
    pub script_runner: Option<ScriptRunnerDto>,
    /// Active navigation route.
    pub active_route: Option<ActiveGoRouteDto>,
    /// True while actively jumping or traveling.
    pub in_transit: bool,
    /// Destination system id during transit.
    pub transit_dest_system: Option<String>,
    /// Destination POI id during transit.
    pub transit_dest_poi: Option<String>,
    /// True when the latest state says the active ship is in battle.
    pub in_battle: bool,
    /// Current combat stance when known.
    pub combat_stance: Option<String>,
    /// Current focused combat target id/name when known.
    pub combat_target: Option<String>,
}

/// Active route DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGoRouteDto {
    /// Original target token.
    pub target: String,
    /// Resolved target system.
    pub target_system: String,
    /// Resolved target POI, when the route targets a POI/base.
    pub target_poi: Option<String>,
    /// Hops.
    pub hops: Vec<String>,
    /// Total jumps.
    pub total_jumps: i32,
    /// Estimated fuel use.
    pub estimated_fuel_use: i32,
    /// Arrival time.
    pub arrival_time: Option<DateTime<Utc>>,
}

/// Commander/global state projection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommanderStateResponse {
    /// Highest session state version included in this projection.
    pub state_version: u64,
    /// Shared knowledge version used by this projection.
    pub knowledge_version: u64,
    /// Shared world projection, emitted once instead of repeated per session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<CommanderWorldStateResponse>,
    /// Per-session state projections.
    pub sessions: Vec<CommanderSessionStateResponse>,
    /// Shared social knowledge: every other player ever sighted, most
    /// recently seen first.
    pub social: SocialResponse,
}

/// Commander/global world projection shared by every session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommanderWorldStateResponse {
    /// Galaxy state.
    pub galaxy: RuntimeCommanderGalaxyStateDto,
}

/// Commander/global storage projection.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommanderStorageResponse {
    /// Highest session state version included in this projection.
    pub state_version: u64,
    /// Shared knowledge version used by this projection.
    pub knowledge_version: u64,
    /// Number of sessions with state included in this projection.
    pub sessions_observed: usize,
    /// Number of registered sessions considered for this projection.
    pub sessions_total: usize,
    /// Flat storage/cargo lots ready for commander UI display.
    pub rows: Vec<CommanderStorageRowDto>,
}

/// Commander/global owned ship projection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommanderFleetResponse {
    /// Highest session state version included in this projection.
    pub state_version: u64,
    /// Shared knowledge version used by this projection.
    pub knowledge_version: u64,
    /// Number of sessions with state included in this projection.
    pub sessions_observed: usize,
    /// Number of registered sessions considered for this projection.
    pub sessions_total: usize,
    /// Flat owned-ship list ready for commander UI display.
    pub owned_ships: Vec<RuntimeOwnedShipProjectionDto>,
    /// Flat faction-garage ship list ready for commander UI display.
    pub faction_garage_ships: Vec<RuntimeFactionGarageShipProjectionDto>,
}

/// One storage/cargo lot in the commander/global storage projection.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommanderStorageRowDto {
    /// Stable UI key for this lot.
    pub key: String,
    /// Item id.
    pub item_id: String,
    /// Quantity in this lot.
    pub quantity: i64,
    /// Estimated market price per unit, derived from remembered station order books.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_market_price: Option<f64>,
    /// Estimated total market value for this lot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_market_value: Option<f64>,
    /// Median buy price per unit, derived from remembered station order books.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_median_buy_price: Option<f64>,
    /// Median sell price per unit, derived from remembered station order books.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_median_sell_price: Option<f64>,
    /// Estimated total value if sold into median buy-side demand.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_median_buy_value: Option<f64>,
    /// Estimated total value if bought from median sell-side supply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_median_sell_value: Option<f64>,
    /// Market aggregate used to derive `unit_market_price`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_price_source: Option<String>,
    /// Source kind: cargo, personal, faction, or financial.
    pub source_kind: String,
    /// Stable owner id when known.
    pub owner_id: Option<String>,
    /// Human-facing owner name.
    pub owner_name: String,
    /// Stable location id.
    pub location_id: String,
    /// Human-facing location name when known.
    pub location_name: Option<String>,
    /// System id when known.
    pub system_id: Option<String>,
    /// Session handles that observed this lot.
    pub observed_by: Vec<String>,
    /// Highest session state version that contributed to this lot.
    pub state_version: u64,
    /// Optional source-specific detail payload for richer UI rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// One session inside the commander/global state projection.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommanderSessionStateResponse {
    /// Human-facing session handle.
    pub player_name: String,
    /// Monotonic session state version.
    pub state_version: u64,
    /// Shared knowledge version used by this session projection.
    pub knowledge_version: u64,
    /// Runtime state with shared world fields removed, absent until the session
    /// has fetched state at least once.
    pub state: Option<Value>,
    /// Memory strings.
    pub memory: Vec<String>,
    /// Execution status lines.
    pub execution_status_lines: Vec<String>,
}

/// Runtime game-state DTO contract.
#[cfg(test)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGameStateDto {
    /// Current system id.
    pub system: String,
    /// Current POI.
    pub current_poi: RuntimePoiInfoDto,
    /// Known POIs.
    pub pois: Vec<RuntimePoiInfoDto>,
    /// Known systems.
    pub systems: Vec<String>,
    /// Galaxy state.
    pub galaxy: RuntimeGalaxyStateDto,
    /// Remembered station storage this session: POI id -> item id -> quantity.
    pub storage_by_poi: HashMap<String, HashMap<String, i64>>,
    /// Remembered faction storage this session: item id -> quantity.
    pub faction_storage: HashMap<String, i64>,
    /// Economy deals.
    pub economy_deals: Vec<RuntimeEconomyDealDto>,
    /// Own buy orders.
    pub own_buy_orders: Vec<spacemolt_lib_rs::schema::ExchangeOrder>,
    /// Own sell orders.
    pub own_sell_orders: Vec<spacemolt_lib_rs::schema::ExchangeOrder>,
    /// Typed optimistic crafting queue projection for this bot when docked.
    pub crafting_queue: Vec<prayer_state::CraftingQueueProjection>,
    /// Player ship (no cargo — see top-level `cargo`).
    pub ship: RuntimePlayerShipProjectionDto,
    /// Ship cargo, keyed by item id.
    pub cargo: HashMap<String, i64>,
    /// Credits.
    pub credits: i64,
    /// Docked flag.
    pub docked: bool,
    /// Home base id.
    pub home_base: String,
    /// Home canonical POI id.
    pub home_poi: String,
    /// True while actively jumping or traveling.
    pub in_transit: bool,
    /// Transit type: "jump", "travel", or "pathfinder".
    pub transit_type: Option<String>,
    /// Destination system id during a jump.
    pub transit_dest_system: Option<String>,
    /// Destination POI id during travel.
    pub transit_dest_poi: Option<String>,
    /// Current location details from v2 status.
    pub location: RuntimeLocationDto,
    /// Player username.
    pub username: String,
    /// Player id.
    pub player_id: Option<String>,
    /// Player empire id.
    pub empire: String,
    /// Player clan tag.
    pub clan_tag: Option<String>,
    /// Player status message.
    pub status_message: Option<String>,
    /// Primary color.
    pub primary_color: Option<String>,
    /// Secondary color.
    pub secondary_color: Option<String>,
    /// Cloaking flag.
    pub is_cloaked: Option<bool>,
    /// Wreck currently being towed.
    pub towing_wreck_id: Option<String>,
    /// Visible and remembered salvage/wreck/container cargo.
    pub salvage: RuntimeSalvageStateDto,
    /// Standings keyed by empire id.
    pub standings: std::collections::HashMap<
        String,
        spacemolt_lib_rs::schema::V2GameStatePlayerStandingsValue,
    >,
    /// Player statistics supplied by the generated v2 player schema.
    pub player_stats: serde_json::Map<String, serde_json::Value>,
    /// Player faction id.
    pub faction_id: String,
    /// Player faction rank.
    pub faction_rank: String,
    /// Player skills keyed by skill id.
    pub skills: std::collections::HashMap<String, spacemolt_lib_rs::schema::V2GameStateSkillsValue>,
    /// Installed module ids on the active ship.
    pub installed_modules: Vec<String>,
    /// Installed modules from the generated v2 game-state schema.
    pub modules: Vec<spacemolt_lib_rs::schema::V2GameStateModulesItem>,
    /// Shipyard showroom.
    pub shipyard_showroom: Vec<RuntimeShipyardShowroomEntryDto>,
    /// Shipyard listings.
    pub shipyard_listings: Vec<RuntimeShipyardListingEntryDto>,
    /// Active/in-progress ship commissions.
    pub in_progress_commissions: Vec<spacemolt_lib_rs::schema::CommissionEntry>,
    /// Ship catalog.
    pub ship_catalogue: RuntimeCatalogueDto,
    /// Owned ships.
    pub owned_ships: Vec<RuntimeOwnedShipProjectionDto>,
    /// Faction garage contents.
    pub faction_garage: RuntimeFactionGarageDto,
    /// Passenger berths, aboard passengers, and current station board.
    pub passengers: RuntimePassengerStateDto,
    /// Available recipes.
    pub available_recipes: Vec<spacemolt_lib_rs::schema::CatalogDumpItemsItem>,
    /// Active missions.
    pub active_missions: Vec<spacemolt_lib_rs::schema::V2GameStateMissionsActiveItem>,
    /// Available missions.
    pub available_missions: Vec<spacemolt_lib_rs::schema::MissionInfo>,
    /// Notifications.
    pub notifications: Vec<RuntimeGameNotificationDto>,
    /// Chat messages.
    pub chat_messages: Vec<RuntimeGameChatMessageDto>,
    /// Current market.
    pub current_market: Option<RuntimeMarketStateDto>,
    /// Station context when docked.
    pub station: Option<RuntimeStationContextDto>,
}

/// Normalized item quantity projection combined across cargo/storage sources.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeItemQuantityProjectionDto {
    /// Item id.
    pub item_id: String,
    /// Quantity.
    pub quantity: i64,
}

/// Salvage knowledge DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSalvageStateDto {
    /// Lootables visible at the current/last observed POI.
    pub visible_lootables: Vec<RuntimeSpaceLootInfoDto>,
    /// Remembered lootables by POI id.
    pub lootables_by_poi: HashMap<String, Vec<RuntimeSpaceLootInfoDto>>,
    /// POI covered by visible_lootables.
    pub last_seen_poi: Option<String>,
    /// System covered by visible_lootables.
    pub last_seen_system: Option<String>,
    /// Unix seconds when observed.
    pub observed_at_unix: Option<i64>,
}

/// Visible wreck/container DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSpaceLootInfoDto {
    pub id: String,
    pub kind: String,
    pub poi_id: String,
    pub system_id: String,
    pub cargo: Vec<spacemolt_lib_rs::data::WreckCargoItem>,
    pub modules: Vec<spacemolt_lib_rs::data::WreckModule>,
    pub salvage_value: Option<i64>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub expire_tick: Option<i64>,
    pub ship_class: Option<String>,
    pub ship_name: Option<String>,
    pub victim_name: Option<String>,
    pub killer_name: Option<String>,
}

/// POI resource DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePoiResourceInfoDto {
    /// Resource id.
    pub resource_id: String,
    /// Name.
    pub name: String,
    /// Richness text.
    pub richness_text: String,
    /// Richness numeric score.
    pub richness: Option<i64>,
    /// Remaining amount.
    pub remaining: Option<i64>,
    /// Remaining display text.
    pub remaining_display: String,
}

/// POI DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePoiInfoDto {
    /// POI id.
    pub id: String,
    /// System id.
    pub system_id: String,
    /// Name.
    pub name: String,
    /// Type.
    pub r#type: String,
    /// Type-specific class.
    pub class_name: String,
    /// Description.
    pub description: String,
    /// Hidden flag.
    pub hidden: bool,
    /// X coordinate.
    pub x: Option<f64>,
    /// Y coordinate.
    pub y: Option<f64>,
    /// Base flag.
    pub has_base: bool,
    /// Base id.
    pub base_id: Option<String>,
    /// Base name.
    pub base_name: Option<String>,
    /// Online count.
    pub online: i64,
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
    /// Resource details.
    pub resources: Vec<RuntimePoiResourceInfoDto>,
}

/// Current location details from SpaceMolt v2 status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLocationDto {
    /// Complete generated SpaceMolt location section.
    pub spacemolt: spacemolt_lib_rs::schema::V2GameStateLocation,
    pub nearby_creature_count: Option<i64>,
    pub nearby_creatures: Vec<RuntimeWildlifeCreatureDto>,
}

/// Runtime galaxy state DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyStateDto {
    /// Map snapshot.
    pub map: RuntimeGalaxyMapSnapshotDto,
    /// Galaxy market.
    pub market: RuntimeGalaxyMarketDto,
    /// Galaxy catalog.
    pub catalog: RuntimeGalaxyCatalogDto,
    /// Resource indices.
    pub resources: RuntimeGalaxyResourcesDto,
    /// Wildlife sightings by system/POI.
    pub wildlife: RuntimeGalaxyWildlifeDto,
    /// Last update timestamp.
    pub updated_at_utc: DateTime<Utc>,
}

/// Commander galaxy state DTO. Market data intentionally lives behind
/// explicit market endpoints/tools, not the high-frequency commander snapshot.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCommanderGalaxyStateDto {
    /// Map snapshot.
    pub map: RuntimeGalaxyMapSnapshotDto,
    /// Galaxy catalog.
    pub catalog: RuntimeGalaxyCatalogDto,
    /// Resource indices.
    pub resources: RuntimeGalaxyResourcesDto,
    /// Wildlife sightings by system/POI.
    pub wildlife: RuntimeGalaxyWildlifeDto,
    /// Last update timestamp.
    pub updated_at_utc: DateTime<Utc>,
}

/// Runtime wildlife knowledge DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyWildlifeDto {
    pub systems: Vec<RuntimeWildlifeSystemDto>,
    pub pois: Vec<RuntimeWildlifePoiDto>,
}

/// Wildlife observed in one system.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWildlifeSystemDto {
    pub system_id: String,
    pub creature_count: i64,
    pub species: Vec<RuntimeWildlifeSpeciesDto>,
    pub pois: Vec<String>,
    pub observed_at_unix: i64,
}

/// Wildlife species summary within one system.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWildlifeSpeciesDto {
    pub species: String,
    pub name: String,
    pub role: String,
    pub count: i64,
}

/// Wildlife observed at one POI.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWildlifePoiDto {
    pub system_id: String,
    pub poi_id: String,
    pub creature_count: i64,
    pub observed_at_unix: i64,
    pub creatures: Vec<RuntimeWildlifeCreatureDto>,
}

/// Visible wildlife creature.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWildlifeCreatureDto {
    pub creature_id: String,
    pub species: String,
    pub name: String,
    pub role: String,
    pub hull: i64,
    pub max_hull: i64,
    pub in_combat: bool,
    pub system_id: String,
    pub poi_id: String,
    pub observed_at_unix: i64,
}

/// Galaxy map snapshot DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyMapSnapshotDto {
    /// Systems.
    pub systems: Vec<RuntimeGalaxySystemInfoDto>,
    /// Known POIs.
    pub known_pois: Vec<RuntimeGalaxyKnownPoiInfoDto>,
}

/// Galaxy system info DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxySystemInfoDto {
    /// System id.
    pub id: String,
    /// Last observed display name.
    pub name: Option<String>,
    /// Empire.
    pub empire: String,
    /// True when the map API marks this system as an empire stronghold.
    pub is_stronghold: bool,
    /// X coordinate.
    pub x: Option<f64>,
    /// Y coordinate.
    pub y: Option<f64>,
    /// Connections.
    pub connections: Vec<String>,
    pub poi_count: Option<usize>,
    pub pois_complete: bool,
    pub first_entered_unix: Option<i64>,
    pub last_entered_unix: Option<i64>,
    pub last_scanned_unix: Option<i64>,
    pub last_surveyed_unix: Option<i64>,
    pub bloom_status: Option<String>,
    pub bloom_intensity: Option<f64>,
    /// Unresolved signatures reported by the latest system survey.
    pub faint_signatures: Vec<serde_json::Value>,
    /// Wildlife summary reported by the latest system survey.
    pub wildlife: Vec<serde_json::Value>,
    /// System POIs.
    pub pois: Vec<RuntimeGalaxyPoiInfoDto>,
}

/// Galaxy POI info DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyPoiInfoDto {
    /// POI id.
    pub id: String,
    /// X coordinate.
    pub x: Option<f64>,
    /// Y coordinate.
    pub y: Option<f64>,
}

/// Known POI DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyKnownPoiInfoDto {
    /// POI id.
    pub id: String,
    /// System id.
    pub system_id: String,
    /// Name.
    pub name: String,
    /// Type.
    pub r#type: String,
    /// X coordinate.
    pub x: Option<f64>,
    /// Y coordinate.
    pub y: Option<f64>,
    /// Base flag.
    pub has_base: bool,
    /// Base id.
    pub base_id: Option<String>,
    /// Base name.
    pub base_name: Option<String>,
    pub resources: Vec<RuntimePoiResourceInfoDto>,
    pub first_discovered_unix: Option<i64>,
    pub last_observed_unix: Option<i64>,
    pub first_visited_unix: Option<i64>,
    pub last_visited_unix: Option<i64>,
}

/// Galaxy prices DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalaxyPricesResponse {
    /// Median buy prices.
    pub global_median_buy_prices: HashMap<String, f64>,
    /// Median sell prices.
    pub global_median_sell_prices: HashMap<String, f64>,
    /// Weighted mid prices.
    pub global_weighted_mid_prices: HashMap<String, f64>,
}

/// Runtime galaxy market DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyMarketDto {
    /// Market snapshots by station.
    pub markets_by_station: HashMap<String, RuntimeMarketStateDto>,
    /// Global median buy prices.
    pub global_median_buy_prices: HashMap<String, f64>,
    /// Global median sell prices.
    pub global_median_sell_prices: HashMap<String, f64>,
    /// Global weighted mid prices.
    pub global_weighted_mid_prices: HashMap<String, f64>,
}

/// Query params for compact auction-house style market lookups.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMarketQueryRequest {
    /// Item id/name substring to match.
    #[serde(alias = "item_id", alias = "item", alias = "q")]
    pub item_id: Option<String>,
    /// Station id/name substring to match.
    #[serde(alias = "station_id")]
    pub station: Option<String>,
    /// System id to match.
    #[serde(alias = "system_id")]
    pub system: Option<String>,
    /// Order side: `sell`, `buy`, or `both`. Sell orders are asks you can buy.
    pub side: Option<String>,
    /// Minimum price per unit.
    #[serde(alias = "min_price")]
    pub min_price: Option<f64>,
    /// Maximum price per unit.
    #[serde(alias = "max_price")]
    pub max_price: Option<f64>,
    /// Minimum listed quantity.
    #[serde(alias = "min_quantity")]
    pub min_quantity: Option<i64>,
    /// Only return orders owned by the current player/faction.
    #[serde(alias = "own_only")]
    pub own_only: Option<bool>,
    /// Sort mode: `price_asc`, `price_desc`, `quantity_desc`, `jumps`, `station`, or `item`.
    pub sort: Option<String>,
    /// Maximum rows to return. Defaults to 50 and caps at 200.
    pub limit: Option<usize>,
}

/// Compact market query response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMarketQueryResponse {
    /// Matching order rows after sorting and limit.
    pub orders: Vec<RuntimeMarketQueryOrderProjectionDto>,
    /// Matching rows before limit.
    pub total_matches: usize,
    /// Returned row count.
    pub returned: usize,
    /// True when more rows matched than were returned.
    pub truncated: bool,
}

/// Normalized cross-station auction-house market projection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMarketQueryOrderProjectionDto {
    /// Station id.
    pub station_id: String,
    /// Station display name, when known.
    pub station_name: Option<String>,
    /// System id, when known.
    pub system_id: Option<String>,
    /// Jumps from the session's current system, when known.
    pub jumps: Option<i32>,
    /// Item id.
    pub item_id: String,
    /// Item display name, when known.
    pub item_name: Option<String>,
    /// Order side: `sell` means market asks you can buy; `buy` means bids you can sell into.
    pub side: String,
    /// Price each.
    pub price_each: f64,
    /// Quantity available at this price level.
    pub quantity: i64,
    /// Unix seconds when this station market was observed, when known.
    pub observed_at_unix: Option<i64>,
    /// Upstream market cursor from the last successful `view_market`.
    pub current_tick: Option<i64>,
    /// Upstream order source label, when provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Quantity at this level owned by the current player/faction, when provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub my_quantity: Option<i64>,
}

/// Runtime galaxy catalog DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyCatalogDto {
    /// Items by id.
    pub items_by_id: HashMap<String, spacemolt_lib_rs::schema::CatalogDumpItemsItem>,
    /// Ships by id.
    pub ships_by_id: HashMap<String, spacemolt_lib_rs::schema::ShipClass>,
    /// Recipes by id.
    pub recipes_by_id: HashMap<String, spacemolt_lib_rs::schema::Recipe>,
    /// Facility types by id.
    pub facilities_by_id: HashMap<String, spacemolt_lib_rs::schema::FacilityDefinition>,
    /// Skills by id.
    pub skills_by_id: HashMap<String, spacemolt_lib_rs::schema::SkillDefinition>,
}

/// Galaxy resources DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyResourcesDto {
    /// Systems by resource.
    pub systems_by_resource: HashMap<String, Vec<String>>,
    /// POIs by resource.
    pub pois_by_resource: HashMap<String, Vec<String>>,
}

/// Known sources and storage destinations for mining a resource.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResourceSourcesResponse {
    /// Canonical resource id that matched the request.
    pub resource_id: String,
    /// Known POIs that can source the resource.
    pub sources: Vec<RuntimeResourceSourcePoiDto>,
    /// Known station/base POIs suitable as storage destinations.
    pub destinations: Vec<RuntimeResourceDestinationPoiDto>,
}

/// A known POI source for a resource.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResourceSourcePoiDto {
    /// POI id.
    pub poi_id: String,
    /// System id.
    pub system_id: String,
    /// Display name.
    pub name: String,
    /// POI type.
    pub r#type: String,
    /// Base flag.
    pub has_base: bool,
    /// Jump distance from current system, when known.
    pub jumps: Option<i32>,
    /// Whether current resource details indicate depletion.
    pub depleted: bool,
    /// Last observed resource details for this POI/resource, when known.
    pub resource: Option<RuntimePoiResourceInfoDto>,
}

/// A known POI destination for storing mined cargo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResourceDestinationPoiDto {
    /// Explicit POI id.
    pub poi_id: String,
    /// System id.
    pub system_id: String,
    /// Display name.
    pub name: String,
    /// POI type.
    pub r#type: String,
    /// Base flag.
    pub has_base: bool,
    /// Jump distance from current system, when known.
    pub jumps: Option<i32>,
}

/// Social response: every other player ("bot") ever sighted.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialResponse {
    /// Sighted players, most recently seen first.
    pub bots: Vec<SocialBotDto>,
    /// Optional session-scoped consolidated chat snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<GameChatResponse>,
}

/// Consolidated chat response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameChatResponse {
    /// Messages from all requested channels, newest first.
    pub messages: Vec<GameChatMessageDto>,
    /// Channel summaries.
    pub channels: Vec<GameChatChannelSummaryDto>,
    /// Total message count across returned channels.
    pub total_count: usize,
    /// True if any channel reports older messages.
    pub has_more: bool,
}

/// Per-channel chat summary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameChatChannelSummaryDto {
    /// Channel.
    pub channel: String,
    /// Returned messages.
    pub message_count: usize,
    /// Total available messages when reported by upstream.
    pub total_count: Option<usize>,
    /// True if more messages are available.
    pub has_more: bool,
    /// Error for this channel, if the channel failed.
    pub error: Option<String>,
}

/// Rich game chat message DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameChatMessageDto {
    /// Message id.
    pub id: String,
    /// Channel.
    pub channel: String,
    /// Sender id.
    pub sender_id: Option<String>,
    /// Sender name.
    pub sender: String,
    /// Content.
    pub content: String,
    /// Message timestamp.
    pub timestamp_utc: Option<DateTime<Utc>>,
    /// System id.
    pub system_id: Option<String>,
    /// POI id.
    pub poi_id: Option<String>,
    /// Faction id.
    pub faction_id: Option<String>,
    /// Target player id.
    pub target_id: Option<String>,
    /// Target player name.
    pub target_name: Option<String>,
    /// True if posted by empire/system authority.
    pub empire_official: bool,
}

/// One sighted player DTO. Optional fields were not revealed by the server
/// at the last sighting.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialBotDto {
    /// Actor kind: player, pirate, or empire.
    pub actor_kind: String,
    /// True for non-player rows synthesized from current nearby NPC state.
    pub synthetic: bool,
    /// Player id.
    pub player_id: Option<String>,
    /// Username.
    pub username: String,
    /// Faction id at last sighting.
    pub faction_id: Option<String>,
    /// Faction tag at last sighting.
    pub faction_tag: Option<String>,
    /// Clan tag at last sighting.
    pub clan_tag: Option<String>,
    /// Ship class at last sighting.
    pub ship_class: Option<String>,
    /// Ship name at last sighting.
    pub ship_name: Option<String>,
    /// Status message at last sighting.
    pub status_message: Option<String>,
    /// Ship primary color at last sighting.
    pub primary_color: Option<String>,
    /// Ship secondary color at last sighting.
    pub secondary_color: Option<String>,
    /// In-combat flag at last sighting.
    pub in_combat: bool,
    /// Offline flag at last sighting.
    pub offline: bool,
    /// System the player was last seen in.
    pub last_seen_system: String,
    /// First recorded sighting.
    pub first_seen_utc: DateTime<Utc>,
    /// Most recent sighting.
    pub last_seen_utc: DateTime<Utc>,
    /// Recorded sighting count.
    pub times_seen: i64,
}

/// Market state DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMarketStateDto {
    /// Station id.
    pub station_id: String,
    /// Point-of-interest id containing this market.
    pub poi_id: Option<String>,
    /// Human-readable point-of-interest name.
    pub station_name: Option<String>,
    /// Sell orders.
    pub sell_orders: HashMap<String, Vec<spacemolt_lib_rs::data::MarketOrder>>,
    /// Buy orders.
    pub buy_orders: HashMap<String, Vec<spacemolt_lib_rs::data::MarketOrder>>,
    /// Unix seconds when this snapshot was observed, when known.
    pub observed_at_unix: Option<i64>,
    /// Upstream market cursor from the last successful `view_market`.
    pub current_tick: Option<i64>,
}

/// Planning-only faction-storage market order.
pub type RuntimeVirtualMarketOrderDto = prayer_runtime::knowledge::VirtualMarketOrder;

/// Virtual market orders response.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualMarketOrdersResponse {
    /// Configured virtual orders.
    pub orders: Vec<RuntimeVirtualMarketOrderDto>,
}

/// Result of trying to reserve a virtual market order.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualOrderReservationResultDto {
    /// Virtual order id.
    pub order_id: String,
    /// Stable id for the active reservation, when any quantity was accepted.
    pub reservation_id: Option<String>,
    /// Quantity requested in this reservation call.
    pub requested: i64,
    /// Quantity accepted by this reservation call.
    pub accepted: i64,
    /// Reserved quantity before this reservation call.
    pub reserved_before: i64,
    /// Reserved quantity after this reservation call.
    pub reserved_after: i64,
}

/// Reserve response including both the updated orders and accepted quantities.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualOrderReserveResponse {
    /// Configured virtual orders after the reservation attempt.
    pub orders: Vec<RuntimeVirtualMarketOrderDto>,
    /// Per-order accepted quantities for this reservation attempt.
    pub reservation_results: Vec<RuntimeVirtualOrderReservationResultDto>,
}

/// Reserve response for virtual craft orders.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualCraftOrderReserveResponse {
    /// Configured craft orders after the reservation attempt.
    pub orders: Vec<RuntimeVirtualCraftOrderDto>,
    /// Per-order accepted quantities for this reservation attempt.
    pub reservation_results: Vec<RuntimeVirtualOrderReservationResultDto>,
}

/// Replace all virtual market orders.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualMarketOrdersRequest {
    /// New order list.
    #[serde(default)]
    pub orders: Vec<RuntimeVirtualMarketOrderDto>,
}

/// Virtual order reservation row.
pub type RuntimeVirtualOrderUseDto = prayer_runtime::knowledge::VirtualOrderUse;

/// Reserve one or more virtual order quantities.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualOrderReserveRequest {
    /// Reservation requests.
    #[serde(default)]
    pub uses: Vec<RuntimeVirtualOrderUseDto>,
}

/// One canonical physical-inventory claim in a market movement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventoryClaimDto {
    /// Stable lot id returned by inventory projection when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_id: Option<String>,
    /// Source kind: cargo, personal_storage, or faction_storage.
    pub source_kind: String,
    /// Canonical player/faction owner id.
    pub owner_id: String,
    /// Canonical POI id or accepted station/base alias.
    pub location_id: String,
    /// Item id being claimed.
    pub item_id: String,
    /// Positive quantity requested.
    pub quantity: i64,
}

/// Atomic inventory reservation request for one movement package.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventoryMovementReserveRequest {
    /// Public bot selector whose current state establishes identity and live stock.
    pub session_id: String,
    /// Caller-defined movement kind, normally arbitrage or logistics.
    pub kind: String,
    /// All physical lots required before launch.
    #[serde(default)]
    pub claims: Vec<RuntimeInventoryClaimDto>,
    /// Virtual faction order capacities required by the same package.
    #[serde(default)]
    pub virtual_order_uses: Vec<RuntimeVirtualOrderUseDto>,
    /// Prayer-owned workflow context used to recover after an MCP restart.
    #[serde(default)]
    pub context: Value,
}

/// Lifecycle of an inventory-backed movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInventoryMovementStatusDto {
    Reserved,
    Running,
    Completed,
    Failed,
    Released,
    NeedsReconciliation,
}

/// One inventory-backed market movement record.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventoryMovementDto {
    #[schemars(with = "String")]
    pub movement_id: Uuid,
    #[schemars(with = "String")]
    pub session_id: Uuid,
    pub kind: String,
    pub status: RuntimeInventoryMovementStatusDto,
    pub claims: Vec<RuntimeInventoryClaimDto>,
    pub virtual_order_uses: Vec<RuntimeVirtualOrderUseDto>,
    pub context: Value,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

/// Result of an all-or-nothing inventory reservation.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventoryMovementReserveResponse {
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement: Option<RuntimeInventoryMovementDto>,
    #[serde(default)]
    pub unavailable_claims: Vec<RuntimeInventoryClaimDto>,
    #[serde(default)]
    pub unavailable_virtual_order_uses: Vec<RuntimeVirtualOrderUseDto>,
}

/// Active and historical inventory movements.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventoryMovementsResponse {
    pub movements: Vec<RuntimeInventoryMovementDto>,
}

/// Planning-only faction-storage craft goal.
pub type RuntimeVirtualCraftOrderDto = prayer_runtime::knowledge::VirtualCraftOrder;

/// Virtual craft orders response.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualCraftOrdersResponse {
    /// Configured virtual craft orders.
    pub orders: Vec<RuntimeVirtualCraftOrderDto>,
}

/// Replace all virtual craft orders.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualCraftOrdersRequest {
    /// New craft order list.
    #[serde(default)]
    pub orders: Vec<RuntimeVirtualCraftOrderDto>,
}

/// Craft job lifecycle state.
pub type RuntimeCraftJobStatusDto = prayer_state::CraftJobStatus;

/// Merged SpaceMolt/API crafting queue response.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCraftingQueueResponse {
    pub crafting_queue: Vec<prayer_state::CraftingQueueProjection>,
}

/// Logistics endpoint descriptor.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogisticsEndpointDto {
    /// Endpoint kind: "market", "virtual_faction", or "personal_storage".
    pub kind: String,
    /// Local virtual order id when this endpoint came from faction storage liquidity.
    pub virtual_order_id: Option<String>,
    /// Stable inventory/depth claim key for atomic movement reservation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_key: Option<String>,
}

/// Scored logistics item DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogisticsItemDto {
    /// Item id.
    pub item_id: String,
    /// Planned quantity.
    pub quantity: i64,
    /// Cargo volume one unit occupies.
    pub item_size: i64,
    /// Source-side planning price.
    pub source_price: f64,
    /// Destination-side planning price.
    pub destination_price: f64,
    /// Where the bot obtains this item.
    pub source: RuntimeLogisticsEndpointDto,
    /// Where the bot sends this item.
    pub destination: RuntimeLogisticsEndpointDto,
    /// Priority multiplier used for scoring.
    pub priority: f64,
    /// Route value per unit.
    pub value_per_unit: f64,
    /// Route value for the planned quantity.
    pub route_value: f64,
    /// Item score.
    pub score: f64,
}

/// One executable logistics haul.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogisticsPackageDto {
    /// Source station.
    pub source_station_id: String,
    /// Source system.
    pub source_system_id: String,
    /// Destination station.
    pub destination_station_id: String,
    /// Destination system.
    pub destination_system_id: String,
    /// Package items.
    pub items: Vec<RuntimeLogisticsItemDto>,
    /// Cargo volume used by the package.
    pub cargo_used: i64,
    /// Cargo volume budget used while building the package.
    pub cargo_capacity: i64,
    /// Jumps from current system/origin to source station.
    pub jumps_to_source: i64,
    /// Jumps from source station to destination station.
    pub jumps_source_to_destination: i64,
    /// Scoring jump denominator.
    pub total_jumps: i64,
    /// Package score.
    pub score: f64,
}

/// Economy deal DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEconomyDealDto {
    /// Item id.
    pub item_id: String,
    /// Buy station id.
    pub buy_station_id: String,
    /// Buy price.
    pub buy_price: f64,
    /// Sell station id.
    pub sell_station_id: String,
    /// Sell price.
    pub sell_price: f64,
    /// Profit per unit.
    pub profit_per_unit: f64,
}

/// Scored single-source-station craft-profit deal DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCraftProfitDealDto {
    /// Recipe id.
    pub recipe_id: String,
    /// Recipe display name.
    pub recipe_name: String,
    /// Station where all recipe inputs can be bought.
    pub buy_station_id: String,
    /// System of the input-buy station.
    pub buy_system_id: String,
    /// Station where crafted outputs can be sold.
    pub sell_station_id: String,
    /// System of the output-sell station.
    pub sell_system_id: String,
    /// Number of recipe crafts priced through current order-book depth.
    pub crafts: i64,
    /// Total cost to buy all inputs for `crafts` at `buyStationId`.
    pub input_cost: i64,
    /// Total revenue from selling crafted outputs at `sellStationId`.
    pub output_revenue: i64,
    /// Output revenue minus input cost.
    pub total_profit: i64,
    /// Average profit per craft.
    pub profit_per_craft: f64,
    /// Cargo volume produced per craft, used with maxUnits/cargo-capacity caps.
    pub output_volume_per_craft: i64,
    /// Jumps from the current system/origin to the input-buy station.
    pub jumps_to_buy: i64,
    /// Jumps from the input-buy station to the output-sell station.
    pub jumps_buy_to_sell: i64,
    /// Cargo-constrained profit per jump.
    pub score: f64,
}

/// How an arbitrage deal obtains cargo.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArbitrageAcquireFromDto {
    /// Source kind: "market", "virtual_faction", or "personal_storage".
    pub kind: String,
    /// Local virtual order id when this source came from configured faction storage liquidity.
    pub virtual_order_id: Option<String>,
    /// Stable physical/depth claim key for movement reservation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_key: Option<String>,
}

/// How an arbitrage deal disposes of cargo.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArbitrageDisposeToDto {
    /// Target kind: "market" or "virtual_faction".
    pub kind: String,
    /// Local virtual order id when this target came from configured faction storage liquidity.
    pub virtual_order_id: Option<String>,
}

/// Scored arbitrage deal DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArbitrageDealDto {
    /// Item id.
    pub item_id: String,
    /// Station to buy from.
    pub buy_station_id: String,
    /// System of the buy station.
    pub buy_system_id: String,
    /// Where the bot obtains this item.
    pub acquire_from: RuntimeArbitrageAcquireFromDto,
    /// Average buy price per unit over the matched volume.
    pub buy_price: f64,
    /// Station to sell at.
    pub sell_station_id: String,
    /// System of the sell station.
    pub sell_system_id: String,
    /// Where the bot sends this item.
    pub dispose_to: RuntimeArbitrageDisposeToDto,
    /// Average sell price per unit over the matched volume.
    pub sell_price: f64,
    /// Average profit per unit.
    pub profit_per_unit: f64,
    /// Cargo volume one unit of this item occupies (catalog `size`, ≥1).
    pub item_size: i64,
    /// Units that can be flipped profitably, capped so `quantity * itemSize`
    /// fits the cargo-volume budget.
    pub quantity: i64,
    /// Total profit over the matched volume.
    pub total_profit: i64,
    /// Credits required to buy the matched volume.
    pub capital_required: i64,
    /// Return on invested credits: `totalProfit / capitalRequired`.
    pub roi: f64,
    /// Profit as a share of sell-side revenue; the sell-price drop cushion
    /// before break-even.
    pub gross_margin: f64,
    /// Destination buy-order depth at or above buyPrice divided by planned
    /// sell quantity.
    pub break_even_cover: f64,
    /// Coarse margin-of-safety band: low, medium, high, or thin.
    pub risk_band: String,
    /// Jumps from the current system to the buy station. Zero in global
    /// arbitrage scope, where only buy-to-sell travel is scored.
    pub jumps_to_buy: i64,
    /// Jumps from the buy station to the sell station.
    pub jumps_buy_to_sell: i64,
    /// Age of the deal's stalest market snapshot in seconds; null when a
    /// leg predates snapshot timestamps.
    pub data_age_seconds: Option<i64>,
    /// Cargo-constrained profit per jump, exponentially discounted by data age.
    /// Current scope divides by current-to-buy plus buy-to-sell jumps; global
    /// scope divides by buy-to-sell jumps only. `totalProfit` already reflects
    /// what fits in the cargo budget, so this is profit-per-haul per jump.
    /// Unknown-age data scores zero.
    pub raw_score: f64,
    /// Risk-adjusted score: `rawScore` times softened margin and depth
    /// multipliers, so thin-spread flips are discounted without erasing large
    /// absolute profit.
    pub score: f64,
}

/// Passenger berth usage/capacity by class.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePassengerBerthUsageDto {
    /// Economy berths.
    pub economy: i64,
    /// Business berths.
    pub business: i64,
    /// First-class berths.
    pub first: i64,
}

/// Scored passenger fare DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePassengerFareDealDto {
    /// Stable citizen id.
    pub citizen_id: String,
    /// Passenger display name.
    pub name: String,
    /// Requested berth class.
    pub class_name: String,
    /// Boarding station.
    pub origin_station_id: String,
    /// Destination station/base.
    pub destination_station_id: String,
    /// Destination system.
    pub destination_system_id: Option<String>,
    /// Estimated fare on successful delivery.
    pub estimated_fare: i64,
    /// Base fare when known.
    pub base_fare: Option<i64>,
    /// Speed bonus when known.
    pub speed_bonus: Option<i64>,
    /// Berth units consumed in the requested class.
    pub berth_units: i64,
    /// Route jumps from origin to destination.
    pub total_jumps: i64,
    /// Estimated fare divided by route jumps.
    pub fare_per_jump: f64,
    /// Fare score.
    pub score: f64,
    /// Risk band, currently always "passenger".
    pub risk_band: String,
}

/// First-class arbitrage package member DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeArbitradePackageMemberDto {
    /// Market/logistics item deal.
    ItemDeal { deal: RuntimeArbitrageDealDto },
    /// Passenger fare.
    PassengerFare { fare: RuntimePassengerFareDealDto },
}

/// Optimized one-hop arbitrage package DTO.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArbitradePackageDto {
    /// Station to buy all package items from.
    pub buy_station_id: String,
    /// System of the buy station.
    pub buy_system_id: String,
    /// Station to sell all package items at.
    pub sell_station_id: String,
    /// System of the sell station.
    pub sell_system_id: String,
    /// Package item deals.
    pub deals: Vec<RuntimeArbitrageDealDto>,
    /// First-class package members.
    pub members: Vec<RuntimeArbitradePackageMemberDto>,
    /// Passenger fares in this package.
    pub passenger_fares: Vec<RuntimePassengerFareDealDto>,
    /// Cargo volume used by the package.
    pub cargo_used: i64,
    /// Cargo volume budget used while building the package.
    pub cargo_capacity: i64,
    /// Credits required to buy the package.
    pub capital_required: i64,
    /// Total package profit.
    pub total_profit: i64,
    /// Portion of total profit expected from passenger fares.
    pub passenger_revenue: i64,
    /// Passenger berths consumed by class.
    pub berth_used: RuntimePassengerBerthUsageDto,
    /// Total passenger berth capacity by class.
    pub berth_capacity: RuntimePassengerBerthUsageDto,
    /// Return on invested credits.
    pub roi: f64,
    /// Profit as a share of sell-side revenue.
    pub gross_margin: f64,
    /// Capital-weighted destination buy-order depth at or above each deal's
    /// buyPrice divided by planned capital required.
    pub break_even_cover: f64,
    /// Coarse margin-of-safety band: low, medium, high, or thin.
    pub risk_band: String,
    /// Jumps from current system/origin to buy station.
    pub jumps_to_buy: i64,
    /// Jumps from buy station to sell station.
    pub jumps_buy_to_sell: i64,
    /// Age of the package's stalest market snapshot in seconds.
    pub data_age_seconds: Option<i64>,
    /// Cargo-constrained package profit per jump, discounted by data age.
    pub raw_score: f64,
    /// Risk-adjusted package score.
    pub score: f64,
    /// Anchor kind: "item_deal" or "passenger_fare".
    pub anchor_kind: String,
}

/// Calculated player-ship summary used by the legacy HTTP state projection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePlayerShipProjectionDto {
    /// Ship name.
    pub name: String,
    /// Class id.
    pub class_id: String,
    /// System id.
    pub system_id: String,
    /// Armor.
    pub armor: i64,
    /// Speed.
    pub speed: i64,
    /// CPU used.
    pub cpu_used: i64,
    /// CPU capacity.
    pub cpu_capacity: i64,
    /// Power used.
    pub power_used: i64,
    /// Power capacity.
    pub power_capacity: i64,
    /// Module count.
    pub module_count: i64,
    /// Fuel.
    pub fuel: i64,
    /// Max fuel.
    pub max_fuel: i64,
    /// Fuel percent.
    pub fuel_percent: i64,
    /// Hull.
    pub hull: i64,
    /// Max hull.
    pub max_hull: i64,
    /// Shield.
    pub shield: i64,
    /// Max shield.
    pub max_shield: i64,
    /// Cargo used.
    pub cargo_used: i64,
    /// Cargo capacity.
    pub cargo_capacity: i64,
}

/// Commander ownership annotations around a canonical SpaceMolt owned ship.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOwnedShipProjectionDto {
    /// Owning session handle, when this row came from a commander projection.
    pub owner_handle: String,
    /// Owner kind, such as personal or faction.
    pub owner_kind: String,
    /// Stable owner id, when known.
    pub owner_id: String,
    /// Human-facing owner name, when known.
    pub owner_name: String,
    /// Owning faction id, when known.
    pub faction_id: String,
    /// Owning faction tag, when known.
    pub faction_tag: String,
    /// Canonical SpaceMolt list-ships row.
    pub ship: spacemolt_lib_rs::schema::OwnedShipInfo,
}

/// Faction garage DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFactionGarageDto {
    /// Occupied garage slots, when known.
    pub used: Option<i64>,
    /// Total garage slots, when known.
    pub capacity: Option<i64>,
    /// Ships parked in the garage.
    pub ships: Vec<RuntimeFactionGarageShipProjectionDto>,
}

/// Commander projection around a canonical SpaceMolt faction-garage ship.
///
/// The owner/faction and station fields are Prayer annotations used to merge
/// observations from several sessions. The embedded `ship` remains the
/// generated SpaceMolt wire value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFactionGarageShipProjectionDto {
    /// Session handle that observed this row, for commander projections.
    pub owner_handle: String,
    /// Garage station/base id, when known.
    pub base_id: String,
    /// Garage station/base name, when known.
    pub base_name: String,
    /// Garage station system name, when known.
    pub system_name: String,
    /// Faction id, when known.
    pub faction_id: String,
    /// Faction tag, when known.
    pub faction_tag: String,
    /// Canonical SpaceMolt garage ship value.
    pub ship: spacemolt_lib_rs::schema::GaragedShipEntry,
}

/// Passenger state DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePassengerStateDto {
    /// Count of passengers currently aboard, when known.
    pub aboard_count: Option<i64>,
    /// Economy berth occupancy.
    pub economy_berths: RuntimePassengerBerthViewDto,
    /// Business berth occupancy.
    pub business_berths: RuntimePassengerBerthViewDto,
    /// First-class berth occupancy.
    pub first_berths: RuntimePassengerBerthViewDto,
    /// Passengers currently aboard.
    pub aboard: Vec<spacemolt_lib_rs::schema::PassengerView>,
    /// Station id for the waiting-passenger board.
    pub station: String,
    /// Count of waiting passengers at `station`, when known.
    pub waiting_count: Option<i64>,
    /// Passengers waiting at `station`.
    pub waiting: Vec<spacemolt_lib_rs::schema::WaitingPassengerView>,
}

/// Derived passenger berth occupancy view parsed from SpaceMolt's text field.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePassengerBerthViewDto {
    /// Occupied berths.
    pub current: i64,
    /// Total berths.
    pub max: i64,
}

/// Shipyard showroom entry DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeShipyardShowroomEntryDto {
    /// Ship class id.
    pub ship_class_id: String,
    /// Ship id.
    pub ship_id: Option<String>,
    /// Name.
    pub name: String,
    /// Category.
    pub category: String,
    /// Tier.
    pub tier: Option<i64>,
    /// Scale.
    pub scale: Option<i64>,
    /// Hull.
    pub hull: Option<i64>,
    /// Shield.
    pub shield: Option<i64>,
    /// Cargo.
    pub cargo: Option<i64>,
    /// Speed.
    pub speed: Option<i64>,
    /// Price.
    pub price: Option<f64>,
}

/// Shipyard listing entry DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeShipyardListingEntryDto {
    /// Listing id.
    pub listing_id: String,
    /// Name.
    pub name: String,
    /// Class id.
    pub class_id: String,
    /// Price.
    pub price: Option<f64>,
}

/// Legacy HTTP catalogue envelope containing canonical generated entries.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCatalogueDto {
    /// Catalogue type.
    #[serde(rename = "type")]
    pub r#type: String,
    /// Category.
    pub category: Option<String>,
    /// Selected id.
    pub id: Option<String>,
    /// Page.
    pub page: Option<i64>,
    /// Page size.
    #[serde(rename = "page_size")]
    pub page_size: Option<i64>,
    /// Total pages.
    #[serde(rename = "total_pages")]
    pub total_pages: Option<i64>,
    /// Total items.
    #[serde(rename = "total_items")]
    pub total_items: Option<i64>,
    /// Total entries.
    pub total: Option<i64>,
    /// Message.
    pub message: String,
    /// Items.
    pub items: Vec<spacemolt_lib_rs::schema::CatalogDumpItemsItem>,
    /// Entries.
    pub entries: Vec<spacemolt_lib_rs::schema::CatalogDumpItemsItem>,
    /// Ships.
    pub ships: Vec<spacemolt_lib_rs::schema::CatalogDumpItemsItem>,
}

/// Game notification DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGameNotificationDto {
    /// Notification type.
    pub r#type: String,
    /// Summary.
    pub summary: String,
    /// Raw payload JSON.
    pub payload_json: String,
}

/// Chat message DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGameChatMessageDto {
    /// Message id.
    pub message_id: String,
    /// Channel.
    pub channel: String,
    /// Sender.
    pub sender: String,
    /// Content.
    pub content: String,
    /// Seen tick.
    pub seen_tick: i64,
}

/// Station context DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStationContextDto {
    /// Station id.
    pub station_id: String,
    /// Station name.
    pub station_name: String,
    /// Station market.
    pub market: Option<RuntimeMarketStateDto>,
    /// Shipyard showroom.
    pub shipyard_showroom: Vec<RuntimeShipyardShowroomEntryDto>,
    /// Shipyard listings.
    pub shipyard_listings: Vec<RuntimeShipyardListingEntryDto>,
    /// Craftable item ids.
    pub craftable: Vec<String>,
    /// Typed optimistic crafting queue projection at the docked station.
    pub crafting_queue: Vec<prayer_state::CraftingQueueProjection>,
}

/// Station storage DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationStorageResponse {
    /// Storage credits.
    pub storage_credits: i64,
    /// Storage items.
    pub storage_items: HashMap<String, RuntimeItemQuantityProjectionDto>,
}

/// Station shipyard DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationShipyardResponse {
    /// Compiled owned ship list for this session.
    pub owned_ships: Vec<RuntimeOwnedShipProjectionDto>,
    /// Installed module ids on the active ship.
    pub installed_modules: Vec<String>,
    /// Faction garage contents.
    pub faction_garage: RuntimeFactionGarageDto,
    /// Shipyard showroom.
    pub shipyard_showroom: Vec<RuntimeShipyardShowroomEntryDto>,
    /// Shipyard listings.
    pub shipyard_listings: Vec<RuntimeShipyardListingEntryDto>,
    /// Active/in-progress ship commissions.
    pub in_progress_commissions: Vec<spacemolt_lib_rs::schema::CommissionEntry>,
}

/// Live facility snapshot for one SpaceMolt session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilitiesSnapshotResponse {
    /// Session id.
    pub session_id: String,
    /// Monotonic session state version when cached state is available.
    pub state_version: Option<u64>,
    /// Player username when cached state is available.
    pub username: Option<String>,
    /// Latest known system id.
    pub latest_system: Option<String>,
    /// Latest known POI/station id.
    pub latest_poi: Option<String>,
    /// Docked flag.
    pub docked: Option<bool>,
    /// Whether current/faction-current facility data came from the shared API cache.
    pub current_cached: bool,
    /// Unix timestamp when the current/faction-current facility data was observed.
    pub current_observed_at_unix: Option<i64>,
    /// Generated current-station facility response.
    pub current: Option<spacemolt_lib_rs::schema::FacilityResponse>,
    /// Generated personally-owned facility response.
    pub owned: Option<spacemolt_lib_rs::schema::FacilityResponse>,
    /// Generated current-station faction facility response.
    pub faction_current: Option<spacemolt_lib_rs::schema::FacilityResponse>,
    /// Generated faction-owned facility response.
    pub faction_owned: Option<spacemolt_lib_rs::schema::FacilityResponse>,
    /// Faction id returned by faction-owned facility query.
    pub faction_id: Option<String>,
    /// Faction treasury rent bill per cycle.
    pub faction_rent_per_cycle: Option<i64>,
    /// Faction facility rent arrears currently owed.
    pub faction_arrears_owed: Option<i64>,
    /// Facility type catalog rows.
    pub types: Vec<spacemolt_lib_rs::schema::FacilityDefinition>,
    /// Upstream shape or API errors seen while building the DTO.
    pub errors: Vec<String>,
}

/// Session summary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    /// Session id.
    pub id: String,
    /// Session label.
    pub label: String,
    /// Creation timestamp.
    pub created_utc: DateTime<Utc>,
    /// Last update timestamp.
    pub last_updated_utc: DateTime<Utc>,
    /// Halt flag.
    pub is_halted: bool,
    /// Active command flag.
    pub has_active_command: bool,
    /// Current script line.
    pub current_script_line: Option<usize>,
    /// Latest known system from the projected SpaceMolt state.
    pub latest_system: Option<String>,
    /// Latest known point of interest from the projected SpaceMolt state.
    pub latest_poi: Option<String>,
}

/// Request body for checkpoint restore.
#[derive(Debug, Clone, Deserialize)]
pub struct RestoreCheckpointRequest {
    /// Checkpoint to restore.
    pub checkpoint: PersistedExecutionRun,
}

/// Request body for halt/resume operations.
#[derive(Debug, Clone, Deserialize)]
pub struct ReasonRequest {
    /// Optional reason message.
    pub reason: Option<String>,
}

/// Response body for single-step host execution.
#[derive(Debug, Clone, Serialize)]
pub struct StepResponse {
    /// Whether a command was executed this step.
    pub executed: bool,
    /// Executed command action.
    pub command_action: Option<String>,
    /// Executed command args.
    pub command_args: Option<Vec<String>>,
    /// Result message from command execution.
    pub result_message: Option<String>,
    /// Whether runtime is currently halted.
    pub halted: bool,
    /// Whether the step intentionally delayed before the script runner continues.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
    /// Milliseconds the script runner should wait before continuing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_after_ms: Option<u64>,
    /// Transport/runtime error that failed the command, if any. The runtime
    /// halts when this is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Before/after location. Only emitted when system or POI changed.
/// Values are formatted as "before -> after".
#[derive(Debug, Clone, Serialize)]
pub struct ScriptLocationDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poi: Option<String>,
}

/// State flags captured around script execution.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptDiffFlags {
    pub docked_before: bool,
    pub docked_after: bool,
    pub halted_after: bool,
}

/// Diff of game state before and after script execution.
/// Scalar fields use "before -> after" format. Item lists use "item: before -> after" format.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptDiff {
    /// Only present when credits changed. Format: "before -> after".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<String>,
    /// Only present when fuel changed. Format: "before -> after".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuel: Option<String>,
    /// Only present when system or POI changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ScriptLocationDelta>,
    /// Format per entry: "item: before -> after". Empty when nothing changed.
    pub cargo: Vec<String>,
    /// Omitted when docking state changed (storage visibility is unreliable across dock/undock).
    /// Format per entry: "item: before -> after".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<Vec<String>>,
    pub flags: ScriptDiffFlags,
}

/// Response body for script execution runs.
#[derive(Debug, Clone, Serialize)]
pub struct ExecuteScriptResponse {
    /// Number of steps executed.
    pub steps_executed: usize,
    /// Whether runtime is currently halted.
    pub halted: bool,
    /// Whether runtime reached completion (`decide_next == None` while not halted).
    pub completed: bool,
    /// Parse or execution error that stopped the run, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Result message from the step that caused a halt, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub halt_message: Option<String>,
    /// State diff from before to after execution. Omitted on transport error before any steps ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<ScriptDiff>,
}

/// Response body for event drains.
#[derive(Debug, Clone, Serialize)]
pub struct EventsResponse {
    /// Emitted events.
    pub events: Vec<RuntimeEvent>,
}
