//! Canonical human-facing PrayerLang projection for materialized actions.

use prayer_actions::{
    Action, GoTarget, TransferEndpoint, TransferItem, TransferRequest, TransferSubject,
};

/// Render one exact materialized action as a PrayerLang statement.
pub fn render_action(action: &Action) -> String {
    match action {
        Action::Halt => "halt;".into(),
        Action::Wait { ticks } => format!("wait {ticks};"),
        Action::Go { destination } => format!("go {};", arg(&go_target(destination))),
        Action::Dock => "dock;".into(),
        Action::Undock => "undock;".into(),
        Action::Mine { resource: None } => "mine;".into(),
        Action::Mine {
            resource: Some(resource),
        } => format!("mine {};", arg(&resource.0)),
        Action::Transfer(request) => render_transfer(request),
        Action::SetHome => statement("set_home", vec![]),
        Action::Find(request) => statement("find", request.targets.clone()),
        Action::Survey => statement("survey", vec![]),
        Action::Attack { target_id } => statement("attack", vec![target_id.clone()]),
        Action::Scan { target } => statement("scan", target.clone().into_iter().collect()),
        Action::Cloak { enabled } => {
            statement("cloak", if *enabled { vec![] } else { vec!["off".into()] })
        }
        Action::Hunt { target } => statement("hunt", vec![target.clone()]),
        Action::PrepayTax { quantity } => statement("prepay_tax", vec![quantity.to_string()]),
        Action::AcceptMission { mission_id } => {
            statement("accept_mission", vec![mission_id.clone()])
        }
        Action::AbandonMission { mission_id } => {
            statement("abandon_mission", vec![mission_id.clone()])
        }
        Action::DeclineMission { template_id } => {
            statement("decline_mission", vec![template_id.clone()])
        }
        Action::CompleteMission { mission_id } => {
            statement("complete_mission", vec![mission_id.clone()])
        }
        Action::LoadPassenger { destination } => {
            statement("load_passenger", vec![destination.clone()])
        }
        Action::UnloadPassenger { name, target } => match target {
            Some(target) => format!("unload_passenger {} to {};", arg(name), arg(target)),
            None => statement("unload_passenger", vec![name.clone()]),
        },
        Action::Buy(request) => {
            let order = if request.place_order { "order " } else { "" };
            let price = request
                .max_price
                .map(|p| format!(" at {p}"))
                .unwrap_or_default();
            let destination = request
                .deliver_to
                .as_ref()
                .map(|v| format!(" to {v}"))
                .unwrap_or_default();
            format!(
                "buy {order}{} {}{price}{destination};",
                request.quantity,
                arg(&request.item.0)
            )
        }
        Action::Sell(request) => {
            let item = request.item.as_ref().map(|v| arg(&v.0)).unwrap_or_default();
            let quantity = request
                .quantity
                .map(|q| format!("{q} "))
                .unwrap_or_default();
            let order = if request.place_order { "order " } else { "" };
            let price = request
                .min_price
                .map(|p| format!(" at {p}"))
                .unwrap_or_default();
            let body = format!("{order}{quantity}{item}{price}");
            if body.is_empty() {
                "sell;".into()
            } else {
                format!("sell {body};")
            }
        }
        Action::CancelBuy { item } => statement("cancel_buy", vec![item.0.clone()]),
        Action::CancelSell { item } => statement("cancel_sell", vec![item.0.clone()]),
        Action::FactionCreate { name, tag } => {
            statement("faction_create", vec![name.clone(), tag.clone()])
        }
        Action::FactionInvite { player } => statement("faction_invite", vec![player.clone()]),
        Action::FactionAcceptInvite { faction } => {
            statement("faction_accept_invite", vec![faction.clone()])
        }
        Action::FactionKick { player } => statement("faction_kick", vec![player.clone()]),
        Action::FactionSetRole { player, role } => {
            statement("faction_set_role", vec![player.clone(), role.clone()])
        }
        Action::FacilityBuild { facility_type } => {
            statement("facility_build", vec![facility_type.clone()])
        }
        Action::FactionFacilityBuild { facility_type } => {
            statement("faction_facility_build", vec![facility_type.clone()])
        }
        Action::FacilityUpgrade(request) => statement(
            "facility_upgrade",
            vec![request.facility_id.clone(), request.facility_type.clone()],
        ),
        Action::FactionFacilityUpgrade(request) => statement(
            "faction_facility_upgrade",
            vec![request.facility_id.clone(), request.facility_type.clone()],
        ),
        Action::FacilityDismantle { facility_id } => {
            statement("facility_dismantle", vec![facility_id.clone()])
        }
        Action::FactionFacilityDismantle { facility_id } => {
            statement("faction_facility_dismantle", vec![facility_id.clone()])
        }
        Action::FacilitySetAccess(request) => statement(
            "facility_set_access",
            vec![request.facility_id.clone(), request.access.clone()],
        ),
        Action::FacilitySetOutputPrice(request) => statement(
            "facility_set_output_price",
            vec![
                request.facility_id.clone(),
                request.item.0.clone(),
                request.price.to_string(),
            ],
        ),
        Action::FacilitySetName(request) => statement(
            "facility_set_name",
            vec![request.facility_id.clone(), request.custom_name.clone()],
        ),
        Action::UseItem { item, quantity } => {
            statement("use_item", vec![item.0.clone(), quantity.to_string()])
        }
        Action::Repair(request) => render_repair(request),
        Action::RepairModule { module } => statement("repair_module", vec![module.clone()]),
        Action::Recycle(request) => {
            let mut rendered = format!("recycle {} {}", arg(&request.recipe_id), request.quantity);
            if let Some(source) = &request.source {
                rendered.push_str(&format!(" from {source}"));
            }
            if let Some(destination) = &request.destination {
                rendered.push_str(&format!(" to {destination}"));
            }
            if let Some(facility) = &request.facility_id {
                rendered.push_str(&format!(" at {}", arg(facility)));
            }
            rendered.push(';');
            rendered
        }
        Action::Refuel(request) => render_refuel(request),
        Action::SelfDestruct => statement("self_destruct", vec![]),
        Action::SwitchShip { ship } => statement("switch_ship", vec![ship.clone()]),
        Action::RenameShip { name } => statement("rename_ship", vec![name.clone()]),
        Action::InstallMod { module } => statement("install_mod", vec![module.clone()]),
        Action::UninstallMod { module } => statement("uninstall_mod", vec![module.clone()]),
        Action::BuyShip { listing } => statement("buy_ship", vec![listing.clone()]),
        Action::BuyListedShip { listing } => statement("buy_listed_ship", vec![listing.clone()]),
        Action::CommissionShip(request) => {
            format!(
                "commission_ship {}{};",
                arg(&request.ship_class),
                if request.provide_materials {
                    " with materials"
                } else {
                    ""
                }
            )
        }
        Action::SellShip { ship } => statement("sell_ship", vec![ship.clone()]),
        Action::ScrapShip { ship } => statement("scrap_ship", vec![ship.clone()]),
        Action::ListShipForSale { ship, price } => {
            statement("list_ship_for_sale", vec![ship.clone(), price.to_string()])
        }
        Action::RefitShip => statement("refit_ship", vec![]),
        Action::CancelCommission { commission_id } => {
            statement("cancel_commission", vec![commission_id.clone()])
        }
        Action::SupplyCommission {
            commission_id,
            item,
            quantity,
        } => statement(
            "supply_commission",
            vec![commission_id.clone(), item.0.clone(), quantity.to_string()],
        ),
        Action::CancelShipListing { listing_id } => {
            statement("cancel_ship_listing", vec![listing_id.clone()])
        }
        Action::PlaceShipBuyOrder { ship_class, price } => statement(
            "place_ship_buy_order",
            vec![ship_class.clone(), price.to_string()],
        ),
        Action::CancelShipBuyOrder { order_id } => {
            statement("cancel_ship_buy_order", vec![order_id.clone()])
        }
        Action::SellShipToOrder { order_id, ship_id } => statement(
            "sell_ship_to_order",
            vec![order_id.clone(), ship_id.clone()],
        ),
        Action::CancelOrder { order_id } => statement("cancel_order", vec![order_id.clone()]),
        Action::ModifyOrder {
            order_id,
            price_each,
        } => format!("modify_order {} at {price_each};", arg(order_id)),
        Action::Craft(request) => {
            let mut rendered = format!("craft {} {}", arg(&request.recipe_id), request.quantity);
            if let Some(source) = &request.source {
                rendered.push_str(&format!(" from {source}"));
            }
            if let Some(destination) = &request.destination {
                rendered.push_str(&format!(" to {destination}"));
            }
            if let Some(facility) = &request.facility_id {
                rendered.push_str(&format!(" at {}", arg(facility)));
            }
            if let Some(preset) = &request.preset {
                rendered.push_str(&format!(" preset {}", arg(preset)));
            }
            rendered.push(';');
            rendered
        }
        Action::CancelCraftJob { job_id } => statement("cancel_craft_job", vec![job_id.clone()]),
        Action::SalvageWreck { wreck_id } => statement("salvage_wreck", vec![wreck_id.clone()]),
        Action::TowWreck { wreck_id } => statement("tow_wreck", vec![wreck_id.clone()]),
        Action::ScrapWreck => statement("scrap_wreck", vec![]),
        Action::SellWreck => statement("sell_wreck", vec![]),
        Action::ReleaseWreck => statement("release_wreck", vec![]),
        Action::InsureShip { ticks } => statement("insure_ship", vec![ticks.to_string()]),
        Action::CitizenshipApply { empire_id } => {
            statement("citizenship_apply", vec![empire_id.clone()])
        }
        Action::CitizenshipWithdraw { empire_id } => {
            statement("citizenship_withdraw", vec![empire_id.clone()])
        }
        Action::CitizenshipRenounce { empire_id } => {
            statement("citizenship_renounce", vec![empire_id.clone()])
        }
        Action::TradeOffer(request) => {
            let mut values = vec![request.target.clone()];
            if let Some(credits) = request.offer_credits {
                values.extend(["offer".into(), "credits".into(), credits.to_string()]);
            }
            for item in &request.offer_items {
                values.extend([
                    "offer".into(),
                    item.quantity.to_string(),
                    item.item.0.clone(),
                ]);
            }
            if let Some(credits) = request.request_credits {
                values.extend(["request".into(), "credits".into(), credits.to_string()]);
            }
            for item in &request.request_items {
                values.extend([
                    "request".into(),
                    item.quantity.to_string(),
                    item.item.0.clone(),
                ]);
            }
            statement("trade_offer", values)
        }
        Action::TradeAccept { trade_id } => statement("trade_accept", vec![trade_id.clone()]),
        Action::FactionLeave => statement("faction_leave", vec![]),
        Action::FactionWithdrawInvite { player } => {
            statement("faction_withdraw_invite", vec![player.clone()])
        }
        Action::FactionProposeAlly { faction } => {
            statement("faction_propose_ally", vec![faction.clone()])
        }
        Action::FactionAcceptAlly { faction } => {
            statement("faction_accept_ally", vec![faction.clone()])
        }
        Action::FactionRemoveAlly { faction } => {
            statement("faction_remove_ally", vec![faction.clone()])
        }
        Action::FactionDeclareWar { faction, reason } => statement(
            "faction_declare_war",
            std::iter::once(faction.clone())
                .chain(reason.clone())
                .collect(),
        ),
        Action::FactionProposePeace { faction, message } => statement(
            "faction_propose_peace",
            std::iter::once(faction.clone())
                .chain(message.clone())
                .collect(),
        ),
        Action::FactionAcceptPeace { faction } => {
            statement("faction_accept_peace", vec![faction.clone()])
        }
        Action::FactionSetEnemy { faction } => {
            statement("faction_set_enemy", vec![faction.clone()])
        }
        Action::FactionRemoveEnemy { faction } => {
            statement("faction_remove_enemy", vec![faction.clone()])
        }
        Action::FactionPrepayTax { quantity } => {
            statement("faction_prepay_tax", vec![quantity.to_string()])
        }
        Action::FactionCancelMission { mission_id } => {
            statement("faction_cancel_mission", vec![mission_id.clone()])
        }
        Action::Espionage => statement("espionage", vec![]),
        Action::ScanPoi { poi_id } => statement("scan_poi", vec![poi_id.clone()]),
        Action::DistressSignal { distress_type } => statement(
            "distress_signal",
            distress_type.clone().into_iter().collect(),
        ),
        Action::Say(request) => {
            let target = request
                .target
                .as_ref()
                .map(|v| format!(" {}", arg(v)))
                .unwrap_or_default();
            format!(
                "say {} to {}{target};",
                arg(&request.content),
                arg(&request.channel)
            )
        }
    }
}

fn render_repair(request: &prayer_actions::ServiceTransferRequest) -> String {
    let Some(target) = &request.target else {
        return "repair;".into();
    };
    let mut rendered = format!("repair {}", arg(target));
    if let Some(item) = &request.item {
        rendered.push_str(&format!(" with {}", arg(&item.0)));
        if let Some(quantity) = request.quantity {
            rendered.push_str(&format!(" {quantity}"));
        }
    }
    rendered.push(';');
    rendered
}

fn render_refuel(request: &prayer_actions::ServiceTransferRequest) -> String {
    let mut rendered = "refuel".to_string();
    if let Some(quantity) = request.quantity {
        rendered.push_str(&format!(" {quantity}"));
    }
    if let Some(target) = &request.target {
        rendered.push_str(&format!(" to {}", arg(target)));
    }
    rendered.push(';');
    rendered
}

fn statement(name: &str, values: Vec<String>) -> String {
    if values.is_empty() {
        format!("{name};")
    } else {
        format!(
            "{name} {};",
            values
                .iter()
                .map(|value| arg(value))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn go_target(target: &GoTarget) -> String {
    match target {
        GoTarget::Identifier(value) | GoTarget::System(value) | GoTarget::Poi(value) => {
            value.clone()
        }
        GoTarget::Coordinate { x, y } => format!("{x},{y}"),
    }
}

fn render_transfer(request: &TransferRequest) -> String {
    if let TransferSubject::Items { items } = &request.subject {
        let rows = items
            .iter()
            .map(render_transfer_item)
            .collect::<Vec<_>>()
            .join("\n");
        return format!(
            "transfer {{\n{rows}\n}} from {} to {};",
            endpoint(&request.from),
            endpoint(&request.to)
        );
    }
    let subject = match &request.subject {
        TransferSubject::AllCargo => "all".into(),
        TransferSubject::Credits { quantity } => format!("credits {quantity}"),
        TransferSubject::Item {
            id,
            quantity: Some(quantity),
        } => {
            format!("{} {quantity}", arg(&id.0))
        }
        TransferSubject::Item { id, quantity: None } => arg(&id.0),
        TransferSubject::Ship { id } => format!("ship {}", arg(id)),
        TransferSubject::Module { id } => format!("module {}", arg(id)),
        TransferSubject::Items { .. } => unreachable!("handled above"),
    };
    format!(
        "transfer {subject} from {} to {};",
        endpoint(&request.from),
        endpoint(&request.to)
    )
}

fn render_transfer_item(item: &TransferItem) -> String {
    format!("  {} {};", arg(&item.id.0), item.quantity)
}

fn endpoint(value: &TransferEndpoint) -> String {
    match value {
        TransferEndpoint::Cargo => "cargo".into(),
        TransferEndpoint::Storage => "storage".into(),
        TransferEndpoint::Ship(id) => format!("ship {}", arg(id)),
        TransferEndpoint::Faction => "faction".into(),
        TransferEndpoint::FactionTag(tag) => format!("faction {}", arg(tag)),
        TransferEndpoint::Player(name) => format!("player {}", arg(name)),
        TransferEndpoint::Space(Some(id)) => format!("space {}", arg(id)),
        TransferEndpoint::Space(None) => "space".into(),
        TransferEndpoint::Commission(id) => format!("commission {}", arg(id)),
    }
}

fn arg(value: &str) -> String {
    let requires_quotes = value.is_empty()
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\\' | ';' | '{' | '}' | '#'))
        || value.contains("//");
    if !requires_quotes {
        return value.to_owned();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::default_command_catalog, AnalysisObservation, ArgType, AstProgram, PlanNode,
    };
    use prayer_actions::{BuyRequest, ItemId};

    #[test]
    fn every_initial_typed_action_renders_as_parseable_prayerlang() {
        let actions = [
            Action::Halt,
            Action::Wait { ticks: 2 },
            Action::Go {
                destination: GoTarget::Identifier("sol".into()),
            },
            Action::Dock,
            Action::Mine {
                resource: Some(ItemId("iron_ore".into())),
            },
            Action::Buy(BuyRequest {
                item: ItemId("iron_ore".into()),
                quantity: 3,
                max_price: None,
                place_order: false,
                deliver_to: None,
            }),
        ];
        for action in actions {
            let rendered = render_action(&action);
            assert!(
                AstProgram::parse(&rendered).is_ok(),
                "failed to parse `{rendered}`"
            );
        }
    }

    #[test]
    fn every_catalog_action_semantically_round_trips_through_prayerlang() {
        let catalog = default_command_catalog();
        for (name, spec) in &catalog {
            if matches!(name.as_str(), "combat" | "targeting") {
                continue;
            }
            let args = spec
                .args
                .iter()
                .filter(|arg| arg.required)
                .map(|arg| match arg.kind {
                    ArgType::Integer => "1",
                    ArgType::ItemId => "iron_ore",
                    ArgType::SystemId => "sol",
                    ArgType::PoiId => "earth",
                    ArgType::GoTarget => "earth",
                    ArgType::ShipId => "ship-1",
                    ArgType::ListingId => "listing-1",
                    ArgType::MissionId => "mission-1",
                    ArgType::ModuleId => "module-1",
                    ArgType::RecipeId => "recipe-1",
                    ArgType::Any if name == "say" && arg.name == "channel" => "system",
                    ArgType::Any => "value",
                })
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let action = crate::lower_materialized_command(name, &args).expect("typed lowering");
            let rendered = render_action(&action);
            let analyzed = AstProgram::parse(&rendered)
                .unwrap_or_else(|error| panic!("{name} projection failed to parse: {error:?}"))
                .analyze(&catalog, &AnalysisObservation::default())
                .unwrap_or_else(|errors| panic!("{name} projection failed analysis: {errors:?}"));
            let compiled = analyzed.compile().expect("compile projection");
            let PlanNode::Action(template) = &compiled.nodes[0];
            assert_eq!(
                template.materialize(),
                action,
                "{name} changed meaning after projection"
            );
        }
    }
}
