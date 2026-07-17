//! Buy, sell, and order-cancellation planning.

use super::*;

impl CommandPlanner {
    pub(super) fn start_buy(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if let Some(op) = self.ensure_docked_step(state, true) {
            return Ok(op);
        }
        let item_id = required_text_arg(&self.command, 0, "buy")?.to_string();
        let requested = parse_positive_i64(&self.command, 1, 1);
        // Optional max price per unit: when present, the order only matches
        // asks at or below it and the quantity is pre-trimmed to that depth,
        // so a stale book can't make us overpay or rest a remainder.
        let max_price = optional_positive_i64(&self.command, 2);
        let place_order = explicit_buy_order_mode(&self.command)?;
        if place_order {
            let Some(price_each) = max_price else {
                return Err(OperationFailure::InvalidIntent(
                    "buy order mode requires a max price".to_string(),
                ));
            };
            let quantity = requested.max(1);
            let price_each = price_each.max(1);
            self.phase = Phase::AwaitBuyOrder {
                item_id: item_id.clone(),
                quantity,
                price_each,
            };
            let mut call = create_buy_order_call(&item_id, quantity, price_each);
            if let Some(destination) = self
                .command
                .args
                .get(4)
                .and_then(ActionArg::as_str)
                .and_then(|v| v.strip_prefix("deliver_to="))
            {
                if let RuntimeOperation::SpaceMoltAction {
                    payload: Some(payload),
                    ..
                } = &mut call
                {
                    payload["deliver_to"] = Value::String(destination.to_string());
                }
            }
            return Ok(call);
        }
        let sell_orders = state
            .market
            .sell_orders
            .get(item_id.as_str())
            .cloned()
            .unwrap_or_default();
        let buy_orders = state
            .market
            .buy_orders
            .get(item_id.as_str())
            .cloned()
            .unwrap_or_default();
        if sell_orders.is_empty() && buy_orders.is_empty() {
            return Ok(complete(completed_with_message(format!(
                "No market data for {item_id}."
            ))));
        }
        let available: i64 = sell_orders
            .iter()
            .filter(|o| max_price.is_none_or(|cap| o.price_each <= cap))
            .map(|o| o.quantity.max(0))
            .sum();
        // With a price cap there is no fallback to resting an order: if nothing
        // crosses at or below the cap, buy nothing rather than overpay.
        if let Some(cap) = max_price {
            if available <= 0 {
                return Ok(complete(completed_with_message(format!(
                    "No {item_id} sell orders at or below {cap}/unit."
                ))));
            }
        }
        let quantity = if available > 0 {
            requested.min(available).max(1)
        } else {
            requested.max(1)
        };
        let price_each = if let Some(cap) = max_price {
            cap.max(1)
        } else {
            let highest_buy = buy_orders.iter().map(|o| o.price_each).max();
            let lowest_sell = sell_orders.iter().map(|o| o.price_each).min();
            if highest_buy.is_none() && lowest_sell.is_none() {
                return Ok(complete(completed_with_message(format!(
                    "No price data for {item_id}."
                ))));
            }
            lowest_sell
                .or(highest_buy)
                .unwrap_or(DEFAULT_MARKET_PRICE_EACH)
                .max(1)
        };
        self.phase = Phase::AwaitBuyOrder {
            item_id: item_id.clone(),
            quantity,
            price_each,
        };
        Ok(create_buy_order_call(&item_id, quantity, price_each))
    }

    pub(super) fn continue_buy_order(
        &mut self,
        state: &PlanningState,
        item_id: String,
        quantity: i64,
        price_each: i64,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let value = match buy_order_outcome(last)? {
            BuyOrderOutcome::Placed(value) => {
                return Ok(complete(completed_with_api_message(&value)))
            }
            BuyOrderOutcome::Crossing(value) => value,
        };

        let mut conflicting = extract_crossing_order_ids(&value);
        if conflicting.is_empty() {
            conflicting = state
                .own_sell_orders
                .iter()
                .filter(|o| o.item_id == item_id && o.price_each <= price_each)
                .filter(|o| !o.order_id.trim().is_empty())
                .map(|o| o.order_id.clone())
                .collect::<Vec<_>>();
        }
        if conflicting.is_empty() {
            // Nothing to cancel — retry immediately (mirrors the empty loop).
            self.phase = Phase::AwaitFinalCall;
            return Ok(create_buy_order_call(&item_id, quantity, price_each));
        }
        Ok(self.issue_crossing_cancel(conflicting, 0, item_id, quantity, price_each))
    }

    pub(super) fn issue_crossing_cancel(
        &mut self,
        order_ids: Vec<String>,
        idx: usize,
        item_id: String,
        quantity: i64,
        price_each: i64,
    ) -> RuntimeOperation {
        let payload = serde_json::json!({ "order_id": order_ids[idx] });
        self.phase = Phase::CancelThenRetryBuy {
            order_ids,
            issued_idx: idx,
            item_id,
            quantity,
            price_each,
        };
        RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_market/cancel_order".to_string(),
            payload: Some(payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn continue_cancel_then_retry_buy(
        &mut self,
        order_ids: Vec<String>,
        issued_idx: usize,
        item_id: String,
        quantity: i64,
        price_each: i64,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        // Cancel responses are not inspected, but failures abort the tick.
        let _ = require_success(last)?;
        let next_idx = issued_idx + 1;
        if next_idx < order_ids.len() {
            return Ok(
                self.issue_crossing_cancel(order_ids, next_idx, item_id, quantity, price_each)
            );
        }
        self.phase = Phase::AwaitCrossingBuyRefresh {
            item_id,
            quantity,
            price_each,
        };
        Ok(RuntimeOperation::RefreshState)
    }

    pub(super) fn continue_crossing_buy_refresh(
        &mut self,
        state: &PlanningState,
        item_id: String,
        quantity: i64,
        price_each: i64,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let _ = require_success(last)?;
        let cargo_free = (state.cargo_capacity - state.cargo_used).max(0);
        let fittable = cargo_fittable_units(state, &item_id, cargo_free);
        let storage_available = state
            .storage_at_current_location()
            .and_then(|storage| storage.get(&item_id))
            .copied()
            .unwrap_or(0)
            .max(0);
        let withdraw_quantity = quantity.min(storage_available).min(fittable).max(0);
        if withdraw_quantity > 0 {
            let remaining_quantity = quantity - withdraw_quantity;
            self.phase = Phase::AwaitCrossingBuyWithdraw {
                item_id: item_id.clone(),
                remaining_quantity,
                price_each,
                withdrawn_quantity: withdraw_quantity,
            };
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt_storage/withdraw".to_string(),
                payload: Some(serde_json::json!({
                    "target": "self",
                    "item_id": item_id,
                    "quantity": withdraw_quantity
                })),
            });
        }

        self.phase = Phase::AwaitFinalCall;
        Ok(create_buy_order_call(&item_id, quantity, price_each))
    }

    pub(super) fn continue_crossing_buy_withdraw(
        &mut self,
        item_id: String,
        remaining_quantity: i64,
        price_each: i64,
        withdrawn_quantity: i64,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let value = require_success(last)?;
        if has_error_payload(&value) {
            return Ok(complete(completed_with_message(format!(
                "Storage withdrawal failed after canceling crossing sell order: {}",
                extract_result_message(&value)
                    .unwrap_or_else(|| "unknown upstream error".to_string())
            ))));
        }
        if remaining_quantity <= 0 {
            let message = extract_result_message(&value).unwrap_or_else(|| {
                format!("Withdrew {withdrawn_quantity} {item_id} from canceled sell order.")
            });
            return Ok(complete(completed_with_message(message)));
        }
        self.phase = Phase::AwaitFinalCall;
        Ok(create_buy_order_call(
            &item_id,
            remaining_quantity,
            price_each,
        ))
    }

    pub(super) fn start_sell(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if let Some(op) = self.ensure_docked_step(state, true) {
            return Ok(op);
        }
        let item = self.command.args.first().and_then(|a| a.as_str());
        // Optional quantity cap for a single item sale, matching buy's
        // `item quantity price` ordering.
        let max_quantity = item.and_then(|_| optional_positive_i64(&self.command, 1));
        // Optional minimum price per unit: only match buy orders at or above
        // it, and trim each stack to the depth available there so nothing
        // sells below the floor.
        let min_price = optional_positive_i64(&self.command, 2);
        let place_order = explicit_sell_order_mode(&self.command)?;
        if place_order {
            let Some(price_each) = min_price else {
                return Err(OperationFailure::InvalidIntent(
                    "sell order mode requires a min price".to_string(),
                ));
            };
            let targets: Vec<(String, i64, i64)> = sell_order_targets(state, item)
                .into_iter()
                .filter_map(|(item_id, raw_quantity)| {
                    let quantity = max_quantity.map_or(raw_quantity, |cap| raw_quantity.min(cap));
                    (quantity > 0).then_some((item_id, quantity, price_each.max(1)))
                })
                .collect();
            if targets.is_empty() {
                return Ok(complete(completed_with_message(
                    "No sellable cargo or storage.".to_string(),
                )));
            }
            return Ok(self.issue_sell(targets, 0));
        }
        let raw_targets = sell_targets(state, item);
        // Keep only targets with a usable price, mirroring the in-loop skip.
        let targets: Vec<(String, i64, i64)> = raw_targets
            .into_iter()
            .filter_map(|(item_id, raw_quantity)| {
                let quantity = max_quantity.map_or(raw_quantity, |cap| raw_quantity.min(cap));
                if quantity <= 0 {
                    return None;
                }
                let buy_orders = state.market.buy_orders.get(item_id.as_str());
                if let Some(floor) = min_price {
                    let depth: i64 = buy_orders
                        .into_iter()
                        .flatten()
                        .filter(|o| o.price_each >= floor)
                        .map(|o| o.quantity.max(0))
                        .sum();
                    if depth <= 0 {
                        return None;
                    }
                    return Some((item_id, quantity.min(depth), floor.max(1)));
                }
                let sell_orders = state.market.sell_orders.get(item_id.as_str());
                let highest_buy = buy_orders.into_iter().flatten().map(|o| o.price_each).max();
                let lowest_sell = sell_orders
                    .into_iter()
                    .flatten()
                    .map(|o| o.price_each)
                    .min();
                let price_each = highest_buy.or(lowest_sell)?.max(1);
                Some((item_id, quantity, price_each))
            })
            .collect();
        if targets.is_empty() {
            let message = match min_price {
                Some(floor) => {
                    format!("No cargo with buy orders at or above {floor}/unit.")
                }
                None => "No sellable cargo or storage.".to_string(),
            };
            return Ok(complete(completed_with_message(message)));
        }
        Ok(self.issue_sell(targets, 0))
    }

    pub(super) fn issue_sell(
        &mut self,
        targets: Vec<(String, i64, i64)>,
        idx: usize,
    ) -> RuntimeOperation {
        let (item_id, quantity, price_each) = &targets[idx];
        let payload = serde_json::json!({
            "item_id": item_id,
            "quantity": quantity,
            "price_each": price_each
        });
        self.phase = Phase::SellLoop {
            targets,
            issued_idx: idx,
        };
        RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_market/create_sell_order".to_string(),
            payload: Some(payload),
        }
    }

    pub(super) fn continue_sell(
        &mut self,
        targets: Vec<(String, i64, i64)>,
        issued_idx: usize,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let value = match last {
            Some(ApiOutcome::Failure(error))
                if error.server_code() == Some("insufficient_items") =>
            {
                let available = error
                    .upstream_message()
                    .and_then(available_quantity_from_insufficient_items);
                if let Some(available) = available.filter(|quantity| *quantity > 0) {
                    let mut targets = targets;
                    let requested = targets[issued_idx].1;
                    if available < requested {
                        targets[issued_idx].1 = available;
                        return Ok(self.issue_sell(targets, issued_idx));
                    }
                }
                return Err(error);
            }
            other => require_success(other)?,
        };
        let last_message = extract_result_message(&value);
        let next_idx = issued_idx + 1;
        if next_idx < targets.len() {
            return Ok(self.issue_sell(targets, next_idx));
        }
        let sold = targets.len();
        Ok(complete(completed_with_message(
            last_message
                .unwrap_or_else(|| format!("Finished selling cargo/storage ({sold} item types).")),
        )))
    }

    pub(super) fn start_cancel_orders(
        &mut self,
        state: &PlanningState,
        side: OrderSide,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if let Some(op) = self.ensure_docked_step(state, true) {
            return Ok(op);
        }
        let action = match side {
            OrderSide::Buy => "cancel_buy",
            OrderSide::Sell => "cancel_sell",
        };
        let item_id = required_text_arg(&self.command, 0, action)?.to_string();
        let orders: &[spacemolt_lib_rs::schema::ExchangeOrder] = match side {
            OrderSide::Buy => state.own_buy_orders.as_ref(),
            OrderSide::Sell => state.own_sell_orders.as_ref(),
        };
        let order_ids = orders
            .iter()
            .filter(|o| o.item_id == item_id && !o.order_id.trim().is_empty())
            .map(|o| o.order_id.clone())
            .collect::<Vec<_>>();

        if order_ids.is_empty() {
            return Ok(complete(completed_with_message(format!(
                "No open orders for {item_id}."
            ))));
        }
        Ok(self.issue_order_cancel(order_ids, 0, item_id, 0, Vec::new()))
    }

    pub(super) fn issue_order_cancel(
        &mut self,
        order_ids: Vec<String>,
        idx: usize,
        item_id: String,
        canceled: usize,
        errors: Vec<String>,
    ) -> RuntimeOperation {
        let payload = serde_json::json!({ "order_id": order_ids[idx] });
        self.phase = Phase::CancelOrdersLoop {
            order_ids,
            issued_idx: idx,
            item_id,
            canceled,
            errors,
        };
        RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_market/cancel_order".to_string(),
            payload: Some(payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn continue_cancel_orders(
        &mut self,
        order_ids: Vec<String>,
        issued_idx: usize,
        item_id: String,
        mut canceled: usize,
        mut errors: Vec<String>,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let value = require_success(last)?;
        if has_error_payload(&value) {
            if let Some(message) = extract_result_message(&value) {
                errors.push(message);
            }
        } else {
            canceled += 1;
        }
        let next_idx = issued_idx + 1;
        if next_idx < order_ids.len() {
            return Ok(self.issue_order_cancel(order_ids, next_idx, item_id, canceled, errors));
        }
        let total = order_ids.len();
        let mut message = format!("Canceled {canceled}/{total} order(s) for {item_id}.");
        if !errors.is_empty() {
            message.push_str(" Errors: ");
            message.push_str(&errors.join(" | "));
        }
        Ok(complete(completed_with_message(message)))
    }
}
