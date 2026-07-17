//! Engine-owned planning for high-level commands.
//!
//! `CommandPlanner` turns one engine command plus focused runtime reads and
//! persisted continuation state into a sequence of `RuntimeOperation`s for a
//! single tick. It is pure: no I/O, no async, no sleeps. The host (service)
//! executes operations against `spacemolt-lib-rs` and feeds each API response back
//! into the planner until the tick yields a result.

mod battle;
pub(crate) mod command_map;
mod context;
mod docking;
mod market;
mod mining;
mod navigation;
mod operation;
mod passthrough;
pub(crate) mod responses;
mod social;
mod transfer;

pub use operation::{ApiOutcome, RuntimeOperation, TICK_PAUSE};

use std::collections::HashSet;

use prayer_actions::{Action, ActionArg, ResolvedAction};
use serde_json::Value;

use crate::engine::{
    ActiveCommandState, EngineExecutionResult, GoState, MineState, RefuelState, WaitState,
};
use crate::navigation::{
    nearest_find_navigation_target, nearest_mining_poi, nearest_refuel_station,
    ordered_find_target_systems, resolve_go_target,
};
use crate::operation_failure::OperationFailure;
use crate::read_context::PlanningState;
use crate::read_context::RuntimeReadContext;
use tracing::debug;

use self::command_map::{args_to_generated_payload, craft_args_to_payload, resolve_command};
use self::context::Phase;
use self::responses::{
    complete, completed_with_api_message, completed_with_message, error_code,
    extract_crossing_order_ids, extract_result_message, halted_with_message, has_error_payload,
    incomplete_with_api_message, incomplete_with_message, is_mine_depleted,
};
use self::transfer::continue_storage_batch;

const DEFAULT_MARKET_PRICE_EACH: i64 = 1;

/// Plans the operations of one command tick.
pub struct CommandPlanner {
    command: ResolvedAction,
    continuation: Option<ActiveCommandState>,
    mining_blacklist: HashSet<String>,
    phase: Phase,
}

impl CommandPlanner {
    /// Begin planning a tick from a typed kernel action.
    pub fn from_action(
        action: Action,
        continuation: Option<ActiveCommandState>,
        mining_blacklist: HashSet<String>,
    ) -> Result<Self, crate::action_resolution::ActionBridgeError> {
        Ok(Self::new(
            crate::action_resolution::resolve_action(action)?,
            continuation,
            mining_blacklist,
        ))
    }

    /// Begin planning a tick for `command`.
    ///
    /// `continuation` is the engine-persisted state of the active command (if
    /// it has been ticking already).
    pub fn new(
        command: ResolvedAction,
        continuation: Option<ActiveCommandState>,
        mining_blacklist: HashSet<String>,
    ) -> Self {
        Self {
            command,
            continuation,
            mining_blacklist,
            phase: Phase::Start,
        }
    }

    /// Produce the next operation for this tick.
    ///
    /// `last` carries the outcome of the previous `SpaceMoltAction`, if one was
    /// issued. Returns an error to abort the tick — the host treats it like a
    /// operation failure (halt + requeue).
    #[cfg(test)]
    pub(crate) fn next(
        &mut self,
        state: &PlanningState,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let context = RuntimeReadContext::for_command(state, &self.command.action);
        self.next_with_context(&context, last)
    }

    pub fn next_with_context(
        &mut self,
        context: &RuntimeReadContext,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let state = context.planning_state();
        match std::mem::replace(&mut self.phase, Phase::Finished) {
            Phase::Start => self.start(context),
            Phase::AwaitPositioning { message } => {
                require_success(last)?;
                Ok(complete(incomplete_with_message(message)))
            }
            Phase::AwaitFinalCall => {
                let value = require_success(last)?;
                Ok(complete(completed_with_api_message(&value)))
            }
            Phase::TransferSpaceJettisonLoop {
                targets,
                issued_idx,
            } => self.continue_transfer_space_jettison(targets, issued_idx, last),
            Phase::SpaceLootLoop {
                targets,
                issued_idx,
            } => self.continue_space_loot(targets, issued_idx, last),
            Phase::AwaitTransitCall {
                destination,
                message,
            } => self.continue_transit_call(destination, message, last),
            Phase::AwaitTransitConfirm {
                destination,
                message,
                original_error,
            } => continue_transit_confirm(state, &destination, message, original_error, last),
            Phase::AwaitSurveyThenExplore { targets } => {
                // Survey results (and failures) are intentionally ignored;
                // the response only gates continuing exploration.
                self.find_explore(state, targets)
            }
            Phase::AwaitMineStrike { target_poi } => self.continue_mine(target_poi, last),
            Phase::AwaitStorageBatch {
                count,
                all_cargo,
                allow_no_space_success,
            } => continue_storage_batch(count, all_cargo, allow_no_space_success, last),
            Phase::AwaitBuyOrder {
                item_id,
                quantity,
                price_each,
            } => self.continue_buy_order(state, item_id, quantity, price_each, last),
            Phase::CancelThenRetryBuy {
                order_ids,
                issued_idx,
                item_id,
                quantity,
                price_each,
            } => self.continue_cancel_then_retry_buy(
                order_ids, issued_idx, item_id, quantity, price_each, last,
            ),
            Phase::AwaitCrossingBuyRefresh {
                item_id,
                quantity,
                price_each,
            } => self.continue_crossing_buy_refresh(state, item_id, quantity, price_each, last),
            Phase::AwaitCrossingBuyWithdraw {
                item_id,
                remaining_quantity,
                price_each,
                withdrawn_quantity,
            } => self.continue_crossing_buy_withdraw(
                item_id,
                remaining_quantity,
                price_each,
                withdrawn_quantity,
                last,
            ),
            Phase::SellLoop {
                targets,
                issued_idx,
            } => self.continue_sell(targets, issued_idx, last),
            Phase::CancelOrdersLoop {
                order_ids,
                issued_idx,
                item_id,
                canceled,
                errors,
            } => {
                self.continue_cancel_orders(order_ids, issued_idx, item_id, canceled, errors, last)
            }
            Phase::Finished => Err(OperationFailure::Policy(
                "command tick already finished".to_string(),
            )),
        }
    }

    /// Continuation state to persist back into the engine after this tick.
    pub fn continuation(&self) -> Option<ActiveCommandState> {
        self.continuation.clone()
    }

    fn start(
        &mut self,
        context: &RuntimeReadContext,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let state = context.planning_state();
        let actor = context.planning_state();
        let action = self.command.action.to_ascii_lowercase();

        if action == "halt" {
            return Ok(complete(EngineExecutionResult {
                result_message: Some("Script halted.".to_string()),
                completed: true,
                halt_script: true,
            }));
        }

        if actor.in_transit && !command_can_run_in_transit(&self.command) {
            return Ok(RuntimeOperation::WaitTick {
                message: transit_wait_message(state),
                resume_after: TICK_PAUSE,
            });
        }

        match action.as_str() {
            "wait" => Ok(self.start_wait()),
            "go" => self.start_go(state),
            "refuel" if self.command.args.is_empty() => self.start_refuel(state),
            "refuel" => self.start_passthrough(),
            "find" => self.start_find(state),
            "mine" => self.start_mine(state),
            "transfer" => self.start_transfer(state),
            "buy" => self.start_buy(state),
            "sell" => self.start_sell(state),
            "cancel_buy" => self.start_cancel_orders(state, OrderSide::Buy),
            "cancel_sell" => self.start_cancel_orders(state, OrderSide::Sell),
            "dock" => Ok(self.start_dock(state)),
            "set_home" => self.start_set_home(state),
            "say" => self.start_say(),
            "flee" => Ok(self.issue_battle_stance("flee")),
            "fight" => Ok(self.issue_battle_stance("fire")),
            "stance" => self.start_battle_stance(),
            "target" => self.start_battle_target(),
            "advance" | "retreat" => Ok(self.issue_simple_battle_action(&action)),
            "reload" => self.start_battle_reload(),
            "attack" => self.start_attack(),
            "scan" => Ok(self.start_scan()),
            "unload_passenger" => self.start_unload_passenger(state),
            _ => self.start_passthrough(),
        }
    }

    fn start_wait(&mut self) -> RuntimeOperation {
        let mut wait = match self.continuation.take() {
            Some(ActiveCommandState::Wait(wait)) => wait,
            _ => {
                let ticks = parse_wait_ticks(&self.command);
                WaitState {
                    total_ticks: ticks,
                    remaining_ticks: ticks,
                    origin: None,
                }
            }
        };
        if wait.remaining_ticks == 0 {
            let total = wait.total_ticks;
            self.continuation = Some(ActiveCommandState::Wait(wait));
            return complete(completed_with_message(format!("Waited {total} tick(s).")));
        }
        wait.remaining_ticks -= 1;
        let message = format!(
            "Waiting ({} tick(s) remaining)...",
            wait.remaining_ticks + 1
        );
        self.continuation = Some(ActiveCommandState::Wait(wait));
        RuntimeOperation::WaitTick {
            message,
            resume_after: TICK_PAUSE,
        }
    }
}

fn continue_transit_confirm(
    state: &PlanningState,
    destination: &str,
    message: String,
    original_error: OperationFailure,
    last: Option<ApiOutcome>,
) -> Result<RuntimeOperation, OperationFailure> {
    let refreshed = matches!(last, Some(ApiOutcome::Success(_)));
    let in_transit_to_destination = refreshed
        && state.in_transit
        && (state.transit_dest_system.as_deref() == Some(destination)
            || state.transit_dest_poi.as_deref() == Some(destination));
    if in_transit_to_destination {
        debug!(
            destination,
            error = %original_error,
            "transit request failed but state confirms transit started"
        );
        return Ok(complete(incomplete_with_message(message)));
    }
    Err(original_error)
}

#[derive(Debug, Clone, Copy)]
enum OrderSide {
    Buy,
    Sell,
}

fn create_buy_order_call(item_id: &str, quantity: i64, price_each: i64) -> RuntimeOperation {
    RuntimeOperation::SpaceMoltAction {
        action: "spacemolt_market/create_buy_order".to_string(),
        payload: Some(serde_json::json!({
            "item_id": item_id,
            "quantity": quantity,
            "price_each": price_each
        })),
    }
}

fn parse_positive_i64(command: &ResolvedAction, idx: usize, default: i64) -> i64 {
    let value = command
        .args
        .get(idx)
        .and_then(|arg| match arg {
            ActionArg::Integer(v) => Some(*v),
            other => other.as_str()?.parse::<i64>().ok(),
        })
        .unwrap_or(default);
    value.max(1)
}

/// Read an optional positive integer argument at `idx`; `None` when the arg is
/// absent or not a positive integer.
fn optional_positive_i64(command: &ResolvedAction, idx: usize) -> Option<i64> {
    command
        .args
        .get(idx)
        .and_then(|arg| match arg {
            ActionArg::Integer(v) => Some(*v),
            other => other.as_str()?.parse::<i64>().ok(),
        })
        .filter(|v| *v > 0)
}

fn explicit_buy_order_mode(command: &ResolvedAction) -> Result<bool, OperationFailure> {
    explicit_order_mode(command, 3, "buy")
}

fn explicit_sell_order_mode(command: &ResolvedAction) -> Result<bool, OperationFailure> {
    explicit_order_mode(command, 3, "sell")
}

fn explicit_order_mode(
    command: &ResolvedAction,
    idx: usize,
    verb: &str,
) -> Result<bool, OperationFailure> {
    let Some(mode) = command.args.get(idx).and_then(ActionArg::as_str) else {
        return Ok(false);
    };
    if matches!(mode, "order" | "rest" | "resting") {
        Ok(true)
    } else {
        Err(OperationFailure::InvalidIntent(format!(
            "unsupported {verb} mode '{mode}'"
        )))
    }
}

fn cargo_fittable_units(state: &PlanningState, item_id: &str, free: i64) -> i64 {
    free / state.item_cargo_size(item_id).max(1)
}

fn sell_targets(state: &PlanningState, item: Option<&str>) -> Vec<(String, i64)> {
    let available = sellable_inventory(state);
    if let Some(item_id) = item {
        let quantity = available.get(item_id).copied().unwrap_or(0);
        if quantity > 0 && is_sellable(state, item_id) {
            return vec![(item_id.to_string(), quantity)];
        }
        return Vec::new();
    }

    sell_targets_for_all_cargo(&available, |item_id| is_sellable(state, item_id))
}

fn sell_order_targets(state: &PlanningState, item: Option<&str>) -> Vec<(String, i64)> {
    let available = sellable_inventory(state);
    if let Some(item_id) = item {
        let quantity = available.get(item_id).copied().unwrap_or(0);
        if quantity > 0 {
            return vec![(item_id.to_string(), quantity)];
        }
        return Vec::new();
    }

    sell_targets_for_all_cargo(&available, |_| true)
}

fn sellable_inventory(state: &PlanningState) -> std::collections::HashMap<String, i64> {
    let mut available = state.cargo.as_ref().clone();
    if let Some(storage) = state.storage_at_current_location() {
        for (item_id, qty) in storage {
            *available.entry(item_id.clone()).or_insert(0) += qty;
        }
    }
    available
}

/// Extract the authoritative total from SpaceMolt's insufficient-items message.
/// Example: `You have 63 x Iron available (63 cargo, 0 storage). Need 64.`
fn available_quantity_from_insufficient_items(message: &str) -> Option<i64> {
    message
        .strip_prefix("You have ")?
        .split_whitespace()
        .next()?
        .parse::<i64>()
        .ok()
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

fn is_sellable(state: &PlanningState, item_id: &str) -> bool {
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

fn find_exhausted_result(targets: &[String]) -> EngineExecutionResult {
    let message = if targets.is_empty() {
        "No unvisited systems or locations found anywhere in the galaxy!".to_string()
    } else {
        format!(
            "Can't find {} anywhere in the known galaxy.",
            targets.join(", ")
        )
    };
    EngineExecutionResult {
        result_message: Some(message),
        completed: true,
        halt_script: true,
    }
}

enum DockPlan {
    /// Already docked at a suitable target — proceed with the command body.
    Ready,
    /// No dockable candidate in the current system.
    NoTarget,
    /// One positioning call moves us closer; the tick ends incomplete.
    Issue {
        action: &'static str,
        payload: Option<Value>,
        message: String,
    },
}

fn plan_ensure_docked(state: &PlanningState, requires_station: bool) -> DockPlan {
    let Some(target_poi) = dock_target_in_current_system(state, requires_station) else {
        return DockPlan::NoTarget;
    };

    if state.current_poi.as_deref() != Some(target_poi.as_str()) {
        if state.docked {
            return DockPlan::Issue {
                action: "undock",
                payload: None,
                message: format!("Undocking to reach {target_poi}..."),
            };
        }
        return DockPlan::Issue {
            action: "travel",
            payload: Some(serde_json::json!({ "target_poi": target_poi })),
            message: format!("Traveling to {target_poi}..."),
        };
    }

    if !state.docked {
        return DockPlan::Issue {
            action: "dock",
            payload: None,
            message: format!("Docking at {target_poi}..."),
        };
    }

    DockPlan::Ready
}

fn dock_target_in_current_system(state: &PlanningState, requires_station: bool) -> Option<String> {
    let current_system = state.system.as_deref()?;
    let candidates = state
        .galaxy
        .poi_records
        .values()
        .filter(|poi| {
            poi.system_id == current_system
                && if requires_station {
                    poi.info.poi_type.eq_ignore_ascii_case("station")
                } else {
                    poi.info.has_base || poi.info.base_id.is_some()
                }
        })
        .map(|poi| poi.id.clone())
        .collect::<Vec<_>>();
    if let Some(current) = state.current_poi.as_deref() {
        if candidates.iter().any(|c| c == current) {
            return Some(current.to_string());
        }
    }
    candidates.first().cloned()
}

/// Number of ticks a `wait` command spends, parsed from its first argument
/// (default 1, clamped to 30).
pub(crate) fn parse_wait_ticks(command: &ResolvedAction) -> u64 {
    command
        .args
        .first()
        .and_then(|arg| match arg {
            ActionArg::Integer(v) => u64::try_from(*v).ok(),
            other => other.as_str()?.parse::<u64>().ok(),
        })
        .unwrap_or(1)
        .min(30)
}

fn command_can_run_in_transit(command: &ResolvedAction) -> bool {
    if command.action.eq_ignore_ascii_case("scrap_ship") {
        return true;
    }

    command.action.eq_ignore_ascii_case("transfer")
        && command.args.first().and_then(ActionArg::as_str) == Some("ship")
        && command
            .args
            .get(3)
            .and_then(ActionArg::as_str)
            .is_some_and(|endpoint| endpoint.starts_with("player:"))
}

/// Status line shown while the ship is in transit.
pub(crate) fn transit_wait_message(state: &PlanningState) -> String {
    let destination = state
        .transit_dest_poi
        .as_deref()
        .or(state.transit_dest_system.as_deref())
        .unwrap_or("destination");
    let transit_type = state.transit_type.as_deref().unwrap_or("transit");
    format!("In {transit_type} to {destination}; waiting...")
}

fn require_success(last: Option<ApiOutcome>) -> Result<Value, OperationFailure> {
    match last {
        Some(ApiOutcome::Success(value)) => Ok(value),
        Some(ApiOutcome::Failure(error)) => Err(error),
        None => Err(OperationFailure::Policy(
            "planner expected an API response".to_string(),
        )),
    }
}

enum BuyOrderOutcome {
    Placed(Value),
    Crossing(Value),
}

fn buy_order_outcome(last: Option<ApiOutcome>) -> Result<BuyOrderOutcome, OperationFailure> {
    match last {
        Some(ApiOutcome::Success(value)) => {
            if error_code(&value).as_deref() == Some("crossing_order") {
                Ok(BuyOrderOutcome::Crossing(value))
            } else {
                Ok(BuyOrderOutcome::Placed(value))
            }
        }
        Some(ApiOutcome::Failure(error)) => {
            if error.server_code() == Some("crossing_order") {
                if let Some(value) = error.structured_error_payload() {
                    return Ok(BuyOrderOutcome::Crossing(value));
                }
            }
            Err(error)
        }
        None => Err(OperationFailure::Policy(
            "planner expected an API response".to_string(),
        )),
    }
}

fn is_mine_cargo_full_payload(value: &Value) -> bool {
    error_code(value)
        .map(|code| is_mine_cargo_full_code(&code))
        .unwrap_or(false)
        || extract_result_message(value)
            .map(|message| is_mine_cargo_full_message(&message))
            .unwrap_or(false)
}

fn is_mine_cargo_full_code(code: &str) -> bool {
    matches!(
        code.to_ascii_lowercase().as_str(),
        "cargo_full" | "no_cargo_space" | "no_space"
    )
}

fn is_mine_cargo_full_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("cargo_full")
        || message.contains("no_space")
        || message.contains("cargo hold is full")
        || message.contains("cargo is full")
        || message.contains("cargo full")
        || message.contains("no cargo space")
        || message.contains("not enough cargo space")
}

fn is_mine_depleted_message(message: &str) -> bool {
    message.to_ascii_lowercase().contains("depleted")
}

fn passenger_manifest_observed_empty(state: &PlanningState) -> bool {
    state.passengers.aboard_count == Some(0)
}

fn known_find_target(state: &PlanningState, targets: &[String]) -> Option<String> {
    for target in targets {
        if let Some((resource_id, pois)) = known_resource_locations(state, target) {
            return Some(format!(
                "Found resource `{resource_id}` at {}.",
                pois.join(", ")
            ));
        }
        if let Some((poi_id, system_id)) = known_poi_location(state, target) {
            return Some(format!("Found POI `{poi_id}` in `{system_id}`."));
        }
    }
    None
}

fn unknown_find_target_message(state: &PlanningState, targets: &[String]) -> Option<String> {
    if targets.is_empty() {
        return None;
    }
    let all_resource_ids = state
        .galaxy
        .poi_records
        .values()
        .flat_map(|poi| {
            poi.resources
                .iter()
                .map(|resource| resource.resource_id.as_str())
        })
        .collect::<Vec<_>>();
    let all_item_ids: Vec<&str> = state.catalog.items.keys().map(String::as_str).collect();
    // Only validate if we have catalog data to validate against
    if all_item_ids.is_empty() {
        return None;
    }
    let mut unknown = Vec::new();
    for target in targets {
        let target_lc = target.to_ascii_lowercase();
        let matches_resource = all_resource_ids
            .iter()
            .any(|id| id.to_ascii_lowercase() == target_lc);
        let matches_poi = state
            .galaxy
            .poi_records
            .values()
            .any(|poi| poi.id.to_ascii_lowercase() == target_lc)
            || state
                .galaxy
                .poi_records
                .values()
                .filter_map(|poi| poi.info.base_id.as_deref())
                .any(|id| id.to_ascii_lowercase() == target_lc);
        let matches_item = all_item_ids
            .iter()
            .any(|id| id.to_ascii_lowercase() == target_lc);
        if !matches_resource && !matches_poi && !matches_item {
            let suggestion = all_item_ids
                .iter()
                .chain(all_resource_ids.iter())
                .find(|id| {
                    let id_lc = id.to_ascii_lowercase();
                    id_lc.contains(&target_lc) || target_lc.contains(&id_lc)
                })
                .copied();
            unknown.push((target.as_str(), suggestion));
        }
    }
    if unknown.is_empty() {
        return None;
    }
    let parts: Vec<String> = unknown
        .into_iter()
        .map(|(t, suggestion)| match suggestion {
            Some(s) => format!("Unknown target `{t}`. Did you mean `{s}`?"),
            None => format!("Unknown target `{t}`."),
        })
        .collect();
    Some(parts.join(" "))
}

fn known_resource_locations(state: &PlanningState, target: &str) -> Option<(String, Vec<String>)> {
    let resource_id = state
        .galaxy
        .poi_records
        .values()
        .flat_map(|poi| &poi.resources)
        .find(|resource| resource.resource_id.eq_ignore_ascii_case(target))?
        .resource_id
        .clone();
    let mut pois = state
        .galaxy
        .poi_records
        .values()
        .filter(|poi| {
            poi.resources
                .iter()
                .any(|resource| resource.resource_id.eq_ignore_ascii_case(&resource_id))
        })
        .map(|poi| poi.id.clone())
        .collect::<Vec<_>>();
    pois.sort();
    Some((resource_id, pois))
}

fn known_poi_location(state: &PlanningState, target: &str) -> Option<(String, String)> {
    let poi_id = state
        .galaxy
        .poi_records
        .values()
        .find(|poi| poi.id.eq_ignore_ascii_case(target))
        .map(|poi| poi.id.clone())
        .or_else(|| {
            state
                .galaxy
                .poi_records
                .values()
                .find(|poi| {
                    poi.info
                        .base_id
                        .as_deref()
                        .is_some_and(|base_id| base_id.eq_ignore_ascii_case(target))
                })
                .map(|poi| poi.id.clone())
        })?;
    let system_id = state.galaxy.poi_records.get(&poi_id)?.system_id.clone();
    Some((poi_id, system_id))
}

fn required_text_arg<'a>(
    command: &'a ResolvedAction,
    idx: usize,
    action: &str,
) -> Result<&'a str, OperationFailure> {
    command
        .args
        .get(idx)
        .and_then(|arg| arg.as_str())
        .ok_or_else(|| {
            OperationFailure::InvalidIntent(format!("'{action}' is missing a required argument."))
        })
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
