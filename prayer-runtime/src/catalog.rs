//! Default command and predicate catalogs for DSL analysis/validation.

use std::collections::HashMap;

use crate::dsl::{ArgSpec, ArgType, CommandSpec, PredicateSpec};

/// Build the default command catalog.
pub(crate) fn default_command_catalog() -> HashMap<String, CommandSpec> {
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
    let mut boolean = HashMap::new();
    boolean.insert(
        "MISSION_COMPLETE".to_string(),
        PredicateSpec {
            name: "MISSION_COMPLETE".to_string(),
            arity: 1,
        },
    );

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

    (boolean, numeric)
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
}
