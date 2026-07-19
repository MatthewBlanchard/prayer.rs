//! Idle-session, faction-garage, market-watcher, and mobile-capital refresh loops.

use super::*;

impl RuntimeService {
    pub fn start_idle_session_refresher(self: Arc<Self>) {
        let market_service = Arc::clone(&self);
        self.spawn_background(async move {
            let Some(mut market_rx) = market_service.market_update_rx.lock().take() else {
                warn!("market update batcher already started");
                return;
            };
            while let Some(first) = market_rx.recv().await {
                let mut dirty = HashSet::from([first]);
                tokio::time::sleep(MARKET_UPDATE_BATCH_WINDOW).await;
                while let Ok(id) = market_rx.try_recv() {
                    dirty.insert(id);
                }
                market_service.flush_market_update_batch(dirty).await;
                if market_service.is_shutting_down() {
                    break;
                }
            }
        });

        let service = Arc::clone(&self);
        self.spawn_background(async move {
            let mut interval = tokio::time::interval(IDLE_SESSION_REFRESH_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let Some(mut account_state_rx) = service.account_state_rx.lock().take() else {
                warn!("account state event loop already started");
                return;
            };
            loop {
                if service.is_shutting_down() {
                    break;
                }
                tokio::select! {
                    _ = service.shutdown_requested() => break,
                    _ = interval.tick() => service.refresh_idle_sessions_once().await,
                    changed = account_state_rx.recv() => {
                        let Some(id) = changed else { break };
                        let mut changed_ids = HashSet::from([id]);
                        while let Ok(id) = account_state_rx.try_recv() {
                            changed_ids.insert(id);
                        }
                        for id in changed_ids {
                            if let Err(err) = service.refresh_state_for_host_loop(id, false).await {
                                debug!(%id, error = %err, "account state event projection failed");
                            }
                        }
                    }
                }
            }
        });
    }

    async fn flush_market_update_batch(&self, dirty: HashSet<Uuid>) {
        let mut updates = HashMap::new();
        for id in dirty {
            let Ok(session) = self.get_session(id).await else {
                continue;
            };
            let account = session.lock().await.spacemolt_account.clone();
            let Some(account) = account else { continue };
            let mut projected =
                crate::spacemolt_projection::project_account_state(&account.state());
            let Some(base_id) = projected.market_base_id.clone() else {
                continue;
            };
            let Some(book) = account.market(&base_id) else {
                continue;
            };
            crate::spacemolt_projection::project_market_book_from_client(&mut projected, &book);
            updates.extend(projected.world.market.station_markets.clone());
        }
        if updates.is_empty() {
            return;
        }

        let changed = {
            let mut knowledge = self.knowledge_state.write();
            let mut changed = false;
            for (station_id, snapshot) in updates {
                if !knowledge
                    .station_markets
                    .get(&station_id)
                    .is_some_and(|known| {
                        prayer_runtime::knowledge::station_market_snapshot_eq(known, &snapshot)
                    })
                {
                    knowledge.station_markets.insert(station_id, snapshot);
                    changed = true;
                }
            }
            if changed {
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            }
            changed
        };
        if changed {
            self.knowledge_persistence
                .publish_shared(self.knowledge_state.snapshot(), "market update batch");
        }
    }

    pub async fn refresh_idle_sessions_once(self: &Arc<Self>) {
        if let Err(err) = self.refresh_mobile_capital_location_once().await {
            warn!(error = %err, "mobile base location refresh failed");
        }
        self.reconcile_refresh_watchers().await;
        self.refresh_faction_garages_once().await;
        let ids: Vec<Uuid> = self.sessions.read().keys().copied().collect();
        for id in ids {
            let (connected, player_id, faction_id, poi_id) = match self.get_session(id).await {
                Ok(session) => {
                    let session = session.lock().await;
                    (
                        session.spacemolt_account.is_some(),
                        session.actor.observed.player.id.clone(),
                        session.actor.observed.player.faction_id.clone(),
                        session.actor.observed.location.poi_id.clone(),
                    )
                }
                Err(_) => (false, None, None, None),
            };
            if !connected {
                debug!(%id, "idle session refresh skipped: SpaceMolt account is not connected");
                continue;
            }
            if let Err(err) = self.refresh_idle_session(id).await {
                warn!(%id, error = %err, "idle session refresh failed");
            }
            if let Err(err) = self.refresh_chat_knowledge(id).await {
                debug!(%id, error = %err, "chat history hydration failed");
            }
            if let Err(err) = self.refresh_faction_knowledge(id).await {
                debug!(%id, error = %err, "faction hydration failed");
            }
            let facilities_missing = {
                let knowledge = self.knowledge_state.read();
                player_id
                    .as_ref()
                    .is_some_and(|key| !knowledge.owned_facilities_by_player.contains_key(key))
                    || faction_id
                        .as_ref()
                        .is_some_and(|key| !knowledge.owned_facilities_by_faction.contains_key(key))
                    || poi_id
                        .as_ref()
                        .is_some_and(|key| !knowledge.facilities_by_poi.contains_key(key))
            };
            if facilities_missing {
                if let Err(err) = self.facilities_snapshot_response(id).await {
                    debug!(%id, error = %err, "initial facility hydration failed");
                }
            }
        }
    }

    pub async fn refresh_faction_garages_once(&self) {
        let watchers: Vec<(String, Uuid)> = self
            .faction_garage_watchers_by_key
            .lock()
            .iter()
            .map(|(key, id)| (key.clone(), *id))
            .collect();

        for (faction_key, id) in watchers {
            let fresh = {
                let metadata = self.knowledge_metadata.read();
                metadata
                    .faction_garage_fetched_at_by_key
                    .get(&faction_key)
                    .is_some_and(|at| at.elapsed() < IDLE_SESSION_REFRESH_INTERVAL)
            };
            if fresh {
                continue;
            }

            let Some(()) = ({
                match self.get_session(id).await {
                    Ok(session) => {
                        let session = session.lock().await;
                        if current_faction_key(&session).as_deref() == Some(faction_key.as_str()) {
                            Some(())
                        } else {
                            None
                        }
                    }
                    Err(err) => {
                        warn!(
                            %id,
                            faction_key,
                            error = %err,
                            "faction garage global refresh skipped; watcher session unavailable"
                        );
                        None
                    }
                }
            }) else {
                continue;
            };

            let account = match self.spacemolt_account(id).await {
                Ok(account) => account,
                Err(err) => {
                    warn!(%id, faction_key, error = %err, "faction garage global refresh failed");
                    continue;
                }
            };
            match account
                .commands()
                .spacemolt_faction()
                .garages()
                .await
                .map_err(SdkError::from)
            {
                Ok(result) => {
                    let Some(response) = result.structured_content else {
                        warn!(%id, faction_key, "faction garage response omitted structured content");
                        continue;
                    };
                    let garage = crate::spacemolt_projection::project_faction_garages(response);
                    let runner_count = garage
                        .ships
                        .iter()
                        .filter(|ship| {
                            ship.ship.class_id.eq_ignore_ascii_case("runner")
                                || ship
                                    .ship
                                    .class_name
                                    .as_deref()
                                    .is_some_and(|name| name.eq_ignore_ascii_case("runner"))
                        })
                        .count();
                    let (changed, snapshot) = {
                        let mut knowledge = self.knowledge_state.write();
                        self.knowledge_metadata
                            .write()
                            .faction_garage_fetched_at_by_key
                            .insert(faction_key.clone(), Instant::now());
                        let changed =
                            knowledge.faction_garage_by_faction.get(&faction_key) != Some(&garage);
                        if changed {
                            knowledge
                                .faction_garage_by_faction
                                .insert(faction_key.clone(), garage.clone());
                            knowledge.knowledge_version =
                                knowledge.knowledge_version.saturating_add(1);
                        }
                        let snapshot = changed.then(|| knowledge.clone());
                        (changed, snapshot)
                    };
                    if let Some(snapshot) = snapshot {
                        self.knowledge_persistence
                            .publish(snapshot, "after faction garage global refresh");
                    }
                    info!(
                        %id,
                        faction_key,
                        garage_ships = garage.ships.len(),
                        runner_count,
                        changed,
                        "faction garage global refresh parsed"
                    );
                }
                Err(err) => {
                    warn!(
                        %id,
                        faction_key,
                        error = %err,
                        "faction garage global refresh failed"
                    );
                }
            }
        }
    }

    pub async fn refresh_mobile_capital_location_once(&self) -> Result<(), SdkError> {
        let location = tokio::time::timeout(
            MOBILE_BASE_LOOKUP_TIMEOUT,
            self.spacemolt_client.mobile_base_location(),
        )
        .await
        .map_err(|_| SdkError::BadRequest("mobile base lookup timed out".to_string()))?
        .map_err(SdkError::from)?;

        let system = location.system.trim();
        if system.is_empty() {
            return Err(SdkError::BadRequest(
                "mobile base location response did not include a system".to_string(),
            ));
        }

        let (changed, snapshot) = {
            let mut knowledge = self.knowledge_state.write();
            let mut galaxy = knowledge.galaxy.as_ref().clone();
            let changed = apply_mobile_capital_location(&mut galaxy, system);
            if changed {
                galaxy.precompute_routes();
                knowledge.galaxy = Arc::new(galaxy);
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            }
            let snapshot = changed.then(|| knowledge.clone());
            (changed, snapshot)
        };

        if let Some(snapshot) = snapshot {
            self.knowledge_persistence
                .publish(snapshot, "after mobile base refresh");
        }
        if changed {
            info!(system, "mobile base location updated");
        }
        Ok(())
    }

    pub async fn reconcile_refresh_watchers(&self) {
        let sessions: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, Arc::clone(session)))
            .collect();
        let mut docked_by_station: HashMap<String, Vec<Uuid>> = HashMap::new();
        let mut observation_by_poi: HashMap<String, Vec<Uuid>> = HashMap::new();
        let mut faction_storage_by_key: HashMap<String, Vec<Uuid>> = HashMap::new();
        let mut faction_garage_by_key: HashMap<String, Vec<Uuid>> = HashMap::new();
        for (id, session) in sessions {
            let session = session.lock().await;
            if let Some(account) = session.spacemolt_account.as_ref() {
                let projected =
                    crate::spacemolt_projection::project_account_state(&account.state());
                if let Some(poi_id) = projected
                    .bot
                    .location
                    .poi_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    observation_by_poi
                        .entry(poi_id.to_string())
                        .or_default()
                        .push(id);
                }
            }
            // Market snapshots, market watchers, and passenger boards are all
            // keyed by the docked POI id (the actor's canonical location).
            if let Some(station_id) = docked_station_key(&session) {
                docked_by_station.entry(station_id).or_default().push(id);
            }
            if let Some(key) = current_faction_station_storage_key(&session) {
                faction_storage_by_key.entry(key).or_default().push(id);
            }
            if let Some(key) = current_faction_key(&session) {
                faction_garage_by_key.entry(key).or_default().push(id);
            }
        }

        for candidates in docked_by_station.values_mut() {
            candidates.sort();
        }
        for candidates in observation_by_poi.values_mut() {
            candidates.sort();
        }
        for candidates in faction_storage_by_key.values_mut() {
            candidates.sort();
        }
        for candidates in faction_garage_by_key.values_mut() {
            candidates.sort();
        }
        let occupied_stations: HashSet<String> = docked_by_station.keys().cloned().collect();

        {
            let mut watchers = self.market_watchers.lock();
            watchers.retain(|station_id, watcher| {
                docked_by_station
                    .get(station_id)
                    .is_some_and(|candidates| candidates.contains(watcher))
            });
            for (station_id, candidates) in docked_by_station.clone() {
                if let Some(candidate) = candidates.first().copied() {
                    watchers.entry(station_id).or_insert(candidate);
                }
            }
        }
        {
            let mut watchers = self.faction_storage_watchers_by_key.lock();
            reconcile_watchers_for_key(&mut watchers, faction_storage_by_key);
        }
        {
            let mut watchers = self.observation_watchers_by_poi.lock();
            reconcile_watchers_for_key(&mut watchers, observation_by_poi);
        }
        {
            let mut watchers = self.faction_garage_watchers_by_key.lock();
            reconcile_watchers_for_key(&mut watchers, faction_garage_by_key);
        }

        let snapshot = {
            let mut knowledge = self.knowledge_state.write();
            let before = knowledge.station_markets.len();
            let market_stations_before = knowledge
                .station_markets
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            knowledge
                .station_markets
                .retain(|station_id, _| occupied_stations.contains(station_id));
            let market_stations_after = knowledge
                .station_markets
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            let pruned_market_stations = market_stations_before
                .difference(&market_stations_after)
                .cloned()
                .collect::<Vec<_>>();
            let before_passengers = knowledge.station_passengers.len();
            knowledge
                .station_passengers
                .retain(|station_id, _| occupied_stations.contains(station_id));
            self.knowledge_metadata
                .write()
                .station_passengers_fetched_at_by_station
                .retain(|station_id, _| occupied_stations.contains(station_id));
            if knowledge.station_markets.len() != before
                || knowledge.station_passengers.len() != before_passengers
            {
                if !pruned_market_stations.is_empty() {
                    warn!(
                        ?pruned_market_stations,
                        ?occupied_stations,
                        before_station_count = before,
                        after_station_count = knowledge.station_markets.len(),
                        "market lineage pruned station books during watcher reconciliation"
                    );
                }
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
                Some(knowledge.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = snapshot {
            self.knowledge_persistence
                .publish(snapshot, "after refresh watcher reconciliation");
        }
    }

    pub async fn refresh_idle_session(&self, id: Uuid) -> Result<(), SdkError> {
        if self.script_run_info(id).await.is_some() {
            return Ok(());
        }

        self.refresh_state_for_host_loop(id, false).await?;

        if self.script_run_info(id).await.is_some() {
            return Ok(());
        }

        let session = self.get_session(id).await?;
        let session = session.lock().await;

        let should_run_combat = session.actor.observed.in_battle;
        drop(session);
        if should_run_combat && self.script_run_info(id).await.is_none() {
            let step = self.execute_step_inner(id, None).await?;
            if step.executed {
                debug!(
                    %id,
                    command = step.command_action.as_deref().unwrap_or("(unknown)"),
                    "idle combat interrupt step executed"
                );
            }
        }
        Ok(())
    }

    pub async fn reanalyze_restored_script_with_actor_world(
        &self,
        id: Uuid,
    ) -> Result<(), SdkError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        if !session.restored_checkpoint_needs_reanalysis {
            return Ok(());
        }
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
        session
            .engine
            .reanalyze_current_script(Some(state_snapshot))?;
        session.restored_checkpoint_needs_reanalysis = false;
        Ok(())
    }
}
