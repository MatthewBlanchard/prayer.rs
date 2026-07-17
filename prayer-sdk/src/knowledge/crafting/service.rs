//! RuntimeService facade for observing the authoritative SpaceMolt crafting queue.

use super::super::*;

impl RuntimeService {
    pub fn virtual_craft_orders(&self) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.knowledge_state.read().virtual_craft_orders.clone()
    }

    pub fn replace_virtual_craft_orders(
        &self,
        orders: Vec<RuntimeVirtualCraftOrderDto>,
    ) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.mutate_virtual_craft_orders(|current| {
            *current = orders
                .into_iter()
                .filter_map(prayer_runtime::knowledge::normalize_virtual_craft_order)
                .collect();
        })
    }

    pub fn reserve_virtual_craft_orders(
        &self,
        uses: Vec<RuntimeVirtualOrderUseDto>,
    ) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.reserve_virtual_craft_orders_detailed(uses).0
    }

    pub fn reserve_virtual_craft_orders_detailed(
        &self,
        uses: Vec<RuntimeVirtualOrderUseDto>,
    ) -> (
        Vec<RuntimeVirtualCraftOrderDto>,
        Vec<RuntimeVirtualOrderReservationResultDto>,
    ) {
        let use_by_id = prayer_runtime::knowledge::summed_virtual_order_uses(uses);
        let mut results = Vec::new();
        let orders = self.mutate_virtual_craft_orders(|orders| {
            for order in &mut *orders {
                let requested = use_by_id.get(&order.id).copied().unwrap_or(0);
                if requested <= 0 {
                    continue;
                }
                let reserved_before = order.reserved;
                if order.action == "craft_until" {
                    order.reserved = order.reserved.saturating_add(requested);
                    order.status = "reserved".to_string();
                    if order.reservation_id.is_none() {
                        order.reservation_id = Some(uuid::Uuid::new_v4().to_string());
                    }
                    results.push(RuntimeVirtualOrderReservationResultDto {
                        order_id: order.id.clone(),
                        reservation_id: order.reservation_id.clone(),
                        requested,
                        accepted: requested,
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
                results.push(RuntimeVirtualOrderReservationResultDto {
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
                results.push(RuntimeVirtualOrderReservationResultDto {
                    order_id: order_id.clone(),
                    reservation_id: None,
                    requested: *requested,
                    accepted: 0,
                    reserved_before: 0,
                    reserved_after: 0,
                });
            }
        });
        (orders, results)
    }

    pub fn fill_virtual_craft_order(&self, id: &str) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.mutate_virtual_craft_orders(|orders| {
            for order in orders {
                if order.id == id {
                    if order.action == "craft_until" {
                        order.filled = 0;
                    } else {
                        order.filled = order.filled.saturating_add(order.reserved.max(0));
                    }
                    order.reserved = 0;
                    order.reservation_id = None;
                    order.status = if order.action == "craft_until" {
                        "available"
                    } else {
                        "filled"
                    }
                    .to_string();
                }
            }
        })
    }

    pub fn release_virtual_craft_order(&self, id: &str) -> Vec<RuntimeVirtualCraftOrderDto> {
        self.mutate_virtual_craft_orders(|orders| {
            for order in orders {
                if order.id == id {
                    order.reserved = 0;
                    order.reservation_id = None;
                    order.status = "available".to_string();
                }
            }
        })
    }

    fn mutate_virtual_craft_orders(
        &self,
        mutate: impl FnOnce(&mut Vec<RuntimeVirtualCraftOrderDto>),
    ) -> Vec<RuntimeVirtualCraftOrderDto> {
        let knowledge = {
            let mut knowledge = self.knowledge_state.write();
            mutate(&mut knowledge.virtual_craft_orders);
            knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            knowledge.clone()
        };
        let orders = knowledge.virtual_craft_orders.clone();
        self.knowledge_persistence
            .publish(knowledge, "after virtual craft order mutation");
        orders
    }
}
