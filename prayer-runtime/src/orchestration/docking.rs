//! Docking and home-base command planning.

use super::*;

impl CommandPlanner {
    /// Position for a docked command: returns the operation that moves us
    /// closer (or a completion when no dock target exists), or `None` when
    /// already docked at a suitable target.
    pub(super) fn ensure_docked_step(
        &mut self,
        state: &PlanningState,
        requires_station: bool,
    ) -> Option<RuntimeOperation> {
        match plan_ensure_docked(state, requires_station) {
            DockPlan::Ready => None,
            DockPlan::NoTarget => Some(complete(halted_with_message(
                "No dockable base available in the current system.",
            ))),
            DockPlan::Issue {
                action,
                payload,
                message,
            } => {
                self.phase = Phase::AwaitPositioning { message };
                Some(RuntimeOperation::SpaceMoltAction {
                    action: format!("spacemolt/{action}"),
                    payload,
                })
            }
        }
    }
    pub(super) fn start_dock(&mut self, state: &PlanningState) -> RuntimeOperation {
        match plan_ensure_docked(state, false) {
            DockPlan::Ready => complete(completed_with_message("Docked.")),
            DockPlan::NoTarget => complete(completed_with_message(
                "No dockable base available in the current system.",
            )),
            DockPlan::Issue {
                action,
                payload,
                message,
            } => {
                self.phase = Phase::AwaitPositioning { message };
                RuntimeOperation::SpaceMoltAction {
                    action: format!("spacemolt/{action}"),
                    payload,
                }
            }
        }
    }

    pub(super) fn start_set_home(
        &mut self,
        state: &PlanningState,
    ) -> Result<RuntimeOperation, OperationFailure> {
        match plan_ensure_docked(state, false) {
            DockPlan::NoTarget => Ok(complete(completed_with_message(
                "No dockable base available in the current system.",
            ))),
            DockPlan::Issue {
                action,
                payload,
                message,
            } => {
                self.phase = Phase::AwaitPositioning { message };
                Ok(RuntimeOperation::SpaceMoltAction {
                    action: format!("spacemolt/{action}"),
                    payload,
                })
            }
            DockPlan::Ready => {
                let Some(base_id) = state.current_poi.as_deref() else {
                    return Err(OperationFailure::InvalidIntent(
                        "Can't set home base: current location is unknown.".to_string(),
                    ));
                };
                self.phase = Phase::AwaitFinalCall;
                Ok(RuntimeOperation::SpaceMoltAction {
                    action: "spacemolt/set_home_base".to_string(),
                    payload: Some(serde_json::json!({ "base_id": base_id })),
                })
            }
        }
    }
}
