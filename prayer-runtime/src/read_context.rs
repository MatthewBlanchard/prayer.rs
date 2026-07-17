//! Explicit read capabilities supplied to one runtime command step.

use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use crate::{
    BotState, CatalogData, FactionGarageInfo, GalaxyData, MarketData, PassengerState, SalvageData,
};

/// Actor-selected, immutable lens over canonical shared world state.
#[derive(Debug, Clone, Default)]
pub struct WorldReadState {
    pub nearest_station: Option<String>,
    pub storage: Arc<HashMap<String, HashMap<String, i64>>>,
    /// Faction storage at the actor's current POI. `None` means no faction
    /// storage is known to exist there; `Some(empty)` means it exists but is
    /// empty.
    pub faction_storage: Option<Arc<HashMap<String, i64>>>,
    pub faction_garage: FactionGarageInfo,
    pub catalog: Arc<CatalogData>,
    pub galaxy: Arc<GalaxyData>,
    pub market: Arc<MarketData>,
    pub salvage: Arc<SalvageData>,
    pub station_passengers: PassengerState,
    pub nearby_creature_count: Option<i64>,
    pub wildlife_by_poi: Arc<HashMap<String, crate::WildlifePoiSnapshotData>>,
    pub system_agents: Arc<Vec<crate::AgentSightingData>>,
    pub managed_players: Arc<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionRuntimeState {
    pub script_mined_by_item: Arc<HashMap<String, i64>>,
    pub script_stored_by_item: Arc<HashMap<String, i64>>,
}

/// Borrowed inputs for PrayerLang, policies, and focused planners.
#[derive(Clone, Copy)]
pub struct ExecutionReadContext<'a> {
    pub bot: &'a BotState,
    pub world: &'a WorldReadState,
    pub runtime: &'a ExecutionRuntimeState,
}

impl Default for ExecutionReadContext<'static> {
    fn default() -> Self {
        static BOT: std::sync::LazyLock<BotState> = std::sync::LazyLock::new(BotState::default);
        static WORLD: std::sync::LazyLock<WorldReadState> =
            std::sync::LazyLock::new(WorldReadState::default);
        static RUNTIME: std::sync::LazyLock<ExecutionRuntimeState> =
            std::sync::LazyLock::new(ExecutionRuntimeState::default);
        Self {
            bot: &BOT,
            world: &WORLD,
            runtime: &RUNTIME,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeCapability {
    Actor,
    Navigation,
    Inventory,
    Market,
    Nearby,
    Facility,
}

/// Focused immutable inputs for one planner step.
pub struct RuntimeReadContext {
    state: PlanningState,
    capabilities: BTreeSet<RuntimeCapability>,
}

#[derive(Clone, Default)]
pub(crate) struct PlanningState {
    pub system: Option<String>,
    pub current_poi: Option<String>,
    pub nearest_station: Option<String>,
    pub home_poi: Option<String>,
    pub home_base: Option<String>,
    pub docked: bool,
    pub in_transit: bool,
    pub transit_type: Option<String>,
    pub transit_dest_system: Option<String>,
    pub transit_dest_poi: Option<String>,
    pub fuel_pct: i64,
    pub cargo_pct: i64,
    pub cargo_used: i64,
    pub cargo_capacity: i64,
    pub cargo: Arc<HashMap<String, i64>>,
    pub storage: Arc<HashMap<String, HashMap<String, i64>>>,
    pub faction_storage: Arc<HashMap<String, i64>>,
    pub catalog: Arc<CatalogData>,
    pub galaxy: Arc<GalaxyData>,
    pub market: Arc<MarketData>,
    pub own_buy_orders: Arc<Vec<spacemolt_lib_rs::schema::ExchangeOrder>>,
    pub own_sell_orders: Arc<Vec<spacemolt_lib_rs::schema::ExchangeOrder>>,
    pub passengers: PassengerState,
    pub salvage: Arc<SalvageData>,
}

impl PlanningState {
    pub(crate) fn from_context(context: ExecutionReadContext<'_>) -> Self {
        Self {
            system: context.bot.location.system_id.clone(),
            current_poi: context.bot.location.poi_id.clone(),
            nearest_station: context.world.nearest_station.clone(),
            home_poi: context.bot.player.home_poi.clone(),
            home_base: context.bot.player.home_base.clone(),
            docked: context.bot.location.docked_at.is_some(),
            in_transit: context.bot.location.in_transit.unwrap_or(false),
            transit_type: context.bot.location.transit_type.clone(),
            transit_dest_system: context.bot.location.transit_dest_system_id.clone(),
            transit_dest_poi: context.bot.location.transit_dest_poi_id.clone(),
            fuel_pct: context.bot.fuel_pct,
            cargo_pct: context.bot.cargo_pct,
            cargo_used: context.bot.cargo_used,
            cargo_capacity: context.bot.cargo_capacity,
            cargo: Arc::clone(&context.bot.cargo),
            storage: Arc::clone(&context.world.storage),
            faction_storage: context.world.faction_storage.clone().unwrap_or_default(),
            catalog: Arc::clone(&context.world.catalog),
            galaxy: Arc::clone(&context.world.galaxy),
            market: Arc::clone(&context.world.market),
            own_buy_orders: Arc::clone(&context.bot.own_buy_orders),
            own_sell_orders: Arc::clone(&context.bot.own_sell_orders),
            passengers: PassengerState {
                aboard_count: context.bot.passengers.aboard_count,
                economy_berths: context.bot.passengers.economy_berths,
                economy_berths_raw: context.bot.passengers.economy_berths_raw.clone(),
                business_berths: context.bot.passengers.business_berths,
                business_berths_raw: context.bot.passengers.business_berths_raw.clone(),
                first_berths: context.bot.passengers.first_berths,
                first_berths_raw: context.bot.passengers.first_berths_raw.clone(),
                aboard: Arc::clone(&context.bot.passengers.aboard),
                station: context.world.station_passengers.station.clone(),
                waiting_count: context.world.station_passengers.waiting_count,
                waiting: Arc::clone(&context.world.station_passengers.waiting),
            },
            salvage: Arc::clone(&context.world.salvage),
        }
    }

    pub(crate) fn item_cargo_size(&self, item_id: &str) -> i64 {
        self.catalog
            .items
            .get(item_id)
            .and_then(spacemolt_lib_rs::schema::CatalogDumpItemsItem::cargo_size)
            .filter(|size| *size > 0)
            .unwrap_or(1)
    }
    pub(crate) fn storage_at_current_location(&self) -> Option<&HashMap<String, i64>> {
        self.current_poi
            .as_deref()
            .and_then(|poi| self.storage.get(poi))
    }
}

impl RuntimeReadContext {
    pub fn from_execution(context: ExecutionReadContext<'_>, action: &str) -> Self {
        let capabilities = capabilities_for_action(action).into_iter().collect();
        Self {
            state: PlanningState::from_context(context),
            capabilities,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_command(state: &PlanningState, action: &str) -> Self {
        let capabilities = capabilities_for_action(action).into_iter().collect();
        Self {
            state: state.clone(),
            capabilities,
        }
    }

    pub fn capabilities(&self) -> &BTreeSet<RuntimeCapability> {
        &self.capabilities
    }

    pub(crate) fn planning_state(&self) -> &PlanningState {
        &self.state
    }
}

pub fn capabilities_for_action(action: &str) -> Vec<RuntimeCapability> {
    use RuntimeCapability::*;
    match action.trim().to_ascii_lowercase().as_str() {
        "go" | "find" | "refuel" | "dock" | "set_home" => vec![Actor, Navigation],
        "buy" | "sell" | "cancel_buy" | "cancel_sell" => vec![Actor, Inventory, Market],
        "transfer" | "mine" => vec![Actor, Navigation, Inventory],
        "say" => vec![Actor, Nearby],
        "craft" | "commission_ship" => vec![Actor, Inventory, Facility],
        _ => vec![Actor],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_declare_focused_capabilities() {
        assert_eq!(
            capabilities_for_action("buy"),
            vec![
                RuntimeCapability::Actor,
                RuntimeCapability::Inventory,
                RuntimeCapability::Market
            ]
        );
        assert_eq!(
            capabilities_for_action("go"),
            vec![RuntimeCapability::Actor, RuntimeCapability::Navigation]
        );
    }
}
