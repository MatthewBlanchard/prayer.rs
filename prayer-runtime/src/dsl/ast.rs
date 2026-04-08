use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{ArgType, Span};

/// DSL program AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstProgram {
    /// Top-level statements.
    pub statements: Vec<AstNode>,
}

/// DSL statement node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstNode {
    /// Command statement.
    Command(CommandNode),
    /// If block.
    If(ConditionalNode),
    /// Until block.
    Until(ConditionalNode),
}

/// DSL command node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandNode {
    /// Command name.
    pub name: String,
    /// Raw argument tokens.
    pub args: Vec<String>,
    /// Source location.
    pub span: Span,
}

/// Conditional block node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionalNode {
    /// Condition expression.
    pub condition: ConditionExpr,
    /// Block body.
    pub body: Vec<AstNode>,
    /// Source location.
    pub span: Span,
}

/// Condition expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionExpr {
    /// Boolean metric call.
    MetricCall(MetricCall),
    /// Numeric comparison.
    Comparison {
        /// Left operand.
        left: NumericOperand,
        /// Operator.
        op: ComparisonOp,
        /// Right operand.
        right: NumericOperand,
    },
}

/// Comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `==`
    Eq,
    /// `!=`
    Ne,
}

/// Numeric operand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumericOperand {
    /// Integer literal.
    Integer(i64),
    /// Metric call operand.
    MetricCall(MetricCall),
    /// `$param` reference.
    ArgRef(String),
}

/// Metric call node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricCall {
    /// Metric name.
    pub name: String,
    /// Metric args.
    pub args: Vec<String>,
}

/// Skill parameter definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillParamDef {
    /// Parameter name.
    pub name: String,
    /// Expected type.
    pub kind: ArgType,
}

/// Skill definition AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAstNode {
    /// Skill name.
    pub name: String,
    /// Skill parameters.
    pub params: Vec<SkillParamDef>,
    /// Skill body statements.
    pub body: Vec<AstNode>,
}

/// Override definition AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideAstNode {
    /// Override name.
    pub name: String,
    /// Trigger condition.
    pub condition: ConditionExpr,
    /// Override body statements.
    pub body: Vec<AstNode>,
}

/// Parsed library AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SkillLibraryAst {
    /// Declared skills.
    pub skills: Vec<SkillAstNode>,
    /// Declared overrides.
    pub overrides: Vec<OverrideAstNode>,
    /// Disabled command directives.
    pub disabled_commands: HashSet<String>,
}
