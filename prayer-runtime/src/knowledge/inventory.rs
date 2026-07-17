use std::ops::{BitOr, BitOrAssign};

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InventoryLotId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryOwner {
    Player {
        canonical_id: String,
        display_name: Option<String>,
    },
    Faction {
        faction_id: String,
    },
    Market,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryLocation {
    pub poi_id: String,
    pub system_id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventorySource {
    Cargo {
        session_id: Uuid,
    },
    PersonalStorage,
    FactionStorage,
    MarketAsk {
        price_each: i64,
        order_source: Option<String>,
    },
    MarketBid,
    Passenger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryProvenance {
    Live,
    Remembered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryObservation {
    pub provenance: InventoryProvenance,
    pub observed_at_unix: Option<i64>,
    pub state_version: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryLot {
    pub id: InventoryLotId,
    pub item_id: String,
    pub quantity: i64,
    pub owner: InventoryOwner,
    pub location: InventoryLocation,
    pub source: InventorySource,
    pub observation: InventoryObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryFreshnessPolicy {
    IncludeRemembered,
    LiveOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryAvailabilityReason {
    RememberedInventory,
    FullyReserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryAvailability {
    pub observed: i64,
    pub reserved: i64,
    pub available: i64,
    pub executable: bool,
    pub reason: Option<InventoryAvailabilityReason>,
}

#[derive(Debug, Clone, Copy)]
pub struct InventoryCostPolicy {
    personal_storage_unit_cost: f64,
    faction_storage_unit_cost: f64,
}

impl InventoryCostPolicy {
    pub const fn parity() -> Self {
        Self {
            personal_storage_unit_cost: 1.0,
            faction_storage_unit_cost: 1.0,
        }
    }
    pub const fn personal_storage_unit_cost(self) -> f64 {
        self.personal_storage_unit_cost
    }
    pub const fn faction_storage_unit_cost(self) -> f64 {
        self.faction_storage_unit_cost
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventorySourceMask(pub u8);

impl InventorySourceMask {
    pub const CARGO: Self = Self(1);
    pub const PERSONAL_STORAGE: Self = Self(2);
    pub const FACTION_STORAGE: Self = Self(4);
    pub const MARKET: Self = Self(8);
    pub const MARKET_BID: Self = Self(16);
    pub const PASSENGER: Self = Self(32);
    pub const ALL: Self = Self(63);
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
    pub const fn has_only_known_sources(self) -> bool {
        self.0 & !Self::ALL.0 == 0
    }
}

impl BitOr for InventorySourceMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl BitOrAssign for InventorySourceMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryOwnerSelector<'a> {
    Player(&'a str),
    Faction(&'a str),
    Market,
}

pub struct InventoryQuery<'a> {
    pub item_id: Option<&'a str>,
    pub owner: Option<InventoryOwnerSelector<'a>>,
    pub location: Option<&'a str>,
    pub sources: InventorySourceMask,
    pub freshness: InventoryFreshnessPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryQueryError {
    EmptySourceMask,
    UnresolvedLocation(String),
}

pub fn inventory_availability(
    lot: &InventoryLot,
    reserved: i64,
    freshness: InventoryFreshnessPolicy,
) -> InventoryAvailability {
    let reserved = reserved.max(0).min(lot.quantity.max(0));
    let available = lot.quantity.max(0).saturating_sub(reserved);
    let stale = freshness == InventoryFreshnessPolicy::LiveOnly
        && lot.observation.provenance != InventoryProvenance::Live;
    InventoryAvailability {
        observed: lot.quantity,
        reserved,
        available: if stale { 0 } else { available },
        executable: !stale && available > 0,
        reason: if stale {
            Some(InventoryAvailabilityReason::RememberedInventory)
        } else if available == 0 {
            Some(InventoryAvailabilityReason::FullyReserved)
        } else {
            None
        },
    }
}
