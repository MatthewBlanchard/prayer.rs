//! Shared navigation target selection helpers.

use crate::engine::ActiveCommandState;
use crate::read_context::{ExecutionReadContext, PlanningState};
use crate::state::GalaxyData;
use prayer_actions::ActionArg;

pub trait NavigationState {
    fn system(&self) -> Option<&str>;
    fn current_poi(&self) -> Option<&str>;
    fn nearest_station(&self) -> Option<&str>;
    fn home_poi(&self) -> Option<&str>;
    fn home_base(&self) -> Option<&str>;
    fn galaxy(&self) -> &GalaxyData;

    fn effective_poi_system_id(&self, poi_id: &str) -> Option<&str> {
        self.galaxy()
            .poi_records
            .get(poi_id)
            .map(|poi| poi.system_id.as_str())
            .or_else(|| {
                (self.current_poi() == Some(poi_id))
                    .then(|| self.system())
                    .flatten()
            })
    }
}

macro_rules! impl_navigation_state {
    ($ty:ty) => {
        impl NavigationState for $ty {
            fn system(&self) -> Option<&str> {
                self.system.as_deref()
            }
            fn current_poi(&self) -> Option<&str> {
                self.current_poi.as_deref()
            }
            fn nearest_station(&self) -> Option<&str> {
                self.nearest_station.as_deref()
            }
            fn home_poi(&self) -> Option<&str> {
                self.home_poi.as_deref()
            }
            fn home_base(&self) -> Option<&str> {
                self.home_base.as_deref()
            }
            fn galaxy(&self) -> &GalaxyData {
                &self.galaxy
            }
        }
    };
}

impl_navigation_state!(PlanningState);

impl NavigationState for ExecutionReadContext<'_> {
    fn system(&self) -> Option<&str> {
        self.bot.location.system_id.as_deref()
    }
    fn current_poi(&self) -> Option<&str> {
        self.bot.location.poi_id.as_deref()
    }
    fn nearest_station(&self) -> Option<&str> {
        self.world.nearest_station.as_deref()
    }
    fn home_poi(&self) -> Option<&str> {
        self.bot.player.home_poi.as_deref()
    }
    fn home_base(&self) -> Option<&str> {
        self.bot.player.home_base.as_deref()
    }
    fn galaxy(&self) -> &GalaxyData {
        &self.world.galaxy
    }
}

/// Resolved navigation target for a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationTarget {
    /// Display/source label for the target.
    pub label: String,
    /// Target system id.
    pub system: String,
    /// Target POI id, when navigating to a POI.
    pub poi: Option<String>,
}

/// Resolve the current navigation target for an active command.
pub fn active_command_navigation_target(
    state: &impl NavigationState,
    active_command: &ActiveCommandState,
) -> Option<NavigationTarget> {
    match active_command {
        ActiveCommandState::Go(go) => resolve_go_target(state, go.target.as_str()).ok(),
        ActiveCommandState::Mine(mine) => {
            let poi = nearest_mining_poi(state, mine.resource.as_deref(), |_| true)?;
            poi_navigation_target(state, poi.clone().as_str(), poi)
        }
        ActiveCommandState::Refuel(refuel) => {
            let (system, poi) = refuel
                .target_system
                .clone()
                .zip(refuel.target_poi.clone())
                .or_else(|| nearest_refuel_station(state))?;
            Some(NavigationTarget {
                label: poi.clone(),
                system,
                poi: Some(poi),
            })
        }
        ActiveCommandState::Find(_) => nearest_find_navigation_target(state),
        // Waiting has no navigation target.
        ActiveCommandState::Wait(_) => None,
    }
}

/// Resolve a `go` target token into a navigation target.
pub fn resolve_go_target(
    state: &impl NavigationState,
    target: &str,
) -> Result<NavigationTarget, String> {
    let resolved = target;

    if state.galaxy().system_records.contains_key(resolved) {
        return Ok(NavigationTarget {
            label: target.to_string(),
            system: resolved.to_string(),
            poi: None,
        });
    }

    let poi_id = resolve_poi_id(state, resolved)
        .ok_or_else(|| format!("Unknown destination: '{resolved}'."))?;
    let system = state
        .galaxy()
        .poi_records
        .get(poi_id.as_str())
        .map(|poi| poi.system_id.clone())
        .or_else(|| state.system().map(str::to_string))
        .unwrap_or_else(|| resolved.to_string());

    Ok(NavigationTarget {
        label: target.to_string(),
        system,
        poi: Some(poi_id),
    })
}

/// Return the nearest station POI, avoiding unsafe station systems.
pub fn nearest_station_poi(state: &impl NavigationState) -> Option<String> {
    state
        .nearest_station()
        .filter(|station| !is_unsafe_refuel_station(state, station))
        .map(str::to_string)
        .or_else(|| nearest_refuel_station(state).map(|(_, poi)| poi))
}

/// Return the nearest known mining POI.
pub fn nearest_mining_poi(
    state: &impl NavigationState,
    resource: Option<&str>,
    poi_filter: impl Fn(&str) -> bool,
) -> Option<String> {
    let candidates = if let Some(resource_id) = resource {
        state
            .galaxy()
            .poi_records
            .values()
            .filter(|poi| {
                poi.resources
                    .iter()
                    .any(|row| row.resource_id.eq_ignore_ascii_case(resource_id))
            })
            .map(|poi| poi.id.clone())
            .collect()
    } else {
        let typed_mineable = state
            .galaxy()
            .poi_records
            .values()
            .filter_map(|poi| {
                if is_mineable_poi_type(poi.info.poi_type.as_str()) {
                    Some(poi.id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if typed_mineable.is_empty() {
            state.galaxy().poi_records.keys().cloned().collect()
        } else {
            typed_mineable
        }
    };

    if candidates.is_empty() {
        return None;
    }

    let current_system = state.system().map(str::to_string).unwrap_or_default();
    candidates
        .into_iter()
        .filter(|poi| poi_filter(poi.as_str()))
        .filter_map(|poi| {
            let poi_system = state
                .galaxy()
                .poi_records
                .get(poi.as_str())
                .map(|poi| poi.system_id.clone())
                .unwrap_or_else(|| current_system.clone());
            let distance = state.galaxy().hop_distance(&current_system, &poi_system)?;
            Some((poi, distance))
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(poi, _)| poi)
}

/// Return the nearest known refuel station as `(system_id, poi_id)`.
pub fn nearest_refuel_station(state: &impl NavigationState) -> Option<(String, String)> {
    let current_system = state.system()?;

    if let Some(current_poi) = state.current_poi() {
        if state
            .galaxy()
            .poi_records
            .get(current_poi)
            .is_some_and(|poi| {
                poi.system_id == current_system && poi.info.poi_type.eq_ignore_ascii_case("station")
            })
            && is_empire_station(state, current_poi)
            && !is_unsafe_refuel_station(state, current_poi)
        {
            return Some((current_system.to_string(), current_poi.to_string()));
        }
    }

    let mut best: Option<(usize, String, String)> = None;
    for poi in state
        .galaxy()
        .poi_records
        .values()
        .filter(|poi| poi.info.poi_type.eq_ignore_ascii_case("station"))
    {
        let system_id = &poi.system_id;
        let poi_id = &poi.id;
        if !is_empire_station(state, poi_id) || is_unsafe_refuel_station(state, poi_id) {
            continue;
        }
        // Rank stations by the same stronghold-aware cost used by
        // `next_hop_toward`. Mixing naked hop counts with weighted movement
        // can change the winner while following the selected safe route.
        let Some(distance) = state.galaxy().path_cost(current_system, system_id) else {
            continue;
        };
        let candidate = (distance, system_id.clone(), poi_id.clone());
        match &best {
            None => best = Some(candidate),
            Some(existing) if candidate < *existing => best = Some(candidate),
            _ => {}
        }
    }
    best.map(|(_, system, poi)| (system, poi))
}

/// Return the next target for exploratory `find`.
pub fn nearest_find_navigation_target(state: &impl NavigationState) -> Option<NavigationTarget> {
    let current_system = state.system()?;

    if let Some(poi) = nearest_unvisited_poi_in_system(state, current_system) {
        return poi_navigation_target(state, poi.clone().as_str(), poi);
    }

    ordered_find_target_systems(state, current_system)
        .into_iter()
        .find(|target_system| {
            target_system == current_system
                || state
                    .galaxy()
                    .next_hop_toward(current_system, target_system.as_str())
                    .is_some()
        })
        .map(|system| NavigationTarget {
            label: system.clone(),
            system,
            poi: None,
        })
}

/// Return ordered target systems for exploratory `find`.
pub fn ordered_find_target_systems(
    state: &impl NavigationState,
    current_system: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    if nearest_unvisited_poi_in_system(state, current_system).is_some() {
        out.push(current_system.to_string());
    }

    let mut candidates = state
        .galaxy()
        .system_records
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.sort_by_key(|system_id| {
        state
            .galaxy()
            .hop_distance(current_system, system_id)
            .unwrap_or(usize::MAX / 2)
    });

    for system_id in candidates {
        if system_id == current_system {
            continue;
        }
        if nearest_unvisited_poi_in_system(state, system_id.as_str()).is_some()
            || !state
                .galaxy()
                .system_records
                .get(system_id.as_str())
                .is_some_and(|system| system.first_entered_unix.is_some())
        {
            out.push(system_id);
        }
    }

    out
}

/// Return the nearest unvisited POI in a system.
pub fn nearest_unvisited_poi_in_system(
    state: &impl NavigationState,
    system_id: &str,
) -> Option<String> {
    let mut candidates = state
        .galaxy()
        .poi_records
        .values()
        .filter_map(|poi| {
            if poi.system_id == system_id && poi.first_visited_unix.is_none() {
                Some(poi.id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}

fn resolve_poi_id(state: &impl NavigationState, value: &str) -> Option<String> {
    if state.galaxy().poi_records.contains_key(value) {
        return Some(value.to_string());
    }
    state
        .galaxy()
        .poi_records
        .values()
        .find(|poi| poi.info.base_id.as_deref() == Some(value))
        .map(|poi| poi.id.clone())
}

fn poi_navigation_target(
    state: &impl NavigationState,
    label: &str,
    poi_id: String,
) -> Option<NavigationTarget> {
    let system = state
        .galaxy()
        .poi_records
        .get(poi_id.as_str())
        .map(|poi| poi.system_id.clone())
        .or_else(|| state.system().map(str::to_string))?;
    Some(NavigationTarget {
        label: label.to_string(),
        system,
        poi: Some(poi_id),
    })
}

fn is_mineable_poi_type(poi_type: &str) -> bool {
    matches!(
        poi_type.to_ascii_lowercase().as_str(),
        "asteroid_belt" | "asteroid_field" | "asteroid_cluster" | "asteroid"
    )
}

fn is_unsafe_refuel_station(state: &impl NavigationState, poi_id: &str) -> bool {
    is_stronghold_station(state, poi_id) || is_pirate_station(state, poi_id)
}

fn is_empire_station(state: &impl NavigationState, poi_id: &str) -> bool {
    state
        .effective_poi_system_id(poi_id)
        .and_then(|system_id| state.galaxy().system_records.get(system_id))
        .and_then(|system| system.empire.as_deref())
        .is_some_and(|empire| !empire.trim().is_empty())
}

fn is_stronghold_station(state: &impl NavigationState, poi_id: &str) -> bool {
    state
        .effective_poi_system_id(poi_id)
        .is_some_and(|system_id| {
            state
                .galaxy()
                .system_records
                .get(system_id)
                .is_some_and(|system| system.is_stronghold)
        })
}

fn is_pirate_station(state: &impl NavigationState, poi_id: &str) -> bool {
    let Some(system_id) = state.effective_poi_system_id(poi_id) else {
        return false;
    };
    if state
        .galaxy()
        .system_records
        .get(system_id)
        .and_then(|system| system.empire.as_deref())
        .is_some_and(is_pirate_label)
    {
        return true;
    }

    state.galaxy().poi_records.get(poi_id).is_some_and(|poi| {
        let info = &poi.info;
        is_pirate_label(&info.name)
            || is_pirate_label(&info.base_name.clone().unwrap_or_default())
            || is_pirate_label(&info.class_name)
            || is_pirate_label(&info.description)
    })
}

fn is_pirate_label(value: &str) -> bool {
    value
        .trim()
        .to_ascii_lowercase()
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|part| matches!(part, "pirate" | "pirates"))
}

/// Convert an active command's first textual argument into a string.
pub fn active_command_first_arg(args: &[ActionArg]) -> Option<String> {
    args.first().map(ActionArg::as_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct NavFixture {
        bot: crate::BotState,
        world: crate::read_context::WorldReadState,
        runtime: crate::read_context::ExecutionRuntimeState,
    }

    impl NavFixture {
        fn context(&self) -> ExecutionReadContext<'_> {
            ExecutionReadContext {
                bot: &self.bot,
                world: &self.world,
                runtime: &self.runtime,
            }
        }
    }

    fn nav_fixture(
        bot: crate::BotState,
        nearest_station: Option<&str>,
        galaxy: crate::GalaxyData,
    ) -> NavFixture {
        NavFixture {
            bot,
            world: crate::read_context::WorldReadState {
                nearest_station: nearest_station.map(str::to_string),
                galaxy: std::sync::Arc::new(galaxy),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn location(
        system: &str,
        poi: Option<&str>,
        docked: bool,
    ) -> spacemolt_lib_rs::schema::V2GameStateLocation {
        spacemolt_lib_rs::schema::V2GameStateLocation {
            system_id: Some(system.to_string()),
            poi_id: poi.map(str::to_string),
            docked_at: docked.then(|| poi.unwrap_or_default().to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn resolve_go_target_errors_when_target_unknown() {
        let state = NavFixture::default();
        let err =
            resolve_go_target(&state.context(), "missing_target").expect_err("expected error");
        assert!(err.to_string().contains("Unknown destination"));
    }

    #[test]
    fn nearest_refuel_station_uses_the_same_weighted_cost_as_navigation() {
        let system =
            |id: &str, connections: &[&str], is_stronghold: bool| crate::state::SystemKnowledge {
                id: id.to_string(),
                connections: connections.iter().map(|value| value.to_string()).collect(),
                empire: Some("solarian".into()),
                is_stronghold,
                ..Default::default()
            };
        let station = |id: &str, system_id: &str| crate::state::PoiKnowledge {
            id: id.to_string(),
            system_id: system_id.to_string(),
            info: crate::state::PoiInfoData {
                poi_type: "station".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let galaxy = crate::GalaxyData {
            system_records: std::collections::HashMap::from([
                ("start".into(), system("start", &["penalty", "q"], false)),
                ("penalty".into(), system("penalty", &["near"], false)),
                ("near".into(), system("near", &[], false)),
                ("q".into(), system("q", &["r"], false)),
                ("r".into(), system("r", &["far"], false)),
                ("far".into(), system("far", &[], false)),
                // These directed edges mark penalty and near as within the
                // stronghold safety radius without making the station itself
                // a stronghold station.
                ("hostile".into(), system("hostile", &["penalty"], true)),
            ]),
            poi_records: std::collections::HashMap::from([
                ("near_station".into(), station("near_station", "near")),
                ("far_station".into(), station("far_station", "far")),
            ]),
            ..Default::default()
        };
        let mut bot = crate::BotState::default();
        bot.location = location("start", Some("field"), false);
        let state = nav_fixture(bot, None, galaxy);

        // near_station is two naked hops away, but its safe route costs four;
        // far_station is three safe hops away and is the route navigation uses.
        assert_eq!(
            nearest_refuel_station(&state.context()),
            Some(("far".into(), "far_station".into()))
        );
        assert_eq!(
            state.world.galaxy.next_hop_toward("start", "far"),
            Some("q".into())
        );
    }

    #[test]
    fn nearest_refuel_station_skips_unreachable_candidates() {
        let mut bot = crate::BotState::default();
        bot.location = location("start", Some("field"), false);
        let galaxy = crate::GalaxyData {
            system_records: std::collections::HashMap::from([
                (
                    "start".into(),
                    crate::state::SystemKnowledge {
                        id: "start".into(),
                        connections: vec!["reachable".into()],
                        empire: Some("solarian".into()),
                        ..Default::default()
                    },
                ),
                (
                    "reachable".into(),
                    crate::state::SystemKnowledge {
                        id: "reachable".into(),
                        empire: Some("solarian".into()),
                        ..Default::default()
                    },
                ),
                (
                    "isolated".into(),
                    crate::state::SystemKnowledge {
                        id: "isolated".into(),
                        empire: Some("solarian".into()),
                        ..Default::default()
                    },
                ),
            ]),
            poi_records: std::collections::HashMap::from([
                (
                    "reachable_station".into(),
                    crate::state::PoiKnowledge {
                        id: "reachable_station".into(),
                        system_id: "reachable".into(),
                        info: crate::state::PoiInfoData {
                            poi_type: "station".into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
                (
                    "isolated_station".into(),
                    crate::state::PoiKnowledge {
                        id: "isolated_station".into(),
                        system_id: "isolated".into(),
                        info: crate::state::PoiInfoData {
                            poi_type: "station".into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        };
        let state = nav_fixture(bot, None, galaxy);

        assert_eq!(
            nearest_refuel_station(&state.context()),
            Some(("reachable".into(), "reachable_station".into()))
        );
    }

    #[test]
    fn nearest_refuel_station_skips_closer_non_empire_station() {
        let system =
            |id: &str, connections: &[&str], empire: Option<&str>| crate::state::SystemKnowledge {
                id: id.into(),
                connections: connections.iter().map(|value| (*value).into()).collect(),
                empire: empire.map(str::to_string),
                ..Default::default()
            };
        let station = |id: &str, system_id: &str| crate::state::PoiKnowledge {
            id: id.into(),
            system_id: system_id.into(),
            info: crate::state::PoiInfoData {
                poi_type: "station".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let galaxy = crate::GalaxyData {
            system_records: std::collections::HashMap::from([
                ("start".into(), system("start", &["neutral"], None)),
                ("neutral".into(), system("neutral", &["empire"], None)),
                ("empire".into(), system("empire", &[], Some("solarian"))),
            ]),
            poi_records: std::collections::HashMap::from([
                (
                    "neutral_station".into(),
                    station("neutral_station", "neutral"),
                ),
                ("empire_station".into(), station("empire_station", "empire")),
            ]),
            ..Default::default()
        };
        let mut bot = crate::BotState::default();
        bot.location = location("start", Some("field"), false);
        let state = nav_fixture(bot, None, galaxy);

        assert_eq!(
            nearest_refuel_station(&state.context()),
            Some(("empire".into(), "empire_station".into()))
        );
    }
}
