//! Compile-time example: traverse rich alpha state directly.

use prayer_sdk::prelude::*;

fn inspect_bot(bot: &BotState) -> (Option<&str>, Option<&str>, Option<&str>, usize) {
    (
        bot.player.username.as_deref(),
        bot.location.system_id.as_deref(),
        bot.ship.class_id.as_deref(),
        bot.cargo_items.len(),
    )
}

fn inspect_world(world: &Galaxy, catalog: &Catalog) -> (usize, usize, usize) {
    (
        world.system_records.len(),
        world.poi_records.len(),
        catalog.items.len(),
    )
}

fn main() {
    let bot = BotState::default();
    let world = Galaxy::default();
    let catalog = Catalog::default();
    let _ = (inspect_bot(&bot), inspect_world(&world, &catalog));
}
