use super::knowledge::WorldState;
use super::sessions::SessionHandle;
use super::*;

#[derive(Default)]
pub struct PersistenceTelemetry {
    pub load_failures: AtomicU64,
    pub save_failures: AtomicU64,
}

pub struct KnowledgePersistenceRequest {
    pub snapshot: Arc<WorldState>,
    pub context: &'static str,
}

pub struct KnowledgePersistence {
    pending: Arc<ParkingMutex<Option<KnowledgePersistenceRequest>>>,
    wake_tx: mpsc::SyncSender<()>,
    path: PathBuf,
    telemetry: Arc<PersistenceTelemetry>,
}

impl KnowledgePersistence {
    pub fn start(
        path: PathBuf,
        telemetry: Arc<PersistenceTelemetry>,
        persisted_version: Option<u64>,
    ) -> Self {
        let pending = Arc::new(ParkingMutex::new(None));
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        let worker_pending = Arc::clone(&pending);
        let worker_path = path.clone();
        let worker_telemetry = Arc::clone(&telemetry);
        if let Err(err) = thread::Builder::new()
            .name("knowledge-persistence".to_string())
            .spawn(move || {
                run_knowledge_persistence_worker(
                    worker_path,
                    worker_telemetry,
                    worker_pending,
                    wake_rx,
                    persisted_version,
                );
            })
        {
            let failures = telemetry.save_failures.fetch_add(1, Ordering::Relaxed) + 1;
            warn!(
                path = %path.display(),
                failures,
                error = %err,
                "failed to start knowledge persistence worker"
            );
        }
        Self {
            pending,
            wake_tx,
            path,
            telemetry,
        }
    }

    pub fn publish(&self, snapshot: WorldState, context: &'static str) {
        self.publish_shared(Arc::new(snapshot), context);
    }

    pub fn publish_shared(&self, snapshot: Arc<WorldState>, context: &'static str) {
        let request = KnowledgePersistenceRequest { snapshot, context };
        let queued = {
            let mut pending = self.pending.lock();
            replace_pending_knowledge_snapshot(&mut pending, request)
        };
        if !queued {
            return;
        }
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => {}
            Err(TrySendError::Disconnected(())) => {
                let failures = self.telemetry.save_failures.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(
                    path = %self.path.display(),
                    failures,
                    context,
                    "knowledge persistence worker is unavailable"
                );
            }
        }
    }
}

pub fn replace_pending_knowledge_snapshot(
    pending: &mut Option<KnowledgePersistenceRequest>,
    request: KnowledgePersistenceRequest,
) -> bool {
    if pending.as_ref().is_some_and(|current| {
        current.snapshot.knowledge_version >= request.snapshot.knowledge_version
    }) {
        return false;
    }
    *pending = Some(request);
    true
}

pub fn run_knowledge_persistence_worker(
    path: PathBuf,
    telemetry: Arc<PersistenceTelemetry>,
    pending: Arc<ParkingMutex<Option<KnowledgePersistenceRequest>>>,
    wake_rx: mpsc::Receiver<()>,
    mut latest_attempted_version: Option<u64>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            let failures = telemetry.save_failures.fetch_add(1, Ordering::Relaxed) + 1;
            warn!(
                path = %path.display(),
                failures,
                error = %err,
                "failed to initialize knowledge persistence worker runtime"
            );
            return;
        }
    };

    while wake_rx.recv().is_ok() {
        let Some(request) = pending.lock().take() else {
            continue;
        };
        let version = request.snapshot.knowledge_version;
        if latest_attempted_version.is_some_and(|latest| version <= latest) {
            continue;
        }
        // Advance before the write so a failed newer snapshot can never be
        // followed by an older snapshot that happened to publish later.
        latest_attempted_version = Some(version);
        let write_path = path.clone();
        let snapshot = Arc::clone(&request.snapshot);
        let result = runtime.block_on(async move {
            tokio::task::spawn_blocking(move || save_knowledge_state(&write_path, &snapshot)).await
        });
        match result {
            Ok(Ok(())) => {
                debug!(
                    path = %path.display(),
                    knowledge_version = version,
                    context = request.context,
                    "knowledge cache persisted"
                );
            }
            Ok(Err(err)) => {
                let failures = telemetry.save_failures.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(
                    path = %path.display(),
                    knowledge_version = version,
                    failures,
                    error = %err,
                    context = request.context,
                    "knowledge cache save failed"
                );
            }
            Err(err) => {
                let failures = telemetry.save_failures.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(
                    path = %path.display(),
                    knowledge_version = version,
                    failures,
                    error = %err,
                    context = request.context,
                    "knowledge cache save task failed"
                );
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedWorldKnowledgeV4 {
    pub knowledge_schema_version: u32,
    pub state: WorldState,
}

#[derive(serde::Serialize)]
struct PersistedWorldKnowledgeRef<'a> {
    knowledge_schema_version: u32,
    state: &'a WorldState,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedRuntimeSessionsV1 {
    session_schema_version: u32,
    sessions: Vec<PersistedRuntimeSession>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedRuntimeSession {
    pub id: Uuid,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    pub created_utc: DateTime<Utc>,
    pub last_updated_utc: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<PersistedExecutionRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_execution: Option<ScriptExecutionDto>,
    #[serde(default)]
    pub current_control_input: Option<String>,
    #[serde(default)]
    pub status_lines: Vec<String>,
    #[serde(default)]
    pub spacemolt_account_selector: Option<String>,
    #[serde(default)]
    pub spacemolt_base_url: Option<String>,
}

pub fn prayerrs_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("prayerrs")
}

pub fn knowledge_state_path() -> PathBuf {
    prayerrs_data_dir().join("knowledge-state.json")
}

pub fn session_state_path() -> PathBuf {
    prayerrs_data_dir().join("runtime-sessions.json")
}

pub fn load_knowledge_state(path: &Path) -> Result<WorldState, io::Error> {
    if !path.exists() {
        return Ok(WorldState::default());
    }
    let data = fs::read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&data).map_err(io::Error::other)?;
    let Some(schema_version) = value
        .get("knowledge_schema_version")
        .and_then(serde_json::Value::as_u64)
    else {
        return Err(io::Error::other("missing knowledge_schema_version"));
    };
    let value = match schema_version as u32 {
        KNOWLEDGE_SCHEMA_VERSION => value,
        unsupported => {
            return Err(io::Error::other(format!(
                "unsupported knowledge schema version {unsupported}"
            )))
        }
    };
    let mut persisted: PersistedWorldKnowledgeV4 =
        serde_json::from_value(value).map_err(io::Error::other)?;
    // Managed-order automation used reserved ids. Drop those legacy rows while
    // preserving user-authored virtual market and craft orders.
    persisted
        .state
        .virtual_orders
        .retain(|order| !order.id.starts_with("qm:"));
    persisted
        .state
        .virtual_craft_orders
        .retain(|order| !order.id.starts_with("qmc:"));
    Ok(persisted.state)
}

fn migrate_knowledge_v3_to_v4(
    mut value: serde_json::Value,
) -> Result<serde_json::Value, io::Error> {
    let state = value
        .get_mut("state")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| io::Error::other("knowledge v3 payload is missing state"))?;

    if let Some(sightings) = state
        .get_mut("agent_sightings")
        .and_then(serde_json::Value::as_object_mut)
    {
        for sighting in sightings.values_mut() {
            migrate_agent_sighting(sighting);
        }
    }
    if let Some(systems) = state
        .get_mut("system_agents_by_system")
        .and_then(serde_json::Value::as_object_mut)
    {
        for sightings in systems
            .values_mut()
            .filter_map(serde_json::Value::as_array_mut)
        {
            for sighting in sightings {
                migrate_agent_sighting(sighting);
            }
        }
    }
    if let Some(pois) = state
        .get_mut("wildlife_by_poi")
        .and_then(serde_json::Value::as_object_mut)
    {
        for creature in pois
            .values_mut()
            .filter_map(|snapshot| snapshot.get_mut("creatures"))
            .filter_map(serde_json::Value::as_array_mut)
            .flatten()
        {
            migrate_wildlife_creature(creature);
        }
    }
    if let Some(garages) = state
        .get_mut("faction_garage_by_faction")
        .and_then(serde_json::Value::as_object_mut)
    {
        for ship in garages
            .values_mut()
            .filter_map(|garage| garage.get_mut("ships"))
            .filter_map(serde_json::Value::as_array_mut)
            .flatten()
        {
            migrate_faction_garage_ship(ship);
        }
    }
    if let Some(pois) = state
        .get_mut("salvage_by_poi")
        .and_then(serde_json::Value::as_object_mut)
    {
        for salvage in pois.values_mut() {
            if let Some(lootables) = salvage
                .get_mut("visible_lootables")
                .and_then(serde_json::Value::as_array_mut)
            {
                for lootable in lootables {
                    migrate_lootable(lootable);
                }
            }
            if let Some(by_poi) = salvage
                .get_mut("lootables_by_poi")
                .and_then(serde_json::Value::as_object_mut)
            {
                for lootables in by_poi
                    .values_mut()
                    .filter_map(serde_json::Value::as_array_mut)
                {
                    for lootable in lootables {
                        migrate_lootable(lootable);
                    }
                }
            }
        }
    }
    value["knowledge_schema_version"] = serde_json::Value::from(KNOWLEDGE_SCHEMA_VERSION);
    migrate_embedded_galaxy_catalog(value)
}

fn migrate_embedded_galaxy_catalog(
    mut value: serde_json::Value,
) -> Result<serde_json::Value, io::Error> {
    let state = value
        .get_mut("state")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| io::Error::other("knowledge payload is missing state"))?;
    if state.contains_key("catalog") {
        return Ok(value);
    }
    let galaxy = state
        .get_mut("galaxy")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| io::Error::other("knowledge payload is missing galaxy"))?;
    let mut catalog = serde_json::Map::new();
    for (old, new) in [
        ("catalog_version", "version"),
        ("item_catalog_entries", "items"),
        ("ship_catalog_entries", "ships"),
        ("recipe_catalog_entries", "recipes"),
        ("facility_catalog_entries", "facilities"),
        ("skill_catalog_entries", "skills"),
    ] {
        if let Some(bucket) = galaxy.remove(old) {
            catalog.insert(new.to_string(), bucket);
        }
    }
    for obsolete_ids in [
        "item_ids",
        "ship_ids",
        "recipe_ids",
        "facility_ids",
        "skill_ids",
    ] {
        galaxy.remove(obsolete_ids);
    }
    state.insert("catalog".to_string(), serde_json::Value::Object(catalog));
    Ok(value)
}

fn migrate_agent_sighting(value: &mut serde_json::Value) {
    const CONTACT_FIELDS: &[&str] = &[
        "player_id",
        "username",
        "faction_id",
        "faction_tag",
        "clan_tag",
        "ship_class",
        "ship_name",
        "status_message",
        "primary_color",
        "secondary_color",
        "in_combat",
        "offline",
    ];
    let Some(row) = value.as_object_mut() else {
        return;
    };
    if row.contains_key("contact") {
        return;
    }
    let mut contact = serde_json::Map::new();
    for field in CONTACT_FIELDS {
        if let Some(mut item) = row.remove(*field) {
            if item.as_str().is_some_and(str::is_empty) {
                item = serde_json::Value::Null;
            }
            if !item.is_null() {
                contact.insert((*field).to_string(), item);
            }
        }
    }
    contact
        .entry("in_combat".to_string())
        .or_insert(serde_json::Value::Bool(false));
    row.insert("contact".to_string(), serde_json::Value::Object(contact));
}

fn migrate_wildlife_creature(value: &mut serde_json::Value) {
    const FACT_FIELDS: &[&str] = &[
        "creature_id",
        "species",
        "name",
        "role",
        "hull",
        "max_hull",
        "in_combat",
    ];
    let Some(row) = value.as_object_mut() else {
        return;
    };
    if row.contains_key("creature") {
        return;
    }
    let mut creature = serde_json::Map::new();
    for field in FACT_FIELDS {
        if let Some(item) = row.remove(*field) {
            creature.insert((*field).to_string(), item);
        }
    }
    row.insert("creature".to_string(), serde_json::Value::Object(creature));
}

fn migrate_faction_garage_ship(value: &mut serde_json::Value) {
    const SHIP_FIELDS: &[&str] = &[
        "ship_id",
        "class_id",
        "class_name",
        "custom_name",
        "depositor_id",
        "depositor_name",
        "deposited_tick",
    ];
    let Some(row) = value.as_object_mut() else {
        return;
    };
    if row.contains_key("ship") {
        return;
    }
    let mut ship = serde_json::Map::new();
    for field in SHIP_FIELDS {
        if let Some(mut item) = row.remove(*field) {
            if item.as_str().is_some_and(str::is_empty) {
                item = serde_json::Value::Null;
            }
            if !item.is_null() {
                ship.insert((*field).to_string(), item);
            }
        }
    }
    ship.entry("deposited_tick".to_string())
        .or_insert(serde_json::Value::from(0));
    ship.entry("depositor_id".to_string())
        .or_insert(serde_json::Value::String(String::new()));
    row.insert("ship".to_string(), serde_json::Value::Object(ship));
}

fn migrate_lootable(value: &mut serde_json::Value) {
    let Some(row) = value.as_object_mut() else {
        return;
    };
    let Some(modules) = row
        .get_mut("modules")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for module in modules {
        let Some(module) = module.as_object_mut() else {
            continue;
        };
        if let Some(kind) = module.remove("module_type") {
            module.insert("type".to_string(), kind);
        }
        if module.get("wear").is_none_or(serde_json::Value::is_null) {
            module.insert("wear".to_string(), serde_json::Value::from(0.0));
        }
    }
}

#[cfg(test)]
mod knowledge_schema_tests {
    use super::*;

    fn temporary_path(label: &str) -> PathBuf {
        PathBuf::from("/tmp").join(format!("prayerrs-{label}-{}.json", Uuid::new_v4()))
    }

    #[test]
    fn knowledge_loader_rejects_missing_schema_version() {
        let path = temporary_path("missing-knowledge-schema");
        fs::write(&path, br#"{"state":{}}"#).expect("write stale payload");
        let error = load_knowledge_state(&path).expect_err("missing schema must fail");
        let _ = fs::remove_file(path);
        assert!(error
            .to_string()
            .contains("missing knowledge_schema_version"));
    }

    #[test]
    fn knowledge_loader_rejects_old_schema_version() {
        let path = temporary_path("old-knowledge-schema");
        fs::write(&path, br#"{"knowledge_schema_version":2,"state":{}}"#)
            .expect("write stale payload");
        let error = load_knowledge_state(&path).expect_err("old schema must fail");
        let _ = fs::remove_file(path);
        assert!(error
            .to_string()
            .contains("unsupported knowledge schema version 2"));
    }

    #[test]
    fn canonical_galaxy_and_survey_facts_survive_restart() {
        let path = temporary_path("canonical-galaxy-restart");
        let mut state = WorldState::default();
        let galaxy = Arc::make_mut(&mut state.galaxy);
        galaxy.system_records.insert(
            "sol".into(),
            prayer_state::SystemKnowledge {
                id: "sol".into(),
                name: Some("Sol".into()),
                poi_count: Some(5),
                pois_complete: true,
                last_surveyed_unix: Some(200),
                faint_signatures: vec![serde_json::json!({"kind": "mineral"})],
                wildlife: vec![serde_json::json!({"species": "manta"})],
                bloom_status: Some("active".into()),
                observed_at_unix: 200,
                ..Default::default()
            },
        );
        for index in 0..5 {
            let id = format!("poi-{index}");
            galaxy.poi_records.insert(
                id.clone(),
                prayer_state::PoiKnowledge {
                    id: id.clone(),
                    system_id: "sol".into(),
                    info: prayer_state::PoiInfoData {
                        id: id.clone(),
                        system_id: "sol".into(),
                        name: format!("POI {index}"),
                        ..Default::default()
                    },
                    resources: vec![prayer_state::PoiResourceData {
                        resource_id: "iron".into(),
                        ..Default::default()
                    }],
                    resources_complete: true,
                    last_observed_unix: Some(200),
                    ..Default::default()
                },
            );
        }

        save_knowledge_state(&path, &state).expect("save canonical knowledge");
        let restored = load_knowledge_state(&path).expect("load canonical knowledge");
        let _ = fs::remove_file(path);
        let system = &restored.galaxy.system_records["sol"];
        assert_eq!(system.last_surveyed_unix, Some(200));
        assert_eq!(system.poi_count, Some(5));
        assert_eq!(restored.galaxy.poi_records.len(), 5);
        assert!(restored
            .galaxy
            .poi_records
            .values()
            .all(|poi| !poi.info.name.is_empty() && poi.resources_complete));
    }

    #[test]
    fn knowledge_v3_migration_preserves_sighting_and_wildlife_facts() {
        let mut expected = WorldState::default();
        expected.agent_sightings.insert(
            "player_1".to_string(),
            prayer_state::AgentSightingData {
                contact: spacemolt_lib_rs::schema::NearbyPlayer {
                    player_id: Some("player_1".to_string()),
                    username: Some("Scout".to_string()),
                    in_combat: Some(true),
                    ..prayer_state::AgentSightingData::default().contact
                },
                last_seen_system: "sol".to_string(),
                first_seen_unix: 10,
                last_seen_unix: 20,
                times_seen: 2,
            },
        );
        expected.wildlife_by_poi.insert(
            "asteroid_1".to_string(),
            prayer_state::WildlifePoiSnapshotData {
                system_id: "sol".to_string(),
                poi_id: "asteroid_1".to_string(),
                creature_count: 1,
                observed_at_unix: 20,
                creatures: vec![prayer_state::WildlifeCreatureData {
                    creature: spacemolt_lib_rs::schema::CreatureInfo {
                        creature_id: "creature_1".to_string(),
                        hull: 8,
                        in_combat: false,
                        max_hull: 10,
                        name: "Drifter".to_string(),
                        role: "grazer".to_string(),
                        species: "void_manta".to_string(),
                    },
                    system_id: "sol".to_string(),
                    poi_id: "asteroid_1".to_string(),
                    observed_at_unix: 20,
                }],
            },
        );
        Arc::make_mut(&mut expected.catalog).version = Some("0.377.0".to_string());

        let mut legacy = serde_json::to_value(PersistedWorldKnowledgeV4 {
            knowledge_schema_version: 3,
            state: expected.clone(),
        })
        .expect("serialize canonical fixture");
        let state = legacy["state"].as_object_mut().expect("state object");
        let old_catalog = state
            .remove("catalog")
            .and_then(|value| value.as_object().cloned())
            .expect("catalog object");
        let galaxy = state["galaxy"].as_object_mut().expect("galaxy object");
        for (new, old) in [
            ("version", "catalog_version"),
            ("items", "item_catalog_entries"),
            ("ships", "ship_catalog_entries"),
            ("recipes", "recipe_catalog_entries"),
            ("facilities", "facility_catalog_entries"),
            ("skills", "skill_catalog_entries"),
        ] {
            galaxy.insert(
                old.to_string(),
                old_catalog
                    .get(new)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            );
        }
        let sighting = state["agent_sightings"]["player_1"]
            .as_object_mut()
            .expect("sighting object");
        let contact = sighting
            .remove("contact")
            .and_then(|value| value.as_object().cloned())
            .expect("contact object");
        sighting.extend(contact);
        let creature = state["wildlife_by_poi"]["asteroid_1"]["creatures"][0]
            .as_object_mut()
            .expect("creature observation");
        let facts = creature
            .remove("creature")
            .and_then(|value| value.as_object().cloned())
            .expect("creature facts");
        creature.extend(facts);

        let migrated = migrate_knowledge_v3_to_v4(legacy).expect("migrate v3 fixture");
        let restored: PersistedWorldKnowledgeV4 =
            serde_json::from_value(migrated).expect("deserialize migrated fixture");
        assert_eq!(restored.knowledge_schema_version, KNOWLEDGE_SCHEMA_VERSION);
        assert_eq!(restored.state.agent_sightings, expected.agent_sightings);
        assert_eq!(restored.state.wildlife_by_poi, expected.wildlife_by_poi);
        assert_eq!(restored.state.catalog, expected.catalog);
    }
}

pub fn load_runtime_sessions(path: &Path) -> Result<Vec<PersistedRuntimeSession>, io::Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read(path)?;
    if data.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_slice(&data).map_err(io::Error::other)?;
    parse_runtime_sessions_value(value)
}

pub fn parse_runtime_sessions_value(
    value: serde_json::Value,
) -> Result<Vec<PersistedRuntimeSession>, io::Error> {
    let Some(schema_version) = value
        .get("session_schema_version")
        .and_then(serde_json::Value::as_u64)
    else {
        return Err(io::Error::other("missing session_schema_version"));
    };
    match schema_version as u32 {
        SESSION_SCHEMA_VERSION => {
            let persisted: PersistedRuntimeSessionsV1 =
                serde_json::from_value(value).map_err(io::Error::other)?;
            Ok(persisted.sessions)
        }
        unsupported => Err(io::Error::other(format!(
            "unsupported session schema version {unsupported}"
        ))),
    }
}

pub struct FileLockGuard {
    lock_path: PathBuf,
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

pub fn acquire_file_lock(path: &Path) -> Result<FileLockGuard, io::Error> {
    let lock_path = path.with_extension("lock");
    let started = Instant::now();
    loop {
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path)
        {
            Ok(_) => return Ok(FileLockGuard { lock_path }),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if let Ok(metadata) = fs::metadata(&lock_path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified
                            .elapsed()
                            .map(|elapsed| elapsed.as_secs() >= FILE_LOCK_STALE_SECS)
                            .unwrap_or(false)
                        {
                            let _ = fs::remove_file(&lock_path);
                            continue;
                        }
                    }
                }
                if started.elapsed() >= Duration::from_millis(FILE_LOCK_TIMEOUT_MS) {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        format!(
                            "timed out acquiring lock file {}",
                            lock_path.as_path().display()
                        ),
                    ));
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(err) => return Err(err),
        }
    }
}

pub fn save_knowledge_state(path: &Path, state: &WorldState) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let _guard = acquire_file_lock(path)?;
    let payload = PersistedWorldKnowledgeRef {
        knowledge_schema_version: KNOWLEDGE_SCHEMA_VERSION,
        state,
    };
    let tmp = path.with_extension("tmp");
    {
        let file = fs::File::create(&tmp)?;
        let mut writer = io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, &payload).map_err(io::Error::other)?;
        io::Write::flush(&mut writer)?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn save_runtime_sessions(
    path: &Path,
    sessions: Vec<PersistedRuntimeSession>,
) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let _guard = acquire_file_lock(path)?;
    let payload = PersistedRuntimeSessionsV1 {
        session_schema_version: SESSION_SCHEMA_VERSION,
        sessions,
    };
    let bytes = serde_json::to_vec_pretty(&payload).map_err(io::Error::other)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}

impl RuntimeService {
    pub async fn persisted_session_records(&self) -> Vec<PersistedRuntimeSession> {
        let entries: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, session.clone()))
            .collect();
        let mut out = Vec::with_capacity(entries.len());
        for (id, session) in entries {
            let session = session.lock().await;
            out.push(PersistedRuntimeSession {
                id,
                label: session.label.clone(),
                bot_id: session.bot_id.as_ref().map(ToString::to_string),
                created_utc: session.created_utc,
                last_updated_utc: session.last_updated_utc,
                execution: Some(match session.engine.execution_checkpoint() {
                    Ok(execution) => execution,
                    Err(error) => {
                        warn!(session_id = %id, %error, "skipping session with invalid execution checkpoint");
                        continue;
                    }
                }),
                script_execution: session.script_execution.clone(),
                current_control_input: session.current_control_input.clone(),
                status_lines: session.status_lines.clone(),
                spacemolt_account_selector: session.spacemolt_account_selector.clone(),
                spacemolt_base_url: session.spacemolt_base_url.clone(),
            });
        }
        out.sort_by(|a, b| a.created_utc.cmp(&b.created_utc));
        out
    }

    pub async fn persist_sessions(&self, context: &'static str) {
        let records = self.persisted_session_records().await;
        if let Err(err) = save_runtime_sessions(&self.session_state_path, records) {
            let failures = self
                .persistence_telemetry
                .save_failures
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            warn!(
                path = %self.session_state_path.display(),
                failures,
                error = %err,
                context,
                "runtime session store save failed"
            );
        }
    }

    /// Restore persisted runtime sessions and kick non-halted scripts once.
    pub async fn restore_persisted_sessions_on_startup(self: Arc<Self>) {
        let records = match load_runtime_sessions(&self.session_state_path) {
            Ok(records) => records,
            Err(err) => {
                let failures = self
                    .persistence_telemetry
                    .load_failures
                    .fetch_add(1, Ordering::Relaxed)
                    + 1;
                warn!(
                    path = %self.session_state_path.display(),
                    failures,
                    error = %err,
                    "failed to load runtime session store"
                );
                Vec::new()
            }
        };

        let mut startup_runs = Vec::new();
        if records.is_empty() {
            info!(
                path = %self.session_state_path.display(),
                "startup session hydration: no persisted runtime sessions"
            );
        } else {
            info!(
                path = %self.session_state_path.display(),
                count = records.len(),
                "startup session hydration: loaded persisted runtime sessions"
            );

            for record in records {
                if self.sessions.read().contains_key(&record.id) {
                    info!(
                        id = %record.id,
                        label = %record.label,
                        "startup session hydration: skipping already-loaded session"
                    );
                    continue;
                }
                info!(
                    id = %record.id,
                    label = %record.label,
                    has_spacemolt_account_selector = record.spacemolt_account_selector.is_some()
                        && record.spacemolt_base_url.is_some(),
                    has_control_input = record.current_control_input.is_some(),
                    status_lines = record.status_lines.len(),
                    "startup session hydration: hydrating persisted session"
                );

                match self.restore_persisted_session(record).await {
                    Ok(Some(id)) => {
                        info!(%id, "startup session hydration: restored automation awaiting live account reconciliation");
                        startup_runs.push(id);
                    }
                    Ok(None) => {
                        info!(
                            "startup session hydration: persisted session skipped during restore"
                        );
                    }
                    Err(err) => {
                        warn!(error = %err, "failed to restore persisted runtime session")
                    }
                }
            }
        }

        if !self.options.local_auth_bypass {
            match Arc::clone(&self)
                .connect_owned_spacemolt_accounts_on_startup()
                .await
            {
                Ok(()) => Arc::clone(&self).spawn_canonical_data_hydration(),
                Err(err) => {
                    warn!(error = %err, "startup owned account reconciliation failed");
                    let service = Arc::clone(&self);
                    self.spawn_background(async move {
                        let mut attempt = 1u64;
                        loop {
                            if service.is_shutting_down() {
                                break;
                            }
                            let delay = std::time::Duration::from_secs(30.min(attempt * 10));
                            tokio::select! {
                                _ = tokio::time::sleep(delay) => {}
                                _ = service.shutdown_requested() => break,
                            }
                            if service.is_shutting_down() {
                                break;
                            }
                            info!(attempt, ?delay, "retrying owned account reconciliation");
                            match Arc::clone(&service)
                                .connect_owned_spacemolt_accounts_on_startup()
                                .await
                            {
                                Ok(()) => {
                                    info!(attempt, "owned account reconciliation retry succeeded");
                                    Arc::clone(&service).spawn_canonical_data_hydration();
                                    break;
                                }
                                Err(err) => {
                                    warn!(attempt, error = %err, "owned account reconciliation retry failed");
                                    attempt = attempt.saturating_add(1);
                                }
                            }
                        }
                    });
                }
            }
        } else {
            // Local-auth test hosts have no SpaceMolt account, so catalog hydration
            // can proceed while authenticated v2 map hydration reports unavailable.
            Arc::clone(&self).spawn_canonical_data_hydration();
        }

        let mut resumable_runs = Vec::new();
        for id in startup_runs {
            let session = { self.sessions.read().get(&id).cloned() };
            let connected = if let Some(session) = session.as_ref() {
                session.lock().await.spacemolt_account.is_some()
            } else {
                false
            };
            if connected || self.options.local_auth_bypass {
                resumable_runs.push(id);
            } else if let Some(session) = session {
                session.lock().await.push_status(
                    "Restored runtime state is disconnected; the saved SpaceMolt account is unavailable"
                );
            }
        }

        let total_startup_runs = resumable_runs.len();
        for (index, id) in resumable_runs.into_iter().enumerate() {
            Arc::clone(&self)
                .prepare_restored_session_for_resume(id)
                .await;
            info!(
                %id,
                index = index + 1,
                total = total_startup_runs,
                "startup session hydration: queueing restored script runner"
            );
            if let Err(err) = self.spawn_script_runner(id, "startup restore").await {
                warn!(%id, error = %err, "startup restored script runner not started");
            }
        }

        self.persist_sessions("after startup account reconciliation")
            .await;

        info!("startup session hydration: complete");
    }

    pub async fn connect_owned_spacemolt_accounts_on_startup(
        self: Arc<Self>,
    ) -> Result<(), SdkError> {
        let base_url = self.spacemolt_base_url.clone();
        let client = Arc::clone(&self.spacemolt_client);
        let (account_tx, mut account_rx) = tokio::sync::mpsc::unbounded_channel();
        let connect =
            client.connect_owned(ConnectOwnedOptions::default().on_connect(move |account| {
                let _ = account_tx.send(account);
            }));
        let install = async {
            let mut created = 0;
            let mut attached = 0;
            let mut canonical_hydration_started = false;
            let mut refreshes = tokio::task::JoinSet::new();
            let refresh_limit = Arc::new(tokio::sync::Semaphore::new(3));
            let mut refresh_queued = HashSet::new();
            while let Some(account) = account_rx.recv().await {
                let (newly_created, installed) = self
                    .attach_connected_owned_spacemolt_accounts(vec![account], base_url.clone())
                    .await?;
                created += newly_created;
                attached += installed.len();
                if !canonical_hydration_started {
                    canonical_hydration_started = true;
                    Arc::clone(&self).spawn_canonical_data_hydration();
                }
                for id in installed {
                    if !refresh_queued.insert(id) {
                        continue;
                    }
                    let service = Arc::clone(&self);
                    let refresh_limit = Arc::clone(&refresh_limit);
                    refreshes.spawn(async move {
                        let _permit = refresh_limit.acquire_owned().await.expect("semaphore open");
                        let result = service.refresh_state(id).await;
                        (id, result)
                    });
                }
                info!(
                    created,
                    attached,
                    refresh_queued = refresh_queued.len(),
                    "startup owned account connect: progress"
                );
            }
            let refresh_total = refresh_queued.len();
            let mut refreshed = 0;
            let mut failed = 0;
            while let Some(result) = refreshes.join_next().await {
                match result {
                    Ok((_id, Ok(_state))) => refreshed += 1,
                    Ok((id, Err(err))) => {
                        failed += 1;
                        warn!(%id, error = %err, "startup owned account connect: state refresh failed");
                    }
                    Err(err) => {
                        failed += 1;
                        warn!(error = %err, "startup owned account connect: refresh task failed");
                    }
                }
                info!(
                    attached,
                    refreshed,
                    failed,
                    remaining = refresh_total.saturating_sub(refreshed + failed),
                    "startup owned account refresh: progress"
                );
            }
            Ok::<usize, SdkError>(created)
        };
        let (connected, created) = tokio::join!(connect, install);
        connected.map_err(SdkError::from)?;
        let created = created?;
        info!(
            created,
            "startup owned account connect: runtime sessions installed"
        );
        Ok(())
    }

    pub async fn prepare_restored_session_for_resume(self: Arc<Self>, id: Uuid) {
        info!(%id, "startup session hydration: refreshing restored session state");
        if let Err(err) = self.refresh_state(id).await {
            warn!(%id, error = %err, "startup restored session refresh failed");
        } else {
            info!(
                id = %id,
                "startup session hydration: state refresh complete"
            );
        }
        if let Err(err) = self.reanalyze_restored_script_with_actor_world(id).await {
            warn!(%id, error = %err, "startup restored script reanalysis failed");
        }
    }
}
