//! Prayer runtime engine and checkpoint model.

use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::sync::Arc;

use crate::execution::{
    ActionBatchOutcome, ManualRunCheckpoint, PersistedActionRun, PersistedExecutionRun,
    PersistedProducer, PrayerLangRunCheckpoint, EXECUTION_RUN_SCHEMA_VERSION,
};
use crate::read_context::{ExecutionReadContext, ExecutionRuntimeState};
use prayer_actions::{
    ActionArg, ActionEnvelope, ActionOrigin, ContinuationEnvelope, ResolvedAction, RunId,
};
use prayer_lang::{
    AnalyzedArg, AnalyzedCraft, AnalyzedNode, AnalyzedProgram, AnalyzedTransfer,
    AnalyzedTransferEndpoint, AnalyzedTransferSubject, AnalyzerError, ArgType, AstNode, AstProgram,
    CommandSpec, ValidationContext,
};
use prayer_scheduler::{Lane, QueueClaim, QueueOwner, Scheduler};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::state::{
    ActiveCommissionInfo, AgentSightingData, BotState, CatalogData, FactionGarageInfo,
    FactionGarageShipObservation, GalaxyData, GlobalPriceAggregates, MarketData, MarketOrder,
    MissionData, PoiInfoData, PoiResourceData, SalvageData, ShipState, SpaceLootInfo,
    StationMarketData,
};

// Keep the historical `engine::*` API while locating implementation details
// beside the responsibility that owns them.
include!("checkpoint.rs");
include!("evaluation.rs");
