//! Pure, claimable per-session action scheduler.

use prayer_actions::{
    ActionEnvelope, ActionOutcome, ContinuationEnvelope, RunId, ACTION_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;

pub const SCHEDULER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner_type", rename_all = "snake_case")]
pub enum QueueOwner {
    PrayerLang { run_id: RunId },
    Controller { run_id: RunId, kind: String },
    Manual { run_id: RunId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueClaim {
    pub owner: QueueOwner,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Normal,
    Interrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningAction {
    pub envelope: ActionEnvelope,
    pub continuation: Option<ContinuationEnvelope>,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedAction {
    pub envelope: ActionEnvelope,
    pub outcome: ActionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchedulerEvent {
    Claimed {
        claim: QueueClaim,
    },
    Appended {
        generation: u64,
        count: usize,
    },
    Started {
        lane: Lane,
        action: ActionEnvelope,
    },
    Continued {
        lane: Lane,
    },
    Completed {
        lane: Lane,
        completed: CompletedAction,
    },
    Halted {
        reason: String,
    },
    Cancelled {
        generation: u64,
        reason: String,
    },
    Released {
        generation: u64,
    },
    Drained,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerSnapshot {
    pub schema_version: u32,
    pub generation: u64,
    pub claim: Option<QueueClaim>,
    pub running: Option<RunningAction>,
    pub pending: Vec<ActionEnvelope>,
    pub interrupt: Option<RunningAction>,
    pub interrupt_pending: Vec<ActionEnvelope>,
    pub halted: bool,
    pub halt_reason: Option<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SchedulerError {
    #[error("unsupported scheduler checkpoint schema version {0}")]
    InvalidCheckpointSchema(u32),
    #[error("unsupported queued action schema version {0}; expected {ACTION_SCHEMA_VERSION}")]
    InvalidActionSchema(u32),
    #[error("the requested lane is already claimed")]
    AlreadyClaimed,
    #[error("claim generation is stale or the caller is not the owner")]
    StaleClaim,
    #[error("scheduler is halted")]
    Halted,
    #[error("the requested lane has no running action")]
    NothingRunning,
    #[error("the requested lane already has a running action")]
    AlreadyRunning,
    #[error("expected scheduler generation does not match")]
    GenerationMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scheduler {
    schema_version: u32,
    generation: u64,
    claim: Option<QueueClaim>,
    pending: VecDeque<ActionEnvelope>,
    running: Option<RunningAction>,
    interrupt_pending: VecDeque<ActionEnvelope>,
    interrupt: Option<RunningAction>,
    completed: Vec<CompletedAction>,
    halted: bool,
    halt_reason: Option<String>,
    events: Vec<SchedulerEvent>,
}

/// Intentional durable scheduler representation. Transient events are not
/// checkpointed and internal queue containers are kept out of the wire format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerCheckpoint {
    pub schema_version: u32,
    pub generation: u64,
    pub claim: Option<QueueClaim>,
    pub pending: Vec<ActionEnvelope>,
    pub running: Option<RunningAction>,
    pub interrupt_pending: Vec<ActionEnvelope>,
    pub interrupt: Option<RunningAction>,
    pub completed: Vec<CompletedAction>,
    pub halted: bool,
    pub halt_reason: Option<String>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            schema_version: SCHEDULER_SCHEMA_VERSION,
            generation: 0,
            claim: None,
            pending: VecDeque::new(),
            running: None,
            interrupt_pending: VecDeque::new(),
            interrupt: None,
            completed: Vec::new(),
            halted: false,
            halt_reason: None,
            events: Vec::new(),
        }
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn checkpoint(&self) -> SchedulerCheckpoint {
        SchedulerCheckpoint {
            schema_version: self.schema_version,
            generation: self.generation,
            claim: self.claim.clone(),
            pending: self.pending.iter().cloned().collect(),
            running: self.running.clone(),
            interrupt_pending: self.interrupt_pending.iter().cloned().collect(),
            interrupt: self.interrupt.clone(),
            completed: self.completed.clone(),
            halted: self.halted,
            halt_reason: self.halt_reason.clone(),
        }
    }

    pub fn from_checkpoint(checkpoint: SchedulerCheckpoint) -> Result<Self, SchedulerError> {
        if checkpoint.schema_version != SCHEDULER_SCHEMA_VERSION {
            return Err(SchedulerError::InvalidCheckpointSchema(
                checkpoint.schema_version,
            ));
        }
        let action_schema = checkpoint
            .pending
            .iter()
            .chain(checkpoint.interrupt_pending.iter())
            .chain(checkpoint.running.iter().map(|running| &running.envelope))
            .chain(checkpoint.interrupt.iter().map(|running| &running.envelope))
            .chain(
                checkpoint
                    .completed
                    .iter()
                    .map(|completed| &completed.envelope),
            )
            .find_map(|envelope| {
                (envelope.schema_version != ACTION_SCHEMA_VERSION)
                    .then_some(envelope.schema_version)
            });
        if let Some(version) = action_schema {
            return Err(SchedulerError::InvalidActionSchema(version));
        }
        Ok(Self {
            schema_version: checkpoint.schema_version,
            generation: checkpoint.generation,
            claim: checkpoint.claim,
            pending: checkpoint.pending.into(),
            running: checkpoint.running,
            interrupt_pending: checkpoint.interrupt_pending.into(),
            interrupt: checkpoint.interrupt,
            completed: checkpoint.completed,
            // Halt is a terminal clear operation, not a persisted scheduler mode.
            // Normalize checkpoints written by older versions back to reusable state.
            halted: false,
            halt_reason: None,
            events: Vec::new(),
        })
    }

    pub fn claim(&mut self, owner: QueueOwner) -> Result<QueueClaim, SchedulerError> {
        if self.halted {
            return Err(SchedulerError::Halted);
        }
        if self.claim.is_some() {
            return Err(SchedulerError::AlreadyClaimed);
        }
        self.generation = self.generation.saturating_add(1);
        let claim = QueueClaim {
            owner,
            generation: self.generation,
        };
        self.claim = Some(claim.clone());
        self.events.push(SchedulerEvent::Claimed {
            claim: claim.clone(),
        });
        Ok(claim)
    }

    /// Acquire an unowned scheduler only if its generation is still the one observed.
    pub fn reclaim(
        &mut self,
        expected_generation: u64,
        owner: QueueOwner,
    ) -> Result<QueueClaim, SchedulerError> {
        if self.generation != expected_generation {
            return Err(SchedulerError::GenerationMismatch);
        }
        self.claim(owner)
    }

    pub fn append(
        &mut self,
        claim: &QueueClaim,
        actions: impl IntoIterator<Item = ActionEnvelope>,
    ) -> Result<(), SchedulerError> {
        self.authorize(claim)?;
        let before = self.pending.len();
        self.pending.extend(actions);
        self.events.push(SchedulerEvent::Appended {
            generation: claim.generation,
            count: self.pending.len() - before,
        });
        Ok(())
    }

    pub fn append_interrupt(&mut self, action: ActionEnvelope) -> Result<(), SchedulerError> {
        self.append_interrupts([action])
    }

    /// Append work to the higher-precedence lane in submission order.
    pub fn append_interrupts(
        &mut self,
        actions: impl IntoIterator<Item = ActionEnvelope>,
    ) -> Result<(), SchedulerError> {
        if self.halted {
            return Err(SchedulerError::Halted);
        }
        if self.interrupt.is_some() || !self.interrupt_pending.is_empty() {
            return Err(SchedulerError::AlreadyClaimed);
        }
        self.interrupt_pending.extend(actions);
        if let Some(running) = &mut self.running {
            running.paused = true;
        }
        Ok(())
    }

    /// Materialize deferred interrupt actions when the lane first preempts.
    pub fn replace_interrupt_actions_by_policy(
        &mut self,
        policy: &str,
        replacement_policy: &str,
        action: prayer_actions::Action,
    ) {
        for envelope in &mut self.interrupt_pending {
            if let prayer_actions::ActionOrigin::Interrupt { policy: value } = &mut envelope.origin
            {
                if value == policy {
                    envelope.action = action.clone();
                    *value = replacement_policy.to_owned();
                }
            }
        }
    }

    pub fn start_next(&mut self) -> Result<Option<(Lane, ActionEnvelope)>, SchedulerError> {
        if self.halted {
            return Err(SchedulerError::Halted);
        }
        if self.interrupt.is_none() {
            if let Some(envelope) = self.interrupt_pending.pop_front() {
                if let Some(running) = &mut self.running {
                    running.paused = true;
                }
                self.interrupt = Some(RunningAction {
                    envelope: envelope.clone(),
                    continuation: None,
                    paused: false,
                });
                self.events.push(SchedulerEvent::Started {
                    lane: Lane::Interrupt,
                    action: envelope.clone(),
                });
                return Ok(Some((Lane::Interrupt, envelope)));
            }
        } else {
            return Err(SchedulerError::AlreadyRunning);
        }
        if self.running.is_some() {
            return Err(SchedulerError::AlreadyRunning);
        }
        let Some(envelope) = self.pending.pop_front() else {
            self.events.push(SchedulerEvent::Drained);
            return Ok(None);
        };
        self.running = Some(RunningAction {
            envelope: envelope.clone(),
            continuation: None,
            paused: false,
        });
        self.events.push(SchedulerEvent::Started {
            lane: Lane::Normal,
            action: envelope.clone(),
        });
        Ok(Some((Lane::Normal, envelope)))
    }

    pub fn set_continuation(
        &mut self,
        lane: Lane,
        continuation: ContinuationEnvelope,
    ) -> Result<(), SchedulerError> {
        let running = self.running_mut(lane)?;
        running.continuation = Some(continuation);
        self.events.push(SchedulerEvent::Continued { lane });
        Ok(())
    }

    pub fn clear_continuation(&mut self, lane: Lane) -> Result<(), SchedulerError> {
        self.running_mut(lane)?.continuation = None;
        Ok(())
    }

    pub fn complete(
        &mut self,
        lane: Lane,
        outcome: ActionOutcome,
    ) -> Result<CompletedAction, SchedulerError> {
        let running = match lane {
            Lane::Normal => self.running.take(),
            Lane::Interrupt => self.interrupt.take(),
        }
        .ok_or(SchedulerError::NothingRunning)?;
        let completed = CompletedAction {
            envelope: running.envelope,
            outcome,
        };
        self.completed.push(completed.clone());
        self.events.push(SchedulerEvent::Completed {
            lane,
            completed: completed.clone(),
        });
        if lane == Lane::Interrupt && self.interrupt_pending.is_empty() {
            if let Some(normal) = &mut self.running {
                normal.paused = false;
            }
        }
        Ok(completed)
    }

    pub fn release(&mut self, claim: &QueueClaim) -> Result<(), SchedulerError> {
        self.authorize(claim)?;
        self.invalidate();
        self.events.push(SchedulerEvent::Released {
            generation: self.generation,
        });
        Ok(())
    }

    pub fn cancel(
        &mut self,
        claim: &QueueClaim,
        reason: impl Into<String>,
    ) -> Result<(), SchedulerError> {
        self.authorize(claim)?;
        let reason = reason.into();
        self.pending.clear();
        if let Some(running) = self.running.take() {
            self.completed.push(CompletedAction {
                envelope: running.envelope,
                outcome: ActionOutcome::Cancelled {
                    reason: reason.clone(),
                },
            });
        }
        self.invalidate();
        self.events.push(SchedulerEvent::Cancelled {
            generation: self.generation,
            reason,
        });
        Ok(())
    }

    pub fn halt(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        self.halted = false;
        self.halt_reason = None;
        self.pending.clear();
        self.interrupt_pending.clear();
        for running in [self.running.take(), self.interrupt.take()]
            .into_iter()
            .flatten()
        {
            self.completed.push(CompletedAction {
                envelope: running.envelope,
                outcome: ActionOutcome::Cancelled {
                    reason: reason.clone(),
                },
            });
        }
        self.invalidate();
        self.events.push(SchedulerEvent::Halted { reason });
    }

    pub fn snapshot(&self) -> SchedulerSnapshot {
        SchedulerSnapshot {
            schema_version: self.schema_version,
            generation: self.generation,
            claim: self.claim.clone(),
            running: self.running.clone(),
            pending: self.pending.iter().cloned().collect(),
            interrupt: self.interrupt.clone(),
            interrupt_pending: self.interrupt_pending.iter().cloned().collect(),
            halted: self.halted,
            halt_reason: self.halt_reason.clone(),
        }
    }

    pub fn completed(&self) -> &[CompletedAction] {
        &self.completed
    }
    pub fn take_events(&mut self) -> Vec<SchedulerEvent> {
        std::mem::take(&mut self.events)
    }

    fn authorize(&self, claim: &QueueClaim) -> Result<(), SchedulerError> {
        if self.halted {
            return Err(SchedulerError::Halted);
        }
        if self.claim.as_ref() == Some(claim) {
            Ok(())
        } else {
            Err(SchedulerError::StaleClaim)
        }
    }

    fn running_mut(&mut self, lane: Lane) -> Result<&mut RunningAction, SchedulerError> {
        match lane {
            Lane::Normal => self.running.as_mut(),
            Lane::Interrupt => self.interrupt.as_mut(),
        }
        .ok_or(SchedulerError::NothingRunning)
    }

    fn invalidate(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.claim = None;
    }
}

// Versioned serialized scheduler state is kept as an alias so persistence APIs
// can name their boundary without maintaining a second state representation.

#[cfg(test)]
mod tests {
    use super::*;
    use prayer_actions::{Action, ActionOrigin};

    fn action(id: &str) -> ActionEnvelope {
        ActionEnvelope::new(
            id,
            Action::Wait { ticks: 1 },
            ActionOrigin::Manual {
                run_id: "test".into(),
            },
        )
    }

    #[test]
    fn stale_writer_is_rejected_after_release() {
        let mut scheduler = Scheduler::new();
        let claim = scheduler
            .claim(QueueOwner::Manual {
                run_id: "one".into(),
            })
            .expect("claim");
        scheduler.release(&claim).expect("release");
        assert_eq!(
            scheduler.append(&claim, [action("late")]),
            Err(SchedulerError::StaleClaim)
        );
    }

    #[test]
    fn interrupt_pauses_and_resumes_same_normal_action() {
        let mut scheduler = Scheduler::new();
        let claim = scheduler
            .claim(QueueOwner::Manual {
                run_id: "one".into(),
            })
            .expect("claim");
        scheduler
            .append(&claim, [action("normal")])
            .expect("append");
        scheduler.start_next().expect("start");
        scheduler
            .append_interrupt(action("safety"))
            .expect("interrupt");
        let started = scheduler
            .start_next()
            .expect("start interrupt")
            .expect("action");
        assert_eq!(started.0, Lane::Interrupt);
        assert!(scheduler.snapshot().running.expect("normal").paused);
        scheduler
            .complete(Lane::Interrupt, ActionOutcome::Succeeded)
            .expect("complete");
        let snapshot = scheduler.snapshot();
        let normal = snapshot.running.as_ref().expect("normal");
        assert_eq!(normal.envelope.id.0, "normal");
        assert!(!normal.paused);
    }

    #[test]
    fn interrupt_batch_is_exclusive_and_runs_in_submission_order() {
        let mut scheduler = Scheduler::new();
        scheduler
            .append_interrupts([action("first"), action("second")])
            .expect("append");
        assert_eq!(
            scheduler.append_interrupt(action("competing")),
            Err(SchedulerError::AlreadyClaimed)
        );
        assert_eq!(scheduler.snapshot().interrupt_pending.len(), 2);
        let first = scheduler.start_next().expect("start").expect("first");
        assert_eq!(first.1.id.0, "first");
        scheduler
            .complete(Lane::Interrupt, ActionOutcome::Succeeded)
            .expect("complete");
        let second = scheduler.start_next().expect("start").expect("second");
        assert_eq!(second.1.id.0, "second");
    }

    #[test]
    fn interrupt_lane_accepts_new_owner_after_prior_batch_drains() {
        let mut scheduler = Scheduler::new();
        scheduler.append_interrupt(action("first")).expect("first");
        scheduler.start_next().expect("start first");
        scheduler
            .complete(Lane::Interrupt, ActionOutcome::Succeeded)
            .expect("complete first");
        scheduler
            .append_interrupt(action("second"))
            .expect("second owner");
    }

    #[test]
    fn continuation_survives_serialization() {
        let mut scheduler = Scheduler::new();
        let claim = scheduler
            .claim(QueueOwner::Manual {
                run_id: "one".into(),
            })
            .expect("claim");
        scheduler.append(&claim, [action("go")]).expect("append");
        scheduler.start_next().expect("start");
        scheduler
            .set_continuation(
                Lane::Normal,
                ContinuationEnvelope {
                    schema_version: 1,
                    executor: "go".into(),
                    state: serde_json::json!({"hop": 2}),
                },
            )
            .expect("continuation");
        let json = serde_json::to_string(&scheduler).expect("serialize");
        let restored: Scheduler = serde_json::from_str(&json).expect("restore");
        assert_eq!(restored.snapshot(), scheduler.snapshot());
    }

    #[test]
    fn restore_rejects_incompatible_action_schema_before_mutation() {
        let mut scheduler = Scheduler::default();
        let claim = scheduler
            .claim(QueueOwner::Manual {
                run_id: "request-1".into(),
            })
            .expect("claim");
        scheduler
            .append(&claim, vec![action("old-action")])
            .expect("append");
        let mut checkpoint = scheduler.checkpoint();
        checkpoint.pending[0].schema_version = 1;
        assert!(matches!(
            Scheduler::from_checkpoint(checkpoint),
            Err(SchedulerError::InvalidActionSchema(1))
        ));
    }

    #[test]
    fn drained_does_not_release_the_producer() {
        let mut scheduler = Scheduler::new();
        let claim = scheduler
            .claim(QueueOwner::Manual {
                run_id: "one".into(),
            })
            .expect("claim");
        assert_eq!(scheduler.start_next().expect("drained"), None);
        assert_eq!(scheduler.snapshot().claim, Some(claim));
    }

    #[test]
    fn reclaim_is_compare_and_swap() {
        let mut scheduler = Scheduler::new();
        let first = scheduler
            .claim(QueueOwner::Manual {
                run_id: "one".into(),
            })
            .expect("claim");
        scheduler.release(&first).expect("release");
        let generation = scheduler.snapshot().generation;
        assert_eq!(
            scheduler.reclaim(
                generation - 1,
                QueueOwner::Manual {
                    run_id: "stale".into()
                }
            ),
            Err(SchedulerError::GenerationMismatch)
        );
        let next = scheduler
            .reclaim(
                generation,
                QueueOwner::Manual {
                    run_id: "next".into(),
                },
            )
            .expect("reclaim");
        assert!(next.generation > generation);
    }

    #[test]
    fn halt_clears_running_normal_and_interrupt_actions() {
        let mut scheduler = Scheduler::new();
        let claim = scheduler
            .claim(QueueOwner::Manual {
                run_id: "one".into(),
            })
            .expect("claim");
        scheduler
            .append(&claim, [action("normal"), action("pending")])
            .expect("append");
        scheduler.start_next().expect("start normal");
        scheduler
            .append_interrupt(action("interrupt"))
            .expect("append interrupt");
        scheduler.start_next().expect("start interrupt");

        scheduler.halt("stop everything");

        let snapshot = scheduler.snapshot();
        assert!(snapshot.claim.is_none());
        assert!(snapshot.running.is_none());
        assert!(snapshot.interrupt.is_none());
        assert!(snapshot.pending.is_empty());
        assert!(!snapshot.halted);
        assert!(snapshot.halt_reason.is_none());
        assert_eq!(scheduler.completed().len(), 2);
        assert!(scheduler.completed().iter().all(|completed| matches!(
            &completed.outcome,
            ActionOutcome::Cancelled { reason } if reason == "stop everything"
        )));
    }
}
