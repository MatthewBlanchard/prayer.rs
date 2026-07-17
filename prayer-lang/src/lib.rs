//! Parser, validator, formatter, analyzer, and compiler for linear PrayerLang scripts.

use std::{collections::HashMap, str::FromStr};

use ariadne::{Color, Config, Label, Report, ReportKind, Source};
use serde::{Deserialize, Serialize};

mod action_projection;
mod analyzer;
mod ast;
pub mod catalog;
mod compiler;
mod parser;
mod render;

pub use action_projection::*;
pub use analyzer::*;
pub use ast::*;
pub use compiler::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub severity: Severity,
}

impl Diagnostic {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgType {
    Any,
    Integer,
    ItemId,
    SystemId,
    PoiId,
    GoTarget,
    ShipId,
    ListingId,
    MissionId,
    ModuleId,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgSpec {
    pub name: String,
    pub kind: ArgType,
    pub required: bool,
    #[serde(default)]
    pub variadic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub args: Vec<ArgSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationContext {
    pub commands: HashMap<String, CommandSpec>,
}

impl ValidationContext {
    pub fn with_defaults() -> Self {
        Self {
            commands: catalog::default_command_catalog(),
        }
    }
}

impl AstProgram {
    pub fn parse(input: &str) -> Result<Self, Vec<Diagnostic>> {
        parser::parse_script(input)
    }

    pub fn validate(&self, context: &ValidationContext) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for node in &self.statements {
            let AstNode::Command(cmd) = node else {
                continue;
            };
            let Some(spec) = context.commands.get(&cmd.name.to_ascii_lowercase()) else {
                out.push(diag(
                    "DSL200",
                    &format!("unknown command '{}'", cmd.name),
                    cmd.span.start,
                    cmd.span.end,
                ));
                continue;
            };
            validate_command_args(cmd, spec, &mut out);
        }
        out
    }

    pub fn normalize(&self) -> String {
        render::normalize(self)
    }

    pub fn analyze(
        &self,
        catalog: &HashMap<String, CommandSpec>,
        observation: &AnalysisObservation,
    ) -> Result<AnalyzedProgram, Vec<AnalyzerError>> {
        analyzer::analyze(self, catalog, observation)
    }
}

fn validate_command_args(cmd: &CommandNode, spec: &CommandSpec, out: &mut Vec<Diagnostic>) {
    let variadic = spec.args.last().is_some_and(|arg| arg.variadic);
    if !variadic && cmd.args.len() > spec.args.len() {
        out.push(diag(
            "DSL202",
            &format!("command '{}' has too many arguments", cmd.name),
            cmd.span.start,
            cmd.span.end,
        ));
        return;
    }
    let required = spec.args.iter().filter(|arg| arg.required).count();
    if cmd.args.len() < required {
        out.push(diag(
            "DSL203",
            &format!("command '{}' is missing required arguments", cmd.name),
            cmd.span.start,
            cmd.span.end,
        ));
    }
    for (index, value) in cmd.args.iter().enumerate() {
        let Some(arg) = spec
            .args
            .get(index)
            .or_else(|| variadic.then(|| spec.args.last()).flatten())
        else {
            continue;
        };
        if arg.kind == ArgType::Integer && !parser::is_valid_integer_token(value) {
            out.push(diag(
                "DSL204",
                &format!("argument '{}' must be an integer", arg.name),
                cmd.span.start,
                cmd.span.end,
            ));
        }
    }
}

pub(crate) fn diag(code: &'static str, message: &str, start: usize, end: usize) -> Diagnostic {
    Diagnostic {
        code,
        message: message.into(),
        span: Span { start, end },
        severity: Severity::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_normalization_is_idempotent() {
        let first = AstProgram::parse("go alpha; dock; buy iron_ore 10 5;")
            .unwrap()
            .normalize();
        assert_eq!(AstProgram::parse(&first).unwrap().normalize(), first);
    }

    #[test]
    fn validation_rejects_unknown_commands() {
        let parsed = AstProgram::parse("warp alpha;").unwrap();
        assert_eq!(
            parsed.validate(&ValidationContext::with_defaults())[0].code,
            "DSL200"
        );
    }
}
