//! Source-independent execution producer contracts and persisted run envelope.

use prayer_actions::{Action, ContinuationEnvelope, RunId};
use prayer_scheduler::{QueueClaim, SchedulerCheckpoint};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{read_context::ExecutionReadContext, PrayerLangRun};

/// One authoritative projection shared by HTTP, UI, and MCP/VFS surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSnapshot {
    pub scheduler: prayer_scheduler::SchedulerSnapshot,
    pub producer: ProducerSnapshot,
    pub interrupt_producer: Option<ProducerSnapshot>,
    pub source_prayer: String,
    pub queue_prayer: String,
    #[serde(default)]
    pub normal_queue_prayer: String,
    #[serde(default)]
    pub override_queue_prayer: String,
    pub active_continuation: Option<Value>,
}

/// Current persisted execution-run schema.
pub const EXECUTION_RUN_SCHEMA_VERSION: u32 = 2;

/// Scheduler state and producer state are persisted independently so restore
/// never needs to infer queue lifecycle from source frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedExecutionRun {
    pub schema_version: u32,
    pub scheduler: SchedulerCheckpoint,
    pub producer: PersistedProducer,
    #[serde(default)]
    pub active_continuation: Option<ContinuationEnvelope>,
    #[serde(default)]
    pub action_run: Option<PersistedActionRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedActionRun {
    pub run_id: RunId,
    pub actions: Vec<prayer_actions::ActionEnvelope>,
    pub outcome: Option<ActionBatchOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActionBatchOutcome {
    Succeeded,
    Failed {
        action_index: usize,
        message: String,
    },
    Cancelled {
        reason: String,
    },
    Halted {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "checkpoint", rename_all = "snake_case")]
pub enum PersistedProducer {
    PrayerLang(Box<PrayerLangRunCheckpoint>),
    Controller(ControllerCheckpoint),
    Manual(ManualRunCheckpoint),
}

impl PersistedExecutionRun {
    /// Whether a restored PrayerLang producer must be analyzed against fresh state.
    pub fn needs_prayerlang_reanalysis(&self) -> bool {
        match &self.producer {
            PersistedProducer::PrayerLang(checkpoint) => !checkpoint.run.has_analysis(),
            PersistedProducer::Controller(_) | PersistedProducer::Manual(_) => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrayerLangRunCheckpoint {
    pub schema_version: u32,
    pub run: PrayerLangRun,
    pub claim: Option<QueueClaim>,
    #[serde(default)]
    pub action_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControllerCheckpoint {
    pub schema_version: u32,
    pub run_id: RunId,
    pub controller: String,
    pub state: Value,
    pub claim: Option<QueueClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualRunCheckpoint {
    pub schema_version: u32,
    pub run_id: RunId,
    pub claim: Option<QueueClaim>,
}

/// Read-only inputs available when a durable workflow chooses its next work.
pub struct ControllerContext<'a> {
    pub state: ExecutionReadContext<'a>,
    pub last_completed_action: Option<&'a prayer_scheduler::CompletedAction>,
}

/// Source-independent result returned by any normal-lane producer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", content = "value", rename_all = "snake_case")]
pub enum ProducerDecision {
    Enqueue(Vec<Action>),
    WaitForChange,
    Complete,
    Halt(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProducerSnapshot {
    PrayerLang {
        halted: bool,
        finished: bool,
        current_source_line: Option<usize>,
        frame_depth: usize,
        mined_by_item: std::collections::HashMap<String, i64>,
        stored_by_item: std::collections::HashMap<String, i64>,
    },
    Manual {
        run_id: RunId,
    },
    Controller(ControllerStatus),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerStatus {
    pub kind: String,
    pub run_id: RunId,
    pub phase: String,
    pub progress: Value,
    pub next_decision: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ControllerError {
    #[error("controller state is invalid: {0}")]
    InvalidState(String),
    #[error("controller cannot continue: {0}")]
    CannotContinue(String),
}

/// A typed, checkpointable producer. Implementations decide workflows but do
/// not execute I/O and do not mutate the scheduler directly.
pub trait WorkflowController: Send {
    fn kind(&self) -> &'static str;
    fn decide(
        &mut self,
        context: &ControllerContext<'_>,
    ) -> Result<ProducerDecision, ControllerError>;
    fn snapshot(&self) -> ControllerStatus;
    fn checkpoint(&self) -> Result<ControllerCheckpoint, ControllerError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_context::ExecutionReadContext, EngineExecutionResult, RuntimeEngine};
    use prayer_actions::{Action, ActionEnvelope, ActionOrigin, RunId};
    use prayer_scheduler::{QueueOwner, Scheduler};

    fn submit_wait_action(engine: &mut RuntimeEngine, ticks: u64) {
        let run_id = RunId("checkpoint-wait".into());
        let claim = engine
            .try_acquire_action_run(run_id.clone())
            .expect("claim");
        engine
            .submit_action_batch(
                &claim,
                vec![ActionEnvelope::new(
                    "checkpoint-wait",
                    Action::Wait { ticks },
                    ActionOrigin::Manual { run_id },
                )],
            )
            .expect("submit wait action");
    }

    #[test]
    fn execution_envelope_round_trips_without_reparsing_source() {
        let mut scheduler = Scheduler::new();
        let claim = scheduler
            .claim(QueueOwner::Manual {
                run_id: "request-7".into(),
            })
            .expect("claim");
        let run = PersistedExecutionRun {
            schema_version: EXECUTION_RUN_SCHEMA_VERSION,
            scheduler: scheduler.checkpoint(),
            producer: PersistedProducer::Manual(ManualRunCheckpoint {
                schema_version: 1,
                run_id: "request-7".into(),
                claim: Some(claim),
            }),
            active_continuation: None,
            action_run: Some(PersistedActionRun {
                run_id: "request-7".into(),
                actions: Vec::new(),
                outcome: None,
            }),
        };

        let json = serde_json::to_string(&run).expect("serialize");
        let restored: PersistedExecutionRun = serde_json::from_str(&json).expect("restore");
        assert_eq!(restored.schema_version, EXECUTION_RUN_SCHEMA_VERSION);
        assert!(matches!(restored.producer, PersistedProducer::Manual(_)));
    }

    #[test]
    fn runtime_execution_checkpoint_restores_scheduler_and_continuation_atomically() {
        let mut engine = RuntimeEngine::new();
        submit_wait_action(&mut engine, 3);
        let command = engine
            .decide_next(ExecutionReadContext::default())
            .expect("decide")
            .expect("command");
        engine.execute_result(
            &command,
            EngineExecutionResult {
                result_message: Some("waiting".into()),
                completed: false,
                halt_script: false,
            },
            ExecutionReadContext::default(),
        );
        let expected_scheduler = engine.scheduler_snapshot();
        let expected_continuation = engine.active_command_state();
        let checkpoint = engine.execution_checkpoint().expect("checkpoint");

        let mut restored = RuntimeEngine::new();
        restored
            .restore_execution_checkpoint(checkpoint)
            .expect("restore");

        assert_eq!(restored.scheduler_snapshot(), expected_scheduler);
        assert_eq!(restored.active_command_state(), expected_continuation);
    }

    #[test]
    fn execution_checkpoint_excludes_override_lane_and_resumes_normal_work() {
        let mut engine = RuntimeEngine::new();
        submit_wait_action(&mut engine, 3);
        let normal = engine
            .decide_next(ExecutionReadContext::default())
            .expect("decide normal")
            .expect("normal");
        engine
            .submit_action_override(vec![prayer_actions::ActionEnvelope::new(
                "override-wait",
                prayer_actions::Action::Wait { ticks: 1 },
                prayer_actions::ActionOrigin::Interrupt {
                    policy: "client".into(),
                },
            )])
            .expect("override");
        let override_action = engine
            .decide_next(ExecutionReadContext::default())
            .expect("decide override")
            .expect("override action");
        assert_eq!(override_action.action, "wait");

        let checkpoint = engine.execution_checkpoint().expect("checkpoint");
        assert!(checkpoint.scheduler.interrupt.is_none());
        assert!(checkpoint.scheduler.interrupt_pending.is_empty());
        assert!(
            !checkpoint
                .scheduler
                .running
                .as_ref()
                .expect("normal running")
                .paused
        );

        let mut restored = RuntimeEngine::new();
        restored
            .restore_execution_checkpoint(checkpoint)
            .expect("restore");
        assert!(!restored.override_lane_busy());
        let resumed = restored
            .decide_next(ExecutionReadContext::default())
            .expect("decide resumed")
            .expect("resumed normal");
        assert_eq!(resumed, normal);
    }
}
