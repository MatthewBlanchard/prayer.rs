//! Canonical, read-only inventory/source projection.

use std::collections::HashSet;

use prayer_runtime::{BotState, GalaxyData, MarketData};
use tracing::debug;
use uuid::Uuid;

use crate::{
    RuntimeInventoryClaimDto, RuntimeInventoryMovementDto, RuntimeInventoryMovementReserveRequest,
    RuntimeInventoryMovementReserveResponse, RuntimeInventoryMovementStatusDto,
    RuntimeInventoryMovementsResponse, SdkError,
};

use super::{faction_station_storage_key, player_station_storage_key, RuntimeService, WorldState};

#[cfg(test)]
pub use prayer_runtime::knowledge::{
    inventory_availability, InventoryAvailability, InventoryAvailabilityReason,
};
pub use prayer_runtime::knowledge::{
    InventoryFreshnessPolicy, InventoryLocation, InventoryLot, InventoryLotId,
    InventoryObservation, InventoryOwner, InventoryOwnerSelector, InventoryProvenance,
    InventoryQuery, InventoryQueryError, InventorySource, InventorySourceMask,
};
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InventoryClaimKey(String);

#[derive(Debug, Default)]
pub struct InventoryReservationLedger {
    inner: prayer_runtime::knowledge::InventoryReservationLedger,
}

impl InventoryReservationLedger {
    pub fn has_active_market_reservations(&self) -> bool {
        self.inner.movements().iter().any(|movement| {
            movement.status.is_active()
                && movement.claims.iter().any(|claim| {
                    matches!(claim.source_kind.as_str(), "market" | "market_bid")
                        && claim.quantity > 0
                })
        })
    }

    /// Subtract active physical market claims from projected order-book depth.
    /// Claims are compound-keyed rather than price-level keyed, so consume the
    /// economically best rows first, matching arbitrage package construction.
    pub fn apply_market_reservations(&self, market: &mut MarketData) {
        for (station, snapshot) in &mut market.station_markets {
            for (item_id, orders) in &mut snapshot.sell_orders {
                let reserved = self.reserved_for_compound("market", "market", station, item_id);
                subtract_reserved_market_depth(orders, reserved, false);
            }
            for (item_id, orders) in &mut snapshot.buy_orders {
                let reserved = self.reserved_for_compound("market_bid", "market", station, item_id);
                subtract_reserved_market_depth(orders, reserved, true);
            }
        }
    }

    pub fn reserve_canonical(
        &mut self,
        index: &InventoryIndex,
        actor: &BotState,
        galaxy: &GalaxyData,
        session_id: Uuid,
        request: RuntimeInventoryMovementReserveRequest,
    ) -> RuntimeInventoryMovementReserveResponse {
        use prayer_runtime::knowledge::{ResolvedInventoryClaim, VirtualOrderUse};

        let mut unavailable = Vec::new();
        let mut resolved = Vec::with_capacity(request.claims.len());
        for claim in request.claims {
            match resolve_claim_canonical(index, actor, galaxy, claim.clone()) {
                Ok((claim, _, observed_quantity)) => resolved.push(ResolvedInventoryClaim {
                    claim: claim_to_domain(claim),
                    observed_quantity,
                }),
                Err(claim) => unavailable.push(claim),
            }
        }
        if !unavailable.is_empty() {
            return RuntimeInventoryMovementReserveResponse {
                accepted: false,
                movement: None,
                unavailable_claims: unavailable,
                unavailable_virtual_order_uses: Vec::new(),
            };
        }
        let outcome = self.inner.reserve(
            session_id,
            request.kind,
            resolved,
            request
                .virtual_order_uses
                .into_iter()
                .map(|order| VirtualOrderUse {
                    order_id: order.order_id,
                    quantity: order.quantity,
                })
                .collect(),
            request.context,
            chrono::Utc::now().timestamp(),
        );
        RuntimeInventoryMovementReserveResponse {
            accepted: outcome.accepted,
            movement: outcome.movement.map(movement_to_dto),
            unavailable_claims: outcome
                .unavailable_claims
                .into_iter()
                .map(claim_to_dto)
                .collect(),
            unavailable_virtual_order_uses: Vec::new(),
        }
    }

    pub fn movements(&self) -> Vec<RuntimeInventoryMovementDto> {
        self.inner
            .movements()
            .into_iter()
            .map(movement_to_dto)
            .collect()
    }

    pub fn reserved_for_compound(
        &self,
        source_kind: &str,
        owner_id: &str,
        location_id: &str,
        item_id: &str,
    ) -> i64 {
        self.inner
            .reserved_for_compound(source_kind, owner_id, location_id, item_id)
    }

    pub fn transition(
        &mut self,
        movement_id: Uuid,
        status: RuntimeInventoryMovementStatusDto,
    ) -> Option<RuntimeInventoryMovementDto> {
        self.inner
            .transition(
                movement_id,
                status_to_domain(status),
                chrono::Utc::now().timestamp(),
            )
            .map(movement_to_dto)
    }

    pub fn reconcile(
        &mut self,
        movement_id: Uuid,
        reason: &str,
    ) -> Option<RuntimeInventoryMovementDto> {
        self.inner
            .reconcile(movement_id, reason, chrono::Utc::now().timestamp())
            .map(movement_to_dto)
    }
}

fn subtract_reserved_market_depth(
    orders: &mut Vec<prayer_state::MarketOrder>,
    mut reserved: i64,
    highest_price_first: bool,
) {
    if reserved <= 0 {
        return;
    }
    let mut indexes = orders
        .iter()
        .enumerate()
        .filter(|(_, order)| {
            order.quantity > 0
                && !order
                    .source
                    .as_deref()
                    .is_some_and(|source| source.starts_with("virtual_faction:"))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indexes.sort_by(|left, right| {
        let ordering = orders[*left].price_each.cmp(&orders[*right].price_each);
        if highest_price_first {
            ordering.reverse()
        } else {
            ordering
        }
        .then(left.cmp(right))
    });
    for index in indexes {
        if reserved <= 0 {
            break;
        }
        let claimed = reserved.min(orders[index].quantity.max(0));
        orders[index].quantity = orders[index].quantity.saturating_sub(claimed);
        reserved = reserved.saturating_sub(claimed);
    }
    orders.retain(|order| order.quantity > 0);
}

fn claim_to_domain(claim: RuntimeInventoryClaimDto) -> prayer_runtime::knowledge::InventoryClaim {
    prayer_runtime::knowledge::InventoryClaim {
        lot_id: claim.lot_id,
        source_kind: claim.source_kind,
        owner_id: claim.owner_id,
        location_id: claim.location_id,
        item_id: claim.item_id,
        quantity: claim.quantity,
    }
}

fn claim_to_dto(claim: prayer_runtime::knowledge::InventoryClaim) -> RuntimeInventoryClaimDto {
    RuntimeInventoryClaimDto {
        lot_id: claim.lot_id,
        source_kind: claim.source_kind,
        owner_id: claim.owner_id,
        location_id: claim.location_id,
        item_id: claim.item_id,
        quantity: claim.quantity,
    }
}

fn status_to_domain(
    status: RuntimeInventoryMovementStatusDto,
) -> prayer_runtime::knowledge::InventoryMovementStatus {
    use prayer_runtime::knowledge::InventoryMovementStatus as Domain;
    match status {
        RuntimeInventoryMovementStatusDto::Reserved => Domain::Reserved,
        RuntimeInventoryMovementStatusDto::Running => Domain::Running,
        RuntimeInventoryMovementStatusDto::Completed => Domain::Completed,
        RuntimeInventoryMovementStatusDto::Failed => Domain::Failed,
        RuntimeInventoryMovementStatusDto::Released => Domain::Released,
        RuntimeInventoryMovementStatusDto::NeedsReconciliation => Domain::NeedsReconciliation,
    }
}

fn status_to_dto(
    status: prayer_runtime::knowledge::InventoryMovementStatus,
) -> RuntimeInventoryMovementStatusDto {
    use prayer_runtime::knowledge::InventoryMovementStatus as Domain;
    match status {
        Domain::Reserved => RuntimeInventoryMovementStatusDto::Reserved,
        Domain::Running => RuntimeInventoryMovementStatusDto::Running,
        Domain::Completed => RuntimeInventoryMovementStatusDto::Completed,
        Domain::Failed => RuntimeInventoryMovementStatusDto::Failed,
        Domain::Released => RuntimeInventoryMovementStatusDto::Released,
        Domain::NeedsReconciliation => RuntimeInventoryMovementStatusDto::NeedsReconciliation,
    }
}

fn movement_to_dto(
    movement: prayer_runtime::knowledge::InventoryMovement,
) -> RuntimeInventoryMovementDto {
    RuntimeInventoryMovementDto {
        movement_id: movement.movement_id,
        session_id: movement.session_id,
        kind: movement.kind,
        status: status_to_dto(movement.status),
        claims: movement.claims.into_iter().map(claim_to_dto).collect(),
        virtual_order_uses: movement
            .virtual_order_uses
            .into_iter()
            .map(|order| crate::RuntimeVirtualOrderUseDto {
                order_id: order.order_id,
                quantity: order.quantity,
            })
            .collect(),
        context: movement.context,
        created_at_unix: movement.created_at_unix,
        updated_at_unix: movement.updated_at_unix,
    }
}

#[derive(Debug, Clone, Default)]
pub struct InventoryIndex {
    lots: Vec<InventoryLot>,
    known_faction_locations: HashSet<(String, String)>,
}

impl InventoryIndex {
    pub fn project_canonical(knowledge: &WorldState, sessions: &[(Uuid, u64, BotState)]) -> Self {
        Self::project_canonical_with_metadata(knowledge, &Default::default(), sessions)
    }

    pub fn project_canonical_with_metadata(
        knowledge: &WorldState,
        metadata: &prayer_runtime::knowledge::WorldRuntimeMetadata,
        sessions: &[(Uuid, u64, BotState)],
    ) -> Self {
        let mut index = Self::default();
        for (session_id, version, actor) in sessions {
            index.project_canonical_session(knowledge, metadata, *session_id, *version, actor);
        }
        index.project_canonical_markets(knowledge, sessions.first().map(|entry| &entry.2));
        index.project_canonical_passengers(knowledge, sessions.first().map(|entry| &entry.2));
        index
    }

    pub fn query_canonical<'a>(
        &'a self,
        actor: &BotState,
        galaxy: &GalaxyData,
        query: InventoryQuery<'_>,
    ) -> Result<Vec<&'a InventoryLot>, InventoryQueryError> {
        if query.sources.is_empty() {
            return Err(InventoryQueryError::EmptySourceMask);
        }
        debug_assert!(query.sources.has_only_known_sources());
        let location = query
            .location
            .map(|alias| {
                canonical_poi_for_actor(actor, galaxy, alias)
                    .ok_or_else(|| InventoryQueryError::UnresolvedLocation(alias.to_string()))
            })
            .transpose()?;
        Ok(self
            .lots
            .iter()
            .filter(|lot| {
                source_mask(&lot.source).is_some_and(|mask| query.sources.contains(mask))
                    && query.item_id.map_or(true, |item| item == lot.item_id)
                    && location
                        .as_ref()
                        .map_or(true, |poi| poi == &lot.location.poi_id)
                    && owner_matches(&lot.owner, query.owner.as_ref())
                    && (query.freshness == InventoryFreshnessPolicy::IncludeRemembered
                        || lot.observation.provenance == InventoryProvenance::Live)
            })
            .collect())
    }

    #[cfg(test)]
    pub fn availability(
        lot: &InventoryLot,
        reserved: i64,
        freshness: InventoryFreshnessPolicy,
    ) -> InventoryAvailability {
        inventory_availability(lot, reserved, freshness)
    }

    fn project_canonical_session(
        &mut self,
        knowledge: &WorldState,
        metadata: &prayer_runtime::knowledge::WorldRuntimeMetadata,
        session_id: Uuid,
        version: u64,
        actor: &BotState,
    ) {
        let owner_id = actor
            .player
            .id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| actor.player.username.clone())
            .unwrap_or_default();
        let owner = InventoryOwner::Player {
            canonical_id: owner_id,
            display_name: actor.player.username.clone(),
        };
        let ship_poi = actor.effective_poi_id().unwrap_or("ship");
        for (item, quantity) in actor.cargo.iter() {
            self.push(
                item,
                *quantity,
                owner.clone(),
                location_for_actor(actor, &knowledge.galaxy, ship_poi),
                InventorySource::Cargo { session_id },
                InventoryProvenance::Live,
                Some(version),
            );
        }
        let mut remembered = std::collections::HashMap::new();
        for key in super::storage_player_keys_for_actor(actor) {
            if let Some(storage) = knowledge.storage_by_player.get(key) {
                for (poi, items) in storage {
                    let canonical = canonical_poi_for_actor(actor, &knowledge.galaxy, poi)
                        .unwrap_or_else(|| poi.clone());
                    remembered.entry(canonical).or_insert_with(|| items.clone());
                }
            }
        }
        for (poi, items) in remembered {
            let provenance =
                if metadata
                    .storage_fetched_at_by_key
                    .contains_key(&player_station_storage_key(
                        match &owner {
                            InventoryOwner::Player { canonical_id, .. } => canonical_id,
                            _ => unreachable!("personal storage owner must be a player"),
                        },
                        &poi,
                    ))
                {
                    InventoryProvenance::Live
                } else {
                    InventoryProvenance::Remembered
                };
            for (item, quantity) in items {
                self.push(
                    &item,
                    quantity,
                    owner.clone(),
                    location_for_actor(actor, &knowledge.galaxy, &poi),
                    InventorySource::PersonalStorage,
                    provenance,
                    None,
                );
            }
        }
        if let Some(faction) = actor
            .player
            .faction_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            if let Some(by_poi) = knowledge.faction_storage_by_faction_poi.get(faction) {
                for (poi, items) in by_poi {
                    let canonical = canonical_poi_for_actor(actor, &knowledge.galaxy, poi)
                        .unwrap_or_else(|| poi.clone());
                    self.known_faction_locations
                        .insert((faction.to_string(), canonical));
                    let provenance = if metadata
                        .faction_storage_fetched_at_by_key
                        .contains_key(&faction_station_storage_key(faction, poi))
                    {
                        InventoryProvenance::Live
                    } else {
                        InventoryProvenance::Remembered
                    };
                    for (item, quantity) in items {
                        self.push(
                            item,
                            *quantity,
                            InventoryOwner::Faction {
                                faction_id: faction.to_string(),
                            },
                            location_for_actor(actor, &knowledge.galaxy, poi),
                            InventorySource::FactionStorage,
                            provenance,
                            None,
                        );
                    }
                }
            }
        }
    }

    fn project_canonical_markets(&mut self, knowledge: &WorldState, actor: Option<&BotState>) {
        let Some(actor) = actor else {
            return;
        };
        for (poi, market) in &knowledge.station_markets {
            for (item, asks) in &market.sell_orders {
                for ask in asks {
                    self.push(
                        item,
                        ask.quantity,
                        InventoryOwner::Market,
                        location_for_actor(actor, &knowledge.galaxy, poi),
                        InventorySource::MarketAsk {
                            price_each: ask.price_each,
                            order_source: ask.source.clone(),
                        },
                        InventoryProvenance::Live,
                        None,
                    );
                }
            }
            for (item, bids) in &market.buy_orders {
                for bid in bids {
                    self.push(
                        item,
                        bid.quantity,
                        InventoryOwner::Market,
                        location_for_actor(actor, &knowledge.galaxy, poi),
                        InventorySource::MarketBid,
                        InventoryProvenance::Live,
                        None,
                    );
                }
            }
        }
    }

    fn project_canonical_passengers(&mut self, knowledge: &WorldState, actor: Option<&BotState>) {
        let Some(actor) = actor else {
            return;
        };
        for (station, board) in &knowledge.station_passengers {
            for passenger in board.waiting.iter() {
                self.push(
                    &passenger.citizen_id,
                    1,
                    InventoryOwner::Market,
                    location_for_actor(actor, &knowledge.galaxy, station),
                    InventorySource::Passenger,
                    InventoryProvenance::Live,
                    None,
                );
            }
        }
    }

    fn push(
        &mut self,
        item: &str,
        quantity: i64,
        owner: InventoryOwner,
        location: InventoryLocation,
        source: InventorySource,
        provenance: InventoryProvenance,
        version: Option<u64>,
    ) {
        let owner_key = match &owner {
            InventoryOwner::Player { canonical_id, .. } => canonical_id.as_str(),
            InventoryOwner::Faction { faction_id } => faction_id.as_str(),
            InventoryOwner::Market => "market",
        };
        let source_key = match &source {
            InventorySource::Cargo { session_id } => session_id.to_string(),
            InventorySource::PersonalStorage => "personal".into(),
            InventorySource::FactionStorage => "faction".into(),
            InventorySource::MarketAsk {
                price_each,
                order_source,
            } => format!(
                "ask:{price_each}:{}",
                order_source.as_deref().unwrap_or("real")
            ),
            InventorySource::MarketBid => "bid".into(),
            InventorySource::Passenger => "passenger".into(),
        };
        let id = InventoryLotId(format!(
            "{source_key}|{owner_key}|{}|{item}",
            location.poi_id
        ));
        self.lots.push(InventoryLot {
            id,
            item_id: item.to_string(),
            quantity,
            owner,
            location,
            source,
            observation: InventoryObservation {
                provenance,
                observed_at_unix: None,
                state_version: version,
            },
        });
    }
}

impl RuntimeService {
    pub fn economy_market_ask_candidates(
        &self,
        state: &prayer_runtime::economy::EconomyReadState,
        item_id: &str,
        max_price: i64,
    ) -> Vec<(String, i64, i64, Option<String>)> {
        let reservations = self.inventory_reservations.lock();
        let mut out = Vec::new();
        for (station, snapshot) in &state.market.station_markets {
            let mut reserved =
                reservations.reserved_for_compound("market", "market", station, item_id);
            let mut orders = snapshot
                .sell_orders
                .get(item_id)
                .cloned()
                .unwrap_or_default();
            orders.sort_by_key(|order| order.price_each);
            for order in orders {
                if order.quantity <= 0 || order.price_each <= 0 || order.price_each > max_price {
                    continue;
                }
                let available = order.quantity.saturating_sub(reserved).max(0);
                reserved = reserved.saturating_sub(order.quantity.max(0));
                if available > 0 {
                    out.push((station.clone(), available, order.price_each, order.source));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
        out
    }

    pub fn economy_personal_storage_quantity_at(
        &self,
        state: &prayer_runtime::economy::EconomyReadState,
        location: &str,
        item_id: &str,
    ) -> i64 {
        let knowledge = self.knowledge_state.read();
        let poi = state.galaxy.poi_id_for_base(location).unwrap_or(location);
        for owner in [state.player_id.as_deref(), state.username.as_deref()]
            .into_iter()
            .flatten()
        {
            if let Some(quantity) = knowledge
                .storage_by_player
                .get(owner)
                .and_then(|by_poi| by_poi.get(poi).or_else(|| by_poi.get(location)))
                .and_then(|items| items.get(item_id))
                .copied()
            {
                let reserved = self.inventory_reservations.lock().reserved_for_compound(
                    "personal_storage",
                    owner,
                    poi,
                    item_id,
                );
                return quantity.saturating_sub(reserved).max(0);
            }
        }
        0
    }

    pub async fn reserve_inventory_movement(
        &self,
        request: RuntimeInventoryMovementReserveRequest,
    ) -> Result<RuntimeInventoryMovementReserveResponse, SdkError> {
        let session_id = self
            .session_for_bot_selector(request.session_id.as_str())
            .await?;
        let session = self.get_session(session_id).await?;
        let session = session.lock().await;
        if !session.has_state {
            return Err(SdkError::BadRequest(
                "session has no observed state".to_string(),
            ));
        }
        let mut actor = session.actor.observed.as_ref().clone();
        Self::seed_state_identity_from_session(&mut actor, &session);
        drop(session);
        let knowledge = self.knowledge_state.read().clone();
        let index = {
            let metadata = self.knowledge_metadata.read();
            InventoryIndex::project_canonical_with_metadata(
                &knowledge,
                &metadata,
                &[(session_id, 0, actor.clone())],
            )
        };
        let _gate = self.inventory_reservation_gate.lock();
        let mut requested_by_order = std::collections::HashMap::<String, i64>::new();
        for order_use in &request.virtual_order_uses {
            let quantity = requested_by_order
                .entry(order_use.order_id.trim().to_string())
                .or_default();
            *quantity = quantity.saturating_add(order_use.quantity.max(0));
        }
        let orders = self.virtual_orders();
        let unavailable_virtual_order_uses = requested_by_order
            .iter()
            .filter_map(|(order_id, quantity)| {
                let available = orders
                    .iter()
                    .find(|order| order.id == *order_id)
                    .map(|order| self.virtual_order_open_quantity(&actor, order))
                    .unwrap_or(0);
                (*quantity <= 0 || available < *quantity).then(|| {
                    crate::RuntimeVirtualOrderUseDto {
                        order_id: order_id.clone(),
                        quantity: *quantity,
                    }
                })
            })
            .collect::<Vec<_>>();
        if !unavailable_virtual_order_uses.is_empty() {
            return Ok(RuntimeInventoryMovementReserveResponse {
                accepted: false,
                movement: None,
                unavailable_claims: Vec::new(),
                unavailable_virtual_order_uses,
            });
        }
        let mut result = self.inventory_reservations.lock().reserve_canonical(
            &index,
            &actor,
            &knowledge.galaxy,
            session_id,
            request,
        );
        if result.accepted && !requested_by_order.is_empty() {
            let uses = requested_by_order
                .iter()
                .map(|(order_id, quantity)| crate::RuntimeVirtualOrderUseDto {
                    order_id: order_id.clone(),
                    quantity: *quantity,
                })
                .collect::<Vec<_>>();
            if !self.reserve_virtual_order_uses_atomic(uses) {
                if let Some(movement) = result.movement.take() {
                    self.inventory_reservations.lock().transition(
                        movement.movement_id,
                        RuntimeInventoryMovementStatusDto::Released,
                    );
                }
                result.accepted = false;
                result.unavailable_virtual_order_uses = requested_by_order
                    .into_iter()
                    .map(|(order_id, quantity)| crate::RuntimeVirtualOrderUseDto {
                        order_id,
                        quantity,
                    })
                    .collect();
            }
        }
        debug!(
            accepted = result.accepted,
            physical_claims = result
                .movement
                .as_ref()
                .map(|movement| movement.claims.len())
                .unwrap_or(0),
            virtual_order_uses = result
                .movement
                .as_ref()
                .map(|movement| movement.virtual_order_uses.len())
                .unwrap_or(0),
            unavailable_physical = result.unavailable_claims.len(),
            unavailable_virtual = result.unavailable_virtual_order_uses.len(),
            "inventory movement reservation evaluated"
        );
        drop(_gate);
        if let Some(movement) = result.movement.as_ref() {
            {
                let mut knowledge = self.knowledge_state.write();
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
            }
            let session = self.get_session(movement.session_id).await?;
            session.lock().await.active_movement_id = Some(movement.movement_id);
        }
        Ok(result)
    }

    pub fn inventory_movements(&self) -> RuntimeInventoryMovementsResponse {
        RuntimeInventoryMovementsResponse {
            movements: self.inventory_reservations.lock().movements(),
        }
    }

    pub async fn transition_inventory_movement(
        &self,
        movement_id: Uuid,
        status: RuntimeInventoryMovementStatusDto,
    ) -> Result<RuntimeInventoryMovementDto, SdkError> {
        let movement = self
            .inventory_reservations
            .lock()
            .transition(movement_id, status)
            .ok_or_else(|| {
                SdkError::BadRequest(format!("unknown inventory movement '{movement_id}'"))
            })?;
        {
            let mut knowledge = self.knowledge_state.write();
            knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
        }
        match status {
            RuntimeInventoryMovementStatusDto::Completed => {
                self.settle_virtual_order_uses(&movement.virtual_order_uses, true);
            }
            RuntimeInventoryMovementStatusDto::Failed
            | RuntimeInventoryMovementStatusDto::Released => {
                self.settle_virtual_order_uses(&movement.virtual_order_uses, false);
            }
            _ => {}
        }
        if matches!(
            status,
            RuntimeInventoryMovementStatusDto::Completed
                | RuntimeInventoryMovementStatusDto::Failed
                | RuntimeInventoryMovementStatusDto::Released
        ) {
            if let Ok(session) = self.get_session(movement.session_id).await {
                let mut session = session.lock().await;
                if session.active_movement_id == Some(movement_id) {
                    session.active_movement_id = None;
                }
            }
        }
        Ok(movement)
    }

    pub async fn reconcile_inventory_movement(
        &self,
        movement_id: Uuid,
        reason: &str,
    ) -> Result<RuntimeInventoryMovementDto, SdkError> {
        if reason.trim().is_empty() {
            return Err(SdkError::BadRequest(
                "reconciliation reason is required".to_string(),
            ));
        }
        self.inventory_reservations
            .lock()
            .reconcile(movement_id, reason)
            .ok_or_else(|| {
                SdkError::BadRequest(format!(
                    "movement '{movement_id}' cannot be reconciled from its current state"
                ))
            })
    }
}

fn canonical_poi_for_actor(actor: &BotState, galaxy: &GalaxyData, value: &str) -> Option<String> {
    if galaxy.poi_records.contains_key(value) || actor.effective_poi_id() == Some(value) {
        Some(value.to_string())
    } else {
        galaxy
            .poi_id_for_base(value)
            .or(Some(value))
            .map(str::to_string)
    }
}

fn location_for_actor(actor: &BotState, galaxy: &GalaxyData, poi: &str) -> InventoryLocation {
    let canonical = canonical_poi_for_actor(actor, galaxy, poi).unwrap_or_else(|| poi.to_string());
    let display_name = galaxy
        .poi_records
        .get(&canonical)
        .map(|poi| poi.info.name.clone())
        .filter(|name| !name.trim().is_empty());
    let system_id = galaxy
        .poi_records
        .get(&canonical)
        .map(|poi| poi.system_id.clone())
        .or_else(|| {
            (actor.effective_poi_id() == Some(canonical.as_str()))
                .then(|| actor.effective_system_id().map(str::to_string))
                .flatten()
        })
        .or_else(|| {
            galaxy
                .poi_records
                .get(&canonical)
                .map(|poi| poi.system_id.clone())
                .filter(|system| !system.trim().is_empty())
        });
    InventoryLocation {
        system_id,
        display_name,
        poi_id: canonical,
    }
}
fn source_mask(source: &InventorySource) -> Option<InventorySourceMask> {
    Some(match source {
        InventorySource::Cargo { .. } => InventorySourceMask::CARGO,
        InventorySource::PersonalStorage => InventorySourceMask::PERSONAL_STORAGE,
        InventorySource::FactionStorage => InventorySourceMask::FACTION_STORAGE,
        InventorySource::MarketAsk { .. } => InventorySourceMask::MARKET,
        InventorySource::MarketBid => InventorySourceMask::MARKET_BID,
        InventorySource::Passenger => InventorySourceMask::PASSENGER,
    })
}
fn owner_matches(owner: &InventoryOwner, selector: Option<&InventoryOwnerSelector<'_>>) -> bool {
    match selector {
        None => true,
        Some(InventoryOwnerSelector::Player(id)) => {
            matches!(owner, InventoryOwner::Player { canonical_id, display_name } if canonical_id == id || display_name.as_deref() == Some(*id))
        }
        Some(InventoryOwnerSelector::Faction(id)) => {
            matches!(owner, InventoryOwner::Faction { faction_id } if faction_id == id)
        }
        Some(InventoryOwnerSelector::Market) => matches!(owner, InventoryOwner::Market),
    }
}

fn claim_key(claim: &RuntimeInventoryClaimDto) -> InventoryClaimKey {
    InventoryClaimKey(claim.lot_id.clone().unwrap_or_else(|| {
        format!(
            "{}|{}|{}|{}",
            claim.source_kind, claim.owner_id, claim.location_id, claim.item_id
        )
    }))
}

fn resolve_claim_canonical(
    index: &InventoryIndex,
    actor: &BotState,
    galaxy: &GalaxyData,
    mut claim: RuntimeInventoryClaimDto,
) -> Result<(RuntimeInventoryClaimDto, InventoryClaimKey, i64), RuntimeInventoryClaimDto> {
    if claim.quantity <= 0
        || claim.owner_id.trim().is_empty()
        || claim.item_id.trim().is_empty()
        || claim.location_id.trim().is_empty()
    {
        return Err(claim);
    }
    let (source_kind, sources, owner) = match claim
        .source_kind
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "cargo" => (
            "cargo",
            InventorySourceMask::CARGO,
            InventoryOwnerSelector::Player(claim.owner_id.trim()),
        ),
        "personal" | "storage" | "player_storage" | "personal_storage" => (
            "personal_storage",
            InventorySourceMask::PERSONAL_STORAGE,
            InventoryOwnerSelector::Player(claim.owner_id.trim()),
        ),
        "faction" | "virtual_faction" | "faction_storage" => (
            "faction_storage",
            InventorySourceMask::FACTION_STORAGE,
            InventoryOwnerSelector::Faction(claim.owner_id.trim()),
        ),
        "market" => (
            "market",
            InventorySourceMask::MARKET,
            InventoryOwnerSelector::Market,
        ),
        "market_bid" => (
            "market_bid",
            InventorySourceMask::MARKET_BID,
            InventoryOwnerSelector::Market,
        ),
        "passenger" => (
            "passenger",
            InventorySourceMask::PASSENGER,
            InventoryOwnerSelector::Market,
        ),
        _ => return Err(claim),
    };
    let canonical_location = match canonical_poi_for_actor(actor, galaxy, &claim.location_id) {
        Some(poi) => poi,
        None => return Err(claim),
    };
    let lots = match index.query_canonical(
        actor,
        galaxy,
        InventoryQuery {
            item_id: Some(&claim.item_id),
            owner: Some(owner),
            location: Some(&canonical_location),
            sources,
            freshness: InventoryFreshnessPolicy::LiveOnly,
        },
    ) {
        Ok(lots) => lots,
        Err(_) => return Err(claim),
    };
    let compound_key = format!(
        "{source_kind}|{}|{canonical_location}|{}",
        if matches!(source_kind, "market" | "market_bid" | "passenger") {
            "market"
        } else {
            claim.owner_id.trim()
        },
        claim.item_id.trim()
    );
    let matching = lots
        .into_iter()
        .filter(|lot| {
            claim
                .lot_id
                .as_deref()
                .map_or(true, |id| lot.id.0 == id || id == compound_key)
        })
        .collect::<Vec<_>>();
    let observed = matching.iter().map(|lot| lot.quantity.max(0)).sum::<i64>();
    if observed <= 0 {
        return Err(claim);
    }
    claim.source_kind = source_kind.to_string();
    claim.owner_id = if matches!(source_kind, "market" | "market_bid" | "passenger") {
        "market".to_string()
    } else {
        claim.owner_id.trim().to_string()
    };
    claim.location_id = canonical_location;
    claim.item_id = claim.item_id.trim().to_string();
    if claim.lot_id.is_none() && matching.len() == 1 {
        claim.lot_id = Some(matching[0].id.0.clone());
    }
    let key = claim_key(&claim);
    Ok((claim, key, observed))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use super::*;

    fn actor() -> BotState {
        BotState {
            player: spacemolt_lib_rs::schema::V2GameStatePlayer {
                id: Some("player-1".into()),
                username: Some("LegacyName".into()),
                faction_id: Some("faction-1".into()),
                ..Default::default()
            },
            ..BotState::default()
        }
    }

    fn galaxy() -> Arc<GalaxyData> {
        let mut galaxy = GalaxyData::default();
        galaxy.poi_records.insert(
            "poi-station".into(),
            prayer_state::PoiKnowledge {
                id: "poi-station".into(),
                system_id: "sol".into(),
                info: prayer_state::PoiInfoData {
                    poi_type: "station".into(),
                    base_id: Some("base-station".into()),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        Arc::new(galaxy)
    }

    fn knowledge() -> WorldState {
        WorldState {
            galaxy: galaxy(),
            storage_by_player: HashMap::from([
                (
                    "player-1".into(),
                    HashMap::from([("poi-station".into(), HashMap::from([("ore".into(), 7)]))]),
                ),
                (
                    "LegacyName".into(),
                    HashMap::from([("poi-other".into(), HashMap::from([("fuel".into(), 3)]))]),
                ),
            ]),
            faction_storage_by_faction_poi: HashMap::from([(
                "faction-1".into(),
                HashMap::from([("poi-station".into(), HashMap::from([("ore".into(), 11)]))]),
            )]),
            ..WorldState::default()
        }
    }

    #[test]
    fn canonical_projection_reads_actor_and_world_without_composition() {
        let mut actor = actor();
        actor.location.system_id = Some("sol".into());
        actor.location.poi_id = Some("poi-station".into());
        actor.cargo = Arc::new(HashMap::from([("cargo-item".into(), 5)]));
        let knowledge = knowledge();
        let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
        metadata.storage_fetched_at_by_key.insert(
            player_station_storage_key("player-1", "poi-station"),
            std::time::Instant::now(),
        );
        let canonical_index = InventoryIndex::project_canonical_with_metadata(
            &knowledge,
            &metadata,
            &[(Uuid::nil(), 9, actor.clone())],
        );
        let lots = canonical_index
            .query_canonical(
                &actor,
                &knowledge.galaxy,
                InventoryQuery {
                    item_id: None,
                    owner: None,
                    location: None,
                    sources: InventorySourceMask::ALL,
                    freshness: InventoryFreshnessPolicy::IncludeRemembered,
                },
            )
            .unwrap();
        assert!(lots.iter().any(|lot| lot.item_id == "cargo-item"
            && matches!(lot.source, InventorySource::Cargo { .. })));
        assert!(lots
            .iter()
            .any(|lot| lot.item_id == "ore"
                && lot.observation.provenance == InventoryProvenance::Live));
        assert!(lots.iter().any(|lot| lot.item_id == "fuel"
            && lot.observation.provenance == InventoryProvenance::Remembered));
    }

    #[test]
    fn live_empty_bucket_overrides_remembered_and_other_buckets_survive() {
        let actor = actor();
        let mut knowledge = knowledge();
        knowledge
            .storage_by_player
            .get_mut("player-1")
            .unwrap()
            .insert("poi-station".into(), HashMap::new());
        let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
        metadata.storage_fetched_at_by_key.insert(
            player_station_storage_key("player-1", "poi-station"),
            std::time::Instant::now(),
        );
        let index = InventoryIndex::project_canonical_with_metadata(
            &knowledge,
            &metadata,
            &[(Uuid::nil(), 4, actor.clone())],
        );
        let personal = index
            .query_canonical(
                &actor,
                &knowledge.galaxy,
                InventoryQuery {
                    item_id: None,
                    owner: Some(InventoryOwnerSelector::Player("player-1")),
                    location: None,
                    sources: InventorySourceMask::PERSONAL_STORAGE,
                    freshness: InventoryFreshnessPolicy::IncludeRemembered,
                },
            )
            .unwrap();
        assert!(!personal.iter().any(|lot| lot.item_id == "ore"));
        assert!(personal.iter().any(|lot| lot.item_id == "fuel"
            && lot.observation.provenance == InventoryProvenance::Remembered));
    }

    #[test]
    fn aliases_and_source_masks_are_strict() {
        let actor = actor();
        let knowledge = knowledge();
        let index =
            InventoryIndex::project_canonical(&knowledge, &[(Uuid::nil(), 1, actor.clone())]);
        let faction = index
            .query_canonical(
                &actor,
                &knowledge.galaxy,
                InventoryQuery {
                    item_id: Some("ore"),
                    owner: Some(InventoryOwnerSelector::Faction("faction-1")),
                    location: Some("base-station"),
                    sources: InventorySourceMask::FACTION_STORAGE,
                    freshness: InventoryFreshnessPolicy::IncludeRemembered,
                },
            )
            .unwrap();
        assert_eq!(faction.len(), 1);
        assert!(matches!(faction[0].source, InventorySource::FactionStorage));
        assert_eq!(
            index.query_canonical(
                &actor,
                &knowledge.galaxy,
                InventoryQuery {
                    item_id: None,
                    owner: None,
                    location: None,
                    sources: InventorySourceMask(0),
                    freshness: InventoryFreshnessPolicy::IncludeRemembered
                }
            ),
            Err(InventoryQueryError::EmptySourceMask)
        );
    }

    #[test]
    fn canonical_player_bucket_wins_over_legacy_at_same_location() {
        let actor = actor();
        let mut knowledge = knowledge();
        knowledge
            .storage_by_player
            .get_mut("LegacyName")
            .unwrap()
            .insert("poi-station".into(), HashMap::from([("ore".into(), 99)]));
        let index =
            InventoryIndex::project_canonical(&knowledge, &[(Uuid::nil(), 1, actor.clone())]);
        let lots = index
            .query_canonical(
                &actor,
                &knowledge.galaxy,
                InventoryQuery {
                    item_id: Some("ore"),
                    owner: Some(InventoryOwnerSelector::Player("player-1")),
                    location: Some("poi-station"),
                    sources: InventorySourceMask::PERSONAL_STORAGE,
                    freshness: InventoryFreshnessPolicy::IncludeRemembered,
                },
            )
            .unwrap();
        assert_eq!(lots.iter().map(|lot| lot.quantity).sum::<i64>(), 7);
    }

    #[test]
    fn every_source_mask_and_all_union_are_selective() {
        let mut actor = actor();
        actor.cargo = Arc::new(HashMap::from([("cargo_item".into(), 1)]));
        let mut knowledge = knowledge();
        knowledge
            .storage_by_player
            .get_mut("player-1")
            .unwrap()
            .insert(
                "poi-station".into(),
                HashMap::from([("personal_item".into(), 2)]),
            );
        knowledge.station_markets.insert(
            "poi-station".into(),
            prayer_state::StationMarketData {
                sell_orders: HashMap::from([(
                    "market_item".into(),
                    vec![prayer_state::MarketOrder {
                        price_each: 3,
                        quantity: 4,
                        source: None,
                        my_quantity: None,
                    }],
                )]),
                ..Default::default()
            },
        );
        let index =
            InventoryIndex::project_canonical(&knowledge, &[(Uuid::nil(), 1, actor.clone())]);
        for (mask, predicate) in [
            (InventorySourceMask::CARGO, 0),
            (InventorySourceMask::PERSONAL_STORAGE, 1),
            (InventorySourceMask::FACTION_STORAGE, 2),
            (InventorySourceMask::MARKET, 3),
        ] {
            let lots = index
                .query_canonical(
                    &actor,
                    &knowledge.galaxy,
                    InventoryQuery {
                        item_id: None,
                        owner: None,
                        location: None,
                        sources: mask,
                        freshness: InventoryFreshnessPolicy::IncludeRemembered,
                    },
                )
                .unwrap();
            assert!(!lots.is_empty());
            assert!(lots.iter().all(|lot| match predicate {
                0 => matches!(lot.source, InventorySource::Cargo { .. }),
                1 => matches!(lot.source, InventorySource::PersonalStorage),
                2 => matches!(lot.source, InventorySource::FactionStorage),
                _ => matches!(lot.source, InventorySource::MarketAsk { .. }),
            }));
        }
        let all = index
            .query_canonical(
                &actor,
                &knowledge.galaxy,
                InventoryQuery {
                    item_id: None,
                    owner: None,
                    location: None,
                    sources: InventorySourceMask::ALL,
                    freshness: InventoryFreshnessPolicy::IncludeRemembered,
                },
            )
            .unwrap();
        assert!(all
            .iter()
            .any(|lot| matches!(lot.source, InventorySource::Cargo { .. })));
        assert!(all
            .iter()
            .any(|lot| matches!(lot.source, InventorySource::PersonalStorage)));
        assert!(all
            .iter()
            .any(|lot| matches!(lot.source, InventorySource::FactionStorage)));
        assert!(all
            .iter()
            .any(|lot| matches!(lot.source, InventorySource::MarketAsk { .. })));
    }

    #[test]
    fn availability_separates_stock_reservations_and_freshness() {
        let lot = InventoryLot {
            id: InventoryLotId("lot".into()),
            item_id: "ore".into(),
            quantity: 10,
            owner: InventoryOwner::Market,
            location: InventoryLocation {
                poi_id: "poi".into(),
                system_id: None,
                display_name: None,
            },
            source: InventorySource::MarketAsk {
                price_each: 2,
                order_source: None,
            },
            observation: InventoryObservation {
                provenance: InventoryProvenance::Remembered,
                observed_at_unix: None,
                state_version: None,
            },
        };
        let planning =
            InventoryIndex::availability(&lot, 4, InventoryFreshnessPolicy::IncludeRemembered);
        assert_eq!(
            (
                planning.observed,
                planning.reserved,
                planning.available,
                planning.executable
            ),
            (10, 4, 6, true)
        );
        let launch = InventoryIndex::availability(&lot, 4, InventoryFreshnessPolicy::LiveOnly);
        assert_eq!(
            (launch.available, launch.executable, launch.reason),
            (
                0,
                false,
                Some(InventoryAvailabilityReason::RememberedInventory)
            )
        );
    }

    #[test]
    fn package_reservations_are_atomic_and_release_all_claims() {
        let actor = actor();
        let mut knowledge = knowledge();
        knowledge
            .storage_by_player
            .get_mut("player-1")
            .unwrap()
            .insert("poi-station".into(), HashMap::from([("ore".into(), 10)]));
        let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
        metadata.storage_fetched_at_by_key.insert(
            player_station_storage_key("player-1", "poi-station"),
            std::time::Instant::now(),
        );
        let index = InventoryIndex::project_canonical_with_metadata(
            &knowledge,
            &metadata,
            &[(Uuid::nil(), 1, actor.clone())],
        );
        let claim = |quantity| RuntimeInventoryClaimDto {
            lot_id: None,
            source_kind: "personal_storage".into(),
            owner_id: "player-1".into(),
            location_id: "base-station".into(),
            item_id: "ore".into(),
            quantity,
        };
        let request = |quantity| RuntimeInventoryMovementReserveRequest {
            session_id: "bot-1".into(),
            kind: "logistics".into(),
            claims: vec![claim(quantity)],
            virtual_order_uses: Vec::new(),
            context: serde_json::Value::Null,
        };
        let mut ledger = InventoryReservationLedger::default();
        let first =
            ledger.reserve_canonical(&index, &actor, &knowledge.galaxy, Uuid::nil(), request(7));
        assert!(first.accepted);
        let movement_id = first.movement.unwrap().movement_id;
        let second =
            ledger.reserve_canonical(&index, &actor, &knowledge.galaxy, Uuid::nil(), request(4));
        assert!(!second.accepted);
        assert_eq!(ledger.movements().len(), 1);
        ledger
            .transition(movement_id, RuntimeInventoryMovementStatusDto::Released)
            .unwrap();
        assert!(
            ledger
                .reserve_canonical(&index, &actor, &knowledge.galaxy, Uuid::nil(), request(4))
                .accepted
        );
    }

    #[test]
    fn reservation_requires_live_faction_observation() {
        let actor = actor();
        let remembered = knowledge();
        let claim = RuntimeInventoryClaimDto {
            lot_id: None,
            source_kind: "faction_storage".into(),
            owner_id: "faction-1".into(),
            location_id: "base-station".into(),
            item_id: "ore".into(),
            quantity: 1,
        };
        let request = |claim| RuntimeInventoryMovementReserveRequest {
            session_id: "bot-1".into(),
            kind: "logistics".into(),
            claims: vec![claim],
            virtual_order_uses: Vec::new(),
            context: serde_json::Value::Null,
        };
        let mut ledger = InventoryReservationLedger::default();
        let index =
            InventoryIndex::project_canonical(&remembered, &[(Uuid::nil(), 1, actor.clone())]);
        assert!(
            !ledger
                .reserve_canonical(
                    &index,
                    &actor,
                    &remembered.galaxy,
                    Uuid::nil(),
                    request(claim.clone())
                )
                .accepted
        );

        let mut metadata = prayer_runtime::knowledge::WorldRuntimeMetadata::default();
        metadata.faction_storage_fetched_at_by_key.insert(
            faction_station_storage_key("faction-1", "poi-station"),
            std::time::Instant::now(),
        );
        let index = InventoryIndex::project_canonical_with_metadata(
            &remembered,
            &metadata,
            &[(Uuid::nil(), 1, actor.clone())],
        );
        assert!(
            ledger
                .reserve_canonical(
                    &index,
                    &actor,
                    &remembered.galaxy,
                    Uuid::nil(),
                    request(claim)
                )
                .accepted
        );
    }

    #[test]
    fn market_depth_claims_compete_on_one_canonical_key() {
        let actor = actor();
        let mut knowledge = knowledge();
        knowledge.station_markets.insert(
            "poi-station".into(),
            prayer_state::StationMarketData {
                sell_orders: HashMap::from([(
                    "ore".into(),
                    vec![prayer_state::MarketOrder {
                        price_each: 5,
                        quantity: 10,
                        source: None,
                        my_quantity: None,
                    }],
                )]),
                ..Default::default()
            },
        );
        let index =
            InventoryIndex::project_canonical(&knowledge, &[(Uuid::nil(), 1, actor.clone())]);
        let request = |quantity| RuntimeInventoryMovementReserveRequest {
            session_id: "bot-1".into(),
            kind: "arbitrage".into(),
            claims: vec![RuntimeInventoryClaimDto {
                lot_id: Some("market|market|poi-station|ore".into()),
                source_kind: "market".into(),
                owner_id: "market".into(),
                location_id: "base-station".into(),
                item_id: "ore".into(),
                quantity,
            }],
            virtual_order_uses: Vec::new(),
            context: serde_json::Value::Null,
        };
        let mut ledger = InventoryReservationLedger::default();
        let first =
            ledger.reserve_canonical(&index, &actor, &knowledge.galaxy, Uuid::nil(), request(7));
        assert!(first.accepted);
        assert!(
            !ledger
                .reserve_canonical(&index, &actor, &knowledge.galaxy, Uuid::nil(), request(4))
                .accepted
        );
        let movement_id = first.movement.unwrap().movement_id;
        ledger
            .transition(movement_id, RuntimeInventoryMovementStatusDto::Running)
            .unwrap();
        ledger
            .transition(movement_id, RuntimeInventoryMovementStatusDto::Completed)
            .unwrap();
        assert!(
            ledger
                .reserve_canonical(&index, &actor, &knowledge.galaxy, Uuid::nil(), request(4))
                .accepted
        );
        ledger
            .transition(movement_id, RuntimeInventoryMovementStatusDto::Released)
            .unwrap();
    }

    #[test]
    fn active_market_reservations_reduce_projected_depth_until_terminal() {
        let actor = actor();
        let mut knowledge = knowledge();
        let physical = |price_each, quantity| prayer_state::MarketOrder {
            price_each,
            quantity,
            source: None,
            my_quantity: None,
        };
        knowledge.station_markets.insert(
            "poi-station".into(),
            prayer_state::StationMarketData {
                sell_orders: HashMap::from([("ore".into(), vec![physical(5, 4), physical(6, 6)])]),
                buy_orders: HashMap::from([(
                    "ore".into(),
                    vec![
                        physical(10, 5),
                        physical(9, 7),
                        prayer_state::MarketOrder {
                            price_each: 11,
                            quantity: 3,
                            source: Some("virtual_faction:vf-buy".into()),
                            my_quantity: None,
                        },
                    ],
                )]),
                ..Default::default()
            },
        );
        let index =
            InventoryIndex::project_canonical(&knowledge, &[(Uuid::nil(), 1, actor.clone())]);
        let request = RuntimeInventoryMovementReserveRequest {
            session_id: "bot-1".into(),
            kind: "arbitrage".into(),
            claims: vec![
                RuntimeInventoryClaimDto {
                    lot_id: Some("market|market|poi-station|ore".into()),
                    source_kind: "market".into(),
                    owner_id: "market".into(),
                    location_id: "poi-station".into(),
                    item_id: "ore".into(),
                    quantity: 7,
                },
                RuntimeInventoryClaimDto {
                    lot_id: Some("market_bid|market|poi-station|ore".into()),
                    source_kind: "market_bid".into(),
                    owner_id: "market".into(),
                    location_id: "poi-station".into(),
                    item_id: "ore".into(),
                    quantity: 12,
                },
            ],
            virtual_order_uses: Vec::new(),
            context: serde_json::Value::Null,
        };
        let mut ledger = InventoryReservationLedger::default();
        let outcome =
            ledger.reserve_canonical(&index, &actor, &knowledge.galaxy, Uuid::nil(), request);
        assert!(outcome.accepted);

        let mut market = MarketData {
            station_markets: knowledge.station_markets.clone(),
            ..MarketData::default()
        };
        ledger.apply_market_reservations(&mut market);
        let station = &market.station_markets["poi-station"];
        assert_eq!(station.sell_orders["ore"].len(), 1);
        assert_eq!(station.sell_orders["ore"][0].quantity, 3);
        assert_eq!(station.buy_orders["ore"].len(), 1);
        assert_eq!(
            station.buy_orders["ore"][0].source.as_deref(),
            Some("virtual_faction:vf-buy")
        );
        assert_eq!(station.buy_orders["ore"][0].quantity, 3);

        ledger
            .transition(
                outcome.movement.unwrap().movement_id,
                RuntimeInventoryMovementStatusDto::Released,
            )
            .unwrap();
        let mut released = MarketData {
            station_markets: knowledge.station_markets,
            ..MarketData::default()
        };
        ledger.apply_market_reservations(&mut released);
        assert_eq!(
            released.station_markets["poi-station"].sell_orders["ore"][0].quantity,
            4
        );
        assert_eq!(
            released.station_markets["poi-station"].buy_orders["ore"][0].quantity,
            5
        );
    }
}
