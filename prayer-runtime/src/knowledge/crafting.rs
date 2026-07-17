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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VirtualCraftOrder {
    pub id: String,
    #[serde(default = "default_order_status")]
    pub status: String,
    pub action: String,
    pub recipe_id: String,
    #[serde(default)]
    pub item_id: String,
    #[serde(default)]
    pub station_id: String,
    pub quantity: i64,
    #[serde(default)]
    pub reserved: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation_id: Option<String>,
    #[serde(default)]
    pub filled: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_priority")]
    pub priority: f64,
    #[serde(default)]
    pub facility_id: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub squad_id: Option<String>,
    #[serde(default)]
    pub session_handles: Vec<String>,
    #[serde(default)]
    pub credit_floor: Option<i64>,
    #[serde(default)]
    pub do_forever: bool,
}

pub fn normalize_virtual_craft_order(mut order: VirtualCraftOrder) -> Option<VirtualCraftOrder> {
    order.id = order.id.trim().to_string();
    order.action = order.action.trim().to_ascii_lowercase();
    order.recipe_id = order.recipe_id.trim().to_string();
    order.item_id = order.item_id.trim().to_string();
    order.station_id = order.station_id.trim().to_string();
    order.facility_id = normalize_optional(&order.facility_id);
    order.preset = normalize_optional(&order.preset);
    order.squad_id = normalize_optional(&order.squad_id);
    order.session_handles = order
        .session_handles
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    if order.id.is_empty()
        || order.quantity <= 0
        || !matches!(
            order.action.as_str(),
            "craft" | "craft_until" | "commission_until" | "credit_floor"
        )
    {
        return None;
    }
    if order.action == "credit_floor" {
        order.recipe_id.clear();
        order.item_id = "credits".to_string();
        order.quantity = order.credit_floor.unwrap_or(order.quantity).max(1);
        order.credit_floor = Some(order.quantity);
        order.reserved = 0;
        order.filled = 0;
        order.facility_id = None;
        order.preset = None;
        if order.squad_id.is_none() || order.session_handles.is_empty() {
            return None;
        }
    } else if order.recipe_id.is_empty() || order.station_id.is_empty() {
        return None;
    }
    if order.action == "commission_until" {
        if order.item_id.is_empty() {
            order.item_id = order.recipe_id.clone();
        }
        order.facility_id = None;
        order.preset = None;
        order.reserved = order.reserved.max(0);
        order.filled = 0;
    } else if order.action == "craft_until" {
        order.reserved = order.reserved.max(0);
        order.filled = 0;
    } else if order.action == "craft" {
        order.do_forever = false;
        order.reserved = order.reserved.max(0).min(order.quantity);
        order.filled = order.filled.max(0).min(order.quantity);
        if order.reserved.saturating_add(order.filled) > order.quantity {
            order.reserved = order.quantity.saturating_sub(order.filled);
        }
    } else {
        order.do_forever = false;
    }
    if !order.priority.is_finite() || order.priority <= 0.0 {
        order.priority = 1.0;
    }
    if order.reserved <= 0 {
        order.reservation_id = None;
    }
    order.status = if !order.enabled {
        "disabled"
    } else if order.reserved > 0 {
        "reserved"
    } else if order.filled >= order.quantity && order.action == "craft" {
        "filled"
    } else {
        "available"
    }
    .to_string();
    Some(order)
}

fn normalize_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
