use crate::economy::quartermaster::{FacilitySnapshot, FacilitySnapshotSource};
pub use crate::state::PoiFacilitiesSnapshot;

use super::WorldState;

impl<VirtualOrder, VirtualCraftOrder> FacilitySnapshotSource
    for WorldState<VirtualOrder, VirtualCraftOrder>
{
    fn facility_snapshot(&self, poi_id: &str) -> Option<FacilitySnapshot> {
        self.facilities_by_poi
            .get(poi_id)
            .map(|snapshot| FacilitySnapshot {
                current: snapshot.current.clone(),
                faction_current: snapshot.faction_current.clone(),
            })
    }
}

pub fn facility_snapshot_fresh(
    snapshot: &PoiFacilitiesSnapshot,
    now_unix: i64,
    ttl_secs: i64,
) -> bool {
    now_unix.saturating_sub(snapshot.observed_at_unix) < ttl_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_is_strict_at_expiry() {
        let snapshot = PoiFacilitiesSnapshot {
            observed_at_unix: 10,
            ..Default::default()
        };
        assert!(facility_snapshot_fresh(&snapshot, 19, 10));
        assert!(!facility_snapshot_fresh(&snapshot, 20, 10));
    }
}
