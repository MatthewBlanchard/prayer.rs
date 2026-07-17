//! Public operation contract between the pure planner and its runtime host.

use std::time::Duration;

use serde_json::Value;

use crate::engine::EngineExecutionResult;
use crate::operation_failure::OperationFailure;

/// Wall-clock length of one paused tick: transit waits, `wait` ticks, and
/// the cooldown after a mining strike comes back depleted.
pub const TICK_PAUSE: Duration = Duration::from_secs(10);

/// One executable intent produced by the planner.
#[derive(Debug)]
pub enum RuntimeOperation {
    SpaceMoltAction {
        /// Generated SpaceMolt action key in `tool/action` form.
        action: String,
        payload: Option<Value>,
    },
    WaitTick {
        message: String,
        resume_after: Duration,
    },
    CompleteAfterWait {
        message: String,
        resume_after: Duration,
    },
    Complete {
        result: EngineExecutionResult,
    },
    RefreshState,
}

/// Outcome of a `SpaceMoltAction` or `RefreshState` operation fed back to the planner.
#[derive(Debug)]
pub enum ApiOutcome {
    Success(Value),
    Failure(OperationFailure),
}
