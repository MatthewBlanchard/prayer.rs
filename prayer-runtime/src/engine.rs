//! Prayer runtime engine and checkpoint model.

use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::sync::Arc;

use crate::dsl::{
    AnalyzedArg, AnalyzedNode, AnalyzedProgram, AnalyzedSkillAstNode, AnalyzedSkillLibrary,
    AnalyzerError, ArgType, AstProgram, CommandSpec, ComparisonOp, ConditionExpr, DynamicMacro,
    NumericOperand, PredicateSpec, SkillLibraryAst, ValidationContext,
};
use ariadne::{Color, Config, Label, Report, ReportKind, Source};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::state::{
    CatalogEntryData, GalaxyData, GameState, MarketData, MarketOrderInfo, MissionData,
    MissionInfoData, OpenOrderInfo, ShipState,
};

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

/// Command emitted by runtime for execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCommand {
    /// Command action name.
    pub action: String,
    /// Command arguments.
    pub args: Vec<CommandArg>,
    /// Optional source line.
    pub source_line: Option<usize>,
}

/// Typed runtime command argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandArg {
    /// Untyped argument.
    Any(String),
    /// Integer argument.
    Integer(i64),
    /// Item id argument.
    ItemId(String),
    /// System id argument.
    SystemId(String),
    /// Poi id argument.
    PoiId(String),
    /// Go target argument.
    GoTarget(String),
    /// Ship id argument.
    ShipId(String),
    /// Listing id argument.
    ListingId(String),
    /// Mission id argument.
    MissionId(String),
    /// Module id argument.
    ModuleId(String),
    /// Recipe id argument.
    RecipeId(String),
}

impl CommandArg {
    /// Convert typed argument to payload/display text.
    pub fn as_text(&self) -> String {
        match self {
            Self::Any(v)
            | Self::ItemId(v)
            | Self::SystemId(v)
            | Self::PoiId(v)
            | Self::GoTarget(v)
            | Self::ShipId(v)
            | Self::ListingId(v)
            | Self::MissionId(v)
            | Self::ModuleId(v)
            | Self::RecipeId(v) => v.clone(),
            Self::Integer(v) => v.to_string(),
        }
    }
}

impl EngineCommand {
    /// Convert command args to plain strings.
    pub fn args_as_strings(&self) -> Vec<String> {
        self.args.iter().map(CommandArg::as_text).collect()
    }
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
    /// `explore` command continuation state.
    Explore(ExploreState),
}

/// Persisted state for `mine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MineState {
    /// Optional resource id filter.
    pub resource: Option<String>,
    /// Selected mining target poi id.
    pub target_poi: Option<String>,
    /// Excluded poi ids while searching.
    pub excluded_pois: Vec<String>,
    /// Excluded system ids while searching.
    pub excluded_systems: Vec<String>,
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
}

/// Persisted state for `explore`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExploreState {
    /// Optional target system currently being explored.
    pub target_system: Option<String>,
    /// Unreachable system ids encountered.
    pub unreachable_systems: Vec<String>,
    /// Completion marker.
    pub completed: bool,
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

/// Snapshot of current runtime state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    /// Current normalized script.
    pub script: String,
    /// Halt state.
    pub is_halted: bool,
    /// Current script line.
    pub current_script_line: Option<usize>,
    /// Active frame kinds.
    pub frame_stack: Vec<ExecutionFrameKind>,
    /// Recent action memory.
    pub memory: Vec<ActionMemory>,
    /// Root mined counters.
    pub mined_by_item: HashMap<String, i64>,
    /// Root stashed counters.
    pub stashed_by_item: HashMap<String, i64>,
}

/// Persisted checkpoint schema v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCheckpoint {
    /// Schema version.
    pub version: u32,
    /// Script source.
    pub script: String,
    /// Halted state.
    pub is_halted: bool,
    /// Current script line.
    pub current_script_line: Option<usize>,
    /// Serialized execution frames.
    pub frames: Vec<ExecutionFrame>,
    /// Requeued commands.
    pub requeued_steps: Vec<EngineCommand>,
    /// Memory entries.
    pub memory: Vec<ActionMemory>,
    /// Counters.
    pub mined_by_item: HashMap<String, i64>,
    /// Counters.
    pub stashed_by_item: HashMap<String, i64>,
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
    CommandSelected(EngineCommand),
    /// Command completed.
    CommandCompleted(EngineCommand),
    /// Override fired.
    OverrideTriggered(String),
}

/// Runtime engine with frame-stack execution model.
pub struct RuntimeEngine {
    script_source: String,
    analyzed_script: Option<AnalyzedProgram>,
    skill_library: SkillLibraryAst,
    analyzed_skill_library: Option<AnalyzedSkillLibrary>,
    frames: Vec<ExecutionFrame>,
    requeued_steps: VecDeque<EngineCommand>,
    memory: VecDeque<ActionMemory>,
    mined_by_item: HashMap<String, i64>,
    stashed_by_item: HashMap<String, i64>,
    is_halted: bool,
    current_script_line: Option<usize>,
    events: Vec<RuntimeEvent>,
    bool_predicates: HashMap<String, BoolPredicate>,
    num_predicates: HashMap<String, NumPredicate>,
    bool_predicate_specs: HashMap<String, PredicateSpec>,
    num_predicate_specs: HashMap<String, PredicateSpec>,
    command_catalog: HashMap<String, CommandSpec>,
}

const MAX_MEMORY: usize = 12;
const ROOT_PATH: &str = "r";

type BoolPredicate = Arc<dyn Fn(&GameState, &[String]) -> Option<bool> + Send + Sync>;
type NumPredicate = Arc<dyn Fn(&GameState, &[String]) -> Option<i64> + Send + Sync>;

impl Default for RuntimeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeEngine {
    /// Create a new runtime with default predicate registry.
    pub fn new() -> Self {
        let mut engine = Self {
            script_source: String::new(),
            analyzed_script: None,
            skill_library: SkillLibraryAst {
                skills: Vec::new(),
                overrides: Vec::new(),
                disabled_commands: Default::default(),
            },
            analyzed_skill_library: None,
            frames: Vec::new(),
            requeued_steps: VecDeque::new(),
            memory: VecDeque::new(),
            mined_by_item: HashMap::new(),
            stashed_by_item: HashMap::new(),
            is_halted: false,
            current_script_line: None,
            events: Vec::new(),
            bool_predicates: HashMap::new(),
            num_predicates: HashMap::new(),
            bool_predicate_specs: HashMap::new(),
            num_predicate_specs: HashMap::new(),
            command_catalog: crate::catalog::default_command_catalog(),
        };
        engine.install_default_predicates();
        engine
    }

    /// Replace runtime skill library.
    pub fn set_skill_library(&mut self, library: SkillLibraryAst) {
        self.skill_library = library;
        self.analyzed_skill_library = None;
    }

    /// Set and parse script, resetting runtime execution context.
    pub fn set_script(
        &mut self,
        script: &str,
        state: Option<&GameState>,
    ) -> Result<String, EngineError> {
        let parsed =
            AstProgram::parse(script).map_err(|d| EngineError::Parse(render_diags(script, &d)))?;
        let validation_context = ValidationContext::with_defaults(Some(&self.skill_library));
        let validation_diags = parsed.validate(&validation_context);
        if !validation_diags.is_empty() {
            return Err(EngineError::Parse(render_diags(script, &validation_diags)));
        }
        let analysis_state = state.cloned().unwrap_or_default();
        let analyzed = parsed
            .analyze(&self.command_catalog, &analysis_state)
            .map_err(|errs| EngineError::Parse(render_analyzer_errors(script, &errs)))?;
        let analyzed_library = self
            .skill_library
            .analyze(&self.command_catalog, &analysis_state)
            .map_err(|errs| EngineError::Parse(render_analyzer_errors(script, &errs)))?;
        self.analyzed_script = Some(analyzed);
        self.analyzed_skill_library = Some(analyzed_library);

        self.script_source = parsed.normalize();
        self.frames.clear();
        self.frames.push(ExecutionFrame::root());
        self.requeued_steps.clear();
        self.memory.clear();
        self.mined_by_item.clear();
        self.stashed_by_item.clear();
        self.is_halted = false;
        self.current_script_line = None;
        self.events.push(RuntimeEvent::ScriptLoaded);

        Ok(self.script_source.clone())
    }

    /// Inject current script/session counters into the provided game state.
    pub fn inject_session_counters(&self, state: &mut GameState) {
        state.script_mined_by_item = Arc::new(self.mined_by_item.clone());
        state.script_stashed_by_item = Arc::new(self.stashed_by_item.clone());
    }

    /// Decide next command from AST walker. Returns `None` when halted or script complete.
    pub fn decide_next(&mut self, state: &GameState) -> Result<Option<EngineCommand>, EngineError> {
        self.try_trigger_override(state);

        if self.is_halted {
            return Ok(None);
        }

        if let Some(cmd) = self.requeued_steps.pop_front() {
            self.current_script_line = cmd.source_line;
            self.events.push(RuntimeEvent::CommandSelected(cmd.clone()));
            return Ok(Some(cmd));
        }

        loop {
            let Some(frame_idx) = self.frames.len().checked_sub(1) else {
                self.is_halted = true;
                self.events
                    .push(RuntimeEvent::Halted("script complete".to_string()));
                return Ok(None);
            };

            let (path, index, kind, until_condition, until_known, bindings) = {
                let frame = &self.frames[frame_idx];
                (
                    frame.path.clone(),
                    frame.index,
                    frame.kind,
                    frame.until_condition.clone(),
                    frame.until_condition_known,
                    frame.bindings.clone(),
                )
            };

            let analyzed_nodes = self.analyzed_nodes_for_path(&path).ok_or_else(|| {
                self.invalid_runtime_state("missing analyzed nodes for frame path")
            })?;
            if index >= analyzed_nodes.len() {
                if kind == ExecutionFrameKind::Until && until_known {
                    if let Some(cond) = until_condition.as_ref() {
                        if let Some(value) = self.eval_condition(cond, state, Some(&bindings)) {
                            if !value {
                                if let Some(frame) = self.frames.get_mut(frame_idx) {
                                    frame.index = 0;
                                }
                                continue;
                            }
                        }
                    }
                }
                self.frames.pop();
                continue;
            }

            let node = analyzed_nodes
                .get(index)
                .cloned()
                .ok_or_else(|| self.invalid_runtime_state("frame index out of bounds"))?;
            if let Some(frame) = self.frames.get_mut(frame_idx) {
                frame.index += 1;
            }

            match node {
                AnalyzedNode::Command(cmd) => {
                    let analyzed_args = self.materialize_analyzed_args(&cmd.args, state);
                    if let Some(skill) = self.find_analyzed_skill(&cmd.source.name) {
                        let new_bindings =
                            self.build_analyzed_skill_bindings(skill, &analyzed_args, &bindings);
                        self.frames
                            .push(ExecutionFrame::skill(skill.name.clone(), new_bindings));
                        continue;
                    }

                    let substituted = self.substitute_bindings(&analyzed_args, &bindings, state);
                    let action = cmd.source.name.to_lowercase();
                    let typed_args = self.cast_command_args(&action, substituted);
                    let line = offset_to_line(&self.script_source, cmd.source.span.start);
                    let out = EngineCommand {
                        action,
                        args: typed_args,
                        source_line: Some(line),
                    };

                    self.current_script_line = out.source_line;
                    self.events.push(RuntimeEvent::CommandSelected(out.clone()));
                    if let Some(frame) = self.frames.get_mut(frame_idx) {
                        frame.active_command = active_state_for_command(&out);
                    }
                    return Ok(Some(out));
                }
                AnalyzedNode::If(block) => {
                    let should_enter = self
                        .eval_condition(&block.condition, state, Some(&bindings))
                        .unwrap_or(true);
                    if should_enter {
                        self.frames.push(ExecutionFrame::block(
                            ExecutionFrameKind::If,
                            path.clone(),
                            index,
                            block.condition,
                            bindings.clone(),
                        ));
                    }
                }
                AnalyzedNode::Until(block) => {
                    let eval = self.eval_condition(&block.condition, state, Some(&bindings));
                    let should_enter = match eval {
                        Some(v) => !v,
                        None => true,
                    };
                    if should_enter {
                        let mut f = ExecutionFrame::block(
                            ExecutionFrameKind::Until,
                            path,
                            index,
                            block.condition,
                            bindings,
                        );
                        f.until_condition_known = eval.is_some();
                        self.frames.push(f);
                    }
                }
            }
        }
    }

    /// Submit command execution result back into runtime.
    pub fn execute_result(
        &mut self,
        command: &EngineCommand,
        result: EngineExecutionResult,
        state: &GameState,
    ) {
        if command.action.eq_ignore_ascii_case("halt") {
            self.halt("halt command");
        }

        if command.action.eq_ignore_ascii_case("mine") {
            self.accumulate_deltas(state.last_mined.as_ref(), true);
        }
        if command.action.eq_ignore_ascii_case("stash") {
            self.accumulate_deltas(state.last_stashed.as_ref(), false);
        }

        self.push_memory(ActionMemory {
            action: command.action.clone(),
            args: command.args_as_strings(),
            result_message: result.result_message.clone(),
        });
        if result.completed {
            self.events
                .push(RuntimeEvent::CommandCompleted(command.clone()));
            if let Some(frame) = self.frames.last_mut() {
                frame.active_command = None;
            }
        } else {
            self.requeue_step(command.clone());
        }

        if result.halt_script {
            self.halt("script halted by command");
        }
    }

    /// Halt runtime execution.
    pub fn halt(&mut self, reason: &str) {
        self.is_halted = true;
        self.events.push(RuntimeEvent::Halted(reason.to_string()));
    }

    /// Resume runtime execution.
    pub fn resume(&mut self, reason: &str) {
        self.is_halted = false;
        self.events.push(RuntimeEvent::Resumed(reason.to_string()));
    }

    /// Requeue a command to execute before next script step.
    pub fn requeue_step(&mut self, cmd: EngineCommand) {
        self.requeued_steps.push_front(cmd);
    }

    /// Build an immutable runtime snapshot.
    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            script: self.script_source.clone(),
            is_halted: self.is_halted,
            current_script_line: self.current_script_line,
            frame_stack: self.frames.iter().map(|f| f.kind).collect(),
            memory: self.memory.iter().cloned().collect(),
            mined_by_item: self.mined_by_item.clone(),
            stashed_by_item: self.stashed_by_item.clone(),
        }
    }

    /// Build a versioned checkpoint payload.
    pub fn checkpoint(&self) -> EngineCheckpoint {
        EngineCheckpoint {
            version: 1,
            script: self.script_source.clone(),
            is_halted: self.is_halted,
            current_script_line: self.current_script_line,
            frames: self.frames.clone(),
            requeued_steps: self.requeued_steps.iter().cloned().collect(),
            memory: self.memory.iter().cloned().collect(),
            mined_by_item: self.mined_by_item.clone(),
            stashed_by_item: self.stashed_by_item.clone(),
        }
    }

    /// Restore runtime from checkpoint payload.
    pub fn restore_checkpoint(&mut self, checkpoint: EngineCheckpoint) -> Result<(), EngineError> {
        if checkpoint.version != 1 {
            return Err(self.invalid_runtime_state(format!(
                "unsupported checkpoint version {}",
                checkpoint.version
            )));
        }

        self.set_script(&checkpoint.script, None)?;
        self.is_halted = checkpoint.is_halted;
        self.current_script_line = checkpoint.current_script_line;

        self.frames = checkpoint.frames;
        self.requeued_steps = checkpoint.requeued_steps.into_iter().collect();
        self.memory = checkpoint.memory.into_iter().collect();
        self.mined_by_item = checkpoint.mined_by_item;
        self.stashed_by_item = checkpoint.stashed_by_item;

        Ok(())
    }

    /// Drain emitted runtime events.
    pub fn drain_events(&mut self) -> Vec<RuntimeEvent> {
        std::mem::take(&mut self.events)
    }

    /// Render a runtime error diagnostic against the current script context.
    pub fn render_runtime_error(&self, message: impl Into<String>) -> String {
        let message = message.into();
        render_runtime_error(&self.script_source, self.current_script_line, &message)
    }

    fn install_default_predicates(&mut self) {
        let (boolean_specs, numeric_specs) = crate::catalog::default_predicate_catalog();
        self.bool_predicate_specs = boolean_specs;
        self.num_predicate_specs = numeric_specs;
        self.bool_predicates.clear();
        self.num_predicates.clear();

        self.num_predicates
            .insert("FUEL".into(), Arc::new(|state, _| Some(state.fuel_pct)));
        self.num_predicates
            .insert("CREDITS".into(), Arc::new(|state, _| Some(state.credits)));
        self.num_predicates.insert(
            "CARGO_PCT".into(),
            Arc::new(|state, _| Some(state.cargo_pct)),
        );
        self.num_predicates.insert(
            "CARGO".into(),
            Arc::new(|state, args| {
                let item = args.first()?;
                Some(*state.cargo.get(item).unwrap_or(&0))
            }),
        );
        self.num_predicates.insert(
            "MINED".into(),
            Arc::new(|state, args| {
                let item = args.first()?;
                Some(*state.script_mined_by_item.get(item).unwrap_or(&0))
            }),
        );
        self.num_predicates.insert(
            "STASHED".into(),
            Arc::new(|state, args| {
                let item = args.first()?;
                Some(*state.script_stashed_by_item.get(item).unwrap_or(&0))
            }),
        );
        self.num_predicates.insert(
            "STASH".into(),
            Arc::new(|state, args| {
                let poi_id = args.first()?;
                let item_id = args.get(1)?;
                Some(
                    state
                        .stash
                        .get(poi_id)
                        .and_then(|items| items.get(item_id))
                        .copied()
                        .unwrap_or(0),
                )
            }),
        );

        debug_assert!(self
            .bool_predicate_specs
            .keys()
            .all(|name| self.bool_predicates.contains_key(name)));
        debug_assert!(self
            .num_predicate_specs
            .keys()
            .all(|name| self.num_predicates.contains_key(name)));
    }

    fn eval_condition(
        &self,
        condition: &ConditionExpr,
        state: &GameState,
        bindings: Option<&IndexMap<String, String>>,
    ) -> Option<bool> {
        match condition {
            ConditionExpr::MetricCall(call) => {
                let name = call.name.to_uppercase();
                let spec = self.bool_predicate_specs.get(&name)?;
                if call.args.len() != spec.arity {
                    return None;
                }
                let pred = self.bool_predicates.get(&name)?;
                let args = self.resolve_args(&call.args, state, bindings);
                pred(state, &args)
            }
            ConditionExpr::Comparison { left, op, right } => {
                let l = self.resolve_operand(left, state, bindings)?;
                let r = self.resolve_operand(right, state, bindings)?;
                Some(match op {
                    ComparisonOp::Gt => l > r,
                    ComparisonOp::Ge => l >= r,
                    ComparisonOp::Lt => l < r,
                    ComparisonOp::Le => l <= r,
                    ComparisonOp::Eq => l == r,
                    ComparisonOp::Ne => l != r,
                })
            }
        }
    }

    fn resolve_operand(
        &self,
        operand: &NumericOperand,
        state: &GameState,
        bindings: Option<&IndexMap<String, String>>,
    ) -> Option<i64> {
        match operand {
            NumericOperand::Integer(v) => Some(*v),
            NumericOperand::ArgRef(arg) => {
                let v = self.resolve_arg(arg, state, bindings);
                v.parse::<i64>().ok()
            }
            NumericOperand::MetricCall(call) => {
                let name = call.name.to_uppercase();
                let spec = self.num_predicate_specs.get(&name)?;
                if call.args.len() != spec.arity {
                    return None;
                }
                let pred = self.num_predicates.get(&name)?;
                let args = self.resolve_args(&call.args, state, bindings);
                pred(state, &args)
            }
        }
    }

    fn resolve_args(
        &self,
        args: &[String],
        state: &GameState,
        bindings: Option<&IndexMap<String, String>>,
    ) -> Vec<String> {
        args.iter()
            .map(|a| self.resolve_arg(a, state, bindings))
            .collect()
    }

    fn resolve_arg(
        &self,
        arg: &str,
        state: &GameState,
        bindings: Option<&IndexMap<String, String>>,
    ) -> String {
        if !arg.starts_with('$') {
            return arg.to_string();
        }

        let name = arg.trim_start_matches('$');
        if let Some(b) = bindings.and_then(|b| b.get(name)) {
            return b.clone();
        }

        match name.to_ascii_lowercase().as_str() {
            "home" => state.home_base.clone().unwrap_or_else(|| arg.to_string()),
            "nearest_station" => state
                .nearest_station
                .clone()
                .unwrap_or_else(|| arg.to_string()),
            "here" => state.system.clone().unwrap_or_else(|| arg.to_string()),
            _ => arg.to_string(),
        }
    }

    fn find_analyzed_skill(&self, name: &str) -> Option<&AnalyzedSkillAstNode> {
        self.analyzed_skill_library
            .as_ref()?
            .skills
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    fn build_analyzed_skill_bindings(
        &self,
        skill: &AnalyzedSkillAstNode,
        args: &[String],
        parent_bindings: &IndexMap<String, String>,
    ) -> IndexMap<String, String> {
        let mut out = parent_bindings.clone();
        for (idx, param) in skill.params.iter().enumerate() {
            if let Some(value) = args.get(idx) {
                out.insert(param.name.clone(), value.clone());
            }
        }
        out
    }

    fn substitute_bindings(
        &self,
        args: &[String],
        bindings: &IndexMap<String, String>,
        state: &GameState,
    ) -> Vec<String> {
        args.iter()
            .map(|arg| self.resolve_arg(arg, state, Some(bindings)))
            .collect()
    }

    fn try_trigger_override(&mut self, state: &GameState) {
        if self.is_halted {
            return;
        }

        let Some(library) = self.analyzed_skill_library.as_ref() else {
            return;
        };

        for ov in &library.overrides {
            if self.frames.iter().any(|f| {
                f.kind == ExecutionFrameKind::Override
                    && f.override_name
                        .as_ref()
                        .is_some_and(|n| n.eq_ignore_ascii_case(&ov.name))
            }) {
                continue;
            }

            if self
                .eval_condition(&ov.condition, state, Some(&IndexMap::new()))
                .unwrap_or(false)
            {
                self.frames
                    .push(ExecutionFrame::override_frame_name(ov.name.clone()));
                self.events
                    .push(RuntimeEvent::OverrideTriggered(ov.name.clone()));
                break;
            }
        }
    }

    fn accumulate_deltas(&mut self, deltas: &HashMap<String, i64>, mined: bool) {
        let scoped = self.frames.iter_mut().rev().find(|f| {
            f.kind == ExecutionFrameKind::Skill || f.kind == ExecutionFrameKind::Override
        });

        let target = if mined {
            if let Some(f) = scoped {
                &mut f.frame_mined_by_item
            } else {
                &mut self.mined_by_item
            }
        } else if let Some(f) = scoped {
            &mut f.frame_stashed_by_item
        } else {
            &mut self.stashed_by_item
        };

        for (item, amount) in deltas {
            if *amount <= 0 {
                continue;
            }
            *target.entry(item.clone()).or_insert(0) += *amount;
        }
    }

    fn push_memory(&mut self, memory: ActionMemory) {
        if self.memory.len() >= MAX_MEMORY {
            let _ = self.memory.pop_front();
        }
        self.memory.push_back(memory);
    }

    fn materialize_analyzed_args(&self, args: &[AnalyzedArg], state: &GameState) -> Vec<String> {
        args.iter()
            .map(|arg| match arg {
                AnalyzedArg::Resolved(value) => value.clone(),
                AnalyzedArg::Dynamic(DynamicMacro::Home) => state
                    .home_base
                    .clone()
                    .unwrap_or_else(|| "$home".to_string()),
                AnalyzedArg::Dynamic(DynamicMacro::NearestStation) => state
                    .nearest_station
                    .clone()
                    .unwrap_or_else(|| "$nearest_station".to_string()),
            })
            .collect()
    }

    fn cast_command_args(&self, action: &str, args: Vec<String>) -> Vec<CommandArg> {
        let Some(spec) = self.command_catalog.get(action) else {
            return args.into_iter().map(CommandArg::Any).collect();
        };

        args.into_iter()
            .enumerate()
            .map(|(idx, value)| {
                let Some(arg_spec) = spec.args.get(idx) else {
                    return CommandArg::Any(value);
                };

                match arg_spec.kind {
                    ArgType::Any => CommandArg::Any(value),
                    ArgType::Integer => match value.parse::<i64>() {
                        Ok(v) => CommandArg::Integer(v),
                        Err(_) => CommandArg::Any(value),
                    },
                    ArgType::ItemId => CommandArg::ItemId(value),
                    ArgType::SystemId => CommandArg::SystemId(value),
                    ArgType::PoiId => CommandArg::PoiId(value),
                    ArgType::GoTarget => CommandArg::GoTarget(value),
                    ArgType::ShipId => CommandArg::ShipId(value),
                    ArgType::ListingId => CommandArg::ListingId(value),
                    ArgType::MissionId => CommandArg::MissionId(value),
                    ArgType::ModuleId => CommandArg::ModuleId(value),
                    ArgType::RecipeId => CommandArg::RecipeId(value),
                }
            })
            .collect()
    }

    fn analyzed_nodes_for_path(&self, path: &str) -> Option<Vec<AnalyzedNode>> {
        if path == ROOT_PATH {
            return Some(self.analyzed_script.as_ref()?.statements.clone());
        }

        if let Some(skill_name) = path.strip_prefix("skill/") {
            let skill = self
                .analyzed_skill_library
                .as_ref()?
                .skills
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(skill_name))?;
            return Some(skill.body.clone());
        }

        if let Some(ov_name) = path.strip_prefix("override/") {
            let ov = self
                .analyzed_skill_library
                .as_ref()?
                .overrides
                .iter()
                .find(|o| o.name.eq_ignore_ascii_case(ov_name))?;
            return Some(ov.body.clone());
        }

        let mut nodes = self.analyzed_script.as_ref()?.statements.clone();
        for raw in path.split('/') {
            if raw == ROOT_PATH || raw.is_empty() {
                continue;
            }
            let idx = raw.parse::<usize>().ok()?;
            let node = nodes.get(idx)?.clone();
            nodes = match node {
                AnalyzedNode::If(c) | AnalyzedNode::Until(c) => c.body,
                AnalyzedNode::Command(_) => return None,
            };
        }
        Some(nodes)
    }

    fn invalid_runtime_state(&self, message: impl Into<String>) -> EngineError {
        EngineError::InvalidState(self.render_runtime_error(message))
    }
}

fn render_diags(script: &str, diags: &[crate::dsl::Diagnostic]) -> String {
    diags
        .iter()
        .map(|d| d.render("script.dsl", script))
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
    let mut output = Vec::new();
    let report_span = ("script.dsl", span);
    let _ = Report::build(ReportKind::Error, report_span.clone())
        .with_config(Config::default().with_compact(true))
        .with_code("runtime.error")
        .with_message(message.to_string())
        .with_label(
            Label::new(report_span)
                .with_message(message.to_string())
                .with_color(Color::Red),
        )
        .finish()
        .write(("script.dsl", Source::from(script)), &mut output);

    let rendered = String::from_utf8_lossy(&output).trim().to_string();
    if rendered.is_empty() {
        message.to_string()
    } else {
        rendered
    }
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

fn active_state_for_command(command: &EngineCommand) -> Option<ActiveCommandState> {
    match command.action.as_str() {
        "mine" => Some(ActiveCommandState::Mine(MineState {
            resource: command.args.first().map(CommandArg::as_text),
            ..MineState::default()
        })),
        "go" => Some(ActiveCommandState::Go(GoState {
            target: command
                .args
                .first()
                .map(CommandArg::as_text)
                .unwrap_or_default(),
            ..GoState::default()
        })),
        "refuel" => Some(ActiveCommandState::Refuel(RefuelState::default())),
        "explore" => Some(ActiveCommandState::Explore(ExploreState::default())),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Runtime execution stack frame persisted in checkpoints.
pub struct ExecutionFrame {
    kind: ExecutionFrameKind,
    path: String,
    index: usize,
    until_condition: Option<ConditionExpr>,
    until_condition_known: bool,
    source_line: Option<usize>,
    override_name: Option<String>,
    bindings: IndexMap<String, String>,
    frame_mined_by_item: HashMap<String, i64>,
    frame_stashed_by_item: HashMap<String, i64>,
    active_command: Option<ActiveCommandState>,
}

impl ExecutionFrame {
    fn root() -> Self {
        Self {
            kind: ExecutionFrameKind::Root,
            path: ROOT_PATH.to_string(),
            index: 0,
            until_condition: None,
            until_condition_known: false,
            source_line: Some(1),
            override_name: None,
            bindings: IndexMap::new(),
            frame_mined_by_item: HashMap::new(),
            frame_stashed_by_item: HashMap::new(),
            active_command: None,
        }
    }

    fn skill(name: String, bindings: IndexMap<String, String>) -> Self {
        Self {
            kind: ExecutionFrameKind::Skill,
            path: format!("skill/{name}"),
            index: 0,
            until_condition: None,
            until_condition_known: false,
            source_line: None,
            override_name: None,
            bindings,
            frame_mined_by_item: HashMap::new(),
            frame_stashed_by_item: HashMap::new(),
            active_command: None,
        }
    }

    fn block(
        kind: ExecutionFrameKind,
        parent_path: String,
        node_index: usize,
        condition: ConditionExpr,
        bindings: IndexMap<String, String>,
    ) -> Self {
        Self {
            kind,
            path: format!("{parent_path}/{node_index}"),
            index: 0,
            until_condition: Some(condition),
            until_condition_known: false,
            source_line: None,
            override_name: None,
            bindings,
            frame_mined_by_item: HashMap::new(),
            frame_stashed_by_item: HashMap::new(),
            active_command: None,
        }
    }

    fn override_frame_name(name: String) -> Self {
        Self {
            kind: ExecutionFrameKind::Override,
            path: format!("override/{name}"),
            index: 0,
            until_condition: None,
            until_condition_known: false,
            source_line: None,
            override_name: Some(name),
            bindings: IndexMap::new(),
            frame_mined_by_item: HashMap::new(),
            frame_stashed_by_item: HashMap::new(),
            active_command: None,
        }
    }
}

/// Frame kind for runtime AST walker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionFrameKind {
    /// Root script frame.
    Root,
    /// Conditional if frame.
    If,
    /// Conditional until frame.
    Until,
    /// Skill frame.
    Skill,
    /// Override frame.
    Override,
}

/// Command-handler trait for single-turn operations.
pub trait SingleTurnCommandHandler: Send + Sync {
    /// Command name.
    fn name(&self) -> &str;
    /// Execute command.
    fn execute(
        &self,
        command: &EngineCommand,
        state: &GameState,
    ) -> Result<EngineExecutionResult, EngineError>;
}

/// Command-handler trait for multi-turn operations.
pub trait MultiTurnCommandHandler: Send + Sync {
    /// Command name.
    fn name(&self) -> &str;
    /// Start command.
    fn start(
        &self,
        command: &EngineCommand,
        state: &GameState,
    ) -> Result<(bool, EngineExecutionResult), EngineError>;
    /// Continue command.
    fn continue_run(&self, state: &GameState)
        -> Result<(bool, EngineExecutionResult), EngineError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{ArgSpec, ArgType, CommandSpec, PredicateSpec, ValidationContext};

    fn state() -> GameState {
        GameState {
            fuel_pct: 100,
            system: Some("sol".into()),
            home_base: Some("earth".into()),
            nearest_station: Some("earth_station".into()),
            ..Default::default()
        }
    }

    #[test]
    fn halts_on_script_completion() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("halt;", None).expect("set script");
        let cmd = engine.decide_next(&state()).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
        engine.execute_result(&cmd, EngineExecutionResult::default(), &state());
        assert!(engine.snapshot().is_halted);
    }

    #[test]
    fn until_rewinds_when_false() {
        let mut s = state();
        s.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("until FUEL() >= 50 { halt; }", None)
            .expect("script");

        let cmd = engine.decide_next(&s).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
    }

    #[test]
    fn checkpoint_roundtrip() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("go alpha;", None).expect("script");
        let cp = engine.checkpoint();

        let mut restored = RuntimeEngine::new();
        restored.restore_checkpoint(cp).expect("restore");
        assert_eq!(restored.snapshot().script.trim(), "go alpha;");
    }

    #[test]
    fn checkpoint_preserves_active_multi_turn_command_state() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("go alpha;", None).expect("script");
        let _ = engine
            .decide_next(&state())
            .expect("decide")
            .expect("command");
        let checkpoint = engine.checkpoint();

        let mut restored = RuntimeEngine::new();
        restored.restore_checkpoint(checkpoint).expect("restore");

        let active = restored
            .frames
            .last()
            .and_then(|f| f.active_command.clone());
        assert_eq!(
            active,
            Some(ActiveCommandState::Go(GoState {
                target: "alpha".to_string(),
                ..GoState::default()
            }))
        );
    }

    #[test]
    fn set_script_rejects_here_macro_without_state_system() {
        let mut engine = RuntimeEngine::new();
        let err = engine
            .set_script("go $here;", Some(&GameState::default()))
            .expect_err("expected analyzer error");
        assert!(err.to_string().contains("$here"));
    }

    #[test]
    fn set_script_rejects_unknown_command_with_default_context() {
        let mut engine = RuntimeEngine::new();
        let err = engine
            .set_script("warp alpha;", Some(&GameState::default()))
            .expect_err("expected validation error");
        assert!(err.to_string().contains("DSL200"));
    }

    #[test]
    fn set_script_accepts_here_macro_with_state_system() {
        let mut engine = RuntimeEngine::new();
        let state = GameState {
            system: Some("sol".to_string()),
            ..GameState::default()
        };
        let normalized = engine
            .set_script("go $here;", Some(&state))
            .expect("set script");
        assert_eq!(normalized.trim(), "go $here;");
    }

    #[test]
    fn nearest_station_macro_resolves_at_emit_time() {
        let mut engine = RuntimeEngine::new();
        let load_state = GameState {
            nearest_station: Some("earth_station".to_string()),
            ..GameState::default()
        };
        let _ = engine
            .set_script("go $nearest_station;", Some(&load_state))
            .expect("set script");

        let emit_state = GameState {
            nearest_station: Some("mars_station".to_string()),
            ..GameState::default()
        };
        let cmd = engine
            .decide_next(&emit_state)
            .expect("decide")
            .expect("command");
        assert_eq!(
            cmd.args,
            vec![CommandArg::GoTarget("mars_station".to_string())]
        );
    }

    #[test]
    fn analyzed_until_rewinds_when_false() {
        let mut s = state();
        s.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("until FUEL() >= 50 { halt; }", Some(&s))
            .expect("script");

        let cmd = engine.decide_next(&s).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
    }

    #[test]
    fn incomplete_result_requeues_same_command() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("wait 1;", None).expect("set script");
        let cmd = engine.decide_next(&state()).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "wait");
        engine.execute_result(
            &cmd,
            EngineExecutionResult {
                result_message: Some("still running".to_string()),
                completed: false,
                halt_script: false,
            },
            &state(),
        );

        let retry = engine
            .decide_next(&state())
            .expect("decide retry")
            .expect("retry cmd");
        assert_eq!(retry.action, "wait");
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
    fn set_script_accepts_optional_sell_and_stash_forms() {
        let mut engine = RuntimeEngine::new();
        let normalized = engine
            .set_script("sell;\nstash;\n", Some(&GameState::default()))
            .expect("set script");
        assert!(normalized.contains("sell;"));
        assert!(normalized.contains("stash;"));
    }

    #[test]
    fn if_block_executes_body_when_condition_true() {
        let mut s = state();
        s.fuel_pct = 10;
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("if FUEL() < 50 { halt; }", Some(&s))
            .expect("set script");
        let cmd = engine.decide_next(&s).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "halt");
    }

    #[test]
    fn if_block_skips_body_when_condition_false() {
        let mut s = state();
        s.fuel_pct = 90;
        let mut engine = RuntimeEngine::new();
        // Pass None for state to skip go-target identity validation
        let _ = engine
            .set_script("if FUEL() < 50 { halt; }\ngo alpha;", None)
            .expect("set script");
        let cmd = engine.decide_next(&s).expect("decide").expect("cmd");
        // condition is false so body is skipped, next statement executes
        assert_eq!(cmd.action, "go");
    }

    #[test]
    fn halt_and_resume_toggle_halted_state() {
        let mut engine = RuntimeEngine::new();
        let _ = engine.set_script("halt;", None).expect("set script");
        engine.halt("manual");
        assert!(engine.snapshot().is_halted);
        engine.resume("manual");
        assert!(!engine.snapshot().is_halted);
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
        let cmd = engine.decide_next(&state()).expect("decide").expect("cmd");
        engine.execute_result(&cmd, EngineExecutionResult::default(), &state());
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
        s.fuel_pct = 5;

        let cmd = engine.decide_next(&s).expect("decide").expect("cmd");
        // override fires before the script command
        assert_eq!(cmd.action, "halt");

        let events = engine.drain_events();
        assert!(events
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
        s.fuel_pct = 80;

        let cmd = engine.decide_next(&s).expect("decide").expect("cmd");
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
        let cmd = engine.decide_next(&state()).expect("decide").expect("cmd");
        assert_eq!(cmd.action, "go");
    }

    #[test]
    fn inject_session_counters_populates_script_mined() {
        let mut engine = RuntimeEngine::new();
        let _ = engine
            .set_script("mine iron_ore;", None)
            .expect("set script");
        let cmd = engine.decide_next(&state()).expect("decide").expect("cmd");
        let mut post_state = state();
        post_state.last_mined = std::sync::Arc::new(std::collections::HashMap::from([(
            "iron_ore".to_string(),
            5i64,
        )]));
        engine.execute_result(&cmd, EngineExecutionResult::default(), &post_state);

        let mut check_state = state();
        engine.inject_session_counters(&mut check_state);
        assert_eq!(
            check_state.script_mined_by_item.get("iron_ore").copied(),
            Some(5)
        );
    }
}
