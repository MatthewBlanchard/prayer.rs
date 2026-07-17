//! Planning for `transfer` commands.
//!
//! Storage-direction transfers (cargo ↔ self ↔ faction ↔ player, plus credits)
//! are a thin wrapper over the v2 `storage` action: resolve the `from`/`to`
//! endpoints, enumerate the item list locally — the server has no "all" sweep —
//! and emit a single call, then surface its message. Space transfers use the
//! v2 storage wrapper too: `cargo → space` calls `spacemolt_storage/jettison`,
//! and `space → cargo` calls `spacemolt_storage/loot`.

use super::*;
use tracing::info;

#[derive(Debug, Clone)]
pub(super) struct SpaceLootTarget {
    pub(super) lootable_id: String,
    pub(super) item_id: Option<String>,
    pub(super) quantity: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransferItemRequest {
    item_id: Option<String>,
    quantity: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SpaceLootOutcome {
    Success(Option<String>),
    CargoFull(Option<String>),
    MissingLoot(Option<String>),
    OtherError(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TransferEndpointArg {
    Cargo,
    Storage,
    Faction,
    FactionTag(String),
    Player(String),
    Space(Option<String>),
    Commission(String),
}

impl TransferEndpointArg {
    fn as_text(&self) -> String {
        match self {
            Self::Cargo => "cargo".to_string(),
            Self::Storage => "storage".to_string(),
            Self::Faction => "faction".to_string(),
            Self::FactionTag(tag) => format!("faction:{tag}"),
            Self::Player(name) => format!("player:{name}"),
            Self::Space(Some(id)) => format!("space:{id}"),
            Self::Space(None) => "space".to_string(),
            Self::Commission(id) => format!("commission:{id}"),
        }
    }
}

impl CommandPlanner {
    /// Issue one `deposit`/`withdraw` storage call. `base` carries the routing
    /// fields (`target`, and `source` for deposits). Single-item transfers use
    /// the documented scalar shape; multi-item transfers use the batch `items`
    /// shape.
    fn issue_storage(
        &mut self,
        action: &'static str,
        mut base: Value,
        targets: Vec<(String, i64)>,
        all_cargo: bool,
        allow_no_space_success: bool,
    ) -> RuntimeOperation {
        if targets.is_empty() {
            let message = if all_cargo {
                "No cargo to deposit."
            } else {
                "Nothing to transfer."
            };
            return complete(completed_with_message(message));
        }
        let count = targets.len();
        match targets.as_slice() {
            [(item_id, quantity)] => {
                base["item_id"] = Value::String(item_id.clone());
                base["quantity"] = Value::Number(serde_json::Number::from(*quantity));
            }
            _ => {
                base["items"] = items_to_json(targets);
            }
        }
        self.phase = Phase::AwaitStorageBatch {
            count,
            all_cargo,
            allow_no_space_success,
        };
        RuntimeOperation::SpaceMoltAction {
            action: format!("spacemolt_storage/{action}"),
            payload: Some(base),
        }
    }

    pub(super) fn start_transfer(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let Some(kind) = self.command.args.first().and_then(ActionArg::as_str) else {
            return Err(OperationFailure::InvalidIntent(
                "transfer requires an encoded subject".to_string(),
            ));
        };

        match kind {
            "items" => {
                let Some(ActionArg::Integer(count)) = self.command.args.get(1) else {
                    return Err(OperationFailure::InvalidIntent(
                        "transfer items requires an item count".to_string(),
                    ));
                };
                let count = usize::try_from(*count).map_err(|_| {
                    OperationFailure::InvalidIntent(
                        "transfer items count must be positive".to_string(),
                    )
                })?;
                let (requests, from_idx) = transfer_items_arg(&self.command, count)?;
                let from = transfer_endpoint_arg(&self.command, from_idx)?;
                let to = transfer_endpoint_arg(&self.command, from_idx + 1)?;
                self.start_transfer_item_requests(state, requests, from, to)
            }
            "all" => {
                let from = transfer_endpoint_arg(&self.command, 1)?;
                let to = transfer_endpoint_arg(&self.command, 2)?;
                self.start_transfer_item_requests(
                    state,
                    vec![TransferItemRequest {
                        item_id: None,
                        quantity: None,
                    }],
                    from,
                    to,
                )
            }
            "item" => {
                let Some(item_id) = self.command.args.get(1).and_then(ActionArg::as_str) else {
                    return Err(OperationFailure::InvalidIntent(
                        "transfer item requires an item id".to_string(),
                    ));
                };
                let qty = transfer_quantity_arg(&self.command, 2);
                let from = transfer_endpoint_arg(&self.command, 3)?;
                let to = transfer_endpoint_arg(&self.command, 4)?;
                self.start_transfer_item_requests(
                    state,
                    vec![TransferItemRequest {
                        item_id: Some(item_id.to_string()),
                        quantity: qty,
                    }],
                    from,
                    to,
                )
            }
            "credits" => {
                let Some(ActionArg::Integer(quantity)) = self.command.args.get(1) else {
                    return Err(OperationFailure::InvalidIntent(
                        "transfer credits requires a quantity".to_string(),
                    ));
                };
                let from = transfer_endpoint_arg(&self.command, 2)?;
                let to = transfer_endpoint_arg(&self.command, 3)?;
                self.start_transfer_credits(state, *quantity, from, to)
            }
            "ship" => {
                let Some(ship_id) = self.command.args.get(1).and_then(ActionArg::as_str) else {
                    return Err(OperationFailure::InvalidIntent(
                        "transfer ship requires a ship id".to_string(),
                    ));
                };
                let from = transfer_endpoint_arg(&self.command, 2)?;
                let to = transfer_endpoint_arg(&self.command, 3)?;
                self.start_transfer_ship(state, ship_id.to_string(), from, to)
            }
            "module" => {
                let module_id = required_text_arg(&self.command, 1, "transfer module")?;
                let from = transfer_endpoint_arg(&self.command, 2)?;
                let to = transfer_endpoint_arg(&self.command, 3)?;
                let (TransferEndpointArg::Space(Some(wreck_id)), TransferEndpointArg::Cargo) =
                    (from, to)
                else {
                    return Err(OperationFailure::InvalidIntent(
                        "module transfers require 'from space <wreck_id> to cargo'".into(),
                    ));
                };
                self.phase = Phase::AwaitFinalCall;
                Ok(RuntimeOperation::SpaceMoltAction {
                    action: "spacemolt_storage/loot".into(),
                    payload: Some(
                        serde_json::json!({"wreck_id": wreck_id, "module_id": module_id}),
                    ),
                })
            }
            other => Err(OperationFailure::InvalidIntent(format!(
                "unknown transfer subject '{other}'"
            ))),
        }
    }

    fn start_transfer_item_requests(
        &mut self,
        state: &PlanningState,
        requests: Vec<TransferItemRequest>,
        from: TransferEndpointArg,
        to: TransferEndpointArg,
    ) -> Result<RuntimeOperation, OperationFailure> {
        info!(
            from = %from.as_text(),
            to = %to.as_text(),
            request_count = requests.len(),
            "transfer: planning item transfer"
        );

        if let TransferEndpointArg::Commission(commission_id) = &to {
            if from != TransferEndpointArg::Cargo {
                return Err(OperationFailure::InvalidIntent(
                    "commission materials must come from cargo".into(),
                ));
            }
            let request = single_optional_item_request(&requests)?;
            let item_id = request.item_id.clone().ok_or_else(|| {
                OperationFailure::InvalidIntent("commission transfer requires an item id".into())
            })?;
            let quantity = request.quantity.filter(|v| *v > 0).ok_or_else(|| {
                OperationFailure::InvalidIntent(
                    "commission transfer requires an explicit positive quantity".into(),
                )
            })?;
            self.phase = Phase::AwaitFinalCall;
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt_ship/supply_commission".into(),
                payload: Some(
                    serde_json::json!({"id": commission_id, "item_id": item_id, "quantity": quantity}),
                ),
            });
        }
        // Looting from space goes through the v2 storage wrapper.
        if let TransferEndpointArg::Space(id) = from {
            if to != TransferEndpointArg::Cargo {
                return Err(OperationFailure::InvalidIntent(format!(
                    "cannot transfer from space to {}",
                    to.as_text()
                )));
            }
            let request = single_optional_item_request(&requests)?;
            info!(
                space_id = id.as_deref().unwrap_or("(any)"),
                item_id = request.item_id.as_deref().unwrap_or("(all)"),
                quantity = ?request.quantity,
                visible_lootables = state.salvage.visible_lootables.len(),
                "transfer: planning space-to-cargo loot"
            );
            return Ok(self.start_transfer_from_space(
                state,
                id,
                request.item_id.clone(),
                request.quantity,
            ));
        }

        let all_items = is_all_items_request(&requests);
        let mut targets = transfer_source_targets(state, &from, &requests);

        // Jettisoning to space goes through the v2 storage wrapper too; only
        // cargo can be jettisoned.
        if let TransferEndpointArg::Space(pile) = &to {
            if pile.is_some() {
                return Err(OperationFailure::InvalidIntent(
                    "cannot transfer to a specific space pile".to_string(),
                ));
            }
            if from != TransferEndpointArg::Cargo {
                return Err(OperationFailure::InvalidIntent(format!(
                    "cannot transfer items from {}",
                    from.as_text()
                )));
            }
            ensure_explicit_quantities_available(state, &from, &requests)?;
            if targets.is_empty() {
                return Ok(complete(completed_with_message(if all_items {
                    "No cargo to jettison."
                } else {
                    "None of the specified items are in cargo."
                })));
            }
            info!(
                target_count = targets.len(),
                "transfer: planning cargo-to-space jettison"
            );
            return Ok(self.issue_transfer_space_jettison(targets, 0));
        }

        ensure_explicit_quantities_available(state, &from, &requests)?;

        if targets.is_empty() {
            return Ok(complete(completed_with_message(empty_transfer_message(
                &from, &requests, all_items,
            ))));
        }

        // Everything else is a single deposit/withdraw call. Both always dock.
        if let Some(op) = self.ensure_docked_step(state, false) {
            return Ok(op);
        }

        // Withdrawals into cargo are trimmed to what fits; the server rejects an
        // overfill rather than clamping it.
        if to == TransferEndpointArg::Cargo {
            let had_items = !targets.is_empty();
            targets = size_targets_to_cargo(state, targets);
            if had_items && targets.is_empty() {
                return Ok(complete(completed_with_message("Cargo full.")));
            }
        }

        let all_cargo = all_items && from == TransferEndpointArg::Cargo;
        if to == TransferEndpointArg::Cargo {
            // Withdraw: items land in cargo; `target` names the store to pull from.
            let Some(store) = store_token(&from) else {
                return Err(OperationFailure::InvalidIntent(format!(
                    "cannot transfer items from {}",
                    from.as_text()
                )));
            };
            Ok(self.issue_storage(
                "withdraw",
                serde_json::json!({ "target": store }),
                targets,
                all_cargo,
                all_items,
            ))
        } else {
            // Deposit: items leave `from` (source) and go to `to` (a store or gift).
            let Some(target) = store_token(&to) else {
                return Err(OperationFailure::InvalidIntent(format!(
                    "cannot transfer from {} to {}",
                    from.as_text(),
                    to.as_text()
                )));
            };
            let Some(source) = deposit_source_token(&from) else {
                return Err(OperationFailure::InvalidIntent(format!(
                    "cannot transfer items from {}",
                    from.as_text()
                )));
            };
            Ok(self.issue_storage(
                "deposit",
                serde_json::json!({ "source": source, "target": target }),
                targets,
                all_cargo,
                false,
            ))
        }
    }

    fn start_transfer_credits(
        &mut self,
        state: &PlanningState,
        quantity: i64,
        from: TransferEndpointArg,
        to: TransferEndpointArg,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if quantity <= 0 {
            return Err(OperationFailure::InvalidIntent(
                "transfer credits requires a positive quantity".to_string(),
            ));
        }
        // Credit transfers run through the treasury, which requires docking.
        if let Some(op) = self.ensure_docked_step(state, false) {
            return Ok(op);
        }
        // Credits use `cargo` as the player-held endpoint: deposit credits into
        // the faction treasury or gift them to another player, or withdraw
        // treasury credits back to the player.
        let (action, payload) = match (&from, &to) {
            (TransferEndpointArg::Faction, TransferEndpointArg::Cargo) => (
                "withdraw",
                serde_json::json!({
                    "target": "faction",
                    "item_id": "credits",
                    "quantity": quantity,
                }),
            ),
            (TransferEndpointArg::Cargo, _) => {
                let Some(target) = store_token(&to) else {
                    return Err(OperationFailure::InvalidIntent(format!(
                        "cannot transfer credits to {}",
                        to.as_text()
                    )));
                };
                (
                    "deposit",
                    serde_json::json!({
                        "source": "cargo",
                        "target": target,
                        "item_id": "credits",
                        "quantity": quantity,
                    }),
                )
            }
            _ => {
                return Err(OperationFailure::InvalidIntent(format!(
                    "cannot transfer credits from {} to {}",
                    from.as_text(),
                    to.as_text()
                )));
            }
        };
        self.phase = Phase::AwaitStorageBatch {
            count: 1,
            all_cargo: false,
            allow_no_space_success: false,
        };
        Ok(RuntimeOperation::SpaceMoltAction {
            action: format!("spacemolt_storage/{action}"),
            payload: Some(payload),
        })
    }

    fn start_transfer_ship(
        &mut self,
        _state: &PlanningState,
        ship_id: String,
        from: TransferEndpointArg,
        to: TransferEndpointArg,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if !matches!(
            from,
            TransferEndpointArg::Cargo | TransferEndpointArg::Storage
        ) {
            return Err(OperationFailure::InvalidIntent(format!(
                "cannot transfer ship from {}",
                from.as_text()
            )));
        }
        let is_faction_target = matches!(to, TransferEndpointArg::Faction);
        let target = match to {
            TransferEndpointArg::Player(player) => player,
            TransferEndpointArg::Faction => "faction".to_string(),
            _ => {
                return Err(OperationFailure::InvalidIntent(format!(
                    "cannot transfer ship to {}",
                    to.as_text()
                )));
            }
        };

        let mut payload = serde_json::json!({
            "target": target,
            "item_id": ship_id,
        });
        if matches!(from, TransferEndpointArg::Storage) && !is_faction_target {
            payload["source"] = Value::String("storage".to_string());
        }
        self.phase = Phase::AwaitStorageBatch {
            count: 1,
            all_cargo: false,
            allow_no_space_success: false,
        };
        Ok(RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_storage/deposit".to_string(),
            payload: Some(payload),
        })
    }

    fn issue_transfer_space_jettison(
        &mut self,
        targets: Vec<(String, i64)>,
        idx: usize,
    ) -> RuntimeOperation {
        let (item_id, qty) = &targets[idx];
        let payload = serde_json::json!({ "item_id": item_id, "quantity": qty });
        info!(
            issued_idx = idx,
            target_count = targets.len(),
            item_id,
            quantity = qty,
            action = "spacemolt_storage/jettison",
            "transfer: issuing cargo-to-space storage jettison"
        );
        self.phase = Phase::TransferSpaceJettisonLoop {
            targets,
            issued_idx: idx,
        };
        RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_storage/jettison".to_string(),
            payload: Some(payload),
        }
    }

    pub(super) fn continue_transfer_space_jettison(
        &mut self,
        targets: Vec<(String, i64)>,
        issued_idx: usize,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        info!(
            issued_idx,
            target_count = targets.len(),
            outcome = %api_outcome_label(last.as_ref()),
            "transfer: continuing cargo-to-space jettison"
        );
        let value = require_success(last)?;
        let last_message = extract_result_message(&value);
        if has_error_payload(&value) {
            let (item_id, _) = &targets[issued_idx];
            info!(
                issued_idx,
                item_id,
                message = last_message.as_deref().unwrap_or("(none)"),
                "transfer: cargo-to-space jettison error payload"
            );
            return Ok(complete(completed_with_message(format!(
                "Jettison stopped on {item_id}: {}",
                last_message.unwrap_or_else(|| "unknown upstream error".to_string())
            ))));
        }
        let next_idx = issued_idx + 1;
        if next_idx < targets.len() {
            return Ok(self.issue_transfer_space_jettison(targets, next_idx));
        }
        let count = targets.len();
        info!(
            target_count = count,
            message = last_message.as_deref().unwrap_or("(fallback)"),
            "transfer: cargo-to-space jettison complete"
        );
        Ok(complete(completed_with_message(
            last_message
                .unwrap_or_else(|| format!("Jettisoned all cargo stacks ({count} item types).")),
        )))
    }

    fn start_transfer_from_space(
        &mut self,
        state: &PlanningState,
        space_id: Option<String>,
        item_id: Option<String>,
        qty: Option<i64>,
    ) -> RuntimeOperation {
        info!(
            space_id = space_id.as_deref().unwrap_or("(any)"),
            item_id = item_id.as_deref().unwrap_or("(all)"),
            quantity = ?qty,
            visible_lootables = state.salvage.visible_lootables.len(),
            "transfer: resolving space loot targets"
        );
        if state.cargo_capacity <= state.cargo_used {
            info!("transfer: space-to-cargo loot skipped because cargo is full");
            return complete(completed_with_message("Cargo full."));
        }
        let targets = space_loot_targets(state, space_id, item_id.as_deref(), qty);
        info!(
            target_count = targets.len(),
            "transfer: resolved space loot targets"
        );
        if targets.is_empty() {
            let message = match item_id {
                Some(item_id) => format!("No {item_id} visible in space; waiting..."),
                None => "No visible space loot; waiting...".to_string(),
            };
            info!(message = %message, "transfer: no space loot targets, waiting");
            return RuntimeOperation::CompleteAfterWait {
                message,
                resume_after: TICK_PAUSE,
            };
        }
        self.issue_space_loot(targets, 0)
    }

    fn issue_space_loot(&mut self, targets: Vec<SpaceLootTarget>, idx: usize) -> RuntimeOperation {
        let target = &targets[idx];
        let mut payload = serde_json::json!({ "wreck_id": target.lootable_id });
        if let Some(item_id) = &target.item_id {
            payload["item_id"] = Value::String(item_id.clone());
        }
        if let Some(quantity) = target.quantity {
            payload["quantity"] = Value::Number(serde_json::Number::from(quantity));
        }
        info!(
            issued_idx = idx,
            target_count = targets.len(),
            wreck_id = %target.lootable_id,
            item_id = target.item_id.as_deref().unwrap_or("(all)"),
            quantity = ?target.quantity,
            action = "spacemolt_storage/loot",
            "transfer: issuing space-to-cargo storage loot"
        );
        self.phase = Phase::SpaceLootLoop {
            targets,
            issued_idx: idx,
        };
        RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_storage/loot".to_string(),
            payload: Some(payload),
        }
    }

    pub(super) fn continue_space_loot(
        &mut self,
        targets: Vec<SpaceLootTarget>,
        issued_idx: usize,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        info!(
            issued_idx,
            target_count = targets.len(),
            outcome = %api_outcome_label(last.as_ref()),
            "transfer: continuing space-to-cargo loot"
        );
        match space_loot_outcome(last)? {
            SpaceLootOutcome::CargoFull(message) => {
                info!(
                    issued_idx,
                    message = message.as_deref().unwrap_or("(fallback)"),
                    "transfer: space-to-cargo loot classified cargo-full"
                );
                return Ok(complete(completed_with_message(
                    message.unwrap_or_else(|| "Cargo full.".to_string()),
                )));
            }
            SpaceLootOutcome::MissingLoot(message) => {
                info!(
                    issued_idx,
                    wreck_id = %targets[issued_idx].lootable_id,
                    message = message.as_deref().unwrap_or("(fallback)"),
                    "transfer: space-to-cargo loot classified missing, waiting"
                );
                return Ok(RuntimeOperation::WaitTick {
                    message: message
                        .unwrap_or_else(|| space_loot_wait_message(&targets[issued_idx])),
                    resume_after: TICK_PAUSE,
                });
            }
            SpaceLootOutcome::OtherError(message) => {
                info!(
                    issued_idx,
                    wreck_id = %targets[issued_idx].lootable_id,
                    message = message.as_deref().unwrap_or("(fallback)"),
                    "transfer: space-to-cargo loot classified other error"
                );
                return Ok(complete(completed_with_message(format!(
                    "Space transfer stopped on {}: {}",
                    targets[issued_idx].lootable_id,
                    message.unwrap_or_else(|| "unknown error".to_string())
                ))));
            }
            SpaceLootOutcome::Success(last_message) => {
                let next_idx = issued_idx + 1;
                if next_idx < targets.len() {
                    info!(
                        issued_idx,
                        next_idx,
                        target_count = targets.len(),
                        message = last_message.as_deref().unwrap_or("(none)"),
                        "transfer: space-to-cargo loot advancing to next target"
                    );
                    return Ok(self.issue_space_loot(targets, next_idx));
                }
                info!(
                    issued_idx,
                    target_count = targets.len(),
                    message = last_message.as_deref().unwrap_or("(fallback)"),
                    "transfer: space-to-cargo loot complete"
                );
                Ok(complete(completed_with_message(
                    last_message.unwrap_or_else(|| "Transferred visible space loot.".to_string()),
                )))
            }
        }
    }
}

fn api_outcome_label(outcome: Option<&ApiOutcome>) -> &'static str {
    match outcome {
        Some(ApiOutcome::Success(_)) => "success",
        Some(ApiOutcome::Failure(error)) if error.is_network() && error.is_transient() => {
            "network-transient-failure"
        }
        Some(ApiOutcome::Failure(error)) if error.is_network() => "network-failure",
        Some(ApiOutcome::Failure(error)) if error.is_transient() => "transient-failure",
        Some(ApiOutcome::Failure(_)) => "failure",
        None => "none",
    }
}

fn space_loot_outcome(last: Option<ApiOutcome>) -> Result<SpaceLootOutcome, OperationFailure> {
    match last {
        Some(ApiOutcome::Success(value)) => {
            let message = extract_result_message(&value);
            if !has_error_payload(&value) {
                return Ok(SpaceLootOutcome::Success(message));
            }
            let code = error_code(&value).unwrap_or_default();
            if is_cargo_full_space_loot(&code) {
                return Ok(SpaceLootOutcome::CargoFull(message));
            }
            if is_missing_space_loot(&code, message.as_deref()) {
                return Ok(SpaceLootOutcome::MissingLoot(message));
            }
            Ok(SpaceLootOutcome::OtherError(message))
        }
        Some(ApiOutcome::Failure(error))
            if is_missing_space_loot(
                error.server_code().unwrap_or_default(),
                error.upstream_message(),
            ) =>
        {
            Ok(SpaceLootOutcome::MissingLoot(None))
        }
        Some(ApiOutcome::Failure(error))
            if error
                .upstream_message()
                .is_some_and(is_cargo_full_space_loot_message) =>
        {
            Ok(SpaceLootOutcome::CargoFull(None))
        }
        Some(ApiOutcome::Failure(error)) => Err(error),
        None => Err(OperationFailure::Policy(
            "planner expected an API response".to_string(),
        )),
    }
}

fn space_loot_wait_message(target: &SpaceLootTarget) -> String {
    format!(
        "Space loot target {} is not available; waiting...",
        target.lootable_id
    )
}

fn is_cargo_full_space_loot(code: &str) -> bool {
    let code = code.to_ascii_lowercase();
    code == "no_cargo_space" || code == "cargo_full" || code == "no_space"
}

fn is_cargo_full_space_loot_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("no_space")
        || message.contains("no cargo space")
        || message.contains("not enough cargo space")
        || message.contains("cargo full")
}

fn transfer_endpoint_arg(
    command: &ResolvedAction,
    idx: usize,
) -> Result<TransferEndpointArg, OperationFailure> {
    let Some(raw) = command.args.get(idx).and_then(ActionArg::as_str) else {
        return Err(OperationFailure::InvalidIntent(
            "transfer endpoint is missing".to_string(),
        ));
    };
    if let Some(tag) = raw.strip_prefix("faction:") {
        return Ok(TransferEndpointArg::FactionTag(tag.to_string()));
    }
    if let Some(name) = raw.strip_prefix("player:") {
        return Ok(TransferEndpointArg::Player(name.to_string()));
    }
    if let Some(id) = raw.strip_prefix("space:") {
        return Ok(TransferEndpointArg::Space(Some(id.to_string())));
    }
    if let Some(id) = raw.strip_prefix("commission:") {
        return Ok(TransferEndpointArg::Commission(id.to_string()));
    }
    match raw {
        "cargo" => Ok(TransferEndpointArg::Cargo),
        "storage" => Ok(TransferEndpointArg::Storage),
        "faction" => Ok(TransferEndpointArg::Faction),
        "space" => Ok(TransferEndpointArg::Space(None)),
        _ => Err(OperationFailure::InvalidIntent(format!(
            "unknown transfer endpoint '{raw}'"
        ))),
    }
}

fn transfer_quantity_arg(command: &ResolvedAction, idx: usize) -> Option<i64> {
    match command.args.get(idx) {
        Some(ActionArg::Integer(qty)) => Some(*qty),
        Some(arg) if arg.as_str() == Some("all") => None,
        Some(arg) => arg.as_str().and_then(|raw| raw.parse::<i64>().ok()),
        None => None,
    }
}

fn transfer_items_arg(
    command: &ResolvedAction,
    count: usize,
) -> Result<(Vec<TransferItemRequest>, usize), OperationFailure> {
    if count == 0 {
        return Err(OperationFailure::InvalidIntent(
            "transfer items count must be positive".to_string(),
        ));
    }
    let mut idx = 2usize;
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        let Some(item_id) = command.args.get(idx).and_then(ActionArg::as_str) else {
            return Err(OperationFailure::InvalidIntent(
                "transfer items requires an item id".to_string(),
            ));
        };
        let Some(ActionArg::Integer(quantity)) = command.args.get(idx + 1) else {
            return Err(OperationFailure::InvalidIntent(
                "transfer items requires an item quantity".to_string(),
            ));
        };
        if *quantity <= 0 {
            return Err(OperationFailure::InvalidIntent(
                "transfer item quantity must be positive".to_string(),
            ));
        }
        items.push(TransferItemRequest {
            item_id: Some(item_id.to_string()),
            quantity: Some(*quantity),
        });
        idx += 2;
    }
    Ok((items, idx))
}

fn is_all_items_request(requests: &[TransferItemRequest]) -> bool {
    matches!(
        requests,
        [TransferItemRequest {
            item_id: None,
            quantity: None,
        }]
    )
}

/// Expand the requested items into `(item_id, quantity)` pairs available at
/// `from`. The same resolver serves every source — cargo, station storage, and
/// faction storage — and both forms: an explicit item list or the `all`
/// sentinel (one request with no item id), which drains the whole source.
fn transfer_source_targets(
    state: &PlanningState,
    from: &TransferEndpointArg,
    requests: &[TransferItemRequest],
) -> Vec<(String, i64)> {
    let mut targets = Vec::new();
    for request in requests {
        match request.item_id.as_deref() {
            None => targets.extend(source_all_targets(state, from)),
            Some(item_id) => {
                let available = source_quantity(state, from, item_id);
                let quantity = request.quantity.unwrap_or(available).min(available).max(0);
                if quantity > 0 {
                    targets.push((item_id.to_string(), quantity));
                }
            }
        }
    }
    targets
}

/// Quantity of `item_id` currently held at `from`.
fn source_quantity(state: &PlanningState, from: &TransferEndpointArg, item_id: &str) -> i64 {
    match from {
        TransferEndpointArg::Cargo => state.cargo.get(item_id).copied().unwrap_or(0),
        TransferEndpointArg::Storage => state
            .current_poi
            .as_ref()
            .and_then(|poi| state.storage.get(poi))
            .and_then(|items| items.get(item_id))
            .copied()
            .unwrap_or(0),
        TransferEndpointArg::Faction => state.faction_storage.get(item_id).copied().unwrap_or(0),
        _ => 0,
    }
}

/// Every positive stack at `from`, sorted by item id for deterministic batches.
fn source_all_targets(state: &PlanningState, from: &TransferEndpointArg) -> Vec<(String, i64)> {
    let mut targets = match from {
        TransferEndpointArg::Cargo => positive_item_targets(&state.cargo),
        TransferEndpointArg::Storage => state
            .current_poi
            .as_ref()
            .and_then(|poi| state.storage.get(poi))
            .map(positive_item_targets)
            .unwrap_or_default(),
        TransferEndpointArg::Faction => positive_item_targets(&state.faction_storage),
        _ => Vec::new(),
    };
    targets.sort_by(|a, b| a.0.cmp(&b.0));
    targets
}

/// Trim `targets` to the stacks (and quantities) that fit in free cargo space,
/// charging each item its cargo size. Items are taken in order until full.
fn size_targets_to_cargo(state: &PlanningState, targets: Vec<(String, i64)>) -> Vec<(String, i64)> {
    let mut free = (state.cargo_capacity - state.cargo_used).max(0);
    let mut fitted = Vec::new();
    for (item_id, quantity) in targets {
        let item_size = state.item_cargo_size(&item_id).max(1);
        let units = (free / item_size).min(quantity);
        if units > 0 {
            free -= units * item_size;
            fitted.push((item_id, units));
        }
    }
    fitted
}

fn positive_item_targets(items: &std::collections::HashMap<String, i64>) -> Vec<(String, i64)> {
    items
        .iter()
        .filter_map(|(item_id, qty)| {
            if *qty > 0 {
                Some((item_id.clone(), *qty))
            } else {
                None
            }
        })
        .collect()
}

/// Identity of a store as used in a `target` field (a deposit destination) or as
/// the store a `withdraw` pulls from. Personal storage is `"self"`; cargo and the
/// credit cargo are not stores.
fn store_token(endpoint: &TransferEndpointArg) -> Option<String> {
    match endpoint {
        TransferEndpointArg::Storage => Some("self".to_string()),
        TransferEndpointArg::Faction => Some("faction".to_string()),
        TransferEndpointArg::FactionTag(tag) => Some(format!("faction:{tag}")),
        TransferEndpointArg::Player(name) => Some(name.clone()),
        _ => None,
    }
}

/// Origin of a `deposit` as used in the `source` field. Personal storage is
/// `"storage"` here (not `"self"`); item cargo and credit cargo both read as
/// `"cargo"`.
fn deposit_source_token(endpoint: &TransferEndpointArg) -> Option<String> {
    match endpoint {
        TransferEndpointArg::Cargo => Some("cargo".to_string()),
        TransferEndpointArg::Storage => Some("storage".to_string()),
        TransferEndpointArg::Faction => Some("faction".to_string()),
        _ => None,
    }
}

fn items_to_json(targets: Vec<(String, i64)>) -> Value {
    Value::Array(
        targets
            .into_iter()
            .map(|(item_id, quantity)| serde_json::json!({ "item_id": item_id, "quantity": quantity }))
            .collect(),
    )
}

fn ensure_explicit_quantities_available(
    state: &PlanningState,
    from: &TransferEndpointArg,
    requests: &[TransferItemRequest],
) -> Result<(), OperationFailure> {
    if !matches!(
        from,
        TransferEndpointArg::Cargo | TransferEndpointArg::Storage | TransferEndpointArg::Faction
    ) {
        return Ok(());
    }

    for request in requests {
        let (Some(item_id), Some(quantity)) = (request.item_id.as_deref(), request.quantity) else {
            continue;
        };
        let available = source_quantity(state, from, item_id);
        if available < quantity {
            return Err(OperationFailure::Policy(format!(
                "Cannot transfer {quantity} {item_id} from {}; only {available} available.",
                from.as_text()
            )));
        }
    }
    Ok(())
}

fn empty_transfer_message(
    from: &TransferEndpointArg,
    requests: &[TransferItemRequest],
    all_items: bool,
) -> String {
    if all_items {
        return match from {
            TransferEndpointArg::Cargo => "No cargo to transfer.".to_string(),
            _ => format!("No items in {}.", from.as_text()),
        };
    }

    let requested = requests
        .iter()
        .filter_map(|request| request.item_id.as_deref())
        .collect::<Vec<_>>();
    match requested.as_slice() {
        [item_id] => format!("No {item_id} in {}.", from.as_text()),
        [] => "Nothing to transfer.".to_string(),
        _ => format!("None of the specified items are in {}.", from.as_text()),
    }
}

fn single_optional_item_request(
    requests: &[TransferItemRequest],
) -> Result<&TransferItemRequest, OperationFailure> {
    match requests {
        [request] => Ok(request),
        _ => Err(OperationFailure::InvalidIntent(
            "this transfer source only supports one item request".to_string(),
        )),
    }
}

fn space_loot_targets(
    state: &PlanningState,
    space_id: Option<String>,
    item_id: Option<&str>,
    qty: Option<i64>,
) -> Vec<SpaceLootTarget> {
    let mut remaining = qty.unwrap_or(i64::MAX).max(0);
    let mut free = (state.cargo_capacity - state.cargo_used).max(0);
    let mut targets = Vec::new();
    for lootable in &state.salvage.visible_lootables {
        if let Some(space_id) = space_id.as_deref() {
            if lootable.id != space_id {
                continue;
            }
        }
        if let Some(item_id) = item_id {
            if remaining <= 0 || free <= 0 {
                break;
            }
            let available = lootable
                .cargo
                .iter()
                .filter(|item| item.item_id == item_id)
                .map(|item| item.quantity.max(0))
                .sum::<i64>();
            if available <= 0 {
                continue;
            }
            let quantity = available
                .min(remaining)
                .min(fittable_units(state, item_id, free));
            if quantity <= 0 {
                continue;
            }
            remaining -= quantity;
            free -= quantity * state.item_cargo_size(item_id).max(1);
            targets.push(SpaceLootTarget {
                lootable_id: lootable.id.clone(),
                item_id: Some(item_id.to_string()),
                quantity: Some(quantity),
            });
        } else {
            for item in lootable.cargo.iter().filter(|item| item.quantity > 0) {
                if free <= 0 {
                    break;
                }
                let quantity = item
                    .quantity
                    .min(fittable_units(state, &item.item_id, free));
                if quantity <= 0 {
                    continue;
                }
                free -= quantity * state.item_cargo_size(&item.item_id).max(1);
                targets.push(SpaceLootTarget {
                    lootable_id: lootable.id.clone(),
                    item_id: Some(item.item_id.clone()),
                    quantity: Some(quantity),
                });
            }
        }
    }
    targets
}

fn fittable_units(state: &PlanningState, item_id: &str, free: i64) -> i64 {
    free / state.item_cargo_size(item_id).max(1)
}

fn is_missing_space_loot(code: &str, message: Option<&str>) -> bool {
    let code = code.to_ascii_lowercase();
    if code == "not_found" || code.contains("not_found") {
        return true;
    }
    let message = message.unwrap_or_default().to_ascii_lowercase();
    message.contains("not_found") || message.contains("item not in wreck")
}

/// Surface the result of a `storage` batch call: the server's own message, or a
/// count-based fallback. Errors are reported, not retried.
pub(super) fn continue_storage_batch(
    count: usize,
    all_cargo: bool,
    allow_no_space_success: bool,
    last: Option<ApiOutcome>,
) -> Result<RuntimeOperation, OperationFailure> {
    let value = match last {
        Some(ApiOutcome::Success(value)) => value,
        Some(ApiOutcome::Failure(error))
            if allow_no_space_success
                && error
                    .upstream_message()
                    .is_some_and(is_no_space_storage_message) =>
        {
            return Ok(complete(completed_with_message("Cargo full.")));
        }
        Some(ApiOutcome::Failure(error)) => return Err(error),
        None => {
            return Err(OperationFailure::Policy(
                "planner expected an API response".to_string(),
            ));
        }
    };
    if has_error_payload(&value) {
        if allow_no_space_success && is_no_space_storage_value(&value) {
            return Ok(complete(completed_with_message("Cargo full.")));
        }
        return Ok(complete(completed_with_message(format!(
            "Storage transfer failed: {}",
            extract_result_message(&value).unwrap_or_else(|| "unknown upstream error".to_string())
        ))));
    }
    let fallback = if all_cargo {
        format!("Transferred all cargo stacks ({count} item types).")
    } else {
        format!("Transferred {count} item type(s).")
    };
    Ok(complete(completed_with_message(
        extract_result_message(&value).unwrap_or(fallback),
    )))
}

fn is_no_space_storage_value(value: &Value) -> bool {
    error_code(value)
        .as_deref()
        .is_some_and(is_no_space_storage_message)
        || extract_result_message(value)
            .as_deref()
            .is_some_and(is_no_space_storage_message)
}

fn is_no_space_storage_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("no_space")
        || message.contains("no cargo space")
        || message.contains("not enough cargo space")
        || message.contains("cargo full")
}
