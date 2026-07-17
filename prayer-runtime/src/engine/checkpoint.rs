/// Snapshot of current runtime state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    /// Current normalized script.
    pub script: String,
    /// Halt state.
    pub is_halted: bool,
    /// Whether the script ran to natural completion (all frames exhausted, no explicit halt).
    pub is_finished: bool,
    /// Current script line.
    pub current_script_line: Option<usize>,
    /// Active frame kinds.
    pub frame_stack: Vec<ExecutionFrameKind>,
    /// Active command continuation state, if a multi-step command is running.
    pub active_command: Option<ActiveCommandState>,
    /// Active script frame, projected for UI display.
    pub active_frame: Option<RuntimeActiveFrameSnapshot>,
    /// Recent action memory.
    pub memory: Vec<ActionMemory>,
    /// Root mined counters.
    pub mined_by_item: HashMap<String, i64>,
    /// Root stored counters.
    #[serde(alias = "stashed_by_item")]
    pub stored_by_item: HashMap<String, i64>,
}

/// User-visible active execution frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeActiveFrameSnapshot {
    /// User-facing frame kind (`main`).
    pub kind: String,
    /// User-facing frame name for named scopes.
    pub name: Option<String>,
    /// Runtime frame path.
    pub path: String,
    /// Canonical script text for this frame body.
    pub script: String,
    /// One-based active line within `script`.
    pub line: Option<usize>,
}


impl RuntimeEngine {
    /// Persist scheduler, producer, and continuation as one atomic envelope.
    pub fn execution_checkpoint(&self) -> Result<PersistedExecutionRun, EngineError> {
        let mut scheduler = self.scheduler.checkpoint();
        scheduler.interrupt = None;
        scheduler.interrupt_pending.clear();
        if let Some(running) = scheduler.running.as_mut() {
            running.paused = false;
        }
        let active_continuation = self
            .producer
            .frames
            .first()
            .and_then(|frame| frame.active_command.as_ref())
            .map(|state| {
                serde_json::to_value(state)
                    .map(|state| ContinuationEnvelope {
                        schema_version: 1,
                        executor: "prayer-runtime".into(),
                        state,
                    })
                    .map_err(|error| EngineError::InvalidState(error.to_string()))
            })
            .transpose()?;
        Ok(PersistedExecutionRun {
            schema_version: EXECUTION_RUN_SCHEMA_VERSION,
            scheduler,
            producer: if let Some(action_run) = &self.action_run {
                PersistedProducer::Manual(ManualRunCheckpoint {
                    schema_version: 1,
                    run_id: action_run.run_id.clone(),
                    claim: self.queue_claim.clone(),
                })
            } else {
                PersistedProducer::PrayerLang(Box::new(PrayerLangRunCheckpoint {
                    schema_version: 2,
                    run: self.producer.clone(),
                    claim: self.queue_claim.clone(),
                    action_sequence: self.action_sequence,
                }))
            },
            active_continuation,
            action_run: self.action_run.clone(),
        })
    }

    /// Restore an atomic execution envelope. Only PrayerLang is enabled as a
    /// production producer at this migration checkpoint.
    pub fn restore_execution_checkpoint(
        &mut self,
        run: PersistedExecutionRun,
    ) -> Result<(), EngineError> {
        if run.schema_version != EXECUTION_RUN_SCHEMA_VERSION {
            return Err(EngineError::InvalidState(format!(
                "execution checkpoint schema {} is incompatible with linear PrayerLang schema {}; legacy condition, skill, and policy frames cannot be resumed",
                run.schema_version, EXECUTION_RUN_SCHEMA_VERSION
            )));
        }
        let mut scheduler_checkpoint = run.scheduler.clone();
        let discarded_override = scheduler_checkpoint.interrupt.is_some()
            || !scheduler_checkpoint.interrupt_pending.is_empty();
        scheduler_checkpoint.interrupt = None;
        scheduler_checkpoint.interrupt_pending.clear();
        if let Some(running) = scheduler_checkpoint.running.as_mut() {
            running.paused = false;
        }
        let scheduler = prayer_scheduler::Scheduler::from_checkpoint(scheduler_checkpoint.clone())
            .map_err(|error| EngineError::InvalidState(error.to_string()))?;
        if let PersistedProducer::Manual(producer) = run.producer {
            if producer.schema_version != 1 {
                return Err(EngineError::InvalidState(format!("unsupported manual producer schema version {}", producer.schema_version)));
            }
            if run.scheduler.claim != producer.claim || run.action_run.as_ref().map(|r| &r.run_id) != Some(&producer.run_id) {
                return Err(EngineError::InvalidState("scheduler, producer, and action run do not match".into()));
            }
            self.scheduler = scheduler;
            self.queue_claim = producer.claim;
            self.action_run = run.action_run;
            return Ok(());
        }
        let PersistedProducer::PrayerLang(producer) = run.producer else {
            return Err(EngineError::InvalidState("controller producer is not enabled in this runtime".into()));
        };
        if producer.schema_version != 2 {
            return Err(EngineError::InvalidState(format!(
                "unsupported PrayerLang producer schema version {}",
                producer.schema_version
            )));
        }
        let scheduler_claim = run.scheduler.claim.clone();
        if scheduler_claim != producer.claim {
            return Err(EngineError::InvalidState(
                "scheduler and producer claims do not match".into(),
            ));
        }
        self.producer = producer.run;
        self.scheduler = scheduler;
        self.queue_claim = producer.claim;
        self.action_sequence = producer.action_sequence;
        self.action_run = None;
        let active_continuation = if discarded_override {
            scheduler_checkpoint
                .running
                .and_then(|running| running.continuation)
        } else {
            run.active_continuation
        };
        if let Some(continuation) = active_continuation {
            if continuation.executor != "prayer-runtime" {
                return Err(EngineError::InvalidState(format!(
                    "unsupported continuation executor {}",
                    continuation.executor
                )));
            }
            let state = serde_json::from_value(continuation.state)
                .map_err(|error| EngineError::InvalidState(error.to_string()))?;
            if let Some(frame) = self.producer.frames.first_mut() {
                frame.active_command = Some(state);
            }
        }
        Ok(())
    }

    /// Build an immutable runtime snapshot.
    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            script: self.producer.script_source.clone(),
            is_halted: self.producer.is_halted,
            is_finished: self.producer.is_finished,
            current_script_line: self.producer.current_script_line,
            frame_stack: self.producer.frames.iter().map(|f| f.kind).collect(),
            active_command: self.active_command_state(),
            active_frame: self.active_frame_snapshot(),
            memory: self.producer.memory.iter().cloned().collect(),
            mined_by_item: self.producer.mined_by_item.clone(),
            stored_by_item: self.producer.stored_by_item.clone(),
        }
    }

    fn active_frame_snapshot(&self) -> Option<RuntimeActiveFrameSnapshot> {
        let frame = self.producer.frames.first()?;
        let nodes = self.producer.analyzed_script.as_ref()?.statements.clone();
        let script = render_analyzed_nodes(&nodes);
        Some(RuntimeActiveFrameSnapshot {
            kind: "main".into(),
            name: None,
            path: "r".into(),
            line: active_frame_source_line(&script, &nodes, frame),
            script,
        })
    }

}
