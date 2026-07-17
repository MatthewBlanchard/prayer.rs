//! Materialization between kernel actions and executor-ready typed actions.

use prayer_actions::{
    Action, ActionArg, BuyRequest, CraftRequest, GoTarget, ItemId, ResolvedAction, SellRequest,
    TransferEndpoint, TransferItem, TransferRequest, TransferSubject,
};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ActionBridgeError {
    #[error("invalid typed action: {0}")]
    Invalid(String),
}

pub fn resolve_action(action: Action) -> Result<ResolvedAction, ActionBridgeError> {
    let (name, args) = match action {
        Action::Halt => ("halt".to_owned(), vec![]),
        Action::Wait { ticks } => ("wait".to_owned(), vec![integer(ticks)?]),
        Action::Go { destination } => (
            "go".to_owned(),
            vec![ActionArg::GoTarget(go_text(destination))],
        ),
        Action::Dock => ("dock".to_owned(), vec![]),
        Action::Undock => plain("undock"),
        Action::Mine { resource } => (
            "mine".to_owned(),
            resource
                .map(|id| vec![ActionArg::ItemId(id.0)])
                .unwrap_or_default(),
        ),
        Action::Transfer(request) => ("transfer".to_owned(), transfer_args(request)?),
        Action::SetHome => plain("set_home"),
        Action::Find(request) => ("find".into(), any_args(request.targets)),
        Action::Survey => plain("survey"),
        Action::Attack { target_id } => one("attack", target_id),
        Action::Scan { target } => (
            "scan".into(),
            target.map(|v| vec![ActionArg::Any(v)]).unwrap_or_default(),
        ),
        Action::Cloak { enabled } => (
            "cloak".into(),
            vec![ActionArg::Any(if enabled { "on" } else { "off" }.into())],
        ),
        Action::Hunt { target } => one("hunt", target),
        Action::PrepayTax { quantity } => ("prepay_tax".into(), vec![integer(quantity)?]),
        Action::AcceptMission { mission_id } => {
            typed_one("accept_mission", ActionArg::MissionId(mission_id))
        }
        Action::AbandonMission { mission_id } => {
            typed_one("abandon_mission", ActionArg::MissionId(mission_id))
        }
        Action::DeclineMission { template_id } => {
            typed_one("decline_mission", ActionArg::MissionId(template_id))
        }
        Action::CompleteMission { mission_id } => {
            typed_one("complete_mission", ActionArg::MissionId(mission_id))
        }
        Action::LoadPassenger { destination } => {
            typed_one("load_passenger", ActionArg::PoiId(destination))
        }
        Action::UnloadPassenger { name, target } => (
            "unload_passenger".into(),
            std::iter::once(ActionArg::Any(name))
                .chain(target.map(ActionArg::Any))
                .collect(),
        ),
        Action::Buy(request) => ("buy".into(), buy_args(request)?),
        Action::Sell(request) => ("sell".into(), sell_args(request)?),
        Action::CancelBuy { item } => typed_one("cancel_buy", ActionArg::ItemId(item.0)),
        Action::CancelSell { item } => typed_one("cancel_sell", ActionArg::ItemId(item.0)),
        Action::FactionCreate { name, tag } => ("faction_create".into(), any_args(vec![name, tag])),
        Action::FactionInvite { player } => one("faction_invite", player),
        Action::FactionAcceptInvite { faction } => one("faction_accept_invite", faction),
        Action::FactionKick { player } => one("faction_kick", player),
        Action::FactionSetRole { player, role } => {
            ("faction_set_role".into(), any_args(vec![player, role]))
        }
        Action::FacilityBuild { facility_type } => one("facility_build", facility_type),
        Action::FactionFacilityBuild { facility_type } => {
            one("faction_facility_build", facility_type)
        }
        Action::FacilityUpgrade(request) => (
            "facility_upgrade".into(),
            any_args(vec![request.facility_id, request.facility_type]),
        ),
        Action::FactionFacilityUpgrade(request) => (
            "faction_facility_upgrade".into(),
            any_args(vec![request.facility_id, request.facility_type]),
        ),
        Action::FacilityDismantle { facility_id } => one("facility_dismantle", facility_id),
        Action::FactionFacilityDismantle { facility_id } => {
            one("faction_facility_dismantle", facility_id)
        }
        Action::FacilitySetAccess(request) => (
            "facility_set_access".into(),
            any_args(vec![request.facility_id, request.access]),
        ),
        Action::FacilitySetOutputPrice(request) => (
            "facility_set_output_price".into(),
            vec![
                ActionArg::Any(request.facility_id),
                ActionArg::ItemId(request.item.0),
                integer(request.price)?,
            ],
        ),
        Action::FacilitySetName(request) => (
            "facility_set_name".into(),
            any_args(vec![request.facility_id, request.custom_name]),
        ),
        Action::UseItem { item, quantity } => (
            "use_item".into(),
            vec![ActionArg::ItemId(item.0), integer(quantity)?],
        ),
        Action::Repair(request) => ("repair".into(), service_args(request)?),
        Action::RepairModule { module } => typed_one("repair_module", ActionArg::ModuleId(module)),
        Action::Recycle(request) => (
            "recycle".into(),
            routed_job_args(
                request.recipe_id,
                request.quantity,
                request.source,
                request.destination,
                request.facility_id,
                None,
            )?,
        ),
        Action::Refuel(request) => ("refuel".into(), service_args(request)?),
        Action::SelfDestruct => plain("self_destruct"),
        Action::SwitchShip { ship } => typed_one("switch_ship", ActionArg::ShipId(ship)),
        Action::RenameShip { name } => one("rename_ship", name),
        Action::InstallMod { module } => typed_one("install_mod", ActionArg::ModuleId(module)),
        Action::UninstallMod { module } => typed_one("uninstall_mod", ActionArg::ModuleId(module)),
        Action::BuyShip { listing } => typed_one("buy_ship", ActionArg::ListingId(listing)),
        Action::BuyListedShip { listing } => {
            typed_one("buy_listed_ship", ActionArg::ListingId(listing))
        }
        Action::CommissionShip(request) => {
            let mut args = vec![ActionArg::Any(request.ship_class)];
            if request.provide_materials {
                args.push(ActionArg::Any("materials".into()));
            }
            ("commission_ship".into(), args)
        }
        Action::SellShip { ship } => typed_one("sell_ship", ActionArg::ShipId(ship)),
        Action::ScrapShip { ship } => typed_one("scrap_ship", ActionArg::ShipId(ship)),
        Action::ListShipForSale { ship, price } => (
            "list_ship_for_sale".into(),
            vec![ActionArg::ShipId(ship), integer(price)?],
        ),
        Action::RefitShip => plain("refit_ship"),
        Action::CancelCommission { commission_id } => one("cancel_commission", commission_id),
        Action::SupplyCommission {
            commission_id,
            item,
            quantity,
        } => (
            "supply_commission".into(),
            vec![
                ActionArg::Any(commission_id),
                ActionArg::ItemId(item.0),
                integer(quantity)?,
            ],
        ),
        Action::CancelShipListing { listing_id } => one("cancel_ship_listing", listing_id),
        Action::PlaceShipBuyOrder { ship_class, price } => (
            "place_ship_buy_order".into(),
            vec![ActionArg::Any(ship_class), integer(price)?],
        ),
        Action::CancelShipBuyOrder { order_id } => one("cancel_ship_buy_order", order_id),
        Action::SellShipToOrder { order_id, ship_id } => (
            "sell_ship_to_order".into(),
            vec![ActionArg::Any(order_id), ActionArg::ShipId(ship_id)],
        ),
        Action::CancelOrder { order_id } => one("cancel_order", order_id),
        Action::ModifyOrder {
            order_id,
            price_each,
        } => (
            "modify_order".into(),
            vec![ActionArg::Any(order_id), integer(price_each)?],
        ),
        Action::Craft(request) => ("craft".to_owned(), craft_args(request)?),
        Action::CancelCraftJob { job_id } => one("cancel_craft_job", job_id),
        Action::SalvageWreck { wreck_id } => one("salvage_wreck", wreck_id),
        Action::TowWreck { wreck_id } => one("tow_wreck", wreck_id),
        Action::ScrapWreck => plain("scrap_wreck"),
        Action::SellWreck => plain("sell_wreck"),
        Action::ReleaseWreck => plain("release_wreck"),
        Action::InsureShip { ticks } => ("insure_ship".into(), vec![integer(ticks)?]),
        Action::CitizenshipApply { empire_id } => one("citizenship_apply", empire_id),
        Action::CitizenshipWithdraw { empire_id } => one("citizenship_withdraw", empire_id),
        Action::CitizenshipRenounce { empire_id } => one("citizenship_renounce", empire_id),
        Action::TradeOffer(request) => {
            let encoded = serde_json::to_string(&request)
                .map_err(|e| ActionBridgeError::Invalid(e.to_string()))?;
            ("trade_offer".into(), vec![ActionArg::Any(encoded)])
        }
        Action::TradeAccept { trade_id } => one("trade_accept", trade_id),
        Action::FactionLeave => plain("faction_leave"),
        Action::FactionWithdrawInvite { player } => one("faction_withdraw_invite", player),
        Action::FactionProposeAlly { faction } => one("faction_propose_ally", faction),
        Action::FactionAcceptAlly { faction } => one("faction_accept_ally", faction),
        Action::FactionRemoveAlly { faction } => one("faction_remove_ally", faction),
        Action::FactionDeclareWar { faction, reason } => (
            "faction_declare_war".into(),
            std::iter::once(ActionArg::Any(faction))
                .chain(reason.map(ActionArg::Any))
                .collect(),
        ),
        Action::FactionProposePeace { faction, message } => (
            "faction_propose_peace".into(),
            std::iter::once(ActionArg::Any(faction))
                .chain(message.map(ActionArg::Any))
                .collect(),
        ),
        Action::FactionAcceptPeace { faction } => one("faction_accept_peace", faction),
        Action::FactionSetEnemy { faction } => one("faction_set_enemy", faction),
        Action::FactionRemoveEnemy { faction } => one("faction_remove_enemy", faction),
        Action::FactionPrepayTax { quantity } => {
            ("faction_prepay_tax".into(), vec![integer(quantity)?])
        }
        Action::FactionCancelMission { mission_id } => one("faction_cancel_mission", mission_id),
        Action::Espionage => plain("espionage"),
        Action::ScanPoi { poi_id } => one("scan_poi", poi_id),
        Action::DistressSignal { distress_type } => (
            "distress_signal".into(),
            distress_type
                .map(|v| vec![ActionArg::Any(v)])
                .unwrap_or_default(),
        ),
        Action::Say(request) => {
            let mut args = any_args(vec![request.content, request.channel]);
            if let Some(target) = request.target {
                args.push(ActionArg::Any(target));
            }
            ("say".into(), args)
        }
    };
    Ok(ResolvedAction {
        action: name,
        args,
        source_line: None,
    })
}

fn plain(name: &str) -> (String, Vec<ActionArg>) {
    (name.into(), vec![])
}
fn service_args(
    request: prayer_actions::ServiceTransferRequest,
) -> Result<Vec<ActionArg>, ActionBridgeError> {
    let mut args = Vec::new();
    if let Some(target) = request.target {
        args.push(ActionArg::Any(target));
    }
    if let Some(item) = request.item {
        args.push(ActionArg::ItemId(item.0));
    }
    if let Some(quantity) = request.quantity {
        args.push(integer(quantity)?);
    }
    Ok(args)
}
fn one(name: &str, value: String) -> (String, Vec<ActionArg>) {
    typed_one(name, ActionArg::Any(value))
}
fn typed_one(name: &str, value: ActionArg) -> (String, Vec<ActionArg>) {
    (name.into(), vec![value])
}
fn any_args(values: Vec<String>) -> Vec<ActionArg> {
    values.into_iter().map(ActionArg::Any).collect()
}

fn integer(value: u64) -> Result<ActionArg, ActionBridgeError> {
    i64::try_from(value)
        .map(ActionArg::Integer)
        .map_err(|_| ActionBridgeError::Invalid(format!("integer {value} exceeds runtime range")))
}

fn go_text(target: GoTarget) -> String {
    match target {
        GoTarget::Identifier(value) | GoTarget::System(value) | GoTarget::Poi(value) => value,
        GoTarget::Coordinate { x, y } => format!("{x},{y}"),
    }
}

fn buy_args(request: BuyRequest) -> Result<Vec<ActionArg>, ActionBridgeError> {
    let mut args = vec![
        ActionArg::ItemId(request.item.0),
        integer(request.quantity)?,
    ];
    if let Some(price) = request.max_price {
        args.push(integer(price)?);
    }
    if request.place_order {
        args.push(ActionArg::Any("order".into()));
    }
    if let Some(destination) = request.deliver_to {
        args.push(ActionArg::Any(format!("deliver_to={destination}")));
    }
    Ok(args)
}

fn sell_args(request: SellRequest) -> Result<Vec<ActionArg>, ActionBridgeError> {
    let mut args = Vec::new();
    if let Some(item) = request.item {
        args.push(ActionArg::ItemId(item.0));
    }
    if let Some(quantity) = request.quantity {
        args.push(integer(quantity)?);
    }
    if let Some(price) = request.min_price {
        args.push(integer(price)?);
    }
    if request.place_order {
        args.push(ActionArg::Any("order".into()));
    }
    Ok(args)
}

fn craft_args(request: CraftRequest) -> Result<Vec<ActionArg>, ActionBridgeError> {
    routed_job_args(
        request.recipe_id,
        request.quantity,
        request.source,
        request.destination,
        request.facility_id,
        request.preset,
    )
}

fn routed_job_args(
    recipe_id: String,
    quantity: u64,
    source: Option<String>,
    destination: Option<String>,
    facility_id: Option<String>,
    preset: Option<String>,
) -> Result<Vec<ActionArg>, ActionBridgeError> {
    let mut args = vec![ActionArg::RecipeId(recipe_id), integer(quantity)?];
    if let Some(value) = destination {
        args.push(ActionArg::Any(format!("deliver_to={value}")));
    }
    if let Some(value) = source {
        args.push(ActionArg::Any(format!("source={value}")));
    }
    if let Some(value) = facility_id {
        args.push(ActionArg::Any(format!("facility_id={value}")));
    }
    if let Some(value) = preset {
        args.push(ActionArg::Any(format!("preset={value}")));
    }
    Ok(args)
}

fn transfer_args(request: TransferRequest) -> Result<Vec<ActionArg>, ActionBridgeError> {
    let mut args = match request.subject {
        TransferSubject::AllCargo => vec![ActionArg::Any("all".into())],
        TransferSubject::Credits { quantity } => {
            vec![ActionArg::Any("credits".into()), integer(quantity)?]
        }
        TransferSubject::Item { id, quantity } => vec![
            ActionArg::Any("item".into()),
            ActionArg::ItemId(id.0),
            quantity
                .map(integer)
                .transpose()?
                .unwrap_or_else(|| ActionArg::Any("all".into())),
        ],
        TransferSubject::Ship { id } => {
            vec![ActionArg::Any("ship".into()), ActionArg::ShipId(id)]
        }
        TransferSubject::Module { id } => {
            vec![ActionArg::Any("module".into()), ActionArg::ModuleId(id)]
        }
        TransferSubject::Items { items } => {
            let mut values = vec![ActionArg::Any("items".into()), integer(items.len() as u64)?];
            for TransferItem { id, quantity } in items {
                values.push(ActionArg::ItemId(id.0));
                values.push(integer(quantity)?);
            }
            values
        }
    };
    args.push(ActionArg::Any(endpoint_text(request.from)));
    args.push(ActionArg::Any(endpoint_text(request.to)));
    Ok(args)
}

fn endpoint_text(endpoint: TransferEndpoint) -> String {
    match endpoint {
        TransferEndpoint::Cargo => "cargo".into(),
        TransferEndpoint::Storage => "storage".into(),
        TransferEndpoint::Ship(id) => format!("ship:{id}"),
        TransferEndpoint::Faction => "faction".into(),
        TransferEndpoint::FactionTag(tag) => format!("faction:{tag}"),
        TransferEndpoint::Player(name) => format!("player:{name}"),
        TransferEndpoint::Space(Some(id)) => format!("space:{id}"),
        TransferEndpoint::Space(None) => "space".into(),
        TransferEndpoint::Commission(id) => format!("commission:{id}"),
    }
}

pub fn materialize_action(command: &ResolvedAction) -> Result<Action, ActionBridgeError> {
    if command.action.eq_ignore_ascii_case("transfer") {
        return typed_transfer(&command.args)
            .ok_or_else(|| ActionBridgeError::Invalid("invalid arguments for transfer".into()));
    }
    let args = command.args_as_strings();
    prayer_lang::lower_materialized_command(&command.action, &args)
        .map_err(|error| ActionBridgeError::Invalid(error.to_string()))
}

fn text(arg: &ActionArg) -> Option<&str> {
    arg.as_str()
}

fn unsigned(arg: &ActionArg) -> Option<u64> {
    match arg {
        ActionArg::Integer(value) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn item_id(arg: &ActionArg) -> Result<String, ActionBridgeError> {
    match arg {
        ActionArg::ItemId(value) => Ok(value.clone()),
        _ => Err(ActionBridgeError::Invalid(
            "expected item_id argument".into(),
        )),
    }
}

fn typed_transfer(args: &[ActionArg]) -> Option<Action> {
    if args.len() < 3 {
        return None;
    }
    let from = parse_endpoint(text(args.get(args.len() - 2)?)?)?;
    let to = parse_endpoint(text(args.last()?)?)?;
    let subject = match text(args.first()?)? {
        "all" if args.len() == 3 => TransferSubject::AllCargo,
        "credits" if args.len() == 4 => TransferSubject::Credits {
            quantity: unsigned(args.get(1)?)?,
        },
        "item" if args.len() == 5 => TransferSubject::Item {
            id: ItemId(item_id(args.get(1)?).ok()?),
            quantity: match text(args.get(2)?).or_else(|| Some(""))? {
                "all" => None,
                _ => Some(unsigned(args.get(2)?)?),
            },
        },
        "ship" if args.len() == 4 => TransferSubject::Ship {
            id: match args.get(1)? {
                ActionArg::ShipId(id) => id.clone(),
                _ => return None,
            },
        },
        "items" => {
            let count = usize::try_from(unsigned(args.get(1)?)?).ok()?;
            if args.len() != count.saturating_mul(2).saturating_add(4) {
                return None;
            }
            let mut items = Vec::with_capacity(count);
            for index in 0..count {
                items.push(TransferItem {
                    id: ItemId(item_id(args.get(2 + index * 2)?).ok()?),
                    quantity: unsigned(args.get(3 + index * 2)?)?,
                });
            }
            TransferSubject::Items { items }
        }
        _ => return None,
    };
    Some(Action::Transfer(TransferRequest { subject, from, to }))
}

fn parse_endpoint(value: &str) -> Option<TransferEndpoint> {
    match value {
        "cargo" => Some(TransferEndpoint::Cargo),
        "storage" => Some(TransferEndpoint::Storage),
        "faction" => Some(TransferEndpoint::Faction),
        "space" => Some(TransferEndpoint::Space(None)),
        _ => value
            .strip_prefix("faction:")
            .map(|tag| TransferEndpoint::FactionTag(tag.to_owned()))
            .or_else(|| {
                value
                    .strip_prefix("player:")
                    .map(|name| TransferEndpoint::Player(name.to_owned()))
            })
            .or_else(|| {
                value
                    .strip_prefix("space:")
                    .map(|id| TransferEndpoint::Space(Some(id.to_owned())))
            })
            .or_else(|| {
                value
                    .strip_prefix("ship:")
                    .map(|id| TransferEndpoint::Ship(id.to_owned()))
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrated_action_round_trips_through_resolved_action() {
        let actions = vec![
            Action::Halt,
            Action::Wait { ticks: 3 },
            Action::Go {
                destination: GoTarget::Identifier("sol".into()),
            },
            Action::Dock,
            Action::Mine {
                resource: Some(ItemId("iron_ore".into())),
            },
            Action::Transfer(TransferRequest {
                subject: TransferSubject::Items {
                    items: vec![TransferItem {
                        id: ItemId("iron_ore".into()),
                        quantity: 4,
                    }],
                },
                from: TransferEndpoint::Cargo,
                to: TransferEndpoint::FactionTag("nova".into()),
            }),
        ];
        for action in actions {
            let command = resolve_action(action.clone()).expect("bridge");
            assert_eq!(materialize_action(&command).expect("materialize"), action);
        }
    }

    #[test]
    fn dedicated_action_preserves_typed_fields_through_executor_adapter() {
        let action = Action::CommissionShip(prayer_actions::CommissionShipRequest {
            ship_class: "corvette".into(),
            provide_materials: false,
        });
        let command = resolve_action(action.clone()).expect("adapter");
        assert_eq!(materialize_action(&command).expect("materialize"), action);
    }

    #[test]
    fn malformed_command_does_not_escape_into_queue() {
        let invalid_wait = ResolvedAction {
            action: "wait".into(),
            args: vec![ActionArg::Any("tomorrow".into())],
            source_line: None,
        };
        assert!(matches!(
            materialize_action(&invalid_wait),
            Err(ActionBridgeError::Invalid(_))
        ));

        let invalid_transfer = ResolvedAction {
            action: "transfer".into(),
            args: vec![ActionArg::Any("item".into())],
            source_line: None,
        };
        assert!(matches!(
            materialize_action(&invalid_transfer),
            Err(ActionBridgeError::Invalid(_))
        ));
    }

    #[test]
    fn arbitrage_buy_materializes_with_quantity_and_price_cap() {
        let command = ResolvedAction {
            action: "buy".into(),
            args: vec![
                ActionArg::ItemId("navigation_core".into()),
                ActionArg::Integer(10),
                ActionArg::Integer(3409),
            ],
            source_line: None,
        };
        assert_eq!(
            materialize_action(&command).expect("typed buy"),
            Action::Buy(BuyRequest {
                item: ItemId("navigation_core".into()),
                quantity: 10,
                max_price: Some(3409),
                place_order: false,
                deliver_to: None,
            })
        );
    }
}
