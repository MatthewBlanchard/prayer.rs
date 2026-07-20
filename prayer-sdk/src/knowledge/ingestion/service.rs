//! RuntimeService observation ingestion and refresh-state application.

use super::super::*;

pub(crate) fn should_refresh_owned_ships(
    has_state: bool,
    has_owned_ship_details: bool,
    previous_docked_at: Option<&str>,
    current_docked_at: Option<&str>,
) -> bool {
    !has_state
        || !has_owned_ship_details
        || (current_docked_at.is_some() && current_docked_at != previous_docked_at)
}

impl RuntimeService {
    /// Canonical entry point for query, mutation, refresh, status, and
    /// subscription observations.
    pub fn ingest_observations(
        &self,
        observations: impl IntoIterator<Item = StateObservation>,
        save_context: &'static str,
    ) -> Arc<WorldState> {
        for observation in observations {
            self.merge_observation_into_knowledge(&observation, save_context);
        }
        self.knowledge_state.snapshot()
    }

    /// Merge an observation into the shared knowledge store and publish an
    /// immutable snapshot for asynchronous persistence when it changes.
    pub fn merge_observation_into_knowledge(
        &self,
        observation: &StateObservation,
        save_context: &'static str,
    ) -> Arc<WorldState> {
        let incoming_market_stations = observation
            .world
            .market
            .station_markets
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let changed = {
            let mut knowledge = self.knowledge_state.write();
            let mut metadata = self.knowledge_metadata.write();
            let merged = merge_knowledge_state_if_changed_with_metadata(
                &mut knowledge,
                &mut metadata,
                observation,
            );
            let dump_modes_changed =
                reconcile_virtual_order_dump_modes(&mut knowledge, &observation.bot.state);
            if dump_modes_changed && !merged {
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            }
            merged || dump_modes_changed
        };
        let knowledge = self.knowledge_state.snapshot();
        if !incoming_market_stations.is_empty() {
            let retained_market_stations = knowledge
                .station_markets
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            let missing_after_merge = incoming_market_stations
                .difference(&retained_market_stations)
                .cloned()
                .collect::<Vec<_>>();
            let incoming_market_orders = observation
                .world
                .market
                .station_markets
                .values()
                .map(|snapshot| {
                    snapshot.buy_orders.values().map(Vec::len).sum::<usize>()
                        + snapshot.sell_orders.values().map(Vec::len).sum::<usize>()
                })
                .sum::<usize>();
            info!(
                save_context,
                incoming_station_count = incoming_market_stations.len(),
                incoming_market_orders,
                incoming_stations = ?incoming_market_stations,
                retained_station_count = retained_market_stations.len(),
                retained_stations = ?retained_market_stations,
                missing_after_merge = ?missing_after_merge,
                knowledge_version = knowledge.knowledge_version,
                "market lineage after knowledge merge"
            );
            if !missing_after_merge.is_empty() {
                warn!(
                    ?missing_after_merge,
                    "fresh market books disappeared during knowledge merge"
                );
            }
        }
        if changed {
            self.knowledge_persistence
                .publish_shared(Arc::clone(&knowledge), save_context);
        }
        knowledge
    }

    /// Merge a fresh observation into the shared knowledge store, persist it
    /// (with failure telemetry), and apply the result to the session.
    pub fn ingest_fetched_state(
        &self,
        session: &mut SessionHandle,
        observation: StateObservation,
        save_context: &'static str,
    ) {
        let knowledge = self.ingest_observations([observation.clone()], save_context);
        let docked_crafting_queue_fetched = observation.docked_crafting_queue_fetched;
        let commission_status_fetched = observation.commission_status_fetched;
        let passengers_fetched = observation.passengers_fetched;
        let docked_passengers_fetched = observation.docked_passengers_fetched;
        let preserve_core_status = observation.bot.state.location.system_id.is_none()
            && observation.bot.state.location.poi_id.is_none()
            && (observation.agents_fetched
                || observation.nearby_fetched
                || observation.wrecks_fetched
                || observation.missions_fetched
                || observation.ships_fetched
                || observation.commission_status_fetched
                || observation.docked_missions_fetched
                || observation.docked_storage_fetched
                || observation.docked_faction_storage_fetched
                || observation.docked_crafting_queue_fetched
                || observation.passengers_fetched
                || observation.docked_passengers_fetched);
        apply_live_state_inner(
            session,
            observation.bot.state,
            &knowledge,
            docked_crafting_queue_fetched,
            commission_status_fetched,
            passengers_fetched,
            docked_passengers_fetched,
            preserve_core_status,
        );
    }

    /// Ingest a refreshed state plus the first-state/halt bookkeeping shared by
    /// the host loop and the `execute_step` prefetch.
    pub fn apply_refreshed_state(
        &self,
        id: Uuid,
        session: &mut SessionHandle,
        observation: StateObservation,
        is_halted: bool,
    ) {
        self.ingest_fetched_state(session, observation, "during refresh");
        if session.bot_id.is_none() {
            session.bot_id = session
                .actor
                .observed
                .player
                .id
                .as_deref()
                .or(session.actor.observed.player.username.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(BotId::from);
        }
        if is_halted {
            session.last_halted_state_refresh = Some(Instant::now());
        }
        session.last_state_refresh_completed_at = Some(Instant::now());
        session.touch_state();
        self.note_session_changed(id);
    }

    pub async fn refresh_state_for_host_loop(
        &self,
        id: Uuid,
        force: bool,
    ) -> Result<prayer_state::FleetEntry, SdkError> {
        let requested_at = Instant::now();
        let refresh_lock = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            Arc::clone(&session.state_refresh_lock)
        };
        let _refresh_guard = refresh_lock.lock().await;

        let (account, cached, had_state, had_owned_ship_details, previous_docked_at) = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            if !force
                && session.has_state
                && session.engine.snapshot().is_halted
                && session
                    .last_halted_state_refresh
                    .is_some_and(|last| last.elapsed() < Duration::from_secs(1))
            {
                drop(session);
                return self.bot_snapshot(id).await;
            }
            (
                session.spacemolt_account.clone(),
                (force
                    && session.has_state
                    && session
                        .last_state_refresh_completed_at
                        .is_some_and(|completed_at| completed_at >= requested_at))
                .then_some(()),
                session.has_state,
                !session.actor.observed.owned_ship_details.is_empty(),
                session.actor.observed.location.docked_at.clone(),
            )
        };
        if cached.is_some() {
            return self.bot_snapshot(id).await;
        }
        let account = account.ok_or_else(|| {
            SdkError::BadRequest("SpaceMolt account is not connected".to_string())
        })?;

        let account_state = account.state();
        let snapshot_sections = account_state.section_count();
        let location = account_state.location().ok().flatten().unwrap_or_default();
        let ship = account_state.ship().ok().flatten().unwrap_or_default();
        let snapshot_system = location.system_id.as_deref().unwrap_or("(none)");
        let snapshot_poi = location.poi_id.as_deref().unwrap_or("(none)");
        let snapshot_docked = location.docked_at.as_deref().unwrap_or("(none)");
        let snapshot_fuel = ship.fuel;
        let snapshot_max_fuel = ship.max_fuel;
        let snapshot_cargo_capacity = ship.cargo_capacity;
        info!(
            id = %id,
            account_id = account.id().unwrap_or("(none)"),
            snapshot_sections,
            snapshot_system,
            snapshot_poi,
            snapshot_docked,
            snapshot_fuel = snapshot_fuel.unwrap_or(-1),
            snapshot_max_fuel = snapshot_max_fuel.unwrap_or(-1),
            snapshot_cargo_capacity = snapshot_cargo_capacity.unwrap_or(-1),
            "owned account refresh snapshot received"
        );

        let mut projected = crate::spacemolt_projection::project_account_state(&account_state);
        let docked_at_changed = projected.bot.location.docked_at.is_some()
            && projected.bot.location.docked_at != previous_docked_at;
        let refresh_owned_ships = should_refresh_owned_ships(
            had_state,
            had_owned_ship_details,
            previous_docked_at.as_deref(),
            projected.bot.location.docked_at.as_deref(),
        );
        let ships_fetched = if refresh_owned_ships {
            match account.commands().spacemolt_ship().list_ships().await {
                Ok(result) => match result.into_typed() {
                    Ok(response) => {
                        let ship_count = response.ships.len();
                        projected.bot.owned_ship_details = Arc::new(response.ships);
                        info!(
                            id = %id,
                            account_id = account.id().unwrap_or("(none)"),
                            ship_count,
                            initial = !had_state || !had_owned_ship_details,
                            docked_at_changed,
                            "owned ships refreshed"
                        );
                        true
                    }
                    Err(err) => {
                        warn!(%id, error = %err, "owned ship response could not be decoded");
                        false
                    }
                },
                Err(err) => {
                    warn!(%id, error = %err, "owned ship refresh failed");
                    false
                }
            }
        } else {
            false
        };
        let mut market_subscription_attempted = false;
        let mut market_subscription_ok = false;
        let mut market_subscription_owner = false;
        let mut market_book_items = 0usize;
        if projected.bot.location.docked_at.is_some() {
            // Market watchers are keyed by the POI id (the shared-knowledge
            // station key), while the client-side market cache is keyed by the
            // base id (`nearest_station` / `docked_at`).
            if let Some(poi_id) = projected.bot.location.poi_id.clone() {
                let base_station = projected.market_base_id.clone();
                market_subscription_owner =
                    self.market_subscription_owner_for_station(&poi_id).await == Some(id);
                let has_book = base_station
                    .as_deref()
                    .is_some_and(|base| account.market(base).is_some());
                if market_subscription_owner && !has_book {
                    market_subscription_attempted = true;
                    match account.subscribe_market().await {
                        Ok(snapshot) => {
                            market_subscription_ok = true;
                            info!(
                                id = %id,
                                account_id = account.id().unwrap_or("(none)"),
                                poi_id,
                                base_station = base_station.as_deref().unwrap_or("(none)"),
                                snapshot_base = snapshot
                                    .get("base_id")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("(none)"),
                                snapshot_items = snapshot
                                    .get("items")
                                    .and_then(|value| value.as_array())
                                    .map(Vec::len)
                                    .unwrap_or(0),
                                "owned account market subscription seeded"
                            );
                        }
                        Err(err) => {
                            warn!(
                                id = %id,
                                account_id = account.id().unwrap_or("(none)"),
                                poi_id,
                                base_station = base_station.as_deref().unwrap_or("(none)"),
                                error = %err,
                                "owned account market subscription failed"
                            );
                        }
                    }
                }
                if let Some(book) = base_station
                    .as_deref()
                    .and_then(|base| account.market(base))
                {
                    market_book_items = book.items.len();
                    crate::spacemolt_projection::project_market_book_from_client(
                        &mut projected,
                        &book,
                    );
                }
            }
        }
        if let Some(poi_id) = projected.bot.location.poi_id.clone() {
            let owns_observation =
                self.observation_subscription_owner_for_poi(&poi_id).await == Some(id);
            if owns_observation && !account.observation_subscribed() {
                match account.subscribe_observation(false).await {
                    Ok(_) => info!(
                        id = %id,
                        account_id = account.id().unwrap_or("(none)"),
                        poi_id,
                        "owned account observation subscription seeded"
                    ),
                    Err(err) => warn!(
                        id = %id,
                        account_id = account.id().unwrap_or("(none)"),
                        poi_id,
                        error = %err,
                        "owned account observation subscription failed"
                    ),
                }
            } else if !owns_observation && account.observation_subscribed() {
                if let Err(err) = account.unsubscribe_observation().await {
                    warn!(
                        id = %id,
                        account_id = account.id().unwrap_or("(none)"),
                        poi_id,
                        error = %err,
                        "non-owner account observation unsubscribe failed"
                    );
                }
            }
        } else if account.observation_subscribed() {
            if let Err(err) = account.unsubscribe_observation().await {
                warn!(id = %id, error = %err, "account observation unsubscribe failed");
            }
        }
        let account_observation = account.observation();
        if let Some(observation) = account_observation.as_ref() {
            crate::spacemolt_projection::project_observation_view_from_client(
                &mut projected,
                observation,
            );
        }
        let docked_storage_fetched = self
            .refresh_docked_personal_storage_from_account(id, &mut projected)
            .await;
        let docked_faction_storage_fetched = self
            .refresh_docked_faction_storage_from_account(id, &mut projected)
            .await;
        let commands = account.commands().spacemolt();
        let current_system = projected.bot.location.system_id.clone();
        let current_poi = projected.bot.location.poi_id.clone();
        let needs_poi_hydration = current_poi.as_deref().is_some_and(|poi_id| {
            !self
                .knowledge_state
                .read()
                .galaxy
                .poi_records
                .get(poi_id)
                .is_some_and(|poi| poi.info_complete && !poi.info.name.trim().is_empty())
        });
        if needs_poi_hydration {
            match commands.get_poi().await {
                Ok(response) => match response.into_value() {
                    Ok(value) => {
                        if let Some(galaxy) =
                            crate::spacemolt_projection::project_get_poi_json(&value)
                        {
                            self.ingest_observations(
                                [StateObservation {
                                    world: prayer_runtime::snapshot::WorldObservation {
                                        galaxy: Arc::new(galaxy),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }],
                                "canonical current-POI hydration",
                            );
                            info!(
                                id = %id,
                                poi_id = current_poi.as_deref().unwrap_or("(none)"),
                                "canonical current POI hydrated"
                            );
                        } else {
                            warn!(
                                id = %id,
                                poi_id = current_poi.as_deref().unwrap_or("(none)"),
                                "get_poi returned no projectable galaxy facts"
                            );
                        }
                    }
                    Err(err) => warn!(
                        id = %id,
                        poi_id = current_poi.as_deref().unwrap_or("(none)"),
                        error = %err,
                        "get_poi response could not be serialized for observation ingestion"
                    ),
                },
                Err(err) => warn!(
                    id = %id,
                    poi_id = current_poi.as_deref().unwrap_or("(none)"),
                    error = %err,
                    "canonical current-POI hydration failed"
                ),
            }
        }
        let needs_system_scan = current_system.as_deref().is_some_and(|system_id| {
            !self
                .knowledge_state
                .read()
                .galaxy
                .system_records
                .get(system_id)
                .is_some_and(|system| system.pois_complete)
        });
        if needs_system_scan {
            match commands.get_system().await {
                Ok(response) => match response.into_value() {
                    Ok(value) => {
                        if let Some(galaxy) = serde_json::from_value(value)
                            .ok()
                            .and_then(crate::spacemolt_projection::project_get_system_galaxy)
                        {
                            let poi_count = galaxy.poi_records.len();
                            self.ingest_observations(
                                [StateObservation {
                                    world: prayer_runtime::snapshot::WorldObservation {
                                        galaxy: Arc::new(galaxy),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }],
                                "canonical current-system hydration",
                            );
                            info!(
                                id = %id,
                                system_id = current_system.as_deref().unwrap_or("(none)"),
                                poi_count,
                                "canonical current-system POIs hydrated"
                            );
                        } else {
                            warn!(
                                id = %id,
                                system_id = current_system.as_deref().unwrap_or("(none)"),
                                "get_system returned no projectable galaxy facts"
                            );
                        }
                    }
                    Err(err) => warn!(
                        id = %id,
                        system_id = current_system.as_deref().unwrap_or("(none)"),
                        error = %err,
                        "get_system response could not be serialized for observation ingestion"
                    ),
                },
                Err(err) => warn!(
                    id = %id,
                    system_id = current_system.as_deref().unwrap_or("(none)"),
                    error = %err,
                    "canonical current-system hydration failed"
                ),
            }
        }
        let passengers_fetched = match commands.list_passengers().await {
            Ok(response) => match response.structured_content {
                Some(passengers) => {
                    crate::spacemolt_projection::project_aboard_passengers(
                        &mut projected,
                        passengers,
                    );
                    true
                }
                None => {
                    warn!(id = %id, "aboard passenger query returned no structured content");
                    false
                }
            },
            Err(err) => {
                warn!(id = %id, error = %err, "aboard passenger refresh failed");
                false
            }
        };
        let docked_passengers_fetched = if projected.bot.location.docked_at.is_some() {
            match commands.list_station_passengers().await {
                Ok(response) => match response.structured_content {
                    Some(passengers) => {
                        crate::spacemolt_projection::project_station_passengers(
                            &mut projected,
                            passengers,
                        );
                        true
                    }
                    None => {
                        warn!(id = %id, "station passenger query returned no structured content");
                        false
                    }
                },
                Err(err) => {
                    warn!(id = %id, error = %err, "station passenger refresh failed");
                    false
                }
            }
        } else {
            false
        };
        info!(
            id = %id,
            account_id = account.id().unwrap_or("(none)"),
            observation_system = account_observation
                .as_ref()
                .and_then(|observation| observation.system_id.as_deref())
                .unwrap_or("(none)"),
            observation_poi = account_observation
                .as_ref()
                .and_then(|observation| observation.poi_id.as_deref())
                .unwrap_or("(none)"),
            observation_nearby = account_observation
                .as_ref()
                .map(|observation| observation.nearby.len())
                .unwrap_or(0),
            projected_system = projected.bot.location.system_id.as_deref().unwrap_or("(none)"),
            projected_poi = projected.bot.location.poi_id.as_deref().unwrap_or("(none)"),
            projected_fuel = projected.bot.fuel,
            projected_max_fuel = projected.bot.max_fuel,
            projected_cargo_capacity = projected.bot.cargo_capacity,
            market_subscription_attempted,
            market_subscription_ok,
            market_subscription_owner,
            market_subscribed = account.market_subscribed(),
            market_book_items,
            projected_station_markets = projected.world.market.station_markets.len(),
            projected_current_buy_item_keys = projected.world.market.buy_orders.len(),
            projected_current_sell_item_keys = projected.world.market.sell_orders.len(),
            projected_current_buy_orders = projected
                .world
                .market
                .buy_orders
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            projected_current_sell_orders = projected
                .world
                .market
                .sell_orders
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            "owned account refresh projected"
        );
        let mut observation = projected.into_observation();
        observation.ships_fetched = ships_fetched;
        observation.docked_storage_fetched = docked_storage_fetched;
        observation.docked_faction_storage_fetched = docked_faction_storage_fetched;
        observation.passengers_fetched = passengers_fetched;
        observation.docked_passengers_fetched = docked_passengers_fetched;

        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let is_halted = session.engine.snapshot().is_halted;
        self.apply_refreshed_state(id, &mut session, observation, is_halted);
        let summary = Self::summary_from_session(id, &session);
        self.cache_session_summary(id, &summary);
        drop(session);
        self.bot_snapshot(id).await
    }
}
