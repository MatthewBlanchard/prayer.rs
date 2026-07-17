use super::{RuntimeVirtualCraftOrderDto, RuntimeVirtualMarketOrderDto};

pub use prayer_state::FactionTreasuryInfo;

pub type WorldState =
    prayer_state::WorldState<RuntimeVirtualMarketOrderDto, RuntimeVirtualCraftOrderDto>;
