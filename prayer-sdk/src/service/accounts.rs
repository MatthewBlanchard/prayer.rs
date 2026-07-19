//! SpaceMolt account construction, identity matching, and subscription ownership.

use super::*;

impl RuntimeService {
    pub fn wire_spacemolt_account_events(&self, id: Uuid, account: &Account) {
        let state_tx = self.account_state_tx.clone();
        account.on_state_change(move |_| {
            let _ = state_tx.send(id);
        });
        let market_tx = self.market_update_tx.clone();
        account.on("market_update", move |_| {
            let _ = market_tx.send(id);
        });
        let reconnect_tx = self.account_state_tx.clone();
        account.on_reconnected(move || {
            let _ = reconnect_tx.send(id);
        });
    }

    pub fn install_spacemolt_account(
        session: &mut SessionHandle,
        account: Account,
        selector: String,
        base_url: String,
    ) {
        let player_id = {
            let state = account.state();
            state
                .player()
                .ok()
                .flatten()
                .and_then(|player| player.id)
                .or_else(|| account.id().map(ToOwned::to_owned))
                .unwrap_or_else(|| selector.clone())
        };
        session.bot_id = Some(BotId::from(player_id));
        session.spacemolt_account = Some(account);
        session.spacemolt_account_selector = Some(selector);
        session.spacemolt_base_url = Some(base_url);
        session.push_status("Connected SpaceMolt account");
        session.last_updated_utc = Utc::now();
    }

    pub fn connected_account_identity(account: &Account) -> Option<(String, String, Vec<String>)> {
        let state = account.state();
        let player = state.player().ok().flatten().unwrap_or_default();
        let account_id = account
            .id()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let player_id = player.id;
        let username = player.username;
        let selector = account_id
            .clone()
            .or_else(|| player_id.clone())
            .or_else(|| username.clone())?;
        let label = username.clone().unwrap_or_else(|| selector.clone());
        let mut candidates = Vec::new();
        for candidate in [account_id, player_id, username] {
            if let Some(candidate) = candidate {
                if !candidates
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(&candidate))
                {
                    candidates.push(candidate);
                }
            }
        }
        Some((selector, label, candidates))
    }

    pub fn seed_state_identity_from_session(state: &mut BotState, session: &SessionHandle) {
        let (account_player_id, account_username) =
            if let Some(account) = session.spacemolt_account.as_ref() {
                let state = account.state();
                let player = state.player().ok().flatten().unwrap_or_default();
                (
                    player.id.or_else(|| {
                        account
                            .id()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned)
                    }),
                    player.username,
                )
            } else {
                (None, None)
            };

        if state
            .player
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            state.player.id =
                account_player_id.or_else(|| session.spacemolt_account_selector.clone());
        }
        if state
            .player
            .username
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            state.player.username = account_username.or_else(|| Some(session.label.clone()));
        }
    }

    pub fn session_id_for_spacemolt_identity(&self, candidates: &[String]) -> Option<Uuid> {
        let labels = self.session_labels.read();
        for candidate in candidates {
            if let Some(id) = labels.get(candidate) {
                return Some(*id);
            }
        }
        for (label, id) in labels.iter() {
            if candidates
                .iter()
                .any(|candidate| label.eq_ignore_ascii_case(candidate))
            {
                return Some(*id);
            }
        }
        None
    }

    pub async fn ensure_canonical_catalog_loaded(&self) -> Result<(), SdkError> {
        if self.canonical_catalog_loaded.load(Ordering::Acquire) {
            let knowledge = self.knowledge_state.read();
            if has_canonical_catalog(&knowledge.catalog) {
                return Ok(());
            }
        }

        let _guard = self.canonical_catalog_gate.lock().await;
        if self.canonical_catalog_loaded.load(Ordering::Acquire) {
            let knowledge = self.knowledge_state.read();
            if has_canonical_catalog(&knowledge.catalog) {
                return Ok(());
            }
        }

        let catalog = self
            .spacemolt_client
            .catalog(false)
            .await
            .map_err(SdkError::from)?;
        let catalog = canonical_catalog_from_cache(&catalog)?;
        info!(
            catalog_version = catalog.version.as_deref().unwrap_or("(none)"),
            items = catalog.items.len(),
            ships = catalog.ships.len(),
            recipes = catalog.recipes.len(),
            facilities = catalog.facilities.len(),
            skills = catalog.skills.len(),
            "canonical SpaceMolt catalog fetched"
        );
        let observation = StateObservation {
            catalog: Some(catalog),
            ..StateObservation::default()
        };
        self.ingest_observations([observation], "canonical catalog hydration");
        self.canonical_catalog_loaded.store(true, Ordering::Release);
        Ok(())
    }

    pub async fn ensure_canonical_map_loaded(&self) -> Result<(), SdkError> {
        if self.canonical_map_loaded.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self.canonical_map_gate.lock().await;
        if self.canonical_map_loaded.load(Ordering::Acquire) {
            return Ok(());
        }
        let account = self
            .spacemolt_client
            .accounts()
            .into_iter()
            .next()
            .ok_or_else(|| {
                SdkError::BadRequest(
                    "canonical map hydration requires a connected SpaceMolt account".to_string(),
                )
            })?;
        let response = account
            .commands()
            .spacemolt()
            .get_map(None)
            .await
            .map_err(SdkError::from)?
            .into_typed()?;
        let spacemolt_lib_rs::schema::GetMapCommandResponse::GetMapResponse(response) = response
        else {
            return Err(SdkError::InvalidRuntimeState(
                "full v2 get_map request returned a single-system response".to_string(),
            ));
        };
        let mut galaxy =
            crate::spacemolt_projection::project_get_map_galaxy(response).ok_or_else(|| {
                SdkError::InvalidRuntimeState(
                    "v2 get_map returned no canonical galaxy data".to_string(),
                )
            })?;
        // Canonical hydration uses get_map as static topology. The upstream
        // player-level `visited` flag is not Prayer exploration knowledge and
        // must not repopulate locally reset visit history.
        for system in galaxy.system_records.values_mut() {
            system.first_entered_unix = None;
            system.last_entered_unix = None;
        }
        self.ingest_observations(
            [StateObservation {
                world: prayer_runtime::snapshot::WorldObservation {
                    galaxy: Arc::new(galaxy),
                    ..Default::default()
                },
                map_fetched: true,
                ..Default::default()
            }],
            "canonical map hydration",
        );
        self.canonical_map_loaded.store(true, Ordering::Release);
        Ok(())
    }

    pub fn spawn_canonical_data_hydration(self: Arc<Self>) {
        let service = Arc::clone(&self);
        self.spawn_background(async move {
            if service.is_shutting_down() {
                return;
            }
            if let Err(err) = service.ensure_canonical_map_loaded().await {
                warn!(error = %err, "canonical map hydration failed after account connection");
            }
            if service.is_shutting_down() {
                return;
            }
            if let Err(err) = service.ensure_canonical_catalog_loaded().await {
                warn!(error = %err, "canonical catalog hydration failed after account connection");
            }
            info!("one-shot canonical map and catalog hydration finished");
        });
    }

    pub async fn spacemolt_account(&self, id: Uuid) -> Result<Account, SdkError> {
        let session = self.get_session(id).await?;
        let account = session.lock().await.spacemolt_account.clone();
        account
            .ok_or_else(|| SdkError::BadRequest("SpaceMolt account is not connected".to_string()))
    }

    pub fn faction_storage_refresh_target(
        &self,
        id: Uuid,
        state: &BotState,
    ) -> Option<(String, String)> {
        let station_id = state
            .location
            .docked_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?
            .to_string();
        let poi_id = state
            .location
            .poi_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let faction_id = state
            .player
            .faction_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?
            .to_string();
        // Watchers and shared knowledge are keyed by the canonical world POI,
        // while SpaceMolt's storage API expects the docked station/base id.
        let key = faction_station_storage_key(&faction_id, poi_id);
        if self
            .faction_storage_watchers_by_key
            .lock()
            .get(&key)
            .is_some_and(|watcher| *watcher != id)
        {
            return None;
        }
        if self
            .knowledge_metadata
            .read()
            .faction_storage_fetched_at_by_key
            .get(&key)
            .is_some_and(|fetched_at| fetched_at.elapsed() < IDLE_SESSION_REFRESH_INTERVAL)
        {
            return None;
        }
        Some((faction_id, station_id))
    }

    pub async fn refresh_docked_personal_storage_from_account(
        &self,
        id: Uuid,
        state: &mut crate::spacemolt_projection::ProjectedState,
    ) -> bool {
        let Some(station_id) = state.bot.location.docked_at.clone() else {
            return false;
        };
        let poi_id = state
            .bot
            .location
            .poi_id
            .clone()
            .unwrap_or_else(|| station_id.clone());
        let account = match self.spacemolt_account(id).await {
            Ok(account) => account,
            Err(_) => return false,
        };
        let response = match account
            .refresh_storage(spacemolt_lib_rs::StorageTarget::Personal, station_id)
            .await
        {
            Ok(response) => response,
            Err(err) => {
                debug!(%id, error = %err, "personal storage refresh unavailable");
                return false;
            }
        };
        let spacemolt_lib_rs::schema::StorageResponse::ViewStorageResponse(
            spacemolt_lib_rs::schema::ViewStorageResponse { items, .. },
        ) = response
        else {
            return false;
        };
        Arc::make_mut(&mut state.world.storage).insert(
            poi_id,
            items
                .into_iter()
                .filter(|item| item.quantity > 0)
                .map(|item| (item.item_id, item.quantity))
                .collect(),
        );
        true
    }

    pub async fn refresh_docked_faction_storage_from_account(
        &self,
        id: Uuid,
        state: &mut crate::spacemolt_projection::ProjectedState,
    ) -> bool {
        let Some((faction_id, station_id)) = self.faction_storage_refresh_target(id, &state.bot)
        else {
            return false;
        };
        let result = match self.spacemolt_account(id).await {
            Ok(account) => account
                .refresh_storage(spacemolt_lib_rs::StorageTarget::Faction, station_id.clone())
                .await
                .map_err(SdkError::from),
            Err(err) => Err(err),
        };
        let response = match result {
            Ok(value) => value,
            Err(err) => {
                debug!(
                    %id,
                    faction_id,
                    station_id,
                    error = %err,
                    "faction storage refresh unavailable"
                );
                return false;
            }
        };

        if let spacemolt_lib_rs::schema::StorageResponse::ViewFactionStorageResponse(
            spacemolt_lib_rs::schema::ViewFactionStorageResponse { faction_id, .. },
        ) = &response
        {
            state.bot.player.faction_id = Some(faction_id.clone());
        }
        state.world.faction_storage = Arc::new(faction_storage_items_from_view_response(&response));
        info!(
            %id,
            faction_id = state.bot.player.faction_id.as_deref().unwrap_or(faction_id.as_str()),
            station_id,
            item_stacks = state.world.faction_storage.len(),
            total_quantity = state.world.faction_storage.values().sum::<i64>(),
            "faction storage refresh projected"
        );
        // The designated faction-storage watcher also owns the shared treasury
        // balance, so the storage tab never has to fetch it on the request path.
        let treasury_faction_id = state
            .bot
            .player
            .faction_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(faction_id.as_str())
            .to_string();
        self.refresh_watched_faction_treasury(id, &treasury_faction_id)
            .await;
        true
    }

    /// Resolve the single session that owns the live market subscription for a
    /// station, identified by its POI id (`current_poi`) — the same key shared
    /// market snapshots and watchers use.
    pub async fn market_subscription_owner_for_station(&self, station_id: &str) -> Option<Uuid> {
        let station_id = station_id.trim();
        if station_id.is_empty() {
            return None;
        }

        let sessions: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, Arc::clone(session)))
            .collect();
        let mut candidates = Vec::new();
        for (id, session) in sessions {
            let account = {
                let session = session.lock().await;
                session.spacemolt_account.clone()
            };
            let Some(account) = account else {
                continue;
            };
            let projected = crate::spacemolt_projection::project_account_state(&account.state());
            if projected.bot.location.docked_at.is_some()
                && projected.bot.location.poi_id.as_deref() == Some(station_id)
            {
                candidates.push(id);
            }
        }
        candidates.sort();

        let mut watchers = self.market_watchers.lock();
        if let Some(existing) = watchers
            .get(station_id)
            .copied()
            .filter(|existing| candidates.contains(existing))
        {
            return Some(existing);
        }
        if let Some(owner) = candidates.first().copied() {
            watchers.insert(station_id.to_string(), owner);
            Some(owner)
        } else {
            watchers.remove(station_id);
            None
        }
    }

    /// Resolve the single session that owns nearby-player observation for a POI.
    pub async fn observation_subscription_owner_for_poi(&self, poi_id: &str) -> Option<Uuid> {
        let poi_id = poi_id.trim();
        if poi_id.is_empty() {
            return None;
        }

        let sessions: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, Arc::clone(session)))
            .collect();
        let mut candidates = Vec::new();
        for (id, session) in sessions {
            let account = {
                let session = session.lock().await;
                session.spacemolt_account.clone()
            };
            let Some(account) = account else {
                continue;
            };
            let projected = crate::spacemolt_projection::project_account_state(&account.state());
            if projected.bot.location.poi_id.as_deref() == Some(poi_id) {
                candidates.push(id);
            }
        }
        candidates.sort();

        let mut watchers = self.observation_watchers_by_poi.lock();
        if let Some(existing) = watchers
            .get(poi_id)
            .copied()
            .filter(|existing| candidates.contains(existing))
        {
            return Some(existing);
        }
        if let Some(owner) = candidates.first().copied() {
            watchers.insert(poi_id.to_string(), owner);
            Some(owner)
        } else {
            watchers.remove(poi_id);
            None
        }
    }
}
