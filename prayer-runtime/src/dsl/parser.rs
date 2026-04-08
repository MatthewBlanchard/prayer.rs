use std::collections::HashSet;

use chumsky::prelude::*;

use super::{
    diag, ArgType, AstNode, AstProgram, ComparisonOp, ConditionExpr, Diagnostic, MetricCall,
    NumericOperand, OverrideAstNode, SkillAstNode, SkillLibraryAst, SkillParamDef, Span,
};

type PErr<'a> = extra::Err<Rich<'a, char>>;

/// Parse a DSL script body.
pub(super) fn parse_script(input: &str) -> Result<AstProgram, Vec<Diagnostic>> {
    if input.trim().is_empty() {
        return Ok(AstProgram {
            statements: Vec::new(),
        });
    }

    script_parser()
        .parse(input)
        .into_result()
        .map(|statements| AstProgram { statements })
        .map_err(|errs| map_errors("DSL104", errs))
}

/// Parse a skill library (`skill`, `override`, and `@disable`).
pub(super) fn parse_library(input: &str) -> Result<SkillLibraryAst, Vec<Diagnostic>> {
    let nodes = library_parser()
        .parse(input)
        .into_result()
        .map_err(|errs| map_errors("DSL052", errs))?;

    let mut skills = Vec::new();
    let mut overrides = Vec::new();
    let mut disabled_commands = HashSet::new();

    for node in nodes {
        match node {
            LibraryNode::Skill(skill) => skills.push(skill),
            LibraryNode::Override(ovr) => overrides.push(ovr),
            LibraryNode::Disable(cmd) => {
                disabled_commands.insert(cmd.to_lowercase());
            }
        }
    }

    Ok(SkillLibraryAst {
        skills,
        overrides,
        disabled_commands,
    })
}

/// Parse an isolated condition expression.
pub(super) fn parse_condition(input: &str) -> Result<ConditionExpr, Vec<Diagnostic>> {
    condition_parser()
        .then_ignore(ws())
        .then_ignore(end())
        .parse(input.trim())
        .into_result()
        .map_err(|errs| map_errors("DSL108", errs))
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

#[derive(Debug, Clone)]
enum LibraryNode {
    Skill(SkillAstNode),
    Override(OverrideAstNode),
    Disable(String),
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

fn ws<'a>() -> impl Parser<'a, &'a str, (), PErr<'a>> + Clone {
    one_of(" \t\r\n").repeated().ignored()
}

fn ws1<'a>() -> impl Parser<'a, &'a str, (), PErr<'a>> + Clone {
    one_of(" \t\r\n").repeated().at_least(1).ignored()
}

fn ws_and_comments<'a>() -> impl Parser<'a, &'a str, (), PErr<'a>> + Clone {
    let comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .then(just('\n').or_not())
        .ignored();

    choice((one_of(" \t\r\n").ignored(), comment))
        .repeated()
        .ignored()
}

fn ws_or_comments1<'a>() -> impl Parser<'a, &'a str, (), PErr<'a>> + Clone {
    let comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .then(just('\n').or_not())
        .ignored();

    choice((one_of(" \t\r\n").ignored(), comment))
        .repeated()
        .at_least(1)
        .ignored()
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
    arg_token_text_parser().try_map(|token: &str, span| {
        if is_valid_arg_token(token) {
            Ok(token.to_string())
        } else {
            Err(Rich::custom(span, "invalid argument token"))
        }
    })
}

fn metric_call_parser<'a>() -> impl Parser<'a, &'a str, MetricCall, PErr<'a>> + Clone {
    let args = arg_token()
        .padded_by(ws())
        .separated_by(just(',').padded_by(ws()))
        .allow_trailing()
        .collect::<Vec<_>>()
        .or_not()
        .map(|args| args.unwrap_or_default());

    ident()
        .then_ignore(ws())
        .then_ignore(just('('))
        .then(args)
        .then_ignore(just(')'))
        .map(|(name, args)| MetricCall {
            name: name.to_uppercase(),
            args,
        })
}

fn int_operand<'a>() -> impl Parser<'a, &'a str, NumericOperand, PErr<'a>> + Clone {
    text::int(10).try_map(|token: &str, span| {
        token
            .parse::<i64>()
            .map(NumericOperand::Integer)
            .map_err(|_| Rich::custom(span, "invalid integer"))
    })
}

fn arg_ref_operand<'a>() -> impl Parser<'a, &'a str, NumericOperand, PErr<'a>> + Clone {
    just('$')
        .ignore_then(ident())
        .map(|name| NumericOperand::ArgRef(format!("${name}")))
}

fn numeric_operand<'a>() -> impl Parser<'a, &'a str, NumericOperand, PErr<'a>> + Clone {
    choice((
        int_operand(),
        arg_ref_operand(),
        metric_call_parser().map(NumericOperand::MetricCall),
    ))
}

fn comparison_op<'a>() -> impl Parser<'a, &'a str, ComparisonOp, PErr<'a>> + Clone {
    choice((
        just(">=").to(ComparisonOp::Ge),
        just("<=").to(ComparisonOp::Le),
        just("==").to(ComparisonOp::Eq),
        just("!=").to(ComparisonOp::Ne),
        just('>').to(ComparisonOp::Gt),
        just('<').to(ComparisonOp::Lt),
    ))
}

fn condition_parser<'a>() -> impl Parser<'a, &'a str, ConditionExpr, PErr<'a>> + Clone {
    numeric_operand()
        .then(
            ws().ignore_then(comparison_op())
                .then(ws().ignore_then(numeric_operand()))
                .or_not(),
        )
        .try_map(|(left, maybe_cmp), span| {
            if let Some((op, right)) = maybe_cmp {
                return Ok(ConditionExpr::Comparison { left, op, right });
            }

            match left {
                NumericOperand::MetricCall(call) => Ok(ConditionExpr::MetricCall(call)),
                _ => Err(Rich::custom(
                    span,
                    "condition must be a metric call or numeric comparison",
                )),
            }
        })
}

fn statement_parser<'a>() -> impl Parser<'a, &'a str, AstNode, PErr<'a>> + Clone {
    recursive(|stmt| {
        let block = stmt
            .clone()
            .padded_by(ws())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just('{').padded_by(ws()), just('}').padded_by(ws()));

        let if_stmt = keyword("if")
            .then_ignore(ws1())
            .ignore_then(condition_parser().padded_by(ws()))
            .then(block.clone())
            .map_with(|(condition, body), e| {
                let span = e.span();
                AstNode::If(super::ConditionalNode {
                    condition,
                    body,
                    span: Span {
                        start: span.start,
                        end: span.end,
                    },
                })
            });

        let until_stmt = keyword("until")
            .then_ignore(ws1())
            .ignore_then(condition_parser().padded_by(ws()))
            .then(block)
            .map_with(|(condition, body), e| {
                let span = e.span();
                AstNode::Until(super::ConditionalNode {
                    condition,
                    body,
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
            .map_with(|(name, args), e| {
                let span = e.span();
                AstNode::Command(super::CommandNode {
                    name,
                    args,
                    span: Span {
                        start: span.start,
                        end: span.end,
                    },
                })
            });

        choice((if_stmt, until_stmt, command))
    })
}

fn script_parser<'a>() -> impl Parser<'a, &'a str, Vec<AstNode>, PErr<'a>> + Clone {
    statement_parser()
        .padded_by(ws())
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(ws())
        .then_ignore(end())
}

fn skill_decl_parser<'a>() -> impl Parser<'a, &'a str, SkillAstNode, PErr<'a>> + Clone {
    let param = ident()
        .then_ignore(ws_and_comments())
        .then_ignore(just(':'))
        .then_ignore(ws_and_comments())
        .then(ident())
        .try_map(|(name, ty), span| {
            ty.parse::<ArgType>()
                .map(|kind| SkillParamDef { name, kind })
                .map_err(|_| Rich::custom(span, "unknown parameter type"))
        });

    let params = param
        .separated_by(just(',').padded_by(ws_and_comments()))
        .allow_trailing()
        .collect::<Vec<_>>()
        .or_not()
        .map(|params| params.unwrap_or_default());

    let body = statement_parser()
        .padded_by(ws())
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(
            just('{').padded_by(ws_and_comments()),
            just('}').padded_by(ws_and_comments()),
        );

    keyword("skill")
        .then_ignore(ws_or_comments1())
        .ignore_then(ident())
        .then_ignore(ws_and_comments())
        .then_ignore(just('('))
        .then(params)
        .then_ignore(just(')'))
        .then_ignore(ws_and_comments())
        .then(body)
        .map(|((name, params), body)| SkillAstNode { name, params, body })
}

fn override_decl_parser<'a>() -> impl Parser<'a, &'a str, OverrideAstNode, PErr<'a>> + Clone {
    let body = statement_parser()
        .padded_by(ws())
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(
            just('{').padded_by(ws_and_comments()),
            just('}').padded_by(ws_and_comments()),
        );

    keyword("override")
        .then_ignore(ws_or_comments1())
        .ignore_then(ident())
        .then_ignore(ws_or_comments1())
        .then_ignore(keyword("when"))
        .then_ignore(ws_or_comments1())
        .then(condition_parser().padded_by(ws_and_comments()))
        .then(body)
        .map(|((name, condition), body)| OverrideAstNode {
            name,
            condition,
            body,
        })
}

fn disable_directive_parser<'a>() -> impl Parser<'a, &'a str, String, PErr<'a>> + Clone {
    keyword("@disable")
        .then_ignore(ws_or_comments1())
        .ignore_then(arg_token())
        .then_ignore(ws_and_comments())
        .then_ignore(just(';').or_not())
}

fn library_parser<'a>() -> impl Parser<'a, &'a str, Vec<LibraryNode>, PErr<'a>> + Clone {
    let entry = choice((
        skill_decl_parser().map(LibraryNode::Skill),
        override_decl_parser().map(LibraryNode::Override),
        disable_directive_parser().map(LibraryNode::Disable),
    ));

    entry
        .padded_by(ws_and_comments())
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(ws_and_comments())
        .then_ignore(end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_node(node: &AstNode) -> &super::super::CommandNode {
        match node {
            AstNode::Command(cmd) => Some(cmd),
            _ => None,
        }
        .expect("expected command")
    }

    fn if_node(node: &AstNode) -> &super::super::ConditionalNode {
        match node {
            AstNode::If(block) => Some(block),
            _ => None,
        }
        .expect("expected if")
    }

    fn until_node(node: &AstNode) -> &super::super::ConditionalNode {
        match node {
            AstNode::Until(block) => Some(block),
            _ => None,
        }
        .expect("expected until")
    }

    #[test]
    fn parses_control_flow() {
        let src = r#"
        if MISSION_COMPLETE(m1) { halt; }
        until FUEL() >= 50 { go alpha-1; }
        "#;
        let p = parse_script(src).expect("parse");
        assert_eq!(p.statements.len(), 2);
    }

    #[test]
    fn rejects_mixed_case_control_flow_keywords() {
        let src = r#"
        IF MISSION_COMPLETE(m1) { halt; }
        UnTiL FUEL() >= 50 { go alpha-1; }
        "#;
        let err = parse_script(src).expect_err("expected parse error");
        assert_eq!(err[0].code, "DSL104");
    }

    #[test]
    fn parses_library() {
        let src = r#"
        @disable mine;
        @disable salvage
        skill travel(system: system_id) {
          go $system;
        }
        override safety when FUEL() <= 5 {
          halt;
        }
        "#;
        let lib = parse_library(src).expect("library");
        assert_eq!(lib.skills.len(), 1);
        assert_eq!(lib.overrides.len(), 1);
        assert!(lib.disabled_commands.contains("mine"));
        assert!(lib.disabled_commands.contains("salvage"));
    }

    #[test]
    fn rejects_mixed_case_library_keywords_and_disable() {
        let src = r#"
        @DiSaBlE MINE;
        SkIlL travel(system: system_id) {
          go $system;
        }
        OvErRiDe safety WhEn FUEL() <= 5 {
          halt;
        }
        "#;
        let err = parse_library(src).expect_err("expected parse error");
        assert_eq!(err[0].code, "DSL052");
    }

    #[test]
    fn rejects_unknown_directive_in_library() {
        let src = r#"
        @nope mine
        "#;
        let err = parse_library(src).expect_err("expected parse error");
        assert_eq!(err[0].code, "DSL052");
    }

    #[test]
    fn parses_empty_script() {
        let p = parse_script("").expect("empty script");
        assert_eq!(p.statements.len(), 0);
    }

    #[test]
    fn parses_whitespace_only_script() {
        let p = parse_script("   \n  \t  ").expect("whitespace only");
        assert_eq!(p.statements.len(), 0);
    }

    #[test]
    fn rejects_comment_only_script() {
        // The parser does not treat a comment-only (non-empty) source as valid
        let src = "// this is a comment\n// another comment";
        let err = parse_script(src).expect_err("expected parse error");
        assert_eq!(err[0].code, "DSL104");
    }

    #[test]
    fn parses_multi_argument_command() {
        let src = "buy iron_ore 10;";
        let p = parse_script(src).expect("multi-arg");
        assert_eq!(p.statements.len(), 1);
        let cmd = command_node(&p.statements[0]);
        assert_eq!(cmd.name, "buy");
        assert_eq!(cmd.args, vec!["iron_ore", "10"]);
    }

    #[test]
    fn parses_command_with_hyphenated_arg() {
        let src = "go alpha-1;";
        let p = parse_script(src).expect("hyphen arg");
        assert_eq!(p.statements.len(), 1);
        let cmd = command_node(&p.statements[0]);
        assert_eq!(cmd.args, vec!["alpha-1"]);
    }

    #[test]
    fn parses_all_comparison_operators() {
        for op in [">", ">=", "<", "<=", "==", "!="] {
            let src = format!("if FUEL() {op} 50 {{ halt; }}");
            let p = parse_script(&src).expect("parse failed for comparison operator");
            assert_eq!(p.statements.len(), 1, "op: {op}");
        }
    }

    #[test]
    fn parses_nested_if_inside_until() {
        let src = "until FUEL() >= 50 { if CREDITS() > 0 { halt; } }";
        let p = parse_script(src).expect("nested");
        assert_eq!(p.statements.len(), 1);
        let until_block = until_node(&p.statements[0]);
        assert_eq!(until_block.body.len(), 1);
        assert!(matches!(until_block.body[0], AstNode::If(_)));
    }

    #[test]
    fn parses_macro_tokens_as_args() {
        for macro_name in ["$here", "$home", "$nearest_station"] {
            let src = format!("go {macro_name};");
            let p = parse_script(&src).expect("failed to parse macro token");
            let cmd = command_node(&p.statements[0]);
            assert_eq!(cmd.args[0], macro_name, "macro: {macro_name}");
        }
    }

    #[test]
    fn parses_metric_call_with_argument() {
        let src = "if CARGO(iron_ore) >= 5 { halt; }";
        let p = parse_script(src).expect("metric with arg");
        assert_eq!(p.statements.len(), 1);
    }

    #[test]
    fn parses_boolean_predicate_in_if() {
        let src = "if MISSION_COMPLETE(m1) { halt; }";
        let p = parse_script(src).expect("boolean pred");
        assert_eq!(p.statements.len(), 1);
        let block = if_node(&p.statements[0]);
        assert!(matches!(block.condition, ConditionExpr::MetricCall(_)));
    }
}
