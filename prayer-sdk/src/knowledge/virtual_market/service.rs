//! RuntimeService facade methods for virtual orders and quartermaster reconciliation.

use super::super::*;
use prayer_runtime::knowledge::summed_virtual_order_uses;

impl RuntimeService {
    pub async fn commander_versions(&self) -> (u64, u64) {
        let knowledge_version = self.knowledge_state.read().knowledge_version;
        let state_version = self.commander_state_sequence.load(Ordering::Acquire);
        (state_version, knowledge_version)
    }

    pub fn knowledge_version(&self) -> u64 {
        self.knowledge_state.read().knowledge_version
    }

    pub fn virtual_orders(&self) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.knowledge_state.read().virtual_orders.clone()
    }

    pub fn virtual_order_open_quantity(
        &self,
        actor: &BotState,
        order: &RuntimeVirtualMarketOrderDto,
    ) -> i64 {
        let knowledge = self.knowledge_state.read();
        let open = virtual_market_order_open_quantity_for_actor(&knowledge, actor, order);
        if !matches!(order.side.as_str(), "sell" | "sell_until") {
            return open;
        }
        let Some(faction_id) = faction_storage_key_for_actor(actor) else {
            return open;
        };
        let location_id = knowledge
            .galaxy
            .poi_id_for_base(&order.station_id)
            .unwrap_or(order.station_id.as_str());
        let claimed = self.inventory_reservations.lock().reserved_for_compound(
            "faction_storage",
            faction_id,
            location_id,
            &order.item_id,
        );
        match order.side.as_str() {
            "sell" => {
                let storage = known_faction_storage_at_station_quantity_for_actor(
                    &knowledge,
                    actor,
                    &order.station_id,
                    &order.item_id,
                )
                .unwrap_or(i64::MAX);
                open.min(storage.saturating_sub(claimed).max(0))
            }
            "sell_until" => open.saturating_sub(claimed),
            _ => open,
        }
    }

    pub fn economy_virtual_order_open_quantity(
        &self,
        state: &prayer_runtime::economy::EconomyReadState,
        order: &RuntimeVirtualMarketOrderDto,
    ) -> i64 {
        let knowledge = self.knowledge_state.read();
        let actor = BotState {
            player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                faction_id: state.faction_id.clone(),
                clan_tag: state.clan_tag.clone(),
                username: state.username.clone(),
                id: state.player_id.clone(),
                ..Default::default()
            },
            ..BotState::default()
        };
        let open = virtual_market_order_open_quantity_for_actor(&knowledge, &actor, order);
        if !matches!(order.side.as_str(), "sell" | "sell_until") {
            return open;
        }
        let Some(faction) = state.faction_id.as_deref() else {
            return open;
        };
        let location = state
            .galaxy
            .poi_id_for_base(&order.station_id)
            .unwrap_or(&order.station_id);
        let claimed = self.inventory_reservations.lock().reserved_for_compound(
            "faction_storage",
            faction,
            location,
            &order.item_id,
        );
        match order.side.as_str() {
            "sell" => open.min(
                state
                    .faction_storage
                    .get(&order.item_id)
                    .copied()
                    .unwrap_or(i64::MAX)
                    .saturating_sub(claimed)
                    .max(0),
            ),
            "sell_until" => open.saturating_sub(claimed),
            _ => open,
        }
    }

    pub fn faction_storage_quantity_at_station(
        &self,
        state: &prayer_runtime::economy::EconomyReadState,
        station_id: &str,
        item_id: &str,
    ) -> i64 {
        let knowledge = self.knowledge_state.read();
        let actor = BotState {
            player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                faction_id: state.faction_id.clone(),
                ..Default::default()
            },
            ..BotState::default()
        };
        // Resolve the caller-supplied station id (which may be a base id alias)
        // against the read snapshot's galaxy before the storage lookup; the
        // shared knowledge galaxy is not the authority for this read path.
        let station_id = state
            .galaxy
            .poi_id_for_base(station_id)
            .unwrap_or(station_id);
        known_faction_storage_at_station_quantity_for_actor(&knowledge, &actor, station_id, item_id)
            .unwrap_or(0)
    }

    pub fn replace_virtual_orders(
        &self,
        orders: Vec<RuntimeVirtualMarketOrderDto>,
    ) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.mutate_virtual_orders(|current| {
            *current = orders
                .into_iter()
                .filter_map(normalize_virtual_order)
                .collect();
        })
    }

    pub fn reserve_virtual_orders_detailed(
        &self,
        uses: Vec<RuntimeVirtualOrderUseDto>,
    ) -> (
        Vec<RuntimeVirtualMarketOrderDto>,
        Vec<RuntimeVirtualOrderReservationResultDto>,
    ) {
        let use_by_id = summed_virtual_order_uses(uses);
        let mut reservation_results = Vec::new();
        let orders = self.mutate_virtual_orders(|orders| {
            for order in &mut *orders {
                let requested = use_by_id.get(&order.id).copied().unwrap_or(0);
                if requested <= 0 {
                    continue;
                }
                let reserved_before = order.reserved;
                if matches!(order.side.as_str(), "buy_until" | "sell_until") {
                    if order.side.as_str() == "sell_until" && order.tipping_point.is_some() {
                        order.dumping = true;
                    }
                    order.reserved = order.reserved.saturating_add(requested.max(0));
                    order.status = "reserved".to_string();
                    if order.reserved > 0 && order.reservation_id.is_none() {
                        order.reservation_id = Some(uuid::Uuid::new_v4().to_string());
                    }
                    reservation_results.push(RuntimeVirtualOrderReservationResultDto {
                        order_id: order.id.clone(),
                        reservation_id: order.reservation_id.clone(),
                        requested,
                        accepted: requested.max(0),
                        reserved_before,
                        reserved_after: order.reserved,
                    });
                    continue;
                }
                let available = order
                    .quantity
                    .saturating_sub(order.reserved.max(0))
                    .saturating_sub(order.filled.max(0));
                let accepted = if requested <= available { requested } else { 0 };
                order.reserved = order.reserved.saturating_add(accepted);
                if accepted > 0 {
                    order.status = "reserved".to_string();
                }
                if accepted > 0 && order.reservation_id.is_none() {
                    order.reservation_id = Some(uuid::Uuid::new_v4().to_string());
                }
                reservation_results.push(RuntimeVirtualOrderReservationResultDto {
                    order_id: order.id.clone(),
                    reservation_id: order.reservation_id.clone(),
                    requested,
                    accepted,
                    reserved_before,
                    reserved_after: order.reserved,
                });
            }
            for (order_id, requested) in &use_by_id {
                if *requested <= 0 || orders.iter().any(|order| order.id == *order_id) {
                    continue;
                }
                reservation_results.push(RuntimeVirtualOrderReservationResultDto {
                    order_id: order_id.clone(),
                    reservation_id: None,
                    requested: *requested,
                    accepted: 0,
                    reserved_before: 0,
                    reserved_after: 0,
                });
            }
        });
        (orders, reservation_results)
    }

    pub fn reserve_virtual_order_uses_atomic(&self, uses: Vec<RuntimeVirtualOrderUseDto>) -> bool {
        let use_by_id = summed_virtual_order_uses(uses);
        if use_by_id.is_empty() {
            return true;
        }
        let knowledge = {
            let mut knowledge = self.knowledge_state.write();
            let all_available = use_by_id.iter().all(|(order_id, requested)| {
                if *requested <= 0 {
                    return false;
                }
                knowledge
                    .virtual_orders
                    .iter()
                    .find(|order| order.id == *order_id)
                    .is_some_and(|order| {
                        matches!(order.side.as_str(), "buy_until" | "sell_until")
                            || order
                                .quantity
                                .saturating_sub(order.reserved.max(0))
                                .saturating_sub(order.filled.max(0))
                                >= *requested
                    })
            });
            if !all_available {
                return false;
            }
            for order in &mut knowledge.virtual_orders {
                let requested = use_by_id.get(&order.id).copied().unwrap_or(0);
                if requested <= 0 {
                    continue;
                }
                if order.side == "sell_until" && order.tipping_point.is_some() {
                    order.dumping = true;
                }
                order.reserved = order.reserved.saturating_add(requested);
                order.status = "reserved".to_string();
                if order.reservation_id.is_none() {
                    order.reservation_id = Some(uuid::Uuid::new_v4().to_string());
                }
            }
            knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            knowledge.clone()
        };
        self.knowledge_persistence
            .publish(knowledge, "after atomic movement virtual reservation");
        true
    }

    pub fn fill_virtual_order(&self, id: &str) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.mutate_virtual_orders(|orders| {
            for order in orders {
                if order.id == id {
                    if matches!(order.side.as_str(), "buy_until" | "sell_until") {
                        order.filled = 0;
                    } else {
                        order.filled = order.filled.saturating_add(order.reserved.max(0));
                    }
                    order.reserved = 0;
                    order.reservation_id = None;
                    order.status = if matches!(order.side.as_str(), "buy_until" | "sell_until") {
                        "available"
                    } else {
                        "filled"
                    }
                    .to_string();
                }
            }
        })
    }

    pub fn release_virtual_order(&self, id: &str) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.mutate_virtual_orders(|orders| {
            for order in orders {
                if order.id == id {
                    order.reserved = 0;
                    order.reservation_id = None;
                    order.status = "available".to_string();
                }
            }
        })
    }

    pub fn settle_virtual_order_uses(&self, uses: &[RuntimeVirtualOrderUseDto], completed: bool) {
        let quantities = summed_virtual_order_uses(uses.to_vec());
        self.mutate_virtual_orders(|orders| {
            for order in orders {
                let quantity = quantities.get(&order.id).copied().unwrap_or(0).max(0);
                if quantity == 0 {
                    continue;
                }
                order.reserved = order.reserved.saturating_sub(quantity);
                if completed && !matches!(order.side.as_str(), "buy_until" | "sell_until") {
                    order.filled = order.filled.saturating_add(quantity).min(order.quantity);
                }
                if order.reserved == 0 {
                    order.reservation_id = None;
                    order.status = if completed
                        && !matches!(order.side.as_str(), "buy_until" | "sell_until")
                        && order.filled >= order.quantity
                    {
                        "filled"
                    } else {
                        "available"
                    }
                    .to_string();
                }
            }
        });
    }

    pub fn mutate_virtual_orders(
        &self,
        mutate: impl FnOnce(&mut Vec<RuntimeVirtualMarketOrderDto>),
    ) -> Vec<RuntimeVirtualMarketOrderDto> {
        self.mutate_virtual_orders_with_knowledge(|knowledge| {
            mutate(&mut knowledge.virtual_orders);
        })
    }

    pub fn mutate_virtual_orders_with_knowledge(
        &self,
        mutate: impl FnOnce(&mut WorldState),
    ) -> Vec<RuntimeVirtualMarketOrderDto> {
        let knowledge = {
            let mut knowledge = self.knowledge_state.write();
            mutate(&mut knowledge);
            prune_settled_virtual_orders(&mut knowledge.virtual_orders);
            knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            knowledge.clone()
        };
        let orders = knowledge.virtual_orders.clone();
        self.knowledge_persistence
            .publish(knowledge, "after virtual order mutation");
        orders
    }
}
