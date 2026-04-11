//! API request/response contracts.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use prayer_runtime::engine::{EngineCheckpoint, EngineExecutionResult, GameState, RuntimeEvent};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Serializable API error payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Human-readable message.
    pub error: String,
}

/// Session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// Session id.
    pub id: Uuid,
}

/// Create session request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    /// Bot username.
    pub username: String,
    /// Bot password.
    pub password: String,
    /// Optional label.
    pub label: Option<String>,
}

/// Register session request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterSessionRequest {
    /// Bot username.
    pub username: String,
    /// Empire.
    pub empire: String,
    /// Registration code.
    pub registration_code: String,
    /// Optional label.
    pub label: Option<String>,
}

/// Register session response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterSessionResponse {
    /// Session id.
    pub session_id: String,
    /// Generated password.
    pub password: String,
}

/// Request body for setting scripts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetScriptRequest {
    /// Raw DSL script.
    pub script: String,
}

/// Command ack response.
#[derive(Debug, Clone, Serialize)]
pub struct CommandAckResponse {
    /// Session id.
    pub session_id: String,
    /// Command name.
    pub command: String,
    /// Message.
    pub message: String,
}

/// Response body after setting scripts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetScriptResponse {
    /// Normalized script.
    pub normalized_script: String,
}

/// Runtime host snapshot DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostSnapshotDto {
    /// Halt flag.
    pub is_halted: bool,
    /// Active command flag.
    pub has_active_command: bool,
    /// Current script line.
    pub current_script_line: Option<usize>,
    /// Current script.
    pub current_script: Option<String>,
}

/// Runtime snapshot response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshotResponse {
    /// Session id.
    pub session_id: String,
    /// Host snapshot.
    pub snapshot: RuntimeHostSnapshotDto,
    /// Latest system.
    pub latest_system: Option<String>,
    /// Latest poi.
    pub latest_poi: Option<String>,
    /// Docked flag.
    pub docked: Option<bool>,
    /// Fuel.
    pub fuel: Option<i64>,
    /// Max fuel.
    pub max_fuel: Option<i64>,
    /// Credits.
    pub credits: Option<i64>,
    /// Last update timestamp.
    pub last_updated_utc: DateTime<Utc>,
}

/// Active route DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGoRouteDto {
    /// Target system.
    pub target: String,
    /// Hops.
    pub hops: Vec<String>,
    /// Total jumps.
    pub total_jumps: i32,
    /// Estimated fuel use.
    pub estimated_fuel_use: i32,
    /// Arrival time.
    pub arrival_time: Option<DateTime<Utc>>,
}

/// Runtime state response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStateResponse {
    /// Runtime state.
    pub state: Option<RuntimeGameStateDto>,
    /// Memory strings.
    pub memory: Vec<String>,
    /// Execution status lines.
    pub execution_status_lines: Vec<String>,
    /// Current control input.
    pub control_input: Option<String>,
    /// Current script line.
    pub current_script_line: Option<usize>,
    /// Script running flag.
    pub script_running: bool,
    /// Last generation prompt.
    pub last_generation_prompt: Option<String>,
    /// Current tick.
    pub current_tick: Option<i64>,
    /// Last transport update.
    pub last_space_molt_post_utc: Option<DateTime<Utc>>,
    /// Active route.
    pub active_route: Option<ActiveGoRouteDto>,
    /// Active override name.
    pub active_override_name: Option<String>,
}

/// Runtime game-state DTO contract.
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
    /// Storage credits.
    pub storage_credits: i64,
    /// Storage items.
    pub storage_items: HashMap<String, RuntimeItemStackDto>,
    /// Economy deals.
    pub economy_deals: Vec<RuntimeEconomyDealDto>,
    /// Own buy orders.
    pub own_buy_orders: Vec<RuntimeOpenOrderInfoDto>,
    /// Own sell orders.
    pub own_sell_orders: Vec<RuntimeOpenOrderInfoDto>,
    /// Player ship.
    pub ship: RuntimePlayerShipDto,
    /// Credits.
    pub credits: i64,
    /// Docked flag.
    pub docked: bool,
    /// Home base id.
    pub home_base: String,
    /// Shipyard showroom.
    pub shipyard_showroom: Vec<RuntimeShipyardShowroomEntryDto>,
    /// Shipyard listings.
    pub shipyard_listings: Vec<RuntimeShipyardListingEntryDto>,
    /// Ship catalog.
    pub ship_catalogue: RuntimeCatalogueDto,
    /// Owned ships.
    pub owned_ships: Vec<RuntimeOwnedShipInfoDto>,
    /// Available recipes.
    pub available_recipes: Vec<RuntimeCatalogueEntryDto>,
    /// Player skills.
    pub skills: HashMap<String, i64>,
    /// Active missions.
    pub active_missions: Vec<RuntimeMissionInfoDto>,
    /// Available missions.
    pub available_missions: Vec<RuntimeMissionInfoDto>,
    /// Notifications.
    pub notifications: Vec<RuntimeGameNotificationDto>,
    /// Chat messages.
    pub chat_messages: Vec<RuntimeGameChatMessageDto>,
    /// Current market.
    pub current_market: Option<RuntimeMarketStateDto>,
    /// Station context when docked.
    pub station: Option<RuntimeStationContextDto>,
}

/// Item stack DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeItemStackDto {
    /// Item id.
    pub item_id: String,
    /// Quantity.
    pub quantity: i64,
}

/// POI resource DTO.
#[derive(Debug, Clone, Serialize)]
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
    /// Resource details.
    pub resources: Vec<RuntimePoiResourceInfoDto>,
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
    /// Exploration state.
    pub exploration: RuntimeGalaxyExplorationDto,
    /// Last update timestamp.
    pub updated_at_utc: DateTime<Utc>,
}

/// Galaxy map snapshot DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyMapSnapshotDto {
    /// Systems.
    pub systems: Vec<RuntimeGalaxySystemInfoDto>,
    /// Known POIs.
    pub known_pois: Vec<RuntimeGalaxyKnownPoiInfoDto>,
}

/// Galaxy system info DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxySystemInfoDto {
    /// System id.
    pub id: String,
    /// Empire.
    pub empire: String,
    /// X coordinate.
    pub x: Option<f64>,
    /// Y coordinate.
    pub y: Option<f64>,
    /// Connections.
    pub connections: Vec<String>,
    /// System POIs.
    pub pois: Vec<RuntimeGalaxyPoiInfoDto>,
}

/// Galaxy POI info DTO.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
    /// Last seen.
    pub last_seen_utc: DateTime<Utc>,
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

/// Runtime galaxy catalog DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyCatalogDto {
    /// Items by id.
    pub items_by_id: HashMap<String, RuntimeItemCatalogueEntryDto>,
    /// Ships by id.
    pub ships_by_id: HashMap<String, RuntimeShipCatalogueEntryDto>,
    /// Recipes by id.
    pub recipes_by_id: HashMap<String, RuntimeRecipeEntryDto>,
}

/// Galaxy resources DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyResourcesDto {
    /// Systems by resource.
    pub systems_by_resource: HashMap<String, Vec<String>>,
    /// POIs by resource.
    pub pois_by_resource: HashMap<String, Vec<String>>,
}

/// Galaxy exploration DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGalaxyExplorationDto {
    /// Explored systems.
    pub explored_systems: Vec<String>,
    /// Visited POIs.
    pub visited_pois: Vec<String>,
    /// Surveyed systems.
    pub surveyed_systems: Vec<String>,
    /// Mining checked POIs by resource.
    pub mining_checked_pois_by_resource: HashMap<String, Vec<String>>,
    /// Mining explored systems by resource.
    pub mining_explored_systems_by_resource: HashMap<String, Vec<String>>,
}

/// Item catalog entry DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCatalogueEntryDto {
    /// Item id.
    pub id: String,
    /// Entry name.
    pub name: String,
    /// Class id.
    #[serde(rename = "class_id")]
    pub class_id: String,
    /// Class.
    #[serde(rename = "class")]
    pub class_name: String,
    /// Category.
    pub category: String,
    /// Type.
    #[serde(rename = "type")]
    pub type_name: String,
    /// Tier.
    pub tier: Option<i64>,
    /// Scale.
    pub scale: Option<i64>,
    /// Hull.
    pub hull: Option<i64>,
    /// Base hull.
    #[serde(rename = "base_hull")]
    pub base_hull: Option<i64>,
    /// Shield.
    pub shield: Option<i64>,
    /// Base shield.
    #[serde(rename = "base_shield")]
    pub base_shield: Option<i64>,
    /// Cargo.
    pub cargo: Option<i64>,
    /// Cargo capacity.
    #[serde(rename = "cargo_capacity")]
    pub cargo_capacity: Option<i64>,
    /// Speed.
    pub speed: Option<i64>,
    /// Base speed.
    #[serde(rename = "base_speed")]
    pub base_speed: Option<i64>,
    /// Price.
    pub price: Option<f64>,
    /// Materials by id.
    pub materials: Option<HashMap<String, i64>>,
    /// Ingredients.
    pub ingredients: Vec<RuntimeRecipeIngredientEntryDto>,
    /// Inputs.
    pub inputs: Vec<RuntimeRecipeIngredientEntryDto>,
    /// Outputs.
    pub outputs: Vec<RuntimeRecipeIngredientEntryDto>,
    /// Required skills.
    #[serde(rename = "required_skills")]
    pub required_skills: Option<HashMap<String, i64>>,
}

/// Ship catalog entry DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeItemCatalogueEntryDto {
    /// Item entry.
    #[serde(flatten)]
    pub entry: RuntimeCatalogueEntryDto,
}

/// Ship catalog entry DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeShipCatalogueEntryDto {
    /// Ship entry.
    #[serde(flatten)]
    pub entry: RuntimeCatalogueEntryDto,
}

/// Recipe entry DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRecipeEntryDto {
    /// Recipe id.
    pub id: String,
    /// Recipe name.
    pub name: String,
    /// Inputs.
    pub inputs: Vec<RuntimeRecipeIngredientEntryDto>,
    /// Outputs.
    pub outputs: Vec<RuntimeRecipeIngredientEntryDto>,
    /// Required skills.
    pub required_skills: Option<HashMap<String, i64>>,
}

/// Recipe ingredient DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRecipeIngredientEntryDto {
    /// Item id.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// Item alias.
    pub item: String,
    /// Identifier.
    pub id: String,
    /// Name.
    pub name: String,
    /// Quantity.
    pub quantity: Option<i64>,
    /// Amount.
    pub amount: Option<i64>,
    /// Count.
    pub count: Option<i64>,
}

/// Market state DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMarketStateDto {
    /// Station id.
    pub station_id: String,
    /// Sell orders.
    pub sell_orders: HashMap<String, Vec<RuntimeMarketOrderDto>>,
    /// Buy orders.
    pub buy_orders: HashMap<String, Vec<RuntimeMarketOrderDto>>,
}

/// Market order DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMarketOrderDto {
    /// Item id.
    pub item_id: String,
    /// Price each.
    pub price_each: f64,
    /// Quantity.
    pub quantity: i64,
}

/// Economy deal DTO.
#[derive(Debug, Clone, Serialize)]
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

/// Open-order info DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOpenOrderInfoDto {
    /// Order id.
    pub order_id: String,
    /// Item id.
    pub item_id: String,
    /// Price each.
    pub price_each: f64,
    /// Quantity.
    pub quantity: i64,
}

/// Player ship DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePlayerShipDto {
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
    /// Cargo stacks.
    pub cargo: HashMap<String, RuntimeItemStackDto>,
}

/// Owned ship DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOwnedShipInfoDto {
    /// Ship id.
    pub ship_id: String,
    /// Class id.
    pub class_id: String,
    /// Location.
    pub location: String,
    /// Active flag.
    pub is_active: bool,
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

/// Catalogue DTO.
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
    pub items: Vec<RuntimeCatalogueEntryDto>,
    /// Entries.
    pub entries: Vec<RuntimeCatalogueEntryDto>,
    /// Ships.
    pub ships: Vec<RuntimeCatalogueEntryDto>,
}

/// Mission info DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMissionInfoDto {
    /// Mission instance id.
    pub id: String,
    /// Mission id.
    pub mission_id: String,
    /// Template id.
    pub template_id: String,
    /// Title.
    pub title: String,
    /// Type.
    pub r#type: String,
    /// Description.
    pub description: String,
    /// Progress text.
    pub progress_text: String,
    /// Completion flag.
    pub completed: bool,
    /// Difficulty.
    pub difficulty: Option<i64>,
    /// Ticks until expiry.
    pub expires_in_ticks: Option<i64>,
    /// Accepted timestamp text.
    pub accepted_at: String,
    /// Issuing base.
    pub issuing_base: String,
    /// Issuing base id.
    pub issuing_base_id: String,
    /// Giver name.
    pub giver_name: String,
    /// Giver title.
    pub giver_title: String,
    /// Repeatable flag.
    pub repeatable: Option<bool>,
    /// Faction id.
    pub faction_id: String,
    /// Faction name.
    pub faction_name: String,
    /// Chain next id.
    pub chain_next: String,
    /// Objectives summary.
    pub objectives_summary: String,
    /// Progress summary.
    pub progress_summary: String,
    /// Requirements summary.
    pub requirements_summary: String,
    /// Rewards summary.
    pub rewards_summary: String,
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
    /// Storage credits.
    pub storage_credits: i64,
    /// Storage items.
    pub storage_items: HashMap<String, RuntimeItemStackDto>,
    /// Station market.
    pub market: Option<RuntimeMarketStateDto>,
    /// Shipyard showroom.
    pub shipyard_showroom: Vec<RuntimeShipyardShowroomEntryDto>,
    /// Shipyard listings.
    pub shipyard_listings: Vec<RuntimeShipyardListingEntryDto>,
    /// Craftable item ids.
    pub craftable: Vec<String>,
}

/// Station storage DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationStorageResponse {
    /// Storage credits.
    pub storage_credits: i64,
    /// Storage items.
    pub storage_items: HashMap<String, RuntimeItemStackDto>,
}

/// Station shipyard DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationShipyardResponse {
    /// Shipyard showroom.
    pub shipyard_showroom: Vec<RuntimeShipyardShowroomEntryDto>,
    /// Shipyard listings.
    pub shipyard_listings: Vec<RuntimeShipyardListingEntryDto>,
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
}

/// Set skill library request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSkillLibraryRequest {
    /// Raw library text.
    pub text: String,
}

/// Canonical skill-library text response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLibraryTextResponse {
    /// Canonicalized skill library text.
    pub text: String,
}

/// Spacemolt passthrough request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceMoltPassthroughRequest {
    /// Command name.
    pub command: String,
    /// Optional payload.
    pub payload: Option<serde_json::Value>,
}

/// Spacemolt passthrough response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceMoltPassthroughResponse {
    /// Success flag.
    pub succeeded: bool,
    /// Raw result.
    pub result: serde_json::Value,
    /// Optional error.
    pub error: Option<String>,
}

/// Request body for checkpoint restore.
#[derive(Debug, Clone, Deserialize)]
pub struct RestoreCheckpointRequest {
    /// Checkpoint to restore.
    pub checkpoint: EngineCheckpoint,
}

/// Request body for halt/resume operations.
#[derive(Debug, Clone, Deserialize)]
pub struct ReasonRequest {
    /// Optional reason message.
    pub reason: Option<String>,
}

/// Request body for runtime transport selection.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetTransportRequest {
    /// In-memory mock transport for local testing.
    Mock {
        /// Static state payload.
        state: Option<Box<GameState>>,
        /// Optional static command responses.
        responses: Option<HashMap<String, EngineExecutionResult>>,
    },
    /// SpaceMolt HTTP transport.
    SpaceMolt {
        /// Base URL for SpaceMolt runtime endpoints.
        base_url: String,
        /// Bearer token for SpaceMolt runtime endpoints.
        token: String,
    },
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
    /// Omitted when docking state changed (stash visibility is unreliable across dock/undock).
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
