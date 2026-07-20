//! Source-independent actions shared by Prayer producers and executors.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Current persisted action-envelope schema.
pub const ACTION_SCHEMA_VERSION: u32 = 6;

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

string_id!(ActionId);
string_id!(RunId);
string_id!(ItemId);

/// A source location retained for diagnostics and active-line projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceRef {
    pub source_name: Option<String>,
    pub start: usize,
    pub end: usize,
    pub line: Option<u32>,
}

/// A materialized navigation destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GoTarget {
    Identifier(String),
    System(String),
    Poi(String),
    Coordinate { x: i64, y: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TransferRequest {
    pub subject: TransferSubject,
    pub from: TransferEndpoint,
    pub to: TransferEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransferSubject {
    AllCargo,
    Credits { quantity: u64 },
    Item { id: ItemId, quantity: Option<u64> },
    Ship { id: String },
    Module { id: String },
    Items { items: Vec<TransferItem> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TransferItem {
    pub id: ItemId,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum TransferEndpoint {
    Cargo,
    Storage,
    Ship(String),
    Faction,
    FactionTag(String),
    Player(String),
    Space(Option<String>),
    Commission(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CraftRequest {
    pub recipe_id: String,
    pub quantity: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facility_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecycleRequest {
    pub recipe_id: String,
    pub quantity: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facility_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommissionShipRequest {
    pub ship_class: String,
    #[serde(default)]
    pub provide_materials: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BuyRequest {
    pub item: ItemId,
    pub quantity: u64,
    pub max_price: Option<u64>,
    pub place_order: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SellRequest {
    pub item: Option<ItemId>,
    pub quantity: Option<u64>,
    pub min_price: Option<u64>,
    pub place_order: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindRequest {
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SayRequest {
    pub content: String,
    pub channel: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceTransferRequest {
    pub target: Option<String>,
    pub quantity: Option<u64>,
    pub item: Option<ItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TradeItem {
    pub item: ItemId,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TradeOfferRequest {
    pub target: String,
    pub offer_items: Vec<TradeItem>,
    pub offer_credits: Option<u64>,
    pub request_items: Vec<TradeItem>,
    pub request_credits: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FacilityUpgradeRequest {
    pub facility_id: String,
    pub facility_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FacilityAccessRequest {
    pub facility_id: String,
    pub access: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FacilityOutputPriceRequest {
    pub facility_id: String,
    pub item: ItemId,
    pub price: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FacilityNameRequest {
    pub facility_id: String,
    pub custom_name: String,
}

/// Typed value carried by catalog actions that do not yet warrant a dedicated
/// request structure. This is the stable action representation, not a raw JSON
/// or legacy engine-command payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ActionArg {
    Any(String),
    Integer(i64),
    ItemId(String),
    SystemId(String),
    PoiId(String),
    GoTarget(String),
    ShipId(String),
    ListingId(String),
    MissionId(String),
    ModuleId(String),
    RecipeId(String),
}

impl ActionArg {
    pub fn as_text(&self) -> String {
        match self {
            Self::Integer(value) => value.to_string(),
            Self::Any(value)
            | Self::ItemId(value)
            | Self::SystemId(value)
            | Self::PoiId(value)
            | Self::GoTarget(value)
            | Self::ShipId(value)
            | Self::ListingId(value)
            | Self::MissionId(value)
            | Self::ModuleId(value)
            | Self::RecipeId(value) => value.clone(),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Integer(_) => None,
            Self::Any(value)
            | Self::ItemId(value)
            | Self::SystemId(value)
            | Self::PoiId(value)
            | Self::GoTarget(value)
            | Self::ShipId(value)
            | Self::ListingId(value)
            | Self::MissionId(value)
            | Self::ModuleId(value)
            | Self::RecipeId(value) => Some(value),
        }
    }
}

/// Materialized typed action passed to runtime executors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedAction {
    pub action: String,
    pub args: Vec<ActionArg>,
    pub source_line: Option<usize>,
}

impl ResolvedAction {
    pub fn args_as_strings(&self) -> Vec<String> {
        self.args.iter().map(ActionArg::as_text).collect()
    }
}

/// One exhaustive high-level Prayer operation.
///
/// This is the durable queue protocol. Every executable operation has a
/// dedicated variant; generic names and positional argument vectors are
/// intentionally confined to transient executor adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "request", rename_all = "snake_case")]
pub enum Action {
    Halt,
    Wait {
        ticks: u64,
    },
    Go {
        destination: GoTarget,
    },
    Dock,
    Undock,
    Mine {
        resource: Option<ItemId>,
    },
    Transfer(TransferRequest),
    SetHome,
    Find(FindRequest),
    Survey,
    Attack {
        target_id: String,
    },
    Scan {
        target: Option<String>,
    },
    Cloak {
        enabled: bool,
    },
    Hunt {
        target: String,
    },
    PrepayTax {
        quantity: u64,
    },
    AcceptMission {
        mission_id: String,
    },
    AbandonMission {
        mission_id: String,
    },
    DeclineMission {
        template_id: String,
    },
    CompleteMission {
        mission_id: String,
    },
    LoadPassenger {
        destination: String,
    },
    UnloadPassenger {
        name: String,
        target: Option<String>,
    },
    Buy(BuyRequest),
    Sell(SellRequest),
    CancelBuy {
        item: ItemId,
    },
    CancelSell {
        item: ItemId,
    },
    FactionCreate {
        name: String,
        tag: String,
    },
    FactionInvite {
        player: String,
    },
    FactionAcceptInvite {
        faction: String,
    },
    FactionKick {
        player: String,
    },
    FactionSetRole {
        player: String,
        role: String,
    },
    FoundStation {
        name: String,
        public_access: bool,
    },
    FacilityBuild {
        facility_type: String,
    },
    FactionFacilityBuild {
        facility_type: String,
    },
    FacilityUpgrade(FacilityUpgradeRequest),
    FactionFacilityUpgrade(FacilityUpgradeRequest),
    FacilityDismantle {
        facility_id: String,
    },
    FactionFacilityDismantle {
        facility_id: String,
    },
    FacilitySetAccess(FacilityAccessRequest),
    FacilitySetOutputPrice(FacilityOutputPriceRequest),
    FacilitySetName(FacilityNameRequest),
    UseItem {
        item: ItemId,
        quantity: u64,
    },
    Repair(ServiceTransferRequest),
    RepairModule {
        module: String,
    },
    Recycle(RecycleRequest),
    Refuel(ServiceTransferRequest),
    SelfDestruct,
    SwitchShip {
        ship: String,
    },
    RenameShip {
        name: String,
    },
    InstallMod {
        module: String,
    },
    UninstallMod {
        module: String,
    },
    BuyShip {
        listing: String,
    },
    BuyListedShip {
        listing: String,
    },
    CommissionShip(CommissionShipRequest),
    SellShip {
        ship: String,
    },
    ScrapShip {
        ship: String,
    },
    ListShipForSale {
        ship: String,
        price: u64,
    },
    RefitShip,
    CancelCommission {
        commission_id: String,
    },
    SupplyCommission {
        commission_id: String,
        item: ItemId,
        quantity: u64,
    },
    CancelShipListing {
        listing_id: String,
    },
    PlaceShipBuyOrder {
        ship_class: String,
        price: u64,
    },
    CancelShipBuyOrder {
        order_id: String,
    },
    SellShipToOrder {
        order_id: String,
        ship_id: String,
    },
    CancelOrder {
        order_id: String,
    },
    ModifyOrder {
        order_id: String,
        price_each: u64,
    },
    Craft(CraftRequest),
    CancelCraftJob {
        job_id: String,
    },
    SalvageWreck {
        wreck_id: String,
    },
    TowWreck {
        wreck_id: String,
    },
    ScrapWreck,
    SellWreck,
    ReleaseWreck,
    InsureShip {
        ticks: u64,
    },
    CitizenshipApply {
        empire_id: String,
    },
    CitizenshipWithdraw {
        empire_id: String,
    },
    CitizenshipRenounce {
        empire_id: String,
    },
    TradeOffer(TradeOfferRequest),
    TradeAccept {
        trade_id: String,
    },
    FactionLeave,
    FactionWithdrawInvite {
        player: String,
    },
    FactionProposeAlly {
        faction: String,
    },
    FactionAcceptAlly {
        faction: String,
    },
    FactionRemoveAlly {
        faction: String,
    },
    FactionDeclareWar {
        faction: String,
        reason: Option<String>,
    },
    FactionProposePeace {
        faction: String,
        message: Option<String>,
    },
    FactionAcceptPeace {
        faction: String,
    },
    FactionSetEnemy {
        faction: String,
    },
    FactionRemoveEnemy {
        faction: String,
    },
    FactionPrepayTax {
        quantity: u64,
    },
    FactionCancelMission {
        mission_id: String,
    },
    Espionage,
    ScanPoi {
        poi_id: String,
    },
    DistressSignal {
        distress_type: Option<String>,
    },
    Say(SayRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionOrigin {
    PrayerLang {
        run_id: RunId,
        source: Option<SourceRef>,
    },
    Controller {
        run_id: RunId,
        controller: String,
    },
    Manual {
        run_id: RunId,
    },
    Interrupt {
        policy: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub schema_version: u32,
    pub id: ActionId,
    pub action: Action,
    pub origin: ActionOrigin,
    pub idempotency_key: Option<String>,
}

impl ActionEnvelope {
    pub fn new(id: impl Into<ActionId>, action: Action, origin: ActionOrigin) -> Self {
        Self {
            schema_version: ACTION_SCHEMA_VERSION,
            id: id.into(),
            action,
            origin,
            idempotency_key: None,
        }
    }
}

/// Serializable executor state attached to a single running action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuationEnvelope {
    pub schema_version: u32,
    pub executor: String,
    pub state: serde_json::Value,
}

/// Stable completion classification; detailed messages remain executor-owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActionOutcome {
    Succeeded,
    Failed {
        class: FailureClass,
        message: String,
        retryable: bool,
    },
    Cancelled {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    Invalid,
    Conflict,
    Transport,
    Rejected,
    Unsupported,
    Internal,
}
