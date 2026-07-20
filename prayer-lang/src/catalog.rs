//! Default executable command catalog for PrayerLang analysis and validation.

use std::collections::HashMap;

use crate::{ArgSpec, ArgType, CommandSpec};

/// Documentation strings for every built-in command.
///
/// Each entry is `(command_name, doc_string)`.
/// asserts that every command in [`default_command_catalog`] has a non-empty entry
/// here — missing docs are a compile-time build error.
///
/// Order controls the order commands appear in the generated reference document.
pub const COMMAND_DOCS: &[(&str, &str)] = &[
    ("cloak", "Enable cloaking, or pass off to disable it."),
    ("hunt", "Hunt a wildlife target by ID."),
    ("prepay_tax", "Prepay personal tax credits."),
    ("cancel_craft_job", "Cancel a queued craft or recycle job."),
    ("refit_ship", "Refit the active ship to its latest class specification."),
    ("cancel_commission", "Cancel a ship commission by ID."),
    ("supply_commission", "Supply an item quantity to a ship commission."),
    ("cancel_ship_listing", "Cancel a ship sale listing."),
    ("place_ship_buy_order", "Place a ship-class buy order at a price."),
    ("cancel_ship_buy_order", "Cancel a ship buy order."),
    ("sell_ship_to_order", "Sell a stored ship into a ship buy order."),
    ("release_wreck", "Release the currently towed wreck."),
    ("insure_ship", "Insure the active ship for a number of ticks."),
    ("citizenship_apply", "Apply for citizenship with an empire."),
    ("citizenship_withdraw", "Withdraw a pending citizenship application."),
    ("citizenship_renounce", "Renounce an empire citizenship."),
    ("trade_accept", "Accept a player trade offer by ID."),
    ("trade_offer", "Offer and request structured item quantities or credits with another player."),
    ("faction_leave", "Leave the current faction."),
    ("faction_withdraw_invite", "Withdraw a faction invitation."),
    ("faction_propose_ally", "Propose an alliance with a faction."),
    ("faction_accept_ally", "Accept an alliance proposal."),
    ("faction_remove_ally", "Dissolve a faction alliance."),
    ("faction_declare_war", "Declare war on a faction with an optional reason."),
    ("faction_propose_peace", "Propose peace with an optional message."),
    ("faction_accept_peace", "Accept a faction peace proposal."),
    ("faction_set_enemy", "Mark a faction as an enemy."),
    ("faction_remove_enemy", "Return an enemy faction to neutral."),
    ("faction_prepay_tax", "Prepay corporate tax from the faction treasury."),
    ("faction_cancel_mission", "Cancel a posted faction mission."),
    ("espionage", "Gather intelligence at the current station."),
    ("scan_poi", "Run a long-range faction intel scan of a POI."),
    // Navigation
    (
        "go",
        "Navigate to an explicit system or POI identifier selected by the client.",
    ),
    (
        "dock",
        "Navigate to the nearest dockable POI in the current system and dock.",
    ),
    (
        "set_home",
        "Set your home base to the current docked location.",
    ),
    ("undock", "Undock the active ship from its current location."),
    (
        "find",
        "Find something in the galaxy. With no args, explores normally. With resource or POI ids, explores until any target location is known.",
    ),
    (
        "survey",
        "Survey the current system, registering all POIs and resources.",
    ),
    (
        "attack",
        "Force an attack against an explicit target id, bypassing targeting policy selection.",
    ),
    (
        "scan",
        "Refresh combat/battle context for a target id.",
    ),
    // Mining
    (
        "mine",
        "Navigate to the nearest mining site and mine until cargo is full. Optionally filter by item type (e.g. iron_ore). Mining iron_ore might give you other ores too. Use transfer; when mining.",
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
    // Passengers
    (
        "load_passenger",
        "Load waiting passengers at the current station whose destination matches the given POI or base id.",
    ),
    (
        "unload_passenger",
        "Unload a passenger by name or citizen id at the current station. Pass `all` to unload everyone aboard.",
    ),
    // Cargo
    (
        "sell",
        "Create a sell order at the current station. Pass an item ID to sell that item, or omit to sell all cargo and current-station storage. \
        Pass an optional quantity cap for single-item sales, followed by an optional minimum price per unit to \
        only match buy orders at or above it; units with no qualifying bid are left in cargo/storage rather \
        than sold cheap. Add `order` as a fourth argument to place the requested sell order at that price \
        even when it will rest.",
    ),
    (
        "buy",
        "Create a buy order for the specified item and quantity at the current station. \
        Pass an optional maximum price per unit as a third argument to only match sell orders at or below it; \
        the quantity is trimmed to the units available at or under that price so the order never overpays. \
        Add `order` as a fourth argument to place the requested buy order at that price even when it will rest.",
    ),
    (
        "cancel_buy",
        "Cancel all open buy orders for the specified item.",
    ),
    (
        "cancel_sell",
        "Cancel all open sell orders for the specified item.",
    ),
    ("faction_create", "Create a faction with a unique name and tag."),
    ("faction_invite", "Invite a player to your faction by player ID or username."),
    ("faction_accept_invite", "Accept an invitation from the specified faction ID."),
    ("faction_kick", "Remove a player from your faction."),
    ("faction_set_role", "Assign a faction member the recruit, member, officer, or leader role."),
    // Facilities
    (
        "found_station",
        "Found a faction station with the given name; pass true to allow public docking or false to keep it private.",
    ),
    (
        "facility_build",
        "Build a personal facility of the given facility type at the current docked location.",
    ),
    (
        "faction_facility_build",
        "Build a faction facility of the given facility type at the current docked location.",
    ),
    (
        "facility_upgrade",
        "Upgrade a personal facility by facility ID to the given facility type.",
    ),
    (
        "faction_facility_upgrade",
        "Upgrade a faction facility by facility ID to the given facility type.",
    ),
    (
        "facility_dismantle",
        "Dismantle a personal facility by facility ID, when the facility type allows dismantling.",
    ),
    (
        "faction_facility_dismantle",
        "Dismantle a faction facility by facility ID, when the facility type allows dismantling.",
    ),
    (
        "facility_set_access",
        "Set a production facility's rental access to public or private.",
    ),
    (
        "facility_set_output_price",
        "Set a production facility's rental price for an output item.",
    ),
    (
        "facility_set_name",
        "Set or clear a custom name for a facility you own.",
    ),
    (
        "use_item",
        "Use a consumable item such as a repair kit, shield cell, or emergency warp. Quantity defaults to 1.",
    ),
    // Ship management
    ("repair", "Repair your ship's hull at the current location."),
    ("repair_module", "Repair a module instance currently in cargo."),
    ("recycle", "Recycle a recipe's output items. Quantity defaults to 1."),
    ("refuel", "Navigate to the nearest station and refuel your ship."),
    (
        "self_destruct",
        "Destroy your ship (triggers insurance payout if active).",
    ),
    (
        "switch_ship",
        "Switch your active ship to another in your fleet.",
    ),
    ("rename_ship", "Set or clear the active ship's custom name."),
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
        "scrap_ship",
        "Remotely scrap a parked ship by ship id.",
    ),
    (
        "list_ship_for_sale",
        "List a ship for sale at the specified price.",
    ),
    ("cancel_order", "Cancel a market order by ID, or pass all."),
    ("modify_order", "Change a market order's price by order ID."),
    // Crafting
    (
        "craft",
        "Craft items using a recipe. Count defaults to 1.",
    ),
    // Wrecks
    ("salvage_wreck", "Salvage a wreck for components."),
    ("tow_wreck", "Tow a wreck to your current location."),
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
    (
        "say",
        "Send an in-game chat message. Preferred syntax: say \"message\" to system|local|faction|private; private also requires a target player after the channel. Emergency is read-only.",
    ),
];

/// Build the default command catalog.
pub fn default_command_catalog() -> HashMap<String, CommandSpec> {
    let mut commands = HashMap::new();
    let simple = [
        ("cloak", vec![arg("mode", ArgType::Any, false)]),
        ("hunt", vec![arg("target", ArgType::Any, true)]),
        ("prepay_tax", vec![arg("quantity", ArgType::Integer, true)]),
        ("cancel_craft_job", vec![arg("job_id", ArgType::Any, true)]),
        ("refit_ship", vec![]),
        (
            "cancel_commission",
            vec![arg("commission_id", ArgType::Any, true)],
        ),
        (
            "cancel_ship_listing",
            vec![arg("listing_id", ArgType::ListingId, true)],
        ),
        (
            "place_ship_buy_order",
            vec![
                arg("ship_class", ArgType::Any, true),
                arg("price", ArgType::Integer, true),
            ],
        ),
        (
            "cancel_ship_buy_order",
            vec![arg("order_id", ArgType::Any, true)],
        ),
        (
            "sell_ship_to_order",
            vec![
                arg("order_id", ArgType::Any, true),
                arg("ship_id", ArgType::ShipId, true),
            ],
        ),
        ("release_wreck", vec![]),
        ("insure_ship", vec![arg("ticks", ArgType::Integer, true)]),
        (
            "citizenship_apply",
            vec![arg("empire_id", ArgType::Any, true)],
        ),
        (
            "citizenship_withdraw",
            vec![arg("empire_id", ArgType::Any, true)],
        ),
        (
            "citizenship_renounce",
            vec![arg("empire_id", ArgType::Any, true)],
        ),
        ("trade_accept", vec![arg("trade_id", ArgType::Any, true)]),
        ("faction_leave", vec![]),
        (
            "faction_withdraw_invite",
            vec![arg("player", ArgType::Any, true)],
        ),
        (
            "faction_propose_ally",
            vec![arg("faction", ArgType::Any, true)],
        ),
        (
            "faction_accept_ally",
            vec![arg("faction", ArgType::Any, true)],
        ),
        (
            "faction_remove_ally",
            vec![arg("faction", ArgType::Any, true)],
        ),
        (
            "faction_declare_war",
            vec![
                arg("faction", ArgType::Any, true),
                arg("reason", ArgType::Any, false),
            ],
        ),
        (
            "faction_propose_peace",
            vec![
                arg("faction", ArgType::Any, true),
                arg("message", ArgType::Any, false),
            ],
        ),
        (
            "faction_accept_peace",
            vec![arg("faction", ArgType::Any, true)],
        ),
        (
            "faction_set_enemy",
            vec![arg("faction", ArgType::Any, true)],
        ),
        (
            "faction_remove_enemy",
            vec![arg("faction", ArgType::Any, true)],
        ),
        (
            "faction_prepay_tax",
            vec![arg("quantity", ArgType::Integer, true)],
        ),
        (
            "faction_cancel_mission",
            vec![arg("mission_id", ArgType::MissionId, true)],
        ),
        ("espionage", vec![]),
        ("scan_poi", vec![arg("poi_id", ArgType::PoiId, true)]),
    ];
    for (name, args) in simple {
        commands.insert(name.to_string(), command(name, args));
    }
    commands.insert(
        "supply_commission".into(),
        command(
            "supply_commission",
            vec![
                arg("commission_id", ArgType::Any, true),
                arg("item", ArgType::ItemId, true),
                arg("quantity", ArgType::Integer, true),
            ],
        ),
    );
    commands.insert(
        "trade_offer".into(),
        command(
            "trade_offer",
            std::iter::once(arg("target", ArgType::Any, true))
                .chain((0..20).map(|_| arg("clause", ArgType::Any, false)))
                .collect(),
        ),
    );

    commands.insert(
        "mine".to_string(),
        command("mine", vec![arg("resource", ArgType::ItemId, false)]),
    );
    commands.insert("survey".to_string(), command("survey", vec![]));
    commands.insert(
        "attack".to_string(),
        command("attack", vec![arg("target_id", ArgType::Any, true)]),
    );
    commands.insert(
        "scan".to_string(),
        command("scan", vec![arg("target", ArgType::Any, false)]),
    );
    commands.insert(
        "find".to_string(),
        command("find", vec![variadic_arg("target", ArgType::Any)]),
    );
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
    commands.insert("undock".to_string(), command("undock", vec![]));
    commands.insert("set_home".to_string(), command("set_home", vec![]));
    commands.insert(
        "repair".to_string(),
        command(
            "repair",
            vec![
                arg("target", ArgType::Any, false),
                arg("item", ArgType::ItemId, false),
                arg("quantity", ArgType::Integer, false),
            ],
        ),
    );
    commands.insert(
        "repair_module".to_string(),
        command(
            "repair_module",
            vec![arg("module", ArgType::ModuleId, true)],
        ),
    );
    commands.insert(
        "recycle".to_string(),
        command(
            "recycle",
            vec![
                arg("recipe", ArgType::RecipeId, true),
                arg("quantity", ArgType::Integer, false),
            ],
        ),
    );
    commands.insert(
        "refuel".to_string(),
        command(
            "refuel",
            vec![
                arg("target", ArgType::Any, false),
                arg("quantity", ArgType::Integer, false),
            ],
        ),
    );
    commands.insert(
        "self_destruct".to_string(),
        command("self_destruct", vec![]),
    );
    commands.insert(
        "load_passenger".to_string(),
        command(
            "load_passenger",
            vec![arg("destination", ArgType::PoiId, true)],
        ),
    );
    commands.insert(
        "unload_passenger".to_string(),
        command(
            "unload_passenger",
            vec![
                arg("name", ArgType::Any, true),
                arg("target", ArgType::Any, false),
            ],
        ),
    );
    commands.insert(
        "sell".to_string(),
        command(
            "sell",
            vec![
                arg("item", ArgType::ItemId, false),
                arg("quantity", ArgType::Integer, false),
                arg("min_price", ArgType::Integer, false),
                arg("mode", ArgType::Any, false),
            ],
        ),
    );
    commands.insert(
        "buy".to_string(),
        command(
            "buy",
            vec![
                arg("item", ArgType::ItemId, true),
                arg("quantity", ArgType::Integer, true),
                arg("max_price", ArgType::Integer, false),
                arg("mode", ArgType::Any, false),
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
        "faction_create".to_string(),
        command(
            "faction_create",
            vec![
                arg("name", ArgType::Any, true),
                arg("tag", ArgType::Any, true),
            ],
        ),
    );
    commands.insert(
        "faction_invite".to_string(),
        command("faction_invite", vec![arg("player", ArgType::Any, true)]),
    );
    commands.insert(
        "faction_accept_invite".to_string(),
        command(
            "faction_accept_invite",
            vec![arg("faction", ArgType::Any, true)],
        ),
    );
    commands.insert(
        "faction_kick".to_string(),
        command("faction_kick", vec![arg("player", ArgType::Any, true)]),
    );
    commands.insert(
        "faction_set_role".to_string(),
        command(
            "faction_set_role",
            vec![
                arg("player", ArgType::Any, true),
                arg("role", ArgType::Any, true),
            ],
        ),
    );
    commands.insert(
        "found_station".to_string(),
        command(
            "found_station",
            vec![
                arg("name", ArgType::Any, true),
                arg("public_access", ArgType::Any, true),
            ],
        ),
    );
    commands.insert(
        "facility_build".to_string(),
        command(
            "facility_build",
            vec![arg("facility_type", ArgType::Any, true)],
        ),
    );
    commands.insert(
        "faction_facility_build".to_string(),
        command(
            "faction_facility_build",
            vec![arg("facility_type", ArgType::Any, true)],
        ),
    );
    commands.insert(
        "facility_upgrade".to_string(),
        command(
            "facility_upgrade",
            vec![
                arg("facility_id", ArgType::Any, true),
                arg("facility_type", ArgType::Any, true),
            ],
        ),
    );
    commands.insert(
        "faction_facility_upgrade".to_string(),
        command(
            "faction_facility_upgrade",
            vec![
                arg("facility_id", ArgType::Any, true),
                arg("facility_type", ArgType::Any, true),
            ],
        ),
    );
    commands.insert(
        "facility_dismantle".to_string(),
        command(
            "facility_dismantle",
            vec![arg("facility_id", ArgType::Any, true)],
        ),
    );
    commands.insert(
        "faction_facility_dismantle".to_string(),
        command(
            "faction_facility_dismantle",
            vec![arg("facility_id", ArgType::Any, true)],
        ),
    );
    commands.insert(
        "facility_set_access".to_string(),
        command(
            "facility_set_access",
            vec![
                arg("facility_id", ArgType::Any, true),
                arg("access", ArgType::Any, true),
            ],
        ),
    );
    commands.insert(
        "facility_set_output_price".to_string(),
        command(
            "facility_set_output_price",
            vec![
                arg("facility_id", ArgType::Any, true),
                arg("item_id", ArgType::ItemId, true),
                arg("price", ArgType::Integer, true),
            ],
        ),
    );
    commands.insert(
        "facility_set_name".to_string(),
        command(
            "facility_set_name",
            vec![
                arg("facility_id", ArgType::Any, true),
                arg("custom_name", ArgType::Any, true),
            ],
        ),
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
        "rename_ship".to_string(),
        command("rename_ship", vec![arg("name", ArgType::Any, true)]),
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
            vec![
                arg("ship_class", ArgType::Any, true),
                arg("materials", ArgType::Any, false),
            ],
        ),
    );
    commands.insert(
        "sell_ship".to_string(),
        command("sell_ship", vec![arg("ship", ArgType::ShipId, true)]),
    );
    commands.insert(
        "scrap_ship".to_string(),
        command("scrap_ship", vec![arg("ship", ArgType::ShipId, true)]),
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
        "cancel_order".to_string(),
        command("cancel_order", vec![arg("order_id", ArgType::Any, true)]),
    );
    commands.insert(
        "modify_order".to_string(),
        command(
            "modify_order",
            vec![
                arg("order_id", ArgType::Any, true),
                arg("price_each", ArgType::Integer, true),
            ],
        ),
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
    commands.insert("scrap_wreck".to_string(), command("scrap_wreck", vec![]));
    commands.insert("sell_wreck".to_string(), command("sell_wreck", vec![]));
    commands.insert(
        "distress_signal".to_string(),
        command(
            "distress_signal",
            vec![arg("distress_type", ArgType::Any, false)],
        ),
    );
    commands.insert(
        "say".to_string(),
        command(
            "say",
            vec![
                arg("content", ArgType::Any, true),
                arg("channel", ArgType::Any, true),
                arg("target", ArgType::Any, false),
            ],
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
        variadic: false,
    }
}

fn variadic_arg(name: &str, kind: ArgType) -> ArgSpec {
    ArgSpec {
        name: name.to_string(),
        kind,
        required: false,
        variadic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_control_commands_are_not_registered() {
        let catalog = default_command_catalog();
        assert!(!catalog.contains_key("halt"));
        assert!(!catalog.contains_key("wait"));
    }

    #[test]
    fn default_command_catalog_contains_core_runtime_commands() {
        let catalog = default_command_catalog();
        assert!(catalog.contains_key("mine"));
        assert!(catalog.contains_key("go"));
        assert!(catalog.contains_key("accept_mission"));
        assert!(catalog.contains_key("list_ship_for_sale"));
        assert!(catalog.contains_key("craft"));
        assert!(catalog.contains_key("load_passenger"));
        assert!(catalog.contains_key("unload_passenger"));
        assert_eq!(catalog["load_passenger"].args[0].kind, ArgType::PoiId);
    }

    #[test]
    fn removed_claim_commission_is_not_registered() {
        let catalog = default_command_catalog();
        assert!(!catalog.contains_key("claim_commission"));
    }

    #[test]
    fn removed_jettison_commands_are_not_registered() {
        let catalog = default_command_catalog();
        assert!(!catalog.contains_key("jettison"));
        assert!(!catalog.contains_key("jettison_except"));
    }

    #[test]
    fn removed_loot_wreck_is_not_registered() {
        let catalog = default_command_catalog();
        assert!(!catalog.contains_key("loot_wreck"));
    }

    #[test]
    fn sell_optional_args_are_registered() {
        let catalog = default_command_catalog();
        let sell = catalog.get("sell").expect("sell command");
        assert_eq!(sell.args.len(), 4);
        assert_eq!(sell.args[0].name, "item");
        assert_eq!(sell.args[1].name, "quantity");
        assert_eq!(sell.args[2].name, "min_price");
        assert_eq!(sell.args[3].name, "mode");
        assert!(!sell.args[0].required);
        assert!(!sell.args[1].required);
        assert!(!sell.args[2].required);
        assert!(!sell.args[3].required);
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
