use super::knowledge::*;
use super::persistence::*;
use super::*;

pub struct SessionHandle {
    pub bot_id: Option<BotId>,
    pub label: String,
    pub created_utc: DateTime<Utc>,
    pub last_updated_utc: DateTime<Utc>,
    pub engine: RuntimeEngine,
    pub script_execution: Option<ScriptExecutionDto>,
    pub active_movement_id: Option<Uuid>,
    pub actor: LiveActorSnapshot,
    pub has_state: bool,
    pub last_halted_state_refresh: Option<Instant>,
    pub status_lines: Vec<String>,
    pub current_control_input: Option<String>,
    pub restored_checkpoint_needs_reanalysis: bool,
    pub spacemolt_account_selector: Option<String>,
    pub spacemolt_base_url: Option<String>,
    pub spacemolt_account: Option<Account>,
    pub state_version: u64,
    pub knowledge_version: u64,
    pub(crate) tax_estimate_cache: Option<CachedTaxEstimate>,
    pub state_refresh_lock: Arc<Mutex<()>>,
    pub last_state_refresh_completed_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct ScriptRunInfo {
    origin: &'static str,
    started_utc: DateTime<Utc>,
    halt_tx: watch::Sender<bool>,
    action_generation: Option<u64>,
}

fn fleet_execution_projection(
    mut script_execution: Option<ScriptExecutionDto>,
    execution: &prayer_runtime::ExecutionSnapshot,
    current_script_line: Option<usize>,
) -> Option<serde_json::Value> {
    let override_active = execution.scheduler.interrupt.is_some()
        || !execution.scheduler.interrupt_pending.is_empty();
    if let Some(claim) = execution.scheduler.claim.as_ref() {
        let run_id = match &claim.owner {
            prayer_scheduler::QueueOwner::Controller { run_id, .. }
            | prayer_scheduler::QueueOwner::Manual { run_id } => Some(run_id.0.clone()),
            prayer_scheduler::QueueOwner::PrayerLang { .. } => None,
        };
        if let Some(run_id) = run_id {
            let displayed_prayer = if override_active {
                &execution.override_queue_prayer
            } else {
                &execution.normal_queue_prayer
            };
            if displayed_prayer.is_empty() {
                return None;
            }
            let current_line = (execution.scheduler.running.is_some()
                || execution.scheduler.interrupt.is_some())
            .then_some(1usize);
            return Some(serde_json::json!({
                "id": run_id,
                "runId": run_id,
                "script": displayed_prayer,
                "state": "running",
                "currentLine": current_line,
                "lastLine": null,
                "outcome": null,
                "frameKind": if override_active { "override" } else { "main" },
                "frameName": null,
            }));
        }
    }

    if let Some(script_execution) = script_execution.as_mut() {
        if override_active {
            script_execution.script = Some(execution.override_queue_prayer.clone());
            script_execution.frame_kind = Some("override".into());
            script_execution.frame_name = None;
        } else {
            script_execution.frame_kind = Some("main".into());
            script_execution.frame_name = None;
        }
        if let ScriptExecutionStateDto::Running { current_line, .. } = &mut script_execution.state {
            *current_line = if override_active {
                Some(1)
            } else {
                current_script_line
            };
        }
    }
    script_execution.and_then(|value| serde_json::to_value(value).ok())
}

pub struct ScriptRunGuard {
    active_script_runs: Arc<ParkingMutex<HashMap<Uuid, ScriptRunInfo>>>,
    id: Uuid,
    origin: &'static str,
    started_utc: DateTime<Utc>,
    session: Arc<Mutex<SessionHandle>>,
    execution_id: Uuid,
    action_generation: Option<u64>,
    released: bool,
}

impl Drop for ScriptRunGuard {
    fn drop(&mut self) {
        if !self.released {
            let mut active_runs = self.active_script_runs.lock();
            let should_remove = active_runs.get(&self.id).is_some_and(|run| {
                run.origin == self.origin && run.started_utc == self.started_utc
            });
            if should_remove {
                active_runs.remove(&self.id);
            }
        }
        let session = Arc::clone(&self.session);
        let execution_id = self.execution_id;
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(async move {
            let mut session = session.lock().await;
            let last_line = session.engine.snapshot().current_script_line;
            if let Some(execution) = session.script_execution.as_mut() {
                if execution.id == execution_id
                    && matches!(execution.state, ScriptExecutionStateDto::Running { .. })
                {
                    execution.state = ScriptExecutionStateDto::Stopped {
                        current_line: None,
                        last_line,
                        outcome: ScriptOutcomeDto::Error {
                            kind: ScriptErrorKindDto::RunnerExited,
                            message: "Script runner exited before completing".into(),
                        },
                    };
                }
            }
        });
    }
}

#[derive(Debug, Clone, Default)]
pub struct LiveActorSnapshot {
    pub observed: Arc<BotState>,
    pub observation: ActorObservationMeta,
}

#[derive(Debug, Clone, Default)]
pub struct ActorObservationMeta {
    pub observed_at_utc: Option<DateTime<Utc>>,
}
impl SessionHandle {
    pub fn bot_state_mut(&mut self) -> &mut BotState {
        Arc::make_mut(&mut self.actor.observed)
    }

    pub fn new(label: String) -> Self {
        let now = Utc::now();
        Self {
            bot_id: None,
            label,
            created_utc: now,
            last_updated_utc: now,
            engine: RuntimeEngine::default(),
            script_execution: None,
            active_movement_id: None,
            actor: LiveActorSnapshot::default(),
            has_state: false,
            last_halted_state_refresh: None,
            status_lines: vec!["Awaiting script input".to_string()],
            current_control_input: None,
            restored_checkpoint_needs_reanalysis: false,
            spacemolt_account_selector: None,
            spacemolt_base_url: None,
            spacemolt_account: None,
            state_version: 1,
            knowledge_version: 0,
            tax_estimate_cache: None,
            state_refresh_lock: Arc::new(Mutex::new(())),
            last_state_refresh_completed_at: None,
        }
    }

    pub fn push_status(&mut self, line: impl Into<String>) {
        self.status_lines.push(line.into());
        if self.status_lines.len() > MAX_STATUS_LINES {
            let drop_count = self.status_lines.len() - MAX_STATUS_LINES;
            self.status_lines.drain(0..drop_count);
        }
    }

    pub fn execution_status_lines(&self) -> Vec<String> {
        self.status_lines.clone()
    }

    pub fn touch_state(&mut self) {
        self.state_version = self.state_version.saturating_add(1);
        self.last_updated_utc = Utc::now();
    }
}

impl RuntimeService {
    pub async fn register_spacemolt_account(
        &self,
        params: spacemolt_lib_rs::RegisterParams,
    ) -> Result<(Uuid, spacemolt_lib_rs::RegisterResult), SdkError> {
        let (account, result) = self.spacemolt_client.register(params).await?;
        let (_, installed) = self
            .attach_connected_owned_spacemolt_accounts(
                vec![account],
                self.spacemolt_base_url.clone(),
            )
            .await?;
        let id = installed.into_iter().next().ok_or_else(|| {
            SdkError::BadRequest("registered account could not be attached".to_string())
        })?;
        if let Err(error) = self.refresh_state(id).await {
            warn!(%id, %error, "registered account initial refresh failed");
        }
        Ok((id, result))
    }

    fn fleet_username(session: &SessionHandle) -> Option<String> {
        session
            .spacemolt_account
            .as_ref()
            .and_then(|account| {
                account
                    .state()
                    .player()
                    .ok()
                    .flatten()
                    .and_then(|player| player.username)
            })
            .or_else(|| session.actor.observed.player.username.clone())
            .or_else(|| {
                let label = session.label.trim();
                (!label.is_empty()).then(|| label.to_string())
            })
    }

    pub async fn bot_snapshot(&self, id: Uuid) -> Result<prayer_state::FleetEntry, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        let knowledge = self.knowledge_state.read().clone();
        let bot_id = session.bot_id.clone().ok_or_else(|| {
            SdkError::InvalidRuntimeState(format!("session {id} has no stable bot identity"))
        })?;
        let runtime_snapshot = session.engine.snapshot();
        let navigation = ActorNavigationRead::new(
            session.actor.observed.as_ref(),
            Arc::clone(&knowledge.galaxy),
        );
        let active_route = active_go_route(&runtime_snapshot, &navigation);
        let execution = session.engine.execution_snapshot();
        let script_execution = fleet_execution_projection(
            session.script_execution.clone(),
            &execution,
            runtime_snapshot.current_script_line,
        );
        Ok(prayer_state::FleetEntry {
            id: bot_id,
            username: Self::fleet_username(&session),
            state: Arc::clone(&session.actor.observed),
            version: session.state_version,
            observed_at: session.actor.observation.observed_at_utc,
            connection: if session.spacemolt_account.is_some() {
                prayer_state::BotConnectionState::Connected
            } else {
                prayer_state::BotConnectionState::Disconnected
            },
            script_execution,
            active_route: active_route.and_then(|value| serde_json::to_value(value).ok()),
            in_transit: session.actor.observed.location.in_transit.unwrap_or(false),
            transit_dest_system: session
                .actor
                .observed
                .location
                .transit_dest_system_id
                .clone(),
            transit_dest_poi: session.actor.observed.location.transit_dest_poi_id.clone(),
        })
    }

    pub async fn state_snapshot(
        &self,
    ) -> prayer_state::StateSnapshot<RuntimeVirtualMarketOrderDto, RuntimeVirtualCraftOrderDto>
    {
        let sessions = self.sessions.read().values().cloned().collect::<Vec<_>>();
        let mut world = self.knowledge_state.snapshot();
        {
            let reservations = self.inventory_reservations.lock();
            if reservations.has_active_market_reservations() {
                let mut projected_market = prayer_runtime::MarketData {
                    station_markets: world.station_markets.clone(),
                    ..prayer_runtime::MarketData::default()
                };
                reservations.apply_market_reservations(&mut projected_market);
                Arc::make_mut(&mut world).station_markets = projected_market.station_markets;
            }
        }
        let mut bots = HashMap::new();
        let mut version = 0u64;
        for session in sessions {
            let session = session.lock().await;
            let Some(bot_id) = session.bot_id.clone().filter(|_| session.has_state) else {
                continue;
            };
            version = version.max(session.state_version);
            let runtime_snapshot = session.engine.snapshot();
            let navigation = ActorNavigationRead::new(
                session.actor.observed.as_ref(),
                Arc::clone(&world.galaxy),
            );
            let active_route = active_go_route(&runtime_snapshot, &navigation);
            let execution = session.engine.execution_snapshot();
            let script_execution = fleet_execution_projection(
                session.script_execution.clone(),
                &execution,
                runtime_snapshot.current_script_line,
            );
            bots.insert(
                bot_id.clone(),
                prayer_state::FleetEntry {
                    id: bot_id,
                    username: Self::fleet_username(&session),
                    state: Arc::clone(&session.actor.observed),
                    version: session.state_version,
                    observed_at: session.actor.observation.observed_at_utc,
                    connection: if session.spacemolt_account.is_some() {
                        prayer_state::BotConnectionState::Connected
                    } else {
                        prayer_state::BotConnectionState::Disconnected
                    },
                    script_execution,
                    active_route: active_route.and_then(|value| serde_json::to_value(value).ok()),
                    in_transit: session.actor.observed.location.in_transit.unwrap_or(false),
                    transit_dest_system: session
                        .actor
                        .observed
                        .location
                        .transit_dest_system_id
                        .clone(),
                    transit_dest_poi: session.actor.observed.location.transit_dest_poi_id.clone(),
                },
            );
        }
        version = version.max(world.knowledge_version);
        prayer_state::StateSnapshot {
            fleet: prayer_state::FleetSnapshot { bots },
            world: prayer_state::WorldSnapshot {
                version: world.knowledge_version,
                state: world,
            },
            version,
        }
    }

    pub async fn session_for_bot_selector(&self, selector: &str) -> Result<Uuid, SdkError> {
        let selector = selector.trim();
        let sessions = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, Arc::clone(session)))
            .collect::<Vec<_>>();
        let mut named_matches = Vec::new();
        for (id, session) in sessions {
            let session = session.lock().await;
            if session
                .bot_id
                .as_ref()
                .is_some_and(|bot_id| bot_id.as_str() == selector)
            {
                return Ok(id);
            }
            if session.label.eq_ignore_ascii_case(selector)
                || session
                    .actor
                    .observed
                    .player
                    .username
                    .as_deref()
                    .is_some_and(|username| username.eq_ignore_ascii_case(selector))
            {
                named_matches.push(id);
            }
        }
        match named_matches.as_slice() {
            [id] => Ok(*id),
            [] => Err(SdkError::SessionNotFound),
            _ => Err(SdkError::AmbiguousBot {
                selector: selector.to_string(),
            }),
        }
    }

    pub async fn restore_persisted_session(
        &self,
        record: PersistedRuntimeSession,
    ) -> Result<Option<Uuid>, SdkError> {
        let mut session = SessionHandle::new(record.label);
        session.created_utc = record.created_utc;
        session.last_updated_utc = record.last_updated_utc;
        session.current_control_input = record.current_control_input;
        session.script_execution = record.script_execution;
        if !record.status_lines.is_empty() {
            session.status_lines = record.status_lines;
        }
        session.spacemolt_account_selector = record.spacemolt_account_selector.clone();
        session.bot_id = record
            .bot_id
            .map(BotId::from)
            .or_else(|| record.spacemolt_account_selector.clone().map(BotId::from));
        session.spacemolt_base_url = record.spacemolt_base_url.clone();
        session.restored_checkpoint_needs_reanalysis = record
            .execution
            .as_ref()
            .map(PersistedExecutionRun::needs_prayerlang_reanalysis)
            .unwrap_or(false);
        if session.spacemolt_account_selector.is_some() && !self.options.local_auth_bypass {
            session.push_status(
                "Restored SpaceMolt account selector; reconnect with Clerk key before live commands"
                    .to_string(),
            );
        }

        let execution = record.execution.ok_or_else(|| {
            SdkError::InvalidRuntimeState(
                "discarding persisted session without an atomic execution checkpoint".into(),
            )
        })?;
        session.engine.restore_execution_checkpoint(execution)?;

        if !session.engine.snapshot().script.trim().is_empty() {
            session.engine.reanalyze_current_script(None)?;
        }
        session.push_status("Restored from runtime session store");
        let snapshot = session.engine.snapshot();
        // Typed-action queues own their runner lifecycle independently of the
        // legacy PrayerLang producer flags. If scheduler work survived a
        // restart, it must always cause a runner to be restored.
        let should_kick = session.engine.has_unfinished_action_run()
            || (!snapshot.is_halted && !snapshot.is_finished);
        info!(
            id = %record.id,
            label = %session.label,
            is_halted = snapshot.is_halted,
            is_finished = snapshot.is_finished,
            should_kick,
            "startup session hydration: checkpoint restored"
        );

        if !self.reserve_session_label(&session.label, record.id)? {
            warn!(
                %record.id,
                label = %session.label,
                "skipping restored runtime session with duplicate label"
            );
            return Ok(None);
        }

        let session = Arc::new(Mutex::new(session));
        self.sessions.write().insert(record.id, session);
        self.note_roster_changed(record.id);
        info!(
            id = %record.id,
            "startup session hydration: session inserted"
        );

        Ok(should_kick.then_some(record.id))
    }

    /// Create a disconnected runtime session for service-level tests.
    pub fn create_session(&self) -> Uuid {
        self.create_session_with_label(None)
            .expect("generated session labels should be unique")
    }

    pub fn create_session_with_label(&self, label: Option<String>) -> Result<Uuid, SdkError> {
        let id = Uuid::new_v4();
        let label = label
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| format!("session-{}", &id.to_string()[..8]));
        self.reserve_session_label(&label, id)?;
        self.sessions
            .write()
            .insert(id, Arc::new(Mutex::new(SessionHandle::new(label))));
        self.note_roster_changed(id);
        info!(%id, "created runtime session");
        Ok(id)
    }

    pub fn reserve_session_label(&self, label: &str, id: Uuid) -> Result<bool, SdkError> {
        let label = label.trim();
        if label.is_empty() {
            return Ok(true);
        }
        let mut labels = self.session_labels.write();
        if let Some(existing_id) = labels.get(label) {
            if *existing_id == id {
                return Ok(false);
            }
            return Err(SdkError::BadRequest(format!(
                "session label already exists: {label}"
            )));
        }
        labels.insert(label.to_string(), id);
        Ok(true)
    }

    #[cfg(test)]
    pub async fn install_connected_owned_spacemolt_accounts(
        &self,
        accounts: Vec<Account>,
        base_url: String,
    ) -> Result<usize, SdkError> {
        let (created, installed) = self
            .attach_connected_owned_spacemolt_accounts(accounts, base_url)
            .await?;

        for id in &installed {
            if let Err(err) = self.refresh_state(*id).await {
                warn!(
                    %id,
                    error = %err,
                    "startup owned account connect: state refresh failed"
                );
            }
        }
        Ok(created)
    }

    /// Attach connected accounts without performing any network refresh work.
    ///
    /// Startup uses this path so roster visibility and subsequent account
    /// discovery are not serialized behind the comparatively expensive state
    /// and knowledge refresh for each account.
    pub async fn attach_connected_owned_spacemolt_accounts(
        &self,
        accounts: Vec<Account>,
        base_url: String,
    ) -> Result<(usize, Vec<Uuid>), SdkError> {
        let mut created = Vec::new();
        let mut installed = Vec::new();
        for account in accounts {
            let Some((selector, label, candidates)) = Self::connected_account_identity(&account)
            else {
                warn!("startup owned account connect: skipping account without id");
                continue;
            };
            let existing_id = self.session_id_for_spacemolt_identity(&candidates);
            if let Some(id) = existing_id {
                info!(
                    selector,
                    %id,
                    "startup owned account connect: attaching account to existing session"
                );
                let session = self.get_session(id).await?;
                let mut session = session.lock().await;
                self.wire_spacemolt_account_events(id, &account);
                Self::install_spacemolt_account(
                    &mut session,
                    account,
                    selector.clone(),
                    base_url.clone(),
                );
                installed.push(id);
                continue;
            }

            let id = Uuid::new_v4();
            if !self.reserve_session_label(&label, id)? {
                continue;
            }
            let mut session = SessionHandle::new(label);
            self.wire_spacemolt_account_events(id, &account);
            Self::install_spacemolt_account(
                &mut session,
                account,
                selector.clone(),
                base_url.clone(),
            );
            self.sessions
                .write()
                .insert(id, Arc::new(Mutex::new(session)));
            self.note_roster_changed(id);
            created.push(id);
            installed.push(id);
        }

        Ok((created.len(), installed))
    }

    pub fn parse_id(id: &str) -> Result<Uuid, SdkError> {
        Uuid::parse_str(id).map_err(|_| SdkError::InvalidSessionId)
    }

    pub async fn get_session(&self, id: Uuid) -> Result<Arc<Mutex<SessionHandle>>, SdkError> {
        self.sessions
            .read()
            .get(&id)
            .cloned()
            .ok_or(SdkError::SessionNotFound)
    }

    pub async fn get_session_by_str(
        &self,
        id: &str,
    ) -> Result<Arc<Mutex<SessionHandle>>, SdkError> {
        let id = Self::parse_id(id)?;
        self.get_session(id).await
    }

    pub async fn begin_script_run(
        &self,
        id: Uuid,
        origin: &'static str,
    ) -> Result<ScriptRunGuard, SdkError> {
        let session_handle = self.get_session(id).await?;
        let (halt_tx, _halt_rx) = watch::channel(false);
        let started_utc = Utc::now();
        {
            let mut active_runs = self.active_script_runs.lock();
            if let Some(existing) = active_runs.get(&id) {
                return Err(SdkError::BadRequest(format!(
                    "script already running for session {id} via {} since {}",
                    existing.origin, existing.started_utc
                )));
            }
            active_runs.insert(
                id,
                ScriptRunInfo {
                    origin,
                    started_utc,
                    halt_tx,
                    action_generation: origin.starts_with("sdk action").then_some(0),
                },
            );
        }
        let mut session = session_handle.lock().await;
        let current_line = session.engine.snapshot().current_script_line;
        let run_id = session
            .engine
            .normal_lane_claim()
            .and_then(|claim| match claim.owner {
                QueueOwner::PrayerLang { run_id } => Some(run_id),
                _ => None,
            });
        let execution_script = session.current_control_input.clone();
        let execution_id = match session.script_execution.as_mut() {
            Some(execution)
                if matches!(execution.state, ScriptExecutionStateDto::Running { .. }) =>
            {
                execution.state = ScriptExecutionStateDto::Running {
                    current_line,
                    last_line: None,
                    outcome: None,
                };
                execution.run_id = run_id.clone();
                execution.script = execution_script.clone();
                execution.id
            }
            _ => {
                let execution_id = Uuid::new_v4();
                session.script_execution = Some(ScriptExecutionDto {
                    id: execution_id,
                    run_id,
                    script: execution_script,
                    frame_kind: Some("main".into()),
                    frame_name: None,
                    state: ScriptExecutionStateDto::Running {
                        current_line,
                        last_line: None,
                        outcome: None,
                    },
                });
                execution_id
            }
        };
        drop(session);
        Ok(ScriptRunGuard {
            active_script_runs: Arc::clone(&self.active_script_runs),
            id,
            origin,
            started_utc,
            session: session_handle,
            execution_id,
            action_generation: origin.starts_with("sdk action").then_some(0),
            released: false,
        })
    }

    /// Ensure an action runner owns this session. An existing action runner is
    /// reusable: advancing its generation makes it re-check the scheduler before
    /// it can unregister. Explicit script runners remain exclusive.
    pub async fn ensure_action_runner(
        self: &Arc<Self>,
        id: Uuid,
        origin: &'static str,
    ) -> Result<(), SdkError> {
        {
            let mut active_runs = self.active_script_runs.lock();
            if let Some(existing) = active_runs.get_mut(&id) {
                if let Some(generation) = existing.action_generation.as_mut() {
                    *generation = generation.wrapping_add(1);
                    return Ok(());
                }
                return Err(SdkError::BadRequest(format!(
                    "script already running for session {id} via {} since {}",
                    existing.origin, existing.started_utc
                )));
            }
        }
        self.spawn_script_runner(id, origin).await
    }

    async fn action_runner_should_continue(&self, guard: &mut ScriptRunGuard) -> bool {
        let session = match self.get_session(guard.id).await {
            Ok(session) => session,
            Err(_) => return false,
        };
        let session = session.lock().await;
        let scheduler_has_work = session.engine.has_unfinished_action_run();
        let mut active_runs = guard.active_script_runs.lock();
        let Some(active) = active_runs.get(&guard.id) else {
            guard.released = true;
            return false;
        };
        let generation_changed = active.action_generation != guard.action_generation;
        if scheduler_has_work || generation_changed {
            guard.action_generation = active.action_generation;
            return true;
        }
        active_runs.remove(&guard.id);
        guard.released = true;
        false
    }

    pub async fn script_run_info(&self, id: Uuid) -> Option<ScriptRunInfo> {
        self.active_script_runs.lock().get(&id).cloned()
    }

    pub async fn script_halt_receiver(&self, id: Uuid) -> Option<watch::Receiver<bool>> {
        self.active_script_runs
            .lock()
            .get(&id)
            .map(|run| run.halt_tx.subscribe())
    }

    pub async fn notify_script_halt(&self, id: Uuid) {
        if let Some(run) = self.active_script_runs.lock().get(&id) {
            let _ = run.halt_tx.send(true);
        }
    }

    pub async fn execute_registered_script_run(
        &self,
        id: Uuid,
        origin: &'static str,
        poll_across_waits: bool,
    ) -> Result<ExecuteScriptResponse, SdkError> {
        let run_guard = self.begin_script_run(id, origin).await?;
        self.execute_script_run_after_begin(id, poll_across_waits, run_guard)
            .await
    }

    pub async fn execute_script_run_after_begin(
        &self,
        id: Uuid,
        poll_across_waits: bool,
        mut run_guard: ScriptRunGuard,
    ) -> Result<ExecuteScriptResponse, SdkError> {
        let result = loop {
            let result = self
                .execute_script_with_wait_policy(id, poll_across_waits)
                .await;
            if run_guard.action_generation.is_some()
                && self.action_runner_should_continue(&mut run_guard).await
            {
                continue;
            }
            break result;
        };
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let last_line = session.engine.snapshot().current_script_line;
        let outcome = match &result {
            Ok(response) if response.error.is_some() => ScriptOutcomeDto::Error {
                kind: ScriptErrorKindDto::Runtime,
                message: response.error.clone().unwrap(),
            },
            Ok(response) if response.halt_message.as_deref() == Some("halt requested") => {
                ScriptOutcomeDto::Error {
                    kind: ScriptErrorKindDto::UserHalt,
                    message: "Halted by user".into(),
                }
            }
            Ok(response) if response.completed || response.halted => ScriptOutcomeDto::Success {
                message: response.halt_message.clone(),
            },
            Ok(_) => ScriptOutcomeDto::Error {
                kind: ScriptErrorKindDto::RunnerExited,
                message: "Script runner exited before completing".into(),
            },
            Err(err) => ScriptOutcomeDto::Error {
                kind: ScriptErrorKindDto::Internal,
                message: err.to_string(),
            },
        };
        if let Some(execution) = session.script_execution.as_mut() {
            if matches!(execution.state, ScriptExecutionStateDto::Running { .. }) {
                execution.state = ScriptExecutionStateDto::Stopped {
                    current_line: None,
                    last_line,
                    outcome,
                };
            }
        }
        drop(session);
        result
    }

    pub async fn spawn_script_runner(
        self: &Arc<Self>,
        id: Uuid,
        origin: &'static str,
    ) -> Result<(), SdkError> {
        let run_guard = self.begin_script_run(id, origin).await?;
        let service = Arc::clone(self);
        self.spawn_background(async move {
            let started = Instant::now();
            info!(%id, origin, "script runner started");
            let result = service
                .execute_script_run_after_begin(id, true, run_guard)
                .await;
            match result {
                Ok(result) => info!(
                    %id,
                    origin,
                    elapsed_ms = started.elapsed().as_millis(),
                    steps_executed = result.steps_executed,
                    halted = result.halted,
                    completed = result.completed,
                    error = result.error.as_deref().unwrap_or("(none)"),
                    "script runner finished"
                ),
                Err(err) => warn!(
                    %id,
                    origin,
                    elapsed_ms = started.elapsed().as_millis(),
                    error = %err,
                    "script runner failed"
                ),
            }
            service.archive_current_action_run(id).await;
        });
        Ok(())
    }

    pub async fn start_script_runner(
        self: &Arc<Self>,
        id: Uuid,
        origin: &'static str,
    ) -> Result<(), SdkError> {
        self.spawn_script_runner(id, origin).await
    }

    /// Fetch faction information through the runtime domain boundary.
    pub async fn faction_info(
        &self,
        id: Uuid,
        faction_id: Option<String>,
    ) -> Result<serde_json::Value, SdkError> {
        self.spacemolt_account(id)
            .await?
            .commands()
            .spacemolt_faction()
            .info(Some(SpacemoltFactionInfoParams {
                limit: None,
                offset: None,
                id: faction_id.filter(|value| !value.trim().is_empty()),
            }))
            .await
            .map_err(SdkError::from)?
            .into_value()
            .map_err(SdkError::from)
    }

    /// Execute a typed faction operation and refresh the session state after it.
    pub fn summary_from_session(id: Uuid, session: &SessionHandle) -> SessionSummary {
        let snapshot = session.engine.snapshot();
        let latest_system = session
            .has_state
            .then(|| session.actor.observed.location.system_id.clone())
            .flatten();
        let latest_poi = session
            .has_state
            .then(|| session.actor.observed.location.poi_id.clone())
            .flatten();
        SessionSummary {
            id: id.to_string(),
            label: session.label.clone(),
            created_utc: session.created_utc,
            last_updated_utc: session.last_updated_utc,
            is_halted: snapshot.is_halted,
            has_active_command: snapshot.active_command.is_some(),
            current_script_line: snapshot.current_script_line,
            latest_system,
            latest_poi,
        }
    }

    pub fn cache_session_summary(&self, id: Uuid, summary: &SessionSummary) {
        self.session_summary_cache
            .lock()
            .insert(id, summary.clone());
    }

    pub async fn summary_for(
        &self,
        id: Uuid,
        session: &Arc<Mutex<SessionHandle>>,
    ) -> SessionSummary {
        let session = session.lock().await;
        let summary = Self::summary_from_session(id, &session);
        self.cache_session_summary(id, &summary);
        summary
    }

    /// List all session summaries.
    pub async fn list_sessions(&self) -> Vec<SessionSummary> {
        let started = Instant::now();
        // In production, the owned-account connection is the authoritative
        // session inventory. Persisted automation may be staged in memory
        // while startup reconciliation is still running, but it must never
        // make a disconnected historical account visible to clients.
        let include_disconnected = self.options.local_auth_bypass;
        let entries: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, session.clone()))
            .collect();
        let collect_entries_ms = started.elapsed().as_millis();
        let mut out = Vec::with_capacity(entries.len());
        let mut max_lock_wait_ms = 0;
        let mut max_lock_wait_session = String::new();
        let mut cached_busy_sessions = 0;
        let mut uncached_busy_sessions = 0;
        for (id, session) in entries {
            let lock_started = Instant::now();
            let Ok(session) = session.try_lock() else {
                if let Some(summary) = self.session_summary_cache.lock().get(&id).cloned() {
                    cached_busy_sessions += 1;
                    out.push(summary);
                    continue;
                }

                uncached_busy_sessions += 1;
                let session = session.lock().await;
                let lock_wait_ms = lock_started.elapsed().as_millis();
                if lock_wait_ms > max_lock_wait_ms {
                    max_lock_wait_ms = lock_wait_ms;
                    max_lock_wait_session = session.label.clone();
                }
                if !include_disconnected && session.spacemolt_account.is_none() {
                    self.session_summary_cache.lock().remove(&id);
                    continue;
                }
                let summary = Self::summary_from_session(id, &session);
                self.cache_session_summary(id, &summary);
                out.push(summary);
                continue;
            };
            let lock_wait_ms = lock_started.elapsed().as_millis();
            if lock_wait_ms > max_lock_wait_ms {
                max_lock_wait_ms = lock_wait_ms;
                max_lock_wait_session = session.label.clone();
            }
            if !include_disconnected && session.spacemolt_account.is_none() {
                self.session_summary_cache.lock().remove(&id);
                continue;
            }
            let summary = Self::summary_from_session(id, &session);
            self.cache_session_summary(id, &summary);
            out.push(summary);
        }
        out.sort_by(|a, b| a.created_utc.cmp(&b.created_utc));
        let total_ms = started.elapsed().as_millis();
        if total_ms >= 750 {
            warn!(
                total_ms,
                count = out.len(),
                collect_entries_ms,
                max_lock_wait_ms,
                max_lock_wait_session = %max_lock_wait_session,
                cached_busy_sessions,
                uncached_busy_sessions,
                "runtime sessions list slow"
            );
        }
        out
    }

    /// Return one session summary.
    pub async fn session_summary(&self, id: &str) -> Result<SessionSummary, SdkError> {
        let uid = Self::parse_id(id)?;
        let session = self.get_session(uid).await?;
        Ok(self.summary_for(uid, &session).await)
    }

    /// Set active script for a session.
    pub async fn set_script(&self, id: Uuid, script: String) -> Result<String, SdkError> {
        if let Some(existing) = self.script_run_info(id).await {
            return Err(SdkError::BadRequest(format!(
                "script already running for session {id} via {} since {}; halt it before loading a new script",
                existing.origin, existing.started_utc
            )));
        }
        let mut refreshed_after_stale_identity = false;
        let normalized = loop {
            let session = self.get_session(id).await?;
            let mut session = session.lock().await;
            if let Some(claim) = session.engine.normal_lane_claim() {
                if matches!(
                    claim.owner,
                    prayer_scheduler::QueueOwner::Manual { .. }
                        | prayer_scheduler::QueueOwner::Controller { .. }
                ) {
                    return Err(SdkError::LaneBusy {
                        run_id: queue_owner_run_id(&claim.owner),
                        owner: (&claim.owner).into(),
                        generation: claim.generation,
                    });
                }
            }
            let actor = Arc::clone(&session.actor.observed);
            let knowledge = self.knowledge_state.snapshot();
            let world =
                world_read_state_with_metadata(&knowledge, &self.knowledge_metadata.read(), &actor);
            let runtime = session.engine.execution_runtime_state();
            let state_snapshot =
                session
                    .has_state
                    .then_some(prayer_runtime::read_context::ExecutionReadContext {
                        bot: &actor,
                        world: &world,
                        runtime: &runtime,
                    });
            let normalized = match session.engine.set_script(&script, state_snapshot) {
                Ok(normalized) => normalized,
                Err(err)
                    if !refreshed_after_stale_identity
                        && script_error_may_be_stale_identity(&err) =>
                {
                    drop(session);
                    self.refresh_state(id).await?;
                    refreshed_after_stale_identity = true;
                    continue;
                }
                Err(err) => return Err(err.into()),
            };
            session.current_control_input = Some(script);
            session.push_status("Script loaded and activated");
            session.last_updated_utc = Utc::now();
            break normalized;
        };
        self.persist_sessions("after script update").await;
        Ok(normalized)
    }

    pub async fn engine_snapshot_response(
        &self,
        id: Uuid,
    ) -> Result<RuntimeSnapshotResponse, SdkError> {
        let total_started = Instant::now();

        // Lazily fetch live state the first time a snapshot is requested so
        // idle sessions show real system/POI instead of null.
        let initial_lookup_started = Instant::now();
        let session = self.get_session(id).await?;
        let initial_lookup_ms = initial_lookup_started.elapsed().as_millis();
        let initial_lock_started = Instant::now();
        let needs_init = {
            let session = session.lock().await;
            !session.has_state && session.spacemolt_account.is_some()
        };
        let initial_lock_wait_ms = initial_lock_started.elapsed().as_millis();

        let mut init_lookup_ms = 0;
        let init_lock_wait_ms = 0;
        let mut init_refresh_ms = 0;
        if needs_init {
            let started = Instant::now();
            let _session = self.get_session(id).await?;
            init_lookup_ms = started.elapsed().as_millis();
            let started = Instant::now();
            if let Err(err) = self.refresh_state_for_host_loop(id, true).await {
                warn!(?err, "engine_snapshot_response: initial state fetch failed");
            }
            init_refresh_ms = started.elapsed().as_millis();
        }

        let runner_started = Instant::now();
        let runner = self.script_run_info(id).await;
        let runner_ms = runner_started.elapsed().as_millis();
        let final_lookup_started = Instant::now();
        let session = self.get_session(id).await?;
        let final_lookup_ms = final_lookup_started.elapsed().as_millis();
        let final_lock_started = Instant::now();
        let session = session.lock().await;
        let final_lock_wait_ms = final_lock_started.elapsed().as_millis();
        let session_label = session.label.clone();
        let state_version = session.state_version;
        let has_state = session.has_state;
        let has_spacemolt_account = session.spacemolt_account.is_some();
        let live_system = session.actor.observed.location.system_id.clone();
        let live_poi = session.actor.observed.location.poi_id.clone();
        let effective_system = session.actor.observed.location.system_id.clone();
        let effective_poi = session.actor.observed.location.poi_id.clone();
        let snapshot_started = Instant::now();
        let snap = session.engine.snapshot();
        let execution = session.engine.execution_snapshot();
        let mut script_execution = session.script_execution.clone();
        let snapshot_ms = snapshot_started.elapsed().as_millis();
        // A restored checkpoint can be unhalted and retain an active line or
        // command even when no task is executing it. Only the runner registry
        // is authoritative for whether a script is actually running.
        if let Some(execution) = script_execution.as_mut() {
            if let ScriptExecutionStateDto::Running { current_line, .. } = &mut execution.state {
                *current_line = snap.current_script_line;
            }
        }
        let script_running = matches!(
            script_execution.as_ref().map(|v| &v.state),
            Some(ScriptExecutionStateDto::Running { .. })
        );
        let knowledge_started = Instant::now();
        let knowledge = self.knowledge_state.read().clone();
        let knowledge_clone_ms = knowledge_started.elapsed().as_millis();
        let compose_started = Instant::now();
        let actor = session.has_state.then_some(&session.actor.observed);
        let navigation =
            actor.map(|actor| ActorNavigationRead::new(actor, Arc::clone(&knowledge.galaxy)));
        let compose_ms = compose_started.elapsed().as_millis();
        let active_route_started = Instant::now();
        let active_route = navigation.as_ref().and_then(|s| active_go_route(&snap, s));
        let active_route_ms = active_route_started.elapsed().as_millis();
        let response_started = Instant::now();
        let (
            username,
            latest_system,
            latest_poi,
            home_base,
            home_poi,
            docked,
            fuel,
            max_fuel,
            fuel_percent,
            fuel_per_jump,
            hull,
            max_hull,
            cargo_used,
            cargo_capacity,
            passenger_berths,
            cargo,
            credits,
            in_transit,
            transit_dest_system,
            transit_dest_poi,
            in_battle,
            combat_stance,
            combat_target,
        ) = if let Some(s) = actor {
            (
                s.player.username.clone(),
                s.location.system_id.clone(),
                s.location.poi_id.clone(),
                s.player.home_base.clone(),
                s.player.home_poi.clone(),
                Some(s.location.docked_at.is_some()),
                Some(s.fuel),
                Some(s.max_fuel),
                Some(s.fuel_pct),
                estimated_jump_fuel_per_jump(s, &knowledge.catalog),
                s.ship.hull,
                s.ship.max_hull,
                Some(s.cargo_used),
                Some(s.cargo_capacity),
                Some(
                    s.passengers.economy_berths.max
                        + s.passengers.business_berths.max
                        + s.passengers.first_berths.max,
                ),
                s.cargo.as_ref().clone(),
                Some(s.player.credits.unwrap_or_default()),
                s.location.in_transit.unwrap_or(false),
                s.location.transit_dest_system_id.clone(),
                s.location.transit_dest_poi_id.clone(),
                s.in_battle,
                s.combat_stance.clone(),
                s.combat_target.clone(),
            )
        } else {
            (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Default::default(),
                None,
                false,
                None,
                None,
                false,
                None,
                None,
            )
        };
        let cargo_len = cargo.len();
        let (compat_halted, compat_finished, compat_message) =
            match script_execution.as_ref().map(|v| &v.state) {
                Some(ScriptExecutionStateDto::Running { .. }) => (false, false, None),
                Some(ScriptExecutionStateDto::Stopped {
                    outcome: ScriptOutcomeDto::Success { message },
                    ..
                }) => (false, true, message.clone()),
                Some(ScriptExecutionStateDto::Stopped {
                    outcome: ScriptOutcomeDto::Error { message, .. },
                    ..
                }) => (true, false, Some(message.clone())),
                None => (
                    snap.is_halted,
                    snap.is_finished,
                    latest_result_message(&snap),
                ),
            };
        let response = RuntimeSnapshotResponse {
            session_id: id.to_string(),
            username,
            state_version,
            knowledge_version: knowledge.knowledge_version,
            snapshot: RuntimeHostSnapshotDto {
                is_halted: compat_halted,
                is_finished: compat_finished,
                has_active_command: snap.active_command.is_some(),
                current_script_line: snap.current_script_line,
                current_script: if snap.script.is_empty() {
                    None
                } else {
                    Some(snap.script.clone())
                },
                result_message: compat_message,
                active_frame: map_active_frame(&snap),
            },
            execution,
            script_execution,
            latest_system,
            latest_poi,
            home_base,
            home_poi,
            docked,
            fuel,
            max_fuel,
            fuel_percent,
            fuel_per_jump,
            hull,
            max_hull,
            cargo_used,
            cargo_capacity,
            passenger_berths,
            cargo,
            credits,
            last_updated_utc: session.last_updated_utc,
            script_running,
            script_runner: runner.map(|runner| ScriptRunnerDto {
                origin: runner.origin.to_string(),
                started_utc: runner.started_utc,
            }),
            active_route,
            in_transit,
            transit_dest_system,
            transit_dest_poi,
            in_battle,
            combat_stance,
            combat_target,
        };
        let response_ms = response_started.elapsed().as_millis();
        let total_ms = total_started.elapsed().as_millis();
        info!(
            %id,
            session = %session_label,
            total_ms,
            needs_init,
            has_state,
            has_spacemolt_account,
            state_version,
            live_system = live_system.as_deref().unwrap_or("(none)"),
            live_poi = live_poi.as_deref().unwrap_or("(none)"),
            effective_system = effective_system.as_deref().unwrap_or("(none)"),
            effective_poi = effective_poi.as_deref().unwrap_or("(none)"),
            latest_system = response.latest_system.as_deref().unwrap_or("(none)"),
            latest_poi = response.latest_poi.as_deref().unwrap_or("(none)"),
            fuel = response.fuel.unwrap_or(-1),
            max_fuel = response.max_fuel.unwrap_or(-1),
            fuel_percent = response.fuel_percent.unwrap_or(-1),
            cargo_used = response.cargo_used.unwrap_or(-1),
            cargo_capacity = response.cargo_capacity.unwrap_or(-1),
            script_running = response.script_running,
            "engine snapshot response built"
        );
        if total_ms >= 750 {
            warn!(
                %id,
                session = %session_label,
                total_ms,
                needs_init,
                has_state,
                has_spacemolt_account,
                state_version,
                initial_lookup_ms,
                initial_lock_wait_ms,
                init_lookup_ms,
                init_lock_wait_ms,
                init_refresh_ms,
                runner_ms,
                final_lookup_ms,
                final_lock_wait_ms,
                snapshot_ms,
                knowledge_clone_ms,
                compose_ms,
                active_route_ms,
                response_ms,
                cargo_len,
                "engine snapshot response slow"
            );
        }
        Ok(response)
    }

    /// Halt a session.
    pub async fn halt(&self, id: Uuid, reason: Option<String>) -> Result<(), SdkError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let reason = reason.unwrap_or_else(|| "halt requested".to_string());
        session.engine.clear(&reason);
        let last_line = session.engine.snapshot().current_script_line;
        if let Some(execution) = session.script_execution.as_mut() {
            execution.state = ScriptExecutionStateDto::Stopped {
                current_line: None,
                last_line,
                outcome: ScriptOutcomeDto::Error {
                    kind: ScriptErrorKindDto::UserHalt,
                    message: reason.clone(),
                },
            };
        }
        session.push_status(reason);
        session.last_updated_utc = Utc::now();
        drop(session);
        self.notify_script_halt(id).await;
        self.persist_sessions("after halt").await;
        Ok(())
    }

    /// Build runtime snapshot.
    pub async fn snapshot(&self, id: Uuid) -> Result<RuntimeSnapshot, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.snapshot())
    }

    pub async fn scheduler_snapshot(
        &self,
        id: Uuid,
    ) -> Result<prayer_scheduler::SchedulerSnapshot, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.scheduler_snapshot())
    }

    pub async fn scheduler_prayer(&self, id: Uuid) -> Result<String, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.scheduler_prayer_projection())
    }

    pub async fn normal_scheduler_prayer(&self, id: Uuid) -> Result<String, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.normal_scheduler_prayer_projection())
    }

    pub async fn override_scheduler_prayer(&self, id: Uuid) -> Result<String, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.override_scheduler_prayer_projection())
    }

    pub async fn producer_snapshot(
        &self,
        id: Uuid,
    ) -> Result<prayer_runtime::ProducerSnapshot, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.producer_snapshot())
    }

    pub async fn execution_snapshot(
        &self,
        id: Uuid,
    ) -> Result<prayer_runtime::ExecutionSnapshot, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.execution_snapshot())
    }

    /// Build checkpoint payload.
    pub async fn execution_checkpoint(&self, id: Uuid) -> Result<PersistedExecutionRun, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        session
            .engine
            .execution_checkpoint()
            .map_err(SdkError::from)
    }

    pub async fn try_acquire_action_lane(
        &self,
        id: Uuid,
        run_id: prayer_actions::RunId,
    ) -> Result<prayer_scheduler::QueueClaim, SdkError> {
        self.archive_current_action_run(id).await;
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        if let Some(claim) = session.engine.normal_lane_claim() {
            return Err(SdkError::LaneBusy {
                run_id: queue_owner_run_id(&claim.owner),
                owner: (&claim.owner).into(),
                generation: claim.generation,
            });
        }
        let claim = session.engine.try_acquire_action_run(run_id)?;
        drop(session);
        self.persist_sessions("after action lane acquisition").await;
        Ok(claim)
    }

    pub async fn submit_action_batch(
        &self,
        id: Uuid,
        claim: &prayer_scheduler::QueueClaim,
        actions: Vec<prayer_actions::ActionEnvelope>,
    ) -> Result<(), SdkError> {
        let session = self.get_session(id).await?;
        session
            .lock()
            .await
            .engine
            .submit_action_batch(claim, actions)?;
        self.persist_sessions("after action batch submission").await;
        Ok(())
    }

    pub async fn submit_action_override(
        self: &Arc<Self>,
        id: Uuid,
        mut actions: Vec<prayer_actions::Action>,
        options: crate::OverrideOptions,
    ) -> Result<(), SdkError> {
        if actions.is_empty() {
            return Err(SdkError::BadRequest(
                "override action batch must not be empty".into(),
            ));
        }
        let mut restoration_added = false;
        if options.return_to_origin {
            // This placeholder is materialized from the fresh location passed
            // to the first scheduler preemption check, after in-flight I/O.
            actions.push(prayer_actions::Action::Wait { ticks: 0 });
            restoration_added = true;
        }
        let run_id = prayer_actions::RunId(Uuid::new_v4().to_string());
        let actions_len = actions.len();
        let envelopes = actions
            .into_iter()
            .enumerate()
            .map(|(index, action)| {
                let policy = if restoration_added && index + 1 == actions_len {
                    "client_return_to_origin"
                } else {
                    "client"
                };
                prayer_actions::ActionEnvelope::new(
                    format!("override-{}-{index}", run_id.0),
                    action,
                    prayer_actions::ActionOrigin::Interrupt {
                        policy: policy.into(),
                    },
                )
            })
            .collect();
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        if session.engine.override_lane_busy() {
            let snapshot = session.engine.scheduler_snapshot();
            let active = snapshot
                .interrupt
                .as_ref()
                .map(|running| &running.envelope)
                .or_else(|| snapshot.interrupt_pending.first())
                .expect("busy override lane has an owner");
            return Err(SdkError::LaneBusy {
                owner: crate::LaneOwner::Manual,
                run_id: prayer_actions::RunId(active.id.0.clone()),
                generation: snapshot.generation,
            });
        }
        session.engine.submit_action_override(envelopes)?;
        drop(session);
        self.persist_sessions("after action override submission")
            .await;
        let service = Arc::clone(self);
        self.spawn_background(async move {
            let _ = service.start_script_runner(id, "sdk override lane").await;
        });
        Ok(())
    }

    pub async fn submit_script_override(
        self: &Arc<Self>,
        id: Uuid,
        script: String,
        options: crate::OverrideOptions,
    ) -> Result<(), SdkError> {
        let actions = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            let actor = Arc::clone(&session.actor.observed);
            let knowledge = self.knowledge_state.snapshot();
            let world =
                world_read_state_with_metadata(&knowledge, &self.knowledge_metadata.read(), &actor);
            let runtime = session.engine.execution_runtime_state();
            let context = prayer_runtime::read_context::ExecutionReadContext {
                bot: &actor,
                world: &world,
                runtime: &runtime,
            };
            prayer_runtime::RuntimeEngine::compile_override_script(&script, context)?
        };
        self.submit_action_override(id, actions, options).await
    }

    pub async fn action_run(
        &self,
        id: Uuid,
        run_id: &prayer_actions::RunId,
    ) -> Result<Option<prayer_runtime::execution::PersistedActionRun>, SdkError> {
        let session = self.get_session(id).await?;
        let result = session.lock().await.engine.action_run(run_id);
        Ok(result.or_else(|| {
            self.action_run_history
                .lock()
                .get(&(id, run_id.0.clone()))
                .cloned()
        }))
    }

    pub async fn cancel_action_run(
        &self,
        id: Uuid,
        run_id: &prayer_actions::RunId,
        reason: String,
    ) -> Result<prayer_runtime::execution::PersistedActionRun, SdkError> {
        let session = self.get_session(id).await?;
        let result = session
            .lock()
            .await
            .engine
            .cancel_action_run(run_id, reason)?;
        self.persist_sessions("after action run cancellation").await;
        self.action_run_history
            .lock()
            .insert((id, run_id.0.clone()), result.clone());
        Ok(result)
    }

    async fn archive_current_action_run(&self, id: Uuid) {
        let Ok(session) = self.get_session(id).await else {
            return;
        };
        let run = session
            .lock()
            .await
            .engine
            .execution_checkpoint()
            .ok()
            .and_then(|checkpoint| checkpoint.action_run)
            .filter(|run| run.outcome.is_some());
        if let Some(run) = run {
            self.action_run_history
                .lock()
                .insert((id, run.run_id.0.clone()), run);
        }
    }

    pub async fn script_execution(&self, id: Uuid) -> Result<Option<ScriptExecutionDto>, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        let mut execution = session.script_execution.clone();
        if let Some(execution) = execution.as_mut() {
            if let ScriptExecutionStateDto::Running { current_line, .. } = &mut execution.state {
                *current_line = session.engine.snapshot().current_script_line;
            }
        }
        Ok(execution)
    }

    pub async fn cancel_script_run(&self, id: Uuid, reason: String) -> Result<(), SdkError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let last_line = session.engine.snapshot().current_script_line;
        session.engine.halt(&reason);
        let execution = session
            .script_execution
            .as_mut()
            .ok_or_else(|| SdkError::BadRequest("script run not found".into()))?;
        execution.state = ScriptExecutionStateDto::Stopped {
            current_line: None,
            last_line,
            outcome: ScriptOutcomeDto::Error {
                kind: ScriptErrorKindDto::Cancelled,
                message: reason,
            },
        };
        drop(session);
        self.notify_script_halt(id).await;
        self.persist_sessions("after script run cancellation").await;
        Ok(())
    }

    /// Restore checkpoint.
    pub async fn restore_execution_checkpoint(
        &self,
        id: Uuid,
        checkpoint: PersistedExecutionRun,
    ) -> Result<(), SdkError> {
        let checkpoint_has_analysis = !checkpoint.needs_prayerlang_reanalysis();
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let actor = Arc::clone(&session.actor.observed);
        let knowledge = self.knowledge_state.snapshot();
        let world =
            world_read_state_with_metadata(&knowledge, &self.knowledge_metadata.read(), &actor);
        let runtime = session.engine.execution_runtime_state();
        let state_snapshot = prayer_runtime::read_context::ExecutionReadContext {
            bot: &actor,
            world: &world,
            runtime: &runtime,
        };
        session.engine.restore_execution_checkpoint(checkpoint)?;
        session.restored_checkpoint_needs_reanalysis = !checkpoint_has_analysis;
        if session.restored_checkpoint_needs_reanalysis {
            session
                .engine
                .reanalyze_current_script(Some(state_snapshot))?;
            session.restored_checkpoint_needs_reanalysis = false;
        }
        session.push_status("Resumed from checkpoint");
        session.last_updated_utc = Utc::now();
        drop(session);
        self.persist_sessions("after checkpoint restore").await;
        Ok(())
    }

    /// Drain emitted runtime events.
    pub async fn drain_events(&self, id: Uuid) -> Result<Vec<RuntimeEvent>, SdkError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        Ok(session.engine.drain_events())
    }
}
