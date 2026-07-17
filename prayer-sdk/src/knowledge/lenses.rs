//! Borrowed read models over one actor observation and shared world knowledge.

use super::*;

pub type InventoryLens = inventory::InventoryIndex;

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LensError {
    Unresolved(&'static str),
    Stale(&'static str),
    Inaccessible(&'static str),
    Unavailable(&'static str),
}

pub type WorldLens<'a> =
    prayer_state::WorldLens<'a, RuntimeVirtualMarketOrderDto, RuntimeVirtualCraftOrderDto>;

pub struct ActorLens<'a> {
    actor: &'a BotState,
}

impl<'a> ActorLens<'a> {
    pub fn new(actor: &'a BotState) -> Self {
        Self { actor }
    }

    pub fn state(&self) -> &'a BotState {
        self.actor
    }

    #[cfg(test)]
    pub fn current_poi(&self) -> Result<&'a str, LensError> {
        self.actor
            .location
            .poi_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(LensError::Unresolved("current_poi"))
    }
}

#[cfg(test)]
pub struct MarketLens<'a> {
    actor: &'a BotState,
    knowledge: &'a WorldState,
}

#[cfg(test)]
impl<'a> MarketLens<'a> {
    pub fn station(&self, station_id: &str) -> Result<&'a StationMarketData, LensError> {
        self.knowledge
            .station_markets
            .get(station_id)
            .ok_or(LensError::Unavailable("station_market"))
    }

    pub fn current_station(&self) -> Result<&'a StationMarketData, LensError> {
        let poi = ActorLens::new(self.actor).current_poi()?;
        if !self.actor.location.docked_at.is_some() {
            return Err(LensError::Inaccessible("current_station_market"));
        }
        self.station(poi)
    }
}

#[cfg(test)]
pub struct WildlifeLens<'a> {
    knowledge: &'a WorldState,
}

pub struct SocialLens<'a> {
    knowledge: &'a WorldState,
}

impl<'a> SocialLens<'a> {
    pub fn new(knowledge: &'a WorldState) -> Self {
        Self { knowledge }
    }

    pub fn sightings(&self) -> impl Iterator<Item = &'a AgentSightingData> {
        self.knowledge.agent_sightings.values()
    }
}

#[cfg(test)]
impl<'a> WildlifeLens<'a> {
    pub fn at_poi(&self, poi: &str) -> Result<&'a WildlifePoiSnapshotData, LensError> {
        self.knowledge
            .wildlife_by_poi
            .get(poi)
            .ok_or(LensError::Unavailable("wildlife"))
    }
}

#[cfg(test)]
pub struct SalvageLens<'a> {
    knowledge: &'a WorldState,
    metadata: prayer_runtime::knowledge::WorldRuntimeMetadata,
}

#[cfg(test)]
impl<'a> SalvageLens<'a> {
    pub fn at_poi(&self, poi: &str) -> Result<&'a SalvageData, LensError> {
        let snapshot = self
            .knowledge
            .salvage_by_poi
            .get(poi)
            .ok_or(LensError::Unavailable("salvage"))?;
        if !salvage_snapshot_fresh(&self.metadata, poi) {
            return Err(LensError::Stale("salvage"));
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
pub struct ActorWorldLens<'a> {
    pub market: MarketLens<'a>,
    pub wildlife: WildlifeLens<'a>,
    pub salvage: SalvageLens<'a>,
}

#[cfg(test)]
impl<'a> ActorWorldLens<'a> {
    pub fn new(actor: &'a BotState, knowledge: &'a WorldState) -> Self {
        Self {
            market: MarketLens { actor, knowledge },
            wildlife: WildlifeLens { knowledge },
            salvage: SalvageLens {
                knowledge,
                metadata: Default::default(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_lens_borrows_the_canonical_galaxy() {
        let knowledge = WorldState::default();
        let lens = WorldLens::new(&knowledge);
        assert!(std::ptr::eq(lens.galaxy(), knowledge.galaxy.as_ref()));
    }

    #[test]
    fn focused_lenses_report_typed_absence_and_access() {
        let actor = BotState {
            location: spacemolt_lib_rs::schema::V2GameStateLocation {
                system_id: None,
                poi_id: Some("station".to_string()),
                docked_at: None,
                ..Default::default()
            },
            ..BotState::default()
        };
        let knowledge = WorldState::default();
        let lens = ActorWorldLens::new(&actor, &knowledge);
        assert_eq!(
            lens.market.current_station(),
            Err(LensError::Inaccessible("current_station_market"))
        );
        assert_eq!(
            lens.salvage.at_poi("station"),
            Err(LensError::Unavailable("salvage"))
        );
        assert_eq!(
            lens.wildlife.at_poi("station"),
            Err(LensError::Unavailable("wildlife"))
        );
    }
}
