//! Battle stance, targeting, reload, attack, and scan planning.

use super::*;

impl CommandPlanner {
    pub(super) fn issue_battle_stance(&mut self, stance: &str) -> RuntimeOperation {
        self.phase = Phase::AwaitFinalCall;
        RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_battle/stance".to_string(),
            payload: Some(serde_json::json!({ "stance": stance })),
        }
    }

    pub(super) fn start_battle_stance(&mut self) -> Result<RuntimeOperation, OperationFailure> {
        let Some(stance) = self.command.args.first().map(ActionArg::as_text) else {
            return Err(OperationFailure::InvalidIntent(
                "stance requires fire, evade, brace, or flee".to_string(),
            ));
        };
        if !matches!(stance.as_str(), "fire" | "evade" | "brace" | "flee") {
            return Err(OperationFailure::InvalidIntent(format!(
                "unknown stance '{stance}'; expected fire, evade, brace, or flee"
            )));
        }
        Ok(self.issue_battle_stance(&stance))
    }

    pub(super) fn start_battle_target(&mut self) -> Result<RuntimeOperation, OperationFailure> {
        let Some(target) = self.command.args.first().map(ActionArg::as_text) else {
            return Err(OperationFailure::InvalidIntent(
                "target requires a target id or username".to_string(),
            ));
        };
        self.phase = Phase::AwaitFinalCall;
        Ok(RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_battle/target".to_string(),
            payload: Some(serde_json::json!({ "target_id": target })),
        })
    }

    pub(super) fn issue_simple_battle_action(&mut self, action: &str) -> RuntimeOperation {
        self.phase = Phase::AwaitFinalCall;
        RuntimeOperation::SpaceMoltAction {
            action: format!("spacemolt_battle/{action}"),
            payload: Some(serde_json::json!({})),
        }
    }

    pub(super) fn start_battle_reload(&mut self) -> Result<RuntimeOperation, OperationFailure> {
        let Some(weapon) = self.command.args.first().map(ActionArg::as_text) else {
            return Err(OperationFailure::InvalidIntent(
                "reload requires a weapon instance id and ammo item id".to_string(),
            ));
        };
        let Some(ammo) = self.command.args.get(1).map(ActionArg::as_text) else {
            return Err(OperationFailure::InvalidIntent(
                "reload requires a weapon instance id and ammo item id".to_string(),
            ));
        };
        self.phase = Phase::AwaitFinalCall;
        Ok(RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_battle/reload".to_string(),
            payload: Some(serde_json::json!({
                "weapon_instance_id": weapon,
                "ammo_item_id": ammo,
            })),
        })
    }

    pub(super) fn start_attack(&mut self) -> Result<RuntimeOperation, OperationFailure> {
        let Some(target) = self.command.args.first().map(ActionArg::as_text) else {
            return Err(OperationFailure::InvalidIntent(
                "attack requires a resolved target id".to_string(),
            ));
        };
        self.phase = Phase::AwaitFinalCall;
        Ok(RuntimeOperation::SpaceMoltAction {
            action: "spacemolt/attack".to_string(),
            payload: Some(serde_json::json!({ "target_id": target })),
        })
    }

    pub(super) fn start_scan(&mut self) -> RuntimeOperation {
        self.phase = Phase::AwaitFinalCall;
        let payload = self
            .command
            .args
            .first()
            .map(|target| serde_json::json!({ "id": target.as_text() }))
            .unwrap_or_else(|| serde_json::json!({}));
        RuntimeOperation::SpaceMoltAction {
            action: "spacemolt/scan".to_string(),
            payload: Some(payload),
        }
    }
}
