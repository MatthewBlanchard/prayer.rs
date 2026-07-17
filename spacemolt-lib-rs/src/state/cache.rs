//! Per-account local cache of the eight game-state sections.

use std::collections::HashMap;
use std::time::SystemTime;

use serde_json::Value;

use crate::protocol::{GameState, StateDelta, StateSection};
use crate::schema::{
    V2GameStateCargoItem, V2GameStateLocation, V2GameStateMissions, V2GameStateModulesItem,
    V2GameStatePlayer, V2GameStateQueue, V2GameStateShip, V2GameStateSkillsValue,
};

/// Per-account cache seeded by `get_status` and updated by action deltas.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateCache {
    state: HashMap<StateSection, Value>,
    freshness: HashMap<StateSection, SectionFreshness>,
}

/// Reconciliation metadata for one cached section.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionFreshness {
    pub fresh_at: Option<SystemTime>,
    pub dirty_since: Option<SystemTime>,
    pub source_query: Option<String>,
}

impl StateCache {
    /// Replace the cache from a canonical full snapshot.
    pub fn seed(&mut self, snapshot: &GameState) -> Vec<StateSection> {
        let mut next = HashMap::new();
        let mut changed = Vec::new();

        for section in StateSection::ALL {
            if let Some(value) = snapshot.get(section.as_str()) {
                next.insert(section, value.clone());
                changed.push(section);
            }
        }

        self.state = next;
        let now = SystemTime::now();
        for section in changed.iter().copied() {
            self.mark_fresh(section, "spacemolt/get_status", now);
        }
        changed
    }

    /// Replace exactly one section from an authoritative lean query.
    pub fn replace_section(
        &mut self,
        section: StateSection,
        value: Value,
        source: &str,
    ) -> Vec<StateSection> {
        self.state.insert(section, value);
        self.mark_fresh(section, source, SystemTime::now());
        vec![section]
    }

    pub fn mark_dirty(&mut self, section: StateSection) {
        let entry = self.freshness.entry(section).or_default();
        entry.dirty_since.get_or_insert_with(SystemTime::now);
    }

    pub fn freshness(&self, section: StateSection) -> Option<&SectionFreshness> {
        self.freshness.get(&section)
    }

    fn mark_fresh(&mut self, section: StateSection, source: &str, at: SystemTime) {
        self.freshness.insert(
            section,
            SectionFreshness {
                fresh_at: Some(at),
                dirty_since: None,
                source_query: Some(source.to_string()),
            },
        );
    }

    /// Apply a delta, replacing each present section.
    pub fn apply_delta(&mut self, delta: &StateDelta) -> Vec<StateSection> {
        let mut changed = Vec::new();

        for section in StateSection::ALL {
            if let Some(value) = delta.get(section.as_str()) {
                self.state.insert(section, value.clone());
                changed.push(section);
            }
        }

        changed
    }

    /// Merge a partial patch into a single section in place.
    pub fn patch_section(
        &mut self,
        section: StateSection,
        patch: serde_json::Map<String, Value>,
    ) -> Vec<StateSection> {
        let Some(current) = self.state.get_mut(&section) else {
            return Vec::new();
        };
        let Some(current_obj) = current.as_object_mut() else {
            return Vec::new();
        };
        for (key, value) in patch {
            current_obj.insert(key, value);
        }
        vec![section]
    }

    /// Live view of the cached state.
    pub(crate) fn raw_snapshot(&self) -> &HashMap<StateSection, Value> {
        &self.state
    }

    /// Number of game-state sections currently cached.
    pub fn section_count(&self) -> usize {
        self.state.len()
    }

    /// Get one cached state section.
    pub(crate) fn raw_section(&self, section: StateSection) -> Option<&Value> {
        self.state.get(&section)
    }

    pub(crate) fn raw_ship(&self) -> Option<&Value> {
        self.raw_section(StateSection::Ship)
    }

    pub(crate) fn raw_cargo(&self) -> Option<&Value> {
        self.raw_section(StateSection::Cargo)
    }

    /// Deserialize the cached player fact into its canonical generated type.
    pub fn player(&self) -> Result<Option<V2GameStatePlayer>, serde_json::Error> {
        self.typed_section(StateSection::Player)
    }

    /// Deserialize the cached ship fact into its canonical generated type.
    pub fn ship(&self) -> Result<Option<V2GameStateShip>, serde_json::Error> {
        self.typed_section(StateSection::Ship)
    }

    /// Deserialize the cached location fact into its canonical generated type.
    pub fn location(&self) -> Result<Option<V2GameStateLocation>, serde_json::Error> {
        self.typed_section(StateSection::Location)
    }

    /// Deserialize the cached cargo facts into their canonical generated type.
    pub fn cargo(&self) -> Result<Option<Vec<V2GameStateCargoItem>>, serde_json::Error> {
        self.typed_section(StateSection::Cargo)
    }

    /// Deserialize the cached module facts into their canonical generated type.
    pub fn modules(&self) -> Result<Option<Vec<V2GameStateModulesItem>>, serde_json::Error> {
        self.typed_section(StateSection::Modules)
    }

    /// Deserialize the cached mission facts into their canonical generated type.
    pub fn missions(&self) -> Result<Option<V2GameStateMissions>, serde_json::Error> {
        self.typed_section(StateSection::Missions)
    }

    /// Deserialize the cached skill facts into their canonical generated type.
    pub fn skills(
        &self,
    ) -> Result<Option<HashMap<String, V2GameStateSkillsValue>>, serde_json::Error> {
        self.typed_section(StateSection::Skills)
    }

    /// Deserialize the cached action queue into its canonical generated type.
    pub fn queue(&self) -> Result<Option<V2GameStateQueue>, serde_json::Error> {
        self.typed_section(StateSection::Queue)
    }

    fn typed_section<T: serde::de::DeserializeOwned>(
        &self,
        section: StateSection,
    ) -> Result<Option<T>, serde_json::Error> {
        self.raw_section(section)
            .cloned()
            .map(serde_json::from_value)
            .transpose()
    }

    /// True when a tick-deferred action is queued for this account.
    pub fn has_pending_action(&self) -> bool {
        self.raw_section(StateSection::Queue)
            .and_then(|queue| queue.get("has_pending"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Current credit balance, when the player section is seeded.
    pub fn credits(&self) -> Option<i64> {
        self.player()
            .ok()
            .flatten()
            .and_then(|player| player.credits)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn seed_replaces_cache_and_reports_sections() {
        let mut cache = StateCache::default();
        let changed = cache.seed(&json!({
            "player": { "credits": 42 },
            "ship": { "id": "ship_1" },
            "queue": { "has_pending": true },
            "ignored": true
        }));

        assert_eq!(
            changed,
            vec![
                StateSection::Player,
                StateSection::Ship,
                StateSection::Queue
            ]
        );
        assert_eq!(cache.credits(), Some(42));
        assert!(cache.has_pending_action());
        assert!(cache.raw_section(StateSection::Cargo).is_none());
    }

    #[test]
    fn apply_delta_replaces_present_sections_only() {
        let mut cache = StateCache::default();
        cache.seed(&json!({
            "player": { "credits": 1 },
            "ship": { "id": "old" }
        }));

        let changed = cache.apply_delta(&json!({
            "ship": { "id": "new" }
        }));

        assert_eq!(changed, vec![StateSection::Ship]);
        assert_eq!(cache.credits(), Some(1));
        assert_eq!(
            cache
                .ship()
                .expect("valid ship")
                .expect("ship")
                .id
                .as_deref(),
            Some("new")
        );
    }

    #[test]
    fn authoritative_partial_refresh_keeps_unrelated_sections_and_clears_dirty() {
        let mut cache = StateCache::default();
        cache.seed(&json!({ "cargo": [{"item_id":"ore","quantity":5}], "ship": {"cargo_used":5} }));
        cache.mark_dirty(StateSection::Cargo);
        cache.replace_section(StateSection::Cargo, json!([]), "spacemolt/get_cargo");
        assert!(cache
            .cargo()
            .expect("valid cargo")
            .expect("cargo")
            .is_empty());
        assert_eq!(
            cache.ship().expect("valid ship").expect("ship").cargo_used,
            Some(5)
        );
        let freshness = cache.freshness(StateSection::Cargo).expect("freshness");
        assert!(freshness.dirty_since.is_none());
        assert_eq!(
            freshness.source_query.as_deref(),
            Some("spacemolt/get_cargo")
        );
    }

    #[test]
    fn generated_accessors_traverse_facts_without_json_pointers() {
        let mut cache = StateCache::default();
        cache.seed(&json!({
            "player": { "id": "p1", "credits": 42 },
            "ship": { "id": "s1", "fuel": 7, "max_fuel": 10 },
            "cargo": [{ "item_id": "ore", "quantity": 5 }]
        }));

        let player = cache.player().expect("valid player").expect("player");
        let ship = cache.ship().expect("valid ship").expect("ship");
        let cargo = cache.cargo().expect("valid cargo").expect("cargo");
        assert_eq!(player.credits, Some(42));
        assert_eq!(ship.fuel, Some(7));
        assert_eq!(cargo[0].item_id.as_deref(), Some("ore"));
    }
}
