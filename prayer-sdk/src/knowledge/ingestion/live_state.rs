//! Live-session state preservation and effective-state application.

use super::super::*;

pub fn carry_station_mission_board_forward(previous: &BotState, fetched: &mut BotState) {
    if fetched.location.docked_at.is_none()
        || fetched.location.poi_id.is_none()
        || fetched.location.poi_id != previous.location.poi_id
    {
        return;
    }

    let previous_missions = previous.missions.as_ref();
    if previous_missions.available.is_empty() && previous_missions.available_details.is_empty() {
        return;
    }

    let fetched_missions = fetched.missions.as_ref();
    if !fetched_missions.available.is_empty() || !fetched_missions.available_details.is_empty() {
        return;
    }

    let mut missions = fetched_missions.clone();
    missions.available = previous_missions.available.clone();
    missions.available_details = previous_missions.available_details.clone();
    fetched.missions = Arc::new(missions);
}

pub fn carry_station_crafting_queue_forward(previous: &BotState, fetched: &mut BotState) {
    if fetched.location.docked_at.is_none()
        || fetched.location.poi_id.is_none()
        || fetched.location.poi_id != previous.location.poi_id
    {
        return;
    }

    if previous.crafting_queue.is_empty() || !fetched.crafting_queue.is_empty() {
        return;
    }

    fetched.crafting_queue = Arc::clone(&previous.crafting_queue);
}

pub fn carry_passengers_forward(
    previous: &BotState,
    fetched: &mut BotState,
    passengers_fetched: bool,
    _docked_passengers_fetched: bool,
) {
    if !passengers_fetched {
        fetched.passengers.aboard_count = previous.passengers.aboard_count;
        fetched.passengers.economy_berths = previous.passengers.economy_berths.clone();
        fetched.passengers.business_berths = previous.passengers.business_berths.clone();
        fetched.passengers.first_berths = previous.passengers.first_berths.clone();
        fetched.passengers.aboard = Arc::clone(&previous.passengers.aboard);
    }
}

pub fn canonicalize_owned_ship_active_from_status(state: &mut BotState) {
    let active_ship_id = state.ship.id.as_deref().unwrap_or_default().trim();
    if active_ship_id.is_empty() || state.owned_ship_details.is_empty() {
        return;
    }

    let mut details = state.owned_ship_details.as_ref().clone();
    let mut found = false;
    for ship in &mut details {
        let is_active = ship.ship_id == active_ship_id;
        found |= is_active;
        ship.is_active = is_active;
        if is_active {
            ship.location = Some("Active ship".to_string());
        }
    }
    if !found {
        return;
    }

    details.sort_by(|a, b| {
        b.is_active
            .cmp(&a.is_active)
            .then_with(|| a.location.cmp(&b.location))
            .then_with(|| a.ship_id.cmp(&b.ship_id))
    });
    state.owned_ship_details = Arc::new(details);
}

#[cfg(test)]
pub fn apply_live_state(session: &mut SessionHandle, fetched: BotState, knowledge: &WorldState) {
    apply_live_state_inner(
        session, fetched, knowledge, false, false, false, false, false,
    );
}

pub fn preserve_core_status_from_previous(previous: &BotState, fetched: &mut BotState) {
    if fetched.location.system_id.is_none() {
        fetched.location.system_id = previous.location.system_id.clone();
    }
    if fetched.location.poi_id.is_none() {
        fetched.location.poi_id = previous.location.poi_id.clone();
    }
    if fetched.player.home_base.is_none() {
        fetched.player.home_base = previous.player.home_base.clone();
    }
    if fetched.player.home_poi.is_none() {
        fetched.player.home_poi = previous.player.home_poi.clone();
    }
    if fetched.player.home_system.is_none() {
        fetched.player.home_system = previous.player.home_system.clone();
    }
    if fetched.player.username.is_none() {
        fetched.player.username = previous.player.username.clone();
    }
    if fetched.player.id.is_none() {
        fetched.player.id = previous.player.id.clone();
    }
    if fetched.player.empire.is_none() {
        fetched.player.empire = previous.player.empire.clone();
    }
    if fetched.player.clan_tag.is_none() {
        fetched.player.clan_tag = previous.player.clan_tag.clone();
    }
    if fetched.player.status_message.is_none() {
        fetched.player.status_message = previous.player.status_message.clone();
    }
    if fetched.player.primary_color.is_none() {
        fetched.player.primary_color = previous.player.primary_color.clone();
    }
    if fetched.player.secondary_color.is_none() {
        fetched.player.secondary_color = previous.player.secondary_color.clone();
    }
    if fetched.player.is_cloaked.is_none() {
        fetched.player.is_cloaked = previous.player.is_cloaked;
    }
    if fetched.location.docked_at.is_none() && previous.location.docked_at.is_some() {
        fetched.location.docked_at = previous.location.docked_at.clone();
    }
    if fetched.player.credits.is_none() {
        fetched.player.credits = previous.player.credits;
    }
    if fetched.max_fuel == 0 && previous.max_fuel != 0 {
        fetched.fuel_pct = previous.fuel_pct;
        fetched.fuel = previous.fuel;
        fetched.max_fuel = previous.max_fuel;
    }
    if fetched.cargo_capacity == 0 && previous.cargo_capacity != 0 {
        fetched.cargo_pct = previous.cargo_pct;
        fetched.cargo_used = previous.cargo_used;
        fetched.cargo_capacity = previous.cargo_capacity;
        fetched.cargo = Arc::clone(&previous.cargo);
        fetched.cargo_items = Arc::clone(&previous.cargo_items);
    }
    if fetched.ship == prayer_runtime::engine::ShipState::default() {
        fetched.ship = previous.ship.clone();
    }
    if fetched.installed_modules.is_empty() && !previous.installed_modules.is_empty() {
        fetched.installed_modules = Arc::clone(&previous.installed_modules);
    }
}

pub fn apply_live_state_inner(
    session: &mut SessionHandle,
    mut fetched: BotState,
    knowledge: &WorldState,
    docked_crafting_queue_fetched: bool,
    commission_status_fetched: bool,
    passengers_fetched: bool,
    docked_passengers_fetched: bool,
    preserve_core_status: bool,
) {
    let previous = session.actor.observed.clone();
    if preserve_core_status {
        preserve_core_status_from_previous(&previous, &mut fetched);
    }
    if fetched.owned_ship_details.is_empty()
        && !session.actor.observed.owned_ship_details.is_empty()
    {
        fetched.owned_ship_details = Arc::clone(&session.actor.observed.owned_ship_details);
    }
    if !commission_status_fetched {
        fetched.active_commissions = Arc::clone(&session.actor.observed.active_commissions);
    }
    canonicalize_owned_ship_active_from_status(&mut fetched);
    carry_station_mission_board_forward(&previous, &mut fetched);
    if !docked_crafting_queue_fetched {
        carry_station_crafting_queue_forward(&previous, &mut fetched);
    }
    carry_passengers_forward(
        &previous,
        &mut fetched,
        passengers_fetched,
        docked_passengers_fetched,
    );
    session.actor.observed = Arc::new(fetched);
    session.actor.observation.observed_at_utc = Some(Utc::now());
    session.knowledge_version = knowledge.knowledge_version;
    session.has_state = true;
}
