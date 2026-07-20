//! Compilation from analyzed PrayerLang into source-independent plan nodes.

use prayer_actions::{
    Action, BuyRequest, CommissionShipRequest, CraftRequest, FacilityAccessRequest,
    FacilityNameRequest, FacilityOutputPriceRequest, FacilityUpgradeRequest, FindRequest, GoTarget,
    ItemId, RecycleRequest, SayRequest, SellRequest, TransferEndpoint as ActionEndpoint,
    TransferItem, TransferRequest, TransferSubject as ActionSubject,
};
use serde::{Deserialize, Serialize};

use crate::{
    AnalyzedArg, AnalyzedCraft, AnalyzedNode, AnalyzedProgram, AnalyzedRecycle, AnalyzedTransfer,
    AnalyzedTransferEndpoint, AnalyzedTransferSubject, Span,
};

pub const COMPILED_PROGRAM_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledProgram {
    pub schema_version: u32,
    pub nodes: Vec<PlanNode>,
    pub source_map: SourceMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanNode {
    Action(ActionTemplate),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionTemplate {
    pub action: TemplateAction,
    pub source: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateAction {
    Materialized(Action),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMap {
    pub actions: Vec<Span>,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CompileError {
    #[error("invalid analyzed action: {0}")]
    Invalid(String),
}

impl AnalyzedProgram {
    pub fn compile(&self) -> Result<CompiledProgram, CompileError> {
        let mut source_map = SourceMap::default();
        let nodes = compile_nodes(&self.statements, &mut source_map)?;
        Ok(CompiledProgram {
            schema_version: COMPILED_PROGRAM_SCHEMA_VERSION,
            nodes,
            source_map,
        })
    }
}

impl ActionTemplate {
    pub fn materialize(&self) -> Action {
        match &self.action {
            TemplateAction::Materialized(action) => action.clone(),
        }
    }
}

fn compile_nodes(
    nodes: &[AnalyzedNode],
    source_map: &mut SourceMap,
) -> Result<Vec<PlanNode>, CompileError> {
    nodes
        .iter()
        .map(|node| match node {
            AnalyzedNode::Command(command) => {
                let source = command.source.span;
                source_map.actions.push(source);
                let args = command
                    .args
                    .iter()
                    .map(|arg| match arg {
                        AnalyzedArg::Resolved(value) => value.clone(),
                    })
                    .collect::<Vec<_>>();
                let action =
                    TemplateAction::Materialized(command_action(&command.source.name, &args)?);
                Ok(PlanNode::Action(ActionTemplate { action, source }))
            }
            AnalyzedNode::Transfer(transfer) => {
                action_node(transfer_action(transfer)?, transfer.source.span, source_map)
            }
            AnalyzedNode::Craft(craft) => {
                action_node(craft_action(craft)?, craft.source.span, source_map)
            }
            AnalyzedNode::Say(say) => action_node(
                Action::Say(SayRequest {
                    content: say.content.clone(),
                    channel: say.channel.clone(),
                    target: say.target.clone(),
                }),
                say.span,
                source_map,
            ),
            AnalyzedNode::Buy(buy) => action_node(
                Action::Buy(BuyRequest {
                    item: ItemId(buy.item_id.clone()),
                    quantity: buy.quantity,
                    max_price: buy.max_price,
                    place_order: buy.place_order,
                    deliver_to: buy.deliver_to.clone(),
                }),
                buy.span,
                source_map,
            ),
            AnalyzedNode::Sell(sell) => action_node(
                Action::Sell(SellRequest {
                    item: sell.item_id.clone().map(ItemId),
                    quantity: sell.quantity,
                    min_price: sell.min_price,
                    place_order: sell.place_order,
                }),
                sell.span,
                source_map,
            ),
            AnalyzedNode::Recycle(recycle) => {
                action_node(recycle_action(recycle), recycle.source.span, source_map)
            }
            AnalyzedNode::CommissionShip(commission) => action_node(
                Action::CommissionShip(CommissionShipRequest {
                    ship_class: commission.ship_class.clone(),
                    provide_materials: commission.provide_materials,
                }),
                commission.span,
                source_map,
            ),
        })
        .collect()
}

fn action_node(
    action: Action,
    source: Span,
    source_map: &mut SourceMap,
) -> Result<PlanNode, CompileError> {
    source_map.actions.push(source);
    Ok(PlanNode::Action(ActionTemplate {
        action: TemplateAction::Materialized(action),
        source,
    }))
}

fn command_action(name: &str, args: &[String]) -> Result<Action, CompileError> {
    let req = |index: usize| args.get(index).cloned().ok_or_else(|| invalid_args(name));
    let action = match name.to_ascii_lowercase().as_str() {
        // Internal typed actions retained for queue/checkpoint compatibility. These names are
        // intentionally absent from the public command catalog, so PrayerLang source cannot
        // invoke them.
        "halt" if args.is_empty() => Action::Halt,
        "wait" if args.is_empty() => Action::Wait { ticks: 1 },
        "wait" if args.len() == 1 => Action::Wait {
            ticks: args[0].parse().map_err(|_| {
                CompileError::Invalid(format!(
                    "wait expects a non-negative integer, got `{}`",
                    args[0]
                ))
            })?,
        },
        "dock" if args.is_empty() => Action::Dock,
        "undock" if args.is_empty() => Action::Undock,
        "go" if args.len() == 1 => Action::Go {
            destination: GoTarget::Identifier(args[0].clone()),
        },
        "mine" if args.len() <= 1 => Action::Mine {
            resource: args.first().cloned().map(ItemId),
        },
        "set_home" => Action::SetHome,
        "find" => Action::Find(FindRequest {
            targets: args.to_vec(),
        }),
        "survey" => Action::Survey,
        "attack" => Action::Attack { target_id: req(0)? },
        "scan" => Action::Scan {
            target: args.first().cloned(),
        },
        "cloak" => Action::Cloak {
            enabled: args.first().map(|v| v != "off").unwrap_or(true),
        },
        "hunt" => Action::Hunt { target: req(0)? },
        "prepay_tax" => Action::PrepayTax {
            quantity: parse_u(name, args, 0)?,
        },
        "accept_mission" => Action::AcceptMission {
            mission_id: req(0)?,
        },
        "abandon_mission" => Action::AbandonMission {
            mission_id: req(0)?,
        },
        "decline_mission" => Action::DeclineMission {
            template_id: req(0)?,
        },
        "complete_mission" => Action::CompleteMission {
            mission_id: req(0)?,
        },
        "load_passenger" => Action::LoadPassenger {
            destination: req(0)?,
        },
        "unload_passenger" => Action::UnloadPassenger {
            name: req(0)?,
            target: args.get(1).cloned(),
        },
        "buy" => Action::Buy(BuyRequest {
            item: ItemId(req(0)?),
            quantity: parse_u(name, args, 1)?,
            max_price: parse_optional_u(name, args, 2)?,
            place_order: args.get(3).is_some_and(|v| v == "order"),
            deliver_to: None,
        }),
        "sell" => Action::Sell(SellRequest {
            item: args.first().cloned().map(ItemId),
            quantity: parse_optional_u(name, args, 1)?,
            min_price: parse_optional_u(name, args, 2)?,
            place_order: args.get(3).is_some_and(|v| v == "order"),
        }),
        "cancel_buy" => Action::CancelBuy {
            item: ItemId(req(0)?),
        },
        "cancel_sell" => Action::CancelSell {
            item: ItemId(req(0)?),
        },
        "faction_create" => Action::FactionCreate {
            name: req(0)?,
            tag: req(1)?,
        },
        "faction_invite" => Action::FactionInvite { player: req(0)? },
        "faction_accept_invite" => Action::FactionAcceptInvite { faction: req(0)? },
        "faction_kick" => Action::FactionKick { player: req(0)? },
        "faction_set_role" => Action::FactionSetRole {
            player: req(0)?,
            role: req(1)?,
        },
        "found_station" => Action::FoundStation {
            name: req(0)?,
            public_access: parse_bool(name, args, 1)?,
        },
        "facility_build" => Action::FacilityBuild {
            facility_type: req(0)?,
        },
        "faction_facility_build" => Action::FactionFacilityBuild {
            facility_type: req(0)?,
        },
        "facility_upgrade" => Action::FacilityUpgrade(FacilityUpgradeRequest {
            facility_id: req(0)?,
            facility_type: req(1)?,
        }),
        "faction_facility_upgrade" => Action::FactionFacilityUpgrade(FacilityUpgradeRequest {
            facility_id: req(0)?,
            facility_type: req(1)?,
        }),
        "facility_dismantle" => Action::FacilityDismantle {
            facility_id: req(0)?,
        },
        "faction_facility_dismantle" => Action::FactionFacilityDismantle {
            facility_id: req(0)?,
        },
        "facility_set_access" => Action::FacilitySetAccess(FacilityAccessRequest {
            facility_id: req(0)?,
            access: req(1)?,
        }),
        "facility_set_output_price" => Action::FacilitySetOutputPrice(FacilityOutputPriceRequest {
            facility_id: req(0)?,
            item: ItemId(req(1)?),
            price: parse_u(name, args, 2)?,
        }),
        "facility_set_name" => Action::FacilitySetName(FacilityNameRequest {
            facility_id: req(0)?,
            custom_name: req(1)?,
        }),
        "use_item" => Action::UseItem {
            item: ItemId(req(0)?),
            quantity: parse_optional_u(name, args, 1)?.unwrap_or(1),
        },
        "repair" => Action::Repair(prayer_actions::ServiceTransferRequest {
            target: args.first().cloned(),
            quantity: parse_optional_u(name, args, 2)?,
            item: args.get(1).cloned().map(ItemId),
        }),
        "repair_module" => Action::RepairModule { module: req(0)? },
        "recycle" => Action::Recycle(RecycleRequest {
            recipe_id: req(0)?,
            quantity: args
                .get(1)
                .map(|value| value.parse())
                .transpose()
                .map_err(|_| invalid_args(name))?
                .unwrap_or(1),
            source: None,
            destination: None,
            facility_id: None,
        }),
        "refuel" => {
            let quantity_only = args.first().is_some_and(|v| v.parse::<u64>().is_ok());
            Action::Refuel(prayer_actions::ServiceTransferRequest {
                target: if quantity_only {
                    None
                } else {
                    args.first().cloned()
                },
                quantity: parse_optional_u(name, args, if quantity_only { 0 } else { 1 })?,
                item: None,
            })
        }
        "self_destruct" => Action::SelfDestruct,
        "switch_ship" => Action::SwitchShip { ship: req(0)? },
        "rename_ship" => Action::RenameShip { name: req(0)? },
        "install_mod" => Action::InstallMod { module: req(0)? },
        "uninstall_mod" => Action::UninstallMod { module: req(0)? },
        "buy_ship" => Action::BuyShip { listing: req(0)? },
        "buy_listed_ship" => Action::BuyListedShip { listing: req(0)? },
        "commission_ship" => Action::CommissionShip(CommissionShipRequest {
            ship_class: req(0)?,
            provide_materials: false,
        }),
        "sell_ship" => Action::SellShip { ship: req(0)? },
        "scrap_ship" => Action::ScrapShip { ship: req(0)? },
        "list_ship_for_sale" => Action::ListShipForSale {
            ship: req(0)?,
            price: parse_u(name, args, 1)?,
        },
        "refit_ship" => Action::RefitShip,
        "cancel_commission" => Action::CancelCommission {
            commission_id: req(0)?,
        },
        "supply_commission" => Action::SupplyCommission {
            commission_id: req(0)?,
            item: ItemId(req(1)?),
            quantity: parse_u(name, args, 2)?,
        },
        "cancel_ship_listing" => Action::CancelShipListing {
            listing_id: req(0)?,
        },
        "place_ship_buy_order" => Action::PlaceShipBuyOrder {
            ship_class: req(0)?,
            price: parse_u(name, args, 1)?,
        },
        "cancel_ship_buy_order" => Action::CancelShipBuyOrder { order_id: req(0)? },
        "sell_ship_to_order" => Action::SellShipToOrder {
            order_id: req(0)?,
            ship_id: req(1)?,
        },
        "cancel_order" => Action::CancelOrder { order_id: req(0)? },
        "modify_order" => Action::ModifyOrder {
            order_id: req(0)?,
            price_each: parse_u(name, args, 1)?,
        },
        "craft" => Action::Craft(CraftRequest {
            recipe_id: req(0)?,
            quantity: parse_optional_u(name, args, 1)?.unwrap_or(1),
            source: None,
            destination: None,
            facility_id: None,
            preset: None,
        }),
        "cancel_craft_job" => Action::CancelCraftJob { job_id: req(0)? },
        "salvage_wreck" => Action::SalvageWreck { wreck_id: req(0)? },
        "tow_wreck" => Action::TowWreck { wreck_id: req(0)? },
        "scrap_wreck" => Action::ScrapWreck,
        "sell_wreck" => Action::SellWreck,
        "release_wreck" => Action::ReleaseWreck,
        "insure_ship" => Action::InsureShip {
            ticks: parse_u(name, args, 0)?,
        },
        "citizenship_apply" => Action::CitizenshipApply { empire_id: req(0)? },
        "citizenship_withdraw" => Action::CitizenshipWithdraw { empire_id: req(0)? },
        "citizenship_renounce" => Action::CitizenshipRenounce { empire_id: req(0)? },
        "trade_accept" => Action::TradeAccept { trade_id: req(0)? },
        "trade_offer" => trade_offer_action(name, args)?,
        "faction_leave" => Action::FactionLeave,
        "faction_withdraw_invite" => Action::FactionWithdrawInvite { player: req(0)? },
        "faction_propose_ally" => Action::FactionProposeAlly { faction: req(0)? },
        "faction_accept_ally" => Action::FactionAcceptAlly { faction: req(0)? },
        "faction_remove_ally" => Action::FactionRemoveAlly { faction: req(0)? },
        "faction_declare_war" => Action::FactionDeclareWar {
            faction: req(0)?,
            reason: args.get(1).cloned(),
        },
        "faction_propose_peace" => Action::FactionProposePeace {
            faction: req(0)?,
            message: args.get(1).cloned(),
        },
        "faction_accept_peace" => Action::FactionAcceptPeace { faction: req(0)? },
        "faction_set_enemy" => Action::FactionSetEnemy { faction: req(0)? },
        "faction_remove_enemy" => Action::FactionRemoveEnemy { faction: req(0)? },
        "faction_prepay_tax" => Action::FactionPrepayTax {
            quantity: parse_u(name, args, 0)?,
        },
        "faction_cancel_mission" => Action::FactionCancelMission {
            mission_id: req(0)?,
        },
        "espionage" => Action::Espionage,
        "scan_poi" => Action::ScanPoi { poi_id: req(0)? },
        "distress_signal" => Action::DistressSignal {
            distress_type: args.first().cloned(),
        },
        "say" => Action::Say(SayRequest {
            content: req(0)?,
            channel: req(1)?,
            target: args.get(2).cloned(),
        }),
        _ => {
            return Err(CompileError::Invalid(format!(
                "executable command {name} has no typed lowering"
            )))
        }
    };
    Ok(action)
}

/// Lower one already-analyzed, fully materialized command into the exhaustive
/// durable action protocol. Runtime compatibility adapters use this same
/// boundary; unknown names never become queued work.
pub fn lower_materialized_command(name: &str, args: &[String]) -> Result<Action, CompileError> {
    command_action(name, args)
}

fn invalid_args(name: &str) -> CompileError {
    CompileError::Invalid(format!("invalid arguments for {name}"))
}

fn parse_u(name: &str, args: &[String], index: usize) -> Result<u64, CompileError> {
    args.get(index)
        .ok_or_else(|| invalid_args(name))?
        .parse()
        .map_err(|_| invalid_args(name))
}

fn parse_optional_u(
    name: &str,
    args: &[String],
    index: usize,
) -> Result<Option<u64>, CompileError> {
    args.get(index)
        .map(|value| value.parse().map_err(|_| invalid_args(name)))
        .transpose()
}

fn craft_action(craft: &AnalyzedCraft) -> Result<Action, CompileError> {
    Ok(Action::Craft(CraftRequest {
        recipe_id: craft.recipe_id.clone(),
        quantity: craft.source.quantity,
        source: craft.source.clauses.source.clone(),
        destination: craft.source.clauses.deliver_to.clone(),
        facility_id: craft.source.clauses.facility_id.clone(),
        preset: craft.source.clauses.preset.clone(),
    }))
}

fn recycle_action(recycle: &AnalyzedRecycle) -> Action {
    Action::Recycle(RecycleRequest {
        recipe_id: recycle.recipe_id.clone(),
        quantity: recycle.source.quantity,
        source: recycle.source.clauses.source.clone(),
        destination: recycle.source.clauses.deliver_to.clone(),
        facility_id: recycle.source.clauses.facility_id.clone(),
    })
}

fn transfer_action(transfer: &AnalyzedTransfer) -> Result<Action, CompileError> {
    let subject = if transfer.items.is_empty() {
        match &transfer.subject {
            AnalyzedTransferSubject::AllCargo => ActionSubject::AllCargo,
            AnalyzedTransferSubject::Credits(quantity) => ActionSubject::Credits {
                quantity: positive(*quantity)?,
            },
            AnalyzedTransferSubject::Item { id, qty } => ActionSubject::Item {
                id: ItemId(id.clone()),
                quantity: qty.map(positive).transpose()?,
            },
            AnalyzedTransferSubject::Ship { id } => ActionSubject::Ship { id: id.clone() },
            AnalyzedTransferSubject::Module { id } => ActionSubject::Module { id: id.clone() },
        }
    } else {
        ActionSubject::Items {
            items: transfer
                .items
                .iter()
                .map(|item| {
                    Ok(TransferItem {
                        id: ItemId(item.id.clone()),
                        quantity: positive(item.qty)?,
                    })
                })
                .collect::<Result<_, CompileError>>()?,
        }
    };
    Ok(Action::Transfer(TransferRequest {
        subject,
        from: endpoint(&transfer.from),
        to: endpoint(&transfer.to),
    }))
}

fn positive(value: i64) -> Result<u64, CompileError> {
    u64::try_from(value)
        .map_err(|_| CompileError::Invalid(format!("quantity {value} must be non-negative")))
}

fn parse_bool(name: &str, args: &[String], index: usize) -> Result<bool, CompileError> {
    match args.get(index).map(String::as_str) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err(invalid_args(name)),
    }
}

fn trade_offer_action(name: &str, args: &[String]) -> Result<Action, CompileError> {
    let target = args.first().cloned().ok_or_else(|| invalid_args(name))?;
    let mut request = prayer_actions::TradeOfferRequest {
        target,
        offer_items: vec![],
        offer_credits: None,
        request_items: vec![],
        request_credits: None,
    };
    if args.len() == 1 {
        request.offer_credits = Some(1);
        return Ok(Action::TradeOffer(request));
    }
    let mut idx = 1;
    while idx < args.len() {
        let side = args
            .get(idx)
            .map(String::as_str)
            .ok_or_else(|| invalid_args(name))?;
        if !matches!(side, "offer" | "request") {
            return Err(invalid_args(name));
        }
        idx += 1;
        if args.get(idx).is_some_and(|v| v == "credits") {
            let quantity = parse_u(name, args, idx + 1)?;
            let slot = if side == "offer" {
                &mut request.offer_credits
            } else {
                &mut request.request_credits
            };
            if slot.replace(quantity).is_some() {
                return Err(invalid_args(name));
            }
            idx += 2;
        } else {
            let quantity = parse_u(name, args, idx)?;
            let item = ItemId(
                args.get(idx + 1)
                    .cloned()
                    .ok_or_else(|| invalid_args(name))?,
            );
            let entry = prayer_actions::TradeItem { item, quantity };
            if side == "offer" {
                request.offer_items.push(entry);
            } else {
                request.request_items.push(entry);
            }
            idx += 2;
        }
    }
    if request.offer_items.is_empty()
        && request.offer_credits.is_none()
        && request.request_items.is_empty()
        && request.request_credits.is_none()
    {
        return Err(invalid_args(name));
    }
    Ok(Action::TradeOffer(request))
}

fn endpoint(endpoint: &AnalyzedTransferEndpoint) -> ActionEndpoint {
    match endpoint {
        AnalyzedTransferEndpoint::Cargo => ActionEndpoint::Cargo,
        AnalyzedTransferEndpoint::Storage => ActionEndpoint::Storage,
        AnalyzedTransferEndpoint::Faction => ActionEndpoint::Faction,
        AnalyzedTransferEndpoint::FactionTag(tag) => ActionEndpoint::FactionTag(tag.clone()),
        AnalyzedTransferEndpoint::Player(name) => ActionEndpoint::Player(name.clone()),
        AnalyzedTransferEndpoint::Space(id) => ActionEndpoint::Space(id.clone()),
        AnalyzedTransferEndpoint::Commission(id) => ActionEndpoint::Commission(id.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{catalog::default_command_catalog, AnalysisObservation, AstProgram};

    #[test]
    fn compiles_linear_actions_in_source_order() {
        let program = AstProgram::parse("go alpha_station; dock;").unwrap();
        let analyzed = program
            .analyze(&default_command_catalog(), &AnalysisObservation::default())
            .unwrap();
        let compiled = analyzed.compile().unwrap();
        assert_eq!(compiled.nodes.len(), 2);
        let PlanNode::Action(first) = &compiled.nodes[0];
        assert!(matches!(first.materialize(), Action::Go { .. }));
        let PlanNode::Action(second) = &compiled.nodes[1];
        assert_eq!(second.materialize(), Action::Dock);
    }
}
