//! Canonical PrayerLang formatter.

use super::{AstNode, AstProgram, CraftClauses, TransferEndpoint, TransferSubject};

pub(super) fn normalize(program: &AstProgram) -> String {
    let mut out = String::new();
    render_nodes(&program.statements, 0, &mut out);
    out.trim_end().to_string()
}

fn render_nodes(nodes: &[AstNode], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    for node in nodes {
        match node {
            AstNode::Command(cmd) => {
                out.push_str(&pad);
                let command_name = cmd.name.to_lowercase();
                if command_name == "say" && cmd.args.len() >= 2 {
                    out.push_str("say ");
                    out.push_str(&render_arg(&cmd.args[0]));
                    out.push_str(" to ");
                    out.push_str(&render_arg(&cmd.args[1]));
                    if let Some(target) = cmd.args.get(2) {
                        out.push(' ');
                        out.push_str(&render_arg(target));
                    }
                    out.push_str(";\n");
                    continue;
                }
                out.push_str(&command_name);
                if !cmd.args.is_empty() {
                    out.push(' ');
                    out.push_str(
                        &cmd.args
                            .iter()
                            .map(|arg| render_arg(arg))
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
                out.push_str(";\n");
            }
            AstNode::Transfer(transfer) => {
                out.push_str(&pad);
                if !transfer.items.is_empty() {
                    out.push_str("transfer");
                    if let Some(from) = &transfer.from {
                        out.push_str(" from ");
                        out.push_str(&render_transfer_endpoint(from));
                    }
                    if let Some(to) = &transfer.to {
                        out.push_str(" to ");
                        out.push_str(&render_transfer_endpoint(to));
                    }
                    out.push_str(" {\n");
                    for item in &transfer.items {
                        out.push_str(&pad);
                        out.push_str("  ");
                        out.push_str(&render_arg(&item.id));
                        out.push(' ');
                        out.push_str(&item.qty.to_string());
                        out.push_str(";\n");
                    }
                    out.push_str(&pad);
                    out.push_str("}\n");
                    continue;
                }

                out.push_str("transfer");
                let subject = render_transfer_subject(&transfer.subject);
                if !subject.is_empty() {
                    out.push(' ');
                    out.push_str(&subject);
                }
                if let Some(from) = &transfer.from {
                    out.push_str(" from ");
                    out.push_str(&render_transfer_endpoint(from));
                }
                if let Some(to) = &transfer.to {
                    out.push_str(" to ");
                    out.push_str(&render_transfer_endpoint(to));
                }
                out.push_str(";\n");
            }
            AstNode::Craft(craft) => {
                out.push_str(&pad);
                out.push_str("craft ");
                out.push_str(&render_arg(&craft.recipe_id));
                if craft.quantity != 1 {
                    out.push(' ');
                    out.push_str(&craft.quantity.to_string());
                }
                render_craft_clauses(&craft.clauses, out);
                out.push_str(";\n");
            }
            AstNode::Recycle(recycle) => {
                out.push_str(&pad);
                out.push_str("recycle ");
                out.push_str(&render_arg(&recycle.recipe_id));
                if recycle.quantity != 1 {
                    out.push(' ');
                    out.push_str(&recycle.quantity.to_string());
                }
                render_craft_clauses(&recycle.clauses, out);
                out.push_str(";\n");
            }
            AstNode::Say(say) => {
                out.push_str(&pad);
                out.push_str("say ");
                out.push_str(&render_arg(&say.content));
                out.push_str(" to ");
                out.push_str(&render_arg(&say.channel));
                if let Some(target) = &say.target {
                    out.push(' ');
                    out.push_str(&render_arg(target));
                }
                out.push_str(";\n");
            }
            AstNode::Buy(buy) => {
                out.push_str(&pad);
                out.push_str("buy ");
                if buy.place_order {
                    out.push_str("order ");
                }
                out.push_str(&buy.quantity.to_string());
                out.push(' ');
                out.push_str(&render_arg(&buy.item_id));
                if let Some(price) = buy.max_price {
                    out.push_str(" at ");
                    out.push_str(&price.to_string());
                }
                if let Some(destination) = &buy.deliver_to {
                    out.push_str(" to ");
                    out.push_str(destination);
                }
                out.push_str(";\n");
            }
            AstNode::Sell(sell) => {
                out.push_str(&pad);
                out.push_str("sell");
                if sell.place_order {
                    out.push_str(" order");
                }
                if sell.quantity.is_some() || sell.item_id.is_some() {
                    out.push(' ');
                }
                if let Some(quantity) = sell.quantity {
                    out.push_str(&quantity.to_string());
                    out.push(' ');
                }
                if let Some(item) = &sell.item_id {
                    out.push_str(&render_arg(item));
                }
                if let Some(price) = sell.min_price {
                    out.push_str(" at ");
                    out.push_str(&price.to_string());
                }
                out.push_str(";\n");
            }
            AstNode::CommissionShip(commission) => {
                out.push_str(&pad);
                out.push_str("commission_ship ");
                out.push_str(&render_arg(&commission.ship_class));
                if commission.provide_materials {
                    out.push_str(" with materials");
                }
                out.push_str(";\n");
            }
        }
    }
}

fn render_craft_clauses(clauses: &CraftClauses, out: &mut String) {
    if let Some(source) = &clauses.source {
        out.push_str(" from ");
        out.push_str(source);
    }
    if let Some(deliver_to) = &clauses.deliver_to {
        out.push_str(" to ");
        out.push_str(deliver_to);
    }
    if let Some(facility_id) = &clauses.facility_id {
        out.push_str(" at ");
        out.push_str(&render_arg(facility_id));
    }
    if let Some(preset) = &clauses.preset {
        out.push_str(" preset ");
        out.push_str(&render_arg(preset));
    }
}

fn render_arg(arg: &str) -> String {
    if arg.is_empty()
        || arg
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\\' | ';' | '{' | '}'))
    {
        format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

fn render_transfer_subject(subject: &TransferSubject) -> String {
    match subject {
        TransferSubject::AllCargo => String::new(),
        TransferSubject::Credits(qty) => format!("credits {qty}"),
        TransferSubject::Ship { id } => format!("ship {}", render_arg(id)),
        TransferSubject::Module { id } => format!("module {}", render_arg(id)),
        TransferSubject::Item { id, qty } => match qty {
            Some(qty) => format!("{} {qty}", render_arg(id)),
            None => render_arg(id),
        },
    }
}

fn render_transfer_endpoint(endpoint: &TransferEndpoint) -> String {
    match endpoint {
        TransferEndpoint::Cargo => "cargo".to_string(),
        TransferEndpoint::Storage => "storage".to_string(),
        TransferEndpoint::Faction => "faction".to_string(),
        TransferEndpoint::FactionTag(tag) => format!("faction {}", render_arg(tag)),
        TransferEndpoint::Player(name) => format!("player {}", render_arg(name)),
        TransferEndpoint::Space(Some(id)) => format!("space {}", render_arg(id)),
        TransferEndpoint::Space(None) => "space".to_string(),
        TransferEndpoint::Commission(id) => format!("commission {}", render_arg(id)),
    }
}

#[cfg(test)]
mod tests {
    use crate::AstProgram;

    #[test]
    fn normalizes_linear_statements_only() {
        let normalized = AstProgram::parse("GO alpha;DOCK;").unwrap().normalize();
        assert_eq!(normalized, "go alpha;\ndock;");
    }
}
