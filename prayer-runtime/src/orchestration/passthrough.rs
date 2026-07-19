//! Passenger unload and typed passthrough command planning.

use super::*;

impl CommandPlanner {
    pub(super) fn start_unload_passenger(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let unload_all = self
            .command
            .args
            .first()
            .is_some_and(|arg| arg.as_text().eq_ignore_ascii_case("all"));
        if unload_all && passenger_manifest_observed_empty(state) {
            return Ok(complete(completed_with_message(
                "No passengers aboard; unload_passenger all already complete.",
            )));
        }
        if let Some(operation) = self.ensure_docked_step(state, false) {
            return Ok(operation);
        }
        required_text_arg(&self.command, 0, "unload_passenger")?;
        self.start_passthrough(state)
    }

    pub(super) fn start_passthrough(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let spec = resolve_command(&self.command.action)?;
        if spec.docking == DockingRequirement::DockableBase {
            if let Some(operation) = self.ensure_docked_step(state, false) {
                return Ok(operation);
            }
        }
        if self.command.action.eq_ignore_ascii_case("scrap_ship") && self.command.args.is_empty() {
            return Err(OperationFailure::InvalidIntent(
                "scrap_ship requires a ship id".to_string(),
            ));
        }
        let payload = if self.command.action.eq_ignore_ascii_case("craft")
            || self.command.action.eq_ignore_ascii_case("recycle")
        {
            let mut payload = craft_args_to_payload(&self.command.args)?;
            if self.command.action.eq_ignore_ascii_case("recycle") {
                if let Value::Object(map) = &mut payload {
                    if let Some(recipe) = map.remove("recipe_id") {
                        map.insert("id".into(), recipe);
                    }
                    map.remove("preset");
                }
            }
            payload
        } else if self.command.action.eq_ignore_ascii_case("trade_offer") {
            trade_offer_payload(&self.command.args)?
        } else {
            args_to_generated_payload(&self.command.action, &self.command.args, spec.definition)?
        };
        self.phase = Phase::AwaitFinalCall;
        Ok(RuntimeOperation::SpaceMoltAction {
            action: spec.key.to_string(),
            payload: Some(payload),
        })
    }
}

fn trade_offer_payload(args: &[ActionArg]) -> Result<Value, OperationFailure> {
    let encoded = args.first().map(ActionArg::as_text).ok_or_else(|| {
        OperationFailure::InvalidIntent("trade_offer requires an encoded request".into())
    })?;
    let request: prayer_actions::TradeOfferRequest = serde_json::from_str(&encoded)
        .map_err(|e| OperationFailure::InvalidIntent(format!("invalid trade offer: {e}")))?;
    let items = |values: Vec<prayer_actions::TradeItem>| {
        values
            .into_iter()
            .map(|v| serde_json::json!({"item_id": v.item.0, "quantity": v.quantity}))
            .collect::<Vec<_>>()
    };
    Ok(serde_json::json!({
        "target": request.target,
        "offer_items": items(request.offer_items),
        "offer_credits": request.offer_credits,
        "request_items": items(request.request_items),
        "request_credits": request.request_credits,
    }))
}
