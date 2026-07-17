//! PrayerLang source syntax tree.

use serde::{Deserialize, Serialize};

use super::Span;

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
    /// Transfer statement.
    Transfer(TransferNode),
    /// Craft statement.
    Craft(CraftNode),
    /// Chat statement.
    Say(SayNode),
    /// Immediate or standing market buy.
    Buy(BuyNode),
    /// Immediate or standing market sell.
    Sell(SellNode),
    /// Recycling job statement.
    Recycle(RecycleNode),
    /// Ship commissioning statement.
    CommissionShip(CommissionShipNode),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SayNode {
    pub content: String,
    pub channel: String,
    pub target: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyNode {
    pub item_id: String,
    pub quantity: u64,
    pub max_price: Option<u64>,
    pub place_order: bool,
    pub deliver_to: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SellNode {
    pub item_id: Option<String>,
    pub quantity: Option<u64>,
    pub min_price: Option<u64>,
    pub place_order: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecycleNode {
    pub recipe_id: String,
    pub quantity: u64,
    pub clauses: CraftClauses,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommissionShipNode {
    pub ship_class: String,
    pub provide_materials: bool,
    pub span: Span,
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

/// Craft statement node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftNode {
    /// Recipe id.
    pub recipe_id: String,
    /// Desired output item count.
    pub quantity: u64,
    /// Optional routing clauses.
    pub clauses: CraftClauses,
    /// Source location.
    pub span: Span,
}

/// Craft statement routing clauses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CraftClauses {
    /// Input/credit source (`storage` or `faction`).
    pub source: Option<String>,
    /// Output destination (`storage` or `faction`).
    pub deliver_to: Option<String>,
    /// Facility id.
    pub facility_id: Option<String>,
    /// Crafting preset.
    pub preset: Option<String>,
}

/// Transfer statement node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferNode {
    /// Transfer subject.
    pub subject: TransferSubject,
    /// Block-form item entries. Empty for flat form.
    #[serde(default)]
    pub items: Vec<TransferItem>,
    /// Source endpoint, if explicitly provided.
    pub from: Option<TransferEndpoint>,
    /// Destination endpoint, if explicitly provided.
    pub to: Option<TransferEndpoint>,
    /// Source location.
    pub span: Span,
}

/// Block-form transfer item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferItem {
    /// Item id.
    pub id: String,
    /// Item quantity.
    pub qty: i64,
}

/// Transfer subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferSubject {
    /// All current cargo.
    AllCargo,
    /// Credits quantity.
    Credits(i64),
    /// Ship instance id.
    Ship {
        id: String,
    },
    Module {
        id: String,
    },
    /// Item id and optional quantity.
    Item {
        id: String,
        qty: Option<i64>,
    },
}

/// Transfer endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferEndpoint {
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
