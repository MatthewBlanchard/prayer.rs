use prayer_actions::{Action, ActionEnvelope, ActionOrigin, RunId};
use prayer_runtime::{ActionBatchOutcome, EngineExecutionResult, RuntimeEngine};
use prayer_runtime::read_context::ExecutionReadContext;

fn prepared() -> (RuntimeEngine, RunId) {
    let mut engine = RuntimeEngine::new();
    let id = RunId("durable".into());
    let claim = engine.try_acquire_action_run(id.clone()).unwrap();
    engine.submit_action_batch(&claim, vec![ActionEnvelope::new("purchase", Action::Wait { ticks: 1 },
        ActionOrigin::Manual { run_id: id.clone() })]).unwrap();
    engine.decide_next(ExecutionReadContext::default()).unwrap().unwrap();
    let token = engine.execution_token().unwrap();
    engine.prepare_dispatch(&token, "spacemolt_market/buy").unwrap();
    (engine, id)
}

#[test]
fn restart_does_not_replay_a_prepared_mutation_with_unknown_outcome() {
    let (engine, id) = prepared();
    let bytes = serde_json::to_vec(&engine.execution_checkpoint().unwrap()).unwrap();
    let mut restored = RuntimeEngine::new();
    restored.restore_execution_checkpoint(serde_json::from_slice(&bytes).unwrap()).unwrap();
    assert!(restored.decide_next(ExecutionReadContext::default()).unwrap().is_none());
    assert!(matches!(restored.action_run(&id).unwrap().outcome,
        Some(ActionBatchOutcome::Halted { reason }) if reason.contains("outcome unknown")));
    assert!(!restored.has_unfinished_action_run());
}

#[test]
fn committed_success_clears_dispatch_marker_in_the_same_checkpoint() {
    let (mut engine, id) = prepared();
    let token = engine.execution_token().unwrap();
    let command = prayer_runtime::resolve_action(Action::Wait { ticks: 1 }).unwrap();
    assert!(engine.execute_result_for(&token, &command, None, EngineExecutionResult::default(), ExecutionReadContext::default()));
    let checkpoint = engine.execution_checkpoint().unwrap();
    assert!(checkpoint.pending_dispatch.is_none());
    let mut restored = RuntimeEngine::new();
    restored.restore_execution_checkpoint(checkpoint).unwrap();
    assert_eq!(restored.action_run(&id).unwrap().outcome, Some(ActionBatchOutcome::Succeeded));
}

#[test]
fn pre_intent_checkpoints_remain_readable_but_new_checkpoints_require_v3() {
    let mut engine = RuntimeEngine::new();
    engine.set_script("go alpha;", None).unwrap();
    let mut legacy = serde_json::to_value(engine.execution_checkpoint().unwrap()).unwrap();
    legacy["schema_version"] = serde_json::json!(2);
    legacy.as_object_mut().unwrap().remove("pending_dispatch");
    let mut restored = RuntimeEngine::new();
    restored.restore_execution_checkpoint(serde_json::from_value(legacy).unwrap()).unwrap();
    assert!(restored.decide_next(ExecutionReadContext::default()).unwrap().is_some());
    assert_eq!(restored.execution_checkpoint().unwrap().schema_version, 3);
}
