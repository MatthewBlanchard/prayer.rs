use super::sessions::SessionHandle;
use super::*;

pub mod inventory;
pub use inventory::InventoryReservationLedger;
mod model;
mod store;

pub use model::{FactionTreasuryInfo, WorldState};
pub use prayer_state::PoiFacilitiesSnapshot;
pub use store::KnowledgeStore;

impl FacilitySnapshotSource for RuntimeService {
    fn facility_snapshot(&self, poi_id: &str) -> Option<FacilitySnapshot> {
        self.knowledge_state.read().facility_snapshot(poi_id)
    }
}

impl QuartermasterPlanningSource for RuntimeService {
    fn faction_storage_quantity(
        &self,
        state: &prayer_runtime::economy::EconomyReadState,
        station_id: &str,
        item_id: &str,
    ) -> i64 {
        self.faction_storage_quantity_at_station(state, station_id, item_id)
    }
}
mod metrics;

use metrics::{serialized_len, WorldKnowledgeByteBreakdown, WorldKnowledgeCounts};

pub fn normalized_state_id(value: Option<&String>) -> Option<String> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn current_poi_key(session: &SessionHandle) -> Option<String> {
    normalized_state_id(session.actor.observed.location.poi_id.as_ref())
}

pub fn docked_station_key(session: &SessionHandle) -> Option<String> {
    session
        .actor
        .observed
        .location
        .docked_at
        .is_some()
        .then(|| current_poi_key(session))
        .flatten()
}

pub fn faction_station_storage_key(faction_id: &str, station_id: &str) -> String {
    format!("{faction_id}@{station_id}")
}

pub fn player_station_storage_key(player_id: &str, station_id: &str) -> String {
    format!("{player_id}@{station_id}")
}

pub fn current_faction_station_storage_key(session: &SessionHandle) -> Option<String> {
    let station_id = docked_station_key(session)?;
    let faction_id = session
        .actor
        .observed
        .player
        .faction_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(faction_station_storage_key(faction_id, &station_id))
}

pub fn current_faction_key(session: &SessionHandle) -> Option<String> {
    session
        .actor
        .observed
        .player
        .faction_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            session
                .actor
                .observed
                .player
                .clan_tag
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .map(str::to_string)
}

pub fn random_watcher_candidate(candidates: &[Uuid]) -> Option<Uuid> {
    if candidates.is_empty() {
        return None;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as usize)
        .unwrap_or(0);
    Some(candidates[nanos % candidates.len()])
}

pub fn reconcile_watchers_for_key(
    watchers: &mut HashMap<String, Uuid>,
    candidates_by_key: HashMap<String, Vec<Uuid>>,
) {
    watchers.retain(|key, watcher| {
        candidates_by_key
            .get(key)
            .is_some_and(|candidates| candidates.contains(watcher))
    });
    for (key, candidates) in candidates_by_key {
        if !watchers.contains_key(&key) {
            if let Some(candidate) = random_watcher_candidate(&candidates) {
                watchers.insert(key, candidate);
            }
        }
    }
}

pub fn mission_related_api_action(tool: &str, action: &str) -> bool {
    tool.eq_ignore_ascii_case("spacemolt")
        && matches!(
            action.to_ascii_lowercase().as_str(),
            "accept_mission" | "abandon_mission" | "decline_mission" | "complete_mission"
        )
}

pub fn crafting_queue_related_api_action(tool: &str, action: &str) -> bool {
    tool.eq_ignore_ascii_case("spacemolt") && action.eq_ignore_ascii_case("craft")
}

pub fn passenger_related_api_action(tool: &str, action: &str) -> bool {
    tool.eq_ignore_ascii_case("spacemolt")
        && matches!(
            action.to_ascii_lowercase().as_str(),
            "load_passenger" | "unload_passenger"
        )
}

pub fn market_related_api_action(tool: &str, action: &str) -> bool {
    tool.eq_ignore_ascii_case("spacemolt_market")
        && matches!(
            action.to_ascii_lowercase().as_str(),
            "cancel_order" | "create_buy_order" | "create_sell_order" | "modify_order"
        )
}

pub fn commission_related_api_action(tool: &str, action: &str) -> bool {
    tool.eq_ignore_ascii_case("spacemolt_ship")
        && matches!(
            action.to_ascii_lowercase().as_str(),
            "commission_ship" | "cancel_commission" | "supply_commission"
        )
}

pub fn switch_ship_related_api_action(
    command: &prayer_actions::ResolvedAction,
    tool: &str,
    action: &str,
) -> bool {
    command.action.eq_ignore_ascii_case("switch_ship")
        && tool.eq_ignore_ascii_case("spacemolt_ship")
        && action.eq_ignore_ascii_case("switch_ship")
}

pub fn switch_ship_already_active_error(error: &OperationFailure) -> bool {
    error.server_code() == Some("already_active")
}

pub fn unload_all_no_passengers_error(
    tool: &str,
    action: &str,
    payload: Option<&Value>,
    error: &OperationFailure,
) -> bool {
    if !tool.eq_ignore_ascii_case("spacemolt") || !action.eq_ignore_ascii_case("unload_passenger") {
        return false;
    }
    let Some(name) = payload
        .and_then(|payload| payload.get("name"))
        .and_then(Value::as_str)
    else {
        return false;
    };
    name.eq_ignore_ascii_case("all") && error.server_code() == Some("no_passengers")
}

pub fn garage_related_passthrough_action(
    tool: &str,
    action: &str,
    payload: Option<&Value>,
) -> bool {
    let tool = tool.to_ascii_lowercase();
    let action = action.to_ascii_lowercase();
    if tool == "spacemolt_ship" && matches!(action.as_str(), "switch_ship" | "list_ships") {
        return true;
    }
    if tool == "spacemolt_fleet" && action == "board" {
        return payload
            .and_then(|payload| payload.get("garage"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
    }
    if tool == "spacemolt_storage" && action == "deposit" {
        return payload
            .and_then(|payload| payload.get("target"))
            .and_then(Value::as_str)
            .is_some_and(|target| target.eq_ignore_ascii_case("faction"));
    }
    false
}

pub fn script_error_may_be_stale_identity(error: &EngineError) -> bool {
    matches!(error, EngineError::Parse(message) if message.contains("unknown identifier"))
}

pub fn craft_enqueue_response_text(value: &Value) -> Option<String> {
    let text = value
        .get("result")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("result")
                .and_then(|result| {
                    result
                        .get("message")
                        .or_else(|| result.get("status"))
                        .or_else(|| result.get("summary"))
                })
                .and_then(Value::as_str)
        })
        .or_else(|| value.as_str())?;
    text.contains("Crafting queued").then(|| text.to_string())
}

pub fn craft_enqueue_job_id(text: &str) -> Option<String> {
    let start = text.find("(job ")? + "(job ".len();
    let rest = &text[start..];
    let end = rest.find(')')?;
    let job_id = rest[..end].trim();
    (!job_id.is_empty()).then(|| job_id.to_string())
}

pub fn preserve_craft_enqueue_as_queue(state: &mut BotState, message: &str) {
    if !state.crafting_queue.is_empty() {
        return;
    }
    state.crafting_queue = Arc::new(vec![prayer_state::CraftingQueueProjection {
        job_id: craft_enqueue_job_id(message),
        raw_text: Some(message.to_string()),
        source: Some("craft_enqueue".to_string()),
        ..Default::default()
    }]);
}

pub fn facility_mutating_api_action(tool: &str, action: &str) -> bool {
    tool.eq_ignore_ascii_case("spacemolt_facility")
        && matches!(
            action.to_ascii_lowercase().as_str(),
            "build"
                | "faction_build"
                | "upgrade"
                | "faction_upgrade"
                | "dismantle"
                | "faction_dismantle"
                | "set_access"
                | "set_output_price"
                | "set_name"
        )
}

pub fn clear_ship_and_garage_cache(
    state: &mut BotState,
    world: &mut prayer_runtime::read_context::WorldReadState,
) {
    state.owned_ship_details = Arc::new(Vec::new());
    world.faction_garage = Default::default();
}

mod catalog;

pub use catalog::*;

pub fn is_reserved_mobile_base_poi_id(poi_id: &str) -> bool {
    poi_id == MOBILE_BASE_POI_ID
        || poi_id == MOBILE_BASE_STATION_ID
        || poi_id == LEGACY_MOBILE_BASE_STATION_ID
}

pub fn diff_positive_item_deltas(
    before: &HashMap<String, i64>,
    after: &HashMap<String, i64>,
) -> HashMap<String, i64> {
    let mut deltas = HashMap::new();
    for (item, after_qty) in after {
        let before_qty = before.get(item).copied().unwrap_or(0);
        let gained = after_qty - before_qty;
        if gained > 0 {
            deltas.insert(item.clone(), gained);
        }
    }
    deltas
}

pub fn diff_item_deltas(
    before: &HashMap<String, i64>,
    after: &HashMap<String, i64>,
) -> Vec<String> {
    let mut all_keys: std::collections::BTreeSet<String> = before.keys().cloned().collect();
    all_keys.extend(after.keys().cloned());
    all_keys
        .into_iter()
        .filter_map(|item| {
            let b = before.get(&item).copied().unwrap_or(0);
            let a = after.get(&item).copied().unwrap_or(0);
            if b != a {
                Some(format!("{item}: {b} -> {a}"))
            } else {
                None
            }
        })
        .collect()
}

pub fn arrow(before: &Option<String>, after: &Option<String>) -> String {
    format!(
        "{} -> {}",
        before.as_deref().unwrap_or("?"),
        after.as_deref().unwrap_or("?")
    )
}

pub fn map_active_frame(snapshot: &RuntimeSnapshot) -> Option<RuntimeActiveFrameDto> {
    snapshot
        .active_frame
        .as_ref()
        .map(|frame| RuntimeActiveFrameDto {
            kind: frame.kind.clone(),
            name: frame.name.clone(),
            path: frame.path.clone(),
            script: frame.script.clone(),
            line: frame.line,
        })
}

pub fn latest_result_message(snapshot: &RuntimeSnapshot) -> Option<String> {
    snapshot
        .memory
        .iter()
        .rev()
        .filter_map(|entry| entry.result_message.as_deref())
        .map(str::trim)
        .find(|message| !message.is_empty())
        .map(str::to_string)
}

pub fn active_go_route(
    snapshot: &RuntimeSnapshot,
    state: &impl prayer_runtime::navigation::NavigationState,
) -> Option<ActiveGoRouteDto> {
    let target = active_command_navigation_target(state, snapshot.active_command.as_ref()?)?;
    let start = state.system()?;
    let hops = state
        .galaxy()
        .shortest_path_hops(start, target.system.as_str())?;
    let total_jumps = i32::try_from(hops.len()).ok()?;

    Some(ActiveGoRouteDto {
        target: target.label,
        target_system: target.system,
        target_poi: target.poi,
        hops,
        total_jumps,
        estimated_fuel_use: total_jumps,
        arrival_time: None,
    })
}

pub fn estimated_jump_fuel_per_jump(actor: &BotState, catalog: &CatalogData) -> Option<i64> {
    let scale = catalog
        .ships
        .get(actor.ship.class_id.as_deref()?)
        .and_then(|entry| entry.scale)?;
    let speed = actor.ship.speed?;
    if scale <= 0 || speed <= 0 {
        return None;
    }
    let fuel = (scale as f64).powf(1.5) * speed as f64;
    Some(fuel.ceil().max(1.0) as i64)
}

pub struct ActorNavigationRead {
    pub system: Option<String>,
    pub current_poi: Option<String>,
    pub nearest_station: Option<String>,
    pub home_poi: Option<String>,
    pub home_base: Option<String>,
    pub galaxy: Arc<GalaxyData>,
}

impl prayer_runtime::navigation::NavigationState for ActorNavigationRead {
    fn system(&self) -> Option<&str> {
        self.system.as_deref()
    }
    fn current_poi(&self) -> Option<&str> {
        self.current_poi.as_deref()
    }
    fn nearest_station(&self) -> Option<&str> {
        self.nearest_station.as_deref()
    }
    fn home_poi(&self) -> Option<&str> {
        self.home_poi.as_deref()
    }
    fn home_base(&self) -> Option<&str> {
        self.home_base.as_deref()
    }
    fn galaxy(&self) -> &GalaxyData {
        &self.galaxy
    }
}

impl ActorNavigationRead {
    pub fn new(actor: &BotState, galaxy: Arc<GalaxyData>) -> Self {
        let mut value = Self {
            system: actor.location.system_id.clone(),
            current_poi: actor.location.poi_id.clone(),
            nearest_station: None,
            home_poi: actor.player.home_poi.clone(),
            home_base: actor.player.home_base.clone(),
            galaxy,
        };
        value.nearest_station = prayer_runtime::navigation::nearest_station_poi(&value);
        value
    }
}

pub struct ScriptDiffSnapshot {
    credits: i64,
    fuel_pct: i64,
    system: Option<String>,
    current_poi: Option<String>,
    docked: bool,
    cargo: HashMap<String, i64>,
    storage: HashMap<String, i64>,
}

impl ScriptDiffSnapshot {
    pub fn from_scopes(state: &BotState, knowledge: &WorldState) -> Self {
        let storage = storage_player_keys_for_actor(state)
            .into_iter()
            .find_map(|key| knowledge.storage_by_player.get(key))
            .map(storage_totals_by_item)
            .unwrap_or_default();
        Self {
            credits: state.player.credits.unwrap_or_default(),
            fuel_pct: state.fuel_pct,
            system: state.location.system_id.clone(),
            current_poi: state.location.poi_id.clone(),
            docked: state.location.docked_at.is_some(),
            cargo: state.cargo.as_ref().clone(),
            storage,
        }
    }
}

pub fn compute_script_diff(
    before: &ScriptDiffSnapshot,
    after: &ScriptDiffSnapshot,
    halted_after: bool,
) -> ScriptDiff {
    let docking_changed = before.docked != after.docked;

    let credits = (before.credits != after.credits)
        .then(|| format!("{} -> {}", before.credits, after.credits));
    let fuel = (before.fuel_pct != after.fuel_pct)
        .then(|| format!("{} -> {}", before.fuel_pct, after.fuel_pct));

    let system_changed = before.system != after.system;
    let poi_changed = before.current_poi != after.current_poi;
    let location = (system_changed || poi_changed).then(|| ScriptLocationDelta {
        system: system_changed.then(|| arrow(&before.system, &after.system)),
        poi: poi_changed.then(|| arrow(&before.current_poi, &after.current_poi)),
    });

    let cargo = diff_item_deltas(&before.cargo, &after.cargo);

    // Storage visibility is unreliable across a dock/undock transition — suppress to avoid noise.
    let storage = (!docking_changed).then(|| diff_item_deltas(&before.storage, &after.storage));

    ScriptDiff {
        credits,
        fuel,
        location,
        cargo,
        storage,
        flags: ScriptDiffFlags {
            docked_before: before.docked,
            docked_after: after.docked,
            halted_after,
        },
    }
}

pub fn storage_totals_by_item(
    storage: &HashMap<String, HashMap<String, i64>>,
) -> HashMap<String, i64> {
    let mut totals = HashMap::new();
    for items in storage.values() {
        for (item, qty) in items {
            if *qty <= 0 {
                continue;
            }
            *totals.entry(item.clone()).or_insert(0) += *qty;
        }
    }
    totals
}

pub fn command_stores_to_personal_storage(command: &prayer_actions::ResolvedAction) -> bool {
    if !command.action.eq_ignore_ascii_case("transfer") {
        return false;
    }
    let args = command.args_as_strings();
    let Some(first) = args.first().map(String::as_str) else {
        return false;
    };
    let cargo_to_storage = args
        .get(args.len().saturating_sub(2))
        .zip(args.last())
        .is_some_and(|(from, to)| from == "cargo" && to == "storage");
    cargo_to_storage && matches!(first, "all" | "item" | "items")
}

pub fn storage_player_key_for_actor(state: &BotState) -> Option<&str> {
    state
        .player
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            state
                .player
                .username
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
}

pub fn storage_player_keys_for_actor(state: &BotState) -> Vec<&str> {
    let mut keys = Vec::new();
    for candidate in [state.player.id.as_deref(), state.player.username.as_deref()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !keys
            .iter()
            .any(|existing: &&str| existing.eq_ignore_ascii_case(candidate))
        {
            keys.push(candidate);
        }
    }
    keys
}

pub fn faction_storage_key_for_actor(state: &BotState) -> Option<&str> {
    state
        .player
        .faction_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn faction_garage_key_for_actor(state: &BotState) -> Option<&str> {
    faction_storage_key_for_actor(state).or_else(|| {
        state
            .player
            .clan_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub fn faction_garage_has_locations(garage: &FactionGarageInfo) -> bool {
    garage
        .ships
        .iter()
        .any(|ship| !ship.base_id.trim().is_empty())
}

pub use ingestion::observations::*;

mod facilities;

pub use facilities::*;

mod execution_adapter;
mod lenses;

pub use execution_adapter::*;
pub use lenses::*;

pub use ingestion::mobile_capital::*;

mod ingestion;

pub use ingestion::*;
mod virtual_market;

pub use virtual_market::*;

mod crafting;

mod projection;
