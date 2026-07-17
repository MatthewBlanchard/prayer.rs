//! Transport-neutral facility snapshots and station identity mechanics.

use std::collections::BTreeSet;

use crate::economy::EconomyReadState;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FacilitySnapshot {
    pub current: Option<spacemolt_lib_rs::schema::FacilityResponse>,
    pub faction_current: Option<spacemolt_lib_rs::schema::FacilityResponse>,
}

pub trait FacilitySnapshotSource {
    fn facility_snapshot(&self, poi_id: &str) -> Option<FacilitySnapshot>;
}

pub trait QuartermasterPlanningSource: FacilitySnapshotSource {
    fn faction_storage_quantity(
        &self,
        state: &EconomyReadState,
        station_id: &str,
        item_id: &str,
    ) -> i64;
}

pub fn station_poi_id<'a>(state: &'a EconomyReadState, station_id: &'a str) -> Option<&'a str> {
    let station_id = station_id.trim();
    if station_id.is_empty() {
        return None;
    }
    if state.effective_poi_system_id(station_id).is_some() {
        return Some(station_id);
    }
    state
        .galaxy
        .poi_records
        .values()
        .find(|poi| poi.info.base_id.as_deref() == Some(station_id))
        .map(|poi| poi.id.as_str())
}

pub fn same_station_or_base(state: &EconomyReadState, left: &str, right: &str) -> bool {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if left == right {
        return true;
    }
    match (station_poi_id(state, left), station_poi_id(state, right)) {
        (Some(left_poi), Some(right_poi)) => left_poi == right_poi,
        _ => false,
    }
}

pub fn recipe_is_manual_craftable(recipe_id: &str, category: &str) -> bool {
    let category = category.trim();
    !category.eq_ignore_ascii_case("ship passive")
        && !category.eq_ignore_ascii_case("passive")
        && !recipe_id.starts_with("onboard_")
}

/// Merge declared facility requirements with catalog entries that advertise a recipe.
pub fn required_facility_types<'a>(
    state: &EconomyReadState,
    recipe_id: &str,
    declared: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut facility_types = declared
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    for (facility_type, entry) in &state.catalog.facilities {
        if entry.recipe_id.as_deref() != Some(recipe_id) {
            continue;
        }
        let normalized = entry.id.trim();
        let normalized = if normalized.is_empty() {
            facility_type
        } else {
            normalized
        };
        if !normalized.is_empty() {
            facility_types.insert(normalized.to_string());
        }
    }
    facility_types.into_iter().collect()
}

pub fn station_supports_recipe(
    state: &EconomyReadState,
    snapshots: &impl FacilitySnapshotSource,
    station_id: &str,
    facility_id: Option<&str>,
    required_facility_types: &[String],
) -> bool {
    if required_facility_types.is_empty() {
        return true;
    }
    let Some(poi_id) = station_poi_id(state, station_id) else {
        return false;
    };
    let Some(snapshot) = snapshots.facility_snapshot(poi_id) else {
        return false;
    };
    for response in [&snapshot.current, &snapshot.faction_current]
        .into_iter()
        .flatten()
    {
        use spacemolt_lib_rs::schema::FacilityResponse;
        let matches = |id: &str, kind: &str| {
            facility_id.is_none_or(|wanted| wanted == id)
                && required_facility_types.iter().any(|wanted| wanted == kind)
        };
        match response {
            FacilityResponse::FacilityListResponse(
                spacemolt_lib_rs::schema::FacilityListResponse {
                    player_facilities,
                    faction_facilities,
                    station_facilities,
                    public_facilities,
                    ..
                },
            ) => {
                if player_facilities
                    .iter()
                    .any(|row| matches(&row.facility_id, &row.type_))
                    || faction_facilities
                        .iter()
                        .any(|row| matches(&row.facility_id, &row.type_))
                    || station_facilities.iter().any(|row| {
                        facility_owner_matches(state, row.owner_id.as_deref())
                            && matches(&row.facility_id, &row.type_)
                    })
                    || public_facilities.iter().any(|row| {
                        facility_owner_matches(state, row.owner_id.as_deref())
                            && matches(&row.facility_id, &row.type_)
                    })
                {
                    return true;
                }
            }
            FacilityResponse::FacilityFactionListResponse(
                spacemolt_lib_rs::schema::FacilityFactionListResponse {
                    faction_facilities, ..
                },
            ) if faction_facilities
                .iter()
                .any(|row| matches(&row.facility_id, &row.type_)) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn facility_owner_matches(state: &EconomyReadState, owner: Option<&str>) -> bool {
    owner.is_some_and(|owner| {
        [
            state.player_id.as_deref(),
            state.username.as_deref(),
            state.faction_id.as_deref(),
            state.clan_tag.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|candidate| !candidate.trim().is_empty() && candidate.trim() == owner)
    })
}
