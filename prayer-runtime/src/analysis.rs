//! Runtime game-state adapter for PrayerLang analysis.

use crate::read_context::ExecutionReadContext;

use prayer_lang::AnalysisObservation;

/// Project live runtime state into the narrow model accepted by `prayer-lang`.
pub fn analysis_observation(context: Option<ExecutionReadContext<'_>>) -> AnalysisObservation {
    let default_bot = crate::BotState::default();
    let default_world = crate::read_context::WorldReadState::default();
    let bot = context.map(|context| context.bot).unwrap_or(&default_bot);
    let world = context
        .map(|context| context.world)
        .unwrap_or(&default_world);
    let mut item_ids = bot.cargo.keys().cloned().collect::<Vec<_>>();
    item_ids.extend(world.catalog.items.keys().cloned());
    item_ids.extend(
        world
            .storage
            .values()
            .flat_map(|items| items.keys().cloned()),
    );

    let mut poi_ids = world.galaxy.poi_records.keys().cloned().collect::<Vec<_>>();
    poi_ids.extend(
        world
            .galaxy
            .poi_records
            .values()
            .filter_map(|poi| poi.info.base_id.clone()),
    );
    poi_ids.extend(world.storage.keys().cloned());
    poi_ids.extend(
        [bot.player.home_poi.clone(), world.nearest_station.clone()]
            .into_iter()
            .flatten(),
    );

    let mut system_ids = world
        .galaxy
        .system_records
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    system_ids.extend(bot.location.system_id.iter().cloned());

    let owned_ship_ids = bot.owned_ship_ids().map(str::to_string).collect();
    let mut ship_ids = bot.owned_ship_ids().map(str::to_string).collect::<Vec<_>>();
    ship_ids.extend(
        world
            .faction_garage
            .ships
            .iter()
            .map(|ship| ship.ship.ship_id.clone()),
    );

    let mut module_ids = bot.installed_modules.iter().cloned().collect::<Vec<_>>();
    module_ids.extend(world.catalog.items.iter().filter_map(|(id, entry)| {
        let module_type = entry.module_type()?.trim().to_lowercase();
        matches!(module_type.as_str(), "weapon" | "defense" | "utility").then(|| id.clone())
    }));

    AnalysisObservation {
        system: bot.location.system_id.clone(),
        item_ids,
        poi_ids,
        system_ids,
        mission_ids: bot
            .missions
            .active
            .iter()
            .chain(&bot.missions.available)
            .cloned()
            .collect(),
        ship_ids,
        owned_ship_ids,
        module_ids,
        recipe_ids: world.catalog.recipes.keys().cloned().collect(),
        listing_ids: world.market.shipyard_listings.clone(),
    }
}
