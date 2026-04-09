use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::engine::GameState;

use super::{
    ArgType, AstNode, AstProgram, CommandNode, CommandSpec, ConditionExpr, SkillAstNode,
    SkillLibraryAst, SkillParamDef, Span,
};

/// Analyzer argument form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyzedArg {
    /// Canonical/resolved static value.
    Resolved(String),
    /// Value resolved at emit time from live `GameState`.
    Dynamic(DynamicMacro),
}

/// Dynamic macro resolved by engine at emit time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DynamicMacro {
    /// `$home`
    Home,
    /// `$nearest_station`
    NearestStation,
}

/// Analyzer node tree mirroring `AstNode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyzedNode {
    /// Command with resolved/dynamic arguments.
    Command(AnalyzedCommand),
    /// If block.
    If(AnalyzedConditional),
    /// Until block.
    Until(AnalyzedConditional),
}

/// Analyzer command payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedCommand {
    /// Original source command node.
    pub source: CommandNode,
    /// Resolved or dynamic arguments.
    pub args: Vec<AnalyzedArg>,
}

/// Analyzer conditional payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedConditional {
    /// Condition expression copied from source AST.
    pub condition: ConditionExpr,
    /// Nested analyzer nodes.
    pub body: Vec<AnalyzedNode>,
    /// Source span from source AST.
    pub span: Span,
}

/// Analyzer output program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedProgram {
    /// Top-level analyzer nodes.
    pub statements: Vec<AnalyzedNode>,
    /// Script-load resolved `$here` value.
    pub here: Option<String>,
}

/// Analyzer skill node with analyzed body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedSkillAstNode {
    /// Skill name.
    pub name: String,
    /// Skill parameters.
    pub params: Vec<SkillParamDef>,
    /// Analyzed body.
    pub body: Vec<AnalyzedNode>,
}

/// Analyzer override node with analyzed body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedOverrideAstNode {
    /// Override name.
    pub name: String,
    /// Condition.
    pub condition: ConditionExpr,
    /// Analyzed body.
    pub body: Vec<AnalyzedNode>,
}

/// Analyzer skill-library output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AnalyzedSkillLibrary {
    /// Analyzed skills.
    pub skills: Vec<AnalyzedSkillAstNode>,
    /// Analyzed overrides.
    pub overrides: Vec<AnalyzedOverrideAstNode>,
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

/// Analyze parsed AST into resolved/dynamic argument representation.
pub(super) fn analyze(
    program: &AstProgram,
    catalog: &HashMap<String, CommandSpec>,
    state: &GameState,
) -> Result<AnalyzedProgram, Vec<AnalyzerError>> {
    let mut errors = Vec::new();
    let statements = analyze_nodes(&program.statements, catalog, state, &mut errors);

    if errors.is_empty() {
        Ok(AnalyzedProgram {
            statements,
            here: state.system.clone(),
        })
    } else {
        Err(errors)
    }
}

/// Analyze parsed skill library nodes into analyzed tree form.
pub(super) fn analyze_library(
    library: &SkillLibraryAst,
    catalog: &HashMap<String, CommandSpec>,
    state: &GameState,
) -> Result<AnalyzedSkillLibrary, Vec<AnalyzerError>> {
    let mut errors = Vec::new();

    let skills = library
        .skills
        .iter()
        .map(|skill| AnalyzedSkillAstNode {
            name: skill.name.clone(),
            params: skill.params.clone(),
            body: analyze_nodes(&skill.body, catalog, state, &mut errors),
        })
        .collect::<Vec<_>>();

    let overrides = library
        .overrides
        .iter()
        .map(|ov| AnalyzedOverrideAstNode {
            name: ov.name.clone(),
            condition: ov.condition.clone(),
            body: analyze_nodes(&ov.body, catalog, state, &mut errors),
        })
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(AnalyzedSkillLibrary { skills, overrides })
    } else {
        Err(errors)
    }
}

fn analyze_nodes(
    nodes: &[AstNode],
    catalog: &HashMap<String, CommandSpec>,
    state: &GameState,
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
            AstNode::If(block) => AnalyzedNode::If(AnalyzedConditional {
                condition: block.condition.clone(),
                body: analyze_nodes(&block.body, catalog, state, errors),
                span: block.span,
            }),
            AstNode::Until(block) => AnalyzedNode::Until(AnalyzedConditional {
                condition: block.condition.clone(),
                body: analyze_nodes(&block.body, catalog, state, errors),
                span: block.span,
            }),
        })
        .collect()
}

fn analyze_command_args(
    cmd: &CommandNode,
    catalog: &HashMap<String, CommandSpec>,
    state: &GameState,
    errors: &mut Vec<AnalyzerError>,
) -> Vec<AnalyzedArg> {
    let spec = catalog.get(&cmd.name.to_lowercase());

    cmd.args
        .iter()
        .enumerate()
        .map(|(idx, arg)| {
            let arg_type = spec
                .and_then(|s| s.args.get(idx))
                .map(|a| a.kind)
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
    state: &GameState,
    errors: &mut Vec<AnalyzerError>,
) -> AnalyzedArg {
    if let Some(name) = arg.strip_prefix('$') {
        return match name.to_ascii_lowercase().as_str() {
            "here" => match state.system.as_ref() {
                Some(system) => AnalyzedArg::Resolved(system.clone()),
                None => {
                    errors.push(AnalyzerError {
                        command: cmd.name.clone(),
                        arg_index: idx,
                        value: arg.to_string(),
                        suggestion: None,
                        span: cmd.span,
                        message: "macro '$here' is not available in current state".to_string(),
                    });
                    AnalyzedArg::Resolved(arg.to_string())
                }
            },
            "home" => AnalyzedArg::Dynamic(DynamicMacro::Home),
            "nearest_station" => AnalyzedArg::Dynamic(DynamicMacro::NearestStation),
            _ => {
                errors.push(AnalyzerError {
                    command: cmd.name.clone(),
                    arg_index: idx,
                    value: arg.to_string(),
                    suggestion: None,
                    span: cmd.span,
                    message: format!("unknown macro '{arg}'"),
                });
                AnalyzedArg::Resolved(arg.to_string())
            }
        };
    }

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
    state: &GameState,
) -> Option<(String, Option<String>, Option<String>)> {
    let candidates = match arg_type {
        ArgType::ItemId => {
            let mut out = state.cargo.keys().cloned().collect::<Vec<_>>();
            out.extend(state.galaxy.item_ids.iter().cloned());
            for items in state.stash.values() {
                out.extend(items.keys().cloned());
            }
            dedupe(out)
        }
        ArgType::PoiId => {
            let mut out = state.galaxy.pois.clone();
            out.extend(state.stash.keys().cloned());
            out.extend(
                [state.home_base.clone(), state.nearest_station.clone()]
                    .into_iter()
                    .flatten(),
            );
            dedupe(out)
        }
        ArgType::SystemId => {
            let mut out = state.galaxy.systems.clone();
            out.extend([state.system.clone()].into_iter().flatten());
            dedupe(out)
        }
        ArgType::GoTarget => {
            let mut out = state.galaxy.systems.clone();
            out.extend(state.galaxy.pois.iter().cloned());
            out.extend(
                [
                    state.system.clone(),
                    state.home_base.clone(),
                    state.nearest_station.clone(),
                ]
                .into_iter()
                .flatten(),
            );
            dedupe(out)
        }
        ArgType::MissionId => dedupe(
            state
                .missions
                .active
                .iter()
                .chain(state.missions.available.iter())
                .cloned()
                .collect(),
        ),
        ArgType::ShipId => dedupe(state.owned_ships.iter().cloned().collect()),
        ArgType::ModuleId => dedupe(state.installed_modules.iter().cloned().collect()),
        ArgType::RecipeId => dedupe(state.galaxy.recipe_ids.clone()),
        ArgType::ListingId => dedupe(state.market.shipyard_listings.clone()),
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

fn _collect_skill_names(skills: &[SkillAstNode]) -> HashMap<String, usize> {
    skills
        .iter()
        .map(|s| (s.name.to_lowercase(), s.params.len()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{ArgSpec, AstProgram, OverrideAstNode};

    fn analyzed_command(node: &AnalyzedNode) -> &AnalyzedCommand {
        match node {
            AnalyzedNode::Command(cmd) => Some(cmd),
            _ => None,
        }
        .expect("expected command")
    }

    fn analyzed_if(node: &AnalyzedNode) -> &AnalyzedConditional {
        match node {
            AnalyzedNode::If(block) => Some(block),
            _ => None,
        }
        .expect("expected if block")
    }

    fn analyzed_until(node: &AnalyzedNode) -> &AnalyzedConditional {
        match node {
            AnalyzedNode::Until(block) => Some(block),
            _ => None,
        }
        .expect("expected until block")
    }

    #[test]
    fn analyze_resolves_here_macro() {
        let ast = AstProgram::parse("go $here;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "go".to_string(),
            CommandSpec {
                name: "go".to_string(),
                args: vec![ArgSpec {
                    name: "destination".to_string(),
                    kind: ArgType::Any,
                    required: true,
                }],
            },
        );
        let state = GameState {
            system: Some("sol".to_string()),
            ..GameState::default()
        };

        let analyzed = analyze(&ast, &catalog, &state).expect("analyze");
        let cmd = analyzed_command(&analyzed.statements[0]);
        assert_eq!(cmd.args[0], AnalyzedArg::Resolved("sol".to_string()));
    }

    #[test]
    fn analyze_marks_dynamic_macros() {
        let ast = AstProgram::parse("go $home;\ngo $nearest_station;").expect("parse");
        let analyzed = analyze(&ast, &HashMap::new(), &GameState::default()).expect("analyze");

        let first = analyzed_command(&analyzed.statements[0]);
        let second = analyzed_command(&analyzed.statements[1]);

        assert_eq!(first.args[0], AnalyzedArg::Dynamic(DynamicMacro::Home));
        assert_eq!(
            second.args[0],
            AnalyzedArg::Dynamic(DynamicMacro::NearestStation)
        );
    }

    #[test]
    fn analyze_rejects_non_integer_for_integer_arg() {
        let ast = AstProgram::parse("wait nope;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "wait".to_string(),
            CommandSpec {
                name: "wait".to_string(),
                args: vec![ArgSpec {
                    name: "ticks".to_string(),
                    kind: ArgType::Integer,
                    required: true,
                }],
            },
        );

        let err = analyze(&ast, &catalog, &GameState::default()).expect_err("expected error");
        assert_eq!(err.len(), 1);
        assert!(err[0].message.contains("expected integer"));
    }

    #[test]
    fn analyze_library_marks_dynamic_macros() {
        let library = SkillLibraryAst {
            skills: vec![SkillAstNode {
                name: "travel".to_string(),
                params: vec![],
                body: vec![AstNode::Command(CommandNode {
                    name: "go".to_string(),
                    args: vec!["$nearest_station".to_string()],
                    span: Span { start: 0, end: 1 },
                })],
            }],
            overrides: vec![OverrideAstNode {
                name: "safety".to_string(),
                condition: ConditionExpr::MetricCall(super::super::MetricCall {
                    name: "FUEL".to_string(),
                    args: vec![],
                }),
                body: vec![AstNode::Command(CommandNode {
                    name: "go".to_string(),
                    args: vec!["$home".to_string()],
                    span: Span { start: 0, end: 1 },
                })],
            }],
            ..SkillLibraryAst::default()
        };

        let analyzed =
            analyze_library(&library, &HashMap::new(), &GameState::default()).expect("analyze");
        let skill_cmd = analyzed_command(&analyzed.skills[0].body[0]);
        let ov_cmd = analyzed_command(&analyzed.overrides[0].body[0]);
        assert_eq!(
            skill_cmd.args[0],
            AnalyzedArg::Dynamic(DynamicMacro::NearestStation)
        );
        assert_eq!(ov_cmd.args[0], AnalyzedArg::Dynamic(DynamicMacro::Home));
    }

    #[test]
    fn analyze_resolves_integer_to_canonical_string() {
        let ast = AstProgram::parse("wait 0042;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "wait".to_string(),
            CommandSpec {
                name: "wait".to_string(),
                args: vec![ArgSpec {
                    name: "ticks".to_string(),
                    kind: ArgType::Integer,
                    required: true,
                }],
            },
        );

        let analyzed = analyze(&ast, &catalog, &GameState::default()).expect("analyze");
        let cmd = analyzed_command(&analyzed.statements[0]);
        assert_eq!(cmd.args[0], AnalyzedArg::Resolved("42".to_string()));
    }

    #[test]
    fn analyze_rejects_unknown_macro() {
        let ast = AstProgram::parse("go $wat;").expect("parse");
        let err = analyze(&ast, &HashMap::new(), &GameState::default()).expect_err("error");
        assert!(err[0].message.contains("unknown macro"));
    }

    #[test]
    fn analyze_suggests_similar_item_id() {
        let ast = AstProgram::parse("buy irn_ore;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "buy".to_string(),
            CommandSpec {
                name: "buy".to_string(),
                args: vec![ArgSpec {
                    name: "item".to_string(),
                    kind: ArgType::ItemId,
                    required: true,
                }],
            },
        );
        let state = GameState {
            cargo: std::sync::Arc::new(HashMap::from([("iron_ore".to_string(), 1)])),
            ..GameState::default()
        };

        let err = analyze(&ast, &catalog, &state).expect_err("error");
        assert_eq!(err[0].suggestion.as_deref(), Some("iron_ore"));
    }

    #[test]
    fn analyze_resolves_mission_id_from_state_mission_sets() {
        let ast = AstProgram::parse("accept_mission rescue_op;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "accept_mission".to_string(),
            CommandSpec {
                name: "accept_mission".to_string(),
                args: vec![ArgSpec {
                    name: "mission_id".to_string(),
                    kind: ArgType::MissionId,
                    required: true,
                }],
            },
        );

        let state = GameState {
            missions: std::sync::Arc::new(crate::engine::MissionData {
                active: vec![],
                available: vec!["rescue_op".to_string()],
                ..crate::engine::MissionData::default()
            }),
            ..GameState::default()
        };

        let analyzed = analyze(&ast, &catalog, &state).expect("analyze");
        let cmd = analyzed_command(&analyzed.statements[0]);
        assert_eq!(cmd.args[0], AnalyzedArg::Resolved("rescue_op".to_string()));
    }

    #[test]
    fn analyze_resolves_mission_id_from_active_missions() {
        let ast = AstProgram::parse("abandon_mission active_op;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "abandon_mission".to_string(),
            CommandSpec {
                name: "abandon_mission".to_string(),
                args: vec![ArgSpec {
                    name: "mission_id".to_string(),
                    kind: ArgType::MissionId,
                    required: true,
                }],
            },
        );

        let state = GameState {
            missions: std::sync::Arc::new(crate::engine::MissionData {
                active: vec!["active_op".to_string()],
                available: vec![],
                ..crate::engine::MissionData::default()
            }),
            ..GameState::default()
        };

        let analyzed = analyze(&ast, &catalog, &state).expect("analyze");
        let cmd = analyzed_command(&analyzed.statements[0]);
        assert_eq!(cmd.args[0], AnalyzedArg::Resolved("active_op".to_string()));
    }

    #[test]
    fn analyze_optional_arg_absent_succeeds() {
        let ast = AstProgram::parse("mine;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "mine".to_string(),
            CommandSpec {
                name: "mine".to_string(),
                args: vec![ArgSpec {
                    name: "resource".to_string(),
                    kind: ArgType::ItemId,
                    required: false,
                }],
            },
        );

        let analyzed = analyze(&ast, &catalog, &GameState::default()).expect("analyze");
        let cmd = analyzed_command(&analyzed.statements[0]);
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn analyze_if_block_contains_analyzed_body() {
        let ast = AstProgram::parse("if MISSION_COMPLETE(m1) { halt; }").expect("parse");
        let analyzed = analyze(&ast, &HashMap::new(), &GameState::default()).expect("analyze");
        let block = analyzed_if(&analyzed.statements[0]);
        assert_eq!(block.body.len(), 1);
        assert!(matches!(&block.body[0], AnalyzedNode::Command(_)));
    }

    #[test]
    fn analyze_until_block_contains_analyzed_body() {
        let ast = AstProgram::parse("until FUEL() >= 50 { halt; }").expect("parse");
        let analyzed = analyze(&ast, &HashMap::new(), &GameState::default()).expect("analyze");
        let block = analyzed_until(&analyzed.statements[0]);
        assert_eq!(block.body.len(), 1);
    }

    #[test]
    fn analyze_home_macro_is_dynamic() {
        let ast = AstProgram::parse("go $home;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "go".to_string(),
            CommandSpec {
                name: "go".to_string(),
                args: vec![ArgSpec {
                    name: "destination".to_string(),
                    kind: ArgType::Any,
                    required: true,
                }],
            },
        );
        let state = GameState {
            home_base: Some("earth_base".to_string()),
            ..GameState::default()
        };

        let analyzed = analyze(&ast, &catalog, &state).expect("analyze");
        let cmd = analyzed_command(&analyzed.statements[0]);
        assert_eq!(cmd.args[0], AnalyzedArg::Dynamic(DynamicMacro::Home));
    }

    #[test]
    fn analyze_accumulates_multiple_errors() {
        let ast = AstProgram::parse("wait nope;\nwait also_nope;").expect("parse");
        let mut catalog = HashMap::new();
        catalog.insert(
            "wait".to_string(),
            CommandSpec {
                name: "wait".to_string(),
                args: vec![ArgSpec {
                    name: "ticks".to_string(),
                    kind: ArgType::Integer,
                    required: true,
                }],
            },
        );

        let errs = analyze(&ast, &catalog, &GameState::default()).expect_err("expected errors");
        assert_eq!(errs.len(), 2);
    }
}
