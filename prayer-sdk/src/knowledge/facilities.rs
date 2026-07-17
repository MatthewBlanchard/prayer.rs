//! Typed facility freshness and catalog projections.

use super::*;

pub fn facility_snapshot_fresh(snapshot: &PoiFacilitiesSnapshot, now_unix: i64) -> bool {
    prayer_runtime::knowledge::facility_snapshot_fresh(
        snapshot,
        now_unix,
        FACILITY_POI_SNAPSHOT_TTL_SECS,
    )
}

pub fn facility_types_from_catalog(
    catalog: &CatalogData,
) -> Vec<spacemolt_lib_rs::schema::FacilityDefinition> {
    let mut out: Vec<_> = catalog.facilities.values().cloned().collect();
    out.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| a.level.cmp(&b.level))
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}
