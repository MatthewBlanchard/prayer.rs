//! Virtual market order normalization, settlement, and dump-mode reconciliation.

use super::super::*;

pub use prayer_runtime::knowledge::{normalize_virtual_order, prune_settled_virtual_orders};

pub fn reconcile_virtual_order_dump_modes(knowledge: &mut WorldState, state: &BotState) -> bool {
    let updates: Vec<(String, bool)> = knowledge
        .virtual_orders
        .iter()
        .filter_map(|order| {
            let tipping_point = order.tipping_point.filter(|point| *point > 0)?;
            if order.side.as_str() != "sell_until" {
                return None;
            }
            let storage_quantity = known_faction_storage_at_station_quantity_for_actor(
                knowledge,
                state,
                &order.station_id,
                &order.item_id,
            )
            .unwrap_or(0)
            .max(0);
            let excess = storage_quantity.saturating_sub(order.quantity);
            let dumping = if excess <= 0 {
                false
            } else if excess >= tipping_point {
                true
            } else {
                order.dumping
            };
            (dumping != order.dumping).then(|| (order.id.clone(), dumping))
        })
        .collect();
    if updates.is_empty() {
        return false;
    }
    for (id, dumping) in updates {
        if let Some(order) = knowledge
            .virtual_orders
            .iter_mut()
            .find(|order| order.id == id)
        {
            order.dumping = dumping;
        }
    }
    true
}
