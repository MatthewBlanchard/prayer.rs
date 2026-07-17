//! Narrow operations used by HTTP administration and planning adapters.
//!
//! This handle deliberately exposes operations, not the runtime service itself.

use std::collections::HashMap;
use std::sync::Arc;

use prayer_runtime::economy::EconomyReadState;
use prayer_state::{CatalogData, PassengerState};
use uuid::Uuid;

use crate::*;

#[derive(Clone)]
pub struct PrayerAdministration {
    pub(crate) service: Arc<RuntimeService>,
}

#[cfg(feature = "test-support")]
impl Default for PrayerAdministration {
    fn default() -> Self {
        Self {
            service: Arc::new(RuntimeService::default()),
        }
    }
}

impl PrayerAdministration {
    pub fn inventory_movements(&self) -> RuntimeInventoryMovementsResponse {
        self.service.inventory_movements()
    }

    pub async fn reserve_inventory_movement(
        &self,
        request: RuntimeInventoryMovementReserveRequest,
    ) -> Result<RuntimeInventoryMovementReserveResponse, SdkError> {
        self.service.reserve_inventory_movement(request).await
    }

    pub async fn transition_inventory_movement(
        &self,
        id: Uuid,
        status: RuntimeInventoryMovementStatusDto,
    ) -> Result<RuntimeInventoryMovementDto, SdkError> {
        self.service.transition_inventory_movement(id, status).await
    }

    pub async fn reconcile_inventory_movement(
        &self,
        id: Uuid,
        reason: &str,
    ) -> Result<RuntimeInventoryMovementDto, SdkError> {
        self.service.reconcile_inventory_movement(id, reason).await
    }

    pub fn virtual_orders(&self) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.service.virtual_orders()
    }
    pub fn replace_virtual_orders(
        &self,
        orders: Vec<RuntimeVirtualMarketOrderDto>,
    ) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.service.replace_virtual_orders(orders)
    }
    pub fn reserve_virtual_orders_detailed(
        &self,
        uses: Vec<RuntimeVirtualOrderUseDto>,
    ) -> (
        Vec<RuntimeVirtualMarketOrderDto>,
        Vec<RuntimeVirtualOrderReservationResultDto>,
    ) {
        self.service.reserve_virtual_orders_detailed(uses)
    }
    pub fn fill_virtual_order(&self, id: &str) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.service.fill_virtual_order(id)
    }
    pub fn release_virtual_order(&self, id: &str) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.service.release_virtual_order(id)
    }

    pub fn virtual_craft_orders(&self) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.service.virtual_craft_orders()
    }
    pub fn replace_virtual_craft_orders(
        &self,
        orders: Vec<RuntimeVirtualCraftOrderDto>,
    ) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.service.replace_virtual_craft_orders(orders)
    }
    pub fn reserve_virtual_craft_orders_detailed(
        &self,
        uses: Vec<RuntimeVirtualOrderUseDto>,
    ) -> (
        Vec<RuntimeVirtualCraftOrderDto>,
        Vec<RuntimeVirtualOrderReservationResultDto>,
    ) {
        self.service.reserve_virtual_craft_orders_detailed(uses)
    }
    pub fn fill_virtual_craft_order(&self, id: &str) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.service.fill_virtual_craft_order(id)
    }
    pub fn release_virtual_craft_order(&self, id: &str) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.service.release_virtual_craft_order(id)
    }

    pub async fn economy_read_snapshot(
        &self,
        selector: impl Into<BotSelector>,
    ) -> Result<Option<EconomyReadState>, SdkError> {
        let id = self
            .service
            .session_for_bot_selector(selector.into().as_str())
            .await?;
        self.service.economy_read_snapshot(id).await
    }

    pub async fn craft_planning_snapshot(
        &self,
        selector: impl Into<BotSelector>,
    ) -> Result<
        Option<(
            Arc<CatalogData>,
            HashMap<String, HashMap<String, i64>>,
            Arc<HashMap<String, i64>>,
        )>,
        SdkError,
    > {
        let id = self
            .service
            .session_for_bot_selector(selector.into().as_str())
            .await?;
        self.service.craft_planning_snapshot(id).await
    }

    pub fn arbitrage_passenger_boards(
        &self,
        state: &EconomyReadState,
        include_origin_jump: bool,
    ) -> Vec<PassengerState> {
        self.service
            .arbitrage_passenger_boards(state, include_origin_jump)
    }
    pub fn economy_virtual_order_open_quantity(
        &self,
        state: &EconomyReadState,
        order: &RuntimeVirtualMarketOrderDto,
    ) -> i64 {
        self.service
            .economy_virtual_order_open_quantity(state, order)
    }
    pub fn economy_market_ask_candidates(
        &self,
        state: &EconomyReadState,
        item: &str,
        max_price: i64,
    ) -> Vec<(String, i64, i64, Option<String>)> {
        self.service
            .economy_market_ask_candidates(state, item, max_price)
    }
    pub fn economy_personal_storage_quantity_at(
        &self,
        state: &EconomyReadState,
        location: &str,
        item: &str,
    ) -> i64 {
        self.service
            .economy_personal_storage_quantity_at(state, location, item)
    }
}
