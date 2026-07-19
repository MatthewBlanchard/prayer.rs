use prayer_actions::ActionArg;
use serde_json::Value;
use spacemolt_lib_rs::actions::{find_action, ActionDef};

use crate::operation_failure::OperationFailure;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockingRequirement {
    None,
    DockableBase,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedCommandDef {
    pub definition: &'static ActionDef,
    pub docking: DockingRequirement,
}

impl std::ops::Deref for ResolvedCommandDef {
    type Target = ActionDef;

    fn deref(&self) -> &Self::Target {
        self.definition
    }
}

/// Resolve the small set of Prayer DSL naming conventions into the generated
/// SpaceMolt action catalog and attach Prayer's execution policy. The generated
/// catalog remains the source of request fields.
pub(crate) fn resolve_command(action: &str) -> Result<ResolvedCommandDef, OperationFailure> {
    let normalized = action.to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "go" | "mine"
            | "find"
            | "buy"
            | "sell"
            | "cancel_buy"
            | "cancel_sell"
            | "transfer"
            | "wait"
            | "set_home"
            | "dock"
    ) {
        return Err(OperationFailure::InvalidIntent(format!(
            "{action} requires command-engine orchestration"
        )));
    }
    let key = match normalized.as_str() {
        // Genuine Prayer aliases.
        "survey" => "spacemolt/survey_system".to_string(),
        "buy_ship" => "spacemolt_ship/buy_listed_ship".to_string(),
        "salvage_wreck" => "spacemolt_salvage/loot".to_string(),
        "tow_wreck" => "spacemolt_salvage/tow".to_string(),
        "scrap_wreck" => "spacemolt_salvage/scrap".to_string(),
        "sell_wreck" => "spacemolt_salvage/sell".to_string(),
        "release_wreck" => "spacemolt_salvage/release".to_string(),
        "insure_ship" => "spacemolt_salvage/insure".to_string(),
        "cancel_craft_job" => "spacemolt/craft".to_string(),
        "citizenship_apply" => "spacemolt_citizenship/apply".to_string(),
        "citizenship_withdraw" => "spacemolt_citizenship/withdraw".to_string(),
        "citizenship_renounce" => "spacemolt_citizenship/renounce".to_string(),
        "trade_offer" => "spacemolt_transfer/trade_offer".to_string(),
        "trade_accept" => "spacemolt_transfer/trade_accept".to_string(),
        "espionage" => "spacemolt_intel/espionage".to_string(),
        "scan_poi" => "spacemolt_intel/scan_poi".to_string(),
        "faction_set_role" => "spacemolt_faction_admin/promote".to_string(),
        // DSL namespaces are naming conventions, not endpoint registries.
        value if value.starts_with("faction_facility_") => format!(
            "spacemolt_facility/faction_{}",
            value.trim_start_matches("faction_facility_")
        ),
        value if value.starts_with("facility_") => format!(
            "spacemolt_facility/{}",
            value.trim_start_matches("facility_")
        ),
        value if value.starts_with("faction_") => {
            format!("spacemolt_faction/{}", value.trim_start_matches("faction_"))
        }
        // Actions whose tool differs from the default SpaceMolt tool.
        "switch_ship"
        | "buy_listed_ship"
        | "commission_ship"
        | "sell_ship"
        | "scrap_ship"
        | "list_ship_for_sale"
        | "rename_ship"
        | "refit_ship"
        | "cancel_commission"
        | "supply_commission"
        | "cancel_ship_listing"
        | "place_ship_buy_order"
        | "cancel_ship_buy_order"
        | "sell_ship_to_order" => {
            format!("spacemolt_ship/{value}", value = normalized)
        }
        "cancel_order" | "modify_order" => format!("spacemolt_market/{value}", value = normalized),
        value => format!("spacemolt/{value}"),
    };

    let definition =
        find_action(&key).ok_or_else(|| OperationFailure::InvalidIntent(action.to_string()))?;
    Ok(ResolvedCommandDef {
        definition,
        docking: docking_requirement(definition.key),
    })
}

fn docking_requirement(key: &str) -> DockingRequirement {
    match key {
        "spacemolt/accept_mission"
        | "spacemolt/decline_mission"
        | "spacemolt/repair_module"
        | "spacemolt/recycle"
        | "spacemolt/load_passenger"
        | "spacemolt/unload_passenger"
        | "spacemolt/craft"
        | "spacemolt_facility/build"
        | "spacemolt_facility/faction_build"
        | "spacemolt_facility/upgrade"
        | "spacemolt_facility/faction_upgrade"
        | "spacemolt_facility/dismantle"
        | "spacemolt_facility/faction_dismantle"
        | "spacemolt_facility/set_access"
        | "spacemolt_facility/set_output_price"
        | "spacemolt_facility/set_name"
        | "spacemolt_ship/buy_listed_ship"
        | "spacemolt_ship/switch_ship"
        | "spacemolt_ship/commission_ship"
        | "spacemolt_ship/list_ship_for_sale"
        | "spacemolt_market/cancel_order"
        | "spacemolt_market/modify_order" => DockingRequirement::DockableBase,
        _ => DockingRequirement::None,
    }
}

pub(crate) fn args_to_generated_payload(
    command_name: &str,
    args: &[ActionArg],
    action: &ActionDef,
) -> Result<Value, OperationFailure> {
    if command_name == "refuel" && matches!(args, [ActionArg::Integer(_)]) {
        return Ok(serde_json::json!({"quantity": command_arg_to_json(&args[0])}));
    }
    let catalog = prayer_lang::catalog::default_command_catalog();
    let command = catalog
        .get(command_name)
        .ok_or_else(|| OperationFailure::InvalidIntent(command_name.to_string()))?;
    if args.len() > command.args.len() {
        return Err(OperationFailure::InvalidIntent(format!(
            "{} expected at most {} args, got {}",
            action.action,
            command.args.len(),
            args.len()
        )));
    }

    let mut map = serde_json::Map::new();
    for (index, (arg, spec)) in args.iter().zip(&command.args).enumerate() {
        let name = payload_name(command_name, index, &spec.name);
        map.insert(name.to_string(), command_arg_to_json(arg));
    }
    if command_name == "cloak" {
        let enabled = args
            .first()
            .map(ActionArg::as_text)
            .map(|v| v != "off")
            .unwrap_or(true);
        map.insert("enable".into(), Value::Bool(enabled));
        map.remove("mode");
    }
    if command_name == "commission_ship" {
        map.insert(
            "provide_materials".into(),
            Value::Bool(args.get(1).is_some_and(|v| v.as_text() == "materials")),
        );
        map.remove("materials");
    }
    Ok(Value::Object(map))
}

fn payload_name<'a>(command: &str, index: usize, dsl_name: &'a str) -> &'a str {
    match (command, index) {
        ("switch_ship" | "sell_ship" | "scrap_ship" | "list_ship_for_sale", 0) => "ship_id",
        ("unload_passenger", 0) => "id",
        ("repair", 1) => "item_id",
        ("cancel_craft_job", 0) => "job_id",
        ("cancel_commission" | "supply_commission", 0) => "id",
        ("supply_commission", 1) => "item_id",
        ("cancel_ship_listing" | "place_ship_buy_order" | "cancel_ship_buy_order", 0) => "id",
        ("sell_ship_to_order", 0) => "id",
        ("sell_ship_to_order", 1) => "ship_id",
        ("insure_ship", 0) => "ticks",
        ("citizenship_apply" | "citizenship_withdraw" | "citizenship_renounce", 0) => "target",
        ("trade_accept", 0) => "trade_id",
        ("scan_poi", 0) => "poi_id",
        (
            "faction_withdraw_invite"
            | "faction_propose_ally"
            | "faction_accept_ally"
            | "faction_remove_ally"
            | "faction_declare_war"
            | "faction_propose_peace"
            | "faction_accept_peace"
            | "faction_set_enemy"
            | "faction_remove_enemy"
            | "faction_cancel_mission",
            0,
        ) => "id",
        ("faction_prepay_tax", 0) => "amount",
        ("faction_declare_war" | "faction_propose_peace", 1) => "text",
        ("install_mod" | "uninstall_mod", 0) => "module_id",
        ("repair_module", 0) => "id",
        ("recycle", 0) => "id",
        ("buy_ship" | "buy_listed_ship", 0) => "listing_id",
        ("commission_ship", 0) => "id",
        ("craft", 1) => "quantity",
        ("faction_create", 0) => "text",
        ("faction_create", 1) => "id",
        ("faction_invite" | "faction_kick", 0) => "id",
        ("faction_accept_invite", 0) => "id",
        ("faction_set_role", 0) => "player_id",
        ("faction_set_role", 1) => "role_id",
        _ => dsl_name,
    }
}

pub(crate) fn craft_args_to_payload(args: &[ActionArg]) -> Result<Value, OperationFailure> {
    if args.len() < 2 {
        return Err(OperationFailure::InvalidIntent(
            "craft requires a recipe id and quantity".to_string(),
        ));
    }
    let mut map = serde_json::Map::new();
    map.insert("recipe_id".to_string(), command_arg_to_json(&args[0]));
    map.insert("quantity".to_string(), command_arg_to_json(&args[1]));
    map.insert(
        "deliver_to".to_string(),
        Value::String("storage".to_string()),
    );
    for arg in &args[2..] {
        let text = arg.as_text();
        let Some((key, value)) = text.split_once('=') else {
            return Err(OperationFailure::InvalidIntent(format!(
                "unsupported craft routing argument '{text}'"
            )));
        };
        match key {
            "source" | "deliver_to" | "facility_id" | "preset" => {
                map.insert(key.to_string(), Value::String(value.to_string()));
            }
            _ => {
                return Err(OperationFailure::InvalidIntent(format!(
                    "unsupported craft routing field '{key}'"
                )))
            }
        }
    }
    Ok(Value::Object(map))
}

fn command_arg_to_json(arg: &ActionArg) -> Value {
    match arg {
        ActionArg::Integer(value) => Value::Number((*value).into()),
        _ => Value::String(arg.as_text()),
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn docking_policy_is_attached_after_alias_resolution() {
        let alias = resolve_command("buy_ship").expect("buy_ship");
        let canonical = resolve_command("buy_listed_ship").expect("buy_listed_ship");
        assert_eq!(alias.definition.key, canonical.definition.key);
        assert_eq!(alias.docking, DockingRequirement::DockableBase);
        assert_eq!(canonical.docking, DockingRequirement::DockableBase);

        let survey = resolve_command("survey").expect("survey");
        assert_eq!(survey.docking, DockingRequirement::None);
    }

    #[test]
    fn added_command_surfaces_resolve_to_generated_mutations() {
        let commands = [
            "cloak",
            "hunt",
            "prepay_tax",
            "repair",
            "refuel",
            "cancel_craft_job",
            "refit_ship",
            "cancel_commission",
            "supply_commission",
            "cancel_ship_listing",
            "place_ship_buy_order",
            "cancel_ship_buy_order",
            "sell_ship_to_order",
            "release_wreck",
            "insure_ship",
            "citizenship_apply",
            "citizenship_withdraw",
            "citizenship_renounce",
            "trade_offer",
            "trade_accept",
            "faction_leave",
            "faction_withdraw_invite",
            "faction_propose_ally",
            "faction_accept_ally",
            "faction_remove_ally",
            "faction_declare_war",
            "faction_propose_peace",
            "faction_accept_peace",
            "faction_set_enemy",
            "faction_remove_enemy",
            "faction_prepay_tax",
            "faction_cancel_mission",
            "espionage",
            "scan_poi",
        ];
        for command in commands {
            let action = resolve_command(command)
                .unwrap_or_else(|error| panic!("{command} has no generated mutation: {error}"));
            assert_eq!(
                action.kind,
                spacemolt_lib_rs::actions::ActionKind::Mutation,
                "{command}"
            );
        }
    }
}
