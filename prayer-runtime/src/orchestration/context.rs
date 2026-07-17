//! Within-tick continuation state shared by command-family planners.

use crate::operation_failure::OperationFailure;

use super::transfer::SpaceLootTarget;

/// Cross-tick continuation lives in `ActiveCommandState`; this only sequences
/// calls made during the current planner tick.
pub(super) enum Phase {
    Start,
    AwaitPositioning {
        message: String,
    },
    AwaitFinalCall,
    TransferSpaceJettisonLoop {
        targets: Vec<(String, i64)>,
        issued_idx: usize,
    },
    SpaceLootLoop {
        targets: Vec<SpaceLootTarget>,
        issued_idx: usize,
    },
    AwaitTransitCall {
        destination: String,
        message: String,
    },
    AwaitTransitConfirm {
        destination: String,
        message: String,
        original_error: OperationFailure,
    },
    AwaitSurveyThenExplore {
        targets: Vec<String>,
    },
    AwaitMineStrike {
        target_poi: String,
    },
    AwaitStorageBatch {
        count: usize,
        all_cargo: bool,
        allow_no_space_success: bool,
    },
    AwaitBuyOrder {
        item_id: String,
        quantity: i64,
        price_each: i64,
    },
    CancelThenRetryBuy {
        order_ids: Vec<String>,
        issued_idx: usize,
        item_id: String,
        quantity: i64,
        price_each: i64,
    },
    AwaitCrossingBuyRefresh {
        item_id: String,
        quantity: i64,
        price_each: i64,
    },
    AwaitCrossingBuyWithdraw {
        item_id: String,
        remaining_quantity: i64,
        price_each: i64,
        withdrawn_quantity: i64,
    },
    SellLoop {
        targets: Vec<(String, i64, i64)>,
        issued_idx: usize,
    },
    CancelOrdersLoop {
        order_ids: Vec<String>,
        issued_idx: usize,
        item_id: String,
        canceled: usize,
        errors: Vec<String>,
    },
    Finished,
}
