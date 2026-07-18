//! Shared galaxy, commander, fleet, tax, and storage projections.

use super::super::*;
use crate::state_mapping::map_shared_runtime_world_state;

impl RuntimeService {
    pub async fn commander_roster_snapshot(&self) -> Value {
        let roster_version = self.roster_sequence.load(Ordering::Acquire);
        let state_version = self.commander_state_sequence.load(Ordering::Acquire);
        let entries = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, Arc::clone(session)))
            .collect::<Vec<_>>();
        let mut sessions = Vec::with_capacity(entries.len());
        for (id, session) in entries {
            let session = session.lock().await;
            sessions.push(serde_json::json!({
                "id": id,
                "playerName": session.label,
                "stateVersion": session.state_version,
                "connected": session.spacemolt_account.is_some(),
            }));
        }
        sessions.sort_by(|a, b| a["playerName"].as_str().cmp(&b["playerName"].as_str()));
        serde_json::json!({
            "rosterVersion": roster_version,
            "stateVersion": state_version,
            "sessions": sessions,
        })
    }

    pub async fn commander_knowledge_snapshot(&self) -> Result<Value, SdkError> {
        let knowledge = self.knowledge_state.read();
        let galaxy = map_shared_runtime_world_state(
            &knowledge.catalog,
            &knowledge.galaxy,
            &knowledge.wildlife_by_poi,
        )?;
        let mut sightings = knowledge
            .agent_sightings
            .values()
            .cloned()
            .collect::<Vec<_>>();
        sightings.sort_by(|a, b| {
            b.last_seen_unix
                .cmp(&a.last_seen_unix)
                .then_with(|| a.contact.username.cmp(&b.contact.username))
        });
        let social = map_social_bots(sightings);
        Ok(serde_json::json!({
            "knowledgeVersion": knowledge.knowledge_version,
            "world": { "galaxy": galaxy },
            "social": social,
        }))
    }

    pub async fn commander_session_state_delta(&self, since: u64) -> Result<Value, SdkError> {
        let state_version = self.commander_state_sequence.load(Ordering::Acquire);
        let changed_ids = self
            .session_change_sequences
            .lock()
            .iter()
            .filter_map(|(id, sequence)| (*sequence > since).then_some(*id))
            .collect::<HashSet<_>>();
        let entries = self
            .sessions
            .read()
            .iter()
            .filter(|(id, _)| since == 0 || changed_ids.contains(id))
            .map(|(id, session)| (*id, Arc::clone(session)))
            .collect::<Vec<_>>();
        let mut local_sessions = Vec::with_capacity(entries.len());
        for (id, session) in entries {
            let session = session.lock().await;
            let snapshot = session.engine.snapshot();
            let memory = snapshot
                .memory
                .iter()
                .map(|memory| {
                    let action = if memory.args.is_empty() {
                        memory.action.clone()
                    } else {
                        format!("{} {}", memory.action, memory.args.join(" "))
                    };
                    memory
                        .result_message
                        .as_ref()
                        .filter(|message| !message.is_empty())
                        .map_or(action.clone(), |message| format!("{action} -> {message}"))
                })
                .collect::<Vec<_>>();
            local_sessions.push((
                id,
                session.label.clone(),
                session.state_version,
                session.has_state.then(|| session.actor.observed.clone()),
                memory,
                session.execution_status_lines(),
            ));
        }
        let knowledge = self.knowledge_state.read();
        let knowledge_version = knowledge.knowledge_version;
        let mut sessions = Vec::with_capacity(local_sessions.len());
        for (id, label, session_state_version, live_state, memory, execution_status_lines) in
            local_sessions
        {
            let state = live_state
                .map(|live| {
                    let mut world = world_read_state_with_metadata(
                        &knowledge,
                        &self.knowledge_metadata.read(),
                        &live,
                    );
                    self.inventory_reservations
                        .lock()
                        .apply_market_reservations(Arc::make_mut(&mut world.market));
                    map_commander_session_state(&live, &world)
                })
                .transpose()?;
            sessions.push(serde_json::json!({
                "id": id,
                "playerName": label,
                "stateVersion": session_state_version,
                "knowledgeVersion": knowledge_version,
                "state": state,
                "memory": memory,
                "executionStatusLines": execution_status_lines,
            }));
        }
        sessions.sort_by(|a, b| a["playerName"].as_str().cmp(&b["playerName"].as_str()));
        let removed = self
            .session_tombstones
            .lock()
            .iter()
            .filter_map(|(sequence, handle)| (*sequence > since).then_some(handle.clone()))
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "baseline": since == 0,
            "stateVersion": state_version,
            "knowledgeVersion": knowledge_version,
            "sessions": sessions,
            "removedSessionHandles": removed,
        }))
    }

    pub async fn commander_state_snapshot(&self) -> Result<CommanderStateResponse, SdkError> {
        let started = Instant::now();
        let knowledge_started = Instant::now();
        let knowledge = self.knowledge_state.read().clone();
        let knowledge_read_ms = knowledge_started.elapsed().as_millis();
        let knowledge_counts = WorldKnowledgeCounts::from_knowledge(&knowledge);
        let memory_size_breakdown = self.options.memory_size_breakdown;
        let entries_started = Instant::now();
        let entries: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, session.clone()))
            .collect();
        let entries_collect_ms = entries_started.elapsed().as_millis();

        let mut sessions = Vec::with_capacity(entries.len());
        let sessions_total = entries.len();
        let mut sessions_observed = 0usize;
        let mut sessions_mapped = 0usize;
        let mut sessions_omitted = 0usize;
        let mut state_version = 0;
        let mut world = None;
        let mut projected_waiting_total = 0usize;
        let mut projected_aboard_total = 0usize;
        let known_station_passenger_boards = knowledge.station_passengers.len();
        let known_station_passenger_waiting_total = knowledge
            .station_passengers
            .values()
            .map(|board| board.waiting.len())
            .sum::<usize>();
        let mut session_lock_ms = 0u128;
        let mut engine_snapshot_ms = 0u128;
        let mut memory_map_ms = 0u128;
        let mut compose_state_ms = 0u128;
        let mut map_world_ms = 0u128;
        let mut map_session_ms = 0u128;
        let mut status_lines_ms = 0u128;
        let mut session_live_state_bytes = 0usize;
        let mut session_effective_state_bytes = 0usize;
        let mut session_engine_snapshot_bytes = 0usize;
        let mut session_memory_line_bytes = 0usize;
        let mut session_status_line_bytes = 0usize;
        let mut session_state_probe_ms = 0u128;
        for (_id, session) in entries {
            let lock_started = Instant::now();
            let session = session.lock().await;
            session_lock_ms = session_lock_ms.saturating_add(lock_started.elapsed().as_millis());
            state_version = state_version.max(session.state_version);
            let engine_snapshot_started = Instant::now();
            let snapshot = session.engine.snapshot();
            engine_snapshot_ms =
                engine_snapshot_ms.saturating_add(engine_snapshot_started.elapsed().as_millis());
            let memory_map_started = Instant::now();
            let memory = snapshot
                .memory
                .iter()
                .map(|m| {
                    let action = if m.args.is_empty() {
                        m.action.clone()
                    } else {
                        format!("{} {}", m.action, m.args.join(" "))
                    };
                    match &m.result_message {
                        Some(msg) if !msg.is_empty() => format!("{action} -> {msg}"),
                        _ => action,
                    }
                })
                .collect();
            memory_map_ms = memory_map_ms.saturating_add(memory_map_started.elapsed().as_millis());
            let state = if session.has_state {
                sessions_observed += 1;
                if memory_size_breakdown {
                    let session_probe_started = Instant::now();
                    session_live_state_bytes = session_live_state_bytes
                        .saturating_add(serialized_len(&session.actor.observed));
                    // Kept in the diagnostics schema for compatibility; the
                    // session no longer retains an effective projection.
                    session_effective_state_bytes = session_effective_state_bytes.saturating_add(0);
                    session_engine_snapshot_bytes =
                        session_engine_snapshot_bytes.saturating_add(serialized_len(&snapshot));
                    session_memory_line_bytes =
                        session_memory_line_bytes.saturating_add(serialized_len(&memory));
                    session_status_line_bytes = session_status_line_bytes
                        .saturating_add(serialized_len(&session.status_lines));
                    session_state_probe_ms = session_state_probe_ms
                        .saturating_add(session_probe_started.elapsed().as_millis());
                }
                let compose_started = Instant::now();
                let mut world_read = world_read_state_with_metadata(
                    &knowledge,
                    &self.knowledge_metadata.read(),
                    &session.actor.observed,
                );
                self.inventory_reservations
                    .lock()
                    .apply_market_reservations(Arc::make_mut(&mut world_read.market));
                compose_state_ms =
                    compose_state_ms.saturating_add(compose_started.elapsed().as_millis());
                projected_waiting_total = projected_waiting_total
                    .saturating_add(world_read.station_passengers.waiting.len());
                projected_aboard_total = projected_aboard_total
                    .saturating_add(session.actor.observed.passengers.aboard.len());
                let map_session_started = Instant::now();
                let mapped = match map_commander_session_state(&session.actor.observed, &world_read)
                {
                    Ok(mapped) => {
                        sessions_mapped += 1;
                        if world.is_none() {
                            let map_world_started = Instant::now();
                            let galaxy = map_shared_runtime_world_state(
                                &knowledge.catalog,
                                &knowledge.galaxy,
                                &knowledge.wildlife_by_poi,
                            )?;
                            world = Some(CommanderWorldStateResponse { galaxy });
                            map_world_ms = map_world_ms
                                .saturating_add(map_world_started.elapsed().as_millis());
                        }
                        Some(mapped)
                    }
                    Err(err) => {
                        sessions_omitted += 1;
                        warn!(
                            session = %session.label,
                            state_version = session.state_version,
                            has_spacemolt_account = session.spacemolt_account.is_some(),
                            live_system = session.actor.observed.location.system_id.as_deref().unwrap_or("(none)"),
                            live_poi = session.actor.observed.location.poi_id.as_deref().unwrap_or("(none)"),
                            effective_system = session.actor.observed.location.system_id.as_deref().unwrap_or("(none)"),
                            effective_poi = session.actor.observed.location.poi_id.as_deref().unwrap_or("(none)"),
                            projected_system = session.actor.observed.location.system_id.as_deref().unwrap_or("(none)"),
                            projected_poi = session.actor.observed.location.poi_id.as_deref().unwrap_or("(none)"),
                            error = %err,
                            "commander session state omitted"
                        );
                        None
                    }
                };
                map_session_ms =
                    map_session_ms.saturating_add(map_session_started.elapsed().as_millis());
                mapped
            } else {
                None
            };
            let status_lines_started = Instant::now();
            let execution_status_lines = session.execution_status_lines();
            status_lines_ms =
                status_lines_ms.saturating_add(status_lines_started.elapsed().as_millis());
            sessions.push(CommanderSessionStateResponse {
                player_name: session.label.clone(),
                state_version: session.state_version,
                knowledge_version: knowledge.knowledge_version,
                state,
                memory,
                execution_status_lines,
            });
        }

        sessions.sort_by(|a, b| a.player_name.cmp(&b.player_name));
        let social_started = Instant::now();
        let social = map_social_bots(self.agent_sightings_snapshot());
        let social_map_ms = social_started.elapsed().as_millis();
        let elapsed_ms = started.elapsed().as_millis();
        info!(
            sessions_total,
            sessions_observed,
            sessions_mapped,
            sessions_omitted,
            state_version,
            knowledge_version = knowledge.knowledge_version,
            knowledge_read_ms,
            entries_collect_ms,
            session_lock_ms,
            engine_snapshot_ms,
            memory_map_ms,
            compose_state_ms,
            map_world_ms,
            map_session_ms,
            status_lines_ms,
            social_map_ms,
            known_station_passenger_boards,
            known_station_passenger_waiting_total,
            projected_waiting_total,
            projected_aboard_total,
            galaxy_systems = knowledge_counts.galaxy_systems,
            galaxy_pois = knowledge_counts.galaxy_pois,
            galaxy_items = knowledge_counts.galaxy_items,
            galaxy_ships = knowledge_counts.galaxy_ships,
            galaxy_recipes = knowledge_counts.galaxy_recipes,
            galaxy_facilities = knowledge_counts.galaxy_facilities,
            galaxy_skills = knowledge_counts.galaxy_skills,
            galaxy_system_connections = knowledge_counts.galaxy_system_connections,
            galaxy_poi_resources = knowledge_counts.galaxy_poi_resources,
            known_station_markets = knowledge_counts.known_station_markets,
            station_market_sell_item_keys = knowledge_counts.station_market_sell_item_keys,
            station_market_sell_orders = knowledge_counts.station_market_sell_orders,
            station_market_buy_item_keys = knowledge_counts.station_market_buy_item_keys,
            station_market_buy_orders = knowledge_counts.station_market_buy_orders,
            station_passenger_boards = knowledge_counts.station_passenger_boards,
            station_passenger_waiting = knowledge_counts.station_passenger_waiting,
            station_passenger_aboard = knowledge_counts.station_passenger_aboard,
            salvage_pois = knowledge_counts.salvage_pois,
            salvage_lootables = knowledge_counts.salvage_lootables,
            storage_players = knowledge_counts.storage_players,
            storage_poi_buckets = knowledge_counts.storage_poi_buckets,
            storage_item_stacks = knowledge_counts.storage_item_stacks,
            faction_storage_factions = knowledge_counts.faction_storage_factions,
            faction_storage_poi_buckets = knowledge_counts.faction_storage_poi_buckets,
            faction_storage_item_stacks = knowledge_counts.faction_storage_item_stacks,
            faction_garages = knowledge_counts.faction_garages,
            faction_garage_ships = knowledge_counts.faction_garage_ships,
            virtual_orders = knowledge_counts.virtual_orders,
            virtual_craft_orders = knowledge_counts.virtual_craft_orders,
            facilities_pois = knowledge_counts.facilities_pois,
            agent_sightings = knowledge_counts.agent_sightings,
            system_agent_systems = knowledge_counts.system_agent_systems,
            system_agent_sightings = knowledge_counts.system_agent_sightings,
            wildlife_pois = knowledge_counts.wildlife_pois,
            wildlife_creatures = knowledge_counts.wildlife_creatures,
            elapsed_ms,
            "commander state snapshot built"
        );
        if memory_size_breakdown {
            let breakdown_started = Instant::now();
            let knowledge_bytes = WorldKnowledgeByteBreakdown::from_knowledge(&knowledge);
            info!(
                sessions_total,
                sessions_observed,
                state_version,
                knowledge_version = knowledge.knowledge_version,
                catalog_bytes = knowledge_bytes.catalog_bytes,
                galaxy_bytes = knowledge_bytes.galaxy_bytes,
                shipyard_listing_ids_bytes = knowledge_bytes.shipyard_listing_ids_bytes,
                station_markets_bytes = knowledge_bytes.station_markets_bytes,
                station_passengers_bytes = knowledge_bytes.station_passengers_bytes,
                salvage_by_poi_bytes = knowledge_bytes.salvage_by_poi_bytes,
                storage_by_player_bytes = knowledge_bytes.storage_by_player_bytes,
                faction_storage_by_faction_poi_bytes =
                    knowledge_bytes.faction_storage_by_faction_poi_bytes,
                faction_garage_by_faction_bytes = knowledge_bytes.faction_garage_by_faction_bytes,
                virtual_orders_bytes = knowledge_bytes.virtual_orders_bytes,
                virtual_craft_orders_bytes = knowledge_bytes.virtual_craft_orders_bytes,
                facilities_by_poi_bytes = knowledge_bytes.facilities_by_poi_bytes,
                agent_sightings_bytes = knowledge_bytes.agent_sightings_bytes,
                system_agents_by_system_bytes = knowledge_bytes.system_agents_by_system_bytes,
                wildlife_by_poi_bytes = knowledge_bytes.wildlife_by_poi_bytes,
                managed_players_bytes = knowledge_bytes.managed_players_bytes,
                session_live_state_bytes,
                session_effective_state_bytes,
                session_engine_snapshot_bytes,
                session_memory_line_bytes,
                session_status_line_bytes,
                session_state_probe_ms,
                knowledge_probe_ms = breakdown_started.elapsed().as_millis(),
                "commander retained state size breakdown"
            );
        }
        Ok(CommanderStateResponse {
            state_version: self.commander_state_sequence.load(Ordering::Acquire),
            knowledge_version: knowledge.knowledge_version,
            world,
            sessions,
            social,
        })
    }

    pub async fn commander_fleet_snapshot(&self) -> Result<CommanderFleetResponse, SdkError> {
        let knowledge = self.knowledge_state.read().clone();
        let entries: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, session.clone()))
            .collect();

        let sessions_total = entries.len();
        let mut sessions_observed = 0usize;
        let mut state_version = 0u64;
        let mut owned_ships: Vec<RuntimeOwnedShipProjectionDto> = Vec::new();
        let mut faction_garage_ships: Vec<RuntimeFactionGarageShipProjectionDto> = Vec::new();

        for (_id, session) in entries {
            let session = session.lock().await;
            state_version = state_version.max(session.state_version);
            if !session.has_state {
                continue;
            }
            sessions_observed += 1;
            let actor = &session.actor.observed;
            let owner_handle = session.label.clone();
            let owner_name = actor
                .player
                .username
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| owner_handle.clone());
            let owner_id = actor.player.id.clone().unwrap_or_default();
            let faction_id = actor.player.faction_id.clone().unwrap_or_default();
            let faction_tag = actor.player.clan_tag.clone().unwrap_or_default();

            for mut ship in map_actor_owned_ships(actor) {
                ship.owner_handle = owner_handle.clone();
                ship.owner_kind = "personal".to_string();
                ship.owner_id = owner_id.clone();
                ship.owner_name = owner_name.clone();
                ship.faction_id = faction_id.clone();
                ship.faction_tag = faction_tag.clone();
                owned_ships.push(ship);
            }
            let garage_value = [
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
            let mut garage = map_faction_garage_value(&garage_value);
            for ship in &mut garage.ships {
                ship.owner_handle = owner_handle.clone();
                ship.faction_id = faction_id.clone();
                ship.faction_tag = faction_tag.clone();
            }
            faction_garage_ships.extend(garage.ships);
        }

        let mut garage_by_ship_id: HashMap<String, RuntimeFactionGarageShipProjectionDto> =
            HashMap::new();
        for ship in faction_garage_ships.drain(..) {
            let ship_id = ship.ship.ship_id.trim();
            if ship_id.is_empty() {
                continue;
            }
            match garage_by_ship_id.get_mut(ship_id) {
                Some(existing) => {
                    if existing.base_id.trim().is_empty() && !ship.base_id.trim().is_empty() {
                        *existing = ship;
                    }
                }
                None => {
                    garage_by_ship_id.insert(ship_id.to_string(), ship);
                }
            }
        }
        faction_garage_ships = garage_by_ship_id.into_values().collect();

        owned_ships.sort_by(|a, b| {
            a.owner_handle
                .cmp(&b.owner_handle)
                .then_with(|| b.ship.is_active.cmp(&a.ship.is_active))
                .then_with(|| a.ship.location.cmp(&b.ship.location))
                .then_with(|| a.ship.class_name.cmp(&b.ship.class_name))
                .then_with(|| a.ship.ship_id.cmp(&b.ship.ship_id))
        });
        faction_garage_ships.sort_by(|a, b| {
            a.faction_tag
                .cmp(&b.faction_tag)
                .then_with(|| a.ship.class_name.cmp(&b.ship.class_name))
                .then_with(|| a.ship.custom_name.cmp(&b.ship.custom_name))
                .then_with(|| a.ship.ship_id.cmp(&b.ship.ship_id))
        });

        Ok(CommanderFleetResponse {
            state_version,
            knowledge_version: knowledge.knowledge_version,
            sessions_observed,
            sessions_total,
            owned_ships,
            faction_garage_ships,
        })
    }

    pub async fn cached_tax_estimate(
        &self,
        id: Uuid,
    ) -> Result<Option<spacemolt_lib_rs::schema::TaxEstimateResponse>, SdkError> {
        {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            let ttl = self.options.tax_estimate_ttl;
            if let Some(cached) = &session.tax_estimate_cache {
                if cached.fetched_at.elapsed() < ttl {
                    return Ok(Some(cached.value.clone()));
                }
            }
        }

        let response = self
            .spacemolt_account(id)
            .await?
            .commands()
            .spacemolt()
            .get_tax_estimate()
            .await
            .map_err(SdkError::from)?
            .into_typed()
            .map_err(SdkError::from)?;

        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        session.tax_estimate_cache = Some(CachedTaxEstimate {
            fetched_at: Instant::now(),
            value: response.clone(),
        });
        Ok(Some(response))
    }

    pub async fn commander_storage_snapshot(&self) -> Result<CommanderStorageResponse, SdkError> {
        let knowledge = self.knowledge_state.read().clone();
        let entries: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, session.clone()))
            .collect();

        let sessions_total = entries.len();
        let mut sessions_observed = 0usize;
        let mut state_version = 0u64;
        let mut rows_by_key: HashMap<String, StorageRowAccumulator> = HashMap::new();
        let market_prices = storage_market_prices(&MarketData {
            station_markets: knowledge.station_markets.clone(),
            ..MarketData::default()
        });

        for (id, session) in entries {
            let session = session.lock().await;
            let session_state_version = session.state_version;
            state_version = state_version.max(session_state_version);
            if !session.has_state {
                continue;
            }
            sessions_observed += 1;
            let session_label = session.label.clone();
            let mut actor = session.actor.observed.as_ref().clone();
            Self::seed_state_identity_from_session(&mut actor, &session);
            drop(session);
            let observed_by = session_label.clone();
            let player_owner_id = actor.player.id.clone();
            let player_owner_name = actor
                .player
                .username
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| session_label.clone());

            insert_storage_row(
                &mut rows_by_key,
                StorageRowAccumulator {
                    item_id: "credits".to_string(),
                    quantity: actor.player.credits.unwrap_or_default(),
                    source_kind: "financial".to_string(),
                    owner_id: player_owner_id.clone(),
                    owner_name: player_owner_name.clone(),
                    location_id: "wallet".to_string(),
                    location_name: Some("wallet".to_string()),
                    system_id: None,
                    observed_by: vec![observed_by.clone()],
                    state_version: session_state_version,
                    details: Some(serde_json::json!({
                        "kind": "credits",
                        "label": "Wallet credits",
                    })),
                },
            );

            if let Some(faction_id) = actor
                .player
                .faction_id
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
            {
                // Read the shared treasury balance kept fresh by the designated
                // faction-storage watcher (see `refresh_watched_faction_treasury`)
                // rather than fetching it live on the request path.
                if let Some(treasury) = knowledge.faction_treasury_by_faction.get(faction_id) {
                    insert_storage_row(
                        &mut rows_by_key,
                        StorageRowAccumulator {
                            item_id: "credits".to_string(),
                            quantity: treasury.treasury,
                            source_kind: "financial".to_string(),
                            owner_id: Some(faction_id.to_string()),
                            owner_name: treasury.faction_name.clone(),
                            location_id: "faction_treasury".to_string(),
                            location_name: Some("faction treasury".to_string()),
                            system_id: None,
                            observed_by: vec![observed_by.clone()],
                            state_version: session_state_version,
                            details: Some(serde_json::json!({
                                "kind": "faction_treasury",
                                "label": "Faction treasury",
                                "faction_id": faction_id,
                            })),
                        },
                    );
                }
            }

            match self.cached_tax_estimate(id).await {
                Ok(Some(tax)) => {
                    let quantity = tax_estimate_net_owed(&tax);
                    if quantity > 0 {
                        insert_storage_row(
                            &mut rows_by_key,
                            StorageRowAccumulator {
                                item_id: "tax_estimate".to_string(),
                                quantity,
                                source_kind: "financial".to_string(),
                                owner_id: player_owner_id.clone(),
                                owner_name: player_owner_name.clone(),
                                location_id: "tax".to_string(),
                                location_name: Some("next assessment".to_string()),
                                system_id: None,
                                observed_by: vec![observed_by.clone()],
                                state_version: session_state_version,
                                details: Some(serde_json::json!({
                                    "kind": "tax_estimate",
                                    "estimate": tax,
                                })),
                            },
                        );
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    debug!(
                        session = %session_label,
                        error = %err,
                        "commander storage: tax estimate unavailable"
                    );
                }
            }

            let inventory = InventoryLens::project_canonical(
                &knowledge,
                &[(id, session_state_version, actor.clone())],
            );
            for lot in inventory
                .query_canonical(
                    &actor,
                    &knowledge.galaxy,
                    inventory::InventoryQuery {
                        item_id: None,
                        owner: None,
                        location: None,
                        sources: inventory::InventorySourceMask::CARGO
                            | inventory::InventorySourceMask::PERSONAL_STORAGE
                            | inventory::InventorySourceMask::FACTION_STORAGE,
                        freshness: inventory::InventoryFreshnessPolicy::IncludeRemembered,
                    },
                )
                .expect("non-empty inventory source mask")
            {
                if matches!(lot.source, inventory::InventorySource::FactionStorage)
                    && !knowledge.galaxy.is_station_poi(&lot.location.poi_id)
                {
                    continue;
                }
                let (source_kind, owner_id, owner_name, details) = match (&lot.source, &lot.owner) {
                    (
                        inventory::InventorySource::Cargo { .. },
                        inventory::InventoryOwner::Player {
                            canonical_id,
                            display_name,
                        },
                    ) => (
                        "cargo",
                        Some(canonical_id.clone()),
                        display_name
                            .clone()
                            .unwrap_or_else(|| player_owner_name.clone()),
                        None,
                    ),
                    (
                        inventory::InventorySource::PersonalStorage,
                        inventory::InventoryOwner::Player {
                            canonical_id,
                            display_name,
                        },
                    ) => (
                        "personal",
                        Some(canonical_id.clone()),
                        display_name
                            .clone()
                            .unwrap_or_else(|| player_owner_name.clone()),
                        Some(
                            serde_json::json!({ "jumps": storage_location_jumps(&actor, &knowledge.galaxy, lot.location.system_id.as_deref()) }),
                        ),
                    ),
                    (
                        inventory::InventorySource::FactionStorage,
                        inventory::InventoryOwner::Faction { faction_id },
                    ) => (
                        "faction",
                        Some(faction_id.clone()),
                        faction_id.clone(),
                        Some(
                            serde_json::json!({ "jumps": storage_location_jumps(&actor, &knowledge.galaxy, lot.location.system_id.as_deref()) }),
                        ),
                    ),
                    _ => continue,
                };
                insert_storage_row(
                    &mut rows_by_key,
                    StorageRowAccumulator {
                        item_id: lot.item_id.clone(),
                        quantity: lot.quantity,
                        source_kind: source_kind.to_string(),
                        owner_id,
                        owner_name,
                        location_id: lot.location.poi_id.clone(),
                        location_name: lot
                            .location
                            .display_name
                            .clone()
                            .or_else(|| (source_kind == "cargo").then(|| "ship".to_string())),
                        system_id: lot.location.system_id.clone(),
                        observed_by: vec![observed_by.clone()],
                        state_version: lot
                            .observation
                            .state_version
                            .unwrap_or(session_state_version),
                        details,
                    },
                );
            }
        }

        let mut rows = rows_by_key
            .into_values()
            .filter(|row| row.quantity > 0 || row.source_kind == "financial")
            .map(|row| {
                let key = storage_row_key(
                    &row.source_kind,
                    row.owner_id.as_deref().unwrap_or(&row.owner_name),
                    &row.location_id,
                    &row.item_id,
                );
                let price = market_prices.get(&row.item_id);
                let median_buy_price = price.and_then(|price| price.median_buy_price);
                let median_sell_price = price.and_then(|price| price.median_sell_price);
                let compatibility_unit_price = median_sell_price.or(median_buy_price);
                let market_price_source = price.map(|_| {
                    if matches!(row.item_id.as_str(), "credits" | "tax_estimate") {
                        "credits".to_string()
                    } else {
                        "globalMedianBuySell".to_string()
                    }
                });
                CommanderStorageRowDto {
                    key,
                    item_id: row.item_id,
                    quantity: row.quantity,
                    unit_market_price: compatibility_unit_price,
                    total_market_value: compatibility_unit_price
                        .map(|price| price * row.quantity as f64),
                    unit_median_buy_price: median_buy_price,
                    unit_median_sell_price: median_sell_price,
                    total_median_buy_value: median_buy_price
                        .map(|price| price * row.quantity as f64),
                    total_median_sell_value: median_sell_price
                        .map(|price| price * row.quantity as f64),
                    market_price_source,
                    source_kind: row.source_kind,
                    owner_id: row.owner_id,
                    owner_name: row.owner_name,
                    location_id: row.location_id,
                    location_name: row.location_name,
                    system_id: row.system_id,
                    observed_by: row.observed_by,
                    state_version: row.state_version,
                    details: row.details,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            a.item_id
                .cmp(&b.item_id)
                .then_with(|| b.quantity.cmp(&a.quantity))
                .then_with(|| a.source_kind.cmp(&b.source_kind))
                .then_with(|| a.owner_name.cmp(&b.owner_name))
                .then_with(|| a.location_id.cmp(&b.location_id))
        });

        Ok(CommanderStorageResponse {
            state_version,
            knowledge_version: knowledge.knowledge_version,
            sessions_observed,
            sessions_total,
            rows,
        })
    }
}
