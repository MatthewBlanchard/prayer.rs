use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use prayer_runtime::dsl::SkillLibraryAst;
use prayer_runtime::engine::{
    EngineCheckpoint, EngineExecutionResult, GalaxyData, GameState, RuntimeEngine, RuntimeEvent,
    RuntimeSnapshot,
};
use prayer_runtime::transport::{MockTransport, RuntimeTransport, SpaceMoltTransport};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state_mapping::map_runtime_state;
use crate::{
    ApiError, ExecuteScriptResponse, RuntimeStateResponse, ScriptDiff, ScriptDiffFlags,
    ScriptLocationDelta, SessionSummary, SetTransportRequest, SkillLibraryTextResponse,
    StepResponse,
};

const DEFAULT_EXECUTE_MAX_STEPS: usize = 10_000;
const MAX_STATUS_LINES: usize = 64;
const DEFAULT_KNOWLEDGE_STATE_PATH: &str = "/tmp/prayer-knowledge-state.json";
const DEBUG_LOG_PATH: &str = "/tmp/prayer-debug.log";
const KNOWLEDGE_SCHEMA_VERSION: u32 = 2;
const FILE_LOCK_TIMEOUT_MS: u64 = 2_000;
const FILE_LOCK_STALE_SECS: u64 = 120;
/// Central runtime session service.
pub struct RuntimeService {
    sessions: RwLock<HashMap<Uuid, Arc<Mutex<SessionHandle>>>>,
    knowledge_state: RwLock<KnowledgeState>,
    knowledge_state_path: PathBuf,
    persistence_telemetry: PersistenceTelemetry,
}

pub(crate) struct SessionHandle {
    pub(crate) label: String,
    pub(crate) created_utc: DateTime<Utc>,
    pub(crate) last_updated_utc: DateTime<Utc>,
    pub(crate) engine: RuntimeEngine,
    pub(crate) live_state: LiveState,
    pub(crate) effective_state: GameState,
    pub(crate) has_state: bool,
    pub(crate) last_halted_state_refresh: Option<Instant>,
    pub(crate) skill_library_text: String,
    pub(crate) transport: Arc<dyn RuntimeTransport>,
    pub(crate) status_lines: Vec<String>,
    pub(crate) current_control_input: Option<String>,
    pub(crate) last_generation_prompt: Option<String>,
    pub(crate) transport_server_key: Option<String>,
    pub(crate) state_version: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LiveState {
    pub(crate) current: GameState,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct KnowledgeState {
    pub(crate) galaxy: GalaxyData,
    pub(crate) shipyard_listing_ids: Vec<String>,
    pub(crate) catalog_versions_by_server: HashMap<String, String>,
}

impl Default for RuntimeService {
    fn default() -> Self {
        let persistence_telemetry = PersistenceTelemetry::default();
        let knowledge_state_path = knowledge_state_path();
        let (knowledge_state, migrated_from_legacy) =
            match load_knowledge_state(&knowledge_state_path) {
                Ok(v) => v,
                Err(err) => {
                    let failures = persistence_telemetry
                        .load_failures
                        .fetch_add(1, Ordering::Relaxed)
                        + 1;
                    warn!(
                        path = %knowledge_state_path.display(),
                        failures,
                        error = %err,
                        "failed to load knowledge cache; starting with empty knowledge"
                    );
                    (KnowledgeState::default(), false)
                }
            };
        if migrated_from_legacy {
            if let Err(err) = save_knowledge_state(&knowledge_state_path, &knowledge_state) {
                warn!(
                    path = %knowledge_state_path.display(),
                    error = %err,
                    "failed to persist migrated knowledge cache"
                );
            }
        }
        Self {
            sessions: RwLock::new(HashMap::new()),
            knowledge_state: RwLock::new(knowledge_state),
            knowledge_state_path,
            persistence_telemetry,
        }
    }
}

impl SessionHandle {
    fn new(label: String) -> Self {
        let now = Utc::now();
        Self {
            label,
            created_utc: now,
            last_updated_utc: now,
            engine: RuntimeEngine::default(),
            live_state: LiveState::default(),
            effective_state: GameState::default(),
            has_state: false,
            last_halted_state_refresh: None,
            skill_library_text: String::new(),
            transport: Arc::new(MockTransport::default()),
            status_lines: vec!["Awaiting script input".to_string()],
            current_control_input: None,
            last_generation_prompt: None,
            transport_server_key: None,
            state_version: 1,
        }
    }

    pub(crate) fn push_status(&mut self, line: impl Into<String>) {
        self.status_lines.push(line.into());
        if self.status_lines.len() > MAX_STATUS_LINES {
            let drop_count = self.status_lines.len() - MAX_STATUS_LINES;
            self.status_lines.drain(0..drop_count);
        }
    }

    pub(crate) fn push_debug_log(&mut self, line: impl Into<String>) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(DEBUG_LOG_PATH)
        {
            let _ = writeln!(f, "{}", line.into());
        }
    }

    fn touch_state(&mut self) {
        self.state_version = self.state_version.saturating_add(1);
        self.last_updated_utc = Utc::now();
    }
}

fn merge_unique_strings(dst: &mut Vec<String>, src: &[String]) {
    for value in src {
        if !dst.iter().any(|v| v == value) {
            dst.push(value.clone());
        }
    }
}

fn merge_map_vec_unique(
    dst: &mut HashMap<String, Vec<String>>,
    src: &HashMap<String, Vec<String>>,
) {
    for (key, values) in src {
        let entry = dst.entry(key.clone()).or_default();
        for value in values {
            if !entry.iter().any(|v| v == value) {
                entry.push(value.clone());
            }
        }
    }
}

fn diff_positive_item_deltas(
    before: &HashMap<String, i64>,
    after: &HashMap<String, i64>,
) -> HashMap<String, i64> {
    let mut deltas = HashMap::new();
    for (item, after_qty) in after {
        let before_qty = before.get(item).copied().unwrap_or(0);
        let gained = after_qty - before_qty;
        if gained > 0 {
            deltas.insert(item.clone(), gained);
        }
    }
    deltas
}

fn diff_item_deltas(before: &HashMap<String, i64>, after: &HashMap<String, i64>) -> Vec<String> {
    let mut all_keys: std::collections::BTreeSet<String> = before.keys().cloned().collect();
    all_keys.extend(after.keys().cloned());
    all_keys
        .into_iter()
        .filter_map(|item| {
            let b = before.get(&item).copied().unwrap_or(0);
            let a = after.get(&item).copied().unwrap_or(0);
            if b != a { Some(format!("{item}: {b} -> {a}")) } else { None }
        })
        .collect()
}

fn arrow(before: &Option<String>, after: &Option<String>) -> String {
    format!(
        "{} -> {}",
        before.as_deref().unwrap_or("?"),
        after.as_deref().unwrap_or("?")
    )
}

fn compute_script_diff(before: &GameState, after: &GameState, halted_after: bool) -> ScriptDiff {
    let docking_changed = before.docked != after.docked;

    let credits = (before.credits != after.credits)
        .then(|| format!("{} -> {}", before.credits, after.credits));
    let fuel = (before.fuel_pct != after.fuel_pct)
        .then(|| format!("{} -> {}", before.fuel_pct, after.fuel_pct));

    let system_changed = before.system != after.system;
    let poi_changed = before.current_poi != after.current_poi;
    let location = (system_changed || poi_changed).then(|| ScriptLocationDelta {
        system: system_changed.then(|| arrow(&before.system, &after.system)),
        poi: poi_changed.then(|| arrow(&before.current_poi, &after.current_poi)),
    });

    let cargo = diff_item_deltas(&before.cargo, &after.cargo);

    // Stash visibility is unreliable across a dock/undock transition — suppress to avoid noise.
    let storage = (!docking_changed).then(|| {
        diff_item_deltas(
            &stash_totals_by_item(&before.stash),
            &stash_totals_by_item(&after.stash),
        )
    });

    ScriptDiff {
        credits,
        fuel,
        location,
        cargo,
        storage,
        flags: ScriptDiffFlags {
            docked_before: before.docked,
            docked_after: after.docked,
            halted_after,
        },
    }
}

fn stash_totals_by_item(stash: &HashMap<String, HashMap<String, i64>>) -> HashMap<String, i64> {
    let mut totals = HashMap::new();
    for items in stash.values() {
        for (item, qty) in items {
            if *qty <= 0 {
                continue;
            }
            *totals.entry(item.clone()).or_insert(0) += *qty;
        }
    }
    totals
}

fn merge_knowledge_state(
    knowledge: &mut KnowledgeState,
    fetched: &GameState,
    server_key: Option<&str>,
) {
    let mut galaxy = knowledge.galaxy.clone();
    let fetched_galaxy = fetched.galaxy.as_ref();

    merge_unique_strings(&mut galaxy.systems, &fetched_galaxy.systems);
    merge_unique_strings(&mut galaxy.pois, &fetched_galaxy.pois);
    merge_unique_strings(&mut galaxy.item_ids, &fetched_galaxy.item_ids);
    merge_unique_strings(&mut galaxy.ship_ids, &fetched_galaxy.ship_ids);
    merge_unique_strings(&mut galaxy.recipe_ids, &fetched_galaxy.recipe_ids);
    for (id, entry) in &fetched_galaxy.item_catalog_entries {
        galaxy
            .item_catalog_entries
            .insert(id.clone(), entry.clone());
    }
    for (id, entry) in &fetched_galaxy.ship_catalog_entries {
        galaxy
            .ship_catalog_entries
            .insert(id.clone(), entry.clone());
    }
    for (id, entry) in &fetched_galaxy.recipe_catalog_entries {
        galaxy
            .recipe_catalog_entries
            .insert(id.clone(), entry.clone());
    }
    if fetched_galaxy.catalog_version.is_some() {
        galaxy.catalog_version = fetched_galaxy.catalog_version.clone();
    }

    for (system_id, neighbors) in &fetched_galaxy.system_connections {
        if neighbors.is_empty() {
            continue;
        }
        let entry = galaxy
            .system_connections
            .entry(system_id.clone())
            .or_default();
        for neighbor in neighbors {
            if !entry.iter().any(|v| v == neighbor) {
                entry.push(neighbor.clone());
            }
        }
    }
    for (system_id, coords) in &fetched_galaxy.system_coordinates {
        galaxy.system_coordinates.insert(system_id.clone(), *coords);
    }
    for (poi_id, system_id) in &fetched_galaxy.poi_system {
        galaxy.poi_system.insert(poi_id.clone(), system_id.clone());
    }
    for (base_id, poi_id) in &fetched_galaxy.poi_base_to_id {
        galaxy
            .poi_base_to_id
            .insert(base_id.clone(), poi_id.clone());
    }
    for (poi_id, poi_type) in &fetched_galaxy.poi_type_by_id {
        galaxy
            .poi_type_by_id
            .insert(poi_id.clone(), poi_type.clone());
    }
    merge_map_vec_unique(
        &mut galaxy.pois_by_resource,
        &fetched_galaxy.pois_by_resource,
    );
    galaxy
        .explored_systems
        .extend(fetched_galaxy.explored_systems.iter().cloned());
    galaxy
        .visited_pois
        .extend(fetched_galaxy.visited_pois.iter().cloned());
    galaxy
        .surveyed_systems
        .extend(fetched_galaxy.surveyed_systems.iter().cloned());
    merge_map_vec_unique(
        &mut galaxy.dockable_pois_by_system,
        &fetched_galaxy.dockable_pois_by_system,
    );
    merge_map_vec_unique(
        &mut galaxy.station_pois_by_system,
        &fetched_galaxy.station_pois_by_system,
    );
    knowledge.galaxy = galaxy;
    if let (Some(key), Some(version)) = (server_key, fetched_galaxy.catalog_version.as_ref()) {
        knowledge
            .catalog_versions_by_server
            .insert(key.to_string(), version.clone());
    }
    merge_unique_strings(
        &mut knowledge.shipyard_listing_ids,
        &fetched.market.shipyard_listings,
    );
}

fn compose_effective_state(knowledge: &KnowledgeState, live: &LiveState) -> GameState {
    let mut composed = live.current.clone();
    composed.galaxy = Arc::new(knowledge.galaxy.clone());

    let mut market = composed.market.as_ref().clone();
    for listing in &knowledge.shipyard_listing_ids {
        if !market.shipyard_listings.iter().any(|v| v == listing) {
            market.shipyard_listings.push(listing.clone());
        }
    }
    composed.market = Arc::new(market);
    composed
}

fn apply_live_state(session: &mut SessionHandle, fetched: GameState, knowledge: &KnowledgeState) {
    session.live_state.current = fetched;
    session.effective_state = compose_effective_state(knowledge, &session.live_state);
    session.has_state = true;
}

#[derive(Default)]
struct PersistenceTelemetry {
    load_failures: AtomicU64,
    save_failures: AtomicU64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedKnowledgeStateV2 {
    knowledge_schema_version: u32,
    state: KnowledgeState,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PersistedKnowledgeStateLegacyV1 {
    galaxy: GalaxyData,
    shipyard_listing_ids: Vec<String>,
}

fn server_key(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn knowledge_state_path() -> PathBuf {
    std::env::var("PRAYER_KNOWLEDGE_STATE_PATH")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_KNOWLEDGE_STATE_PATH))
}

fn load_knowledge_state(path: &Path) -> Result<(KnowledgeState, bool), io::Error> {
    if !path.exists() {
        return Ok((KnowledgeState::default(), false));
    }
    let data = fs::read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&data).map_err(io::Error::other)?;
    if let Some(schema_version) = value
        .get("knowledge_schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        match schema_version as u32 {
            KNOWLEDGE_SCHEMA_VERSION => {
                let persisted: PersistedKnowledgeStateV2 =
                    serde_json::from_value(value).map_err(io::Error::other)?;
                return Ok((persisted.state, false));
            }
            unsupported => {
                return Err(io::Error::other(format!(
                    "unsupported knowledge schema version {unsupported}"
                )));
            }
        }
    }

    let legacy: PersistedKnowledgeStateLegacyV1 =
        serde_json::from_slice(&data).map_err(io::Error::other)?;
    let migrated = KnowledgeState {
        galaxy: legacy.galaxy,
        shipyard_listing_ids: legacy.shipyard_listing_ids,
        catalog_versions_by_server: HashMap::new(),
    };
    Ok((migrated, true))
}

struct FileLockGuard {
    lock_path: PathBuf,
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

fn acquire_file_lock(path: &Path) -> Result<FileLockGuard, io::Error> {
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

fn save_knowledge_state(path: &Path, state: &KnowledgeState) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let _guard = acquire_file_lock(path)?;
    let payload = PersistedKnowledgeStateV2 {
        knowledge_schema_version: KNOWLEDGE_SCHEMA_VERSION,
        state: state.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&payload).map_err(io::Error::other)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}

impl RuntimeService {
    /// Create service instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new runtime session.
    pub fn create_session(&self) -> Uuid {
        self.create_session_with_label(None)
    }

    pub(crate) fn create_session_with_label(&self, label: Option<String>) -> Uuid {
        let id = Uuid::new_v4();
        let label = label
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| format!("session-{}", &id.to_string()[..8]));
        self.sessions
            .write()
            .insert(id, Arc::new(Mutex::new(SessionHandle::new(label))));
        info!(%id, "created runtime session");
        id
    }

    pub(crate) fn parse_id(id: &str) -> Result<Uuid, ApiError> {
        Uuid::parse_str(id).map_err(|_| ApiError::InvalidSessionId)
    }

    pub(crate) async fn get_session(
        &self,
        id: Uuid,
    ) -> Result<Arc<Mutex<SessionHandle>>, ApiError> {
        self.sessions
            .read()
            .get(&id)
            .cloned()
            .ok_or(ApiError::SessionNotFound)
    }

    pub(crate) async fn get_session_by_str(
        &self,
        id: &str,
    ) -> Result<Arc<Mutex<SessionHandle>>, ApiError> {
        let id = Self::parse_id(id)?;
        self.get_session(id).await
    }

    async fn summary_for(&self, id: Uuid, session: &Arc<Mutex<SessionHandle>>) -> SessionSummary {
        let session = session.lock().await;
        let snapshot = session.engine.snapshot();
        SessionSummary {
            id: id.to_string(),
            label: session.label.clone(),
            created_utc: session.created_utc,
            last_updated_utc: session.last_updated_utc,
            is_halted: snapshot.is_halted,
            has_active_command: false,
            current_script_line: snapshot.current_script_line,
        }
    }

    /// List all session summaries.
    pub async fn list_sessions(&self) -> Vec<SessionSummary> {
        let entries: Vec<(Uuid, Arc<Mutex<SessionHandle>>)> = self
            .sessions
            .read()
            .iter()
            .map(|(id, session)| (*id, session.clone()))
            .collect();
        let mut out = Vec::with_capacity(entries.len());
        for (id, session) in entries {
            out.push(self.summary_for(id, &session).await);
        }
        out.sort_by(|a, b| a.created_utc.cmp(&b.created_utc));
        out
    }

    /// Remove a session.
    pub fn remove_session(&self, id: &str) -> Result<bool, ApiError> {
        let id = Self::parse_id(id)?;
        Ok(self.sessions.write().remove(&id).is_some())
    }

    /// Return one session summary.
    pub async fn session_summary(&self, id: &str) -> Result<SessionSummary, ApiError> {
        let uid = Self::parse_id(id)?;
        let session = self.get_session(uid).await?;
        Ok(self.summary_for(uid, &session).await)
    }

    /// Configure runtime transport for a session.
    pub async fn set_transport(&self, id: Uuid, req: SetTransportRequest) -> Result<(), ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        session.transport = match req {
            SetTransportRequest::Mock { state, responses } => {
                session.transport_server_key = None;
                let mut transport = MockTransport::default();
                if let Some(next_state) = state {
                    transport.state = *next_state;
                }
                if let Some(map) = responses {
                    transport.responses = map;
                }
                Arc::new(transport)
            }
            SetTransportRequest::SpaceMolt { base_url, token } => {
                let key = server_key(&base_url);
                session.transport_server_key = Some(key.clone());
                let transport = SpaceMoltTransport::new(base_url, token);
                let knowledge = self.knowledge_state.read().clone();
                transport.seed_catalog_cache(
                    knowledge.catalog_versions_by_server.get(&key).cloned(),
                    &knowledge.galaxy,
                );
                Arc::new(transport)
            }
        };
        session.live_state = LiveState::default();
        session.effective_state = GameState::default();
        session.has_state = false;
        session.push_status("Transport configured");
        session.last_updated_utc = Utc::now();
        Ok(())
    }

    /// Set active script for a session.
    pub async fn set_script(&self, id: Uuid, script: String) -> Result<String, ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let state_snapshot = session.effective_state.clone();
        let normalized = session.engine.set_script(&script, Some(&state_snapshot))?;
        session.current_control_input = Some(script);
        session.push_status("Script loaded and activated");
        session.last_updated_utc = Utc::now();
        Ok(normalized)
    }

    pub(crate) async fn set_library_text(
        &self,
        id: Uuid,
        text: String,
    ) -> Result<SkillLibraryTextResponse, ApiError> {
        let library = SkillLibraryAst::parse(&text)
            .map_err(|errs| ApiError::BadRequest(errs[0].message.clone()))?;
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let canonical = library.normalize();
        session.skill_library_text = canonical.clone();
        session.engine.set_skill_library(library);
        session.last_updated_utc = Utc::now();
        Ok(SkillLibraryTextResponse { text: canonical })
    }

    pub(crate) async fn get_library_text(
        &self,
        id: Uuid,
    ) -> Result<SkillLibraryTextResponse, ApiError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(SkillLibraryTextResponse {
            text: session.skill_library_text.clone(),
        })
    }

    async fn refresh_state_for_host_loop(
        &self,
        session: &mut SessionHandle,
        force: bool,
    ) -> Result<(), ApiError> {
        let is_halted = session.engine.snapshot().is_halted;
        let now = Instant::now();
        let should_refresh = if force || !session.has_state || !is_halted {
            true
        } else {
            match session.last_halted_state_refresh {
                Some(last) => now.duration_since(last) >= Duration::from_secs(1),
                None => true,
            }
        };

        if should_refresh {
            let state = session.transport.fetch_state().await?;
            let knowledge = {
                let mut knowledge = self.knowledge_state.write();
                merge_knowledge_state(
                    &mut knowledge,
                    &state,
                    session.transport_server_key.as_deref(),
                );
                if let Err(err) = save_knowledge_state(&self.knowledge_state_path, &knowledge) {
                    let failures = self
                        .persistence_telemetry
                        .save_failures
                        .fetch_add(1, Ordering::Relaxed)
                        + 1;
                    warn!(
                        path = %self.knowledge_state_path.display(),
                        failures,
                        error = %err,
                        "knowledge cache save failed during refresh"
                    );
                    session.push_status(format!("knowledge cache save failed: {err}"));
                }
                knowledge.clone()
            };
            apply_live_state(session, state, &knowledge);
            if is_halted {
                session.last_halted_state_refresh = Some(now);
            }
            session.touch_state();
        }

        Ok(())
    }

    /// Execute one host-managed runtime step.
    pub async fn execute_step(&self, id: Uuid) -> Result<StepResponse, ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;

        self.refresh_state_for_host_loop(&mut session, false)
            .await?;

        let mut current_state = session.effective_state.clone();
        session.engine.inject_session_counters(&mut current_state);
        let command = session.engine.decide_next(&current_state)?;
        let Some(command) = command else {
            let halted = session.engine.snapshot().is_halted;
            return Ok(StepResponse {
                executed: false,
                command_action: None,
                command_args: None,
                result_message: None,
                halted,
            });
        };

        let command_text = if command.args.is_empty() {
            command.action.clone()
        } else {
            format!("{} {}", command.action, command.args_as_strings().join(" "))
        };
        session.push_debug_log(format!("step: {command_text}"));
        let (result, mut state_after, message) = match session
            .transport
            .execute(&command, Some(&current_state))
            .await
        {
            Ok(result) => {
                let post_state = session.transport.fetch_state().await?;
                let message = result.result_message.clone();
                (result, post_state, message)
            }
            Err(err) => {
                let message = session.engine.render_runtime_error(format!("error: {err}"));
                session.push_debug_log(format!("error: {err}"));
                (
                    EngineExecutionResult {
                        result_message: Some(message.clone()),
                        completed: true,
                        halt_script: false,
                    },
                    session.effective_state.clone(),
                    Some(message),
                )
            }
        };
        let mine_deltas = if command.action.eq_ignore_ascii_case("mine") {
            diff_positive_item_deltas(current_state.cargo.as_ref(), state_after.cargo.as_ref())
        } else {
            HashMap::new()
        };
        let stash_deltas = if command.action.eq_ignore_ascii_case("stash") {
            let before_stash = stash_totals_by_item(current_state.stash.as_ref());
            let after_stash = stash_totals_by_item(state_after.stash.as_ref());
            diff_positive_item_deltas(&before_stash, &after_stash)
        } else {
            HashMap::new()
        };
        state_after.last_mined = Arc::new(mine_deltas);
        state_after.last_stashed = Arc::new(stash_deltas);

        let knowledge = {
            let mut knowledge = self.knowledge_state.write();
            merge_knowledge_state(
                &mut knowledge,
                &state_after,
                session.transport_server_key.as_deref(),
            );
            if let Err(err) = save_knowledge_state(&self.knowledge_state_path, &knowledge) {
                let failures = self
                    .persistence_telemetry
                    .save_failures
                    .fetch_add(1, Ordering::Relaxed)
                    + 1;
                warn!(
                    path = %self.knowledge_state_path.display(),
                    failures,
                    error = %err,
                    "knowledge cache save failed after command execution"
                );
                session.push_status(format!("knowledge cache save failed: {err}"));
            }
            knowledge.clone()
        };
        apply_live_state(&mut session, state_after.clone(), &knowledge);
        session
            .engine
            .execute_result(&command, result, &state_after);
        session.push_status(format!(
            "{command_text} - {}",
            message.clone().unwrap_or_else(|| "done".to_string())
        ));
        let halted = session.engine.snapshot().is_halted;
        if let Some(ref msg) = message {
            session.push_debug_log(format!("result: {msg}"));
        }
        if halted {
            session.push_debug_log("halt: runtime halted after step".to_string());
        }
        session.touch_state();
        let command_action = command.action.clone();
        let command_args = command.args_as_strings();

        Ok(StepResponse {
            executed: true,
            command_action: Some(command_action),
            command_args: Some(command_args),
            result_message: message,
            halted,
        })
    }

    /// Run script loop until completion/halt or step cap.
    pub async fn execute_script(
        &self,
        id: Uuid,
        max_steps: Option<usize>,
    ) -> Result<ExecuteScriptResponse, ApiError> {
        let _ = std::fs::write(DEBUG_LOG_PATH, "run started\n");
        let max_steps = max_steps.unwrap_or(DEFAULT_EXECUTE_MAX_STEPS);
        let mut steps_executed = 0usize;
        let mut error: Option<String> = None;
        let mut halt_message: Option<String> = None;

        let state_before = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            if session.has_state { Some(session.effective_state.clone()) } else { None }
        };

        while steps_executed < max_steps {
            let step = match self.execute_step(id).await {
                Ok(step) => step,
                Err(ApiError::Engine(err)) => {
                    let msg = err.to_string();
                    {
                        use std::io::Write;
                        if let Ok(mut f) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(DEBUG_LOG_PATH)
                        {
                            let _ = writeln!(f, "error: {msg}");
                        }
                    }
                    error = Some(msg);
                    break;
                }
                Err(err) => return Err(err),
            };
            if !step.executed {
                break;
            }
            steps_executed += 1;
            if step.halted {
                halt_message = step.result_message;
                break;
            }
        }

        let snapshot = self.snapshot(id).await?;
        let diff = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            if session.has_state {
                state_before.as_ref().map(|before| {
                    compute_script_diff(before, &session.effective_state, snapshot.is_halted)
                })
            } else {
                None
            }
        };
        Ok(ExecuteScriptResponse {
            steps_executed,
            halted: snapshot.is_halted,
            completed: snapshot.is_finished,
            error,
            halt_message,
            diff,
        })
    }

    /// Fetch latest state from transport and cache it.
    pub async fn refresh_state(&self, id: Uuid) -> Result<GameState, ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        self.refresh_state_for_host_loop(&mut session, true).await?;
        Ok(session.effective_state.clone())
    }

    /// Return currently cached state.
    pub async fn state(&self, id: Uuid) -> Result<GameState, ApiError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.effective_state.clone())
    }

    pub(crate) async fn state_snapshot_with_version(
        &self,
        id: Uuid,
    ) -> Result<(u64, RuntimeStateResponse), ApiError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        let snapshot = session.engine.snapshot();
        let memory = snapshot
            .memory
            .iter()
            .map(|m| {
                let action = if m.args.is_empty() {
                    m.action.clone()
                } else {
                    format!("{} {}", m.action, m.args.join(" "))
                };
                match &m.result_message {
                    Some(msg) if !msg.is_empty() => format!("{action} -> {msg}"),
                    _ => action,
                }
            })
            .collect();
        let response = RuntimeStateResponse {
            state: if session.has_state {
                Some(map_runtime_state(&session.effective_state)?)
            } else {
                None
            },
            memory,
            execution_status_lines: session.status_lines.clone(),
            control_input: session.current_control_input.clone(),
            current_script_line: snapshot.current_script_line,
            script_running: !snapshot.is_halted || snapshot.current_script_line.is_some(),
            last_generation_prompt: session.last_generation_prompt.clone(),
            current_tick: None,
            last_space_molt_post_utc: None,
            active_route: None,
            active_override_name: None,
        };
        Ok((session.state_version, response))
    }

    pub(crate) async fn wait_for_state_change(
        &self,
        id: Uuid,
        since: u64,
        wait_ms: u64,
    ) -> Result<bool, ApiError> {
        let start = Instant::now();
        loop {
            let session = self.get_session(id).await?;
            let current = { session.lock().await.state_version };
            if current > since {
                return Ok(true);
            }
            if start.elapsed() >= Duration::from_millis(wait_ms) {
                return Ok(false);
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    /// Halt a session.
    pub async fn halt(&self, id: Uuid, reason: Option<String>) -> Result<(), ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let reason = reason.unwrap_or_else(|| "halt requested".to_string());
        session.engine.halt(&reason);
        session.push_status(reason);
        session.last_updated_utc = Utc::now();
        Ok(())
    }

    /// Resume a session.
    pub async fn resume(&self, id: Uuid, reason: Option<String>) -> Result<(), ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        let reason = reason.unwrap_or_else(|| "resume requested".to_string());
        session.engine.resume(&reason);
        session.push_status(reason);
        session.last_updated_utc = Utc::now();
        Ok(())
    }

    /// Build runtime snapshot.
    pub async fn snapshot(&self, id: Uuid) -> Result<RuntimeSnapshot, ApiError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.snapshot())
    }

    /// Build checkpoint payload.
    pub async fn checkpoint(&self, id: Uuid) -> Result<EngineCheckpoint, ApiError> {
        let session = self.get_session(id).await?;
        let session = session.lock().await;
        Ok(session.engine.checkpoint())
    }

    /// Restore checkpoint.
    pub async fn restore_checkpoint(
        &self,
        id: Uuid,
        checkpoint: EngineCheckpoint,
    ) -> Result<(), ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        session.engine.restore_checkpoint(checkpoint)?;
        let library = SkillLibraryAst::parse(&session.skill_library_text)
            .map_err(|errs| ApiError::BadRequest(errs[0].message.clone()))?;
        session.engine.set_skill_library(library);
        session.push_status("Resumed from checkpoint");
        session.last_updated_utc = Utc::now();
        Ok(())
    }

    /// Drain emitted runtime events.
    pub async fn drain_events(&self, id: Uuid) -> Result<Vec<RuntimeEvent>, ApiError> {
        let session = self.get_session(id).await?;
        let mut session = session.lock().await;
        Ok(session.engine.drain_events())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use prayer_runtime::engine::{EngineCommand, EngineExecutionResult, GameState};
    use prayer_runtime::transport::{RuntimeTransport, TransportError};
    use serde_json::Value;

    use super::RuntimeService;

    struct FailMineTransport;

    #[async_trait]
    impl RuntimeTransport for FailMineTransport {
        async fn execute(
            &self,
            command: &EngineCommand,
            _runtime_state: Option<&GameState>,
        ) -> Result<EngineExecutionResult, TransportError> {
            if command.action.eq_ignore_ascii_case("mine") {
                return Err(TransportError::UnsupportedCommand("mine".to_string()));
            }
            Ok(EngineExecutionResult {
                result_message: Some("ok".to_string()),
                completed: true,
                halt_script: command.action.eq_ignore_ascii_case("halt"),
            })
        }

        async fn fetch_state(&self) -> Result<GameState, TransportError> {
            Ok(GameState {
                system: Some("sol".to_string()),
                ..GameState::default()
            })
        }

        async fn execute_passthrough(
            &self,
            action: &str,
            _payload: Option<Value>,
            runtime_state: Option<&GameState>,
        ) -> Result<EngineExecutionResult, TransportError> {
            let command = EngineCommand {
                action: action.to_string(),
                args: Vec::new(),
                source_line: None,
            };
            self.execute(&command, runtime_state).await
        }
    }

    #[tokio::test]
    async fn execute_script_continues_after_command_transport_error() {
        let service = RuntimeService::new();
        let id = service.create_session();
        service
            .set_script(id, "mine ore;\nhalt;".to_string())
            .await
            .expect("set script");

        let session = service.get_session(id).await.expect("session");
        {
            let mut session = session.lock().await;
            session.transport = Arc::new(FailMineTransport);
        }

        let run = service.execute_script(id, Some(8)).await.expect("execute");
        assert_eq!(run.steps_executed, 2);
        assert!(run.halted);
        assert!(!run.completed);

        let (_, state) = service
            .state_snapshot_with_version(id)
            .await
            .expect("state snapshot");
        assert!(state.execution_status_lines.iter().any(
            |line| line.contains("mine ore - ") && line.contains("unsupported command 'mine'")
        ));
        assert!(state
            .memory
            .iter()
            .any(|entry| entry.contains("mine ore -> ")
                && entry.contains("unsupported command 'mine'")));
    }

    #[tokio::test]
    async fn create_and_list_sessions() {
        let service = RuntimeService::new();
        let id1 = service.create_session();
        let id2 = service.create_session();
        let list = service.list_sessions().await;
        let ids: Vec<_> = list.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains(&id1.to_string()));
        assert!(ids.contains(&id2.to_string()));
    }

    #[tokio::test]
    async fn remove_session_returns_true_then_false() {
        let service = RuntimeService::new();
        let id = service.create_session();
        let removed = service.remove_session(&id.to_string()).expect("remove");
        assert!(removed);
        let removed_again = service
            .remove_session(&id.to_string())
            .expect("remove again");
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn session_summary_returns_not_found_for_unknown_id() {
        use crate::ApiError;
        let service = RuntimeService::new();
        let fake_id = uuid::Uuid::new_v4().to_string();
        let err = service
            .session_summary(&fake_id)
            .await
            .expect_err("expected not found");
        assert!(matches!(err, ApiError::SessionNotFound));
    }

    #[tokio::test]
    async fn set_script_invalid_dsl_returns_error() {
        use crate::ApiError;
        let service = RuntimeService::new();
        let id = service.create_session();
        let err = service
            .set_script(id, "INVALID GARBAGE !!!".to_string())
            .await
            .expect_err("expected error");
        assert!(matches!(err, ApiError::Engine(_)));
    }

    #[tokio::test]
    async fn set_library_text_invalid_dsl_returns_bad_request() {
        use crate::ApiError;
        let service = RuntimeService::new();
        let id = service.create_session();
        let err = service
            .set_library_text(id, "INVALID GARBAGE !!!".to_string())
            .await
            .expect_err("expected error");
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn halt_and_resume_change_snapshot_state() {
        let service = RuntimeService::new();
        let id = service.create_session();
        service
            .set_script(id, "go alpha;".to_string())
            .await
            .expect("set script");

        service.halt(id, None).await.expect("halt");
        let snapshot = service.snapshot(id).await.expect("snapshot");
        assert!(snapshot.is_halted);

        service.resume(id, None).await.expect("resume");
        let snapshot = service.snapshot(id).await.expect("snapshot after resume");
        assert!(!snapshot.is_halted);
    }

    #[tokio::test]
    async fn checkpoint_roundtrip_restores_script() {
        let service = RuntimeService::new();
        let id = service.create_session();
        service
            .set_script(id, "go alpha;\nhalt;".to_string())
            .await
            .expect("set script");

        let cp = service.checkpoint(id).await.expect("checkpoint");
        assert!(cp.script.contains("go alpha;"));

        let id2 = service.create_session();
        service.restore_checkpoint(id2, cp).await.expect("restore");

        let snap = service.snapshot(id2).await.expect("snapshot");
        assert!(snap.script.contains("go alpha;"));
    }

    #[tokio::test]
    async fn state_snapshot_memory_includes_result_message() {
        let service = RuntimeService::new();
        let id = service.create_session();
        service
            .set_script(id, "mine ore;\nhalt;".to_string())
            .await
            .expect("set script");

        let session = service.get_session(id).await.expect("session");
        {
            let mut session = session.lock().await;
            session.transport = Arc::new(FailMineTransport);
        }

        service.execute_script(id, Some(4)).await.expect("execute");

        let (_, state) = service
            .state_snapshot_with_version(id)
            .await
            .expect("state snapshot");
        assert!(state.memory.iter().any(|m| m.contains("mine ore -> ")));
    }

    #[tokio::test]
    async fn drain_events_clears_after_first_call() {
        let service = RuntimeService::new();
        let id = service.create_session();
        service
            .set_script(id, "halt;".to_string())
            .await
            .expect("set script");

        let events = service.drain_events(id).await.expect("drain");
        assert!(!events.is_empty());

        let events2 = service.drain_events(id).await.expect("drain 2");
        assert!(events2.is_empty());
    }

    #[tokio::test]
    async fn get_and_set_library_text_roundtrip() {
        let service = RuntimeService::new();
        let id = service.create_session();
        let src = "skill noop_skill() { halt; }";
        let resp = service
            .set_library_text(id, src.to_string())
            .await
            .expect("set library");
        // normalizer drops empty parens for parameterless skills
        assert!(resp.text.contains("skill noop_skill"));

        let got = service.get_library_text(id).await.expect("get library");
        assert_eq!(got.text, resp.text);
    }
}
