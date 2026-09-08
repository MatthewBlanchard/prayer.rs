use prayer_actions::{Action, ActionEnvelope, ActionOrigin, RunId};
use prayer_runtime::{EngineExecutionResult, RuntimeEngine};
use prayer_runtime::read_context::ExecutionReadContext;

fn submit(engine: &mut RuntimeEngine, id: &str) -> RunId {
    let id = RunId(id.into());
    let claim = engine.try_acquire_action_run(id.clone()).unwrap();
    engine.submit_action_batch(&claim, vec![ActionEnvelope::new("same-action-id", Action::Wait { ticks: 1 },
        ActionOrigin::Manual { run_id: id.clone() })]).unwrap();
    id
}

#[test]
fn late_success_cannot_change_cancelled_outcome() {
    let mut engine = RuntimeEngine::new();
    let id = submit(&mut engine, "cancelled");
    let command = engine.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    let token = engine.execution_token().unwrap();
    let cancelled = engine.cancel_action_run(&id, "cancel".into()).unwrap();
    assert!(!engine.execute_result_for(&token, &command, None, EngineExecutionResult::default(), ExecutionReadContext::default()));
    assert_eq!(engine.action_run(&id).unwrap(), cancelled);
}

#[test]
fn late_success_cannot_complete_replacement_with_same_action_id() {
    let mut engine = RuntimeEngine::new();
    let id = submit(&mut engine, "old");
    let command = engine.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    let token = engine.execution_token().unwrap();
    engine.cancel_action_run(&id, "replace".into()).unwrap();
    let replacement = submit(&mut engine, "new");
    engine.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    assert!(!engine.execute_result_for(&token, &command, None, EngineExecutionResult::default(), ExecutionReadContext::default()));
    assert!(engine.action_run(&replacement).unwrap().outcome.is_none());
    let current = engine.execution_token().unwrap();
    assert!(engine.execute_result_for(&current, &command, None, EngineExecutionResult::default(), ExecutionReadContext::default()));
    assert!(engine.action_run(&replacement).unwrap().outcome.is_some());
}
