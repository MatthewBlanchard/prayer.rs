use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}
fn default_priority() -> f64 {
    1.0
}
fn default_order_status() -> String {
    "available".to_string()
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMarketOrder {
    pub id: String,
    #[serde(default = "default_order_status")]
    pub status: String,
    pub side: String,
    pub item_id: String,
    pub station_id: String,
    pub price_each: i64,
    pub quantity: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tipping_point: Option<i64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub dumping: bool,
    #[serde(default)]
    pub reserved: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation_id: Option<String>,
    #[serde(default)]
    pub filled: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub internal_only: bool,
    #[serde(default = "default_priority")]
    pub priority: f64,
    #[serde(default)]
    pub do_forever: bool,
}

pub fn normalize_virtual_order(mut order: VirtualMarketOrder) -> Option<VirtualMarketOrder> {
    order.id = order.id.trim().to_string();
    order.side = order.side.trim().to_ascii_lowercase();
    order.item_id = order.item_id.trim().to_string();
    order.station_id = order.station_id.trim().to_string();
    if order.id.is_empty()
        || !matches!(
            order.side.as_str(),
            "buy" | "sell" | "buy_until" | "sell_until"
        )
        || order.item_id.is_empty()
        || order.station_id.is_empty()
        || order.price_each <= 0
        || order.quantity <= 0
    {
        return None;
    }
    if matches!(order.side.as_str(), "buy_until" | "sell_until") {
        order.reserved = order.reserved.max(0);
        order.filled = 0;
    } else {
        order.do_forever = false;
        order.reserved = order.reserved.max(0).min(order.quantity);
        order.filled = order.filled.max(0).min(order.quantity);
        if order.reserved.saturating_add(order.filled) > order.quantity {
            order.reserved = order.quantity.saturating_sub(order.filled);
        }
    }
    if order.reserved == 0 {
        order.reservation_id = None;
    }
    order.status = if !order.enabled {
        "disabled"
    } else if order.reserved > 0 {
        "reserved"
    } else if !matches!(order.side.as_str(), "buy_until" | "sell_until")
        && order.filled >= order.quantity
    {
        "filled"
    } else {
        "available"
    }
    .to_string();
    if !order.priority.is_finite() || order.priority <= 0.0 {
        order.priority = 1.0;
    }
    if order.side != "sell_until" {
        order.tipping_point = None;
        order.dumping = false;
    } else if order.tipping_point.is_none_or(|point| point <= 0) {
        order.tipping_point = None;
        order.dumping = false;
    }
    Some(order)
}

pub fn fixed_virtual_order_is_settled(order: &VirtualMarketOrder) -> bool {
    !matches!(order.side.as_str(), "buy_until" | "sell_until")
        && order.reserved.max(0) == 0
        && order.filled.max(0) >= order.quantity.max(0)
}

pub fn prune_settled_virtual_orders(orders: &mut Vec<VirtualMarketOrder>) {
    orders.retain(|order| !fixed_virtual_order_is_settled(order));
}
