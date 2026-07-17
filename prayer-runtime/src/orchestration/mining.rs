//! Mining target selection and strike continuation.

use super::*;

impl CommandPlanner {
    pub(super) fn start_mine(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        if state.cargo_pct >= 100 {
            return Ok(complete(completed_with_message("Cargo is full.")));
        }

        let resource = self
            .command
            .args
            .first()
            .and_then(|a| a.as_str())
            .map(str::to_string);
        let Some(target_poi) = nearest_mining_poi(state, resource.as_deref(), |poi| {
            !self.mining_blacklist.contains(poi)
        }) else {
            return Ok(complete(EngineExecutionResult {
                result_message: Some(match resource.as_deref() {
                    Some(resource) => {
                        format!("No known minable locations for {resource} anywhere in the galaxy!")
                    }
                    None => "No known minable locations anywhere in the galaxy!".to_string(),
                }),
                completed: true,
                halt_script: true,
            }));
        };

        let mut mine = match self.continuation.take() {
            Some(ActiveCommandState::Mine(mine)) => mine,
            other => {
                self.continuation = other;
                MineState {
                    resource: resource.clone(),
                    ..MineState::default()
                }
            }
        };
        mine.target_poi = Some(target_poi.clone());
        self.continuation = Some(ActiveCommandState::Mine(mine));

        if state.current_poi.as_deref() == Some(target_poi.as_str()) {
            self.phase = Phase::AwaitMineStrike {
                target_poi: target_poi.clone(),
            };
            return Ok(RuntimeOperation::SpaceMoltAction {
                action: "spacemolt/mine".to_string(),
                payload: None,
            });
        }

        let target_system = state
            .galaxy
            .poi_records
            .get(target_poi.as_str())
            .map(|poi| poi.system_id.clone())
            .or_else(|| state.system.clone())
            .unwrap_or_else(|| target_poi.clone());
        self.step_toward_target(
            state,
            &target_poi,
            &target_system,
            Some(target_poi.as_str()),
        )
    }

    pub(super) fn continue_mine(
        &mut self,
        target_poi: String,
        last: Option<ApiOutcome>,
    ) -> Result<RuntimeOperation, OperationFailure> {
        let value = match last {
            Some(ApiOutcome::Success(value)) => {
                if is_mine_cargo_full_payload(&value) {
                    return Ok(complete(completed_with_message(
                        extract_result_message(&value)
                            .unwrap_or_else(|| "Cargo is full.".to_string()),
                    )));
                }
                value
            }
            Some(ApiOutcome::Failure(error))
                if error
                    .upstream_message()
                    .is_some_and(is_mine_cargo_full_message) =>
            {
                return Ok(complete(completed_with_message("Cargo is full.")));
            }
            Some(ApiOutcome::Failure(error))
                if error
                    .upstream_message()
                    .is_some_and(is_mine_depleted_message) =>
            {
                return Ok(complete(incomplete_with_message(format!(
                    "`{target_poi}` is depleted; retrying..."
                ))));
            }
            other => require_success(other)?,
        };
        if is_mine_depleted(&value) {
            let message = extract_result_message(&value)
                .unwrap_or_else(|| format!("`{target_poi}` is depleted; retrying..."));
            return Ok(complete(incomplete_with_message(message)));
        }
        Ok(complete(incomplete_with_api_message(&value)))
    }
}
