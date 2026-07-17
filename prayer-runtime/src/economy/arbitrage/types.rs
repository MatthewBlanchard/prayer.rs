//! Public arbitrage DTOs, enums, score options, and compatibility constants.

use serde::{Deserialize, Serialize};

use crate::economy::EconomyReadState;

fn normalized_passenger_class(class_name: &str) -> String {
    class_name
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .trim_end_matches("_class")
        .to_string()
}

/// Half-life of the recency discount applied to deal scores. A market
/// snapshot is only as fresh as the last visit to its station, and order
/// books move fast, so freshness is weighted aggressively: the score halves
/// for every minute of age on the deal's stalest leg.
pub const RECENCY_HALF_LIFE_SECS: f64 = 60.0;
pub const ARBITRAGE_SCORE_TRIP_OVERHEAD_JUMPS: usize = 2;
pub const RISK_FREE_BREAK_EVEN_COVER: f64 = f64::MAX;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArbitrageScoreOptions {
    pub min_gross_margin: Option<f64>,
    pub min_break_even_cover: Option<f64>,
}

impl Default for ArbitrageScoreOptions {
    fn default() -> Self {
        Self {
            min_gross_margin: None,
            min_break_even_cover: None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ArbitradePackageOptimizationStats {
    pub passenger_fares_ms: u128,
    pub route_overlay_ms: u128,
    pub rank_sort_ms: u128,
    pub greedy_select_ms: u128,
    pub package_materialize_ms: u128,
    pub final_sort_ms: u128,
    pub routes_considered: usize,
    pub ranked_units: usize,
    pub selected_units: usize,
}

impl ArbitrageScoreOptions {
    pub fn accepts(self, gross_margin: f64, break_even_cover: f64) -> bool {
        self.min_gross_margin
            .filter(|minimum| minimum.is_finite())
            .map_or(true, |minimum| gross_margin >= minimum)
            && self
                .min_break_even_cover
                .filter(|minimum| minimum.is_finite())
                .map_or(true, |minimum| break_even_cover >= minimum)
    }
}

/// Coarse margin-of-safety classification for one arbitrage deal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbitrageRiskBand {
    Low,
    Medium,
    High,
    Thin,
}

impl ArbitrageRiskBand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Thin => "thin",
        }
    }
}

/// How an arbitrage haul obtains its cargo.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArbitrageAcquireFrom {
    /// Buy from the station market order book.
    Market,
    /// Pull from faction storage through a local virtual sell order.
    VirtualFaction { order_id: Option<String> },
}

impl ArbitrageAcquireFrom {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::VirtualFaction { .. } => "virtual_faction",
        }
    }

    pub fn virtual_order_id(&self) -> Option<&str> {
        match self {
            Self::Market => None,
            Self::VirtualFaction { order_id } => order_id.as_deref(),
        }
    }
}

impl Default for ArbitrageAcquireFrom {
    fn default() -> Self {
        Self::Market
    }
}

/// How an arbitrage haul disposes of its cargo.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArbitrageDisposeTo {
    /// Sell to the station market order book.
    Market,
    /// Deposit into faction storage through a local virtual buy order.
    VirtualFaction { order_id: Option<String> },
}

impl ArbitrageDisposeTo {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::VirtualFaction { .. } => "virtual_faction",
        }
    }

    pub fn virtual_order_id(&self) -> Option<&str> {
        match self {
            Self::Market => None,
            Self::VirtualFaction { order_id } => order_id.as_deref(),
        }
    }
}

impl Default for ArbitrageDisposeTo {
    fn default() -> Self {
        Self::Market
    }
}

/// Side of a local virtual faction-storage order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualFactionOrderSide {
    /// Faction wants to receive this item.
    Buy,
    /// Faction offers this item from storage.
    Sell,
}

/// Local planning-only liquidity backed by faction storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualFactionOrder {
    /// Local virtual order id.
    pub id: String,
    /// Whether this order acts as supply or demand.
    pub side: VirtualFactionOrderSide,
    /// Item id.
    pub item_id: String,
    /// Station where bots should treat this virtual liquidity as available.
    pub station_id: String,
    /// Price used for scoring only.
    pub price_each: i64,
    /// Maximum units exposed by this virtual order.
    pub quantity: i64,
    /// Disabled virtual orders are ignored.
    pub enabled: bool,
}

/// One profitable buy-here/sell-there opportunity.
#[derive(Debug, Clone, PartialEq)]
pub struct ArbitrageDeal {
    /// Item id.
    pub item_id: String,
    /// Station to buy from (has outstanding sell orders).
    pub buy_station_id: String,
    /// System of the buy station.
    pub buy_system_id: String,
    /// Where the bot obtains this item.
    pub acquire_from: ArbitrageAcquireFrom,
    /// Station to sell at (has outstanding buy orders).
    pub sell_station_id: String,
    /// System of the sell station.
    pub sell_system_id: String,
    /// Where the bot sends this item after hauling.
    pub dispose_to: ArbitrageDisposeTo,
    /// Volume-weighted average price paid per unit over the matched asks.
    pub buy_price: f64,
    /// Volume-weighted average price received per unit over the matched bids.
    pub sell_price: f64,
    /// Average profit per unit over the matched volume.
    pub profit_per_unit: f64,
    /// Cargo volume one unit of this item occupies (catalog `size`, ≥1).
    pub item_size: i64,
    /// Units that can be flipped profitably right now. Capped so the matched
    /// volume fits the cargo budget: `quantity * item_size <= max_units`.
    pub quantity: i64,
    /// Total credits gained by flipping the full matched volume.
    pub total_profit: i64,
    /// Credits required to buy the matched volume.
    pub capital_required: i64,
    /// Return on invested credits: `total_profit / capital_required`.
    pub roi: f64,
    /// Profit as a share of sell-side revenue. This is the price-drop cushion
    /// before the deal breaks even.
    pub gross_margin: f64,
    /// Destination buy-order depth at or above this deal's buy price, divided
    /// by planned sell quantity. Higher values mean demand is deeper above
    /// break-even if the top bids move before arrival.
    pub break_even_cover: f64,
    /// Coarse risk band derived from gross margin.
    pub risk_band: ArbitrageRiskBand,
    /// Jumps from the ship's current system to the buy station. Zero when
    /// the scan is scoped globally.
    pub jumps_to_buy: usize,
    /// Jumps from the buy station to the sell station.
    pub jumps_buy_to_sell: usize,
    /// Age of the deal's stalest market snapshot in seconds; `None` when a
    /// leg predates snapshot timestamps.
    pub data_age_seconds: Option<i64>,
    /// Cargo-constrained profit per jump, exponentially discounted by data age.
    /// Travel cost is total jumps clamped to at least one to keep zero-travel
    /// deals finite. Snapshots of unknown age score zero and rank by the profit
    /// tie-break.
    pub raw_score: f64,
    /// Score used for ranking. This is currently the same as [`Self::raw_score`];
    /// margin and break-even coverage are explicit filters instead of score
    /// multipliers.
    pub score: f64,
}

/// Passenger berth usage/capacity by class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassengerBerthUsage {
    pub economy: i64,
    pub business: i64,
    pub first: i64,
}

impl PassengerBerthUsage {
    pub fn available_from_state(state: &EconomyReadState) -> Self {
        Self {
            economy: (state.passengers.economy_berths.max
                - state.passengers.economy_berths.current)
                .max(0),
            business: (state.passengers.business_berths.max
                - state.passengers.business_berths.current)
                .max(0),
            first: (state.passengers.first_berths.max - state.passengers.first_berths.current)
                .max(0),
        }
    }

    pub fn capacity_from_state(state: &EconomyReadState) -> Self {
        Self {
            economy: state.passengers.economy_berths.max.max(0),
            business: state.passengers.business_berths.max.max(0),
            first: state.passengers.first_berths.max.max(0),
        }
    }

    pub fn add_berth_class(&mut self, berth_class: &str, units: i64) {
        match berth_class {
            "business" => self.business = self.business.saturating_add(units),
            "first" => self.first = self.first.saturating_add(units),
            _ => self.economy = self.economy.saturating_add(units),
        }
    }

    pub fn consumable_berth_class(&self, class_name: &str, units: i64) -> Option<&'static str> {
        match normalized_passenger_class(class_name).as_str() {
            "first" if self.first >= units => Some("first"),
            "business" if self.business >= units => Some("business"),
            "business" if self.first >= units => Some("first"),
            "economy" if self.economy >= units => Some("economy"),
            "economy" if self.business >= units => Some("business"),
            "economy" if self.first >= units => Some("first"),
            _ => None,
        }
    }

    pub fn consume(&mut self, class_name: &str, units: i64) -> Option<&'static str> {
        let berth_class = self.consumable_berth_class(class_name, units)?;
        match berth_class {
            "business" => self.business = self.business.saturating_sub(units),
            "first" => self.first = self.first.saturating_sub(units),
            _ => self.economy = self.economy.saturating_sub(units),
        }
        Some(berth_class)
    }
}

/// One waiting-passenger fare normalized into an arbitrage package member.
#[derive(Debug, Clone, PartialEq)]
pub struct PassengerFareDeal {
    pub citizen_id: String,
    pub name: String,
    pub class_name: String,
    pub origin_station_id: String,
    pub origin_system_id: String,
    pub destination_station_id: String,
    pub destination_system_id: Option<String>,
    pub estimated_fare: i64,
    pub base_fare: Option<i64>,
    pub speed_bonus: Option<i64>,
    pub berth_units: i64,
    pub total_jumps: usize,
    pub fare_per_jump: f64,
    pub score: f64,
    pub risk_band: &'static str,
}

/// First-class member inside an executable arbitrage package.
#[derive(Debug, Clone, PartialEq)]
pub enum ArbitradePackageMember {
    ItemDeal(ArbitrageDeal),
    PassengerFare(PassengerFareDeal),
}

/// Which member family anchored package scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbitradePackageAnchorKind {
    ItemDeal,
    PassengerFare,
}

impl ArbitradePackageAnchorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ItemDeal => "item_deal",
            Self::PassengerFare => "passenger_fare",
        }
    }
}

/// One executable one-hop arbitrage haul composed of multiple same-route deals.
#[derive(Debug, Clone, PartialEq)]
pub struct ArbitradePackage {
    /// Station to buy all package items from.
    pub buy_station_id: String,
    /// System of the buy station.
    pub buy_system_id: String,
    /// Station to sell all package items at.
    pub sell_station_id: String,
    /// System of the sell station.
    pub sell_system_id: String,
    /// Same-route deals in execution order.
    pub deals: Vec<ArbitrageDeal>,
    /// First-class package members in execution/scoring order.
    pub members: Vec<ArbitradePackageMember>,
    /// Same-route passenger fares selected for this package.
    pub passenger_fares: Vec<PassengerFareDeal>,
    /// Cargo volume used by the package.
    pub cargo_used: i64,
    /// Cargo volume budget used while building the package.
    pub cargo_capacity: i64,
    /// Credits required to buy the package.
    pub capital_required: i64,
    /// Total credits gained by flipping the package.
    pub total_profit: i64,
    /// Credits expected from selected passenger fares.
    pub passenger_revenue: i64,
    /// Passenger berths consumed by selected passenger fares.
    pub berth_used: PassengerBerthUsage,
    /// Total passenger berth capacity observed for the ship.
    pub berth_capacity: PassengerBerthUsage,
    /// Return on invested credits: `total_profit / capital_required`.
    pub roi: f64,
    /// Profit as a share of sell-side revenue.
    pub gross_margin: f64,
    /// Capital-weighted destination buy-order depth at or above each deal's
    /// buy price, divided by planned capital at risk.
    pub break_even_cover: f64,
    /// Coarse risk band derived from package gross margin.
    pub risk_band: ArbitrageRiskBand,
    /// Jumps from the ship's current system to the buy station.
    pub jumps_to_buy: usize,
    /// Jumps from the buy station to the sell station.
    pub jumps_buy_to_sell: usize,
    /// Age of the package's stalest market snapshot in seconds.
    pub data_age_seconds: Option<i64>,
    /// Cargo-constrained package profit per jump, discounted by data age.
    pub raw_score: f64,
    /// Package score used for ranking. This is currently the same as
    /// [`Self::raw_score`].
    pub score: f64,
    /// Member family that supplied the package anchor.
    pub anchor_kind: ArbitradePackageAnchorKind,
}

/// Minimum gross margin accepted for thin filler deals.
pub const DEFAULT_THIN_FILLER_MARGIN_FLOOR: f64 = 0.05;

/// Virtual faction sell orders are intentionally favored in arbitrage ranking
/// while still respecting their configured price floor.
pub const VIRTUAL_SOURCE_ARBITRAGE_SCORE_MULTIPLIER: f64 = 1.25;

/// Keep live arbitrage scans responsive even when market memory contains very
/// deep books across many stations. Candidates are pre-ranked per route before
/// greedy package assembly.
pub const MAX_PACKAGE_CANDIDATES_PER_ROUTE: usize = 256;
