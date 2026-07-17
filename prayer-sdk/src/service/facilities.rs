//! Projected state and facility snapshot queries, caching, and invalidation.

use super::*;

impl RuntimeService {
    /// Clone the current shared galaxy projection without performing I/O.
    pub async fn cached_galaxy_snapshot(&self) -> Arc<GalaxyData> {
        Arc::clone(&self.knowledge_state.read().galaxy)
    }

    pub async fn refresh_state(&self, id: Uuid) -> Result<prayer_state::FleetEntry, SdkError> {
        self.refresh_state_for_host_loop(id, true).await
    }

    pub async fn shared_catalog_snapshot(&self) -> Arc<CatalogData> {
        Arc::clone(&self.knowledge_state.read().catalog)
    }

    pub async fn economy_read_snapshot(
        &self,
        id: Uuid,
    ) -> Result<Option<prayer_runtime::economy::EconomyReadState>, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        if !session.has_state {
            return Ok(None);
        }
        let actor = &session.actor.observed;
        let knowledge = self.knowledge_state.read();
        let market_station_summaries = knowledge
            .station_markets
            .iter()
            .map(|(station_id, snapshot)| {
                let buy_orders = snapshot.buy_orders.values().map(Vec::len).sum::<usize>();
                let sell_orders = snapshot.sell_orders.values().map(Vec::len).sum::<usize>();
                format!(
                    "{station_id}:tick={:?}:observed_at={:?}:buy_items={}:sell_items={}:buy_orders={buy_orders}:sell_orders={sell_orders}",
                    snapshot.current_tick,
                    snapshot.observed_at_unix,
                    snapshot.buy_orders.len(),
                    snapshot.sell_orders.len(),
                )
            })
            .collect::<BTreeSet<_>>();
        info!(
            session_id = %id,
            actor = actor.player.username.as_deref().unwrap_or("(unknown)"),
            station_count = knowledge.station_markets.len(),
            stations = ?market_station_summaries,
            knowledge_version = knowledge.knowledge_version,
            "market lineage building economy read snapshot"
        );
        let mut market = MarketData {
            shipyard_listings: knowledge.shipyard_listing_ids.clone(),
            station_markets: knowledge.station_markets.clone(),
            ..MarketData::default()
        };
        overlay_virtual_market_orders_for_actor(&knowledge, actor, &mut market);
        let faction_storage = actor
            .player
            .faction_id
            .as_deref()
            .and_then(|faction| knowledge.faction_storage_by_faction_poi.get(faction))
            .and_then(|by_poi| {
                actor
                    .location
                    .poi_id
                    .as_deref()
                    .and_then(|poi| by_poi.get(poi))
            })
            .cloned()
            .unwrap_or_default();
        Ok(Some(prayer_runtime::economy::EconomyReadState {
            system: actor.location.system_id.clone(),
            current_poi: actor.location.poi_id.clone(),
            cargo_used: actor.cargo_used,
            cargo_capacity: actor.cargo_capacity,
            credits: actor.player.credits.unwrap_or_default(),
            catalog: Arc::clone(&knowledge.catalog),
            galaxy: Arc::clone(&knowledge.galaxy),
            market: Arc::new(market),
            faction_storage: Arc::new(faction_storage),
            passengers: actor.passengers.to_passenger_state(),
            username: actor.player.username.clone(),
            player_id: actor.player.id.clone(),
            faction_id: actor.player.faction_id.clone(),
            clan_tag: actor.player.clan_tag.clone(),
            active_commissions: Arc::clone(&actor.active_commissions),
            crafting_queue: Arc::clone(&actor.crafting_queue),
        }))
    }

    pub async fn station_context_snapshot(
        &self,
        id: Uuid,
    ) -> Result<Option<RuntimeStationContextDto>, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        if !session.has_state {
            return Ok(None);
        }
        let knowledge = self.knowledge_state.read();
        let market = session
            .actor
            .observed
            .location
            .poi_id
            .as_deref()
            .and_then(|poi| knowledge.station_markets.get(poi));
        Ok(map_focused_station_context(
            &session.actor.observed,
            &knowledge.catalog,
            &knowledge.galaxy,
            market,
        ))
    }

    pub async fn station_storage_snapshot(
        &self,
        id: Uuid,
    ) -> Result<Option<StationStorageResponse>, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        let actor = &session.actor.observed;
        if !session.has_state || !actor.location.docked_at.is_some() {
            return Ok(None);
        }
        let Some(station_id) = actor.location.poi_id.as_deref() else {
            return Ok(None);
        };
        let knowledge = self.knowledge_state.read();
        let base_id = knowledge.galaxy.base_id_for_poi(station_id);
        let storage = [actor.player.id.as_deref(), actor.player.username.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .find_map(|key| knowledge.storage_by_player.get(key))
            .and_then(|by_station| {
                by_station
                    .get(station_id)
                    .or_else(|| base_id.and_then(|base| by_station.get(base)))
            });
        Ok(Some(StationStorageResponse {
            storage_credits: 0,
            storage_items: storage.map(map_item_quantities).unwrap_or_default(),
        }))
    }

    pub async fn station_shipyard_snapshot(
        &self,
        id: Uuid,
    ) -> Result<Option<StationShipyardResponse>, SdkError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        let actor = &session.actor.observed;
        if !session.has_state || !actor.location.docked_at.is_some() {
            return Ok(None);
        }
        let knowledge = self.knowledge_state.read();
        let market = actor
            .location
            .poi_id
            .as_deref()
            .and_then(|poi| knowledge.station_markets.get(poi));
        let Some(station) =
            map_focused_station_context(actor, &knowledge.catalog, &knowledge.galaxy, market)
        else {
            return Ok(None);
        };
        let garage = [
            actor.player.faction_id.as_deref(),
            actor.player.clan_tag.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .find_map(|key| knowledge.faction_garage_by_faction.get(key))
        .cloned()
        .unwrap_or_default();
        Ok(Some(StationShipyardResponse {
            owned_ships: map_actor_owned_ships(actor),
            installed_modules: actor.installed_modules.as_ref().clone(),
            faction_garage: map_faction_garage_value(&garage),
            shipyard_showroom: station.shipyard_showroom,
            shipyard_listings: station.shipyard_listings,
            in_progress_commissions: map_actor_active_commissions(actor),
        }))
    }

    pub async fn craft_planning_snapshot(
        &self,
        id: Uuid,
    ) -> Result<
        Option<(
            Arc<CatalogData>,
            HashMap<String, HashMap<String, i64>>,
            Arc<HashMap<String, i64>>,
        )>,
        SdkError,
    > {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        if !session.has_state {
            return Ok(None);
        }
        let actor = &session.actor.observed;
        let knowledge = self.knowledge_state.read();
        let storage = [actor.player.id.as_deref(), actor.player.username.as_deref()]
            .into_iter()
            .flatten()
            .find_map(|key| knowledge.storage_by_player.get(key))
            .cloned()
            .unwrap_or_default();
        let faction_storage = actor
            .player
            .faction_id
            .as_deref()
            .and_then(|faction| knowledge.faction_storage_by_faction_poi.get(faction))
            .and_then(|by_poi| {
                actor
                    .location
                    .poi_id
                    .as_deref()
                    .and_then(|poi| by_poi.get(poi))
            })
            .cloned()
            .unwrap_or_default();
        Ok(Some((
            Arc::clone(&knowledge.catalog),
            storage,
            Arc::new(faction_storage),
        )))
    }

    pub async fn facilities_snapshot_response(
        &self,
        id: Uuid,
    ) -> Result<FacilitiesSnapshotResponse, SdkError> {
        let (
            has_state,
            state_version,
            username,
            player_id,
            player_faction_id,
            latest_system,
            latest_poi,
            docked,
        ) = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            let actor = ActorLens::new(&session.actor.observed);
            let state = actor.state();
            (
                session.has_state,
                session.has_state.then_some(session.state_version),
                state.player.username.clone(),
                state.player.id.clone(),
                state.player.faction_id.clone(),
                state.location.system_id.clone(),
                state.location.poi_id.clone(),
                session
                    .has_state
                    .then_some(state.location.docked_at.is_some()),
            )
        };
        let now_unix = Utc::now().timestamp();
        let cached = latest_poi.as_deref().and_then(|poi| {
            self.knowledge_state
                .read()
                .facilities_by_poi
                .get(poi)
                .filter(|snapshot| facility_snapshot_fresh(snapshot, now_unix))
                .cloned()
        });

        let (current, faction_current, current_cached, current_observed_at_unix) =
            if let Some(snapshot) = cached {
                (
                    snapshot.current,
                    snapshot.faction_current,
                    true,
                    Some(snapshot.observed_at_unix),
                )
            } else {
                let current = self.facility_query(id, "list").await.ok();
                let faction_current = self.facility_query(id, "faction_list").await.ok();
                if let Some(poi) = latest_poi.as_deref() {
                    if current.is_some() && faction_current.is_some() {
                        self.remember_facility_snapshot(
                            poi,
                            PoiFacilitiesSnapshot {
                                observed_at_unix: now_unix,
                                current: current.clone(),
                                faction_current: faction_current.clone(),
                            },
                        );
                    }
                }
                (current, faction_current, false, Some(now_unix))
            };

        let mut errors = Vec::new();
        let owned = self
            .facility_query(id, "owned")
            .await
            .map_err(|error| {
                errors.push(format!("owned: {error}"));
                error
            })
            .ok();
        let faction_owned = self
            .facility_query(id, "faction_owned")
            .await
            .map_err(|error| {
                errors.push(format!("factionOwned: {error}"));
                error
            })
            .ok();
        let (faction_id, faction_rent_per_cycle, faction_arrears_owed) =
            match faction_owned.as_ref() {
                Some(spacemolt_lib_rs::schema::FacilityResponse::FacilityFactionOwnedResponse(
                    spacemolt_lib_rs::schema::FacilityFactionOwnedResponse {
                        faction_id,
                        total_rent_per_cycle,
                        arrears_owed,
                        ..
                    },
                )) => (
                    Some(faction_id.clone()),
                    Some(*total_rent_per_cycle),
                    *arrears_owed,
                ),
                _ => (None, None, None),
            };
        self.remember_owned_facility_snapshots(
            player_id.as_deref(),
            owned.as_ref(),
            faction_id.as_deref().or(player_faction_id.as_deref()),
            faction_owned.as_ref(),
        );
        let types = {
            let knowledge = self.knowledge_state.read();
            let world = WorldLens::new(&knowledge);
            facility_types_from_catalog(world.catalog())
        };

        Ok(FacilitiesSnapshotResponse {
            session_id: id.to_string(),
            state_version,
            username: has_state.then_some(username).flatten(),
            latest_system: has_state.then_some(latest_system).flatten(),
            latest_poi,
            docked,
            current_cached,
            current_observed_at_unix,
            current,
            owned,
            faction_current,
            faction_owned,
            faction_id,
            faction_rent_per_cycle,
            faction_arrears_owed,
            types,
            errors,
        })
    }

    fn remember_owned_facility_snapshots(
        &self,
        player_id: Option<&str>,
        owned: Option<&spacemolt_lib_rs::schema::FacilityResponse>,
        faction_id: Option<&str>,
        faction_owned: Option<&spacemolt_lib_rs::schema::FacilityResponse>,
    ) {
        let mut knowledge = self.knowledge_state.write();
        let mut changed = false;
        if let (Some(id), Some(response)) = (player_id.filter(|id| !id.is_empty()), owned) {
            if knowledge.owned_facilities_by_player.get(id) != Some(response) {
                knowledge
                    .owned_facilities_by_player
                    .insert(id.to_string(), response.clone());
                changed = true;
            }
        }
        if let (Some(id), Some(response)) = (faction_id.filter(|id| !id.is_empty()), faction_owned)
        {
            if knowledge.owned_facilities_by_faction.get(id) != Some(response) {
                knowledge
                    .owned_facilities_by_faction
                    .insert(id.to_string(), response.clone());
                changed = true;
            }
        }
        if !changed {
            return;
        }
        knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
        let snapshot = knowledge.clone();
        drop(knowledge);
        self.knowledge_persistence
            .publish(snapshot, "after owned facility snapshot");
    }

    pub async fn facility_query(
        &self,
        id: Uuid,
        action: &str,
    ) -> Result<spacemolt_lib_rs::schema::FacilityResponse, SdkError> {
        let account = self.spacemolt_account(id).await?;
        let commands = account.commands().spacemolt_facility();
        let result: spacemolt_lib_rs::schema::FacilityResponse = match action {
            "list" => commands
                .list()
                .await
                .map_err(SdkError::from)?
                .into_typed()
                .map_err(SdkError::from)?
                .into(),
            "faction_list" => commands
                .faction_list()
                .await
                .map_err(SdkError::from)?
                .into_typed()
                .map_err(SdkError::from)?
                .into(),
            "owned" => commands
                .owned()
                .await
                .map_err(SdkError::from)?
                .into_typed()
                .map_err(SdkError::from)?
                .into(),
            "faction_owned" => commands
                .faction_owned()
                .await
                .map_err(SdkError::from)?
                .into_typed()
                .map_err(SdkError::from)?
                .into(),
            _ => {
                return Err(SdkError::BadRequest(format!(
                    "unsupported facility query '{action}'"
                )))
            }
        };
        warn!(%id, action = %action, "facilities upstream response decoded as generated type");
        Ok(result)
    }

    pub fn remember_facility_snapshot(&self, poi: &str, snapshot: PoiFacilitiesSnapshot) {
        let knowledge = {
            let mut knowledge = self.knowledge_state.write();
            let before = knowledge.facilities_by_poi.get(poi).cloned();
            if before.as_ref() == Some(&snapshot) {
                return;
            }
            knowledge
                .facilities_by_poi
                .insert(poi.to_string(), snapshot.clone());
            Arc::make_mut(&mut knowledge.galaxy)
                .facilities_by_poi
                .insert(poi.to_string(), snapshot);
            knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            knowledge.clone()
        };
        self.knowledge_persistence
            .publish(knowledge, "after facility snapshot");
    }

    pub fn invalidate_facility_snapshot_for_poi(&self, poi: Option<&str>) {
        let Some(poi) = poi.filter(|poi| !poi.trim().is_empty()) else {
            return;
        };
        let knowledge = {
            let mut knowledge = self.knowledge_state.write();
            if knowledge.facilities_by_poi.remove(poi).is_none() {
                return;
            }
            Arc::make_mut(&mut knowledge.galaxy)
                .facilities_by_poi
                .remove(poi);
            knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            knowledge.clone()
        };
        self.knowledge_persistence
            .publish(knowledge, "after facility invalidation");
    }

    pub fn invalidate_owned_facility_snapshots(
        &self,
        player_id: Option<&str>,
        faction_id: Option<&str>,
    ) {
        let snapshot = {
            let mut knowledge = self.knowledge_state.write();
            let player_removed = player_id
                .is_some_and(|id| knowledge.owned_facilities_by_player.remove(id).is_some());
            let faction_removed = faction_id
                .is_some_and(|id| knowledge.owned_facilities_by_faction.remove(id).is_some());
            if !player_removed && !faction_removed {
                return;
            }
            knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            knowledge.clone()
        };
        self.knowledge_persistence
            .publish(snapshot, "after owned facility invalidation");
    }

    pub fn invalidate_station_passengers_for_poi(&self, poi: Option<&str>) {
        let Some(poi) = poi.map(str::trim).filter(|poi| !poi.is_empty()) else {
            return;
        };
        let snapshot = {
            let mut knowledge = self.knowledge_state.write();
            let removed_board = knowledge.station_passengers.remove(poi).is_some();
            let removed_freshness = self
                .knowledge_metadata
                .write()
                .station_passengers_fetched_at_by_station
                .remove(poi)
                .is_some();
            if removed_board || removed_freshness {
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
                Some(knowledge.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = snapshot {
            self.knowledge_persistence
                .publish(snapshot, "after station passenger invalidation");
        }
    }
}
