use std::time::Duration;

use serde_json::Value;

use crate::engine::{CommandArg, EngineCommand, EngineExecutionResult, GameState, OpenOrderInfo};

use super::{SpaceMoltTransport, TransportError};

const DEFAULT_MARKET_PRICE_EACH: i64 = 1;

impl SpaceMoltTransport {
    pub(super) async fn execute_high_level(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<Option<EngineExecutionResult>, TransportError> {
        let action = command.action.to_ascii_lowercase();

        match action.as_str() {
            "wait" => Ok(Some(self.handle_wait(command).await)),
            "mine" => Ok(Some(self.handle_mine(command, runtime_state).await?)),
            "refuel" => Ok(Some(self.handle_refuel(runtime_state).await?)),
            "explore" => Ok(Some(self.handle_explore(runtime_state).await?)),
            "go" => Ok(Some(self.handle_go(command, runtime_state).await?)),
            "set_home" => Ok(Some(self.handle_set_home(runtime_state).await?)),
            "retrieve" => Ok(Some(self.handle_retrieve(command, runtime_state).await?)),
            "stash" => Ok(Some(self.handle_stash(command, runtime_state).await?)),
            "buy" => Ok(Some(self.handle_buy(command, runtime_state).await?)),
            "sell" => Ok(Some(self.handle_sell(command, runtime_state).await?)),
            "cancel_buy" => Ok(Some(self.handle_cancel_buy(command, runtime_state).await?)),
            "cancel_sell" => Ok(Some(self.handle_cancel_sell(command, runtime_state).await?)),
            "jettison" => Ok(Some(self.handle_jettison(command, runtime_state).await?)),
            _ => Ok(None),
        }
    }

    async fn handle_wait(&self, command: &EngineCommand) -> EngineExecutionResult {
        let ticks = parse_wait_ticks(command);
        tokio::time::sleep(Duration::from_secs(ticks * 10)).await;
        completed_with_message(format!("Waited {ticks} tick(s)."))
    }

    async fn handle_explore(
        &self,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "explore")?;
        if state.docked {
            let _ = self.execute_api("undock", None).await?;
            return Ok(incomplete_with_message("Undocking to explore..."));
        }

        let Some(current_system) = state.system.as_deref() else {
            return Err(TransportError::UnsupportedCommand(
                "Can't explore: current system is unknown.".to_string(),
            ));
        };

        if let Some(target_poi) = nearest_unvisited_poi_in_system(state, current_system) {
            if state.current_poi.as_deref() != Some(target_poi.as_str()) {
                let _ = self
                    .execute_api(
                        "travel",
                        Some(serde_json::json!({ "target_poi": target_poi })),
                    )
                    .await?;
                return Ok(incomplete_with_message(format!(
                    "Exploring `{current_system}`..."
                )));
            }
        }

        if !state.galaxy.surveyed_systems.contains(current_system) {
            let _ = self.execute_api("survey_system", None).await;
        }

        let candidate_systems = ordered_explore_target_systems(state, current_system);
        if candidate_systems.is_empty() {
            return Ok(EngineExecutionResult {
                result_message: Some("No unexplored systems or locations found anywhere in the galaxy!".to_string()),
                completed: true,
                halt_script: true,
            });
        }

        if let Some((target_system, next_hop)) =
            choose_reachable_explore_jump_target(state, current_system, &candidate_systems)
        {
            let _ = self
                .execute_api(
                    "jump",
                    Some(serde_json::json!({ "target_system": next_hop })),
                )
                .await?;
            return Ok(incomplete_with_message(format!(
                "Exploring `{target_system}`..."
            )));
        }

        if candidate_systems.iter().all(|s| s == current_system) {
            return Ok(incomplete_with_message(format!(
                "Exploring `{current_system}`..."
            )));
        }

        Ok(EngineExecutionResult {
            result_message: Some("No unexplored systems or locations found anywhere in the galaxy!".to_string()),
            completed: true,
            halt_script: true,
        })
    }

    async fn handle_go(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "go")?;
        let target = required_text_arg(command, 0, "go")?;
        let (target_system, target_poi) = resolve_go_target(state, target)?;

        if let Some(ref poi_id) = target_poi {
            if state.current_poi.as_deref() == Some(poi_id.as_str()) {
                return Ok(completed_with_message(format!("Already at {target}.")));
            }
        } else if state.system.as_deref() == Some(target_system.as_str()) {
            return Ok(completed_with_message(format!("Already at {target}.")));
        }

        self.step_toward_target(state, target, &target_system, target_poi.as_deref())
            .await
    }

    async fn handle_set_home(
        &self,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "set_home")?;
        if let Some(result) = self.ensure_docked(state, false, "set_home").await? {
            return Ok(result);
        }
        let Some(base_id) = state.current_poi.as_deref() else {
            return Err(TransportError::UnsupportedCommand(
                "Can't set home base: current location is unknown.".to_string(),
            ));
        };
        let value = self
            .execute_api(
                "set_home_base",
                Some(serde_json::json!({ "base_id": base_id })),
            )
            .await?;
        Ok(completed_with_api_message(&value))
    }

    async fn handle_mine(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "mine")?;
        if state.cargo_pct >= 100 {
            return Ok(completed_with_message("Cargo is full."));
        }

        let resource = command_arg_text_at(command, 0);
        let Some(target_poi) = nearest_mining_poi(state, resource) else {
            return Ok(EngineExecutionResult {
                result_message: Some(match resource {
                    Some(resource) => format!("No known minable locations for {resource} anywhere in the galaxy!"),
                    None => "No known minable locations anywhere in the galaxy!".to_string(),
                }),
                completed: true,
                halt_script: true,
            });
        };

        if state.current_poi.as_deref() == Some(target_poi.as_str()) {
            let value = self.execute_api("mine", None).await?;
            if is_mine_depleted(&value) {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            return Ok(incomplete_with_api_message(&value));
        }

        let target_system = state
            .galaxy
            .poi_system
            .get(target_poi.as_str())
            .cloned()
            .or_else(|| state.system.clone())
            .unwrap_or_else(|| target_poi.clone());
        self.step_toward_target(
            state,
            &target_poi,
            &target_system,
            Some(target_poi.as_str()),
        )
        .await
    }

    async fn handle_refuel(
        &self,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "refuel")?;
        if state.fuel_pct >= 100 {
            return Ok(completed_with_message("Fuel already full."));
        }

        let Some((target_system, target_poi)) = nearest_refuel_station(state) else {
            return Ok(EngineExecutionResult {
                result_message: Some("No known refueling station anywhere in the galaxy!".to_string()),
                completed: true,
                halt_script: true,
            });
        };

        if state.system.as_deref() != Some(target_system.as_str())
            || state.current_poi.as_deref() != Some(target_poi.as_str())
        {
            return self
                .step_toward_target(
                    state,
                    target_poi.as_str(),
                    target_system.as_str(),
                    Some(target_poi.as_str()),
                )
                .await;
        }

        if !state.docked {
            let _ = self.execute_api("dock", None).await?;
            return Ok(incomplete_with_message(format!(
                "Docking at `{target_poi}` to refuel..."
            )));
        }

        if state.fuel_pct >= 100 {
            return Ok(completed_with_message("Fuel already full."));
        }

        let value = self
            .execute_api("refuel", Some(serde_json::json!({})))
            .await?;
        Ok(completed_with_api_message(&value))
    }

    async fn step_toward_target(
        &self,
        state: &GameState,
        target_label: &str,
        target_system: &str,
        target_poi: Option<&str>,
    ) -> Result<EngineExecutionResult, TransportError> {
        if state.docked {
            let _ = self.execute_api("undock", None).await?;
            return Ok(incomplete_with_message("Undocking..."));
        }

        if state.system.as_deref() == Some(target_system) {
            if let Some(poi_id) = target_poi {
                let _ = self
                    .execute_api("travel", Some(serde_json::json!({ "target_poi": poi_id })))
                    .await?;
                return Ok(incomplete_with_message(format!(
                    "Traveling to {target_label}..."
                )));
            }
            return Ok(completed_with_message(format!(
                "Arrived at {target_label}."
            )));
        }

        let Some(next_hop) = state
            .system
            .as_deref()
            .and_then(|s| state.galaxy.next_hop_toward(s, target_system))
        else {
            return Err(TransportError::UnsupportedCommand(format!(
                "Can't reach {target_label} — no known route from current system."
            )));
        };
        let _ = self
            .execute_api(
                "jump",
                Some(serde_json::json!({ "target_system": next_hop })),
            )
            .await?;
        Ok(incomplete_with_message(format!(
            "Jumping toward {target_label}..."
        )))
    }

    async fn handle_retrieve(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "retrieve")?;
        if let Some(result) = self.ensure_docked(state, false, "retrieve").await? {
            return Ok(result);
        }
        let item_id = required_text_arg(command, 0, "retrieve")?;
        let available = state
            .current_poi
            .as_ref()
            .and_then(|poi| state.stash.get(poi))
            .and_then(|items| items.get(item_id))
            .copied()
            .unwrap_or(0);
        if available <= 0 {
            return Ok(completed_with_message(format!(
                "No {item_id} in storage."
            )));
        }
        let requested = parse_positive_i64(command, 1, available);
        let cargo_free = (state.cargo_capacity - state.cargo_used).max(0);
        if cargo_free <= 0 {
            return Ok(completed_with_message("Cargo full."));
        }
        let mut quantity = requested.min(available).min(cargo_free).max(1);
        let mut value = Value::Null;
        while quantity > 0 {
            value = self
                .execute_api(
                    "withdraw_items",
                    Some(serde_json::json!({ "item_id": item_id, "quantity": quantity })),
                )
                .await?;
            let code = error_code(&value).unwrap_or_default().to_ascii_lowercase();
            let no_cargo_space = code == "no_cargo_space" || code == "cargo_full";
            if !no_cargo_space || quantity == 1 {
                break;
            }
            quantity = (quantity / 2).max(1);
        }
        Ok(completed_with_api_message(&value))
    }

    async fn handle_stash(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "stash")?;
        if let Some(result) = self.ensure_docked(state, false, "stash").await? {
            return Ok(result);
        }
        if let Some(item_id) = command_arg_text_at(command, 0) {
            let quantity = state.cargo.get(item_id).copied().unwrap_or(0).max(1);
            let value = self
                .execute_api(
                    "deposit_items",
                    Some(serde_json::json!({ "item_id": item_id, "quantity": quantity })),
                )
                .await?;
            return Ok(completed_with_api_message(&value));
        }

        let targets = sell_targets_for_all_cargo(&state.cargo, |_| true);
        if targets.is_empty() {
            return Ok(completed_with_message("No cargo to deposit."));
        }
        let mut deposited_count = 0usize;
        let mut last_message = None;
        for (item_id, qty) in targets {
            let value = self
                .execute_api(
                    "deposit_items",
                    Some(serde_json::json!({ "item_id": item_id, "quantity": qty })),
                )
                .await?;
            last_message = extract_result_message(&value);
            if has_error_payload(&value) {
                return Ok(completed_with_message(format!(
                    "Deposit cargo stopped on {item_id}: {}",
                    last_message.unwrap_or_else(|| "unknown transport error".to_string())
                )));
            }
            deposited_count += 1;
        }
        Ok(completed_with_message(last_message.unwrap_or_else(|| {
            format!("Deposited all cargo stacks ({deposited_count} item types).")
        })))
    }

    async fn handle_buy(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "buy")?;
        if let Some(result) = self.ensure_docked(state, true, "buy").await? {
            return Ok(result);
        }
        let item_id = required_text_arg(command, 0, "buy")?;
        let requested = parse_positive_i64(command, 1, 1);
        let cargo_free = (state.cargo_capacity - state.cargo_used).max(0);
        if cargo_free <= 0 {
            return Ok(completed_with_message("No cargo space."));
        }
        let sell_orders = state
            .market
            .sell_orders
            .get(item_id)
            .cloned()
            .unwrap_or_default();
        let buy_orders = state
            .market
            .buy_orders
            .get(item_id)
            .cloned()
            .unwrap_or_default();
        if sell_orders.is_empty() && buy_orders.is_empty() {
            return Ok(completed_with_message(format!(
                "No market data for {item_id}."
            )));
        }
        let available: i64 = sell_orders.iter().map(|o| o.quantity.max(0)).sum();
        let quantity = if available > 0 {
            requested.min(available).min(cargo_free).max(1)
        } else {
            requested.min(cargo_free).max(1)
        };
        let highest_buy = buy_orders.iter().map(|o| o.price_each).max();
        let lowest_sell = sell_orders.iter().map(|o| o.price_each).min();
        if highest_buy.is_none() && lowest_sell.is_none() {
            return Ok(completed_with_message(format!(
                "No price data for {item_id}."
            )));
        }
        let price_each = highest_buy
            .or(lowest_sell)
            .unwrap_or(DEFAULT_MARKET_PRICE_EACH)
            .max(1);
        let value = self
            .execute_api(
                "create_buy_order",
                Some(serde_json::json!({
                    "item_id": item_id,
                    "quantity": quantity,
                    "price_each": price_each
                })),
            )
            .await?;
        if error_code(&value).as_deref() == Some("crossing_order") {
            let mut conflicting = extract_crossing_order_ids(&value);
            if conflicting.is_empty() {
                conflicting = state
                    .own_sell_orders
                    .iter()
                    .filter(|o| o.item_id == item_id && o.price_each.floor() as i64 <= price_each)
                    .filter(|o| !o.order_id.trim().is_empty())
                    .map(|o| o.order_id.clone())
                    .collect::<Vec<_>>();
            }
            for order_id in conflicting {
                let _ = self
                    .execute_api(
                        "cancel_order",
                        Some(serde_json::json!({ "order_id": order_id })),
                    )
                    .await?;
            }
            let retry = self
                .execute_api(
                    "create_buy_order",
                    Some(serde_json::json!({
                        "item_id": item_id,
                        "quantity": quantity,
                        "price_each": price_each
                    })),
                )
                .await?;
            return Ok(completed_with_api_message(&retry));
        }
        Ok(completed_with_api_message(&value))
    }

    async fn handle_sell(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "sell")?;
        if let Some(result) = self.ensure_docked(state, true, "sell").await? {
            return Ok(result);
        }
        let targets = sell_targets(state, command_arg_text_at(command, 0));
        if targets.is_empty() {
            return Ok(completed_with_message("No sellable cargo."));
        }
        let single_item = command_arg_text_at(command, 0).is_some();
        let mut last_message = None;
        let mut sold = 0usize;
        for (item_id, quantity) in targets {
            let buy_orders = state
                .market
                .buy_orders
                .get(item_id.as_str())
                .cloned()
                .unwrap_or_default();
            let sell_orders = state
                .market
                .sell_orders
                .get(item_id.as_str())
                .cloned()
                .unwrap_or_default();
            let highest_buy = buy_orders.iter().map(|o| o.price_each).max();
            let lowest_sell = sell_orders.iter().map(|o| o.price_each).min();
            let Some(price_each) = highest_buy.or(lowest_sell).map(|p| p.max(1)) else {
                continue;
            };
            let value = self
                .execute_api(
                    "create_sell_order",
                    Some(serde_json::json!({
                        "item_id": item_id,
                        "quantity": quantity,
                        "price_each": price_each
                    })),
                )
                .await?;
            last_message = extract_result_message(&value);
            sold += 1;
            if single_item {
                return Ok(completed_with_api_message(&value));
            }
        }
        if sold == 0 {
            return Ok(completed_with_message("No sellable cargo."));
        }
        Ok(completed_with_message(last_message.unwrap_or_else(|| {
            format!("Finished selling cargo ({sold} item types).")
        })))
    }

    async fn handle_cancel_buy(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "cancel_buy")?;
        if let Some(result) = self.ensure_docked(state, true, "cancel_buy").await? {
            return Ok(result);
        }
        let item_id = required_text_arg(command, 0, "cancel_buy")?;
        let result = cancel_orders_for_item(self, item_id, state.own_buy_orders.as_ref()).await?;
        Ok(completed_with_message(result))
    }

    async fn handle_cancel_sell(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "cancel_sell")?;
        if let Some(result) = self.ensure_docked(state, true, "cancel_sell").await? {
            return Ok(result);
        }
        let item_id = required_text_arg(command, 0, "cancel_sell")?;
        let result = cancel_orders_for_item(self, item_id, state.own_sell_orders.as_ref()).await?;
        Ok(completed_with_message(result))
    }

    async fn handle_jettison(
        &self,
        command: &EngineCommand,
        runtime_state: Option<&GameState>,
    ) -> Result<EngineExecutionResult, TransportError> {
        let state = required_runtime_state(runtime_state, "jettison")?;

        if let Some(item_id) = command_arg_text_at(command, 0) {
            let quantity = state.cargo.get(item_id).copied().unwrap_or(0);
            if quantity <= 0 {
                return Ok(completed_with_message(format!("No {item_id} in cargo.")));
            }
            let value = self
                .execute_api(
                    "jettison",
                    Some(serde_json::json!({ "item_id": item_id, "quantity": quantity })),
                )
                .await?;
            return Ok(completed_with_api_message(&value));
        }

        let targets = sell_targets_for_all_cargo(&state.cargo, |_| true);
        if targets.is_empty() {
            return Ok(completed_with_message("No cargo to jettison."));
        }
        let mut jettisoned_count = 0usize;
        let mut last_message = None;
        for (item_id, qty) in targets {
            let value = self
                .execute_api(
                    "jettison",
                    Some(serde_json::json!({ "item_id": item_id, "quantity": qty })),
                )
                .await?;
            last_message = extract_result_message(&value);
            if has_error_payload(&value) {
                return Ok(completed_with_message(format!(
                    "Jettison stopped on {item_id}: {}",
                    last_message.unwrap_or_else(|| "unknown transport error".to_string())
                )));
            }
            jettisoned_count += 1;
        }
        Ok(completed_with_message(last_message.unwrap_or_else(|| {
            format!("Jettisoned all cargo stacks ({jettisoned_count} item types).")
        })))
    }

    async fn ensure_docked(
        &self,
        state: &GameState,
        requires_station: bool,
        _action: &str,
    ) -> Result<Option<EngineExecutionResult>, TransportError> {
        let Some(target_poi) = dock_target_in_current_system(state, requires_station) else {
            return Ok(Some(completed_with_message(
                "No dockable base available in the current system.".to_string(),
            )));
        };

        if state.current_poi.as_deref() != Some(target_poi.as_str()) {
            if state.docked {
                let _ = self.execute_api("undock", None).await?;
                return Ok(Some(incomplete_with_message(format!(
                    "Undocking to reach {target_poi}..."
                ))));
            }
            let _ = self
                .execute_api(
                    "travel",
                    Some(serde_json::json!({ "target_poi": target_poi })),
                )
                .await?;
            return Ok(Some(incomplete_with_message(format!(
                "Traveling to {target_poi}..."
            ))));
        }

        if !state.docked {
            let _ = self.execute_api("dock", None).await?;
            return Ok(Some(incomplete_with_message(format!(
                "Docking at {target_poi}..."
            ))));
        }

        Ok(None)
    }
}

async fn cancel_orders_for_item(
    transport: &SpaceMoltTransport,
    item_id: &str,
    orders: &[OpenOrderInfo],
) -> Result<String, TransportError> {
    let order_ids = orders
        .iter()
        .filter(|o| o.item_id == item_id && !o.order_id.trim().is_empty())
        .map(|o| o.order_id.as_str())
        .collect::<Vec<_>>();

    if order_ids.is_empty() {
        return Ok(format!("No open orders for {item_id}."));
    }

    let mut canceled = 0usize;
    let total = order_ids.len();
    let mut errors = Vec::new();
    for order_id in order_ids {
        let value = transport
            .execute_api(
                "cancel_order",
                Some(serde_json::json!({ "order_id": order_id })),
            )
            .await?;
        if has_error_payload(&value) {
            if let Some(message) = extract_result_message(&value) {
                errors.push(message);
            }
        } else {
            canceled += 1;
        }
    }

    let mut message = format!("Canceled {canceled}/{total} order(s) for {item_id}.");
    if !errors.is_empty() {
        message.push_str(" Errors: ");
        message.push_str(&errors.join(" | "));
    }
    Ok(message)
}

fn resolve_go_target(
    state: &GameState,
    target: &str,
) -> Result<(String, Option<String>), TransportError> {
    let target = if target.eq_ignore_ascii_case("home_base") {
        state.home_base.as_deref().unwrap_or(target)
    } else if target.eq_ignore_ascii_case("nearest_station") {
        state.nearest_station.as_deref().unwrap_or(target)
    } else {
        target
    };

    if state.galaxy.systems.iter().any(|s| s == target) {
        return Ok((target.to_string(), None));
    }

    let poi_candidate = resolve_poi_id(state, target);

    if let Some(poi_id) = poi_candidate {
        let system_id = state
            .galaxy
            .poi_system
            .get(poi_id.as_str())
            .cloned()
            .or_else(|| state.system.clone())
            .unwrap_or_else(|| target.to_string());
        return Ok((system_id, Some(poi_id)));
    }

    Err(TransportError::UnsupportedCommand(format!(
        "Unknown destination: '{target}'."
    )))
}

fn resolve_poi_id(state: &GameState, value: &str) -> Option<String> {
    if state.galaxy.poi_system.contains_key(value) {
        return Some(value.to_string());
    }
    state.galaxy.poi_base_to_id.get(value).cloned()
}

fn dock_target_in_current_system(state: &GameState, requires_station: bool) -> Option<String> {
    let current_system = state.system.as_deref()?;
    let candidates = if requires_station {
        state.galaxy.station_pois_by_system.get(current_system)?
    } else {
        state.galaxy.dockable_pois_by_system.get(current_system)?
    };
    if let Some(current) = state.current_poi.as_deref() {
        if candidates.iter().any(|c| c == current) {
            return Some(current.to_string());
        }
    }
    candidates.first().cloned()
}

fn nearest_mining_poi(state: &GameState, resource: Option<&str>) -> Option<String> {
    let candidates = if let Some(resource_id) = resource {
        state
            .galaxy
            .pois_by_resource
            .iter()
            .find_map(|(k, v)| {
                if k.eq_ignore_ascii_case(resource_id) {
                    Some(v.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    } else {
        let typed_mineable = state
            .galaxy
            .poi_type_by_id
            .iter()
            .filter_map(|(poi_id, poi_type)| {
                if is_mineable_poi_type(poi_type.as_str()) {
                    Some(poi_id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if typed_mineable.is_empty() {
            state.galaxy.pois.clone()
        } else {
            typed_mineable
        }
    };

    if candidates.is_empty() {
        return None;
    }

    let current_system = state.system.clone().unwrap_or_default();
    candidates
        .into_iter()
        .filter_map(|poi| {
            let poi_system = state
                .galaxy
                .poi_system
                .get(poi.as_str())
                .cloned()
                .unwrap_or_else(|| current_system.clone());
            let distance = state.galaxy.hop_distance(&current_system, &poi_system)?;
            Some((poi, distance))
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(poi, _)| poi)
}

fn nearest_refuel_station(state: &GameState) -> Option<(String, String)> {
    let current_system = state.system.as_deref()?;
    let mut best: Option<(usize, String, String)> = None;
    for (system_id, pois) in &state.galaxy.station_pois_by_system {
        for poi_id in pois {
            let distance = state.galaxy.hop_distance(current_system, system_id)?;
            let candidate = (distance, system_id.clone(), poi_id.clone());
            match &best {
                None => best = Some(candidate),
                Some(existing) if candidate < *existing => best = Some(candidate),
                _ => {}
            }
        }
    }
    best.map(|(_, system, poi)| (system, poi))
}

fn is_mineable_poi_type(poi_type: &str) -> bool {
    matches!(
        poi_type.to_ascii_lowercase().as_str(),
        "asteroid_belt" | "asteroid_field" | "asteroid_cluster" | "asteroid"
    )
}

fn ordered_explore_target_systems(state: &GameState, current_system: &str) -> Vec<String> {
    let mut out = Vec::new();
    if nearest_unvisited_poi_in_system(state, current_system).is_some() {
        out.push(current_system.to_string());
    }

    let mut candidates = state.galaxy.systems.clone();
    candidates.sort();
    candidates.sort_by_key(|system_id| {
        state
            .galaxy
            .hop_distance(current_system, system_id)
            .unwrap_or(usize::MAX / 2)
    });

    for system_id in candidates {
        if system_id == current_system {
            continue;
        }
        if nearest_unvisited_poi_in_system(state, system_id.as_str()).is_some()
            || !state.galaxy.explored_systems.contains(system_id.as_str())
        {
            out.push(system_id);
        }
    }

    out
}

fn choose_reachable_explore_jump_target(
    state: &GameState,
    current_system: &str,
    candidates: &[String],
) -> Option<(String, String)> {
    for target_system in candidates {
        if target_system == current_system {
            continue;
        }
        if let Some(next_hop) = state
            .system
            .as_deref()
            .and_then(|s| state.galaxy.next_hop_toward(s, target_system.as_str()))
        {
            return Some((target_system.clone(), next_hop));
        }
    }
    None
}

fn nearest_unvisited_poi_in_system(state: &GameState, system_id: &str) -> Option<String> {
    let current_poi = state.current_poi.as_deref();
    let mut candidates = known_pois_in_system(state, system_id)
        .into_iter()
        .filter(|poi_id| {
            !state.galaxy.visited_pois.contains(poi_id.as_str())
                && Some(poi_id.as_str()) != current_poi
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}

fn known_pois_in_system(state: &GameState, system_id: &str) -> Vec<String> {
    state
        .galaxy
        .poi_system
        .iter()
        .filter_map(|(poi_id, poi_system)| {
            if poi_system == system_id {
                Some(poi_id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn parse_wait_ticks(command: &EngineCommand) -> u64 {
    command_arg_i64_at(command, 0)
        .and_then(|v| u64::try_from(v).ok())
        .or_else(|| command_arg_text_at(command, 0)?.parse::<u64>().ok())
        .unwrap_or(1)
        .min(30)
}

fn parse_positive_i64(command: &EngineCommand, idx: usize, default: i64) -> i64 {
    let value = command_arg_i64_at(command, idx)
        .or_else(|| command_arg_text_at(command, idx)?.parse::<i64>().ok())
        .unwrap_or(default);
    value.max(1)
}

fn sell_targets(state: &GameState, item: Option<&str>) -> Vec<(String, i64)> {
    if let Some(item_id) = item {
        let quantity = state.cargo.get(item_id).copied().unwrap_or(0);
        if quantity > 0 && is_sellable(state, item_id) {
            return vec![(item_id.to_string(), quantity)];
        }
        return Vec::new();
    }

    sell_targets_for_all_cargo(&state.cargo, |item_id| is_sellable(state, item_id))
}

fn sell_targets_for_all_cargo<F>(
    cargo: &std::collections::HashMap<String, i64>,
    mut filter: F,
) -> Vec<(String, i64)>
where
    F: FnMut(&str) -> bool,
{
    cargo
        .iter()
        .filter_map(|(item_id, qty)| {
            let passes = filter(item_id.as_str());
            if *qty > 0 && passes {
                Some((item_id.clone(), *qty))
            } else {
                None
            }
        })
        .collect()
}

fn is_sellable(state: &GameState, item_id: &str) -> bool {
    state
        .market
        .buy_orders
        .get(item_id)
        .is_some_and(|orders| !orders.is_empty())
        || state
            .market
            .sell_orders
            .get(item_id)
            .is_some_and(|orders| !orders.is_empty())
}

fn required_runtime_state<'a>(
    runtime_state: Option<&'a GameState>,
    action: &str,
) -> Result<&'a GameState, TransportError> {
    runtime_state.ok_or_else(|| {
        TransportError::UnsupportedCommand(format!(
            "'{action}' requires game state — none available yet."
        ))
    })
}

fn completed_with_message(message: impl Into<String>) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: Some(message.into()),
        completed: true,
        halt_script: false,
    }
}

fn completed_with_api_message(value: &Value) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: extract_result_message(value),
        completed: true,
        halt_script: false,
    }
}

fn incomplete_with_message(message: impl Into<String>) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: Some(message.into()),
        completed: false,
        halt_script: false,
    }
}

fn incomplete_with_api_message(value: &Value) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: extract_result_message(value),
        completed: false,
        halt_script: false,
    }
}

fn has_error_payload(value: &Value) -> bool {
    value.get("error").is_some() || value.get("result").and_then(|v| v.get("error")).is_some()
}

fn is_mine_depleted(value: &Value) -> bool {
    let code = error_code(value).unwrap_or_default().to_ascii_lowercase();
    if code.contains("depleted") {
        return true;
    }
    extract_result_message(value)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("depleted")
}

fn error_code(value: &Value) -> Option<String> {
    value
        .get("error")
        .and_then(|e| {
            if let Some(code) = e.get("code").and_then(Value::as_str) {
                Some(code.to_string())
            } else {
                e.as_str().map(ToOwned::to_owned)
            }
        })
        .or_else(|| {
            value
                .get("result")
                .and_then(|r| r.get("error"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn extract_crossing_order_ids(value: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(error) = value.get("error") {
        collect_order_ids(error, &mut ids);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn collect_order_ids(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for key in ["order_id", "orderId"] {
                if let Some(id) = map.get(key).and_then(Value::as_str) {
                    let trimmed = id.trim();
                    if !trimmed.is_empty() {
                        out.push(trimmed.to_string());
                    }
                }
            }
            for nested in map.values() {
                collect_order_ids(nested, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_order_ids(item, out);
            }
        }
        _ => {}
    }
}

fn extract_result_message(value: &Value) -> Option<String> {
    value
        .get("result")
        .and_then(|v| {
            v.get("message")
                .or_else(|| v.get("error"))
                .or_else(|| v.get("status"))
        })
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn command_arg_text_at(command: &EngineCommand, idx: usize) -> Option<&str> {
    match command.args.get(idx)? {
        CommandArg::Any(v)
        | CommandArg::ItemId(v)
        | CommandArg::SystemId(v)
        | CommandArg::PoiId(v)
        | CommandArg::GoTarget(v)
        | CommandArg::ShipId(v)
        | CommandArg::ListingId(v)
        | CommandArg::MissionId(v)
        | CommandArg::ModuleId(v)
        | CommandArg::RecipeId(v) => Some(v.as_str()),
        CommandArg::Integer(_) => None,
    }
}

fn command_arg_i64_at(command: &EngineCommand, idx: usize) -> Option<i64> {
    match command.args.get(idx)? {
        CommandArg::Integer(v) => Some(*v),
        _ => None,
    }
}

fn required_text_arg<'a>(
    command: &'a EngineCommand,
    idx: usize,
    action: &str,
) -> Result<&'a str, TransportError> {
    command_arg_text_at(command, idx).ok_or_else(|| {
        TransportError::UnsupportedCommand(format!(
            "'{action}' is missing a required argument."
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_ticks_are_clamped_to_30() {
        let command = EngineCommand {
            action: "wait".to_string(),
            args: vec![CommandArg::Integer(99)],
            source_line: None,
        };
        assert_eq!(parse_wait_ticks(&command), 30);
    }

    #[test]
    fn sell_targets_without_arg_include_all_positive_cargo() {
        let state = GameState {
            cargo: std::sync::Arc::new(std::collections::HashMap::from([
                ("iron".to_string(), 3),
                ("water".to_string(), 0),
                ("fuel".to_string(), 2),
            ])),
            market: std::sync::Arc::new(crate::engine::MarketData {
                buy_orders: std::collections::HashMap::from([
                    (
                        "iron".to_string(),
                        vec![crate::engine::MarketOrderInfo {
                            price_each: 10,
                            quantity: 3,
                        }],
                    ),
                    (
                        "fuel".to_string(),
                        vec![crate::engine::MarketOrderInfo {
                            price_each: 5,
                            quantity: 2,
                        }],
                    ),
                ]),
                ..Default::default()
            }),
            ..GameState::default()
        };
        let targets = sell_targets(&state, None);
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&("iron".to_string(), 3)));
        assert!(targets.contains(&("fuel".to_string(), 2)));
    }

    #[test]
    fn sell_targets_with_arg_use_single_stack_quantity() {
        let state = GameState {
            cargo: std::sync::Arc::new(std::collections::HashMap::from([("iron".to_string(), 7)])),
            market: std::sync::Arc::new(crate::engine::MarketData {
                buy_orders: std::collections::HashMap::from([(
                    "iron".to_string(),
                    vec![crate::engine::MarketOrderInfo {
                        price_each: 10,
                        quantity: 7,
                    }],
                )]),
                ..Default::default()
            }),
            ..GameState::default()
        };
        let targets = sell_targets(&state, Some("iron"));
        assert_eq!(targets, vec![("iron".to_string(), 7)]);
    }

    #[test]
    fn next_hop_uses_system_connections() {
        let mut state = GameState {
            system: Some("sol".to_string()),
            ..GameState::default()
        };
        state.galaxy = std::sync::Arc::new(crate::engine::GalaxyData {
            system_connections: std::collections::HashMap::from([
                ("sol".to_string(), vec!["alpha".to_string()]),
                (
                    "alpha".to_string(),
                    vec!["sol".to_string(), "beta".to_string()],
                ),
                ("beta".to_string(), vec!["alpha".to_string()]),
            ]),
            ..Default::default()
        });
        assert_eq!(
            state
                .system
                .as_deref()
                .and_then(|s| state.galaxy.next_hop_toward(s, "beta")),
            Some("alpha".to_string())
        );
    }

    #[test]
    fn explore_target_prefers_current_system_when_unvisited_poi_exists() {
        let mut state = GameState {
            system: Some("sol".to_string()),
            current_poi: Some("poi_a".to_string()),
            ..GameState::default()
        };
        state.galaxy = std::sync::Arc::new(crate::engine::GalaxyData {
            systems: vec!["sol".to_string(), "alpha".to_string()],
            poi_system: std::collections::HashMap::from([
                ("poi_a".to_string(), "sol".to_string()),
                ("poi_b".to_string(), "sol".to_string()),
                ("poi_c".to_string(), "alpha".to_string()),
            ]),
            visited_pois: std::collections::HashSet::from(["poi_a".to_string()]),
            ..Default::default()
        });
        let targets = ordered_explore_target_systems(&state, "sol");
        assert_eq!(targets.first().map(String::as_str), Some("sol"));
    }

    #[test]
    fn explore_target_picks_nearest_unexplored_reachable_system() {
        let mut state = GameState {
            system: Some("sol".to_string()),
            ..GameState::default()
        };
        state.galaxy = std::sync::Arc::new(crate::engine::GalaxyData {
            systems: vec!["sol".to_string(), "alpha".to_string(), "beta".to_string()],
            system_connections: std::collections::HashMap::from([
                ("sol".to_string(), vec!["alpha".to_string()]),
                (
                    "alpha".to_string(),
                    vec!["sol".to_string(), "beta".to_string()],
                ),
                ("beta".to_string(), vec!["alpha".to_string()]),
            ]),
            explored_systems: std::collections::HashSet::from(["sol".to_string()]),
            ..Default::default()
        });
        let targets = ordered_explore_target_systems(&state, "sol");
        assert_eq!(targets.first().map(String::as_str), Some("alpha"));
    }

    #[test]
    fn explore_fallback_skips_unreachable_target() {
        let mut state = GameState {
            system: Some("sol".to_string()),
            ..GameState::default()
        };
        state.galaxy = std::sync::Arc::new(crate::engine::GalaxyData {
            systems: vec!["sol".to_string(), "alpha".to_string(), "beta".to_string()],
            system_connections: std::collections::HashMap::from([
                ("sol".to_string(), vec!["beta".to_string()]),
                ("beta".to_string(), vec!["sol".to_string()]),
            ]),
            explored_systems: std::collections::HashSet::from(["sol".to_string()]),
            ..Default::default()
        });
        let candidates = vec!["alpha".to_string(), "beta".to_string()];
        let choice = choose_reachable_explore_jump_target(&state, "sol", &candidates)
            .expect("should pick reachable fallback");
        assert_eq!(choice.0, "beta");
        assert_eq!(choice.1, "beta");
    }

    #[test]
    fn resolve_go_target_maps_base_id_to_poi_id() {
        let state = GameState {
            galaxy: std::sync::Arc::new(crate::engine::GalaxyData {
                poi_system: std::collections::HashMap::from([(
                    "poi_station_1".to_string(),
                    "sol".to_string(),
                )]),
                poi_base_to_id: std::collections::HashMap::from([(
                    "base_station_1".to_string(),
                    "poi_station_1".to_string(),
                )]),
                ..Default::default()
            }),
            ..GameState::default()
        };

        let (system_id, poi_id) =
            resolve_go_target(&state, "base_station_1").expect("go target should resolve");
        assert_eq!(system_id, "sol");
        assert_eq!(poi_id.as_deref(), Some("poi_station_1"));
    }

    #[test]
    fn resolve_go_target_errors_when_target_unknown() {
        let state = GameState::default();
        let err = resolve_go_target(&state, "missing_target").expect_err("expected error");
        assert!(err.to_string().contains("Unknown destination"));
    }

    #[test]
    fn nearest_refuel_station_prefers_lowest_hop_distance() {
        let mut state = GameState {
            system: Some("sol".to_string()),
            ..GameState::default()
        };
        state.galaxy = std::sync::Arc::new(crate::engine::GalaxyData {
            system_connections: std::collections::HashMap::from([
                ("sol".to_string(), vec!["alpha".to_string()]),
                (
                    "alpha".to_string(),
                    vec!["sol".to_string(), "beta".to_string()],
                ),
                ("beta".to_string(), vec!["alpha".to_string()]),
            ]),
            station_pois_by_system: std::collections::HashMap::from([
                ("beta".to_string(), vec!["poi_beta_station".to_string()]),
                ("alpha".to_string(), vec!["poi_alpha_station".to_string()]),
            ]),
            ..Default::default()
        });

        let choice = nearest_refuel_station(&state).expect("expected nearest station");
        assert_eq!(choice.0, "alpha");
        assert_eq!(choice.1, "poi_alpha_station");
    }

    #[test]
    fn nearest_mining_poi_prefers_known_mineable_types() {
        let mut state = GameState {
            system: Some("sol".to_string()),
            ..GameState::default()
        };
        state.galaxy = std::sync::Arc::new(crate::engine::GalaxyData {
            system_connections: std::collections::HashMap::from([
                ("sol".to_string(), vec!["alpha".to_string()]),
                ("alpha".to_string(), vec!["sol".to_string()]),
            ]),
            pois: vec!["poi_station".to_string(), "poi_asteroid".to_string()],
            poi_system: std::collections::HashMap::from([
                ("poi_station".to_string(), "sol".to_string()),
                ("poi_asteroid".to_string(), "alpha".to_string()),
            ]),
            poi_type_by_id: std::collections::HashMap::from([
                ("poi_station".to_string(), "station".to_string()),
                ("poi_asteroid".to_string(), "asteroid_field".to_string()),
            ]),
            ..Default::default()
        });

        assert_eq!(
            nearest_mining_poi(&state, None),
            Some("poi_asteroid".to_string())
        );
    }

    #[test]
    fn orchestrator_uses_csharp_trade_action_names() {
        let source = include_str!("orchestrator.rs");
        assert!(source.contains("\"create_buy_order\""));
        assert!(source.contains("\"create_sell_order\""));
        assert!(source.contains("\"cancel_order\","));
        assert!(!source.contains("execute_api(\"cancel_buy\""));
        assert!(!source.contains("execute_api(\"cancel_sell\""));
    }
}
