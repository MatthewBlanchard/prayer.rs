//! Local cache of the subscribed observation watch using generated DTOs.

use std::collections::HashMap;

use crate::schema::{
    NearbyPlayer, NotificationObservationUpdate, ScanContact, SubscribeObservationResponse,
};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ObservedPlayer {
    NearbySnapshot(NearbyPlayer),
    SystemSnapshot(NearbyPlayer),
    NearbyUpdate(NearbyPlayer),
    SystemUpdate(NearbyPlayer),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum CloakedContact {
    Snapshot(ScanContact),
    Update(ScanContact),
}

#[derive(Debug, Clone)]
pub struct ObservationView {
    pub poi_id: Option<String>,
    pub system_id: Option<String>,
    pub tick: i64,
    pub nearby: HashMap<String, ObservedPlayer>,
    pub system: HashMap<String, ObservedPlayer>,
    pub cloaked: HashMap<String, CloakedContact>,
    pub unknown_signature: bool,
    pub active_scan: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ObservationCache {
    view: Option<ObservationView>,
}

impl ObservationCache {
    pub fn seed(&mut self, snapshot: SubscribeObservationResponse) -> &ObservationView {
        let nearby = snapshot
            .nearby
            .into_iter()
            .filter_map(|item| {
                item.player_id
                    .clone()
                    .map(|id| (id, ObservedPlayer::NearbySnapshot(item)))
            })
            .collect();
        let system = snapshot
            .system_agents
            .into_iter()
            .filter_map(|item| {
                item.player_id
                    .clone()
                    .map(|id| (id, ObservedPlayer::SystemSnapshot(item)))
            })
            .collect();
        let cloaked = snapshot
            .cloaked_contacts
            .into_iter()
            .map(|item| (item.target_id.clone(), CloakedContact::Snapshot(item)))
            .collect();
        self.view = Some(ObservationView {
            poi_id: Some(snapshot.poi_id),
            system_id: Some(snapshot.system_id),
            tick: 0,
            nearby,
            system,
            cloaked,
            unknown_signature: snapshot.unknown_signature,
            active_scan: snapshot.active_scan,
        });
        self.view.as_ref().expect("view was just seeded")
    }

    pub fn apply_update(&mut self, update: NotificationObservationUpdate) {
        let view = self.view.get_or_insert_with(|| ObservationView {
            poi_id: Some(update.poi_id.clone()),
            system_id: Some(update.system_id.clone()),
            tick: 0,
            nearby: HashMap::new(),
            system: HashMap::new(),
            cloaked: HashMap::new(),
            unknown_signature: false,
            active_scan: false,
        });
        view.poi_id = Some(update.poi_id);
        view.system_id = Some(update.system_id);
        view.tick = update.tick;
        for item in update.nearby_changed {
            if let Some(id) = item.player_id.clone() {
                view.nearby.insert(id, ObservedPlayer::NearbyUpdate(item));
            }
        }
        for id in update.nearby_departed {
            view.nearby.remove(&id);
        }
        for item in update.system_changed {
            if let Some(id) = item.player_id.clone() {
                view.system.insert(id, ObservedPlayer::SystemUpdate(item));
            }
        }
        for id in update.system_departed {
            view.system.remove(&id);
        }
        for item in update.cloaked_resolved {
            view.cloaked
                .insert(item.target_id.clone(), CloakedContact::Update(item));
        }
        for id in update.cloaked_lost {
            view.cloaked.remove(&id);
        }
        view.unknown_signature = update.unknown_signature;
        if let Some(active_scan) = update.active_scan {
            view.active_scan = active_scan;
        }
    }

    pub fn current(&self) -> Option<&ObservationView> {
        self.view.as_ref()
    }
    pub fn clear(&mut self) {
        self.view = None;
    }
}
