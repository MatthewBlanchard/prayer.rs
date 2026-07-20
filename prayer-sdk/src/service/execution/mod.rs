//! Step and script execution against the runtime planner facade.

use super::*;

#[derive(Clone)]
struct StepReadState {
    bot: Arc<BotState>,
    world: prayer_runtime::read_context::WorldReadState,
    runtime: prayer_runtime::read_context::ExecutionRuntimeState,
}

impl StepReadState {
    fn context(&self) -> prayer_runtime::read_context::ExecutionReadContext<'_> {
        prayer_runtime::read_context::ExecutionReadContext {
            bot: &self.bot,
            world: &self.world,
            runtime: &self.runtime,
        }
    }
}

impl RuntimeService {
    pub async fn execute_step(&self, id: Uuid) -> Result<StepResponse, SdkError> {
        self.execute_step_inner(id, None).await
    }

    pub async fn execute_step_inner(
        &self,
        id: Uuid,
        mut halt_rx: Option<&mut watch::Receiver<bool>>,
    ) -> Result<StepResponse, SdkError> {
        // Phase 1a: brief lock to check whether a state prefetch is needed.
        let should_prefetch = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            let is_halted = session.engine.snapshot().is_halted;
            !session.has_state || !is_halted || {
                let now = Instant::now();
                session
                    .last_halted_state_refresh
                    .map_or(true, |t| now.duration_since(t) >= Duration::from_secs(1))
            }
        };

        // Phase 1b: fetch current game state without holding the session lock.
        let prefetched_state = if should_prefetch {
            Some(
                match await_with_halt(&mut halt_rx, self.refresh_state_for_host_loop(id, false))
                    .await
                {
                    Ok(result) => result?,
                    Err(()) => return Ok(halted_step_response()),
                },
            )
        } else {
            None
        };

        // Phase 1c: apply prefetched state, then decide the next command.
        self.refresh_managed_players_knowledge().await;

        let (command, current_state, active_command, mining_blacklist) = {
            let session = self.get_session(id).await?;
            let mut session = session.lock().await;
            drop(prefetched_state);

            let knowledge = self.knowledge_state.snapshot();
            let current_state = StepReadState {
                bot: Arc::clone(&session.actor.observed),
                world: world_read_state_with_metadata(
                    &knowledge,
                    &self.knowledge_metadata.read(),
                    &session.actor.observed,
                ),
                runtime: session.engine.execution_runtime_state(),
            };
            let command = session.engine.decide_next(current_state.context())?;
            let Some(command) = command else {
                let halted = session.engine.snapshot().is_halted;
                let response = StepResponse {
                    executed: false,
                    command_action: None,
                    command_args: None,
                    result_message: None,
                    halted,
                    paused: false,
                    resume_after_ms: None,
                    error: None,
                };
                drop(session);
                self.persist_sessions("after script idle step").await;
                return Ok(response);
            };

            let command_text = if command.args.is_empty() {
                command.action.clone()
            } else {
                format!("{} {}", command.action, command.args_as_strings().join(" "))
            };
            let session_label = session.label.clone();
            info!(%id, bot = %session_label, command = %command_text, "{session_label} - {command_text}");
            debug!(
                command = %command_text,
                system = current_state.bot.location.system_id.as_deref().unwrap_or("(unknown)"),
                poi = current_state.bot.location.poi_id.as_deref().unwrap_or("(none)"),
                in_transit = current_state.bot.location.in_transit.unwrap_or(false),
                docked = current_state.bot.location.docked_at.is_some(),
                "executing step"
            );

            let active_command = session.engine.active_command_state();
            let mining_blacklist = std::collections::HashSet::new();
            (command, current_state, active_command, mining_blacklist)
        }; // lock released — snapshot reads can proceed during command I/O

        // Phase 2: plan and execute the command tick without holding the
        // session lock. The planner emits one operation at a time; API
        // responses are fed back until the tick yields a result.
        enum CommandOutcome {
            Success {
                result: EngineExecutionResult,
                state_after: Box<StepReadState>,
                message: Option<String>,
            },
            Paused {
                result: EngineExecutionResult,
                state_after: Box<StepReadState>,
                message: Option<String>,
            },
            SoftError(OperationFailure),
        }
        let mut planner = CommandPlanner::new(command.clone(), active_command, mining_blacklist);
        // Planning state may be refreshed mid-tick (RefreshState); keep
        // `current_state` pristine for the before/after delta diffs below.
        let mut plan_state = current_state.clone();
        let mut paused_step = false;
        let mut resume_after_ms: Option<u64> = None;
        let mut mission_refresh_forced = false;
        let mut craft_enqueue_message: Option<String> = None;
        let tick_result: Result<EngineExecutionResult, OperationFailure> = 'tick: {
            let mut last: Option<ApiOutcome> = None;
            loop {
                let read_context = prayer_runtime::read_context::RuntimeReadContext::from_execution(
                    plan_state.context(),
                    &command.action,
                );
                match planner.next_with_context(&read_context, last.take()) {
                    Ok(RuntimeOperation::SpaceMoltAction { action, payload }) => {
                        let Some(definition) = spacemolt_lib_rs::actions::find_action(&action)
                        else {
                            break 'tick Err(OperationFailure::InvalidIntent(format!(
                                "unknown generated action '{action}'"
                            )));
                        };
                        let tool = definition.tool;
                        let api_action = definition.action;
                        let trace_transfer = command.action == "transfer";
                        let payload_preview = json_preview(&payload);
                        let payload_for_invalidation = payload.clone();
                        if trace_transfer {
                            info!(
                                %id,
                                api_action,
                                current_system = plan_state.bot.location.system_id.as_deref().unwrap_or("(unknown)"),
                                current_poi = plan_state.bot.location.poi_id.as_deref().unwrap_or("(unknown)"),
                                cargo_used = plan_state.bot.cargo_used,
                                cargo_capacity = plan_state.bot.cargo_capacity,
                                cargo = ?plan_state.bot.cargo,
                                payload = %payload_preview,
                                "transfer: dispatch snapshot"
                            );
                        }
                        info!(
                            %id,
                            command = %command.action,
                            api_tool = tool,
                            api_action,
                            payload = %payload_preview,
                            "command lowered to upstream api call"
                        );
                        let started = Instant::now();
                        let api_result = match await_with_halt(&mut halt_rx, async {
                            let account = self.spacemolt_account(id).await?;
                            account
                                .send_action(&action, payload)
                                .await
                                .map(|result| {
                                    crate::spacemolt_projection::ExecutedSpacemoltCommand {
                                        tool: tool.to_string(),
                                        action: api_action.to_string(),
                                        result,
                                    }
                                })
                                .map_err(SdkError::from)
                        })
                        .await
                        {
                            Ok(result) => result,
                            Err(()) => return Ok(halted_step_response()),
                        };
                        if let Ok(executed) = &api_result {
                            match crate::spacemolt_projection::project_executed_command(executed) {
                                Ok(observations) => {
                                    self.ingest_observations(
                                        observations,
                                        "after SpaceMolt command",
                                    );
                                }
                                Err(error) => {
                                    warn!(tool, api_action, %error, "could not project typed SpaceMolt observation")
                                }
                            }
                        }
                        let api_result = api_result.map(
                            crate::spacemolt_projection::ExecutedSpacemoltCommand::runtime_value,
                        );
                        if trace_transfer {
                            match &api_result {
                                Ok(_) => info!(
                                    %id,
                                    command = %command.action,
                                    api_action,
                                    elapsed_ms = started.elapsed().as_millis(),
                                    "transfer: upstream api call success"
                                ),
                                Err(err) => info!(
                                    %id,
                                    command = %command.action,
                                    api_action,
                                    elapsed_ms = started.elapsed().as_millis(),
                                    planned_cargo = ?plan_state.bot.cargo,
                                    dispatched_payload = %payload_preview,
                                    error = %err,
                                    "transfer: upstream api call error"
                                ),
                            }
                        }
                        let switch_already_active = matches!(
                            &api_result,
                            Err(SdkError::Client(err))
                                if switch_ship_related_api_action(&command, tool, api_action)
                                    && switch_ship_already_active_error(err.as_inner())
                        );
                        let unload_all_no_passengers = matches!(
                            &api_result,
                            Err(SdkError::Client(err))
                                if unload_all_no_passengers_error(
                                    tool,
                                    api_action,
                                    payload_for_invalidation.as_ref(),
                                        err.as_inner()
                                )
                        );
                        if api_result.is_ok() {
                            if mission_related_api_action(tool, api_action) {
                                mission_refresh_forced = true;
                            }
                            if market_related_api_action(tool, api_action) {
                                if let Some(station_id) =
                                    current_state.bot.location.poi_id.as_deref()
                                {
                                    self.knowledge_state
                                        .write()
                                        .station_markets
                                        .remove(station_id);
                                }
                            }
                            if commission_related_api_action(tool, api_action) {}
                            if crafting_queue_related_api_action(tool, api_action) {
                                if let Ok(value) = &api_result {
                                    if let Some(message) = craft_enqueue_response_text(value) {
                                        craft_enqueue_message = Some(message);
                                    }
                                }
                            }
                            if passenger_related_api_action(tool, api_action) {
                                self.invalidate_station_passengers_for_poi(
                                    current_state.bot.location.poi_id.as_deref(),
                                );
                            }
                            if facility_mutating_api_action(tool, api_action) {
                                self.invalidate_facility_snapshot_for_poi(
                                    current_state.bot.location.poi_id.as_deref(),
                                );
                                self.invalidate_owned_facility_snapshots(
                                    current_state.bot.player.id.as_deref(),
                                    current_state.bot.player.faction_id.as_deref(),
                                );
                            }
                            if garage_related_passthrough_action(
                                tool,
                                api_action,
                                payload_for_invalidation.as_ref(),
                            ) {
                                clear_ship_and_garage_cache(
                                    Arc::make_mut(&mut plan_state.bot),
                                    &mut plan_state.world,
                                );
                            }
                        }
                        last = Some(match api_result {
                            Ok(value) => ApiOutcome::Success(value),
                            Err(SdkError::Client(err)) if switch_already_active => {
                                ApiOutcome::Success(serde_json::json!({
                                    "result": {
                                        "message": err.to_string(),
                                        "already_active": true,
                                    }
                                }))
                            }
                            Err(SdkError::Client(err)) if unload_all_no_passengers => {
                                ApiOutcome::Success(serde_json::json!({
                                    "result": {
                                        "message": err.to_string(),
                                        "already_empty": true,
                                    }
                                }))
                            }
                            Err(SdkError::Client(err)) => ApiOutcome::Failure(err.into_inner()),
                            Err(SdkError::Command(message)) => {
                                ApiOutcome::Failure(OperationFailure::InvalidIntent(message))
                            }
                            Err(err) => {
                                ApiOutcome::Failure(OperationFailure::Policy(err.to_string()))
                            }
                        });
                    }
                    Ok(RuntimeOperation::RefreshState) => {
                        last = Some(
                            match await_with_halt(
                                &mut halt_rx,
                                self.refresh_state_for_host_loop(id, true),
                            )
                            .await
                            {
                                Ok(Ok(_)) => {
                                    let session = self.get_session(id).await?;
                                    let session = session.lock().await;
                                    let knowledge = self.knowledge_state.snapshot();
                                    plan_state = StepReadState {
                                        bot: Arc::clone(&session.actor.observed),
                                        world: world_read_state_with_metadata(
                                            &knowledge,
                                            &self.knowledge_metadata.read(),
                                            &session.actor.observed,
                                        ),
                                        runtime: session.engine.execution_runtime_state(),
                                    };
                                    ApiOutcome::Success(serde_json::Value::Null)
                                }
                                Ok(Err(SdkError::Client(err))) => {
                                    ApiOutcome::Failure(err.into_inner())
                                }
                                Ok(Err(SdkError::Command(message))) => {
                                    ApiOutcome::Failure(OperationFailure::InvalidIntent(message))
                                }
                                Ok(Err(err)) => {
                                    ApiOutcome::Failure(OperationFailure::Policy(err.to_string()))
                                }
                                Err(()) => return Ok(halted_step_response()),
                            },
                        );
                    }
                    Ok(RuntimeOperation::WaitTick {
                        message,
                        resume_after,
                    }) => {
                        paused_step = true;
                        resume_after_ms = Some(duration_millis_u64(resume_after));
                        break 'tick Ok(EngineExecutionResult {
                            result_message: Some(message),
                            completed: false,
                            halt_script: false,
                        });
                    }
                    Ok(RuntimeOperation::CompleteAfterWait {
                        message,
                        resume_after,
                    }) => {
                        paused_step = true;
                        resume_after_ms = Some(duration_millis_u64(resume_after));
                        break 'tick Ok(EngineExecutionResult {
                            result_message: Some(message),
                            completed: true,
                            halt_script: false,
                        });
                    }
                    Ok(RuntimeOperation::Complete { result }) => break 'tick Ok(result),
                    Err(err) => break 'tick Err(err),
                }
            }
        };
        let outcome = match tick_result {
            Ok(result) => {
                let message = result.result_message.clone();
                if paused_step {
                    CommandOutcome::Paused {
                        result,
                        state_after: Box::new(current_state.clone()),
                        message,
                    }
                } else {
                    match await_with_halt(&mut halt_rx, self.refresh_state_for_host_loop(id, true))
                        .await
                    {
                        Ok(result) => {
                            result?;
                        }
                        Err(()) => return Ok(halted_step_response()),
                    }
                    let refreshed_session = self.get_session(id).await?;
                    let refreshed_session = refreshed_session.lock().await;
                    let knowledge = self.knowledge_state.snapshot();
                    let state_after = Box::new(StepReadState {
                        bot: Arc::clone(&refreshed_session.actor.observed),
                        world: world_read_state_with_metadata(
                            &knowledge,
                            &self.knowledge_metadata.read(),
                            &refreshed_session.actor.observed,
                        ),
                        runtime: refreshed_session.engine.execution_runtime_state(),
                    });
                    drop(refreshed_session);
                    CommandOutcome::Success {
                        result,
                        state_after,
                        message,
                    }
                }
            }
            Err(err) => CommandOutcome::SoftError(err),
        };

        // Phase 3: re-acquire lock, apply results.
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        if mission_refresh_forced {}
        let command_text = if command.args.is_empty() {
            command.action.clone()
        } else {
            format!("{} {}", command.action, command.args_as_strings().join(" "))
        };
        let restoration_step = session.engine.scheduler_snapshot().interrupt.as_ref().is_some_and(|running| {
            matches!(&running.envelope.origin, prayer_actions::ActionOrigin::Interrupt { policy } if policy == "client_return_to_origin_ready")
        });
        let (result, mut state_after, refreshed, message, step_error) = match outcome {
            CommandOutcome::Success {
                result,
                state_after,
                message,
            } => (result, state_after, true, message, None),
            CommandOutcome::Paused {
                result,
                state_after,
                message,
            } => (result, state_after, false, message, None),
            CommandOutcome::SoftError(err) => {
                let err_string = err.to_string();
                let message = session
                    .engine
                    .render_runtime_error(if restoration_step {
                        format!("return-to-origin failed; resuming normal execution from the current location: {err_string}")
                    } else {
                        format!("error: {err_string}")
                    });
                debug!(error = %err_string, "step operation error");
                (
                    EngineExecutionResult {
                        result_message: Some(message.clone()),
                        // Restoration is best effort: report its failure, finish
                        // the override lane, and resume normal work in place.
                        completed: restoration_step,
                        halt_script: !restoration_step,
                    },
                    Box::new(current_state.clone()),
                    false,
                    Some(message),
                    (!restoration_step).then_some(err_string),
                )
            }
        };
        if let Some(message) = craft_enqueue_message.as_deref() {
            preserve_craft_enqueue_as_queue(Arc::make_mut(&mut state_after.bot), message);
        }

        let mine_deltas = if command.action.eq_ignore_ascii_case("mine") {
            diff_positive_item_deltas(
                current_state.bot.cargo.as_ref(),
                state_after.bot.cargo.as_ref(),
            )
        } else {
            HashMap::new()
        };
        let storage_deltas = if command_stores_to_personal_storage(&command) {
            let before_storage = storage_totals_by_item(current_state.world.storage.as_ref());
            let after_storage = storage_totals_by_item(state_after.world.storage.as_ref());
            diff_positive_item_deltas(&before_storage, &after_storage)
        } else {
            HashMap::new()
        };
        Arc::make_mut(&mut state_after.bot).last_mined = Arc::new(mine_deltas);
        Arc::make_mut(&mut state_after.bot).last_stored = Arc::new(storage_deltas);

        {
            let knowledge = self.knowledge_state.snapshot();
            session.actor.observed = Arc::clone(&state_after.bot);
            session.actor.observation.observed_at_utc = Some(Utc::now());
            session.knowledge_version = knowledge.knowledge_version;
            session.has_state = true;
            if refreshed {
                session.last_state_refresh_completed_at = Some(Instant::now());
            }
        }
        session
            .engine
            .set_active_command_state(planner.continuation());
        session
            .engine
            .execute_result(&command, result.clone(), state_after.context());
        session.push_status(format!(
            "{command_text} - {}",
            message.clone().unwrap_or_else(|| "done".to_string())
        ));
        let halted = session.engine.snapshot().is_halted;
        debug!(
            message = message.as_deref().unwrap_or("(none)"),
            completed = result.completed,
            halted,
            system = state_after
                .bot
                .location
                .system_id
                .as_deref()
                .unwrap_or("(unknown)"),
            poi = state_after
                .bot
                .location
                .poi_id
                .as_deref()
                .unwrap_or("(none)"),
            in_transit = state_after.bot.location.in_transit.unwrap_or(false),
            "step result"
        );
        session.touch_state();
        self.note_session_changed(id);
        let command_action = command.action.clone();
        let command_args = command.args_as_strings();
        let response = StepResponse {
            executed: true,
            command_action: Some(command_action),
            command_args: Some(command_args),
            result_message: message,
            halted,
            paused: paused_step,
            resume_after_ms,
            error: step_error,
        };
        drop(session);
        self.persist_sessions("after script step").await;
        Ok(response)
    }

    /// Run script loop until completion, halt, or error, sleeping through wait ticks.
    pub async fn execute_script(&self, id: Uuid) -> Result<ExecuteScriptResponse, SdkError> {
        self.execute_registered_script_run(id, "api execute", true)
            .await
    }

    /// Run script loop until completion, halt, error, or a paused tick.
    pub async fn execute_script_until_pause(
        &self,
        id: Uuid,
    ) -> Result<ExecuteScriptResponse, SdkError> {
        self.execute_registered_script_run(id, "api execute until pause", false)
            .await
    }

    pub async fn execute_script_with_wait_policy(
        &self,
        id: Uuid,
        poll_across_waits: bool,
    ) -> Result<ExecuteScriptResponse, SdkError> {
        let mut steps_executed = 0usize;
        let mut error: Option<String> = None;
        let mut halt_message: Option<String> = None;
        let mut halt_rx = self.script_halt_receiver(id).await;
        debug!("script run started");

        let state_before = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            if session.has_state {
                let knowledge = self.knowledge_state.read();
                Some(ScriptDiffSnapshot::from_scopes(
                    &session.actor.observed,
                    &knowledge,
                ))
            } else {
                None
            }
        };

        loop {
            let step = match self.execute_step_inner(id, halt_rx.as_mut()).await {
                Ok(step) => step,
                Err(SdkError::Engine(err)) => {
                    let msg = err.to_string();
                    let session = self.get_session(id).await?;
                    session.lock().await.engine.fail_action_run(msg.clone())?;
                    self.persist_sessions("after action batch failure").await;
                    debug!(step = steps_executed, error = %msg, "script loop engine error");
                    error = Some(msg);
                    break;
                }
                Err(err) => return Err(err),
            };
            if !step.executed {
                debug!(
                    step = steps_executed,
                    halted = step.halted,
                    "script loop finished: no command to execute"
                );
                break;
            }
            steps_executed += 1;
            if step.error.is_some() || step.halted {
                error = step.error.clone();
                halt_message = step.result_message.clone();
                if let Some(message) = step.error.clone() {
                    let session = self.get_session(id).await?;
                    session.lock().await.engine.fail_action_run(message)?;
                    self.persist_sessions("after action batch failure").await;
                }
                debug!(
                    step = steps_executed,
                    error = step.error.as_deref().unwrap_or("(none)"),
                    message = step.result_message.as_deref().unwrap_or("(no message)"),
                    "script loop stopped"
                );
                break;
            }
            if step.paused {
                if !poll_across_waits {
                    debug!(
                        step = steps_executed,
                        message = step.result_message.as_deref().unwrap_or("(no message)"),
                        "script loop paused"
                    );
                    break;
                }
                let delay = self.script_wait_delay(step.resume_after_ms);
                debug!(
                    step = steps_executed,
                    delay_ms = delay.as_millis(),
                    message = step.result_message.as_deref().unwrap_or("(no message)"),
                    "script loop waiting"
                );
                if let Some(rx) = halt_rx.as_mut() {
                    if *rx.borrow() {
                        halt_message = Some("halt requested".to_string());
                        break;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        changed = rx.changed() => {
                            if changed.is_ok() && *rx.borrow() {
                                halt_message = Some("halt requested".to_string());
                                break;
                            }
                        }
                    }
                } else {
                    tokio::time::sleep(delay).await;
                }
                continue;
            }
        }
        debug!(
            steps_executed,
            had_error = error.is_some(),
            halted = halt_message.is_some(),
            "script run finished"
        );

        let snapshot = self.snapshot(id).await?;
        let action_run_completed = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            session.engine.action_run_outcome().is_some_and(|outcome| {
                matches!(
                    outcome,
                    prayer_runtime::execution::ActionBatchOutcome::Succeeded
                )
            })
        };
        let diff = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            if session.has_state {
                state_before.as_ref().map(|before| {
                    let knowledge = self.knowledge_state.read();
                    let after =
                        ScriptDiffSnapshot::from_scopes(&session.actor.observed, &knowledge);
                    compute_script_diff(before, &after, snapshot.is_halted)
                })
            } else {
                None
            }
        };
        Ok(ExecuteScriptResponse {
            steps_executed,
            halted: snapshot.is_halted,
            completed: snapshot.is_finished || action_run_completed,
            error,
            halt_message,
            diff,
        })
    }
}
