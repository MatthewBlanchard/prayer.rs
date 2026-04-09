//! Default command and predicate catalogs for DSL analysis/validation.

use std::collections::HashMap;

use crate::dsl::{ArgSpec, ArgType, CommandSpec, PredicateSpec};

/// Documentation strings for every built-in command.
///
/// Each entry is `(command_name, doc_string)`. The build script for `prayer-mcp`
/// asserts that every command in [`default_command_catalog`] has a non-empty entry
/// here — missing docs are a compile-time build error.
///
/// Order controls the order commands appear in the generated reference document.
pub const COMMAND_DOCS: &[(&str, &str)] = &[
    // Control
    ("halt", "Stop script execution immediately."),
    (
        "wait",
        "Pause execution for the given number of ticks (default 1, max 30). Each tick is ~10 seconds.",
    ),
    // Navigation
    (
        "go",
        "Navigate to a system, POI, or named destination. Supports $here, $home, $nearest_station.",
    ),
    (
        "dock",
        "Navigate to the nearest dockable POI in the current system and dock.",
    ),
    (
        "set_home",
        "Set your home base to the current docked location.",
    ),
    (
        "explore",
        "Navigate to the nearest unvisited POI in the galaxy and survey it.",
    ),
    (
        "survey",
        "Survey the current system, registering all POIs and resources.",
    ),
    // Mining
    (
        "mine",
        "Navigate to the nearest mining site and mine until cargo is full. Optionally filter by item type (e.g. iron_ore). Mining iron_ore might give you other ores too. Use stash; when mining.",
    ),
    // Missions
    (
        "accept_mission",
        "Accept a mission offer by ID.",
    ),
    (
        "abandon_mission",
        "Abandon an active mission, forfeiting any progress.",
    ),
    (
        "decline_mission",
        "Decline a pending mission offer by template ID.",
    ),
    (
        "complete_mission",
        "Turn in a completed mission for its rewards.",
    ),
    // Cargo
    (
        "sell",
        "Create sell orders at the current station. Omit item to sell all cargo.",
    ),
    (
        "buy",
        "Create a buy order for the specified item and quantity at the current station.",
    ),
    (
        "cancel_buy",
        "Cancel all open buy orders for the specified item.",
    ),
    (
        "cancel_sell",
        "Cancel all open sell orders for the specified item.",
    ),
    (
        "retrieve",
        "Withdraw an item from station storage into ship cargo. Omit quantity to retrieve all.",
    ),
    (
        "stash",
        "Deposit cargo into station storage. Omit item to deposit all cargo.",
    ),
    (
        "jettison",
        "Jettison cargo as a floating container at your location. Omit item to jettison all cargo.",
    ),
    (
        "use_item",
        "Use a consumable item such as a repair kit, shield cell, or emergency warp. Quantity defaults to 1.",
    ),
    // Ship management
    ("repair", "Repair your ship's hull at the current location."),
    ("refuel", "Navigate to the nearest station and refuel your ship."),
    (
        "self_destruct",
        "Destroy your ship (triggers insurance payout if active).",
    ),
    (
        "switch_ship",
        "Switch your active ship to another in your fleet.",
    ),
    ("install_mod", "Install a module onto your ship."),
    ("uninstall_mod", "Uninstall a module from your ship."),
    (
        "buy_ship",
        "Purchase a ship listing by ID. Alias for buy_listed_ship.",
    ),
    ("buy_listed_ship", "Purchase a ship listing by ID."),
    (
        "commission_ship",
        "Commission a new ship of the given class to be built.",
    ),
    ("sell_ship", "Sell a ship from your fleet."),
    (
        "list_ship_for_sale",
        "List a ship for sale at the specified price.",
    ),
    // Crafting
    (
        "craft",
        "Craft items using a recipe. Count defaults to 1.",
    ),
    // Wrecks
    ("salvage_wreck", "Salvage a wreck for components."),
    ("tow_wreck", "Tow a wreck to your current location."),
    (
        "loot_wreck",
        "Loot a specific item and quantity from a wreck.",
    ),
    (
        "scrap_wreck",
        "Scrap the wreck you are currently towing for materials.",
    ),
    ("sell_wreck", "Sell the wreck you are currently towing."),
    // Misc
    (
        "distress_signal",
        "Broadcast a distress signal to nearby players. Type is one of: fuel, repair, combat.",
    ),
];

/// Build the default command catalog.
pub fn default_command_catalog() -> HashMap<String, CommandSpec> {
    let mut commands = HashMap::new();

    commands.insert("halt".to_string(), command("halt", vec![]));

    commands.insert(
        "mine".to_string(),
        command("mine", vec![arg("resource", ArgType::ItemId, false)]),
    );
    commands.insert("survey".to_string(), command("survey", vec![]));
    commands.insert("explore".to_string(), command("explore", vec![]));
    commands.insert(
        "go".to_string(),
        command("go", vec![arg("destination", ArgType::GoTarget, true)]),
    );
    commands.insert(
        "accept_mission".to_string(),
        command(
            "accept_mission",
            vec![arg("mission_id", ArgType::MissionId, true)],
        ),
    );
    commands.insert(
        "abandon_mission".to_string(),
        command(
            "abandon_mission",
            vec![arg("mission_id", ArgType::MissionId, true)],
        ),
    );
    commands.insert(
        "decline_mission".to_string(),
        command(
            "decline_mission",
            vec![arg("template_id", ArgType::MissionId, true)],
        ),
    );
    commands.insert(
        "complete_mission".to_string(),
        command(
            "complete_mission",
            vec![arg("mission_id", ArgType::MissionId, true)],
        ),
    );
    commands.insert("dock".to_string(), command("dock", vec![]));
    commands.insert("set_home".to_string(), command("set_home", vec![]));
    commands.insert("repair".to_string(), command("repair", vec![]));
    commands.insert("refuel".to_string(), command("refuel", vec![]));
    commands.insert(
        "self_destruct".to_string(),
        command("self_destruct", vec![]),
    );
    commands.insert(
        "sell".to_string(),
        command("sell", vec![arg("item", ArgType::ItemId, false)]),
    );
    commands.insert(
        "buy".to_string(),
        command(
            "buy",
            vec![
                arg("item", ArgType::ItemId, true),
                arg("quantity", ArgType::Integer, true),
            ],
        ),
    );
    commands.insert(
        "cancel_buy".to_string(),
        command("cancel_buy", vec![arg("item", ArgType::ItemId, true)]),
    );
    commands.insert(
        "cancel_sell".to_string(),
        command("cancel_sell", vec![arg("item", ArgType::ItemId, true)]),
    );
    commands.insert(
        "retrieve".to_string(),
        command(
            "retrieve",
            vec![
                arg("item", ArgType::ItemId, true),
                arg("quantity", ArgType::Integer, false),
            ],
        ),
    );
    commands.insert(
        "stash".to_string(),
        command("stash", vec![arg("item", ArgType::ItemId, false)]),
    );
    commands.insert(
        "jettison".to_string(),
        command("jettison", vec![arg("item", ArgType::ItemId, false)]),
    );
    commands.insert(
        "use_item".to_string(),
        command(
            "use_item",
            vec![
                arg("item_id", ArgType::ItemId, true),
                arg("quantity", ArgType::Integer, false),
            ],
        ),
    );
    commands.insert(
        "switch_ship".to_string(),
        command("switch_ship", vec![arg("ship", ArgType::ShipId, true)]),
    );
    commands.insert(
        "install_mod".to_string(),
        command("install_mod", vec![arg("mod", ArgType::ModuleId, true)]),
    );
    commands.insert(
        "uninstall_mod".to_string(),
        command("uninstall_mod", vec![arg("mod", ArgType::ModuleId, true)]),
    );
    commands.insert(
        "buy_ship".to_string(),
        command("buy_ship", vec![arg("listing", ArgType::ListingId, true)]),
    );
    commands.insert(
        "buy_listed_ship".to_string(),
        command(
            "buy_listed_ship",
            vec![arg("listing", ArgType::ListingId, true)],
        ),
    );
    commands.insert(
        "commission_ship".to_string(),
        command(
            "commission_ship",
            vec![arg("ship_class", ArgType::Any, true)],
        ),
    );
    commands.insert(
        "sell_ship".to_string(),
        command("sell_ship", vec![arg("ship", ArgType::ShipId, true)]),
    );
    commands.insert(
        "list_ship_for_sale".to_string(),
        command(
            "list_ship_for_sale",
            vec![
                arg("ship", ArgType::ShipId, true),
                arg("price", ArgType::Integer, true),
            ],
        ),
    );
    commands.insert(
        "wait".to_string(),
        command("wait", vec![arg("ticks", ArgType::Integer, false)]),
    );
    commands.insert(
        "craft".to_string(),
        command(
            "craft",
            vec![
                arg("recipe_id", ArgType::RecipeId, true),
                arg("count", ArgType::Integer, false),
            ],
        ),
    );
    commands.insert(
        "salvage_wreck".to_string(),
        command("salvage_wreck", vec![arg("wreck_id", ArgType::Any, true)]),
    );
    commands.insert(
        "tow_wreck".to_string(),
        command("tow_wreck", vec![arg("wreck_id", ArgType::Any, true)]),
    );
    commands.insert(
        "loot_wreck".to_string(),
        command(
            "loot_wreck",
            vec![
                arg("wreck_id", ArgType::Any, true),
                arg("item_id", ArgType::ItemId, true),
                arg("quantity", ArgType::Integer, true),
            ],
        ),
    );
    commands.insert("scrap_wreck".to_string(), command("scrap_wreck", vec![]));
    commands.insert("sell_wreck".to_string(), command("sell_wreck", vec![]));
    commands.insert(
        "distress_signal".to_string(),
        command(
            "distress_signal",
            vec![arg("distress_type", ArgType::Any, false)],
        ),
    );

    commands
}

fn command(name: &str, args: Vec<ArgSpec>) -> CommandSpec {
    CommandSpec {
        name: name.to_string(),
        args,
    }
}

fn arg(name: &str, kind: ArgType, required: bool) -> ArgSpec {
    ArgSpec {
        name: name.to_string(),
        kind,
        required,
    }
}

/// Build the default boolean/numeric predicate catalogs.
pub(crate) fn default_predicate_catalog() -> (
    HashMap<String, PredicateSpec>,
    HashMap<String, PredicateSpec>,
) {
    let mut numeric = HashMap::new();
    for (name, arity) in [
        ("FUEL", 0usize),
        ("CREDITS", 0usize),
        ("CARGO_PCT", 0usize),
        ("CARGO", 1usize),
        ("MINED", 1usize),
        ("STASHED", 1usize),
        ("STASH", 2usize),
    ] {
        numeric.insert(
            name.to_string(),
            PredicateSpec {
                name: name.to_string(),
                arity,
            },
        );
    }

    (HashMap::new(), numeric)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_takes_optional_integer_ticks() {
        let catalog = default_command_catalog();
        let wait = catalog.get("wait").expect("wait command");
        assert_eq!(wait.args.len(), 1);
        assert_eq!(wait.args[0].name, "ticks");
        assert_eq!(wait.args[0].kind, ArgType::Integer);
        assert!(!wait.args[0].required);
    }

    #[test]
    fn stash_predicate_is_registered_with_arity_two() {
        let (_, numeric) = default_predicate_catalog();
        let stash = numeric.get("STASH").expect("stash predicate");
        assert_eq!(stash.arity, 2);
    }

    #[test]
    fn default_command_catalog_contains_core_runtime_commands() {
        let catalog = default_command_catalog();
        assert!(catalog.contains_key("mine"));
        assert!(catalog.contains_key("go"));
        assert!(catalog.contains_key("accept_mission"));
        assert!(catalog.contains_key("list_ship_for_sale"));
        assert!(catalog.contains_key("craft"));
        assert!(catalog.contains_key("halt"));
    }

    #[test]
    fn sell_and_stash_item_args_are_optional() {
        let catalog = default_command_catalog();
        let sell = catalog.get("sell").expect("sell command");
        assert_eq!(sell.args.len(), 1);
        assert!(!sell.args[0].required);

        let stash = catalog.get("stash").expect("stash command");
        assert_eq!(stash.args.len(), 1);
        assert!(!stash.args[0].required);
    }

    #[test]
    fn every_catalog_command_has_a_doc_entry() {
        let catalog = default_command_catalog();
        let docs: std::collections::HashSet<&str> =
            COMMAND_DOCS.iter().map(|(name, _)| *name).collect();
        let mut missing: Vec<&str> = catalog
            .keys()
            .filter(|name| !docs.contains(name.as_str()))
            .map(String::as_str)
            .collect();
        missing.sort();
        assert!(
            missing.is_empty(),
            "Commands missing from COMMAND_DOCS: {missing:?}"
        );
    }
}
