//! Focused immutable inputs for economy and logistics planning.

use std::{collections::HashMap, sync::Arc};

use crate::{
    read_context::ExecutionReadContext, ActiveCommissionInfo, CatalogData, CraftingQueueProjection,
    GalaxyData, MarketData, PassengerState,
};

#[derive(Debug, Clone, Default)]
pub struct EconomyReadState {
    pub system: Option<String>,
    pub current_poi: Option<String>,
    pub cargo_used: i64,
    pub cargo_capacity: i64,
    pub credits: i64,
    pub catalog: Arc<CatalogData>,
    pub galaxy: Arc<GalaxyData>,
    pub market: Arc<MarketData>,
    pub faction_storage: Arc<HashMap<String, i64>>,
    pub passengers: PassengerState,
    pub username: Option<String>,
    pub player_id: Option<String>,
    pub faction_id: Option<String>,
    pub clan_tag: Option<String>,
    pub active_commissions: Arc<Vec<ActiveCommissionInfo>>,
    pub crafting_queue: Arc<Vec<CraftingQueueProjection>>,
}

impl EconomyReadState {
    pub fn effective_system_id(&self) -> Option<&str> {
        self.system
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn effective_poi_system_id(&self, poi_id: &str) -> Option<&str> {
        self.galaxy
            .poi_records
            .get(poi_id)
            .map(|poi| poi.system_id.as_str())
            .or_else(|| {
                (self.current_poi.as_deref() == Some(poi_id))
                    .then(|| self.effective_system_id())
                    .flatten()
            })
    }

    pub fn item_cargo_size(&self, item_id: &str) -> i64 {
        self.catalog
            .items
            .get(item_id)
            .and_then(spacemolt_lib_rs::schema::CatalogDumpItemsItem::cargo_size)
            .filter(|size| *size > 0)
            .unwrap_or(1)
    }
}

impl From<ExecutionReadContext<'_>> for EconomyReadState {
    fn from(state: ExecutionReadContext<'_>) -> Self {
        Self {
            system: state.bot.location.system_id.clone(),
            current_poi: state.bot.location.poi_id.clone(),
            cargo_used: state.bot.cargo_used,
            cargo_capacity: state.bot.cargo_capacity,
            credits: state.bot.player.credits.unwrap_or_default(),
            catalog: Arc::clone(&state.world.catalog),
            galaxy: Arc::clone(&state.world.galaxy),
            market: Arc::clone(&state.world.market),
            faction_storage: state.world.faction_storage.clone().unwrap_or_default(),
            passengers: PassengerState {
                aboard_count: state.bot.passengers.aboard_count,
                economy_berths: state.bot.passengers.economy_berths,
                economy_berths_raw: state.bot.passengers.economy_berths_raw.clone(),
                business_berths: state.bot.passengers.business_berths,
                business_berths_raw: state.bot.passengers.business_berths_raw.clone(),
                first_berths: state.bot.passengers.first_berths,
                first_berths_raw: state.bot.passengers.first_berths_raw.clone(),
                aboard: Arc::clone(&state.bot.passengers.aboard),
                station: state.world.station_passengers.station.clone(),
                waiting_count: state.world.station_passengers.waiting_count,
                waiting: Arc::clone(&state.world.station_passengers.waiting),
            },
            username: state.bot.player.username.clone(),
            player_id: state.bot.player.id.clone(),
            faction_id: state.bot.player.faction_id.clone(),
            clan_tag: state.bot.player.clan_tag.clone(),
            active_commissions: Arc::clone(&state.bot.active_commissions),
            crafting_queue: Arc::clone(&state.bot.crafting_queue),
        }
    }
}
