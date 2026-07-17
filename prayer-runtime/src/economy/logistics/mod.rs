use super::arbitrage::*;

/// How a logistics haul obtains cargo.
pub type LogisticsSource = ArbitrageAcquireFrom;

/// How a logistics haul disposes of cargo.
pub type LogisticsDestination = ArbitrageDisposeTo;

/// One logistics item moved between a source sell book and destination buy book.
#[derive(Debug, Clone, PartialEq)]
pub struct LogisticsItem {
    pub item_id: String,
    pub quantity: i64,
    pub item_size: i64,
    pub source_price: f64,
    pub destination_price: f64,
    pub source: LogisticsSource,
    pub destination: LogisticsDestination,
    pub priority: f64,
    pub value_per_unit: f64,
    pub route_value: f64,
    pub score: f64,
}

/// One executable one-hop logistics haul.
#[derive(Debug, Clone, PartialEq)]
pub struct LogisticsPackage {
    pub source_station_id: String,
    pub source_system_id: String,
    pub destination_station_id: String,
    pub destination_system_id: String,
    pub items: Vec<LogisticsItem>,
    pub cargo_used: i64,
    pub cargo_capacity: i64,
    pub jumps_to_source: usize,
    pub jumps_source_to_destination: usize,
    pub total_jumps: usize,
    pub score: f64,
}
