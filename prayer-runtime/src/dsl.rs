//! Prayer DSL parser, validator, and formatter.

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use ariadne::{Color, Config, Label, Report, ReportKind, Source};
use serde::{Deserialize, Serialize};

use crate::state::GameState;

#[cfg(test)]
use ariadne::CharSet;

mod analyzer;
mod ast;
mod parser;
mod render;

pub use analyzer::*;
pub use ast::*;

/// Byte span in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

/// Severity for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Error-level diagnostic.
    Error,
    /// Warning-level diagnostic.
    Warning,
}

/// Machine-readable diagnostic for parser and validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable error code.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// Error location in source.
    pub span: Span,
    /// Severity level.
    pub severity: Severity,
}

impl Diagnostic {
    /// Render this diagnostic into a colorful string via ariadne.
    pub fn render(&self, source_name: &str, input: &str) -> String {
        let mut output = Vec::new();
        let kind = match self.severity {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
        };

        let report_span = (
            source_name,
            self.span.start..self.span.end.max(self.span.start + 1),
        );
        let _ = Report::build(kind, report_span.clone())
            .with_config(Config::default().with_compact(true))
            .with_code(self.code)
            .with_message(self.message.clone())
            .with_label(
                Label::new(report_span)
                    .with_message(self.message.clone())
                    .with_color(Color::Red),
            )
            .finish()
            .write((source_name, Source::from(input)), &mut output);

        String::from_utf8_lossy(&output).to_string()
    }
}

/// Skill parameter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgType {
    /// Any identifier-like token.
    Any,
    /// Integer value.
    Integer,
    /// Item id.
    ItemId,
    /// System id.
    SystemId,
    /// Poi id.
    PoiId,
    /// Go target.
    GoTarget,
    /// Ship id.
    ShipId,
    /// Listing id.
    ListingId,
    /// Mission id.
    MissionId,
    /// Module id.
    ModuleId,
    /// Recipe id.
    RecipeId,
}

impl FromStr for ArgType {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "any" => Ok(Self::Any),
            "integer" => Ok(Self::Integer),
            "item_id" => Ok(Self::ItemId),
            "system_id" => Ok(Self::SystemId),
            "poi_id" => Ok(Self::PoiId),
            "go_target" => Ok(Self::GoTarget),
            "ship_id" => Ok(Self::ShipId),
            "listing_id" => Ok(Self::ListingId),
            "mission_id" => Ok(Self::MissionId),
            "module_id" => Ok(Self::ModuleId),
            "recipe_id" => Ok(Self::RecipeId),
            _ => Err(()),
        }
    }
}

impl ArgType {
    /// Canonical snake_case name for this type.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Integer => "integer",
            Self::ItemId => "item_id",
            Self::SystemId => "system_id",
            Self::PoiId => "poi_id",
            Self::GoTarget => "go_target",
            Self::ShipId => "ship_id",
            Self::ListingId => "listing_id",
            Self::MissionId => "mission_id",
            Self::ModuleId => "module_id",
            Self::RecipeId => "recipe_id",
        }
    }
}

/// Command argument specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgSpec {
    /// Argument name for diagnostics and payload mapping.
    pub name: String,
    /// Expected argument type.
    pub kind: ArgType,
    /// Required flag.
    pub required: bool,
}

/// Command metadata for semantic validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Command name.
    pub name: String,
    /// Ordered argument specs.
    pub args: Vec<ArgSpec>,
}

/// Predicate metadata for semantic validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateSpec {
    /// Predicate name.
    pub name: String,
    /// Required argument count.
    pub arity: usize,
}

/// Validation context for semantic checks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationContext {
    /// Command signatures keyed by lowercase name.
    pub commands: HashMap<String, CommandSpec>,
    /// Boolean predicates keyed by uppercase name.
    pub boolean_predicates: HashMap<String, PredicateSpec>,
    /// Numeric predicates keyed by uppercase name.
    pub numeric_predicates: HashMap<String, PredicateSpec>,
    /// Optional skill signatures keyed by lowercase name.
    pub skills: HashMap<String, usize>,
}

impl AstProgram {
    /// Parse a DSL script body.
    pub fn parse(input: &str) -> Result<Self, Vec<Diagnostic>> {
        parser::parse_script(input)
    }

    /// Validate a parsed program against the provided context.
    pub fn validate(&self, context: &ValidationContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for node in &self.statements {
            validate_node(node, context, &mut diagnostics);
        }
        diagnostics
    }

    /// Canonically format this program.
    pub fn normalize(&self) -> String {
        render::normalize(self)
    }

    /// Analyze this program into resolved/dynamic argument form.
    pub fn analyze(
        &self,
        catalog: &HashMap<String, CommandSpec>,
        state: &GameState,
    ) -> Result<AnalyzedProgram, Vec<AnalyzerError>> {
        analyzer::analyze(self, catalog, state)
    }
}

impl SkillLibraryAst {
    /// Parse a skill library (`skill`, `override`, and `@disable`).
    pub fn parse(input: &str) -> Result<Self, Vec<Diagnostic>> {
        let parsed = parser::parse_library(input)?;
        validate_library_no_recursion(&parsed.skills)?;
        Ok(parsed)
    }

    /// Canonically format this library.
    pub fn normalize(&self) -> String {
        render::normalize_library(self)
    }

    /// Analyze this library into resolved/dynamic argument form.
    pub fn analyze(
        &self,
        catalog: &HashMap<String, CommandSpec>,
        state: &GameState,
    ) -> Result<AnalyzedSkillLibrary, Vec<AnalyzerError>> {
        analyzer::analyze_library(self, catalog, state)
    }
}

impl ConditionExpr {
    /// Parse an isolated condition expression.
    pub fn parse(input: &str) -> Result<Self, Vec<Diagnostic>> {
        parser::parse_condition(input)
    }
}

impl ValidationContext {
    /// Build a validation context from the default runtime catalogs and an optional skill library.
    pub fn with_defaults(skill_library: Option<&SkillLibraryAst>) -> Self {
        let mut context = Self {
            commands: crate::catalog::default_command_catalog(),
            ..Self::default()
        };
        let (boolean_predicates, numeric_predicates) = crate::catalog::default_predicate_catalog();
        context.boolean_predicates = boolean_predicates;
        context.numeric_predicates = numeric_predicates;

        if let Some(library) = skill_library {
            for skill in &library.skills {
                context
                    .skills
                    .insert(skill.name.to_lowercase(), skill.params.len());
            }
        }

        context
    }
}

fn validate_node(node: &AstNode, context: &ValidationContext, out: &mut Vec<Diagnostic>) {
    match node {
        AstNode::Command(cmd) => {
            let name = cmd.name.to_lowercase();
            if !context.commands.contains_key(&name) && !context.skills.contains_key(&name) {
                out.push(diag(
                    "DSL200",
                    &format!("unknown command '{}'", cmd.name),
                    cmd.span.start,
                    cmd.span.end,
                ));
                return;
            }

            if let Some(spec) = context.commands.get(&name) {
                validate_command_args(cmd, spec, out);
            }
            if let Some(arity) = context.skills.get(&name) {
                if cmd.args.len() != *arity {
                    out.push(diag(
                        "DSL201",
                        &format!(
                            "skill '{}' expects {} args, got {}",
                            cmd.name,
                            arity,
                            cmd.args.len()
                        ),
                        cmd.span.start,
                        cmd.span.end,
                    ));
                }
            }
        }
        AstNode::If(c) | AstNode::Until(c) => {
            validate_condition(&c.condition, c.span, context, out);
            for child in &c.body {
                validate_node(child, context, out);
            }
        }
    }
}

fn validate_command_args(cmd: &CommandNode, spec: &CommandSpec, out: &mut Vec<Diagnostic>) {
    if cmd.args.len() > spec.args.len() {
        out.push(diag(
            "DSL202",
            &format!("command '{}' has too many arguments", cmd.name),
            cmd.span.start,
            cmd.span.end,
        ));
        return;
    }

    let required = spec.args.iter().filter(|a| a.required).count();
    if cmd.args.len() < required {
        out.push(diag(
            "DSL203",
            &format!("command '{}' is missing required arguments", cmd.name),
            cmd.span.start,
            cmd.span.end,
        ));
        return;
    }

    for (idx, arg) in cmd.args.iter().enumerate() {
        if let Some(spec_arg) = spec.args.get(idx) {
            let ok = match spec_arg.kind {
                ArgType::Integer => parser::is_valid_integer_token(arg),
                _ => parser::is_valid_arg_token(arg),
            };
            if !ok {
                out.push(diag(
                    "DSL204",
                    &format!("argument {} of '{}' is invalid", idx + 1, cmd.name),
                    cmd.span.start,
                    cmd.span.end,
                ));
            }
        }
    }
}

fn validate_condition(
    condition: &ConditionExpr,
    span: Span,
    context: &ValidationContext,
    out: &mut Vec<Diagnostic>,
) {
    match condition {
        ConditionExpr::MetricCall(call) => {
            if let Some(pred) = context.boolean_predicates.get(&call.name.to_uppercase()) {
                if call.args.len() != pred.arity {
                    out.push(diag(
                        "DSL205",
                        &format!(
                            "predicate '{}' expects {} args, got {}",
                            call.name,
                            pred.arity,
                            call.args.len()
                        ),
                        span.start,
                        span.end,
                    ));
                }
            } else {
                out.push(diag(
                    "DSL206",
                    &format!("unknown boolean predicate '{}'", call.name),
                    span.start,
                    span.end,
                ));
            }
        }
        ConditionExpr::Comparison { left, right, .. } => {
            validate_numeric_operand(left, span, context, out);
            validate_numeric_operand(right, span, context, out);
        }
    }
}

fn validate_numeric_operand(
    operand: &NumericOperand,
    span: Span,
    context: &ValidationContext,
    out: &mut Vec<Diagnostic>,
) {
    if let NumericOperand::MetricCall(call) = operand {
        if let Some(pred) = context.numeric_predicates.get(&call.name.to_uppercase()) {
            if call.args.len() != pred.arity {
                out.push(diag(
                    "DSL207",
                    &format!(
                        "numeric predicate '{}' expects {} args, got {}",
                        call.name,
                        pred.arity,
                        call.args.len()
                    ),
                    span.start,
                    span.end,
                ));
            }
        } else {
            out.push(diag(
                "DSL208",
                &format!("unknown numeric predicate '{}'", call.name),
                span.start,
                span.end,
            ));
        }
    }
}

fn validate_library_no_recursion(skills: &[SkillAstNode]) -> Result<(), Vec<Diagnostic>> {
    let names: HashSet<String> = skills.iter().map(|s| s.name.to_lowercase()).collect();
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();

    for skill in skills {
        let mut called = HashSet::new();
        collect_calls(&skill.body, &names, &mut called);
        graph.insert(skill.name.to_lowercase(), called);
    }

    for skill in skills {
        let root = skill.name.to_lowercase();
        let mut stack = vec![root.clone()];
        let mut seen = HashSet::new();

        while let Some(curr) = stack.pop() {
            if !seen.insert(curr.clone()) {
                continue;
            }
            if let Some(next) = graph.get(&curr) {
                for n in next {
                    if n == &root {
                        return Err(vec![diag(
                            "DSL090",
                            &format!("recursive skill detected for '{}'", skill.name),
                            0,
                            1,
                        )]);
                    }
                    stack.push(n.clone());
                }
            }
        }
    }

    Ok(())
}

fn collect_calls(nodes: &[AstNode], names: &HashSet<String>, out: &mut HashSet<String>) {
    for n in nodes {
        match n {
            AstNode::Command(c) => {
                let name = c.name.to_lowercase();
                if names.contains(&name) {
                    out.insert(name);
                }
            }
            AstNode::If(b) | AstNode::Until(b) => collect_calls(&b.body, names, out),
        }
    }
}

fn diag(code: &'static str, message: &str, start: usize, end: usize) -> Diagnostic {
    Diagnostic {
        code,
        message: message.to_string(),
        span: Span {
            start,
            end: end.max(start + 1),
        },
        severity: Severity::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> ValidationContext {
        let mut ctx = ValidationContext::default();
        ctx.commands.insert(
            "go".to_string(),
            CommandSpec {
                name: "go".to_string(),
                args: vec![ArgSpec {
                    name: "destination".to_string(),
                    kind: ArgType::GoTarget,
                    required: true,
                }],
            },
        );
        ctx.commands.insert(
            "halt".to_string(),
            CommandSpec {
                name: "halt".to_string(),
                args: vec![],
            },
        );
        ctx.numeric_predicates.insert(
            "FUEL".to_string(),
            PredicateSpec {
                name: "FUEL".to_string(),
                arity: 0,
            },
        );
        ctx.boolean_predicates.insert(
            "MISSION_COMPLETE".to_string(),
            PredicateSpec {
                name: "MISSION_COMPLETE".to_string(),
                arity: 1,
            },
        );
        ctx
    }

    #[test]
    fn validate_unknown_command() {
        let p = AstProgram::parse("warp x;").expect("parse");
        let out = p.validate(&context());
        assert!(out.iter().any(|d| d.code == "DSL200"));
    }

    #[test]
    fn normalize_idempotent() {
        let p1 = AstProgram::parse("go alpha;\nhalt;").expect("p1");
        let s1 = p1.normalize();
        let p2 = AstProgram::parse(&s1).expect("p2");
        let s2 = p2.normalize();
        assert_eq!(s1, s2);
    }

    #[test]
    fn normalize_library_idempotent() {
        let lib1 =
            SkillLibraryAst::parse("skill mine_loop(item: item_id) { mine $item; }\n@disable mine")
                .expect("lib1");
        let text1 = lib1.normalize();
        let lib2 = SkillLibraryAst::parse(&text1).expect("lib2");
        let text2 = lib2.normalize();
        assert_eq!(text1, text2);
    }

    #[test]
    fn parse_library_rejects_recursive_skill_calls() {
        let src = r#"
        skill loop() {
          loop;
        }
        "#;
        let err = SkillLibraryAst::parse(src).expect_err("error");
        assert_eq!(err[0].code, "DSL090");
    }

    #[test]
    fn ariadne_render_works() {
        let d = diag("X", "boom", 0, 1);
        let text = d.render("script.dsl", "halt;");
        assert!(text.contains("boom"));
    }

    #[test]
    fn ariadne_wire_format_demo() {
        let mut output = Vec::new();
        let span = ("script.dsl", 0..4);
        let _ = Report::build(ReportKind::Error, span.clone())
            .with_code("runtime.error")
            .with_message("unsupported command 'mine'")
            .with_label(Label::new(span).with_message("unsupported command 'mine'"))
            .with_config(
                Config::default()
                    .with_color(false)
                    .with_compact(true)
                    .with_char_set(CharSet::Ascii),
            )
            .finish()
            .write(
                ("script.dsl", Source::from("mine ore;\nhalt;")),
                &mut output,
            );
        let text = String::from_utf8_lossy(&output).to_string();
        println!("{text}");
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("unsupported command 'mine'"));
    }

    #[test]
    fn default_validation_context_includes_catalog_and_skills() {
        let library = SkillLibraryAst {
            skills: vec![SkillAstNode {
                name: "loop_mine".to_string(),
                params: vec![SkillParamDef {
                    name: "resource".to_string(),
                    kind: ArgType::ItemId,
                }],
                body: vec![],
            }],
            overrides: vec![],
            disabled_commands: HashSet::new(),
        };

        let ctx = ValidationContext::with_defaults(Some(&library));
        assert!(ctx.commands.contains_key("go"));
        assert!(ctx.numeric_predicates.contains_key("FUEL"));
        assert_eq!(ctx.skills.get("loop_mine"), Some(&1usize));
    }
}
