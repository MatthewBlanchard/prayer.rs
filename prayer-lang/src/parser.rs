//! PrayerLang parser.

use chumsky::prelude::*;

use super::{
    diag, AstNode, AstProgram, BuyNode, CommissionShipNode, CraftClauses, CraftNode, Diagnostic,
    RecycleNode, SayNode, SellNode, Span, TransferEndpoint, TransferItem, TransferNode,
    TransferSubject,
};

type PErr<'a> = extra::Err<Rich<'a, char>>;

/// Parse a DSL script body.
pub(super) fn parse_script(input: &str) -> Result<AstProgram, Vec<Diagnostic>> {
    if let Some(diagnostic) = removed_syntax_diagnostic(input) {
        return Err(vec![diagnostic]);
    }
    if input.trim().is_empty() {
        return Ok(AstProgram {
            statements: Vec::new(),
        });
    }

    script_parser()
        .parse(input)
        .into_result()
        .map(|statements| AstProgram { statements })
        .map_err(|errs| {
            let mut diagnostics = map_errors("DSL104", errs);
            if let Some(diagnostic) = recover_transfer_diagnostic(input) {
                diagnostics.insert(0, diagnostic);
            }
            diagnostics
        })
}

fn removed_syntax_diagnostic(input: &str) -> Option<Diagnostic> {
    for alias in ["nearest_station", "home", "here"] {
        let needle = format!("go {alias}");
        if let Some(start) = input.find(&needle) {
            let alias_start = start + 3;
            return Some(diag(
                "DSL001",
                "injected navigation aliases are no longer supported; clients must submit an explicit system or POI identifier",
                alias_start,
                alias_start + alias.len(),
            ));
        }
    }
    let removed = [
        ("$", "variables and macros"),
        ("if", "control flow"),
        ("until", "control flow"),
        ("skill", "libraries and skill calls"),
        ("override", "override declarations"),
        ("combat", "combat policies"),
        ("targeting", "targeting policies"),
        ("@disable", "policy directives"),
        ("@blacklist", "policy directives"),
        ("@no-overrides", "control-flow directives"),
        ("no_overrides", "control-flow directives"),
    ];
    for (token, feature) in removed {
        let offset = if token == "$" {
            input.find('$')
        } else {
            input.match_indices(token).find_map(|(offset, _)| {
                let before = input[..offset].chars().next_back();
                let after = input[offset + token.len()..].chars().next();
                let boundary =
                    |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
                (boundary(before) && boundary(after)).then_some(offset)
            })
        };
        if let Some(start) = offset {
            return Some(diag("DSL001", &format!("{feature} are no longer supported; PrayerLang accepts only linear executable statements with literal arguments"), start, start + token.len()));
        }
    }
    None
}

pub(super) fn is_valid_arg_token(token: &str) -> bool {
    arg_token_text_parser()
        .then_ignore(end())
        .parse(token)
        .into_result()
        .is_ok()
}

pub(super) fn is_valid_integer_token(token: &str) -> bool {
    let parser = text::int::<&str, extra::Err<Rich<char>>>(10).then_ignore(end());
    let Ok(parsed) = parser.parse(token).into_result() else {
        return false;
    };
    parsed.parse::<i64>().is_ok()
}

fn map_errors(code: &'static str, errors: Vec<Rich<'_, char>>) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|error| {
            let span = error.span();
            diag(code, &error.to_string(), span.start, span.end)
        })
        .collect()
}

fn recover_transfer_diagnostic(input: &str) -> Option<Diagnostic> {
    let start = input.find("transfer")?;
    let before = &input[..start];
    if before
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }

    let after_keyword = &input[start + "transfer".len()..];
    let next = after_keyword.chars().next();
    if !next.map_or(true, |c| c.is_whitespace() || c == ';' || c == '{') {
        return None;
    }
    if after_keyword.trim_start().starts_with('{') {
        return None;
    }

    let semicolon = input[start..].find(';')? + start;
    let statement = &input[start + "transfer".len()..semicolon];
    let parts = lex_transfer_parts(statement)?;
    let span = Span {
        start,
        end: semicolon + 1,
    };
    parse_transfer_parts(parts, span)
        .err()
        .map(|message| diag("DSL104", &message, span.start, span.end))
}

fn lex_transfer_parts(input: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if !current.is_empty() {
                    return None;
                }
                while let Some(quoted) = chars.next() {
                    match quoted {
                        '"' => break,
                        '\\' => current.push(chars.next()?),
                        value => current.push(value),
                    }
                }
                parts.push(std::mem::take(&mut current));
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    Some(parts)
}

fn ws<'a>() -> impl Parser<'a, &'a str, (), PErr<'a>> + Clone {
    one_of(" \t\r\n").repeated().ignored()
}

fn ws1<'a>() -> impl Parser<'a, &'a str, (), PErr<'a>> + Clone {
    one_of(" \t\r\n").repeated().at_least(1).ignored()
}

fn ident<'a>() -> impl Parser<'a, &'a str, String, PErr<'a>> + Clone {
    any()
        .filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .repeated(),
        )
        .to_slice()
        .map(str::to_string)
}

fn keyword<'a>(keyword: &'static str) -> impl Parser<'a, &'a str, (), PErr<'a>> + Clone {
    any()
        .repeated()
        .exactly(keyword.len())
        .to_slice()
        .try_map(move |candidate: &str, span| match candidate == keyword {
            true => Ok(()),
            false => Err(Rich::custom(span, format!("expected '{keyword}'"))),
        })
}

fn arg_token_text_parser<'a>() -> impl Parser<'a, &'a str, &'a str, PErr<'a>> + Clone {
    let plain = any()
        .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .repeated(),
        )
        .ignored();

    let dollar_prefixed = just('$')
        .then(any().filter(|c: &char| c.is_ascii_alphabetic() || *c == '_'))
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .repeated(),
        )
        .ignored();

    choice((dollar_prefixed, plain)).to_slice()
}

fn arg_token<'a>() -> impl Parser<'a, &'a str, String, PErr<'a>> + Clone {
    let quoted = just('"')
        .ignore_then(
            choice((
                just('\\').ignore_then(any()),
                any().filter(|c: &char| *c != '"' && *c != '\\'),
            ))
            .repeated()
            .collect::<String>(),
        )
        .then_ignore(just('"'));
    let plain = arg_token_text_parser().try_map(|token: &str, span| {
        if is_valid_arg_token(token) {
            Ok(token.to_string())
        } else {
            Err(Rich::custom(span, "invalid argument token"))
        }
    });
    choice((quoted, plain))
}

fn statement_parser<'a>() -> impl Parser<'a, &'a str, AstNode, PErr<'a>> + Clone {
    recursive(|_stmt| {
        let transfer_args = ws1()
            .ignore_then(
                arg_token()
                    .then(ws().ignore_then(arg_token()).repeated().collect::<Vec<_>>())
                    .map(|(first, mut rest)| {
                        let mut parts = Vec::with_capacity(rest.len() + 1);
                        parts.push(first);
                        parts.append(&mut rest);
                        parts
                    }),
            )
            .or_not()
            .map(Option::unwrap_or_default);

        let transfer_block_entry = arg_token()
            .then(ws().ignore_then(arg_token()).repeated().collect::<Vec<_>>())
            .then_ignore(ws())
            .then_ignore(just(';'))
            .map(|(first, mut rest)| {
                let mut parts = Vec::with_capacity(rest.len() + 1);
                parts.push(first);
                parts.append(&mut rest);
                parts
            });

        let transfer_block = keyword("transfer")
            .ignore_then(transfer_args.clone())
            .then(
                transfer_block_entry
                    .padded_by(ws())
                    .repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws())),
            )
            .try_map(|(parts, entries), span| {
                parse_transfer_block_entries(
                    parts,
                    entries,
                    Span {
                        start: span.start,
                        end: span.end,
                    },
                )
                .map(AstNode::Transfer)
                .map_err(|message| Rich::custom(span, message))
            });

        let transfer_stmt = keyword("transfer")
            .ignore_then(transfer_args)
            .then_ignore(ws())
            .then_ignore(just(';'))
            .try_map(|parts, span| {
                parse_transfer_parts(
                    parts,
                    Span {
                        start: span.start,
                        end: span.end,
                    },
                )
                .map(AstNode::Transfer)
                .map_err(|message| Rich::custom(span, message))
            });

        let say_stmt = keyword("say")
            .ignore_then(ws1())
            .ignore_then(arg_token())
            .then_ignore(ws1())
            .then_ignore(keyword("to"))
            .then_ignore(ws1())
            .then(arg_token())
            .then(ws1().ignore_then(arg_token()).or_not())
            .then_ignore(ws())
            .then_ignore(just(';'))
            .map_with(|((content, channel), target), e| {
                let span = e.span();
                let mut args = vec![content, channel];
                if let Some(target) = target {
                    args.push(target);
                }
                AstNode::Say(SayNode {
                    content: args.remove(0),
                    channel: args.remove(0),
                    target: args.pop(),
                    span: Span {
                        start: span.start,
                        end: span.end,
                    },
                })
            });

        let command = ident()
            .then(ws().ignore_then(arg_token()).repeated().collect::<Vec<_>>())
            .then_ignore(ws())
            .then_ignore(just(';'))
            .try_map(|(name, args), span| {
                let source_span = Span {
                    start: span.start,
                    end: span.end,
                };
                if name == "transfer" {
                    parse_transfer_parts(args, source_span)
                        .map(AstNode::Transfer)
                        .map_err(|message| Rich::custom(span, message))
                } else if name == "craft" {
                    parse_craft_parts(args, source_span)
                        .map(AstNode::Craft)
                        .map_err(|message| Rich::custom(span, message))
                } else if name == "recycle" {
                    parse_recycle_parts(args, source_span)
                        .map(AstNode::Recycle)
                        .map_err(|message| Rich::custom(span, message))
                } else if name == "buy" {
                    parse_buy_parts(args, source_span)
                        .map(AstNode::Buy)
                        .map_err(|m| Rich::custom(span, m))
                } else if name == "sell" {
                    parse_sell_parts(args, source_span)
                        .map(AstNode::Sell)
                        .map_err(|m| Rich::custom(span, m))
                } else if name == "commission_ship" {
                    parse_commission_ship_parts(args, source_span)
                        .map(AstNode::CommissionShip)
                        .map_err(|m| Rich::custom(span, m))
                } else if name == "modify_order" {
                    let [order_id, at, price] = args.as_slice() else {
                        return Err(Rich::custom(
                            span,
                            "modify_order shape is 'modify_order <order_id> at <price>'",
                        ));
                    };
                    if at != "at" || positive_token(price, "order price").is_err() {
                        return Err(Rich::custom(
                            span,
                            "modify_order shape is 'modify_order <order_id> at <price>'",
                        ));
                    }
                    Ok(AstNode::Command(super::CommandNode {
                        name,
                        args: vec![order_id.clone(), price.clone()],
                        span: source_span,
                    }))
                } else if name == "unload_passenger" && args.get(1).is_some_and(|v| v == "to") {
                    let [passenger, _, target] = args.as_slice() else {
                        return Err(Rich::custom(
                            span,
                            "unload_passenger shape is 'unload_passenger <name|all> [to <target>]'",
                        ));
                    };
                    Ok(AstNode::Command(super::CommandNode {
                        name,
                        args: vec![passenger.clone(), target.clone()],
                        span: source_span,
                    }))
                } else if name == "refuel" && args.iter().any(|v| v == "to") {
                    let at = args.iter().position(|v| v == "to").unwrap();
                    let target = args
                        .get(at + 1)
                        .cloned()
                        .ok_or_else(|| Rich::custom(span, "refuel 'to' clause needs a target"))?;
                    let mut canonical = vec![target];
                    if at == 1 {
                        canonical.push(args[0].clone());
                    } else if at != 0 {
                        return Err(Rich::custom(
                            span,
                            "refuel shape is 'refuel [quantity] [to <target>]'",
                        ));
                    }
                    if args.len() != at + 2 {
                        return Err(Rich::custom(
                            span,
                            "refuel has unknown or duplicate clauses",
                        ));
                    }
                    Ok(AstNode::Command(super::CommandNode {
                        name,
                        args: canonical,
                        span: source_span,
                    }))
                } else if name == "repair" && args.iter().any(|v| v == "with") {
                    let at = args.iter().position(|v| v == "with").unwrap();
                    if at != 1 {
                        return Err(Rich::custom(
                            span,
                            "remote repair shape is 'repair <target> with <item> [quantity]'",
                        ));
                    }
                    let mut canonical = vec![args[0].clone()];
                    canonical.extend(args.iter().skip(at + 1).cloned());
                    if !(2..=3).contains(&canonical.len()) {
                        return Err(Rich::custom(
                            span,
                            "repair 'with' clause needs an item and optional quantity",
                        ));
                    }
                    Ok(AstNode::Command(super::CommandNode {
                        name,
                        args: canonical,
                        span: source_span,
                    }))
                } else {
                    Ok(AstNode::Command(super::CommandNode {
                        name,
                        args,
                        span: source_span,
                    }))
                }
            });

        choice((transfer_block, transfer_stmt, say_stmt, command))
    })
}

fn positive_token(value: &str, what: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| format!("{what} must be a positive integer"))
}

fn parse_buy_parts(parts: Vec<String>, span: Span) -> Result<BuyNode, String> {
    // Explicit compatibility adapter for the pre-clause form:
    // `buy <item> <quantity> [price] [order]`.
    if parts
        .first()
        .is_some_and(|v| v != "order" && v.parse::<u64>().is_err())
    {
        let item_id = parts[0].clone();
        let quantity = positive_token(
            parts.get(1).ok_or("buy statement needs a quantity")?,
            "buy quantity",
        )?;
        let max_price = parts
            .get(2)
            .filter(|v| v.as_str() != "order")
            .map(|v| positive_token(v, "buy price"))
            .transpose()?;
        let place_order = parts.iter().skip(2).any(|v| v == "order");
        if parts.len() > 4
            || parts
                .iter()
                .skip(2)
                .any(|v| v != "order" && v.parse::<u64>().is_err())
        {
            return Err("legacy buy shape is 'buy <item> <quantity> [price] [order]'".into());
        }
        return Ok(BuyNode {
            item_id,
            quantity,
            max_price,
            place_order,
            deliver_to: None,
            span,
        });
    }
    let mut idx = 0;
    let place_order = parts.first().is_some_and(|v| v == "order");
    if place_order {
        idx += 1;
    }
    let quantity = positive_token(
        parts.get(idx).ok_or("buy statement needs a quantity")?,
        "buy quantity",
    )?;
    let item_id = parts
        .get(idx + 1)
        .cloned()
        .ok_or("buy statement needs an item id")?;
    idx += 2;
    let mut max_price = None;
    let mut deliver_to = None;
    while idx < parts.len() {
        match parts[idx].as_str() {
            "at" if max_price.is_none() => {
                max_price = Some(positive_token(
                    parts.get(idx + 1).ok_or("buy 'at' clause needs a price")?,
                    "buy price",
                )?);
                idx += 2;
            }
            "at" => return Err("buy statement has duplicate 'at' clause".into()),
            "to" if deliver_to.is_none() && place_order => {
                let value = parts
                    .get(idx + 1)
                    .ok_or("buy 'to' clause needs cargo or storage")?;
                if !matches!(value.as_str(), "cargo" | "storage") {
                    return Err("buy order destination must be cargo or storage".into());
                }
                deliver_to = Some(value.clone());
                idx += 2;
            }
            "to" if !place_order => return Err("only buy orders accept a 'to' clause".into()),
            "to" => return Err("buy statement has duplicate 'to' clause".into()),
            other => {
                return Err(format!(
                    "unknown buy clause '{other}'; expected 'at' or 'to'"
                ))
            }
        }
    }
    if place_order && max_price.is_none() {
        return Err("buy order requires an 'at <price>' clause".into());
    }
    Ok(BuyNode {
        item_id,
        quantity,
        max_price,
        place_order,
        deliver_to,
        span,
    })
}

fn parse_sell_parts(parts: Vec<String>, span: Span) -> Result<SellNode, String> {
    if parts.is_empty() {
        return Ok(SellNode {
            item_id: None,
            quantity: None,
            min_price: None,
            place_order: false,
            span,
        });
    }
    // Explicit compatibility adapter for `sell <item> [quantity] [price] [order]`.
    if parts
        .first()
        .is_some_and(|v| v != "order" && v.parse::<u64>().is_err())
        && parts.get(1).is_some_and(|v| v.parse::<u64>().is_ok())
    {
        let item_id = Some(parts[0].clone());
        let quantity = Some(positive_token(&parts[1], "sell quantity")?);
        let min_price = parts
            .get(2)
            .filter(|v| v.as_str() != "order")
            .map(|v| positive_token(v, "sell price"))
            .transpose()?;
        let place_order = parts.iter().skip(2).any(|v| v == "order");
        return Ok(SellNode {
            item_id,
            quantity,
            min_price,
            place_order,
            span,
        });
    }
    let mut idx = 0;
    let place_order = parts.first().is_some_and(|v| v == "order");
    if place_order {
        idx += 1;
    }
    let (quantity, item_id) = if parts.get(idx).and_then(|v| v.parse::<u64>().ok()).is_some() {
        let quantity = positive_token(&parts[idx], "sell quantity")?;
        (
            Some(quantity),
            parts
                .get(idx + 1)
                .cloned()
                .ok_or("sell statement needs an item id")?,
        )
    } else {
        (
            None,
            parts
                .get(idx)
                .cloned()
                .ok_or("sell statement needs an item id")?,
        )
    };
    idx += if quantity.is_some() { 2 } else { 1 };
    let mut min_price = None;
    while idx < parts.len() {
        match parts[idx].as_str() {
            "at" if min_price.is_none() => {
                min_price = Some(positive_token(
                    parts.get(idx + 1).ok_or("sell 'at' clause needs a price")?,
                    "sell price",
                )?);
                idx += 2;
            }
            "at" => return Err("sell statement has duplicate 'at' clause".into()),
            other => return Err(format!("unknown sell clause '{other}'; expected 'at'")),
        }
    }
    if place_order && quantity.is_none() {
        return Err("sell order requires a quantity".into());
    }
    if place_order && min_price.is_none() {
        return Err("sell order requires an 'at <price>' clause".into());
    }
    Ok(SellNode {
        item_id: Some(item_id),
        quantity,
        min_price,
        place_order,
        span,
    })
}

fn parse_commission_ship_parts(
    parts: Vec<String>,
    span: Span,
) -> Result<CommissionShipNode, String> {
    let ship_class = parts
        .first()
        .cloned()
        .ok_or("commission_ship needs a ship class")?;
    let provide_materials = match &parts[1..] {
        [] => false,
        [with, materials] if with == "with" && materials == "materials" => true,
        _ => {
            return Err(
                "commission_ship shape is 'commission_ship <class> [with materials]'".into(),
            )
        }
    };
    Ok(CommissionShipNode {
        ship_class,
        provide_materials,
        span,
    })
}

fn parse_recycle_parts(parts: Vec<String>, span: Span) -> Result<RecycleNode, String> {
    let craft = parse_craft_parts(parts, span).map_err(|m| m.replace("craft", "recycle"))?;
    if craft.clauses.preset.is_some() {
        return Err("recycle does not accept a preset clause".into());
    }
    Ok(RecycleNode {
        recipe_id: craft.recipe_id,
        quantity: craft.quantity,
        clauses: craft.clauses,
        span,
    })
}

fn parse_craft_parts(parts: Vec<String>, span: Span) -> Result<CraftNode, String> {
    let Some(recipe_id) = parts.first().cloned() else {
        return Err("craft statement needs a recipe id".to_string());
    };

    let mut idx = 1usize;
    let quantity = match parts.get(idx) {
        Some(token) if is_craft_clause(token) => 1,
        Some(token) => {
            if token == "dry_run" {
                return Err(
                    "use craft_quote for dry-run quotes; craft statements queue jobs".to_string(),
                );
            }
            idx += 1;
            let parsed = token
                .parse::<u64>()
                .map_err(|_| "craft quantity must be a positive integer".to_string())?;
            if parsed == 0 {
                return Err("craft quantity must be a positive integer".to_string());
            }
            parsed
        }
        None => 1,
    };

    let mut clauses = CraftClauses::default();
    while idx < parts.len() {
        let clause = parts[idx].as_str();
        idx += 1;
        match clause {
            "from" => {
                if clauses.source.is_some() {
                    return Err("craft statement has duplicate 'from' clause".to_string());
                }
                clauses.source = Some(parse_craft_store(&parts, &mut idx)?);
            }
            "to" => {
                if clauses.deliver_to.is_some() {
                    return Err("craft statement has duplicate 'to' clause".to_string());
                }
                clauses.deliver_to = Some(parse_craft_store(&parts, &mut idx)?);
            }
            "at" => {
                if clauses.facility_id.is_some() {
                    return Err("craft statement has duplicate 'at' clause".to_string());
                }
                clauses.facility_id = Some(parse_craft_clause_value(
                    &parts,
                    &mut idx,
                    "craft 'at' clause needs a facility id",
                )?);
            }
            "preset" => {
                if clauses.preset.is_some() {
                    return Err("craft statement has duplicate 'preset' clause".to_string());
                }
                clauses.preset = Some(parse_craft_clause_value(
                    &parts,
                    &mut idx,
                    "craft 'preset' clause needs a preset name",
                )?);
            }
            "dry_run" => {
                return Err(
                    "use craft_quote for dry-run quotes; craft statements queue jobs".to_string(),
                )
            }
            _ => {
                return Err(
                    "craft statement shape is 'craft <recipe_id> [quantity] [from cargo|storage|faction] [to cargo|storage|faction] [at facility_id] [preset name]'"
                        .to_string(),
                )
            }
        }
    }

    Ok(CraftNode {
        recipe_id,
        quantity,
        clauses,
        span,
    })
}

fn parse_craft_store(parts: &[String], idx: &mut usize) -> Result<String, String> {
    let Some(store) = parts.get(*idx).map(String::as_str) else {
        return Err("craft source/destination must be cargo, storage, or faction".to_string());
    };
    *idx += 1;
    match store {
        "cargo" | "storage" | "faction" => Ok(store.to_string()),
        _ => Err("craft source/destination must be cargo, storage, or faction".to_string()),
    }
}

fn parse_craft_clause_value(
    parts: &[String],
    idx: &mut usize,
    missing_message: &str,
) -> Result<String, String> {
    let Some(value) = parts.get(*idx).filter(|token| !is_craft_clause(token)) else {
        return Err(missing_message.to_string());
    };
    *idx += 1;
    Ok(value.clone())
}

fn is_craft_clause(token: &str) -> bool {
    matches!(token, "from" | "to" | "at" | "preset")
}

fn parse_transfer_block_entries(
    parts: Vec<String>,
    entries: Vec<Vec<String>>,
    span: Span,
) -> Result<TransferNode, String> {
    let (from, to) = parse_transfer_clauses(&parts)?;
    let mut items = Vec::new();

    for entry in entries {
        let Some(head) = entry.first().map(String::as_str) else {
            continue;
        };
        match head {
            "from" | "to" => {
                return Err("transfer block endpoints belong before the item block".to_string())
            }
            _ => {
                let [id, qty] = entry.as_slice() else {
                    return Err("transfer block entry must be '<item> <qty>'".to_string());
                };
                let qty = qty
                    .parse::<i64>()
                    .map_err(|_| "transfer block item quantity must be an integer".to_string())?;
                items.push(TransferItem {
                    id: id.clone(),
                    qty,
                });
            }
        }
    }

    if items.is_empty() {
        return Err("transfer block requires at least one item quantity pair".to_string());
    }

    Ok(TransferNode {
        subject: TransferSubject::AllCargo,
        items,
        from,
        to,
        span,
    })
}

fn parse_transfer_parts(parts: Vec<String>, span: Span) -> Result<TransferNode, String> {
    let mut idx = 0usize;
    let mut subject_parts = Vec::new();
    while idx < parts.len() && !is_transfer_clause(&parts[idx]) {
        subject_parts.push(parts[idx].clone());
        idx += 1;
    }

    let subject = parse_transfer_subject(&subject_parts)?;
    let (from, to) = parse_transfer_clauses(&parts[idx..])?;

    Ok(TransferNode {
        subject,
        items: Vec::new(),
        from,
        to,
        span,
    })
}

fn parse_transfer_clauses(
    parts: &[String],
) -> Result<(Option<TransferEndpoint>, Option<TransferEndpoint>), String> {
    let mut from = None;
    let mut to = None;
    let mut idx = 0usize;
    while idx < parts.len() {
        let clause = parts[idx].as_str();
        idx += 1;
        let endpoint = parse_transfer_endpoint(parts, &mut idx)?;
        match clause {
            "from" if from.is_none() => from = Some(endpoint),
            "to" if to.is_none() => to = Some(endpoint),
            "from" => return Err("duplicate transfer 'from' clause".to_string()),
            "to" => return Err("duplicate transfer 'to' clause".to_string()),
            _ => return Err("expected transfer clause 'to' or 'from'".to_string()),
        }
    }

    Ok((from, to))
}

fn parse_transfer_subject(parts: &[String]) -> Result<TransferSubject, String> {
    match parts {
        [] => Ok(TransferSubject::AllCargo),
        [kind, qty] if kind == "credits" => {
            let qty = qty
                .parse::<i64>()
                .map_err(|_| "transfer credits quantity must be an integer".to_string())?;
            Ok(TransferSubject::Credits(qty))
        }
        [kind] if kind == "credits" => Err("transfer credits requires a quantity".to_string()),
        [kind, id] if kind == "ship" => Ok(TransferSubject::Ship { id: id.clone() }),
        [kind] if kind == "ship" => Err("transfer ship requires a ship id".to_string()),
        [kind, id] if kind == "module" => Ok(TransferSubject::Module { id: id.clone() }),
        [kind] if kind == "module" => Err("transfer module requires a module id".to_string()),
        [id] => Ok(TransferSubject::Item {
            id: id.clone(),
            qty: None,
        }),
        [id, qty] => {
            let qty = qty
                .parse::<i64>()
                .map_err(|_| "transfer item quantity must be an integer".to_string())?;
            Ok(TransferSubject::Item {
                id: id.clone(),
                qty: Some(qty),
            })
        }
        _ => Err(
            "transfer subject must be '<item> [qty]', 'ship <id>', or 'credits <qty>'".to_string(),
        ),
    }
}

fn parse_transfer_endpoint(parts: &[String], idx: &mut usize) -> Result<TransferEndpoint, String> {
    let Some(head) = parts.get(*idx).map(String::as_str) else {
        return Err("transfer endpoint is missing".to_string());
    };
    *idx += 1;
    match head {
        "cargo" => Ok(TransferEndpoint::Cargo),
        "storage" => Ok(TransferEndpoint::Storage),
        "faction" => {
            if let Some(tag) = parts.get(*idx).filter(|next| !is_transfer_clause(next)) {
                *idx += 1;
                Ok(TransferEndpoint::FactionTag(tag.clone()))
            } else {
                Ok(TransferEndpoint::Faction)
            }
        }
        "player" => {
            let Some(name) = parts.get(*idx).filter(|next| !is_transfer_clause(next)) else {
                return Err("transfer player endpoint requires a player name".to_string());
            };
            *idx += 1;
            Ok(TransferEndpoint::Player(name.clone()))
        }
        "space" => {
            if let Some(id) = parts.get(*idx).filter(|next| !is_transfer_clause(next)) {
                *idx += 1;
                Ok(TransferEndpoint::Space(Some(id.clone())))
            } else {
                Ok(TransferEndpoint::Space(None))
            }
        }
        "commission" => {
            let id = parts
                .get(*idx)
                .filter(|next| !is_transfer_clause(next))
                .ok_or("transfer commission endpoint requires a commission id")?
                .clone();
            *idx += 1;
            Ok(TransferEndpoint::Commission(id))
        }
        _ => Err(unknown_transfer_endpoint_message(head)),
    }
}

fn is_transfer_clause(token: &str) -> bool {
    matches!(token, "to" | "from")
}

fn unknown_transfer_endpoint_message(endpoint: &str) -> String {
    let hint = match endpoint {
        "personal" | "self" => Some("use 'storage' for personal station storage"),
        "hold" | "ship" => Some("use 'cargo' for the ship cargo hold"),
        "treasury" => Some("use 'faction' for faction storage or treasury transfers"),
        _ => None,
    };
    match hint {
        Some(hint) => format!("unknown transfer endpoint '{endpoint}'; {hint}"),
        None => format!(
            "unknown transfer endpoint '{endpoint}'; expected cargo, storage, faction, player <name>, or space [id]"
        ),
    }
}

fn script_parser<'a>() -> impl Parser<'a, &'a str, Vec<AstNode>, PErr<'a>> + Clone {
    statement_parser()
        .padded_by(ws())
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(ws())
        .then_ignore(end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linear_script() {
        let program = parse_script("go alpha_station; dock; buy iron_ore 10 5;").unwrap();
        assert_eq!(program.statements.len(), 3);
    }

    #[test]
    fn rejects_every_removed_construct_with_focused_diagnostic() {
        for source in [
            "go $nearest_station;",
            "go nearest_station;",
            "go home;",
            "go here;",
            "if FUEL() > 0 { dock; }",
            "until DOCKED() { dock; }",
            "skill route() { dock; }",
            "override fuel when FUEL() < 5 { dock; }",
            "combat aggressive() { flee; }",
            "targeting nearest() { attack pirate; }",
            "@disable mine",
            "@blacklist mine iron_ore",
            "@no-overrides go alpha;",
        ] {
            let diagnostics = parse_script(source).expect_err(source);
            assert_eq!(diagnostics[0].code, "DSL001", "{source}");
            assert!(
                diagnostics[0].message.contains("no longer supported"),
                "{source}"
            );
        }
    }

    #[test]
    fn parses_structured_linear_statements() {
        let source =
            r#"transfer iron_ore 10 from cargo to storage; craft steel 2; say system "hello";"#;
        assert_eq!(parse_script(source).unwrap().statements.len(), 3);
    }
}
