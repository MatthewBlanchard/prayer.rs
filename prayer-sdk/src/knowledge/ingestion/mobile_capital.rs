//! Mobile-capital location projection into canonical galaxy knowledge.

use super::super::*;

pub fn apply_mobile_capital_location(galaxy: &mut GalaxyData, system_id: &str) -> bool {
    let system_id = system_id.trim();
    if system_id.is_empty() {
        return false;
    }
    let before = galaxy.clone();
    galaxy.poi_records.remove(MOBILE_BASE_STATION_ID);
    galaxy.poi_records.remove(LEGACY_MOBILE_BASE_STATION_ID);
    galaxy
        .system_records
        .entry(system_id.to_string())
        .or_insert_with(|| prayer_state::SystemKnowledge {
            id: system_id.to_string(),
            ..Default::default()
        });
    let existing = galaxy
        .poi_records
        .get(MOBILE_BASE_POI_ID)
        .cloned()
        .unwrap_or_default();
    galaxy.poi_records.insert(
        MOBILE_BASE_POI_ID.to_string(),
        prayer_state::PoiKnowledge {
            id: MOBILE_BASE_POI_ID.to_string(),
            system_id: system_id.to_string(),
            info: prayer_state::PoiInfoData {
                id: MOBILE_BASE_POI_ID.to_string(),
                name: MOBILE_BASE_NAME.to_string(),
                system_id: system_id.to_string(),
                poi_type: "mobile_capital".to_string(),
                class_name: "frontier_mobile_capital".to_string(),
                description: "Frontier mobile capital".to_string(),
                has_base: true,
                base_id: Some(MOBILE_BASE_STATION_ID.to_string()),
                base_name: Some(MOBILE_BASE_NAME.to_string()),
                ..Default::default()
            },
            info_complete: true,
            resources: existing.resources,
            resources_complete: existing.resources_complete,
            first_discovered_unix: existing.first_discovered_unix,
            last_observed_unix: existing.last_observed_unix,
            first_visited_unix: existing.first_visited_unix,
            last_visited_unix: existing.last_visited_unix,
        },
    );
    galaxy.invalidate_routes();
    *galaxy != before
}
