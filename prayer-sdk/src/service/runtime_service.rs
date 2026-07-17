use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use parking_lot::{Mutex as ParkingMutex, RwLock};
use prayer_runtime::economy::quartermaster::{
    FacilitySnapshot, FacilitySnapshotSource, QuartermasterPlanningSource,
};
use prayer_runtime::engine::{
    AgentSightingData, CatalogData, EngineError, EngineExecutionResult, FactionGarageInfo,
    GalaxyData, MarketData, MarketOrder, RuntimeEngine, RuntimeEvent, RuntimeSnapshot, SalvageData,
};
use prayer_runtime::navigation::{active_command_navigation_target, nearest_station_poi};
use prayer_runtime::operation_failure::OperationFailure;
use prayer_runtime::orchestration::{ApiOutcome, CommandPlanner, RuntimeOperation};
use prayer_runtime::snapshot::StateObservation;
use prayer_runtime::PersistedExecutionRun;
use prayer_scheduler::QueueOwner;
use prayer_state::{BotId, BotState};
#[cfg(test)]
use prayer_state::{StationMarketData, WildlifePoiSnapshotData};
use serde_json::Value;
use spacemolt_lib_rs::commands::{SpacemoltFactionInfoParams, SpacemoltSocialGetChatHistoryParams};
use spacemolt_lib_rs::{Account, ConnectOwnedOptions, SpacemoltClient};
use tokio::sync::{watch, Mutex, Notify};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::spacemolt_origin::DEFAULT_SPACEMOLT_ORIGIN;
use crate::state_mapping::{
    map_actor_active_commissions, map_actor_owned_ships, map_commander_session_state,
    map_faction_garage_value, map_focused_station_context, map_item_quantities, map_social_bots,
};
use crate::{
    ActiveGoRouteDto, CommanderFleetResponse, CommanderSessionStateResponse,
    CommanderStateResponse, CommanderStorageResponse, CommanderStorageRowDto,
    CommanderWorldStateResponse, ExecuteScriptResponse, FacilitiesSnapshotResponse,
    GameChatChannelSummaryDto, GameChatMessageDto, GameChatResponse, RuntimeActiveFrameDto,
    RuntimeFactionGarageShipProjectionDto, RuntimeHostSnapshotDto, RuntimeOwnedShipProjectionDto,
    RuntimeSnapshotResponse, RuntimeStationContextDto, RuntimeVirtualCraftOrderDto,
    RuntimeVirtualMarketOrderDto, RuntimeVirtualOrderReservationResultDto,
    RuntimeVirtualOrderUseDto, ScriptDiff, ScriptDiffFlags, ScriptErrorKindDto, ScriptExecutionDto,
    ScriptExecutionStateDto, ScriptLocationDelta, ScriptOutcomeDto, ScriptRunnerDto, SdkError,
    SessionSummary, SocialBotDto, SocialResponse, StationShipyardResponse, StationStorageResponse,
    StepResponse,
};

mod accounts;
mod execution;
mod facilities;
#[path = "../knowledge/mod.rs"]
mod knowledge;
#[path = "persistence.rs"]
mod persistence;
mod refresh;
#[path = "sessions.rs"]
mod sessions;

use self::knowledge::*;
use self::persistence::*;
pub use self::sessions::*;

const MAX_STATUS_LINES: usize = 64;
const KNOWLEDGE_SCHEMA_VERSION: u32 = 5;
const SESSION_SCHEMA_VERSION: u32 = 3;
const FILE_LOCK_TIMEOUT_MS: u64 = 2_000;
const FILE_LOCK_STALE_SECS: u64 = 120;
const IDLE_SESSION_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const MOBILE_BASE_POI_ID: &str = "mobile_capital";
const MOBILE_BASE_STATION_ID: &str = "frontier_station";
const LEGACY_MOBILE_BASE_STATION_ID: &str = "mobile_base";
const MOBILE_BASE_NAME: &str = "Frontier Mobile Base";
const MOBILE_BASE_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

fn queue_owner_run_id(owner: &QueueOwner) -> prayer_actions::RunId {
    match owner {
        QueueOwner::PrayerLang { run_id }
        | QueueOwner::Manual { run_id }
        | QueueOwner::Controller { run_id, .. } => run_id.clone(),
    }
}
const FACILITY_POI_SNAPSHOT_TTL_SECS: i64 = 24 * 60 * 60;
const TAX_ESTIMATE_TTL_SECS: u64 = 180;
/// How often an otherwise-unchanged agent sighting is re-stamped (and the
/// knowledge cache re-persisted). A bot parked next to another player would
/// otherwise rewrite the cache on every refresh.

#[derive(Debug, Clone)]
struct StorageRowAccumulator {
    item_id: String,
    quantity: i64,
    source_kind: String,
    owner_id: Option<String>,
    owner_name: String,
    location_id: String,
    location_name: Option<String>,
    system_id: Option<String>,
    observed_by: Vec<String>,
    state_version: u64,
    details: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
struct StorageMarketPrice {
    median_buy_price: Option<f64>,
    median_sell_price: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedTaxEstimate {
    fetched_at: Instant,
    value: spacemolt_lib_rs::schema::TaxEstimateResponse,
}

#[derive(Debug, Clone)]
struct FactionTreasurySnapshot {
    faction_name: String,
    treasury: i64,
}

fn storage_row_key(source_kind: &str, owner_key: &str, location_id: &str, item_id: &str) -> String {
    format!("{source_kind}:{owner_key}:{location_id}:{item_id}")
}

fn storage_market_prices(market: &MarketData) -> HashMap<String, StorageMarketPrice> {
    let aggregates = market.global_price_aggregates();
    let mut prices = HashMap::new();

    for item_id in aggregates
        .median_buy_prices
        .keys()
        .chain(aggregates.median_sell_prices.keys())
    {
        if prices.contains_key(item_id) {
            continue;
        }
        let median_buy_price = aggregates
            .median_buy_prices
            .get(item_id)
            .copied()
            .filter(|price| price.is_finite() && *price > 0.0);
        let median_sell_price = aggregates
            .median_sell_prices
            .get(item_id)
            .copied()
            .filter(|price| price.is_finite() && *price > 0.0);
        if median_buy_price.is_some() || median_sell_price.is_some() {
            prices.insert(
                item_id.clone(),
                StorageMarketPrice {
                    median_buy_price,
                    median_sell_price,
                },
            );
        }
    }

    prices.insert(
        "credits".to_string(),
        StorageMarketPrice {
            median_buy_price: Some(1.0),
            median_sell_price: Some(1.0),
        },
    );
    prices.insert(
        "tax_estimate".to_string(),
        StorageMarketPrice {
            median_buy_price: Some(1.0),
            median_sell_price: Some(1.0),
        },
    );

    prices
}

fn storage_location_jumps(
    actor: &BotState,
    galaxy: &GalaxyData,
    system_id: Option<&str>,
) -> Option<i64> {
    let current_system = actor.location.system_id.as_deref()?.trim();
    let target_system = system_id?.trim();
    if current_system.is_empty() || target_system.is_empty() {
        return None;
    }
    galaxy
        .hop_distance(current_system, target_system)
        .and_then(|distance| i64::try_from(distance).ok())
}

fn insert_storage_row(
    rows_by_key: &mut HashMap<String, StorageRowAccumulator>,
    mut row: StorageRowAccumulator,
) {
    if row.quantity <= 0 && row.source_kind != "financial" {
        return;
    }
    row.observed_by.sort();
    row.observed_by.dedup();
    let key = storage_row_key(
        &row.source_kind,
        row.owner_id.as_deref().unwrap_or(&row.owner_name),
        &row.location_id,
        &row.item_id,
    );
    let Some(existing) = rows_by_key.get_mut(&key) else {
        rows_by_key.insert(key, row);
        return;
    };

    for observer in row.observed_by {
        if !existing.observed_by.iter().any(|known| known == &observer) {
            existing.observed_by.push(observer);
        }
    }
    existing.observed_by.sort();
    existing.observed_by.dedup();

    if row.state_version >= existing.state_version {
        existing.quantity = row.quantity;
        existing.owner_name = row.owner_name;
        existing.location_name = row.location_name;
        existing.system_id = row.system_id;
        existing.state_version = row.state_version;
        existing.details = row.details;
    }
}

async fn fetch_faction_treasury_snapshot(
    service: &RuntimeService,
    session_id: Uuid,
    faction_id: &str,
) -> Result<Option<FactionTreasurySnapshot>, SdkError> {
    let response = service
        .spacemolt_account(session_id)
        .await?
        .commands()
        .spacemolt_faction()
        .info(Some(SpacemoltFactionInfoParams {
            limit: None,
            offset: None,
            id: Some(faction_id.to_string()),
        }))
        .await
        .map_err(SdkError::from)?
        .into_typed()
        .map_err(SdkError::from)?;
    let Some(treasury) = response.treasury else {
        return Ok(None);
    };
    let faction_name = if response.name.trim().is_empty() {
        faction_id.to_string()
    } else {
        response.name
    };
    Ok(Some(FactionTreasurySnapshot {
        faction_name,
        treasury,
    }))
}

fn tax_estimate_net_owed(tax: &spacemolt_lib_rs::schema::TaxEstimateResponse) -> i64 {
    tax.income_tax_total
        .saturating_add(tax.property_tax_total)
        .saturating_sub(tax.tax_prepaid)
        .max(0)
}

fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

async fn await_with_halt<T, F>(
    halt_rx: &mut Option<&mut watch::Receiver<bool>>,
    fut: F,
) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    let Some(rx) = halt_rx.as_deref_mut() else {
        return Ok(fut.await);
    };
    if *rx.borrow() {
        return Err(());
    }

    tokio::pin!(fut);
    let mut watch_closed = false;
    loop {
        tokio::select! {
            result = &mut fut => return Ok(result),
            changed = rx.changed(), if !watch_closed => {
                match changed {
                    Ok(()) if *rx.borrow() => return Err(()),
                    Ok(()) => {}
                    Err(_) => watch_closed = true,
                }
            }
        }
    }
}

fn halted_step_response() -> StepResponse {
    StepResponse {
        executed: false,
        command_action: None,
        command_args: None,
        result_message: Some("halt requested".to_string()),
        halted: true,
        paused: false,
        resume_after_ms: None,
        error: None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeStepMode {
    Script,
    CombatInterrupt,
}

fn json_preview(value: &Option<Value>) -> String {
    const MAX_CHARS: usize = 400;
    let Some(value) = value else {
        return "null".to_string();
    };
    let text = value.to_string();
    if text.chars().count() <= MAX_CHARS {
        return text;
    }
    let mut preview = text.chars().take(MAX_CHARS).collect::<String>();
    preview.push_str("...");
    preview
}

fn faction_storage_items_from_view_response(
    value: &spacemolt_lib_rs::schema::StorageResponse,
) -> HashMap<String, i64> {
    let mut totals = HashMap::new();
    if let spacemolt_lib_rs::schema::StorageResponse::ViewFactionStorageResponse(
        spacemolt_lib_rs::schema::ViewFactionStorageResponse { items, buckets, .. },
    ) = value
    {
        for item in items {
            if item.quantity > 0 {
                *totals.entry(item.item_id.clone()).or_default() += item.quantity;
            }
        }
        for item in buckets.iter().flat_map(|bucket| &bucket.items) {
            if item.quantity > 0 {
                *totals.entry(item.item_id.clone()).or_default() += item.quantity;
            }
        }
    }
    totals
}

fn synthetic_social_bots_from_state(
    state: &BotState,
    session_label: &str,
    observed_at: DateTime<Utc>,
) -> Vec<SocialBotDto> {
    let mut bots = Vec::new();
    let system = state
        .location
        .system_id
        .as_deref()
        .or(state.location.system_name.as_deref())
        .unwrap_or("unknown")
        .to_string();
    let place = state
        .location
        .poi_name
        .as_deref()
        .or(state.location.poi_id.as_deref())
        .unwrap_or("current location");

    append_synthetic_social_bots(
        &mut bots,
        &state.location.nearby_pirates,
        "pirate",
        "synthetic:pirates",
        Some("PIRATE"),
        "Pirate",
        &system,
        place,
        session_label,
        observed_at,
    );
    append_empire_social_bots(
        &mut bots,
        &state.location.nearby_empire_npcs,
        state
            .location
            .empire
            .as_deref()
            .or(state.player.empire.as_deref()),
        &system,
        place,
        session_label,
        observed_at,
    );
    bots
}

#[allow(clippy::too_many_arguments)]
fn append_synthetic_social_bots(
    bots: &mut Vec<SocialBotDto>,
    items: &[spacemolt_lib_rs::schema::V2GameStateLocationNearbyPiratesItem],
    actor_kind: &str,
    faction_id: &str,
    faction_tag: Option<&str>,
    default_name: &str,
    system: &str,
    place: &str,
    session_label: &str,
    observed_at: DateTime<Utc>,
) {
    for (index, item) in items.iter().enumerate() {
        let id = item
            .pirate_id
            .clone()
            .unwrap_or_else(|| format!("{actor_kind}:{system}:{index}"));
        let username = item
            .name
            .clone()
            .unwrap_or_else(|| format!("{default_name} {}", index + 1));
        bots.push(SocialBotDto {
            actor_kind: actor_kind.to_string(),
            synthetic: true,
            player_id: Some(id),
            username,
            faction_id: Some(faction_id.to_string()),
            faction_tag: faction_tag.map(ToOwned::to_owned),
            clan_tag: None,
            ship_class: None,
            ship_name: item.name.clone(),
            status_message: Some(format!("Seen by {session_label} at {place}")),
            primary_color: synthetic_actor_color(actor_kind).map(str::to_string),
            secondary_color: None,
            in_combat: false,
            offline: false,
            last_seen_system: system.to_string(),
            first_seen_utc: observed_at,
            last_seen_utc: observed_at,
            times_seen: 1,
        });
    }
}

fn append_empire_social_bots(
    bots: &mut Vec<SocialBotDto>,
    items: &[spacemolt_lib_rs::schema::V2GameStateLocationNearbyEmpireNpcsItem],
    fallback_empire: Option<&str>,
    system: &str,
    place: &str,
    session_label: &str,
    observed_at: DateTime<Utc>,
) {
    for (index, item) in items.iter().enumerate() {
        let empire = item
            .empire
            .clone()
            .or_else(|| fallback_empire.map(ToOwned::to_owned))
            .unwrap_or_else(|| "empire".to_string());
        let faction_id = format!("synthetic:empire:{empire}");
        let tag = empire_tag(&empire);
        bots.push(SocialBotDto {
            actor_kind: "empire".to_string(),
            synthetic: true,
            player_id: item
                .npc_id
                .clone()
                .or_else(|| Some(format!("empire:{empire}:{system}:{index}"))),
            username: item
                .name
                .clone()
                .unwrap_or_else(|| format!("Empire NPC {}", index + 1)),
            faction_id: Some(faction_id),
            faction_tag: Some(tag),
            clan_tag: None,
            ship_class: item.ship_class.clone(),
            ship_name: item.ship_name.clone(),
            status_message: Some(format!("Seen by {session_label} at {place}")),
            primary_color: synthetic_actor_color("empire").map(str::to_string),
            secondary_color: None,
            in_combat: item.in_combat.unwrap_or(false),
            offline: false,
            last_seen_system: system.to_string(),
            first_seen_utc: observed_at,
            last_seen_utc: observed_at,
            times_seen: 1,
        });
    }
}

fn normalize_chat_message(
    raw: spacemolt_lib_rs::schema::ChatHistoryMessage,
    default_system: Option<&str>,
    default_poi: Option<&str>,
) -> GameChatMessageDto {
    let timestamp_utc = DateTime::parse_from_rfc3339(&raw.timestamp_utc)
        .ok()
        .map(|dt| dt.with_timezone(&Utc));
    let system_id = raw.system_id.or_else(|| {
        default_system
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
    });
    let poi_id = raw.poi_id.or_else(|| {
        default_poi
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
    });
    GameChatMessageDto {
        id: raw.id,
        channel: raw.channel,
        sender_id: Some(raw.sender_id),
        sender: raw.sender,
        content: raw.content,
        timestamp_utc,
        system_id,
        poi_id,
        faction_id: raw.faction_id,
        target_id: raw.target_id,
        target_name: raw.target_name,
        empire_official: raw.empire_official.unwrap_or(false),
    }
}

fn synthetic_actor_color(actor_kind: &str) -> Option<&'static str> {
    match actor_kind {
        "pirate" => Some("#b45309"),
        "empire" => Some("#2563eb"),
        _ => None,
    }
}

fn empire_tag(empire: &str) -> String {
    let normalized = empire
        .split(['_', '-', ' '])
        .filter_map(|part| part.chars().next())
        .collect::<String>();
    let tag = if normalized.is_empty() {
        empire.chars().take(3).collect::<String>()
    } else {
        normalized
    };
    tag.to_ascii_uppercase()
}

#[derive(Debug, Clone)]
pub struct RuntimeServiceOptions {
    pub knowledge_state_path: PathBuf,
    pub session_state_path: PathBuf,
    pub local_auth_bypass: bool,
    pub tax_estimate_ttl: Duration,
    pub script_wait_override: Option<Duration>,
    pub memory_size_breakdown: bool,
}

impl Default for RuntimeServiceOptions {
    fn default() -> Self {
        Self {
            knowledge_state_path: knowledge_state_path(),
            session_state_path: session_state_path(),
            local_auth_bypass: cfg!(test),
            tax_estimate_ttl: Duration::from_secs(TAX_ESTIMATE_TTL_SECS),
            script_wait_override: None,
            memory_size_breakdown: false,
        }
    }
}

/// Central runtime session service.
pub struct RuntimeService {
    pub options: RuntimeServiceOptions,
    shutting_down: AtomicBool,
    shutdown_notify: Notify,
    background_workers: ParkingMutex<Vec<tokio::task::JoinHandle<()>>>,
    sessions: RwLock<HashMap<Uuid, Arc<Mutex<SessionHandle>>>>,
    commander_state_sequence: AtomicU64,
    roster_sequence: AtomicU64,
    session_change_sequences: ParkingMutex<HashMap<Uuid, u64>>,
    session_tombstones: ParkingMutex<Vec<(u64, String)>>,
    session_labels: RwLock<HashMap<String, Uuid>>,
    session_summary_cache: ParkingMutex<HashMap<Uuid, SessionSummary>>,
    active_script_runs: Arc<ParkingMutex<HashMap<Uuid, ScriptRunInfo>>>,
    action_run_history:
        ParkingMutex<HashMap<(Uuid, String), prayer_runtime::execution::PersistedActionRun>>,
    spacemolt_client: Arc<SpacemoltClient>,
    spacemolt_base_url: String,
    canonical_catalog_gate: Mutex<()>,
    canonical_catalog_loaded: AtomicBool,
    canonical_map_gate: Mutex<()>,
    canonical_map_loaded: AtomicBool,
    knowledge_state: KnowledgeStore,
    knowledge_metadata: RwLock<prayer_runtime::knowledge::WorldRuntimeMetadata>,
    market_watchers: ParkingMutex<HashMap<String, Uuid>>,
    observation_watchers_by_poi: ParkingMutex<HashMap<String, Uuid>>,
    faction_storage_watchers_by_key: ParkingMutex<HashMap<String, Uuid>>,
    faction_garage_watchers_by_key: ParkingMutex<HashMap<String, Uuid>>,
    inventory_reservations: ParkingMutex<InventoryReservationLedger>,
    inventory_reservation_gate: ParkingMutex<()>,
    knowledge_persistence: KnowledgePersistence,
    session_state_path: PathBuf,
    persistence_telemetry: Arc<PersistenceTelemetry>,
    account_state_tx: tokio::sync::mpsc::UnboundedSender<Uuid>,
    account_state_rx: ParkingMutex<Option<tokio::sync::mpsc::UnboundedReceiver<Uuid>>>,
}

impl Default for RuntimeService {
    fn default() -> Self {
        Self::with_spacemolt_client(
            Arc::new(SpacemoltClient::default()),
            DEFAULT_SPACEMOLT_ORIGIN.to_string(),
            RuntimeServiceOptions::default(),
        )
    }
}

impl RuntimeService {
    /// Construct the service around a client configured by the embedding host.
    pub fn with_spacemolt_client(
        spacemolt_client: Arc<SpacemoltClient>,
        spacemolt_base_url: String,
        options: RuntimeServiceOptions,
    ) -> Self {
        let persistence_telemetry = Arc::new(PersistenceTelemetry::default());
        let knowledge_state_path = options.knowledge_state_path.clone();
        let knowledge_state = match load_knowledge_state(&knowledge_state_path) {
            Ok(v) => v,
            Err(err) => {
                let failures = persistence_telemetry
                    .load_failures
                    .fetch_add(1, Ordering::Relaxed)
                    + 1;
                warn!(
                    path = %knowledge_state_path.display(),
                    failures,
                    error = %err,
                    "failed to load knowledge cache; starting with empty knowledge"
                );
                WorldState::default()
            }
        };
        // Warm the all-pairs route table from the persisted jump graph so
        // the first routing query doesn't pay the build.
        knowledge_state.galaxy.precompute_routes();
        let knowledge_persistence = KnowledgePersistence::start(
            knowledge_state_path,
            Arc::clone(&persistence_telemetry),
            Some(knowledge_state.knowledge_version),
        );
        let session_state_path = options.session_state_path.clone();
        let (account_state_tx, account_state_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            options,
            shutting_down: AtomicBool::new(false),
            shutdown_notify: Notify::new(),
            background_workers: ParkingMutex::new(Vec::new()),
            sessions: RwLock::new(HashMap::new()),
            commander_state_sequence: AtomicU64::new(1),
            roster_sequence: AtomicU64::new(1),
            session_change_sequences: ParkingMutex::new(HashMap::new()),
            session_tombstones: ParkingMutex::new(Vec::new()),
            session_labels: RwLock::new(HashMap::new()),
            session_summary_cache: ParkingMutex::new(HashMap::new()),
            active_script_runs: Arc::new(ParkingMutex::new(HashMap::new())),
            action_run_history: ParkingMutex::new(HashMap::new()),
            spacemolt_client,
            spacemolt_base_url,
            canonical_catalog_gate: Mutex::new(()),
            canonical_catalog_loaded: AtomicBool::new(false),
            canonical_map_gate: Mutex::new(()),
            canonical_map_loaded: AtomicBool::new(false),
            knowledge_state: KnowledgeStore::new(knowledge_state),
            knowledge_metadata: RwLock::new(Default::default()),
            market_watchers: ParkingMutex::new(HashMap::new()),
            observation_watchers_by_poi: ParkingMutex::new(HashMap::new()),
            faction_storage_watchers_by_key: ParkingMutex::new(HashMap::new()),
            faction_garage_watchers_by_key: ParkingMutex::new(HashMap::new()),
            inventory_reservations: ParkingMutex::new(InventoryReservationLedger::default()),
            inventory_reservation_gate: ParkingMutex::new(()),
            knowledge_persistence,
            session_state_path,
            persistence_telemetry,
            account_state_tx,
            account_state_rx: ParkingMutex::new(Some(account_state_rx)),
        }
    }
}

impl RuntimeService {
    pub fn request_shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
        self.shutdown_notify.notify_waiters();
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    pub fn spawn_background(&self, future: impl Future<Output = ()> + Send + 'static) {
        if self.is_shutting_down() {
            return;
        }
        let worker = tokio::spawn(future);
        let mut workers = self.background_workers.lock();
        if self.is_shutting_down() {
            worker.abort();
        } else {
            workers.push(worker);
        }
    }

    pub async fn stop_background_workers(&self) {
        self.request_shutdown();
        let workers = std::mem::take(&mut *self.background_workers.lock());
        for worker in workers {
            worker.abort();
            let _ = worker.await;
        }
    }

    pub async fn shutdown_requested(&self) {
        if !self.is_shutting_down() {
            self.shutdown_notify.notified().await;
        }
    }

    /// Create service instance.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn script_wait_delay(&self, resume_after_ms: Option<u64>) -> Duration {
        self.options
            .script_wait_override
            .unwrap_or_else(|| Duration::from_millis(resume_after_ms.unwrap_or(0)))
    }

    /// Refresh the shared faction treasury balance for `faction_id` using the
    /// designated faction-storage watcher's account. Written straight into the
    /// process-local knowledge map so `commander_storage_snapshot` can read it
    /// without a live game-API round-trip. Deliberately does not bump the
    /// knowledge version or publish: treasury is not part of
    /// `world_knowledge_persisted_eq`, and the storage tab re-polls on its own.
    pub async fn refresh_watched_faction_treasury(&self, id: Uuid, faction_id: &str) {
        // Treasury is faction-wide: one fetch per faction per interval is enough,
        // even when several storage-POI watchers of the same faction refresh.
        if self
            .knowledge_metadata
            .read()
            .faction_treasury_fetched_at_by_key
            .get(faction_id)
            .is_some_and(|fetched_at| fetched_at.elapsed() < IDLE_SESSION_REFRESH_INTERVAL)
        {
            return;
        }
        match fetch_faction_treasury_snapshot(self, id, faction_id).await {
            Ok(Some(snapshot)) => {
                let mut knowledge = self.knowledge_state.write();
                knowledge.faction_treasury_by_faction.insert(
                    faction_id.to_string(),
                    FactionTreasuryInfo {
                        faction_name: snapshot.faction_name,
                        treasury: snapshot.treasury,
                    },
                );
                self.knowledge_metadata
                    .write()
                    .faction_treasury_fetched_at_by_key
                    .insert(faction_id.to_string(), Instant::now());
            }
            Ok(None) => {}
            Err(err) => {
                debug!(%id, faction_id, error = %err, "faction treasury refresh unavailable");
            }
        }
    }

    pub fn note_session_changed(&self, id: Uuid) -> u64 {
        let sequence = self.commander_state_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        self.session_change_sequences.lock().insert(id, sequence);
        sequence
    }

    pub fn note_roster_changed(&self, id: Uuid) -> (u64, u64) {
        let roster = self.roster_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let state = self.note_session_changed(id);
        (roster, state)
    }

    #[cfg(test)]
    pub fn note_session_removed(&self, id: Uuid, handle: String) {
        self.roster_sequence.fetch_add(1, Ordering::AcqRel);
        let state = self.commander_state_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        self.session_change_sequences.lock().remove(&id);
        let mut tombstones = self.session_tombstones.lock();
        tombstones.push((state, handle));
        if tombstones.len() > 4096 {
            let remove = tombstones.len() - 4096;
            tombstones.drain(..remove);
        }
    }

    pub fn roster_version(&self) -> u64 {
        self.roster_sequence.load(Ordering::Acquire)
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
