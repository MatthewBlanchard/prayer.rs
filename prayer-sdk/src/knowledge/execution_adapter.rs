//! Focused actor-relative reads over canonical shared world state.

use super::*;

pub fn world_read_state(
    knowledge: &WorldState,
    actor: &BotState,
) -> prayer_runtime::read_context::WorldReadState {
    world_read_state_with_metadata(knowledge, &Default::default(), actor)
}

pub fn world_read_state_with_metadata(
    knowledge: &WorldState,
    metadata: &prayer_runtime::knowledge::WorldRuntimeMetadata,
    actor: &BotState,
) -> prayer_runtime::read_context::WorldReadState {
    let navigation = ActorNavigationRead::new(actor, Arc::clone(&knowledge.galaxy));
    let mut world = prayer_runtime::read_context::WorldReadState {
        catalog: Arc::clone(&knowledge.catalog),
        galaxy: Arc::clone(&knowledge.galaxy),
        nearest_station: nearest_station_poi(&navigation),
        ..prayer_runtime::read_context::WorldReadState::default()
    };
    for player_key in storage_player_keys_for_actor(actor) {
        if let Some(storage) = knowledge.storage_by_player.get(player_key) {
            world.storage = Arc::new(storage.clone());
            break;
        }
    }
    if let Some(faction_key) = faction_storage_key_for_actor(actor) {
        if let Some(storage) = actor.location.poi_id.as_deref().and_then(|poi| {
            knowledge
                .faction_storage_by_faction_poi
                .get(faction_key)
                .and_then(|by_poi| by_poi.get(poi))
        }) {
            world.faction_storage = Some(Arc::new(storage.clone()));
        }
    }
    if let Some(faction_key) = faction_garage_key_for_actor(actor) {
        if let Some(garage) = knowledge.faction_garage_by_faction.get(faction_key) {
            world.faction_garage = garage.clone();
        }
    }

    let mut market = MarketData {
        shipyard_listings: knowledge.shipyard_listing_ids.clone(),
        station_markets: knowledge.station_markets.clone(),
        ..MarketData::default()
    };
    overlay_virtual_market_orders_for_actor(knowledge, actor, &mut market);
    lift_current_station_orders(
        actor.location.docked_at.is_some(),
        actor.location.poi_id.as_deref(),
        &mut market,
    );
    world.market = Arc::new(market);
    if let Some(station_id) = actor.location.poi_id.as_deref() {
        if let Some(station_passengers) = knowledge.station_passengers.get(station_id) {
            world.station_passengers = station_passengers.clone();
        }
    }
    world.system_agents = Arc::new(
        actor
            .effective_system_id()
            .and_then(|system| knowledge.system_agents_by_system.get(system))
            .cloned()
            .unwrap_or_default(),
    );
    world.wildlife_by_poi = Arc::new(knowledge.wildlife_by_poi.clone());
    if let Some(poi_id) = actor.location.poi_id.as_deref() {
        if let Some(snapshot) = knowledge.wildlife_by_poi.get(poi_id) {
            world.nearby_creature_count = Some(snapshot.creature_count);
        }
    }
    world.managed_players = Arc::new(knowledge.managed_players.clone());

    let mut salvage = SalvageData::default();
    for (poi_id, snapshot) in &knowledge.salvage_by_poi {
        if salvage_snapshot_fresh(metadata, poi_id) {
            salvage
                .lootables_by_poi
                .insert(poi_id.clone(), snapshot.visible_lootables.clone());
        }
    }
    if let Some(poi_id) = actor.location.poi_id.as_deref() {
        if let Some(snapshot) = knowledge
            .salvage_by_poi
            .get(poi_id)
            .filter(|_| salvage_snapshot_fresh(metadata, poi_id))
        {
            salvage.visible_lootables = snapshot.visible_lootables.clone();
            salvage.last_seen_poi = snapshot.last_seen_poi.clone();
            salvage.last_seen_system = snapshot.last_seen_system.clone();
            salvage.observed_at_unix = snapshot.observed_at_unix;
        }
    }
    world.salvage = Arc::new(salvage);
    world
}

#[cfg(test)]
pub struct ExecutionTestState {
    pub bot: BotState,
    pub world: prayer_runtime::read_context::WorldReadState,
}

#[cfg(test)]
pub fn execution_state(knowledge: &WorldState, actor: &BotState) -> ExecutionTestState {
    execution_state_with_metadata(knowledge, &Default::default(), actor)
}

#[cfg(test)]
pub fn execution_state_with_metadata(
    knowledge: &WorldState,
    metadata: &prayer_runtime::knowledge::WorldRuntimeMetadata,
    actor: &BotState,
) -> ExecutionTestState {
    ExecutionTestState {
        bot: actor.clone(),
        world: world_read_state_with_metadata(knowledge, metadata, actor),
    }
}

/// Promote the docked station's order book into the market's top-level
/// `buy_orders`/`sell_orders`, which callers read as "the current station's
/// prices". No-op while undocked or when the station has no known snapshot.
pub fn lift_current_station_orders(
    docked: bool,
    current_poi: Option<&str>,
    market: &mut MarketData,
) {
    if !docked {
        return;
    }
    if let Some(snapshot) =
        current_poi.and_then(|station_id| market.station_markets.get(station_id))
    {
        market.buy_orders = snapshot.buy_orders.clone();
        market.sell_orders = snapshot.sell_orders.clone();
    }
}

pub fn overlay_virtual_market_orders_for_actor(
    knowledge: &WorldState,
    actor: &BotState,
    market: &mut MarketData,
) {
    for order in &knowledge.virtual_orders {
        let side = order.side.trim();
        if !order.enabled
            || order.internal_only
            || order.id.trim().is_empty()
            || !matches!(side, "buy" | "sell" | "buy_until" | "sell_until")
            || order.item_id.trim().is_empty()
            || order.station_id.trim().is_empty()
            || order.price_each <= 0
            || order.quantity <= 0
            || (!knowledge.galaxy.poi_records.contains_key(&order.station_id)
                && knowledge
                    .galaxy
                    .poi_id_for_base(&order.station_id)
                    .is_none())
        {
            continue;
        }
        let quantity = virtual_market_order_open_quantity_for_actor(knowledge, actor, order);
        if quantity <= 0 {
            continue;
        }
        let entry = MarketOrder {
            price_each: order.price_each,
            quantity,
            source: Some(format!("virtual_faction:{}", order.id)),
            my_quantity: None,
        };
        let snapshot = market
            .station_markets
            .entry(order.station_id.clone())
            .or_default();
        if snapshot.observed_at_unix.is_none() {
            snapshot.observed_at_unix = Some(Utc::now().timestamp());
        }
        match side {
            "sell" | "sell_until" => snapshot
                .sell_orders
                .entry(order.item_id.clone())
                .or_default()
                .push(entry),
            "buy" | "buy_until" => snapshot
                .buy_orders
                .entry(order.item_id.clone())
                .or_default()
                .push(entry),
            _ => {}
        }
    }
}

pub fn virtual_market_order_open_quantity_for_actor(
    knowledge: &WorldState,
    actor: &BotState,
    order: &RuntimeVirtualMarketOrderDto,
) -> i64 {
    let fixed_available = order
        .quantity
        .saturating_sub(order.reserved.max(0))
        .saturating_sub(order.filled.max(0));
    let storage_quantity = known_faction_storage_at_station_quantity_for_actor(
        knowledge,
        actor,
        &order.station_id,
        &order.item_id,
    );
    match order.side.as_str() {
        "sell" => storage_quantity.map_or(fixed_available, |storage| fixed_available.min(storage)),
        "buy" => fixed_available,
        "buy_until" => {
            let shortfall = order
                .quantity
                .saturating_sub(storage_quantity.unwrap_or(0).max(0));
            shortfall.saturating_sub(order.reserved.max(0))
        }
        "sell_until" => {
            let excess = storage_quantity
                .unwrap_or(0)
                .max(0)
                .saturating_sub(order.quantity);
            if let Some(tipping_point) = order.tipping_point.filter(|point| *point > 0) {
                if excess <= 0 {
                    return 0;
                }
                if !order.dumping && excess < tipping_point {
                    return 0;
                }
            }
            excess.saturating_sub(order.reserved.max(0))
        }
        _ => 0,
    }
}

pub fn known_faction_storage_at_station_quantity_for_actor(
    knowledge: &WorldState,
    actor: &BotState,
    station_id: &str,
    item_id: &str,
) -> Option<i64> {
    let faction = actor.player.faction_id.as_deref()?.trim();
    if faction.is_empty() {
        return None;
    }
    let poi = knowledge
        .galaxy
        .poi_id_for_base(station_id)
        .unwrap_or(station_id);
    let by_poi = knowledge.faction_storage_by_faction_poi.get(faction)?;
    by_poi
        .get(poi)
        .or_else(|| by_poi.get(station_id))?
        .get(item_id)
        .copied()
}
