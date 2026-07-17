/// Engine errors.
#[derive(Debug, Error)]
pub enum EngineError {
    /// DSL parsing failed.
    #[error("dsl parse error: {0}")]
    Parse(String),
    /// Invalid runtime operation.
    #[error("invalid runtime state: {0}")]
    InvalidState(String),
}

/// Serializable in-flight state for multi-turn commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveCommandState {
    /// `mine` command continuation state.
    Mine(MineState),
    /// `go` command continuation state.
    Go(GoState),
    /// `refuel` command continuation state.
    Refuel(RefuelState),
    /// `find` command continuation state.
    Find(FindState),
    /// `wait` command continuation state.
    Wait(WaitState),
}

/// Persisted state for `wait`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WaitState {
    /// Ticks requested by the script.
    pub total_ticks: u64,
    /// Ticks still to spend before the wait completes.
    pub remaining_ticks: u64,
    /// Location where the command started.
    #[serde(default)]
    pub origin: Option<CommandOrigin>,
}

/// Where a command began executing. Diagnostic and resumability context
/// only — it never overrides fresh state, but lets a restored runtime detect
/// drift and explain resumed command behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandOrigin {
    /// Original command arguments as text.
    pub args: Vec<String>,
    /// System at command start.
    pub system: Option<String>,
    /// POI at command start.
    pub poi: Option<String>,
    /// Docked at command start.
    pub docked: bool,
    /// In transit at command start.
    pub in_transit: bool,
}

impl CommandOrigin {
    fn capture(command: &ResolvedAction, state: ExecutionReadContext<'_>) -> Self {
        Self {
            args: command.args_as_strings(),
            system: state.bot.location.system_id.clone(),
            poi: state.bot.location.poi_id.clone(),
            docked: state.bot.location.docked_at.is_some(),
            in_transit: state.bot.location.in_transit.unwrap_or(false),
        }
    }
}
/// Persisted state for `mine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MineState {
    /// Optional resource id filter.
    pub resource: Option<String>,
    /// Selected mining target poi id.
    pub target_poi: Option<String>,
    /// Location where the command started.
    #[serde(default)]
    pub origin: Option<CommandOrigin>,
}

/// Persisted state for `go`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GoState {
    /// User target token.
    pub target: String,
    /// Resolved system id if any.
    pub resolved_system: Option<String>,
    /// Resolved poi id if any.
    pub resolved_poi: Option<String>,
    /// Whether we moved during this run.
    pub did_move: bool,
    /// Location where the command started.
    #[serde(default)]
    pub origin: Option<CommandOrigin>,
}

/// Persisted state for `refuel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RefuelState {
    /// Current destination system.
    pub target_system: Option<String>,
    /// Current destination poi.
    pub target_poi: Option<String>,
    /// Completion marker.
    pub completed: bool,
    /// Location where the command started.
    #[serde(default)]
    pub origin: Option<CommandOrigin>,
}

/// Persisted state for `find`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FindState {
    /// Optional target system currently being explored.
    pub target_system: Option<String>,
    /// Unreachable system ids encountered.
    pub unreachable_systems: Vec<String>,
    /// Completion marker.
    pub completed: bool,
    /// Location where the command started.
    #[serde(default)]
    pub origin: Option<CommandOrigin>,
}

/// Result submitted back to engine after command execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineExecutionResult {
    /// Optional user-facing message.
    pub result_message: Option<String>,
    /// Whether command completed.
    pub completed: bool,
    /// Whether runtime should halt.
    pub halt_script: bool,
}

impl Default for EngineExecutionResult {
    fn default() -> Self {
        Self {
            result_message: None,
            completed: true,
            halt_script: false,
        }
    }
}

/// Lightweight action memory entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionMemory {
    /// Action name.
    pub action: String,
    /// Action args.
    pub args: Vec<String>,
    /// Optional result message.
    pub result_message: Option<String>,
}

/// Runtime status events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeEvent {
    /// Script was loaded.
    ScriptLoaded,
    /// Runtime halted.
    Halted(String),
    /// Runtime resumed.
    Resumed(String),
    /// Command selected.
    CommandSelected(ResolvedAction),
    /// Command completed.
    CommandCompleted(ResolvedAction),
    /// Override fired.
    OverrideTriggered(String),
}

/// Checkpointable PrayerLang producer state, independent of scheduler state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrayerLangRun {
    script_source: String,
    analyzed_script: Option<AnalyzedProgram>,
    frames: Vec<ExecutionFrame>,
    memory: VecDeque<ActionMemory>,
    mined_by_item: HashMap<String, i64>,
    stored_by_item: HashMap<String, i64>,
    is_halted: bool,
    is_finished: bool,
    current_script_line: Option<usize>,
    command_catalog: HashMap<String, CommandSpec>,
}

enum PrayerLangDecision {
    Command(ResolvedAction),
    Complete,
}

impl PrayerLangRun {
    fn decide(&mut self, state: ExecutionReadContext<'_>) -> Result<PrayerLangDecision, EngineError> {
        let Some(program) = self.analyzed_script.as_ref() else {
            self.is_finished = true;
            return Ok(PrayerLangDecision::Complete);
        };
        let frame = self.frames.first_mut().ok_or_else(|| EngineError::InvalidState("linear run is missing its execution cursor".into()))?;

        if frame.active_command.is_some() {
            let node = frame.index.checked_sub(1).and_then(|index| program.statements.get(index))
                .ok_or_else(|| EngineError::InvalidState("active command cursor is invalid".into()))?;
            let command = materialize_linear_node(node, &self.script_source, &self.command_catalog)?;
            self.current_script_line = command.source_line;
            return Ok(PrayerLangDecision::Command(command));
        }

        let Some(node) = program.statements.get(frame.index) else {
            self.frames.clear();
            self.is_finished = true;
            self.current_script_line = None;
            return Ok(PrayerLangDecision::Complete);
        };
        frame.index += 1;
        let command = materialize_linear_node(node, &self.script_source, &self.command_catalog)?;
        frame.active_command = active_state_for_command(&command, state);
        self.current_script_line = command.source_line;
        Ok(PrayerLangDecision::Command(command))
    }

    pub fn has_analysis(&self) -> bool {
        self.analyzed_script.is_some()
    }

    pub fn producer_snapshot(&self) -> crate::execution::ProducerSnapshot {
        crate::execution::ProducerSnapshot::PrayerLang {
            halted: self.is_halted,
            finished: self.is_finished,
            current_source_line: self.current_script_line,
            frame_depth: usize::from(!self.frames.is_empty()),
            mined_by_item: self.mined_by_item.clone(),
            stored_by_item: self.stored_by_item.clone(),
        }
    }

    fn load(&mut self, script: &str, state: Option<ExecutionReadContext<'_>>) -> Result<String, EngineError> {
        let initial = AstProgram::parse(script)
            .map_err(|diagnostics| EngineError::Parse(render_diags(script, &diagnostics)))?;
        let normalized = initial.normalize();
        let parsed = AstProgram::parse(&normalized)
            .map_err(|diagnostics| EngineError::Parse(render_diags(&normalized, &diagnostics)))?;
        let diagnostics = parsed.validate(&ValidationContext::with_defaults());
        if !diagnostics.is_empty() {
            return Err(EngineError::Parse(render_diags(&normalized, &diagnostics)));
        }
        let observation = crate::analysis_observation(state);
        self.analyzed_script = Some(parsed.analyze(&self.command_catalog, &observation)
            .map_err(|errors| EngineError::Parse(render_analyzer_errors(&normalized, &errors)))?);
        self.script_source = normalized;
        self.frames = vec![ExecutionFrame::root()];
        self.memory.clear();
        self.mined_by_item.clear();
        self.stored_by_item.clear();
        self.is_halted = false;
        self.is_finished = false;
        self.current_script_line = None;
        Ok(self.script_source.clone())
    }

    fn reanalyze(&mut self, state: Option<ExecutionReadContext<'_>>) -> Result<(), EngineError> {
        let script = self.script_source.clone();
        let parsed = AstProgram::parse(&script)
            .map_err(|diagnostics| EngineError::Parse(render_diags(&script, &diagnostics)))?;
        let diagnostics = parsed.validate(&ValidationContext::with_defaults());
        if !diagnostics.is_empty() {
            return Err(EngineError::Parse(render_diags(&script, &diagnostics)));
        }
        self.analyzed_script = Some(parsed.analyze(&self.command_catalog, &crate::analysis_observation(state))
            .map_err(|errors| EngineError::Parse(render_analyzer_errors(&script, &errors)))?);
        Ok(())
    }
}

fn materialize_linear_node(
    node: &AnalyzedNode,
    source: &str,
    catalog: &HashMap<String, CommandSpec>,
) -> Result<ResolvedAction, EngineError> {
    let action = match node {
        AnalyzedNode::Command(command) => {
            let name = command.source.name.to_ascii_lowercase();
            let args = command.args.iter().map(|arg| match arg {
                AnalyzedArg::Resolved(value) => value.clone(),
            }).collect::<Vec<_>>();
            let spec = catalog.get(&name);
            let variadic = spec.and_then(|spec| spec.args.last()).filter(|arg| arg.variadic).map(|arg| arg.kind);
            let args = args.into_iter().enumerate().map(|(index, value)| {
                match spec.and_then(|spec| spec.args.get(index)).map(|arg| arg.kind).or(variadic).unwrap_or(ArgType::Any) {
                    ArgType::Any => ActionArg::Any(value),
                    ArgType::Integer => value.parse().map(ActionArg::Integer).unwrap_or_else(|_| ActionArg::Any(value)),
                    ArgType::ItemId => ActionArg::ItemId(value),
                    ArgType::SystemId => ActionArg::SystemId(value),
                    ArgType::PoiId => ActionArg::PoiId(value),
                    ArgType::GoTarget => ActionArg::GoTarget(value),
                    ArgType::ShipId => ActionArg::ShipId(value),
                    ArgType::ListingId => ActionArg::ListingId(value),
                    ArgType::MissionId => ActionArg::MissionId(value),
                    ArgType::ModuleId => ActionArg::ModuleId(value),
                    ArgType::RecipeId => ActionArg::RecipeId(value),
                }
            }).collect();
            ResolvedAction { action: name, args, source_line: Some(offset_to_line(source, command.source.span.start)) }
        }
        AnalyzedNode::Transfer(value) => transfer_to_command(value, source),
        AnalyzedNode::Craft(value) => craft_to_command(value, source),
        AnalyzedNode::Say(value) => typed_action_to_command(prayer_actions::Action::Say(prayer_actions::SayRequest {
            content: value.content.clone(), channel: value.channel.clone(), target: value.target.clone(),
        }), value.span, source)?,
        AnalyzedNode::Buy(value) => typed_action_to_command(prayer_actions::Action::Buy(prayer_actions::BuyRequest {
            item: prayer_actions::ItemId(value.item_id.clone()), quantity: value.quantity, max_price: value.max_price,
            place_order: value.place_order, deliver_to: value.deliver_to.clone(),
        }), value.span, source)?,
        AnalyzedNode::Sell(value) => typed_action_to_command(prayer_actions::Action::Sell(prayer_actions::SellRequest {
            item: value.item_id.clone().map(prayer_actions::ItemId), quantity: value.quantity, min_price: value.min_price,
            place_order: value.place_order,
        }), value.span, source)?,
        AnalyzedNode::Recycle(value) => typed_action_to_command(prayer_actions::Action::Recycle(prayer_actions::RecycleRequest {
            recipe_id: value.recipe_id.clone(), quantity: value.source.quantity, source: value.source.clauses.source.clone(),
            destination: value.source.clauses.deliver_to.clone(), facility_id: value.source.clauses.facility_id.clone(),
        }), value.source.span, source)?,
        AnalyzedNode::CommissionShip(value) => typed_action_to_command(prayer_actions::Action::CommissionShip(prayer_actions::CommissionShipRequest {
            ship_class: value.ship_class.clone(), provide_materials: value.provide_materials,
        }), value.span, source)?,
    };
    Ok(action)
}

fn typed_action_to_command(action: prayer_actions::Action, span: prayer_lang::Span, source: &str) -> Result<ResolvedAction, EngineError> {
    let mut command = crate::resolve_action(action).map_err(|error| EngineError::InvalidState(error.to_string()))?;
    command.source_line = Some(offset_to_line(source, span.start));
    Ok(command)
}

impl Default for PrayerLangRun {
    fn default() -> Self {
        Self {
            script_source: String::new(),
            analyzed_script: None,
            frames: Vec::new(),
            memory: VecDeque::new(),
            mined_by_item: HashMap::new(),
            stored_by_item: HashMap::new(),
            is_halted: false,
            is_finished: false,
            current_script_line: None,
            command_catalog: prayer_lang::catalog::default_command_catalog(),
        }
    }
}

/// Runtime integration around one PrayerLang producer and one scheduler.
pub struct RuntimeEngine {
    producer: PrayerLangRun,
    scheduler: Scheduler,
    queue_claim: Option<QueueClaim>,
    action_sequence: u64,
    action_run: Option<PersistedActionRun>,
    events: Vec<RuntimeEvent>,
}

const MAX_MEMORY: usize = 12;
impl Default for RuntimeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeEngine {
    /// Parse and materialize a linear PrayerLang override against current state.
    pub fn compile_override_script(
        script: &str,
        state: ExecutionReadContext<'_>,
    ) -> Result<Vec<prayer_actions::Action>, EngineError> {
        let parsed = AstProgram::parse(script)
            .map_err(|diagnostics| EngineError::Parse(render_diags(script, &diagnostics)))?;
        let normalized = parsed.normalize();
        let parsed = AstProgram::parse(&normalized)
            .map_err(|diagnostics| EngineError::Parse(render_diags(&normalized, &diagnostics)))?;
        let diagnostics = parsed.validate(&ValidationContext::with_defaults());
        if !diagnostics.is_empty() {
            return Err(EngineError::Parse(render_diags(&normalized, &diagnostics)));
        }
        let analyzed = parsed
            .analyze(&prayer_lang::catalog::default_command_catalog(), &crate::analysis_observation(Some(state)))
            .map_err(|errors| EngineError::Parse(render_analyzer_errors(&normalized, &errors)))?;
        let compiled = analyzed.compile().map_err(|error| EngineError::InvalidState(error.to_string()))?;
        Ok(compiled.nodes.into_iter().map(|node| match node {
            prayer_lang::PlanNode::Action(action) => action.materialize(),
        }).collect())
    }
    /// Create a new linear runtime.
    pub fn new() -> Self {
        Self {
            producer: PrayerLangRun::default(),
            scheduler: Scheduler::new(),
            queue_claim: None,
            action_sequence: 0,
            action_run: None,
            events: Vec::new(),
        }
    }

    /// Set and parse script, resetting runtime execution context.
    pub fn set_script(
        &mut self,
        script: &str,
        state: Option<ExecutionReadContext<'_>>,
    ) -> Result<String, EngineError> {
        let normalized = self.producer.load(script, state)?;
        self.scheduler = Scheduler::new();
        self.action_sequence = 0;
        self.queue_claim = Some(
            self.scheduler
                .claim(QueueOwner::PrayerLang {
                    run_id: RunId(uuid::Uuid::new_v4().to_string()),
                })
                .map_err(|error| EngineError::InvalidState(error.to_string()))?,
        );
        self.events.push(RuntimeEvent::ScriptLoaded);

        Ok(normalized)
    }

    /// Re-analyze the currently loaded script without resetting runtime
    /// execution state. Use after restoring a checkpoint once current state is
    /// available, so load-time analysis matches a normal script load.
    pub fn reanalyze_current_script(
        &mut self,
        state: Option<ExecutionReadContext<'_>>,
    ) -> Result<(), EngineError> {
        self.producer.reanalyze(state)
    }

    /// Return execution-only counters without mixing them into bot truth.
    pub fn execution_runtime_state(&self) -> ExecutionRuntimeState {
        ExecutionRuntimeState {
            script_mined_by_item: Arc::new(self.producer.mined_by_item.clone()),
            script_stored_by_item: Arc::new(self.producer.stored_by_item.clone()),
        }
    }

    /// Decide next command from AST walker. Returns `None` when halted or script complete.
    pub fn decide_next(&mut self, state: ExecutionReadContext<'_>) -> Result<Option<ResolvedAction>, EngineError> {
        if self.producer.is_halted {
            return Ok(None);
        }
        // The interrupt lane is checked between every executor step. A running
        // orchestration command therefore yields after each atomic API action.
        if let Some(command) = self.scheduled_running_command(Lane::Interrupt)? {
            return Ok(Some(command));
        }
        if !self.scheduler.snapshot().interrupt_pending.is_empty() {
            let destination = state.bot.effective_poi_id()
                .map(|value| prayer_actions::GoTarget::Poi(value.to_owned()))
                .or_else(|| state.bot.effective_system_id()
                    .map(|value| prayer_actions::GoTarget::System(value.to_owned())));
            let restoration = destination
                .map(|destination| prayer_actions::Action::Go { destination })
                .unwrap_or(prayer_actions::Action::Wait { ticks: 0 });
            self.scheduler.replace_interrupt_actions_by_policy(
                "client_return_to_origin",
                "client_return_to_origin_ready",
                restoration,
            );
            self.scheduler.start_next().map_err(|error| EngineError::InvalidState(error.to_string()))?;
            return self.scheduled_running_command(Lane::Interrupt);
        }
        if self.action_run.is_some() {
            if let Some(command) = self.scheduled_running_command(Lane::Normal)? {
                return Ok(Some(command));
            }
            self.scheduler.start_next().map_err(|error| EngineError::InvalidState(error.to_string()))?;
            return self.scheduled_running_command(Lane::Normal);
        }
        if let Some(command) = self.scheduled_running_command(Lane::Normal)? {
            return Ok(Some(command));
        }
        match self.producer.decide(state)? {
            PrayerLangDecision::Command(command) => {
                self.events.push(RuntimeEvent::CommandSelected(command.clone()));
                self.schedule_command(&command, Lane::Normal)?;
                Ok(Some(command))
            }
            PrayerLangDecision::Complete => {
                if let Some(claim) = self.queue_claim.take() {
                    self.scheduler.release(&claim).map_err(|error| EngineError::InvalidState(error.to_string()))?;
                }
                self.events.push(RuntimeEvent::Halted("script complete".into()));
                Ok(None)
            }
        }
    }

    fn scheduled_running_command(&self, lane: Lane) -> Result<Option<ResolvedAction>, EngineError> {
        let snapshot = self.scheduler.snapshot();
        let running = match lane {
            Lane::Normal => snapshot.running.as_ref(),
            Lane::Interrupt => snapshot.interrupt.as_ref(),
        };
        let Some(running) = running else {
            return Ok(None);
        };
        let mut command = crate::action_resolution::resolve_action(running.envelope.action.clone())
            .map_err(|error| EngineError::InvalidState(error.to_string()))?;
        command.source_line = self.producer.current_script_line;
        Ok(Some(command))
    }

    fn schedule_command(&mut self, command: &ResolvedAction, lane: Lane) -> Result<(), EngineError> {
        let snapshot = self.scheduler.snapshot();
        let already_running = match lane {
            Lane::Normal => snapshot.running.as_ref(),
            Lane::Interrupt => snapshot.interrupt.as_ref(),
        }
        .is_some_and(|running| {
            crate::action_resolution::materialize_action(command)
                .is_ok_and(|action| running.envelope.action == action)
        });
        if already_running {
            return Ok(());
        }

        self.action_sequence = self.action_sequence.saturating_add(1);
        let origin = match lane {
            Lane::Normal => ActionOrigin::PrayerLang {
                run_id: self.queue_claim.as_ref().and_then(|claim| match &claim.owner {
                    QueueOwner::PrayerLang { run_id } => Some(run_id.clone()),
                    _ => None,
                }).ok_or_else(|| EngineError::InvalidState("PrayerLang queue claim is missing".into()))?,
                source: None,
            },
            Lane::Interrupt => ActionOrigin::Interrupt {
                policy: "runtime".into(),
            },
        };
        let action = crate::action_resolution::materialize_action(command)
            .map_err(|error| EngineError::InvalidState(error.to_string()))?;
        let envelope = ActionEnvelope::new(
            format!("prayerlang-{}", self.action_sequence),
            action,
            origin,
        );
        match lane {
            Lane::Normal => {
                let claim = self.queue_claim.as_ref().ok_or_else(|| {
                    EngineError::InvalidState("PrayerLang queue claim is missing".into())
                })?;
                self.scheduler
                    .append(claim, [envelope])
                    .map_err(|error| EngineError::InvalidState(error.to_string()))?;
            }
            Lane::Interrupt => self
                .scheduler
                .append_interrupt(envelope)
                .map_err(|error| EngineError::InvalidState(error.to_string()))?,
        }
        self.scheduler
            .start_next()
            .map_err(|error| EngineError::InvalidState(error.to_string()))?;
        Ok(())
    }

    /// Submit command execution result back into runtime.
    pub fn execute_result(
        &mut self,
        command: &ResolvedAction,
        result: EngineExecutionResult,
        state: ExecutionReadContext<'_>,
    ) {
        let lane = if self.scheduler.snapshot().interrupt.is_some() {
            Lane::Interrupt
        } else {
            Lane::Normal
        };
        if result.completed {
            let _ = self
                .scheduler
                .complete(lane, prayer_actions::ActionOutcome::Succeeded);
        }
        if command.action.eq_ignore_ascii_case("halt") {
            self.halt("halt command");
        }

        if command.action.eq_ignore_ascii_case("mine") {
            self.accumulate_deltas(state.bot.last_mined.as_ref(), true);
        }
        if command.action.eq_ignore_ascii_case("transfer") {
            self.accumulate_deltas(state.bot.last_stored.as_ref(), false);
        }

        self.push_memory(ActionMemory {
            action: command.action.clone(),
            args: command.args_as_strings(),
            result_message: result.result_message.clone(),
        });
        if result.completed {
            self.events
                .push(RuntimeEvent::CommandCompleted(command.clone()));
            if lane == Lane::Normal {
                if let Some(frame) = self.producer.frames.first_mut() {
                    frame.active_command = None;
                }
                if self.action_run.is_some() {
                    let drained = self.scheduler.snapshot().pending.is_empty();
                    if drained {
                        if let Some(run) = self.action_run.as_mut() {
                            run.outcome = Some(ActionBatchOutcome::Succeeded);
                        }
                        if let Some(claim) = self.queue_claim.take() {
                            let _ = self.scheduler.release(&claim);
                        }
                    }
                } else {
                    self.finish_linear_run_if_drained();
                }
            }
        }

        if result.halt_script {
            self.producer.current_script_line = None;
            self.halt("script halted by command");
        }
    }

    fn finish_linear_run_if_drained(&mut self) {
        let complete = self.producer.frames.first().is_some_and(|frame| {
            frame.active_command.is_none() && self.producer.analyzed_script.as_ref()
                .is_some_and(|program| frame.index >= program.statements.len())
        });
        if complete {
            self.producer.frames.clear();
            self.producer.is_finished = true;
            self.producer.current_script_line = None;
        }
    }

    /// Continuation state of the currently-active multi-step command, if any.
    pub fn active_command_state(&self) -> Option<ActiveCommandState> {
        let scheduler = self.scheduler.snapshot();
        if let Some(continuation) = scheduler.interrupt.as_ref().and_then(|r| r.continuation.as_ref())
            .or_else(|| scheduler.running.as_ref().and_then(|r| r.continuation.as_ref()))
        {
            return serde_json::from_value(continuation.state.clone()).ok();
        }
        self.producer.frames.first().and_then(|frame| frame.active_command.clone())
    }

    /// Replace the continuation state of the currently-active command.
    ///
    /// The owning frame is always the top frame while a command is in flight
    /// (commands never push frames), mirroring how `execute_result` clears it.
    pub fn set_active_command_state(&mut self, state: Option<ActiveCommandState>) {
        let lane = if self.scheduler.snapshot().interrupt.is_some() { Lane::Interrupt } else { Lane::Normal };
        if let Some(value) = state.as_ref().and_then(|state| serde_json::to_value(state).ok()) {
            let _ = self.scheduler.set_continuation(lane, ContinuationEnvelope {
                schema_version: 1,
                executor: "prayer-runtime".into(),
                state: value,
            });
        } else {
            let _ = self.scheduler.clear_continuation(lane);
        }
        if lane == Lane::Normal {
            if let Some(frame) = self.producer.frames.first_mut() {
                frame.active_command = state;
            }
        }
    }

    pub fn try_acquire_action_run(&mut self, run_id: RunId) -> Result<QueueClaim, EngineError> {
        let claim = self.scheduler.claim(QueueOwner::Manual { run_id: run_id.clone() })
            .map_err(|error| EngineError::InvalidState(error.to_string()))?;
        self.queue_claim = Some(claim.clone());
        self.action_run = Some(PersistedActionRun { run_id, actions: Vec::new(), outcome: None });
        Ok(claim)
    }

    pub fn submit_action_batch(&mut self, claim: &QueueClaim, actions: Vec<ActionEnvelope>) -> Result<(), EngineError> {
        if actions.is_empty() {
            return Err(EngineError::InvalidState("action batch must not be empty".into()));
        }
        let run = self.action_run.as_mut().ok_or_else(|| EngineError::InvalidState("manual action run is missing".into()))?;
        if !run.actions.is_empty() {
            return Err(EngineError::InvalidState("action batch was already submitted".into()));
        }
        self.scheduler.append(claim, actions.clone()).map_err(|error| EngineError::InvalidState(error.to_string()))?;
        run.actions = actions;
        Ok(())
    }

    /// Append a client-selected batch to the higher-precedence execution lane.
    pub fn submit_action_override(&mut self, actions: Vec<ActionEnvelope>) -> Result<(), EngineError> {
        if actions.is_empty() {
            return Err(EngineError::InvalidState("override action batch must not be empty".into()));
        }
        self.scheduler.append_interrupts(actions)
            .map_err(|error| EngineError::InvalidState(error.to_string()))
    }

    pub fn action_run(&self, run_id: &RunId) -> Option<PersistedActionRun> {
        self.action_run.as_ref().filter(|run| &run.run_id == run_id).cloned()
    }

    pub fn cancel_action_run(&mut self, run_id: &RunId, reason: String) -> Result<PersistedActionRun, EngineError> {
        let run = self.action_run.as_mut().filter(|run| &run.run_id == run_id)
            .ok_or_else(|| EngineError::InvalidState("action run not found".into()))?;
        if run.outcome.is_none() {
            run.outcome = Some(ActionBatchOutcome::Cancelled { reason: reason.clone() });
            if let Some(claim) = self.queue_claim.take() {
                self.scheduler.cancel(&claim, reason).map_err(|error| EngineError::InvalidState(error.to_string()))?;
            }
        }
        Ok(run.clone())
    }

    pub fn fail_action_run(&mut self, message: String) -> Result<Option<PersistedActionRun>, EngineError> {
        let Some(run) = self.action_run.as_mut() else {
            return Ok(None);
        };
        if run.outcome.is_none() {
            let action_index = self
                .scheduler
                .completed()
                .iter()
                .filter(|completed| {
                    matches!(completed.outcome, prayer_actions::ActionOutcome::Succeeded)
                })
                .count();
            run.outcome = Some(ActionBatchOutcome::Failed { action_index, message });
            // An execution error may already have followed the script-style halt path,
            // which latches the scheduler and drops the claim. A failed typed batch is
            // terminal, so clear all remaining work and immediately reopen the lane.
            self.clear("action batch failed");
        }
        Ok(self.action_run.clone())
    }

    /// Stop all runtime execution and leave the scheduler immediately reusable.
    pub fn halt(&mut self, reason: &str) {
        self.producer.is_halted = false;
        self.producer.is_finished = true;
        self.scheduler.halt(reason);
        self.queue_claim = None;
        self.events.push(RuntimeEvent::Halted(reason.to_string()));
    }

    /// Cancel all work and leave the runtime idle and immediately reusable.
    pub fn clear(&mut self, reason: &str) {
        self.halt(reason);
    }

    /// Typed scheduler state for API and persistence projections.
    pub fn scheduler_snapshot(&self) -> prayer_scheduler::SchedulerSnapshot {
        self.scheduler.snapshot()
    }

    pub fn normal_lane_claim(&self) -> Option<QueueClaim> {
        self.scheduler.snapshot().claim
    }

    pub fn override_lane_busy(&self) -> bool {
        let snapshot = self.scheduler.snapshot();
        snapshot.interrupt.is_some() || !snapshot.interrupt_pending.is_empty()
    }


    pub fn producer_snapshot(&self) -> crate::execution::ProducerSnapshot {
        self.producer.producer_snapshot()
    }

    /// Authoritative combined execution projection for every external surface.
    pub fn execution_snapshot(&self) -> crate::execution::ExecutionSnapshot {
        crate::execution::ExecutionSnapshot {
            scheduler: self.scheduler_snapshot(),
            producer: self.producer_snapshot(),
            interrupt_producer: None,
            source_prayer: self.producer.script_source.clone(),
            queue_prayer: self.scheduler_prayer_projection(),
            normal_queue_prayer: self.normal_scheduler_prayer_projection(),
            override_queue_prayer: self.override_scheduler_prayer_projection(),
            active_continuation: self.active_command_state()
                .and_then(|state| serde_json::to_value(state).ok()),
        }
    }

    /// Render the scheduler's executable contents as PrayerLang.
    /// Ownership and lifecycle metadata remain available in the structured snapshot.
    pub fn scheduler_prayer_projection(&self) -> String {
        [self.override_scheduler_prayer_projection(), self.normal_scheduler_prayer_projection()]
            .into_iter()
            .filter(|lane| !lane.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Render only the normal lane as PrayerLang.
    pub fn normal_scheduler_prayer_projection(&self) -> String {
        let snapshot = self.scheduler.snapshot();
        let mut lines = Vec::new();
        if let Some(running) = &snapshot.running {
            lines.push(prayer_lang::render_action(&running.envelope.action));
        }
        lines.extend(snapshot.pending.iter().map(|envelope| prayer_lang::render_action(&envelope.action)));
        lines.join("\n")
    }

    /// Render only the higher-precedence override lane as PrayerLang.
    pub fn override_scheduler_prayer_projection(&self) -> String {
        let snapshot = self.scheduler.snapshot();
        let mut lines = Vec::new();
        if let Some(interrupt) = &snapshot.interrupt {
            lines.push(prayer_lang::render_action(&interrupt.envelope.action));
        }
        lines.extend(snapshot.interrupt_pending.iter().map(|envelope| prayer_lang::render_action(&envelope.action)));
        lines.join("\n")
    }

    /// Drain emitted runtime events.
    pub fn drain_events(&mut self) -> Vec<RuntimeEvent> {
        std::mem::take(&mut self.events)
    }

    /// Render a runtime error diagnostic against the current script context.
    pub fn render_runtime_error(&self, message: impl Into<String>) -> String {
        let message = message.into();
        render_runtime_error(&self.producer.script_source, self.producer.current_script_line, &message)
    }

    fn accumulate_deltas(&mut self, deltas: &HashMap<String, i64>, mined: bool) {
        let target = if mined {
            &mut self.producer.mined_by_item
        } else {
            &mut self.producer.stored_by_item
        };

        for (item, amount) in deltas {
            if *amount <= 0 {
                continue;
            }
            *target.entry(item.clone()).or_insert(0) += *amount;
        }
    }

    fn push_memory(&mut self, memory: ActionMemory) {
        if self.producer.memory.len() >= MAX_MEMORY {
            let _ = self.producer.memory.pop_front();
        }
        self.producer.memory.push_back(memory);
    }

}

fn render_analyzed_nodes(nodes: &[AnalyzedNode]) -> String {
    AstProgram {
        statements: nodes.iter().map(analyzed_node_to_ast).collect(),
    }
    .normalize()
}

fn active_frame_source_line(
    script: &str,
    nodes: &[AnalyzedNode],
    frame: &ExecutionFrame,
) -> Option<usize> {
    let active_index = frame.index.checked_sub(1)?;
    if active_index >= nodes.len() {
        return None;
    }
    let base = analyzed_node_span_start(nodes.first()?)?;
    let active = analyzed_node_span_start(nodes.get(active_index)?)?;
    active
        .checked_sub(base)
        .map(|offset| offset_to_line(script, offset))
}

fn analyzed_node_span_start(node: &AnalyzedNode) -> Option<usize> {
    Some(match node {
        AnalyzedNode::Command(command) => command.source.span.start,
        AnalyzedNode::Transfer(transfer) => transfer.source.span.start,
        AnalyzedNode::Craft(craft) => craft.source.span.start,
        AnalyzedNode::Say(v) => v.span.start,
        AnalyzedNode::Buy(v) => v.span.start,
        AnalyzedNode::Sell(v) => v.span.start,
        AnalyzedNode::Recycle(v) => v.source.span.start,
        AnalyzedNode::CommissionShip(v) => v.span.start,
    })
}

fn analyzed_node_to_ast(node: &AnalyzedNode) -> AstNode {
    match node {
        AnalyzedNode::Command(command) => AstNode::Command(command.source.clone()),
        AnalyzedNode::Transfer(transfer) => AstNode::Transfer(transfer.source.clone()),
        AnalyzedNode::Craft(craft) => AstNode::Craft(craft.source.clone()),
        AnalyzedNode::Say(v) => AstNode::Say(v.clone()),
        AnalyzedNode::Buy(v) => AstNode::Buy(v.clone()),
        AnalyzedNode::Sell(v) => AstNode::Sell(v.clone()),
        AnalyzedNode::Recycle(v) => AstNode::Recycle(v.source.clone()),
        AnalyzedNode::CommissionShip(v) => AstNode::CommissionShip(v.clone()),
    }
}

fn craft_to_command(craft: &AnalyzedCraft, script_source: &str) -> ResolvedAction {
    let mut args = vec![
        ActionArg::RecipeId(craft.recipe_id.clone()),
        ActionArg::Integer(craft.source.quantity as i64),
        ActionArg::Any(format!(
            "deliver_to={}",
            craft
                .source
                .clauses
                .deliver_to
                .as_deref()
                .unwrap_or("storage")
        )),
    ];
    if let Some(source) = &craft.source.clauses.source {
        args.push(ActionArg::Any(format!("source={source}")));
    }
    if let Some(facility_id) = &craft.source.clauses.facility_id {
        args.push(ActionArg::Any(format!("facility_id={facility_id}")));
    }
    if let Some(preset) = &craft.source.clauses.preset {
        args.push(ActionArg::Any(format!("preset={preset}")));
    }

    ResolvedAction {
        action: "craft".to_string(),
        args,
        source_line: Some(offset_to_line(script_source, craft.source.span.start)),
    }
}

fn transfer_to_command(transfer: &AnalyzedTransfer, script_source: &str) -> ResolvedAction {
    let mut args = Vec::new();
    if transfer.items.is_empty() {
        match &transfer.subject {
            AnalyzedTransferSubject::AllCargo => {
                args.push(ActionArg::Any("all".to_string()));
            }
            AnalyzedTransferSubject::Credits(qty) => {
                args.push(ActionArg::Any("credits".to_string()));
                args.push(ActionArg::Integer(*qty));
            }
            AnalyzedTransferSubject::Item { id, qty } => {
                args.push(ActionArg::Any("item".to_string()));
                args.push(ActionArg::ItemId(id.clone()));
                if let Some(qty) = qty {
                    args.push(ActionArg::Integer(*qty));
                } else {
                    args.push(ActionArg::Any("all".to_string()));
                }
            }
            AnalyzedTransferSubject::Ship { id } => {
                args.push(ActionArg::Any("ship".to_string()));
                args.push(ActionArg::ShipId(id.clone()));
            }
            AnalyzedTransferSubject::Module { id } => {
                args.push(ActionArg::Any("module".to_string()));
                args.push(ActionArg::ModuleId(id.clone()));
            }
        }
        args.push(ActionArg::Any(format_transfer_endpoint(&transfer.from)));
        args.push(ActionArg::Any(format_transfer_endpoint(&transfer.to)));
    } else {
        args.push(ActionArg::Any("items".to_string()));
        args.push(ActionArg::Integer(transfer.items.len() as i64));
        for item in &transfer.items {
            args.push(ActionArg::ItemId(item.id.clone()));
            args.push(ActionArg::Integer(item.qty));
        }
        args.push(ActionArg::Any(format_transfer_endpoint(&transfer.from)));
        args.push(ActionArg::Any(format_transfer_endpoint(&transfer.to)));
    }

    ResolvedAction {
        action: "transfer".to_string(),
        args,
        source_line: Some(offset_to_line(script_source, transfer.source.span.start)),
    }
}

fn format_transfer_endpoint(endpoint: &AnalyzedTransferEndpoint) -> String {
    match endpoint {
        AnalyzedTransferEndpoint::Cargo => "cargo".to_string(),
        AnalyzedTransferEndpoint::Storage => "storage".to_string(),
        AnalyzedTransferEndpoint::Faction => "faction".to_string(),
        AnalyzedTransferEndpoint::FactionTag(tag) => format!("faction:{tag}"),
        AnalyzedTransferEndpoint::Player(name) => format!("player:{name}"),
        AnalyzedTransferEndpoint::Space(Some(id)) => format!("space:{id}"),
        AnalyzedTransferEndpoint::Space(None) => "space".to_string(),
        AnalyzedTransferEndpoint::Commission(id) => format!("commission:{id}"),
    }
}

fn render_diags(script: &str, diags: &[prayer_lang::Diagnostic]) -> String {
    diags
        .iter()
        .map(|d| {
            format!(
                "script.dsl:{}:{}: {}: {}",
                offset_to_line(script, d.span.start),
                offset_to_col(script, d.span.start),
                d.code,
                d.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_analyzer_errors(script: &str, errs: &[AnalyzerError]) -> String {
    errs.iter()
        .map(|e| {
            format!(
                "script.dsl:{}:{}: {}",
                offset_to_line(script, e.span.start),
                e.arg_index + 1,
                e.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_runtime_error(script: &str, line: Option<usize>, message: &str) -> String {
    if script.is_empty() {
        return message.to_string();
    }

    let span = line
        .and_then(|line| line_to_span(script, line))
        .unwrap_or(0..1.min(script.len()));
    format!(
        "script.dsl:{}:{}: runtime.error: {}",
        offset_to_line(script, span.start),
        offset_to_col(script, span.start),
        message
    )
}

fn line_to_span(text: &str, line: usize) -> Option<Range<usize>> {
    if line == 0 || text.is_empty() {
        return None;
    }

    let mut starts = vec![0usize];
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(idx + 1);
        }
    }

    let start = *starts.get(line.saturating_sub(1))?;
    let mut end = if line < starts.len() {
        starts[line].saturating_sub(1)
    } else {
        text.len()
    };
    if end <= start {
        end = (start + 1).min(text.len());
    }
    Some(start..end)
}

fn offset_to_line(text: &str, offset: usize) -> usize {
    let mut line = 1usize;
    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    line
}

fn offset_to_col(text: &str, offset: usize) -> usize {
    let mut col = 1usize;
    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            col = 1;
        } else {
            col += 1;
        }
    }
    col
}

fn active_state_for_command(
    command: &ResolvedAction,
    state: ExecutionReadContext<'_>,
) -> Option<ActiveCommandState> {
    let origin = Some(CommandOrigin::capture(command, state));
    match command.action.as_str() {
        "mine" => Some(ActiveCommandState::Mine(MineState {
            resource: command.args.first().map(ActionArg::as_text),
            origin,
            ..MineState::default()
        })),
        "go" => Some(ActiveCommandState::Go(GoState {
            target: command
                .args
                .first()
                .map(ActionArg::as_text)
                .unwrap_or_default(),
            origin,
            ..GoState::default()
        })),
        "refuel" => Some(ActiveCommandState::Refuel(RefuelState {
            origin,
            ..RefuelState::default()
        })),
        "find" => Some(ActiveCommandState::Find(FindState {
            origin,
            ..FindState::default()
        })),
        "wait" => {
            let ticks = crate::orchestration::parse_wait_ticks(command);
            Some(ActiveCommandState::Wait(WaitState {
                total_ticks: ticks,
                remaining_ticks: ticks,
                origin,
            }))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Runtime execution stack frame persisted in checkpoints.
pub struct ExecutionFrame {
    kind: ExecutionFrameKind,
    index: usize,
    source_line: Option<usize>,
    active_command: Option<ActiveCommandState>,
}

impl ExecutionFrame {
    fn root() -> Self {
        Self {
            kind: ExecutionFrameKind::Root,
            index: 0,
            source_line: Some(1),
            active_command: None,
        }
    }
}

/// Frame kind for runtime AST walker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionFrameKind {
    /// Root script frame.
    Root,
}

/// Command-handler trait for single-turn operations.
pub trait SingleTurnCommandHandler: Send + Sync {
    /// Command name.
    fn name(&self) -> &str;
    /// Execute command.
    fn execute(
        &self,
        command: &ResolvedAction,
        state: ExecutionReadContext<'_>,
    ) -> Result<EngineExecutionResult, EngineError>;
}

/// Command-handler trait for multi-turn operations.
pub trait MultiTurnCommandHandler: Send + Sync {
    /// Command name.
    fn name(&self) -> &str;
    /// Start command.
    fn start(
        &self,
        command: &ResolvedAction,
        state: ExecutionReadContext<'_>,
    ) -> Result<(bool, EngineExecutionResult), EngineError>;
    /// Continue command.
    fn continue_run(&self, state: ExecutionReadContext<'_>)
        -> Result<(bool, EngineExecutionResult), EngineError>;
}

// The former evaluator suite exercised removed control-flow, macro, library,
// override, and combat-policy semantics. Linear execution is covered at the
// parser/compiler and runtime integration boundaries.
#[cfg(any())]
mod tests {
    use super::*;
    use prayer_lang::{ArgSpec, ArgType, CommandSpec, PredicateSpec, ValidationContext};

    #[derive(Clone, Default)]
    struct TestReadState {
        bot: BotState,
        world: crate::read_context::WorldReadState,
        runtime: ExecutionRuntimeState,
    }

    fn state() -> TestReadState {
        TestReadState {
            bot: BotState {
                fuel_pct: 100,
                location: spacemolt_lib_rs::schema::V2GameStateLocation {
                    system_id: Some("sol".into()),
                    ..Default::default()
                },
                player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                    home_base: Some("earth".into()),
                    ..Default::default()
                },
                ..Default::default()
            },
            world: crate::read_context::WorldReadState {
                nearest_station: Some("earth_station".into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn context(state: &TestReadState) -> ExecutionReadContext<'_> {
        ExecutionReadContext {
            bot: &state.bot,
            world: &state.world,
            runtime: &state.runtime,
        }
    }

    #[test]
    fn halts_on_script_completion() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("halt;", None).expect("set script");
        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
        engine.execute_result(&cmd, EngineExecutionResult::default(), context(&state()));
        assert!(!engine.snapshot().is_halted);
        assert!(engine.snapshot().is_finished);
    }

    #[test]
    fn separate_bot_contexts_drive_here_cargo_and_ownership() {
        let mut first = state();
        first.bot.location.system_id = Some("alpha".to_string());
        first.bot.cargo = Arc::new(HashMap::from([("iron".to_string(), 4)]));
        first.bot.owned_ship_details = Arc::new(vec![
            spacemolt_lib_rs::schema::OwnedShipInfo {
                ship_id: "ship-alpha".to_string(),
                class_id: "runner".to_string(),
                is_active: true,
                cargo_used: None,
                class_name: None,
                custom_name: None,
                fuel: None,
                hull: None,
                listing_base_id: None,
                listing_id: None,
                listing_price: None,
                location: None,
                location_base_id: None,
                modules: None,
            },
        ]);
        first.world.catalog = Arc::new(CatalogData {
            items: HashMap::from([(
                "iron".to_string(),
                serde_json::from_value(serde_json::json!({
                    "base_value": 1, "category": "ore", "description": "Iron",
                    "id": "iron", "name": "Iron", "size": 1,
                    "stackable": true, "tradeable": true
                }))
                    .expect("catalog item"),
            )]),
            ..CatalogData::default()
        });

        let mut second = state();
        second.bot.location.system_id = Some("beta".to_string());
        second.bot.cargo = Arc::new(HashMap::from([("water".to_string(), 9)]));
        second.bot.owned_ship_details = Arc::new(vec![
            spacemolt_lib_rs::schema::OwnedShipInfo {
                ship_id: "ship-beta".to_string(),
                class_id: "runner".to_string(),
                is_active: true,
                cargo_used: None,
                class_name: None,
                custom_name: None,
                fuel: None,
                hull: None,
                listing_base_id: None,
                listing_id: None,
                listing_price: None,
                location: None,
                location_base_id: None,
                modules: None,
            },
        ]);
        second.world.catalog = Arc::clone(&first.world.catalog);

        let first_analysis = crate::analysis::analysis_observation(Some(context(&first)));
        let second_analysis = crate::analysis::analysis_observation(Some(context(&second)));
        assert!(first_analysis.owned_ship_ids.contains("ship-alpha"));
        assert!(!first_analysis.owned_ship_ids.contains("ship-beta"));
        assert!(second_analysis.owned_ship_ids.contains("ship-beta"));
        assert!(!second_analysis.owned_ship_ids.contains("ship-alpha"));

        let mut first_engine = RuntimeEngine::new();
        first_engine
            .set_script(
                "if CARGO(iron) > 0 { go $here; }",
                Some(context(&first)),
            )
            .expect("first script");
        let first_command = first_engine
            .decide_next(context(&first))
            .expect("first decision")
            .expect("first command");
        assert_eq!(first_command.args_as_strings(), ["alpha"]);

        let mut second_engine = RuntimeEngine::new();
        second_engine
            .set_script(
                "if CARGO(iron) > 0 { go $here; }",
                Some(context(&second)),
            )
            .expect("second script");
        assert!(second_engine
            .decide_next(context(&second))
            .expect("second decision")
            .is_none());
    }

    #[test]
    fn combat_decision_ignores_script_halt_state() {
        let mut s = state();
        s.bot.in_battle = true;
        let mut engine = RuntimeEngine::new();
        engine.halt("parked");

        let cmd = engine.decide_combat_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "flee");
        assert!(!engine.snapshot().is_halted);
        assert!(engine.snapshot().is_finished);
    }

    #[test]
    fn combat_result_does_not_advance_script_frame() {
        let mut battle = state();
        battle.bot.in_battle = true;
        let clear = state();
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("go $here; halt;", Some(context(&clear)))
            .expect("script");

        let combat = engine
            .decide_combat_next(context(&battle))
            .expect("decide combat")
            .expect("combat command");
        assert_eq!(combat.action, "flee");
        engine.execute_combat_result(&combat, EngineExecutionResult::default());

        let script = engine
            .decide_next(context(&clear))
            .expect("decide script")
            .expect("script command");
        assert_eq!(script.action, "go");
        assert_eq!(script.args_as_strings(), vec!["sol".to_string()]);
    }

    #[test]
    fn bare_attack_command_forces_target_id() {
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("attack dummy_target;", Some(context(&state())))
            .expect("script");

        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "attack");
        assert_eq!(cmd.args_as_strings(), vec!["dummy_target".to_string()]);
    }

    #[test]
    fn until_rewinds_when_false() {
        let mut s = state();
        s.bot.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("until FUEL() >= 50 { halt; }", None)
            .expect("script");

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
    }

    #[test]
    fn home_macro_materializes_to_home_poi() {
        let mut s = state();
        s.bot.player.home_base = Some("earth_base".to_string());
        s.bot.player.home_poi = Some("earth_poi".to_string());
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("go $home;", Some(context(&s))).expect("script");

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "go");
        assert_eq!(cmd.args_as_strings(), vec!["earth_poi".to_string()]);
    }

    #[test]
    fn checkpoint_roundtrip() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("go alpha;", None).expect("script");
        let cp = engine.execution_checkpoint().expect("checkpoint");

        let mut restored = RuntimeEngine::new();
        restored.restore_execution_checkpoint(cp).expect("restore");
        assert_eq!(restored.snapshot().script.trim(), "go alpha;");
    }

    #[test]
    fn checkpoint_preserves_load_time_here_analysis() {
        let mut load_state = state();
        load_state.bot.location.system_id = Some("alpha".to_string());
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("go $here;", Some(context(&load_state)))
            .expect("script");
        let cp = engine.execution_checkpoint().expect("checkpoint");

        let mut resume_state = state();
        resume_state.bot.location.system_id = Some("beta".to_string());
        let mut restored = RuntimeEngine::new();
        restored.restore_execution_checkpoint(cp).expect("restore");

        let cmd = restored
            .decide_next(context(&resume_state))
            .expect("decide")
            .expect("cmd");
        assert_eq!(cmd.args_as_strings(), vec!["alpha".to_string()]);
    }

    #[test]
    fn checkpoint_preserves_active_multi_turn_command_state() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("go alpha;", None).expect("script");
        let _ = engine
            .decide_next(context(&state()))
            .expect("decide")
            .expect("command");
        let checkpoint = engine.execution_checkpoint().expect("checkpoint");

        let mut restored = RuntimeEngine::new();
        restored.restore_execution_checkpoint(checkpoint).expect("restore");

        let active = restored
            .producer.frames
            .last()
            .and_then(|f| f.active_command.clone());
        // Origin is captured at command selection and survives the restore.
        assert_eq!(
            active,
            Some(ActiveCommandState::Go(GoState {
                target: "alpha".to_string(),
                origin: Some(CommandOrigin {
                    args: vec!["alpha".to_string()],
                    system: state().bot.location.system_id,
                    poi: state().bot.location.poi_id,
                    docked: false,
                    in_transit: false,
                }),
                ..GoState::default()
            }))
        );
    }

    #[test]
    fn v1_active_command_state_without_origin_still_deserializes() {
        // Continuation state persisted before origin tracking existed.
        let v1_go = r#"{"Go":{"target":"alpha","resolved_system":null,"resolved_poi":null,"did_move":false}}"#;
        let restored: ActiveCommandState =
            serde_json::from_str(v1_go).expect("v1 Go state deserializes");
        assert_eq!(
            restored,
            ActiveCommandState::Go(GoState {
                target: "alpha".to_string(),
                ..GoState::default()
            })
        );

        let v1_mine = r#"{"Mine":{"resource":"iron","target_poi":null,"excluded_pois":[],"excluded_systems":[]}}"#;
        let restored: ActiveCommandState =
            serde_json::from_str(v1_mine).expect("v1 Mine state deserializes");
        assert_eq!(
            restored,
            ActiveCommandState::Mine(MineState {
                resource: Some("iron".to_string()),
                ..MineState::default()
            })
        );
    }

    #[test]
    fn set_script_rejects_here_macro_without_state_system() {
        let mut engine = RuntimeEngine::new();
        let err = engine
            .set_script("go $here;", Some(ExecutionReadContext::default()))
            .expect_err("expected analyzer error");
        assert!(err.to_string().contains("$here"));
    }

    #[test]
    fn set_script_rejects_unknown_command_with_default_context() {
        let mut engine = RuntimeEngine::new();
        let err = engine
            .set_script("warp alpha;", Some(ExecutionReadContext::default()))
            .expect_err("expected validation error");
        assert!(err.to_string().contains("DSL200"));
    }

    #[test]
    fn set_script_accepts_here_macro_with_state_system() {
        let mut engine = RuntimeEngine::new();
        let mut state = state();
        state.bot.location.system_id = Some("sol".to_string());
        let normalized = engine
            .set_script("go $here;", Some(context(&state)))
            .expect("set script");
        assert_eq!(normalized.trim(), "go $here;");
    }

    #[test]
    fn nearest_station_macro_resolves_at_emit_time() {
        let mut engine = RuntimeEngine::new();
        let mut load_state = state();
        load_state.world.nearest_station = Some("earth_station".to_string());
        let _ = engine
            .set_script("go $nearest_station;", Some(context(&load_state)))
            .expect("set script");

        let mut emit_state = state();
        emit_state.world.nearest_station = Some("mars_station".to_string());
        let cmd = engine
            .decide_next(context(&emit_state))
            .expect("decide")
            .expect("command");
        assert_eq!(
            cmd.args,
            vec![ActionArg::GoTarget("mars_station".to_string())]
        );
    }

    #[test]
    fn passenger_commands_emit_plain_command_args() {
        let mut engine = RuntimeEngine::new();
        let normalized = engine
            .set_script("load_passenger sol_central; unload_passenger all;", None)
            .expect("set script");
        assert_eq!(
            normalized,
            r#"load_passenger sol_central;
unload_passenger all;"#
        );

        let first = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(first.action, "load_passenger");
        assert_eq!(
            first.args,
            vec![ActionArg::PoiId("sol_central".to_string())]
        );
        engine.execute_result(
            &first,
            EngineExecutionResult {
                result_message: None,
                completed: true,
                halt_script: false,
            },
            context(&state()),
        );

        let second = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(second.action, "unload_passenger");
        assert_eq!(second.args, vec![ActionArg::Any("all".to_string())]);
    }

    #[test]
    fn analyzed_until_rewinds_when_false() {
        let mut s = state();
        s.bot.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("until FUEL() >= 50 { halt; }", Some(context(&s)))
            .expect("script");

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
    }

    #[test]
    fn incomplete_result_requeues_same_command() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("wait 1;", None).expect("set script");
        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "wait");
        let action_id = engine
            .scheduler_snapshot()
            .running
            .as_ref()
            .expect("scheduled action")
            .envelope
            .id
            .clone();
        engine.execute_result(
            &cmd,
            EngineExecutionResult {
                result_message: Some("still running".to_string()),
                completed: false,
                halt_script: false,
            },
            context(&state()),
        );

        let retry = engine
            .decide_next(context(&state()))
            .expect("decide retry")
            .expect("retry cmd");
        assert_eq!(retry.action, "wait");
        assert_eq!(
            engine
                .scheduler_snapshot()
                .running
                .expect("same running action")
                .envelope
                .id,
            action_id
        );
        assert_eq!(engine.snapshot().current_script_line, Some(1));
    }

    #[test]
    fn scheduler_prayer_projection_shows_exact_running_action() {
        let mut engine = RuntimeEngine::new();
        engine.set_script("wait 3;", None).expect("script");
        engine
            .decide_next(context(&state()))
            .expect("decide")
            .expect("command");

        let projection = engine.scheduler_prayer_projection();
        assert_eq!(projection, "wait 3;");
        assert!(!projection.contains("halt;"));
    }

    #[test]
    fn execution_snapshot_is_the_shared_projection_source() {
        let mut engine = RuntimeEngine::new();
        engine.set_script("go alpha;", None).expect("script");
        let command = engine.decide_next(context(&state())).expect("decide").expect("command");
        let snapshot = engine.execution_snapshot();
        assert_eq!(snapshot.scheduler, engine.scheduler_snapshot());
        assert_eq!(snapshot.producer, engine.producer_snapshot());
        assert_eq!(snapshot.source_prayer, "go alpha;");
        assert!(snapshot.queue_prayer.contains("go alpha;"));
        assert_eq!(snapshot.active_continuation, engine.active_command_state()
            .and_then(|state| serde_json::to_value(state).ok()));
        assert_eq!(command.source_line, Some(1));
    }

    #[test]
    fn low_fuel_override_preempts_requeued_go() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse(
            "override low_fuel when FUEL() < 50 { go $nearest_station; refuel; }",
        )
        .expect("library");
        engine.set_skill_library(library);
        let _ = engine
            .set_script("go blood_forge_smelting_works;", None)
            .expect("set script");

        let first = engine
            .decide_next(context(&state()))
            .expect("first decide")
            .expect("first command");
        assert_eq!(first.action, "go");
        assert_eq!(
            first.args,
            vec![ActionArg::GoTarget(
                "blood_forge_smelting_works".to_string()
            )]
        );
        engine.execute_result(
            &first,
            EngineExecutionResult {
                result_message: Some("Jumping toward blood_forge_smelting_works...".to_string()),
                completed: false,
                halt_script: false,
            },
            context(&state()),
        );

        let mut low_fuel = state();
        low_fuel.bot.fuel_pct = 49;
        low_fuel.world.nearest_station = Some("the_rampart_checkpoint".to_string());

        assert_eq!(
            engine.active_command_state(),
            Some(ActiveCommandState::Go(GoState {
                target: "blood_forge_smelting_works".to_string(),
                resolved_system: None,
                resolved_poi: None,
                did_move: false,
                origin: Some(CommandOrigin {
                    args: vec!["blood_forge_smelting_works".to_string()],
                    system: Some("sol".to_string()),
                    poi: None,
                    docked: false,
                    in_transit: false,
                }),
            }))
        );

        let override_go = engine
            .decide_next(context(&low_fuel))
            .expect("override decide")
            .expect("override command");
        assert_eq!(override_go.action, "go");
        assert_eq!(
            override_go.args,
            vec![ActionArg::GoTarget("the_rampart_checkpoint".to_string())]
        );
        assert_eq!(
            engine.snapshot().active_frame.expect("active frame").kind,
            "override"
        );
        assert_eq!(
            engine.active_command_state(),
            Some(ActiveCommandState::Go(GoState {
                target: "the_rampart_checkpoint".to_string(),
                resolved_system: None,
                resolved_poi: None,
                did_move: false,
                origin: Some(CommandOrigin {
                    args: vec!["the_rampart_checkpoint".to_string()],
                    system: Some("sol".to_string()),
                    poi: None,
                    docked: false,
                    in_transit: false,
                }),
            }))
        );
    }

    #[test]
    fn running_out_of_fuel_runs_safety_override_before_resuming_route() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse(
            "override low_fuel when FUEL() <= 5 { go $nearest_station; refuel; }",
        )
        .expect("library");
        engine.set_skill_library(library);
        let _ = engine
            .set_script("go blood_forge_smelting_works;", None)
            .expect("set script");

        let route_cmd = engine
            .decide_next(context(&state()))
            .expect("initial route decide")
            .expect("initial route command");
        assert_eq!(route_cmd.action, "go");
        assert_eq!(
            route_cmd.args,
            vec![ActionArg::GoTarget(
                "blood_forge_smelting_works".to_string()
            )]
        );
        engine.execute_result(
            &route_cmd,
            EngineExecutionResult {
                result_message: Some("Jumping toward blood_forge_smelting_works...".to_string()),
                completed: false,
                halt_script: false,
            },
            context(&state()),
        );

        let mut stranded = state();
        stranded.bot.fuel_pct = 0;
        stranded.bot.location.system_id = Some("midway".to_string());
        stranded.world.nearest_station = Some("midway_station".to_string());

        let safety_go = engine
            .decide_next(context(&stranded))
            .expect("safety override decide")
            .expect("safety override command");
        assert_eq!(safety_go.action, "go");
        assert_eq!(
            safety_go.args,
            vec![ActionArg::GoTarget("midway_station".to_string())]
        );
        let active_frame = engine.snapshot().active_frame.expect("active frame");
        assert_eq!(active_frame.kind, "override");
        assert_eq!(active_frame.name.as_deref(), Some("low_fuel"));

        let mut at_station_empty = stranded.clone();
        at_station_empty.bot.location.poi_id = Some("midway_station".to_string());
        engine.execute_result(
            &safety_go,
            EngineExecutionResult::default(),
            context(&at_station_empty),
        );

        let refuel = engine
            .decide_next(context(&at_station_empty))
            .expect("refuel decide")
            .expect("refuel command");
        assert_eq!(refuel.action, "refuel");
        assert_eq!(
            engine.snapshot().active_frame.expect("active frame").kind,
            "override"
        );

        let mut refueled = at_station_empty;
        refueled.bot.fuel_pct = 100;
        engine.execute_result(&refuel, EngineExecutionResult::default(), context(&refueled));

        let resumed_route = engine
            .decide_next(context(&refueled))
            .expect("route resume decide")
            .expect("resumed route command");
        assert_eq!(resumed_route.action, "go");
        assert_eq!(
            resumed_route.args,
            vec![ActionArg::GoTarget(
                "blood_forge_smelting_works".to_string()
            )]
        );
        assert_ne!(
            engine.snapshot().active_frame.expect("active frame").kind,
            "override"
        );
    }

    #[test]
    fn atomic_checkpoint_restores_independent_safety_prayerlang_run() {
        let mut engine = RuntimeEngine::new();
        engine.set_skill_library(
            SkillLibraryAst::parse(
                "override low_fuel when FUEL() <= 5 { go $nearest_station; refuel; }",
            )
            .expect("library"),
        );
        engine.set_script("go destination;", None).expect("script");
        let normal = engine.decide_next(context(&state())).expect("normal").expect("command");
        engine.execute_result(
            &normal,
            EngineExecutionResult { completed: false, ..Default::default() },
            context(&state()),
        );
        let mut low = state();
        low.bot.fuel_pct = 0;
        low.world.nearest_station = Some("safe_station".into());
        let safety = engine.decide_next(context(&low)).expect("safety").expect("command");
        let checkpoint = engine.execution_checkpoint().expect("checkpoint");

        let mut restored = RuntimeEngine::new();
        restored.restore_execution_checkpoint(checkpoint).expect("restore");
        restored.execute_result(&safety, EngineExecutionResult::default(), context(&low));
        let next = restored.decide_next(context(&low)).expect("next").expect("command");
        assert_eq!(next.action, "refuel");

        let mut full = low;
        full.bot.fuel_pct = 100;
        restored.execute_result(&next, EngineExecutionResult::default(), context(&full));
        let resumed = restored.decide_next(context(&full)).expect("resume").expect("command");
        assert_eq!(resumed.action, "go");
        assert_eq!(resumed.args, normal.args);
    }

    #[test]
    fn restored_active_final_command_resumes_after_override() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse("override low_fuel when FUEL() < 50 { refuel; }")
            .expect("library");
        engine.set_skill_library(library);
        let _ = engine.set_script("wait 1;", None).expect("set script");

        let first = engine
            .decide_next(context(&state()))
            .expect("first decide")
            .expect("first command");
        assert_eq!(first.action, "wait");
        let checkpoint = engine.execution_checkpoint().expect("checkpoint");

        let mut restored = RuntimeEngine::new();
        restored
            .restore_execution_checkpoint(checkpoint)
            .expect("restore checkpoint");

        let mut low_fuel = state();
        low_fuel.bot.fuel_pct = 10;
        let override_cmd = restored
            .decide_next(context(&low_fuel))
            .expect("override decide")
            .expect("override command");
        assert_eq!(override_cmd.action, "refuel");
        restored.execute_result(&override_cmd, EngineExecutionResult::default(), context(&state()));

        let resumed = restored
            .decide_next(context(&state()))
            .expect("resume decide")
            .expect("resumed command");
        assert_eq!(resumed.action, "wait");
    }

    #[test]
    fn completed_result_keeps_current_script_line_until_next_command() {
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("go alpha;\ngo beta;", None)
            .expect("set script");
        let first = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(first.action, "go");
        assert_eq!(engine.snapshot().current_script_line, Some(1));

        engine.execute_result(&first, EngineExecutionResult::default(), context(&state()));
        assert_eq!(engine.snapshot().current_script_line, Some(1));

        let second = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(second.action, "go");
        assert_eq!(engine.snapshot().current_script_line, Some(2));
    }

    #[test]
    fn natural_completion_clears_current_script_line() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("go alpha;", None).expect("set script");
        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        engine.execute_result(&cmd, EngineExecutionResult::default(), context(&state()));

        let next = engine.decide_next(context(&state())).expect("decide");
        assert!(next.is_none());
        let snapshot = engine.snapshot();
        assert!(snapshot.is_finished);
        assert_eq!(snapshot.current_script_line, None);
    }

    #[test]
    fn parses_with_external_validation_context() {
        let parsed = AstProgram::parse("go alpha;").expect("parse");
        let mut ctx = ValidationContext::default();
        ctx.commands.insert(
            "go".into(),
            CommandSpec {
                name: "go".into(),
                args: vec![ArgSpec {
                    name: "destination".into(),
                    kind: ArgType::GoTarget,
                    required: true,
                    variadic: false,
                }],
            },
        );
        ctx.numeric_predicates.insert(
            "FUEL".into(),
            PredicateSpec {
                name: "FUEL".into(),
                arity: 0,
            },
        );

        assert!(parsed.validate(&ctx).is_empty());
    }

    #[test]
    fn set_script_accepts_optional_sell_and_transfer_forms() {
        let mut engine = RuntimeEngine::new();
        let normalized = engine
            .set_script(
                "sell;\nsell iron 10 40 order;\ntransfer;\n",
                Some(ExecutionReadContext::default()),
            )
            .expect("set script");
        assert!(normalized.contains("sell;"));
        assert!(normalized.contains("sell order 10 iron at 40;"));
        assert!(normalized.contains("transfer;"));
    }

    #[test]
    fn combat_policy_selection_is_local_and_only_following_action_is_scheduled() {
        let mut engine = RuntimeEngine::new();
        engine.set_skill_library(
            SkillLibraryAst::parse("combat aggressive() { flee; }").expect("library"),
        );
        engine
            .set_script("combat aggressive; wait 1;", Some(ExecutionReadContext::default()))
            .expect("script");

        let command = engine
            .decide_next(ExecutionReadContext::default())
            .expect("decision")
            .expect("following action");
        assert_eq!(command.action, "wait");
        assert_eq!(
            engine.interrupt_policies.active_combat_policy.as_deref(),
            Some("aggressive")
        );
        assert_eq!(
            engine
                .scheduler
                .snapshot()
                .running
                .as_ref()
                .map(|entry| &entry.envelope.action),
            Some(&prayer_actions::Action::Wait { ticks: 1 })
        );
    }

    #[test]
    fn if_block_executes_body_when_condition_true() {
        let mut s = state();
        s.bot.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("if FUEL() < 50 { halt; }", Some(context(&s)))
            .expect("set script");
        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
    }

    #[test]
    fn snapshot_reports_main_active_frame() {
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("go alpha;\ngo beta;", None)
            .expect("set script");

        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "go");
        let frame = engine.snapshot().active_frame.expect("active frame");
        assert_eq!(frame.kind, "main");
        assert_eq!(frame.name, None);
        assert_eq!(frame.line, Some(1));
        assert!(frame.script.contains("go alpha;"));
    }

    #[test]
    fn snapshot_reports_override_active_frame() {
        let mut s = state();
        s.bot.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse("override refuel when FUEL() < 50 { refuel; }")
            .expect("library");
        engine.set_skill_library(library);
        let _ = engine.set_script("go alpha;", None).expect("set script");

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "refuel");
        let frame = engine.snapshot().active_frame.expect("active frame");
        assert_eq!(frame.kind, "override");
        assert_eq!(frame.name.as_deref(), Some("refuel"));
        assert_eq!(frame.line, Some(1));
        assert_eq!(frame.script.trim(), "refuel;");
    }

    #[test]
    fn completed_override_frame_is_not_reported_after_fuel_recovers() {
        let mut s = state();
        s.bot.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse("override low_fuel when FUEL() < 50 { refuel; }")
            .expect("library");
        engine.set_skill_library(library);
        let _ = engine.set_script("go alpha;", None).expect("set script");

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "refuel");

        let mut refueled = state();
        refueled.bot.fuel_pct = 100;
        refueled.bot.fuel = refueled.bot.max_fuel;
        engine.execute_result(&cmd, EngineExecutionResult::default(), context(&refueled));

        let frame = engine.snapshot().active_frame.expect("active frame");
        assert_eq!(frame.kind, "main");
        assert_ne!(frame.name.as_deref(), Some("low_fuel"));
    }

    #[test]
    fn if_block_skips_body_when_condition_false() {
        let mut s = state();
        s.bot.fuel_pct = 90;
        let mut engine = RuntimeEngine::new();
        // Pass None for state to skip go-target identity validation
        let _ = engine
            .set_script("if FUEL() < 50 { halt; }\ngo alpha;", None)
            .expect("set script");
        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        // condition is false so body is skipped, next statement executes
        assert_eq!(cmd.action, "go");
    }

    #[test]
    fn halt_clears_execution_without_latching_scheduler_state() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("halt;", None).expect("set script");
        engine.halt("manual");
        assert!(!engine.snapshot().is_halted);
        let scheduler = engine.scheduler_snapshot();
        assert!(!scheduler.halted);
        assert!(scheduler.claim.is_none());
        assert!(scheduler.running.is_none());
        assert!(scheduler.pending.is_empty());
    }

    #[test]
    fn failed_action_run_clears_a_previously_halted_scheduler() {
        use prayer_actions::{Action, ActionEnvelope, ActionOrigin, RunId};

        let mut engine = RuntimeEngine::new();
        let run_id = RunId("failed-run".into());
        let claim = engine
            .try_acquire_action_run(run_id.clone())
            .expect("claim action lane");
        engine
            .submit_action_batch(
                &claim,
                vec![ActionEnvelope::new(
                    "wait",
                    Action::Wait { ticks: 1 },
                    ActionOrigin::Manual {
                        run_id: run_id.clone(),
                    },
                )],
            )
            .expect("submit action batch");
        engine.scheduler.start_next().expect("start action");

        engine.halt("action execution failed");
        let failed = engine
            .fail_action_run("storage action already pending".into())
            .expect("record failed action run")
            .expect("action run");

        assert!(matches!(
            failed.outcome,
            Some(ActionBatchOutcome::Failed { action_index: 0, .. })
        ));
        let scheduler = engine.scheduler_snapshot();
        assert!(!scheduler.halted);
        assert!(scheduler.claim.is_none());
        assert!(scheduler.running.is_none());
        assert!(scheduler.pending.is_empty());
        assert!(engine
            .try_acquire_action_run(RunId("replacement-run".into()))
            .is_ok());
    }

    #[test]
    fn low_fuel_override_preempts_and_resumes_action_run() {
        use prayer_actions::{Action, ActionEnvelope, ActionOrigin, RunId};

        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse(
            "override low_fuel when FUEL() < 50 { refuel; }",
        )
        .expect("library");
        engine.set_skill_library(library);

        let run_id = RunId("low-fuel-action-run".into());
        let claim = engine
            .try_acquire_action_run(run_id.clone())
            .expect("claim action lane");
        engine
            .submit_action_batch(
                &claim,
                vec![ActionEnvelope::new(
                    "manual-wait",
                    Action::Wait { ticks: 1 },
                    ActionOrigin::Manual {
                        run_id: run_id.clone(),
                    },
                )],
            )
            .expect("submit action batch");

        let manual = engine
            .decide_next(context(&state()))
            .expect("manual decide")
            .expect("manual command");
        assert_eq!(manual.action, "wait");

        let mut low_fuel = state();
        low_fuel.bot.fuel_pct = 49;
        let interrupt = engine
            .decide_next(context(&low_fuel))
            .expect("override decide")
            .expect("override command");
        assert_eq!(interrupt.action, "refuel");
        assert_eq!(
            engine.snapshot().active_frame.expect("active frame").kind,
            "override"
        );

        let mut refueled = state();
        refueled.bot.fuel_pct = 100;
        engine.execute_result(
            &interrupt,
            EngineExecutionResult::default(),
            context(&refueled),
        );

        let resumed = engine
            .decide_next(context(&refueled))
            .expect("resume decide")
            .expect("resumed command");
        assert_eq!(resumed.action, "wait");
        assert!(engine.action_run(&run_id).is_some());
    }

    #[test]
    fn drain_events_returns_and_clears_events() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("halt;", None).expect("set script");
        let events = engine.drain_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::ScriptLoaded)));
        // second drain is empty
        let events2 = engine.drain_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn execute_result_emits_command_completed_event() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("go alpha;", None).expect("set script");
        let _ = engine.drain_events();
        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        engine.execute_result(&cmd, EngineExecutionResult::default(), context(&state()));
        let events = engine.drain_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::CommandCompleted(_))));
    }

    #[test]
    fn override_triggers_when_condition_met() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse("override low_fuel when FUEL() <= 10 { halt; }")
            .expect("library");
        engine.set_skill_library(library);
        let _ = engine.set_script("go alpha;", None).expect("set script");

        let mut s = state();
        s.bot.fuel_pct = 5;

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        // override fires before the script command
        assert_eq!(cmd.action, "halt");

        let events = engine.drain_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::OverrideTriggered(name) if name == "low_fuel")));
    }

    #[test]
    fn no_overrides_script_directive_disables_override_triggers() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse("override low_fuel when FUEL() <= 10 { halt; }")
            .expect("library");
        engine.set_skill_library(library);
        let normalized = engine
            .set_script("@no-overrides\ngo alpha;", None)
            .expect("set script");
        assert_eq!(normalized, "@no-overrides\ngo alpha;");

        let mut s = state();
        s.bot.fuel_pct = 5;

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "go");

        let events = engine.drain_events();
        assert!(!events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::OverrideTriggered(name) if name == "low_fuel")));
    }

    #[test]
    fn override_does_not_trigger_when_condition_not_met() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse("override low_fuel when FUEL() <= 10 { halt; }")
            .expect("library");
        engine.set_skill_library(library);
        let _ = engine.set_script("go alpha;", None).expect("set script");

        let mut s = state();
        s.bot.fuel_pct = 80;

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "go");
    }

    #[test]
    fn skill_invocation_executes_skill_body() {
        let mut engine = RuntimeEngine::new();
        let library =
            SkillLibraryAst::parse("skill refuel_and_go() { go alpha; }").expect("library");
        engine.set_skill_library(library);
        let _ = engine
            .set_script("refuel_and_go;", None)
            .expect("set script");
        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "go");
    }

    #[test]
    fn skill_nested_block_resolves_analyzed_frame_path() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse("skill guarded_go() { if FUEL() > 0 { go alpha; } }")
            .expect("library");
        engine.set_skill_library(library);
        let _ = engine.set_script("guarded_go;", None).expect("set script");

        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "go");
        assert_eq!(cmd.args_as_strings(), vec!["alpha".to_string()]);
    }

    #[test]
    fn override_nested_block_resolves_analyzed_frame_path() {
        let mut engine = RuntimeEngine::new();
        let library = SkillLibraryAst::parse(
            "override low_fuel when FUEL() < 50 { if CREDITS() >= 0 { refuel; } }",
        )
        .expect("library");
        engine.set_skill_library(library);
        let _ = engine.set_script("go alpha;", None).expect("set script");
        let mut s = state();
        s.bot.fuel_pct = 10;

        let cmd = engine.decide_next(context(&s)).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "refuel");
    }

    #[test]
    fn inject_session_counters_populates_script_mined() {
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("mine iron_ore;", None)
            .expect("set script");
        let cmd = engine.decide_next(context(&state())).expect("decide").expect("cmd");
        let mut post_state = state();
        post_state.bot.last_mined = std::sync::Arc::new(std::collections::HashMap::from([(
            "iron_ore".to_string(),
            5i64,
        )]));
        engine.execute_result(&cmd, EngineExecutionResult::default(), context(&post_state));

        let check_state = engine.execution_runtime_state();
        assert_eq!(
            check_state.script_mined_by_item.get("iron_ore").copied(),
            Some(5)
        );
    }
}
    #[test]
    fn client_override_preempts_a_running_normal_command() {
        let mut engine = RuntimeEngine::new();
        let run_id = RunId("normal-wait".into());
        let claim = engine.try_acquire_action_run(run_id.clone()).expect("claim");
        engine.submit_action_batch(&claim, vec![ActionEnvelope::new(
            "normal-wait",
            prayer_actions::Action::Wait { ticks: 3 },
            ActionOrigin::Manual { run_id },
        )]).expect("submit wait action");
        let normal = engine.decide_next(ExecutionReadContext::default()).expect("decide").expect("normal");
        assert_eq!(normal.action, "wait");
        engine.submit_action_override(vec![ActionEnvelope::new(
            "override-dock",
            prayer_actions::Action::Dock,
            ActionOrigin::Interrupt { policy: "client".into() },
        )]).expect("override");
        let override_action = engine.decide_next(ExecutionReadContext::default()).expect("decide").expect("override");
        assert_eq!(override_action.action, "dock");
        assert!(engine.scheduler_snapshot().running.expect("normal remains").paused);
        assert_eq!(engine.override_scheduler_prayer_projection(), "dock;");
        assert_eq!(engine.normal_scheduler_prayer_projection(), "wait 3;");
        engine.execute_result(
            &override_action,
            EngineExecutionResult::default(),
            ExecutionReadContext::default(),
        );
        assert_eq!(engine.override_scheduler_prayer_projection(), "");
        assert_eq!(engine.normal_scheduler_prayer_projection(), "wait 3;");
    }
