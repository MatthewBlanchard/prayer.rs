//! Source analysis against a narrow, runtime-independent observation.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{
    ArgType, AstNode, AstProgram, BuyNode, CommandNode, CommandSpec, CommissionShipNode, CraftNode,
    RecycleNode, SayNode, SellNode, Span, TransferEndpoint, TransferNode, TransferSubject,
};

/// Identifier candidates visible while compiling PrayerLang source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisObservation {
    pub system: Option<String>,
    pub item_ids: Vec<String>,
    pub poi_ids: Vec<String>,
    pub system_ids: Vec<String>,
    pub mission_ids: Vec<String>,
    pub ship_ids: Vec<String>,
    pub owned_ship_ids: HashSet<String>,
    pub module_ids: Vec<String>,
    pub recipe_ids: Vec<String>,
    pub listing_ids: Vec<String>,
}

/// Analyzer argument form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyzedArg {
    /// Canonical/resolved static value.
    Resolved(String),
}

/// Analyzer node tree mirroring `AstNode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyzedNode {
    /// Command with resolved literal arguments.
    Command(AnalyzedCommand),
    /// Transfer statement.
    Transfer(AnalyzedTransfer),
    /// Craft statement.
    Craft(AnalyzedCraft),
    Say(SayNode),
    Buy(BuyNode),
    Sell(SellNode),
    Recycle(AnalyzedRecycle),
    CommissionShip(CommissionShipNode),
}

/// Analyzer command payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedCommand {
    /// Original source command node.
    pub source: CommandNode,
    /// Resolved or dynamic arguments.
    pub args: Vec<AnalyzedArg>,
}

/// Analyzer transfer payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedTransfer {
    /// Source transfer node.
    pub source: TransferNode,
    /// Resolved subject.
    pub subject: AnalyzedTransferSubject,
    /// Resolved block-form item entries.
    pub items: Vec<AnalyzedTransferItem>,
    /// Resolved source endpoint.
    pub from: AnalyzedTransferEndpoint,
    /// Resolved destination endpoint.
    pub to: AnalyzedTransferEndpoint,
}

/// Analyzer craft payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedCraft {
    /// Source craft node.
    pub source: CraftNode,
    /// Resolved recipe id.
    pub recipe_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedRecycle {
    pub source: RecycleNode,
    pub recipe_id: String,
}

/// Resolved block-form transfer item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedTransferItem {
    /// Item id.
    pub id: String,
    /// Item quantity.
    pub qty: i64,
}

/// Resolved transfer subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyzedTransferSubject {
    /// All current cargo.
    AllCargo,
    /// Credits quantity.
    Credits(i64),
    /// Item id and optional quantity.
    Item {
        id: String,
        qty: Option<i64>,
    },
    /// Owned ship instance id.
    Ship {
        id: String,
    },
    Module {
        id: String,
    },
}

/// Resolved transfer endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyzedTransferEndpoint {
    /// Ship cargo.
    Cargo,
    /// Personal station storage.
    Storage,
    /// Current player's faction storage.
    Faction,
    /// Named faction storage.
    FactionTag(String),
    /// Named player.
    Player(String),
    /// Visible space loot at the current POI, optionally narrowed to one id.
    Space(Option<String>),
    Commission(String),
}

/// Analyzer output program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedProgram {
    /// Top-level analyzer nodes.
    pub statements: Vec<AnalyzedNode>,
}

/// Analyzer error surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzerError {
    /// Command name where error occurred.
    pub command: String,
    /// Argument index (0-based).
    pub arg_index: usize,
    /// Original argument token.
    pub value: String,
    /// Optional suggestion text.
    pub suggestion: Option<String>,
    /// Source span of command.
    pub span: Span,
    /// Human-readable error.
    pub message: String,
}

/// Analyze parsed AST into resolved literal arguments.
pub fn analyze(
    program: &AstProgram,
    catalog: &HashMap<String, CommandSpec>,
    state: &AnalysisObservation,
) -> Result<AnalyzedProgram, Vec<AnalyzerError>> {
    let mut errors = Vec::new();
    let statements = analyze_nodes(&program.statements, catalog, state, &mut errors);

    if errors.is_empty() {
        Ok(AnalyzedProgram { statements })
    } else {
        Err(errors)
    }
}

fn analyze_nodes(
    nodes: &[AstNode],
    catalog: &HashMap<String, CommandSpec>,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> Vec<AnalyzedNode> {
    nodes
        .iter()
        .map(|node| match node {
            AstNode::Command(cmd) => {
                let args = analyze_command_args(cmd, catalog, state, errors);
                AnalyzedNode::Command(AnalyzedCommand {
                    source: cmd.clone(),
                    args,
                })
            }
            AstNode::Transfer(transfer) => {
                AnalyzedNode::Transfer(analyze_transfer(transfer, state, errors))
            }
            AstNode::Craft(craft) => AnalyzedNode::Craft(analyze_craft(craft, state, errors)),
            AstNode::Say(say) => {
                let private = say.channel == "private";
                if !matches!(
                    say.channel.as_str(),
                    "system" | "local" | "faction" | "private"
                ) {
                    errors.push(typed_statement_error(
                        "say",
                        say.span,
                        format!(
                            "unknown say channel '{}'; expected system, local, faction, or private",
                            say.channel
                        ),
                    ));
                } else if private && say.target.is_none() {
                    errors.push(typed_statement_error(
                        "say",
                        say.span,
                        "private say requires a target player",
                    ));
                } else if !private && say.target.is_some() {
                    errors.push(typed_statement_error(
                        "say",
                        say.span,
                        "only private say accepts a target player",
                    ));
                }
                AnalyzedNode::Say(say.clone())
            }
            AstNode::Buy(buy) => AnalyzedNode::Buy(buy.clone()),
            AstNode::Sell(sell) => AnalyzedNode::Sell(sell.clone()),
            AstNode::Recycle(recycle) => {
                let recipe_id = resolve_typed_identity(
                    "recycle",
                    ArgType::RecipeId,
                    &recycle.recipe_id,
                    recycle.span,
                    state,
                    errors,
                );
                AnalyzedNode::Recycle(AnalyzedRecycle {
                    source: recycle.clone(),
                    recipe_id,
                })
            }
            AstNode::CommissionShip(commission) => AnalyzedNode::CommissionShip(commission.clone()),
        })
        .collect()
}

fn resolve_typed_identity(
    command: &str,
    kind: ArgType,
    value: &str,
    span: Span,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> String {
    match resolve_identity(kind, value, state) {
        Some((resolved, _, None)) => resolved,
        Some((_, suggestion, Some(message))) => {
            errors.push(AnalyzerError {
                command: command.to_string(),
                arg_index: 0,
                value: value.to_string(),
                suggestion,
                span,
                message,
            });
            value.to_string()
        }
        None => value.to_string(),
    }
}

fn typed_statement_error(command: &str, span: Span, message: impl Into<String>) -> AnalyzerError {
    AnalyzerError {
        command: command.into(),
        arg_index: 0,
        value: String::new(),
        suggestion: None,
        span,
        message: message.into(),
    }
}

fn analyze_craft(
    craft: &CraftNode,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> AnalyzedCraft {
    let recipe_id = match resolve_identity(ArgType::RecipeId, &craft.recipe_id, state) {
        Some((resolved, _, None)) => resolved,
        Some((_, suggestion, Some(message))) => {
            errors.push(AnalyzerError {
                command: "craft".to_string(),
                arg_index: 0,
                value: craft.recipe_id.clone(),
                suggestion,
                span: craft.span,
                message,
            });
            craft.recipe_id.clone()
        }
        None => craft.recipe_id.clone(),
    };

    AnalyzedCraft {
        source: craft.clone(),
        recipe_id,
    }
}

fn analyze_transfer(
    transfer: &TransferNode,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> AnalyzedTransfer {
    let (from, to) = resolve_transfer_endpoints(transfer);
    let items = analyze_transfer_items(transfer, state, errors);
    let subject = match &transfer.subject {
        TransferSubject::AllCargo => AnalyzedTransferSubject::AllCargo,
        TransferSubject::Credits(qty) => {
            if *qty <= 0 {
                push_transfer_error(errors, transfer.span, "credits quantity must be positive");
            }
            AnalyzedTransferSubject::Credits(*qty)
        }
        TransferSubject::Ship { id } => AnalyzedTransferSubject::Ship { id: id.clone() },
        TransferSubject::Module { id } => AnalyzedTransferSubject::Module { id: id.clone() },
        TransferSubject::Item { id, qty } => {
            if matches!(qty, Some(qty) if *qty <= 0) {
                push_transfer_error(errors, transfer.span, "item quantity must be positive");
            }
            let allow_ship_gift =
                qty.is_none() && matches!(to, AnalyzedTransferEndpoint::Player(_));
            let resolved = resolve_transfer_item_id(id, allow_ship_gift, transfer, state, errors);
            if allow_ship_gift
                && resolved
                    .as_ref()
                    .is_some_and(|resolved| state.owned_ship_ids.contains(resolved))
            {
                AnalyzedTransferSubject::Ship {
                    id: resolved.unwrap_or_else(|| id.clone()),
                }
            } else {
                AnalyzedTransferSubject::Item {
                    id: resolved.unwrap_or_else(|| id.clone()),
                    qty: *qty,
                }
            }
        }
    };

    validate_transfer_pair(transfer, &subject, &items, &from, &to, errors);

    AnalyzedTransfer {
        source: transfer.clone(),
        subject,
        items,
        from,
        to,
    }
}

fn analyze_transfer_items(
    transfer: &TransferNode,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> Vec<AnalyzedTransferItem> {
    transfer
        .items
        .iter()
        .map(|item| {
            if item.qty <= 0 {
                push_transfer_error(errors, transfer.span, "item quantity must be positive");
            }
            let resolved = resolve_transfer_item_id(&item.id, false, transfer, state, errors)
                .unwrap_or_else(|| item.id.clone());
            AnalyzedTransferItem {
                id: resolved,
                qty: item.qty,
            }
        })
        .collect()
}

fn resolve_transfer_item_id(
    id: &str,
    allow_ship_gift: bool,
    transfer: &TransferNode,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> Option<String> {
    let item_result = resolve_identity(ArgType::ItemId, id, state);
    match item_result {
        Some((resolved, _, None)) => Some(resolved),
        Some((_, item_suggestion, Some(item_message))) => {
            if allow_ship_gift {
                if let Some((resolved, _, None)) = resolve_identity(ArgType::ShipId, id, state) {
                    return Some(resolved);
                }
            }
            errors.push(AnalyzerError {
                command: "transfer".to_string(),
                arg_index: 0,
                value: id.to_string(),
                suggestion: item_suggestion,
                span: transfer.span,
                message: item_message,
            });
            None
        }
        None if allow_ship_gift => resolve_identity(ArgType::ShipId, id, state).and_then(
            |(resolved, suggestion, message)| {
                if let Some(message) = message {
                    errors.push(AnalyzerError {
                        command: "transfer".to_string(),
                        arg_index: 0,
                        value: id.to_string(),
                        suggestion,
                        span: transfer.span,
                        message,
                    });
                    None
                } else {
                    Some(resolved)
                }
            },
        ),
        None => None,
    }
}

fn resolve_transfer_endpoints(
    transfer: &TransferNode,
) -> (AnalyzedTransferEndpoint, AnalyzedTransferEndpoint) {
    match (&transfer.from, &transfer.to, &transfer.subject) {
        (None, None, TransferSubject::Credits(_)) => (
            AnalyzedTransferEndpoint::Cargo,
            AnalyzedTransferEndpoint::Cargo,
        ),
        (None, None, _) => (
            AnalyzedTransferEndpoint::Cargo,
            AnalyzedTransferEndpoint::Storage,
        ),
        (Some(from), None, TransferSubject::Credits(_)) => {
            (transfer_endpoint(from), AnalyzedTransferEndpoint::Cargo)
        }
        (Some(from), None, _) => (transfer_endpoint(from), AnalyzedTransferEndpoint::Cargo),
        (None, Some(to), TransferSubject::Credits(_)) => {
            (AnalyzedTransferEndpoint::Cargo, transfer_endpoint(to))
        }
        (None, Some(to), _) => (AnalyzedTransferEndpoint::Cargo, transfer_endpoint(to)),
        (Some(from), Some(to), _) => (transfer_endpoint(from), transfer_endpoint(to)),
    }
}

fn transfer_endpoint(endpoint: &TransferEndpoint) -> AnalyzedTransferEndpoint {
    match endpoint {
        TransferEndpoint::Cargo => AnalyzedTransferEndpoint::Cargo,
        TransferEndpoint::Storage => AnalyzedTransferEndpoint::Storage,
        TransferEndpoint::Faction => AnalyzedTransferEndpoint::Faction,
        TransferEndpoint::FactionTag(tag) => AnalyzedTransferEndpoint::FactionTag(tag.clone()),
        TransferEndpoint::Player(name) => AnalyzedTransferEndpoint::Player(name.clone()),
        TransferEndpoint::Space(id) => AnalyzedTransferEndpoint::Space(id.clone()),
        TransferEndpoint::Commission(id) => AnalyzedTransferEndpoint::Commission(id.clone()),
    }
}

fn validate_transfer_pair(
    transfer: &TransferNode,
    subject: &AnalyzedTransferSubject,
    items: &[AnalyzedTransferItem],
    from: &AnalyzedTransferEndpoint,
    to: &AnalyzedTransferEndpoint,
    errors: &mut Vec<AnalyzerError>,
) {
    if from == to {
        push_transfer_error(
            errors,
            transfer.span,
            "transfer endpoints must be different",
        );
    }

    if matches!(subject, AnalyzedTransferSubject::Credits(_)) {
        for endpoint in [from, to] {
            if matches!(
                endpoint,
                AnalyzedTransferEndpoint::Storage | AnalyzedTransferEndpoint::Space(_)
            ) {
                push_transfer_error(
                    errors,
                    transfer.span,
                    "credits can only move between cargo, faction treasury, or player endpoints",
                );
            }
        }
    }

    if matches!(subject, AnalyzedTransferSubject::Ship { .. }) {
        let supported = matches!(
            (from, to),
            (
                AnalyzedTransferEndpoint::Cargo | AnalyzedTransferEndpoint::Storage,
                AnalyzedTransferEndpoint::Player(_) | AnalyzedTransferEndpoint::Faction
            )
        );
        if !supported {
            push_transfer_error(
                errors,
                transfer.span,
                "ships can only be transferred from cargo/storage to a player or faction garage",
            );
        }
    }
    if matches!(subject, AnalyzedTransferSubject::Module { .. })
        && !matches!(
            (from, to),
            (
                AnalyzedTransferEndpoint::Space(Some(_)),
                AnalyzedTransferEndpoint::Cargo
            )
        )
    {
        push_transfer_error(
            errors,
            transfer.span,
            "modules can only be transferred from a named space wreck to cargo",
        );
    }
    if matches!(to, AnalyzedTransferEndpoint::Commission(_))
        && (!matches!(from, AnalyzedTransferEndpoint::Cargo)
            || !matches!(subject, AnalyzedTransferSubject::Item { qty: Some(_), .. }))
    {
        push_transfer_error(
            errors,
            transfer.span,
            "commission transfers require a quantity item from cargo",
        );
    }

    if !items.is_empty() {
        if !matches!(subject, AnalyzedTransferSubject::AllCargo) {
            push_transfer_error(
                errors,
                transfer.span,
                "transfer block items cannot be mixed with a flat subject",
            );
        }
        if !matches!(from, AnalyzedTransferEndpoint::Cargo) {
            push_transfer_error(
                errors,
                transfer.span,
                "transfer block items can only move from cargo",
            );
        }
        if matches!(to, AnalyzedTransferEndpoint::Cargo) {
            push_transfer_error(
                errors,
                transfer.span,
                "transfer block items require a storage, faction, player, or space destination",
            );
        }
    }

    if matches!(from, AnalyzedTransferEndpoint::Space(_))
        && !matches!(to, AnalyzedTransferEndpoint::Cargo)
    {
        push_transfer_error(
            errors,
            transfer.span,
            "space loot can only be transferred into cargo",
        );
    }

    if matches!(to, AnalyzedTransferEndpoint::Space(_))
        && !matches!(from, AnalyzedTransferEndpoint::Cargo)
    {
        push_transfer_error(
            errors,
            transfer.span,
            "space transfers can only jettison from cargo",
        );
    }
}

fn push_transfer_error(errors: &mut Vec<AnalyzerError>, span: Span, message: &str) {
    errors.push(AnalyzerError {
        command: "transfer".to_string(),
        arg_index: 0,
        value: "transfer".to_string(),
        suggestion: None,
        span,
        message: message.to_string(),
    });
}

fn analyze_command_args(
    cmd: &CommandNode,
    catalog: &HashMap<String, CommandSpec>,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> Vec<AnalyzedArg> {
    let spec = catalog.get(&cmd.name.to_lowercase());
    let variadic_kind = spec
        .and_then(|s| s.args.last())
        .filter(|a| a.variadic)
        .map(|a| a.kind);

    cmd.args
        .iter()
        .enumerate()
        .map(|(idx, arg)| {
            let arg_type = spec
                .and_then(|s| s.args.get(idx))
                .map(|a| a.kind)
                .or(variadic_kind)
                .unwrap_or(ArgType::Any);
            analyze_arg(cmd, idx, arg, arg_type, state, errors)
        })
        .collect()
}

fn analyze_arg(
    cmd: &CommandNode,
    idx: usize,
    arg: &str,
    arg_type: ArgType,
    state: &AnalysisObservation,
    errors: &mut Vec<AnalyzerError>,
) -> AnalyzedArg {
    if arg_type == ArgType::Integer {
        return match arg.parse::<i64>() {
            Ok(v) => AnalyzedArg::Resolved(v.to_string()),
            Err(_) => {
                errors.push(AnalyzerError {
                    command: cmd.name.clone(),
                    arg_index: idx,
                    value: arg.to_string(),
                    suggestion: None,
                    span: cmd.span,
                    message: format!("expected integer argument, got '{arg}'"),
                });
                AnalyzedArg::Resolved(arg.to_string())
            }
        };
    }

    if let Some((resolved, suggestion, message)) = resolve_identity(arg_type, arg, state) {
        if let Some(message) = message {
            errors.push(AnalyzerError {
                command: cmd.name.clone(),
                arg_index: idx,
                value: arg.to_string(),
                suggestion,
                span: cmd.span,
                message,
            });
            return AnalyzedArg::Resolved(arg.to_string());
        }
        return AnalyzedArg::Resolved(resolved);
    }

    AnalyzedArg::Resolved(arg.to_string())
}

fn resolve_identity(
    arg_type: ArgType,
    arg: &str,
    state: &AnalysisObservation,
) -> Option<(String, Option<String>, Option<String>)> {
    let candidates = match arg_type {
        ArgType::ItemId => dedupe(state.item_ids.clone()),
        ArgType::PoiId => dedupe(state.poi_ids.clone()),
        ArgType::SystemId => dedupe(state.system_ids.clone()),
        ArgType::GoTarget => dedupe(
            state
                .system_ids
                .iter()
                .chain(&state.poi_ids)
                .cloned()
                .collect(),
        ),
        ArgType::MissionId => dedupe(state.mission_ids.clone()),
        ArgType::ShipId => dedupe(state.ship_ids.clone()),
        ArgType::ModuleId => dedupe(state.module_ids.clone()),
        ArgType::RecipeId => dedupe(state.recipe_ids.clone()),
        ArgType::ListingId => dedupe(state.listing_ids.clone()),
        _ => return None,
    };

    if candidates.is_empty() {
        return None;
    }

    let wanted = normalize_token(arg);
    if let Some(exact) = candidates
        .iter()
        .find(|candidate| normalize_token(candidate) == wanted)
    {
        return Some((exact.clone(), None, None));
    }

    let best = candidates
        .iter()
        .map(|candidate| {
            let dist = levenshtein(&wanted, &normalize_token(candidate));
            (candidate, dist)
        })
        .min_by_key(|(_, dist)| *dist);

    if let Some((candidate, _)) = best {
        return Some((
            arg.to_string(),
            Some(candidate.clone()),
            Some(format!(
                "unknown identifier '{arg}', did you mean '{candidate}'?"
            )),
        ));
    }

    Some((
        arg.to_string(),
        None,
        Some(format!("unknown identifier '{arg}'")),
    ))
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut seen = HashMap::<String, ()>::new();
    let mut out = Vec::new();
    for value in values {
        let key = normalize_token(&value);
        if seen.insert(key, ()).is_none() {
            out.push(value);
        }
    }
    out
}

fn normalize_token(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .trim()
        .to_string()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let mut prev = (0..=b.len()).collect::<Vec<_>>();
    let mut curr = vec![0; b.len() + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (curr[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::{ArgSpec, CommandSpec};

    fn catalog(kind: ArgType) -> HashMap<String, CommandSpec> {
        HashMap::from([(
            "test".into(),
            CommandSpec {
                name: "test".into(),
                args: vec![ArgSpec {
                    name: "value".into(),
                    kind,
                    required: true,
                    variadic: false,
                }],
            },
        )])
    }

    #[test]
    fn resolves_identifier_without_runtime_types() {
        let program = AstProgram::parse("test \"Iron Ore\";").expect("parse");
        let observation = AnalysisObservation {
            item_ids: vec!["iron_ore".into()],
            ..AnalysisObservation::default()
        };
        let analyzed = analyze(&program, &catalog(ArgType::ItemId), &observation).expect("analyze");
        let AnalyzedNode::Command(command) = &analyzed.statements[0] else {
            unreachable!("command")
        };
        assert_eq!(command.args, vec![AnalyzedArg::Resolved("iron_ore".into())]);
    }
}
