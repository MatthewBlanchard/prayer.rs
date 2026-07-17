//! Navigation, refueling, and discovery command planning.

use super::*;

impl CommandPlanner {
    pub(super) fn start_go(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let target_token = required_text_arg(&self.command, 0, "go")?.to_string();
        let target =
            resolve_go_target(state, &target_token).map_err(OperationFailure::InvalidIntent)?;

        let mut go = match self.continuation.take() {
            Some(ActiveCommandState::Go(go)) if go.target == target_token => go,
            other => {
                self.continuation = other;
                GoState {
                    target: target_token.clone(),
                    ..GoState::default()
                }
            }
        };
        go.resolved_system = Some(target.system.clone());
        go.resolved_poi = target.poi.clone();

        let arrived = if let Some(poi_id) = target.poi.as_deref() {
            state.current_poi.as_deref() == Some(poi_id)
        } else {
            state.system.as_deref() == Some(target.system.as_str())
        };
        if arrived {
            self.continuation = Some(ActiveCommandState::Go(go));
            return Ok(complete(completed_with_message(format!(
                "Already at {}.",
                target.label
            ))));
        }

        go.did_move = true;
        self.continuation = Some(ActiveCommandState::Go(go));
        self.step_toward_target(state, &target.label, &target.system, target.poi.as_deref())
    }

    pub(super) fn start_refuel(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if state.fuel_pct >= 100 {
            return Ok(complete(completed_with_message("Fuel already full.")));
        }

        let mut refuel = match self.continuation.take() {
            Some(ActiveCommandState::Refuel(refuel)) => refuel,
            other => {
                self.continuation = other;
                RefuelState::default()
            }
        };

        // Pin the first selected station for the lifetime of this command.
        // Recomputing "nearest" after every jump can make the destination
        // oscillate as a different station becomes closer en route.
        let target = refuel
            .target_system
            .clone()
            .zip(refuel.target_poi.clone())
            .or_else(|| nearest_refuel_station(state));
        let Some((target_system, target_poi)) = target else {
            self.phase = Phase::AwaitFinalCall;
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt/refuel".to_string(),
                payload: Some(serde_json::json!({})),
            });
        };
        refuel
            .target_system
            .get_or_insert_with(|| target_system.clone());
        refuel.target_poi.get_or_insert_with(|| target_poi.clone());
        self.continuation = Some(ActiveCommandState::Refuel(refuel));

        if state.system.as_deref() != Some(target_system.as_str())
            || state.current_poi.as_deref() != Some(target_poi.as_str())
        {
            return self.step_toward_target(
                state,
                target_poi.as_str(),
                target_system.as_str(),
                Some(target_poi.as_str()),
            );
        }

        if !state.docked {
            self.phase = Phase::AwaitPositioning {
                message: format!("Docking at `{target_poi}` to refuel..."),
            };
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt/dock".to_string(),
                payload: None,
            });
        }

        self.phase = Phase::AwaitFinalCall;
        Ok(RuntimeOperation::SpaceMoltAction {
            action: "spacemolt/refuel".to_string(),
            payload: Some(serde_json::json!({})),
        })
    }

    pub(super) fn start_find(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let targets = self
            .command
            .args
            .iter()
            .map(|a| a.as_text())
            .filter(|target| !target.trim().is_empty())
            .collect::<Vec<_>>();
        if let Some(found) = known_find_target(state, &targets) {
            return Ok(complete(completed_with_message(found)));
        }

        if let Some(msg) = unknown_find_target_message(state, &targets) {
            return Ok(complete(EngineExecutionResult {
                result_message: Some(msg),
                completed: true,
                halt_script: true,
            }));
        }

        self.find_anywhere(state, targets)
    }

    pub(super) fn find_anywhere(
        &mut self,
        state: &PlanningState,
        targets: Vec<String>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        debug!(
            current_poi = state.current_poi.as_deref().unwrap_or("(none)"),
            docked = state.docked,
            system = state.system.as_deref().unwrap_or("(unknown)"),
            "find_anywhere: entry"
        );

        if state.docked {
            self.phase = Phase::AwaitPositioning {
                message: "Undocking to find...".to_string(),
            };
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt/undock".to_string(),
                payload: None,
            });
        }

        let Some(current_system) = state.system.as_deref() else {
            return Err(OperationFailure::InvalidIntent(
                "Can't find: current system is unknown.".to_string(),
            ));
        };

        if let Some(target) = nearest_find_navigation_target(state)
            .filter(|target| target.system == current_system && target.poi.is_some())
        {
            let target_poi = target.poi.as_deref().unwrap_or_default();
            if state.current_poi.as_deref() != Some(target_poi) {
                debug!(target_poi, "find_anywhere: traveling to unvisited poi");
                self.record_find_target(&target.system);
                return self.step_toward_target(
                    state,
                    target.label.as_str(),
                    target.system.as_str(),
                    target.poi.as_deref(),
                );
            }
        }

        if !state
            .galaxy
            .system_records
            .get(current_system)
            .is_some_and(|system| system.last_surveyed_unix.is_some())
        {
            debug!(current_system, "find_anywhere: calling survey_system");
            self.phase = Phase::AwaitSurveyThenExplore { targets };
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt/survey_system".to_string(),
                payload: None,
            });
        }

        self.find_explore(state, targets)
    }

    pub(super) fn find_explore(
        &mut self,
        state: &PlanningState,
        targets: Vec<String>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let Some(current_system) = state.system.as_deref() else {
            return Err(OperationFailure::InvalidIntent(
                "Can't find: current system is unknown.".to_string(),
            ));
        };

        let candidate_systems = ordered_find_target_systems(state, current_system);
        debug!(
            ?candidate_systems,
            "find_explore: candidate systems for exploration"
        );

        if candidate_systems.is_empty() {
            return Ok(complete(find_exhausted_result(&targets)));
        }

        if let Some(target) = nearest_find_navigation_target(state).filter(|target| {
            target.system != current_system && candidate_systems.contains(&target.system)
        }) {
            debug!(
                target_system = target.system,
                "find_explore: jumping toward target system"
            );
            self.record_find_target(&target.system);
            return self.step_toward_target(
                state,
                target.label.as_str(),
                target.system.as_str(),
                target.poi.as_deref(),
            );
        }

        if candidate_systems.iter().all(|s| s == current_system) {
            return Ok(complete(incomplete_with_message(format!(
                "Finding in `{current_system}`..."
            ))));
        }

        Ok(complete(find_exhausted_result(&targets)))
    }

    pub(super) fn record_find_target(&mut self, target_system: &str) {
        if let Some(ActiveCommandState::Find(find)) = self.continuation.as_mut() {
            find.target_system = Some(target_system.to_string());
        }
    }

    /// Plan one movement toward the target: undock, in-system travel, or a
    /// jump along the next hop. The tick ends incomplete after the call.
    pub(super) fn step_toward_target(
        &mut self,
        state: &PlanningState,
        target_label: &str,
        target_system: &str,
        target_poi: Option<&str>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if state.docked {
            self.phase = Phase::AwaitPositioning {
                message: "Undocking...".to_string(),
            };
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt/undock".to_string(),
                payload: None,
            });
        }

        if state.system.as_deref() == Some(target_system) {
            if let Some(poi_id) = target_poi {
                return Ok(self.transit_call(
                    "travel",
                    "target_poi",
                    poi_id,
                    format!("Traveling to {target_label}..."),
                ));
            }
            return Ok(complete(completed_with_message(format!(
                "Arrived at {target_label}."
            ))));
        }

        let Some(next_hop) = state
            .system
            .as_deref()
            .and_then(|s| state.galaxy.next_hop_toward(s, target_system))
        else {
            return Err(OperationFailure::InvalidIntent(format!(
                "Can't reach {target_label} — no known route from current system."
            )));
        };
        Ok(self.transit_call(
            "jump",
            "target_system",
            &next_hop,
            format!("Jumping toward {target_label}..."),
        ))
    }

    pub(super) fn transit_call(
        &mut self,
        api_action: &str,
        payload_key: &str,
        destination: &str,
        message: String,
    ) -> RuntimeOperation {
        self.phase = Phase::AwaitTransitCall {
            destination: destination.to_string(),
            message,
        };
        RuntimeOperation::SpaceMoltAction {
            action: format!("spacemolt/{api_action}"),
            payload: Some(serde_json::json!({ payload_key: destination })),
        }
    }

    pub(super) fn continue_transit_call(
        &mut self,
        destination: String,
        message: String,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        match last {
            Some(ApiOutcome::Success(_)) => Ok(complete(incomplete_with_message(message))),
            Some(ApiOutcome::Failure(error)) if error.is_network() || error.is_transient() => {
                // The request may have timed out after the transit started —
                // confirm against fresh state before surfacing the error.
                self.phase = Phase::AwaitTransitConfirm {
                    destination,
                    message,
                    original_error: error,
                };
                Ok(RuntimeOperation::RefreshState)
            }
            Some(ApiOutcome::Failure(error)) => Err(error),
            None => Err(OperationFailure::Policy(
                "planner expected an API response".to_string(),
            )),
        }
    }
}
