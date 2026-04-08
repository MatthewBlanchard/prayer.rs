use super::{
    AstNode, AstProgram, ComparisonOp, ConditionExpr, MetricCall, NumericOperand, OverrideAstNode,
    SkillAstNode, SkillLibraryAst,
};

pub(super) fn normalize(program: &AstProgram) -> String {
    let mut out = String::new();
    render_nodes(&program.statements, 0, &mut out);
    out
}

pub(super) fn normalize_library(library: &SkillLibraryAst) -> String {
    let mut out = String::new();
    for skill in &library.skills {
        out.push_str(&render_skill(skill));
        out.push('\n');
    }
    for override_rule in &library.overrides {
        out.push_str(&render_override(override_rule));
        out.push('\n');
    }
    let mut disabled = library
        .disabled_commands
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    disabled.sort();
    for command in disabled {
        out.push_str(&format!("@disable {command}\n"));
    }
    out.trim_end().to_string()
}

fn render_nodes(nodes: &[AstNode], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    for node in nodes {
        match node {
            AstNode::Command(cmd) => {
                out.push_str(&pad);
                out.push_str(&cmd.name.to_lowercase());
                if !cmd.args.is_empty() {
                    out.push(' ');
                    out.push_str(&cmd.args.join(" "));
                }
                out.push_str(";\n");
            }
            AstNode::If(block) => {
                out.push_str(&pad);
                out.push_str("if ");
                out.push_str(&render_condition(&block.condition));
                out.push_str(" {\n");
                render_nodes(&block.body, indent + 1, out);
                out.push_str(&pad);
                out.push_str("}\n");
            }
            AstNode::Until(block) => {
                out.push_str(&pad);
                out.push_str("until ");
                out.push_str(&render_condition(&block.condition));
                out.push_str(" {\n");
                render_nodes(&block.body, indent + 1, out);
                out.push_str(&pad);
                out.push_str("}\n");
            }
        }
    }
}

fn render_skill(skill: &SkillAstNode) -> String {
    let mut out = String::new();
    out.push_str("skill ");
    out.push_str(&skill.name);
    if !skill.params.is_empty() {
        out.push('(');
        for (index, param) in skill.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&param.name);
            out.push_str(": ");
            out.push_str(param.kind.as_str());
        }
        out.push(')');
    }
    out.push_str(" {\n");
    render_nodes(&skill.body, 1, &mut out);
    out.push('}');
    out
}

fn render_override(override_rule: &OverrideAstNode) -> String {
    let mut out = String::new();
    out.push_str("override ");
    out.push_str(&override_rule.name);
    out.push_str(" when ");
    out.push_str(&render_condition(&override_rule.condition));
    out.push_str(" {\n");
    render_nodes(&override_rule.body, 1, &mut out);
    out.push('}');
    out
}

fn render_condition(condition: &ConditionExpr) -> String {
    match condition {
        ConditionExpr::MetricCall(m) => render_metric_call(m),
        ConditionExpr::Comparison { left, op, right } => {
            format!(
                "{} {} {}",
                render_operand(left),
                render_op(*op),
                render_operand(right)
            )
        }
    }
}

fn render_operand(operand: &NumericOperand) -> String {
    match operand {
        NumericOperand::Integer(i) => i.to_string(),
        NumericOperand::MetricCall(m) => render_metric_call(m),
        NumericOperand::ArgRef(r) => format!("${r}"),
    }
}

fn render_metric_call(metric: &MetricCall) -> String {
    if metric.args.is_empty() {
        format!("{}()", metric.name.to_uppercase())
    } else {
        format!("{}({})", metric.name.to_uppercase(), metric.args.join(", "))
    }
}

fn render_op(op: ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Gt => ">",
        ComparisonOp::Ge => ">=",
        ComparisonOp::Lt => "<",
        ComparisonOp::Le => "<=",
        ComparisonOp::Eq => "==",
        ComparisonOp::Ne => "!=",
    }
}

#[cfg(test)]
mod tests {
    use crate::dsl::{AstProgram, SkillLibraryAst};

    fn parse_and_normalize(src: &str) -> String {
        AstProgram::parse(src).expect("parse").normalize()
    }

    fn parse_library_and_normalize(src: &str) -> String {
        SkillLibraryAst::parse(src)
            .expect("parse library")
            .normalize()
    }

    #[test]
    fn normalize_simple_command() {
        let out = parse_and_normalize("halt;");
        assert_eq!(out.trim(), "halt;");
    }

    #[test]
    fn normalize_command_with_args() {
        let out = parse_and_normalize("buy iron_ore 10;");
        assert_eq!(out.trim(), "buy iron_ore 10;");
    }

    #[test]
    fn normalize_if_block_true_branch() {
        let out = parse_and_normalize("if FUEL() >= 50 { halt; }");
        assert!(out.contains("if FUEL() >= 50 {"));
        assert!(out.contains("  halt;"));
        assert!(out.contains("}"));
    }

    #[test]
    fn normalize_until_block() {
        let out = parse_and_normalize("until FUEL() >= 50 { go station; }");
        assert!(out.contains("until FUEL() >= 50 {"));
        assert!(out.contains("  go station;"));
    }

    #[test]
    fn normalize_nested_blocks() {
        let src = "until FUEL() >= 50 { if CREDITS() > 100 { halt; } }";
        let out = parse_and_normalize(src);
        assert!(out.contains("until FUEL() >= 50 {"));
        assert!(out.contains("  if CREDITS() > 100 {"));
        assert!(out.contains("    halt;"));
    }

    #[test]
    fn normalize_all_comparison_operators() {
        for (op, expected) in [
            ("FUEL() > 10", "FUEL() > 10"),
            ("FUEL() >= 10", "FUEL() >= 10"),
            ("FUEL() < 10", "FUEL() < 10"),
            ("FUEL() <= 10", "FUEL() <= 10"),
            ("FUEL() == 10", "FUEL() == 10"),
            ("FUEL() != 10", "FUEL() != 10"),
        ] {
            let src = format!("if {op} {{ halt; }}");
            let out = parse_and_normalize(&src);
            assert!(out.contains(expected), "missing '{expected}' in: {out}");
        }
    }

    #[test]
    fn normalize_metric_call_with_arg() {
        let out = parse_and_normalize("if CARGO(iron_ore) >= 5 { halt; }");
        assert!(out.contains("CARGO(iron_ore) >= 5"));
    }

    #[test]
    fn normalize_metric_call_without_arg_uses_uppercase() {
        let out = parse_and_normalize("if FUEL() >= 10 { halt; }");
        assert!(out.contains("FUEL()"));
    }

    #[test]
    fn normalize_library_skill_with_params() {
        let src = "skill travel(dest: go_target) { go $dest; }";
        let out = parse_library_and_normalize(src);
        assert!(out.contains("skill travel(dest: go_target) {"));
        assert!(out.contains("  go $dest;"));
        assert!(out.contains("}"));
    }

    #[test]
    fn normalize_library_override() {
        let src = "override safety when FUEL() <= 5 { halt; }";
        let out = parse_library_and_normalize(src);
        assert!(out.contains("override safety when FUEL() <= 5 {"));
        assert!(out.contains("  halt;"));
    }

    #[test]
    fn normalize_library_disabled_commands_sorted() {
        let src = "@disable mine\n@disable survey\n@disable explore";
        let out = parse_library_and_normalize(src);
        let explore_pos = out.find("@disable explore").expect("explore");
        let mine_pos = out.find("@disable mine").expect("mine");
        let survey_pos = out.find("@disable survey").expect("survey");
        assert!(explore_pos < mine_pos);
        assert!(mine_pos < survey_pos);
    }

    #[test]
    fn normalize_library_empty_is_empty_string() {
        let out = parse_library_and_normalize("");
        assert_eq!(out, "");
    }
}
