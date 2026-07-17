use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InventoryClaim {
    pub lot_id: Option<String>,
    pub source_kind: String,
    pub owner_id: String,
    pub location_id: String,
    pub item_id: String,
    pub quantity: i64,
}

impl InventoryClaim {
    pub fn key(&self) -> String {
        self.lot_id.clone().unwrap_or_else(|| {
            format!(
                "{}|{}|{}|{}",
                self.source_kind, self.owner_id, self.location_id, self.item_id
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VirtualOrderUse {
    pub order_id: String,
    pub quantity: i64,
}

pub fn summed_virtual_order_uses(uses: Vec<VirtualOrderUse>) -> HashMap<String, i64> {
    let mut summed = HashMap::new();
    for order_use in uses {
        let order_id = order_use.order_id.trim();
        if order_id.is_empty() || order_use.quantity <= 0 {
            continue;
        }
        let quantity: &mut i64 = summed.entry(order_id.to_string()).or_default();
        *quantity = quantity.saturating_add(order_use.quantity);
    }
    summed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryMovementStatus {
    Reserved,
    Running,
    Completed,
    Failed,
    Released,
    NeedsReconciliation,
}

impl InventoryMovementStatus {
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Reserved | Self::Running | Self::NeedsReconciliation
        )
    }

    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Reserved,
                Self::Running | Self::Released | Self::Failed
            ) | (
                Self::Running,
                Self::Completed | Self::Failed | Self::NeedsReconciliation
            ) | (
                Self::NeedsReconciliation,
                Self::Completed | Self::Failed | Self::Released
            ) | (Self::Completed, Self::Released | Self::NeedsReconciliation)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryMovement {
    pub movement_id: Uuid,
    pub session_id: Uuid,
    pub kind: String,
    pub status: InventoryMovementStatus,
    pub claims: Vec<InventoryClaim>,
    pub virtual_order_uses: Vec<VirtualOrderUse>,
    pub context: Value,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone)]
pub struct ResolvedInventoryClaim {
    pub claim: InventoryClaim,
    pub observed_quantity: i64,
}

#[derive(Debug, Clone)]
pub struct InventoryReservationOutcome {
    pub accepted: bool,
    pub movement: Option<InventoryMovement>,
    pub unavailable_claims: Vec<InventoryClaim>,
}

#[derive(Debug, Default)]
pub struct InventoryReservationLedger {
    movements: HashMap<Uuid, InventoryMovement>,
    reserved_by_key: HashMap<String, i64>,
}

impl InventoryReservationLedger {
    pub fn movements(&self) -> Vec<InventoryMovement> {
        let mut movements = self.movements.values().cloned().collect::<Vec<_>>();
        movements.sort_by_key(|movement| (movement.created_at_unix, movement.movement_id));
        movements
    }

    pub fn reserved_for_key(&self, key: &str) -> i64 {
        self.reserved_by_key.get(key).copied().unwrap_or(0)
    }

    pub fn reserved_for_compound(
        &self,
        source_kind: &str,
        owner_id: &str,
        location_id: &str,
        item_id: &str,
    ) -> i64 {
        let suffix = format!("|{owner_id}|{location_id}|{item_id}");
        self.reserved_by_key
            .iter()
            .filter(|(key, _)| *key == &format!("{source_kind}{suffix}") || key.ends_with(&suffix))
            .map(|(_, quantity)| *quantity)
            .sum()
    }

    pub fn reserve(
        &mut self,
        session_id: Uuid,
        kind: String,
        claims: Vec<ResolvedInventoryClaim>,
        virtual_order_uses: Vec<VirtualOrderUse>,
        context: Value,
        now_unix: i64,
    ) -> InventoryReservationOutcome {
        let mut totals = HashMap::<String, (i64, i64)>::new();
        let mut unavailable = Vec::new();
        for resolved in &claims {
            let entry = totals
                .entry(resolved.claim.key())
                .or_insert((0, resolved.observed_quantity.max(0)));
            entry.0 = entry.0.saturating_add(resolved.claim.quantity.max(0));
            entry.1 = entry.1.min(resolved.observed_quantity.max(0));
        }
        for resolved in &claims {
            let key = resolved.claim.key();
            let (requested, observed) = totals.get(&key).copied().unwrap_or_default();
            if resolved.claim.quantity <= 0
                || observed.saturating_sub(self.reserved_for_key(&key)) < requested
            {
                unavailable.push(resolved.claim.clone());
            }
        }
        if (!claims.is_empty() || !virtual_order_uses.is_empty()) && unavailable.is_empty() {
            for (key, (quantity, _)) in totals {
                *self.reserved_by_key.entry(key).or_default() += quantity;
            }
            let movement = InventoryMovement {
                movement_id: Uuid::new_v4(),
                session_id,
                kind: kind.trim().to_string(),
                status: InventoryMovementStatus::Reserved,
                claims: claims.into_iter().map(|resolved| resolved.claim).collect(),
                virtual_order_uses,
                context,
                created_at_unix: now_unix,
                updated_at_unix: now_unix,
            };
            self.movements
                .insert(movement.movement_id, movement.clone());
            InventoryReservationOutcome {
                accepted: true,
                movement: Some(movement),
                unavailable_claims: Vec::new(),
            }
        } else {
            InventoryReservationOutcome {
                accepted: false,
                movement: None,
                unavailable_claims: unavailable,
            }
        }
    }

    pub fn transition(
        &mut self,
        movement_id: Uuid,
        status: InventoryMovementStatus,
        now_unix: i64,
    ) -> Option<InventoryMovement> {
        let movement = self.movements.get(&movement_id)?;
        if !movement.status.can_transition_to(status) {
            return None;
        }
        if movement.status.is_active() && !status.is_active() {
            for claim in &movement.claims {
                let key = claim.key();
                if let Some(quantity) = self.reserved_by_key.get_mut(&key) {
                    *quantity = quantity.saturating_sub(claim.quantity.max(0));
                    if *quantity == 0 {
                        self.reserved_by_key.remove(&key);
                    }
                }
            }
        }
        let movement = self.movements.get_mut(&movement_id)?;
        movement.status = status;
        movement.updated_at_unix = now_unix;
        Some(movement.clone())
    }

    pub fn reconcile(
        &mut self,
        movement_id: Uuid,
        reason: &str,
        now_unix: i64,
    ) -> Option<InventoryMovement> {
        let reason = reason.trim();
        if reason.is_empty() {
            return None;
        }
        let movement = self.movements.get_mut(&movement_id)?;
        if !movement.context.is_object() {
            movement.context = serde_json::json!({});
        }
        let context = movement.context.as_object_mut()?;
        context
            .entry("reconciliationAudit")
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()?
            .push(serde_json::json!({"reason": reason, "recordedAtUnix": now_unix}));
        self.transition(
            movement_id,
            InventoryMovementStatus::NeedsReconciliation,
            now_unix,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(quantity: i64, observed_quantity: i64) -> ResolvedInventoryClaim {
        ResolvedInventoryClaim {
            claim: InventoryClaim {
                lot_id: None,
                source_kind: "cargo".into(),
                owner_id: "p1".into(),
                location_id: "sol".into(),
                item_id: "ore".into(),
                quantity,
            },
            observed_quantity,
        }
    }

    #[test]
    fn packages_are_atomic_and_terminal_transition_releases_claims() {
        let mut ledger = InventoryReservationLedger::default();
        let first = ledger.reserve(
            Uuid::nil(),
            "trade".into(),
            vec![claim(7, 10)],
            vec![],
            Value::Null,
            1,
        );
        assert!(first.accepted);
        assert!(
            !ledger
                .reserve(
                    Uuid::nil(),
                    "trade".into(),
                    vec![claim(4, 10)],
                    vec![],
                    Value::Null,
                    2
                )
                .accepted
        );
        let id = first.movement.unwrap().movement_id;
        ledger
            .transition(id, InventoryMovementStatus::Released, 3)
            .unwrap();
        assert!(
            ledger
                .reserve(
                    Uuid::nil(),
                    "trade".into(),
                    vec![claim(4, 10)],
                    vec![],
                    Value::Null,
                    4
                )
                .accepted
        );
    }
}
