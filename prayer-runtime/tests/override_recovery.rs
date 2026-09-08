use prayer_actions::{Action, ActionEnvelope, ActionOrigin, RunId};
use prayer_runtime::{ActiveCommandState, EngineExecutionResult, RuntimeEngine, WaitState, GoState};
use prayer_runtime::read_context::ExecutionReadContext;

fn checkpoint_roundtrip(engine: &RuntimeEngine) -> RuntimeEngine {
    let json = serde_json::to_vec(&engine.execution_checkpoint().unwrap()).unwrap();
    let mut restored = RuntimeEngine::new();
    restored.restore_execution_checkpoint(serde_json::from_slice(&json).unwrap()).unwrap();
    restored
}

fn override_wait() -> ActionEnvelope {
    ActionEnvelope::new("override", Action::Wait { ticks: 2 },
        ActionOrigin::Interrupt { policy: "client".into() })
}

#[test]
fn pending_override_survives_restart_and_runs_before_normal_work() {
    let mut engine = RuntimeEngine::new();
    let id = RunId("normal".into());
    let claim = engine.try_acquire_action_run(id.clone()).unwrap();
    engine.submit_action_batch(&claim, vec![ActionEnvelope::new("normal", Action::Wait { ticks: 9 },
        ActionOrigin::Manual { run_id: id })]).unwrap();
    engine.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    engine.submit_action_override(vec![override_wait()]).unwrap();
    let mut restored = checkpoint_roundtrip(&engine);
    assert!(restored.scheduler_snapshot().running.unwrap().paused);
    let command = restored.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    assert_eq!(command.args_as_strings(), vec!["2"]);
    restored.execute_result(&command, EngineExecutionResult {
        result_message: None, completed: true, halt_script: false,
    }, ExecutionReadContext::default());
    let normal = restored.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    assert_eq!(normal.args_as_strings(), vec!["9"]);
}

#[test]
fn active_override_keeps_its_continuation_and_paused_script() {
    let mut engine = RuntimeEngine::new();
    engine.set_script("go alpha;", None).unwrap();
    engine.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    let normal = ActiveCommandState::Go(GoState { target: "alpha".into(), did_move: true, ..Default::default() });
    engine.set_active_command_state(Some(normal.clone()));
    engine.submit_action_override(vec![override_wait()]).unwrap();
    engine.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    let interrupt = ActiveCommandState::Wait(WaitState { total_ticks: 2, remaining_ticks: 1, origin: None });
    engine.set_active_command_state(Some(interrupt.clone()));
    let mut restored = checkpoint_roundtrip(&engine);
    assert_eq!(restored.active_command_state(), Some(interrupt));
    let command = restored.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    restored.execute_result(&command, EngineExecutionResult {
        result_message: None, completed: true, halt_script: false,
    }, ExecutionReadContext::default());
    assert_eq!(restored.active_command_state(), Some(normal));
}

#[test]
fn standalone_override_survives_without_a_normal_run() {
    let mut engine = RuntimeEngine::new();
    engine.submit_action_override(vec![override_wait()]).unwrap();
    let mut restored = checkpoint_roundtrip(&engine);
    assert!(restored.override_lane_busy());
    assert!(restored.decide_next(ExecutionReadContext::default()).unwrap().is_some());
}
