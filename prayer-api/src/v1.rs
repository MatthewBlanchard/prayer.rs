use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Weak};
use std::time::Instant;

use axum::body::Body;
use axum::extract::rejection::JsonRejection;
use axum::extract::DefaultBodyLimit;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use prayer_actions::{Action, RunId, ACTION_SCHEMA_VERSION};
use prayer_sdk::{ActionRunHandle, PrayerSdk, RunStatus, ScriptRunHandle, SdkError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::contracts::*;

const API_VERSION: &str = "1.0";
const MAX_ACTIONS: usize = 256;

#[derive(Clone)]
struct V1State {
    sdk: Arc<PrayerSdk>,
    idempotency: IdempotencyStore,
    world_history: Arc<Mutex<VecDeque<CachedWorldRevision>>>,
    world_domains: Arc<Mutex<WorldDomainCache>>,
}

type SharedWorldState = prayer_state::WorldState<
    prayer_api_contracts::RuntimeVirtualMarketOrderDto,
    prayer_api_contracts::RuntimeVirtualCraftOrderDto,
>;

struct CachedWorldRevision {
    version: u64,
    state: Arc<SharedWorldState>,
}

#[derive(Default)]
struct WorldDomainCache {
    observed_world_version: Option<u64>,
    state: Option<Arc<SharedWorldState>>,
    map_json: Vec<u8>,
    resources_json: Vec<u8>,
    wildlife_json: Vec<u8>,
    versions: WorldDomainVersions,
}

#[derive(Clone, Copy, Default)]
struct WorldDomainVersions {
    map: u64,
    resources: u64,
    wildlife: u64,
    markets: u64,
    storage: u64,
    facilities: u64,
    observations: u64,
    communications: u64,
    factions: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct IdempotencyRecord {
    fingerprint: String,
    run_id: RunId,
}

type IdempotencyKey = (String, String, &'static str);

#[derive(Clone)]
struct IdempotencyStore {
    records: Arc<Mutex<HashMap<IdempotencyKey, IdempotencyRecord>>>,
    flights: Arc<Mutex<HashMap<IdempotencyKey, Weak<Mutex<()>>>>>,
    persistence: Arc<Mutex<()>>,
    path: Arc<std::path::PathBuf>,
}

impl IdempotencyStore {
    fn new(path: std::path::PathBuf) -> Self {
        Self {
            records: Arc::new(Mutex::new(load_idempotency(&path))),
            flights: Arc::new(Mutex::new(HashMap::new())),
            persistence: Arc::new(Mutex::new(())),
            path: Arc::new(path),
        }
    }

    async fn key_guard(&self, key: &IdempotencyKey) -> tokio::sync::OwnedMutexGuard<()> {
        let mut flights = self.flights.lock().await;
        flights.retain(|_, lock| lock.strong_count() > 0);
        let lock = flights.get(key).and_then(Weak::upgrade).unwrap_or_else(|| {
            let lock = Arc::new(Mutex::new(()));
            flights.insert(key.clone(), Arc::downgrade(&lock));
            lock
        });
        drop(flights);
        lock.lock_owned().await
    }

    async fn get(&self, key: &IdempotencyKey) -> Option<IdempotencyRecord> {
        self.records.lock().await.get(key).cloned()
    }

    async fn insert_and_persist(
        &self,
        key: IdempotencyKey,
        record: IdempotencyRecord,
    ) -> Result<(), V1Error> {
        self.records.lock().await.insert(key, record);
        let _persistence = self.persistence.lock().await;
        let snapshot = self.records.lock().await.clone();
        let path = Arc::clone(&self.path);
        tokio::task::spawn_blocking(move || persist_idempotency(&path, &snapshot))
            .await
            .map_err(|error| V1Error::Internal(error.to_string()))?
    }
}

#[derive(Serialize, Deserialize)]
struct PersistedIdempotencyRecord {
    bot_id: String,
    key: String,
    kind: String,
    fingerprint: String,
    run_id: RunId,
}

#[derive(Debug, Default, Deserialize)]
struct V1StateQuery {
    fleet_version: Option<u64>,
    world_version: Option<u64>,
    map_version: Option<u64>,
    resources_version: Option<u64>,
    wildlife_version: Option<u64>,
    markets_version: Option<u64>,
    storage_version: Option<u64>,
    facilities_version: Option<u64>,
    observations_version: Option<u64>,
    communications_version: Option<u64>,
    factions_version: Option<u64>,
    catalog_version: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct V1StateVersions {
    fleet: u64,
    world: u64,
    map: u64,
    resources: u64,
    wildlife: u64,
    markets: u64,
    storage: u64,
    facilities: u64,
    observations: u64,
    communications: u64,
    factions: u64,
    catalog: String,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct V1WorldState {
    #[serde(skip_serializing_if = "Option::is_none")]
    map: Option<prayer_api_contracts::RuntimeGalaxyMapSnapshotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<prayer_api_contracts::RuntimeGalaxyResourcesDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wildlife: Option<prayer_api_contracts::RuntimeGalaxyWildlifeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    station_markets: Option<HashMap<String, prayer_state::StationMarketData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    station_market_delta: Option<V1StationMarketDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_by_player: Option<HashMap<String, HashMap<String, HashMap<String, i64>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    faction_storage_by_faction_poi: Option<HashMap<String, HashMap<String, HashMap<String, i64>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    facilities_by_poi: Option<HashMap<String, prayer_state::PoiFacilitiesSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owned_facilities_by_player:
        Option<HashMap<String, prayer_api_contracts::CanonicalFacilityResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owned_facilities_by_faction:
        Option<HashMap<String, prayer_api_contracts::CanonicalFacilityResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    station_passengers: Option<HashMap<String, prayer_state::PassengerState>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salvage_by_poi: Option<HashMap<String, prayer_state::SalvageData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_sightings: Option<HashMap<String, prayer_state::AgentSightingData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_messages_by_session: Option<HashMap<String, Vec<prayer_state::ChatMessageData>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    faction_by_session: Option<HashMap<String, prayer_state::FactionSnapshotData>>,
    updated_at_utc: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct V1StationMarketDelta {
    base_version: u64,
    upsert: HashMap<String, prayer_state::StationMarketData>,
    remove: Vec<String>,
}

fn station_market_delta(
    base_version: u64,
    previous: &HashMap<String, prayer_state::StationMarketData>,
    current: &HashMap<String, prayer_state::StationMarketData>,
) -> V1StationMarketDelta {
    let upsert = current
        .iter()
        .filter(|(id, market)| previous.get(*id) != Some(*market))
        .map(|(id, market)| (id.clone(), market.clone()))
        .collect();
    let mut remove = previous
        .keys()
        .filter(|id| !current.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    remove.sort();
    V1StationMarketDelta {
        base_version,
        upsert,
        remove,
    }
}

/// Produces an opaque fleet revision that changes when either bot membership or
/// any bot's state revision changes. Taking only the maximum bot revision makes
/// an empty fleet indistinguishable from a newly connected version-zero bot and
/// can also miss removals when the highest-version bot remains.
fn fleet_version(fleet: &prayer_state::FleetSnapshot) -> u64 {
    fleet_version_for(
        fleet
            .bots
            .iter()
            .map(|(id, bot)| (id.as_str(), bot.version)),
    )
}

fn fleet_version_for<'a>(bots: impl IntoIterator<Item = (&'a str, u64)>) -> u64 {
    let mut bots = bots.into_iter().collect::<Vec<_>>();
    bots.sort_unstable_by(|left, right| left.0.cmp(right.0));

    // Stable FNV-1a over length-delimited ids and bot revisions. This value is
    // only an equality token for conditional reads; it need not be sequential.
    let mut hash = 0xcbf29ce484222325_u64;
    for (id, version) in bots {
        for byte in (id.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(id.bytes())
            .chain(version.to_le_bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct V1StateResponse {
    versions: V1StateVersions,
    fleet: Option<prayer_state::FleetSnapshot>,
    world: Option<V1WorldState>,
    catalog: Option<prayer_api_contracts::RuntimeGalaxyCatalogDto>,
}

#[derive(Debug)]
pub enum V1Error {
    Sdk(SdkError),
    Validation(String),
    IdempotencyConflict,
    Internal(String),
    Unauthorized,
}

impl From<SdkError> for V1Error {
    fn from(value: SdkError) -> Self {
        Self::Sdk(value)
    }
}

impl IntoResponse for V1Error {
    fn into_response(self) -> Response {
        let request_id = Uuid::new_v4().to_string();
        let (status, code, message, retryable, details) = match self {
            Self::Validation(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation",
                message,
                false,
                None,
            ),
            Self::IdempotencyConflict => (
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "idempotency key was reused with a different request".into(),
                false,
                None,
            ),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                message,
                false,
                None,
            ),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "valid bearer authentication is required".into(),
                false,
                None,
            ),
            Self::Sdk(error) => map_sdk_error(error),
        };
        let body = V1ErrorEnvelope {
            error: V1ErrorDetail {
                code: code.into(),
                message,
                retryable,
                details,
            },
            request_id: request_id.clone(),
        };
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            HeaderName::from_static("x-request-id"),
            HeaderValue::from_str(&request_id).expect("UUID is a valid header value"),
        );
        response
    }
}

fn map_sdk_error(error: SdkError) -> (StatusCode, &'static str, String, bool, Option<Value>) {
    let message = error.to_string();
    match error {
        SdkError::BotNotFound { .. } => {
            (StatusCode::NOT_FOUND, "bot_not_found", message, false, None)
        }
        SdkError::AmbiguousBot { .. } => {
            (StatusCode::CONFLICT, "ambiguous_bot", message, false, None)
        }
        SdkError::RunNotFound { .. } => {
            (StatusCode::NOT_FOUND, "run_not_found", message, false, None)
        }
        SdkError::WaitTimedOut { .. } => (
            StatusCode::REQUEST_TIMEOUT,
            "wait_timed_out",
            message,
            true,
            None,
        ),
        SdkError::LaneBusy {
            owner,
            run_id,
            generation,
        } => (
            StatusCode::CONFLICT,
            "lane_busy",
            message,
            false,
            Some(json!({
                "ownerKind": queue_owner_kind(&owner), "runId": run_id.0, "generation": generation
            })),
        ),
        SdkError::ShutdownInProgress => (
            StatusCode::SERVICE_UNAVAILABLE,
            "shutdown_in_progress",
            message,
            true,
            None,
        ),
        SdkError::Client(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "upstream_unavailable",
            message,
            error.is_retryable(),
            error
                .retry_after()
                .map(|delay| json!({ "retryAfterMs": delay.as_millis() })),
        ),
        SdkError::BadRequest(_) | SdkError::Command(_) | SdkError::InvalidSessionId => (
            StatusCode::BAD_REQUEST,
            "invalid_request",
            message,
            false,
            None,
        ),
        SdkError::SessionNotFound => (StatusCode::NOT_FOUND, "bot_not_found", message, false, None),
        SdkError::Engine(_) | SdkError::InvalidRuntimeState(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            message,
            false,
            None,
        ),
    }
}

fn queue_owner_kind(owner: &prayer_sdk::LaneOwner) -> &'static str {
    match owner {
        prayer_sdk::LaneOwner::PrayerLang => "prayer_lang",
        prayer_sdk::LaneOwner::Manual => "action_run",
        prayer_sdk::LaneOwner::Controller { .. } => "controller",
    }
}

pub fn build_v1_router(sdk: Arc<PrayerSdk>) -> Router {
    let idempotency_path = std::env::var("PRAYER_V1_IDEMPOTENCY_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("Prayer")
                .join("api-v1-idempotency.json")
        });
    let state = V1State {
        sdk,
        idempotency: IdempotencyStore::new(idempotency_path),
        world_history: Arc::new(Mutex::new(VecDeque::new())),
        world_domains: Arc::new(Mutex::new(WorldDomainCache::default())),
    };
    let auth = AuthState {
        token: std::env::var("PRAYER_API_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty()),
    };
    Router::new()
        .route("/api/v1/meta", get(meta))
        .route("/api/v1/state", get(get_state))
        .route("/api/v1/routes", post(select_routes))
        .route("/api/v1/bots", get(list_bots))
        .route("/api/v1/bots/register", post(register_bot))
        .route("/api/v1/bots/:bot_id", get(get_bot))
        .route("/api/v1/bots/:bot_id/queue", get(get_queue))
        .route("/api/v1/bots/:bot_id/queue/normal", get(get_normal_queue))
        .route(
            "/api/v1/bots/:bot_id/queue/override",
            get(get_override_queue),
        )
        .route("/api/v1/bots/:bot_id/halt", post(halt_bot))
        .route("/api/v1/bots/:bot_id/action-runs", post(start_action_run))
        .route(
            "/api/v1/bots/:bot_id/action-overrides",
            post(execute_action_override),
        )
        .route(
            "/api/v1/bots/:bot_id/script-overrides",
            post(execute_script_override),
        )
        .route(
            "/api/v1/bots/:bot_id/action-runs/:run_id",
            get(get_action_run),
        )
        .route(
            "/api/v1/bots/:bot_id/action-runs/:run_id/cancel",
            post(cancel_action_run),
        )
        .route("/api/v1/bots/:bot_id/script-runs", post(start_script_run))
        .route(
            "/api/v1/bots/:bot_id/script-runs/:run_id",
            get(get_script_run),
        )
        .route(
            "/api/v1/bots/:bot_id/script-runs/:run_id/cancel",
            post(cancel_script_run),
        )
        .layer(middleware::from_fn_with_state(auth, authorize))
        .layer(middleware::from_fn(add_request_id))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(state)
}

#[derive(Clone)]
struct AuthState {
    token: Option<String>,
}

async fn authorize(
    State(auth): State<AuthState>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let Some(token) = auth.token else {
        return next.run(request).await;
    };
    let expected = format!("Bearer {token}");
    if request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        == Some(expected.as_str())
    {
        next.run(request).await
    } else {
        V1Error::Unauthorized.into_response()
    }
}

async fn add_request_id(request: axum::http::Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    if !response.headers().contains_key("x-request-id") {
        let request_id = Uuid::new_v4().to_string();
        response.headers_mut().insert(
            HeaderName::from_static("x-request-id"),
            HeaderValue::from_str(&request_id).expect("UUID is a valid header value"),
        );
    }
    response
}

fn load_idempotency(
    path: &std::path::Path,
) -> HashMap<(String, String, &'static str), IdempotencyRecord> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    let Ok(records) = serde_json::from_slice::<Vec<PersistedIdempotencyRecord>>(&bytes) else {
        return HashMap::new();
    };
    records
        .into_iter()
        .filter_map(|record| {
            let kind = match record.kind.as_str() {
                "action" => "action",
                "script" => "script",
                _ => return None,
            };
            Some((
                (record.bot_id, record.key, kind),
                IdempotencyRecord {
                    fingerprint: record.fingerprint,
                    run_id: record.run_id,
                },
            ))
        })
        .collect()
}

fn persist_idempotency(
    path: &std::path::Path,
    index: &HashMap<(String, String, &'static str), IdempotencyRecord>,
) -> Result<(), V1Error> {
    let mut records = index
        .iter()
        .map(|((bot_id, key, kind), record)| PersistedIdempotencyRecord {
            bot_id: bot_id.clone(),
            key: key.clone(),
            kind: (*kind).into(),
            fingerprint: record.fingerprint.clone(),
            run_id: record.run_id.clone(),
        })
        .collect::<Vec<_>>();
    records.sort_by(|a, b| (&a.bot_id, &a.key, &a.kind).cmp(&(&b.bot_id, &b.key, &b.kind)));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| V1Error::Internal(error.to_string()))?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(&records)
            .map_err(|error| V1Error::Internal(error.to_string()))?,
    )
    .map_err(|error| V1Error::Internal(error.to_string()))?;
    std::fs::rename(temp, path).map_err(|error| V1Error::Internal(error.to_string()))
}

async fn meta() -> Json<V1MetaResponse> {
    Json(V1MetaResponse {
        api_version: API_VERSION,
        server_version: env!("CARGO_PKG_VERSION"),
        action_schema_version: ACTION_SCHEMA_VERSION,
        capabilities: vec![
            "action_runs",
            "prayerlang_runs",
            "conditional_state",
            "conditional_world_sections",
            "keyed_market_deltas",
            "cached_routes",
        ],
    })
}

async fn get_state(
    State(state): State<V1State>,
    Query(query): Query<V1StateQuery>,
) -> Result<Response, V1Error> {
    let started = Instant::now();
    let snapshot_started = Instant::now();
    let snapshot = state.sdk.host_state().await;
    let snapshot_ms = snapshot_started.elapsed().as_millis();
    let catalog_version = snapshot
        .world
        .state
        .catalog
        .version
        .clone()
        .unwrap_or_default();
    let fleet_version = fleet_version(&snapshot.fleet);
    let world_version = snapshot.world.version;

    let mut projected = None;
    let (domain_versions, market_changed) = {
        let mut domains = state.world_domains.lock().await;
        let changed = domains.observed_world_version != Some(world_version);
        let mut market_changed = false;
        if changed {
            let next = prayer_sdk::state_mapping::map_shared_runtime_world_state(
                snapshot.world.state.catalog.as_ref(),
                snapshot.world.state.galaxy.as_ref(),
                &snapshot.world.state.wildlife_by_poi,
            )?;
            let map_json = serde_json::to_vec(&next.map)
                .map_err(|error| V1Error::Internal(error.to_string()))?;
            let resources_json = serde_json::to_vec(&next.resources)
                .map_err(|error| V1Error::Internal(error.to_string()))?;
            let wildlife_json = serde_json::to_vec(&next.wildlife)
                .map_err(|error| V1Error::Internal(error.to_string()))?;
            let initial = domains.state.is_none();
            let map_changed = initial || domains.map_json != map_json;
            let resources_changed = initial || domains.resources_json != resources_json;
            let wildlife_changed = initial || domains.wildlife_json != wildlife_json;
            market_changed = initial
                || domains.state.as_ref().is_none_or(|previous| {
                    previous.station_markets != snapshot.world.state.station_markets
                });
            let storage_changed = initial
                || domains.state.as_ref().is_none_or(|previous| {
                    previous.storage_by_player != snapshot.world.state.storage_by_player
                        || previous.faction_storage_by_faction_poi
                            != snapshot.world.state.faction_storage_by_faction_poi
                });
            let facilities_changed = initial
                || domains.state.as_ref().is_none_or(|previous| {
                    previous.facilities_by_poi != snapshot.world.state.facilities_by_poi
                        || previous.owned_facilities_by_player
                            != snapshot.world.state.owned_facilities_by_player
                        || previous.owned_facilities_by_faction
                            != snapshot.world.state.owned_facilities_by_faction
                });
            let observations_changed = initial
                || domains.state.as_ref().is_none_or(|previous| {
                    previous.station_passengers != snapshot.world.state.station_passengers
                        || previous.salvage_by_poi != snapshot.world.state.salvage_by_poi
                        || previous.agent_sightings != snapshot.world.state.agent_sightings
                });
            let communications_changed = initial
                || domains.state.as_ref().is_none_or(|previous| {
                    previous.chat_messages_by_session
                        != snapshot.world.state.chat_messages_by_session
                });
            let factions_changed = initial
                || domains.state.as_ref().is_none_or(|previous| {
                    previous.faction_by_session != snapshot.world.state.faction_by_session
                });
            if map_changed {
                domains.versions.map = world_version;
            }
            if resources_changed {
                domains.versions.resources = world_version;
            }
            if wildlife_changed {
                domains.versions.wildlife = world_version;
            }
            if market_changed {
                domains.versions.markets = world_version;
            }
            if storage_changed {
                domains.versions.storage = world_version;
            }
            if facilities_changed {
                domains.versions.facilities = world_version;
            }
            if observations_changed {
                domains.versions.observations = world_version;
            }
            if communications_changed {
                domains.versions.communications = world_version;
            }
            if factions_changed {
                domains.versions.factions = world_version;
            }
            domains.observed_world_version = Some(world_version);
            domains.state = Some(Arc::clone(&snapshot.world.state));
            domains.map_json = map_json;
            domains.resources_json = resources_json;
            domains.wildlife_json = wildlife_json;
            projected = Some(next);
        }
        (domains.versions, market_changed)
    };

    if market_changed {
        let mut history = state.world_history.lock().await;
        if history.back().map(|entry| entry.version) != Some(domain_versions.markets) {
            history.push_back(CachedWorldRevision {
                version: domain_versions.markets,
                state: Arc::clone(&snapshot.world.state),
            });
            while history.len() > 2 {
                history.pop_front();
            }
        }
    }

    let requested_map = query.map_version.or(query.world_version);
    let requested_resources = query.resources_version.or(query.world_version);
    let requested_wildlife = query.wildlife_version.or(query.world_version);
    let requested_markets = query.markets_version.or(query.world_version);
    let requested_storage = query.storage_version.or(query.world_version);
    let requested_facilities = query.facilities_version.or(query.world_version);
    let requested_observations = query.observations_version.or(query.world_version);
    let requested_communications = query.communications_version.or(query.world_version);
    let requested_factions = query.factions_version.or(query.world_version);
    let include_map = requested_map != Some(domain_versions.map);
    let include_resources = requested_resources != Some(domain_versions.resources);
    let include_wildlife = requested_wildlife != Some(domain_versions.wildlife);
    let include_markets = requested_markets != Some(domain_versions.markets);
    let include_storage = requested_storage != Some(domain_versions.storage);
    let include_facilities = requested_facilities != Some(domain_versions.facilities);
    let include_observations = requested_observations != Some(domain_versions.observations);
    let include_communications = requested_communications != Some(domain_versions.communications);
    let include_factions = requested_factions != Some(domain_versions.factions);
    let include_world = include_map
        || include_resources
        || include_wildlife
        || include_markets
        || include_storage
        || include_facilities
        || include_observations
        || include_communications
        || include_factions;

    if include_world && projected.is_none() {
        projected = Some(prayer_sdk::state_mapping::map_shared_runtime_world_state(
            snapshot.world.state.catalog.as_ref(),
            snapshot.world.state.galaxy.as_ref(),
            &snapshot.world.state.wildlife_by_poi,
        )?);
    }

    let market_update = if include_markets {
        let history = state.world_history.lock().await;
        let base = requested_markets.and_then(|version| {
            history
                .iter()
                .find(|entry| entry.version == version)
                .map(|entry| Arc::clone(&entry.state))
        });
        Some(match base {
            Some(base) => (
                None,
                Some(station_market_delta(
                    requested_markets.expect("cached market base has a version"),
                    &base.station_markets,
                    &snapshot.world.state.station_markets,
                )),
            ),
            None => (Some(snapshot.world.state.station_markets.clone()), None),
        })
    } else {
        None
    };

    let fleet = (query.fleet_version != Some(fleet_version)).then(|| snapshot.fleet.clone());
    let world = if include_world {
        let projected = projected.expect("included world has a projection");
        let (station_markets, station_market_delta) = market_update.unwrap_or((None, None));
        Some(V1WorldState {
            map: include_map.then_some(projected.map),
            resources: include_resources.then_some(projected.resources),
            wildlife: include_wildlife.then_some(projected.wildlife),
            station_markets,
            station_market_delta,
            storage_by_player: include_storage
                .then(|| snapshot.world.state.storage_by_player.clone()),
            faction_storage_by_faction_poi: include_storage
                .then(|| snapshot.world.state.faction_storage_by_faction_poi.clone()),
            facilities_by_poi: include_facilities
                .then(|| snapshot.world.state.facilities_by_poi.clone()),
            owned_facilities_by_player: include_facilities
                .then(|| snapshot.world.state.owned_facilities_by_player.clone()),
            owned_facilities_by_faction: include_facilities
                .then(|| snapshot.world.state.owned_facilities_by_faction.clone()),
            station_passengers: include_observations
                .then(|| snapshot.world.state.station_passengers.clone()),
            salvage_by_poi: include_observations
                .then(|| snapshot.world.state.salvage_by_poi.clone()),
            agent_sightings: include_observations
                .then(|| snapshot.world.state.agent_sightings.clone()),
            chat_messages_by_session: include_communications
                .then(|| snapshot.world.state.chat_messages_by_session.clone()),
            faction_by_session: include_factions
                .then(|| snapshot.world.state.faction_by_session.clone()),
            updated_at_utc: projected.updated_at_utc,
        })
    } else {
        None
    };
    let catalog = if query.catalog_version.as_deref() == Some(catalog_version.as_str()) {
        None
    } else {
        Some(prayer_sdk::state_mapping::map_galaxy_catalog(
            snapshot.world.state.catalog.as_ref(),
        ))
    };

    let response = V1StateResponse {
        versions: V1StateVersions {
            fleet: fleet_version,
            world: world_version,
            map: domain_versions.map,
            resources: domain_versions.resources,
            wildlife: domain_versions.wildlife,
            markets: domain_versions.markets,
            storage: domain_versions.storage,
            facilities: domain_versions.facilities,
            observations: domain_versions.observations,
            communications: domain_versions.communications,
            factions: domain_versions.factions,
            catalog: catalog_version,
        },
        fleet,
        world,
        catalog,
    };

    let fleet_included = response.fleet.is_some();
    let catalog_included = response.catalog.is_some();
    let map_included = response
        .world
        .as_ref()
        .is_some_and(|world| world.map.is_some());
    let resources_included = response
        .world
        .as_ref()
        .is_some_and(|world| world.resources.is_some());
    let wildlife_included = response
        .world
        .as_ref()
        .is_some_and(|world| world.wildlife.is_some());
    let storage_included = response
        .world
        .as_ref()
        .is_some_and(|world| world.storage_by_player.is_some());
    let (world_mode, market_mode, market_upserts, market_removes) = match &response.world {
        None => ("unchanged", "none", 0, 0),
        Some(world) => match (&world.station_markets, &world.station_market_delta) {
            (Some(markets), _) => ("included", "full", markets.len(), 0),
            (_, Some(delta)) => ("included", "delta", delta.upsert.len(), delta.remove.len()),
            _ => ("included", "none", 0, 0),
        },
    };
    let build_ms = started.elapsed().as_millis().saturating_sub(snapshot_ms);
    let serialize_started = Instant::now();
    let body = serde_json::to_vec(&response)
        .map_err(|error| V1Error::Internal(format!("state serialization failed: {error}")))?;
    let serialize_ms = serialize_started.elapsed().as_millis();

    tracing::info!(
        payload_bytes = body.len(),
        total_ms = started.elapsed().as_millis(),
        snapshot_ms,
        build_ms,
        serialize_ms,
        fleet_version,
        world_version,
        fleet_included,
        world_mode,
        catalog_included,
        map_included,
        resources_included,
        wildlife_included,
        storage_included,
        market_mode,
        market_upserts,
        market_removes,
        "v1 state response"
    );

    Ok(([("content-type", "application/json")], body).into_response())
}

async fn select_routes(
    State(state): State<V1State>,
    Json(request): Json<RouteBatchRequest>,
) -> Result<Json<RouteBatchResponse>, V1Error> {
    if request.routes.len() > 100_000 {
        return Err(V1Error::Validation(
            "route query accepts at most 100000 routes".into(),
        ));
    }
    let routes = state
        .sdk
        .routes(
            &request.routes,
            prayer_sdk::RouteOptions { safe: request.safe },
        )
        .await;
    Ok(Json(RouteBatchResponse { routes }))
}

async fn list_bots(State(state): State<V1State>) -> Result<Json<Vec<V1BotSummary>>, V1Error> {
    let out = state
        .sdk
        .host_bot_snapshots()
        .await
        .into_iter()
        .map(|snapshot| bot_summary(snapshot.id.to_string(), snapshot))
        .collect();
    Ok(Json(out))
}

async fn register_bot(
    State(state): State<V1State>,
    Json(request): Json<RegisterBotRequest>,
) -> Result<Json<RegisterBotResponse>, V1Error> {
    let username = request.username.trim().to_string();
    let empire = request.empire.trim().to_string();
    if username.is_empty() || empire.is_empty() {
        return Err(V1Error::from(SdkError::BadRequest(
            "username and empire are required".to_string(),
        )));
    }
    let registration_code = request.registration_code.trim().to_string();
    if registration_code.is_empty() {
        return Err(V1Error::from(SdkError::BadRequest(
            "registration code is required".to_string(),
        )));
    }
    let (bot, registration) = state
        .sdk
        .register_bot(username, empire, registration_code)
        .await?;
    let snapshot = bot.host_state().await?;
    Ok(Json(RegisterBotResponse {
        bot: bot_summary(bot.id().to_string(), snapshot),
        player_id: registration.player_id,
        password: registration.password,
    }))
}

async fn get_bot(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
) -> Result<Json<V1BotSummary>, V1Error> {
    let bot = state.sdk.bot(bot_id).await?;
    Ok(Json(bot_summary(
        bot.id().to_string(),
        bot.host_state().await?,
    )))
}

fn bot_summary(bot_id: String, snapshot: prayer_state::FleetEntry) -> V1BotSummary {
    V1BotSummary {
        bot_id,
        name: snapshot.username.clone(),
        connection: snapshot.connection.into(),
        state_version: snapshot.version,
        observed_at: snapshot.observed_at,
    }
}

async fn get_queue(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
) -> Result<Json<crate::contracts::V1QueueResponse>, V1Error> {
    let bot = state.sdk.bot(bot_id).await?;
    let queue = bot.queue().await?;
    let script_execution = bot.script_execution().await?;
    let prayerlang = queue.rendered_prayerlang().to_owned();
    Ok(Json(crate::contracts::V1QueueResponse {
        scheduler: queue,
        prayerlang,
        script_execution,
    }))
}

async fn get_normal_queue(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
) -> Result<Json<prayer_sdk::QueueLaneSnapshot>, V1Error> {
    Ok(Json(state.sdk.bot(bot_id).await?.normal_queue().await?))
}

async fn get_override_queue(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
) -> Result<Json<prayer_sdk::QueueLaneSnapshot>, V1Error> {
    Ok(Json(state.sdk.bot(bot_id).await?.override_queue().await?))
}

async fn halt_bot(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
    body: Option<Json<V1CancelRequest>>,
) -> Result<StatusCode, V1Error> {
    let bot = state.sdk.bot(bot_id).await?;
    bot.halt(body.and_then(|Json(body)| body.reason)).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn idempotency_key(headers: &HeaderMap, body: Option<&str>) -> Result<Option<String>, V1Error> {
    let header = headers
        .get("idempotency-key")
        .map(|value| value.to_str().map(|value| value.trim().to_owned()))
        .transpose()
        .map_err(|_| V1Error::Validation("Idempotency-Key must be valid ASCII".into()))?;
    let body = body.map(str::trim).map(str::to_owned);
    if header.as_deref().is_some_and(str::is_empty) || body.as_deref().is_some_and(str::is_empty) {
        return Err(V1Error::Validation(
            "Idempotency-Key must not be blank".into(),
        ));
    }
    if header.is_some() && body.is_some() && header != body {
        return Err(V1Error::IdempotencyConflict);
    }
    Ok(header.or(body))
}

async fn start_action_run(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<V1ActionRunRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<V1ActionRunResponse>), V1Error> {
    let Json(request) = body.map_err(|error| V1Error::Validation(error.body_text()))?;
    if request.actions.is_empty() || request.actions.len() > MAX_ACTIONS {
        return Err(V1Error::Validation(format!(
            "actions must contain 1 to {MAX_ACTIONS} entries"
        )));
    }
    let key = idempotency_key(&headers, request.idempotency_key.as_deref())?;
    let fingerprint = serde_json::to_string(&request.actions)
        .map_err(|error| V1Error::Internal(error.to_string()))?;
    let idempotency_key = key.map(|key| (bot_id.clone(), key, "action"));
    let _flight = match idempotency_key.as_ref() {
        Some(key) => Some(state.idempotency.key_guard(key).await),
        None => None,
    };
    if let Some(key) = idempotency_key.as_ref() {
        if let Some(existing) = state.idempotency.get(key).await {
            if existing.fingerprint != fingerprint {
                return Err(V1Error::IdempotencyConflict);
            }
            let bot = state.sdk.bot(bot_id).await?;
            let run = bot.action_run(existing.run_id).await?;
            return Ok((
                StatusCode::OK,
                Json(action_run_response(bot.id().to_string(), &run).await?),
            ));
        }
    }
    let actions = request
        .actions
        .into_iter()
        .map(Action::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let bot = state.sdk.bot(bot_id.clone()).await?;
    let run = bot.start_actions(actions).await?;
    if let Some(key) = idempotency_key {
        state
            .idempotency
            .insert_and_persist(
                key,
                IdempotencyRecord {
                    fingerprint,
                    run_id: run.id().clone(),
                },
            )
            .await?;
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(action_run_response(bot.id().to_string(), &run).await?),
    ))
}

async fn execute_action_override(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
    body: Result<Json<V1ActionOverrideRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<V1OverrideResponse>), V1Error> {
    let Json(request) = body.map_err(|error| V1Error::Validation(error.body_text()))?;
    if request.actions.is_empty() || request.actions.len() > MAX_ACTIONS {
        return Err(V1Error::Validation(format!(
            "actions must contain 1 to {MAX_ACTIONS} entries"
        )));
    }
    let actions = request
        .actions
        .into_iter()
        .map(Action::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    state
        .sdk
        .bot(bot_id)
        .await?
        .execute_action_override(
            actions,
            prayer_sdk::OverrideOptions {
                return_to_origin: request.return_to_origin,
            },
        )
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(V1OverrideResponse { accepted: true }),
    ))
}

async fn execute_script_override(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
    body: Result<Json<V1ScriptOverrideRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<V1OverrideResponse>), V1Error> {
    let Json(request) = body.map_err(|error| V1Error::Validation(error.body_text()))?;
    if request.script.trim().is_empty() {
        return Err(V1Error::Validation("script must not be blank".into()));
    }
    state
        .sdk
        .bot(bot_id)
        .await?
        .execute_script_override(
            request.script,
            prayer_sdk::OverrideOptions {
                return_to_origin: request.return_to_origin,
            },
        )
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(V1OverrideResponse { accepted: true }),
    ))
}

async fn get_action_run(
    State(state): State<V1State>,
    Path((bot_id, run_id)): Path<(String, String)>,
) -> Result<Json<V1ActionRunResponse>, V1Error> {
    let bot = state.sdk.bot(bot_id).await?;
    let run = bot.action_run(RunId(run_id)).await?;
    Ok(Json(action_run_response(bot.id().to_string(), &run).await?))
}

async fn cancel_action_run(
    State(state): State<V1State>,
    Path((bot_id, run_id)): Path<(String, String)>,
    body: Option<Json<V1CancelRequest>>,
) -> Result<Json<V1ActionRunResponse>, V1Error> {
    let bot = state.sdk.bot(bot_id).await?;
    let run = bot.action_run(RunId(run_id)).await?;
    let _ = run
        .cancel(
            body.and_then(|Json(body)| body.reason)
                .unwrap_or_else(|| "cancelled by HTTP client".into()),
        )
        .await?;
    Ok(Json(action_run_response(bot.id().to_string(), &run).await?))
}

async fn action_run_response(
    bot_id: String,
    run: &ActionRunHandle,
) -> Result<V1ActionRunResponse, V1Error> {
    let identity = |run_version| V1RunIdentity {
        run_id: run.id().0.clone(),
        bot_id: bot_id.clone(),
        run_version,
        prayerlang: run.prayerlang().into(),
    };
    Ok(match run.status().await? {
        RunStatus::Running => V1ActionRunResponse::Running { run: identity(1) },
        RunStatus::Terminal(outcome) => match outcome {
            prayer_sdk::ActionRunOutcome::Succeeded => V1ActionRunResponse::Succeeded {
                run: identity(2),
                outcome,
            },
            prayer_sdk::ActionRunOutcome::Failed { .. } => V1ActionRunResponse::Failed {
                run: identity(2),
                outcome,
            },
            prayer_sdk::ActionRunOutcome::Cancelled { .. } => V1ActionRunResponse::Cancelled {
                run: identity(2),
                outcome,
            },
            prayer_sdk::ActionRunOutcome::Halted { .. } => V1ActionRunResponse::Halted {
                run: identity(2),
                outcome,
            },
        },
    })
}

async fn start_script_run(
    State(state): State<V1State>,
    Path(bot_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<V1ScriptRunRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<V1ScriptRunResponse>), V1Error> {
    let Json(request) = body.map_err(|error| V1Error::Validation(error.body_text()))?;
    if request.script.trim().is_empty() {
        return Err(V1Error::Validation("script must not be empty".into()));
    }
    let key = idempotency_key(&headers, request.idempotency_key.as_deref())?;
    let fingerprint = request.script.clone();
    let idempotency_key = key.map(|key| (bot_id.clone(), key, "script"));
    let _flight = match idempotency_key.as_ref() {
        Some(key) => Some(state.idempotency.key_guard(key).await),
        None => None,
    };
    if let Some(key) = idempotency_key.as_ref() {
        if let Some(existing) = state.idempotency.get(key).await {
            if existing.fingerprint != fingerprint {
                return Err(V1Error::IdempotencyConflict);
            }
            let bot = state.sdk.bot(bot_id).await?;
            let run = bot.script_run(existing.run_id).await?;
            return Ok((
                StatusCode::OK,
                Json(script_run_response(bot.id().to_string(), &run).await?),
            ));
        }
    }
    let bot = state.sdk.bot(bot_id.clone()).await?;
    let run = bot.start_script(request.script).await?;
    if let Some(key) = idempotency_key {
        state
            .idempotency
            .insert_and_persist(
                key,
                IdempotencyRecord {
                    fingerprint,
                    run_id: run.id().clone(),
                },
            )
            .await?;
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(script_run_response(bot.id().to_string(), &run).await?),
    ))
}

async fn get_script_run(
    State(state): State<V1State>,
    Path((bot_id, run_id)): Path<(String, String)>,
) -> Result<Json<V1ScriptRunResponse>, V1Error> {
    let bot = state.sdk.bot(bot_id).await?;
    let run = bot.script_run(RunId(run_id)).await?;
    Ok(Json(script_run_response(bot.id().to_string(), &run).await?))
}

async fn cancel_script_run(
    State(state): State<V1State>,
    Path((bot_id, run_id)): Path<(String, String)>,
    body: Option<Json<V1CancelRequest>>,
) -> Result<Json<V1ScriptRunResponse>, V1Error> {
    let bot = state.sdk.bot(bot_id).await?;
    let run = bot.script_run(RunId(run_id)).await?;
    let _ = run
        .cancel(
            body.and_then(|Json(body)| body.reason)
                .unwrap_or_else(|| "cancelled by HTTP client".into()),
        )
        .await?;
    Ok(Json(script_run_response(bot.id().to_string(), &run).await?))
}

async fn script_run_response(
    bot_id: String,
    run: &ScriptRunHandle,
) -> Result<V1ScriptRunResponse, V1Error> {
    let identity = |run_version| V1RunIdentity {
        run_id: run.id().0.clone(),
        bot_id: bot_id.clone(),
        run_version,
        prayerlang: run.prayerlang().into(),
    };
    Ok(match run.status().await? {
        RunStatus::Running => V1ScriptRunResponse::Running { run: identity(1) },
        RunStatus::Terminal(outcome) => match &outcome {
            prayer_sdk::ScriptRunOutcome::Success { .. } => V1ScriptRunResponse::Succeeded {
                run: identity(2),
                outcome,
            },
            prayer_sdk::ScriptRunOutcome::Error { kind, .. } => match kind {
                prayer_sdk::ScriptErrorKind::Cancelled => V1ScriptRunResponse::Cancelled {
                    run: identity(2),
                    outcome,
                },
                prayer_sdk::ScriptErrorKind::UserHalt | prayer_sdk::ScriptErrorKind::Shutdown => {
                    V1ScriptRunResponse::Halted {
                        run: identity(2),
                        outcome,
                    }
                }
                _ => V1ScriptRunResponse::Failed {
                    run: identity(2),
                    outcome,
                },
            },
        },
    })
}

impl TryFrom<V1ActionRequest> for Action {
    type Error = V1Error;
    fn try_from(value: V1ActionRequest) -> Result<Self, Self::Error> {
        Ok(match value.0 {
            Action::Wait { ticks: 0 } => {
                return Err(V1Error::Validation(
                    "wait ticks must be greater than zero".into(),
                ))
            }
            action => action,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_keys_are_trimmed_and_blank_values_are_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert("idempotency-key", " durable-key ".parse().unwrap());
        assert_eq!(
            idempotency_key(&headers, Some("durable-key")).unwrap(),
            Some("durable-key".into())
        );
        headers.insert("idempotency-key", "   ".parse().unwrap());
        assert!(matches!(
            idempotency_key(&headers, None),
            Err(V1Error::Validation(_))
        ));
    }
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use spacemolt_lib_rs::SpacemoltClient;
    use tower::ServiceExt;

    async fn test_sdk() -> Arc<PrayerSdk> {
        let options = prayer_sdk::options_from_client(
            Arc::new(SpacemoltClient::default()),
            "https://game.spacemolt.com",
        );
        let root =
            std::path::PathBuf::from("/tmp").join(format!("prayer-api-v1-{}", Uuid::new_v4()));
        let mut runtime = prayer_sdk::RuntimeServiceOptions::default();
        runtime.knowledge_state_path = root.join("knowledge.json");
        runtime.session_state_path = root.join("sessions.json");
        runtime.local_auth_bypass = true;
        let options = prayer_sdk::with_runtime_options(options, runtime);
        let sdk = Arc::new(prayer_sdk::sdk_from_options(options));
        sdk.inject_test_bot("bot-1", "Miner").await.expect("bot");
        sdk
    }

    async fn json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        serde_json::from_slice(&bytes).expect("JSON")
    }

    #[tokio::test]
    async fn metadata_and_structured_not_found_are_stable() {
        let app = build_v1_router(test_sdk().await);
        let meta = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/meta")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(meta.status(), StatusCode::OK);
        assert!(meta.headers().contains_key("x-request-id"));
        assert_eq!(json(meta).await["apiVersion"], "1.0");

        let missing = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/bots/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert!(missing.headers().contains_key("x-request-id"));
        assert_eq!(json(missing).await["error"]["code"], "bot_not_found");
    }

    #[tokio::test]
    async fn bulk_routes_return_one_result_per_query() {
        let response = build_v1_router(test_sdk().await)
            .oneshot(
                Request::post("/api/v1/routes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"routes":[{"from":"missing-a","to":"missing-b"}],"safe":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json(response).await, json!({ "routes": [null] }));
    }

    #[tokio::test]
    async fn aggregate_state_is_conditionally_versioned_and_replaces_split_routes() {
        let app = build_v1_router(test_sdk().await);
        let initial = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/state")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(initial.status(), StatusCode::OK);
        let initial = json(initial).await;
        assert!(initial["fleet"].is_object());
        assert!(initial["world"].is_object());
        assert!(initial["world"]["stationMarkets"].is_object());
        assert!(initial["catalog"].is_object());

        let uri = format!(
            "/api/v1/state?fleet_version={}&world_version={}&catalog_version={}",
            initial["versions"]["fleet"].as_u64().unwrap(),
            initial["versions"]["world"].as_u64().unwrap(),
            initial["versions"]["catalog"].as_str().unwrap(),
        );
        let unchanged = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let unchanged = json(unchanged).await;
        assert!(unchanged["fleet"].is_null());
        assert!(unchanged["world"].is_null());
        assert!(unchanged["catalog"].is_null());

        let uri = format!(
            "/api/v1/state?fleet_version={}&map_version=999&resources_version={}&wildlife_version={}&markets_version={}&storage_version={}&catalog_version={}",
            initial["versions"]["fleet"].as_u64().unwrap(),
            initial["versions"]["resources"].as_u64().unwrap(),
            initial["versions"]["wildlife"].as_u64().unwrap(),
            initial["versions"]["markets"].as_u64().unwrap(),
            initial["versions"]["storage"].as_u64().unwrap(),
            initial["versions"]["catalog"].as_str().unwrap(),
        );
        let map_only = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let map_only = json(map_only).await;
        assert!(map_only["world"]["map"].is_object());
        assert!(map_only["world"].get("resources").is_none());
        assert!(map_only["world"].get("exploration").is_none());
        assert!(map_only["world"].get("wildlife").is_none());
        assert!(map_only["world"].get("stationMarkets").is_none());
        assert!(map_only["world"].get("storageByPlayer").is_none());

        let removed = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/galaxy/map")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(removed.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn station_market_delta_upserts_changes_and_removes_missing_stations() {
        let unchanged = prayer_state::StationMarketData {
            observed_at_unix: Some(1),
            ..Default::default()
        };
        let changed = prayer_state::StationMarketData {
            observed_at_unix: Some(2),
            ..Default::default()
        };
        let previous = HashMap::from([
            ("stable".to_string(), unchanged.clone()),
            ("changed".to_string(), unchanged.clone()),
            ("removed".to_string(), unchanged.clone()),
        ]);
        let current = HashMap::from([
            ("stable".to_string(), unchanged),
            ("changed".to_string(), changed.clone()),
            ("added".to_string(), changed),
        ]);

        let delta = station_market_delta(7, &previous, &current);
        assert_eq!(delta.base_version, 7);
        assert_eq!(delta.upsert.len(), 2);
        assert!(delta.upsert.contains_key("changed"));
        assert!(delta.upsert.contains_key("added"));
        assert!(!delta.upsert.contains_key("stable"));
        assert_eq!(delta.remove, vec!["removed"]);
    }

    #[test]
    fn fleet_version_distinguishes_empty_fleet_from_version_zero_bot() {
        let empty = fleet_version_for(std::iter::empty());
        let connected = fleet_version_for([("bot-1", 0)]);

        assert_ne!(empty, connected);
    }

    #[test]
    fn fleet_version_tracks_membership_and_bot_revisions_independent_of_order() {
        let both = fleet_version_for([("bot-a", 7), ("bot-b", 2)]);
        let reordered = fleet_version_for([("bot-b", 2), ("bot-a", 7)]);
        let removed = fleet_version_for([("bot-a", 7)]);
        let updated = fleet_version_for([("bot-a", 8), ("bot-b", 2)]);

        assert_eq!(both, reordered);
        assert_ne!(both, removed);
        assert_ne!(both, updated);
    }

    #[test]
    fn one_station_delta_is_materially_smaller_than_full_market_state() {
        let previous = (0..100)
            .map(|index| {
                (
                    format!("station-{index}"),
                    prayer_state::StationMarketData {
                        observed_at_unix: Some(1),
                        ..Default::default()
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let mut current = previous.clone();
        current.get_mut("station-42").unwrap().observed_at_unix = Some(2);
        let delta = station_market_delta(1, &previous, &current);
        let delta_bytes = serde_json::to_vec(&delta).unwrap().len();
        let full_bytes = serde_json::to_vec(&current).unwrap().len();
        assert!(
            delta_bytes * 4 < full_bytes,
            "delta={delta_bytes} full={full_bytes}"
        );
    }

    #[tokio::test]
    async fn invalid_action_json_uses_the_structured_error_contract() {
        let app = build_v1_router(test_sdk().await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/bots/bot-1/action-runs")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"actions":[{"type":"teleport"}]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json(response).await["error"]["code"], "validation");
    }

    #[test]
    fn v1_accepts_the_complete_canonical_action_contract() {
        let attack: V1ActionRequest = serde_json::from_value(json!({
            "type": "attack", "request": { "target_id": "pirate-1" }
        }))
        .expect("canonical attack");
        assert_eq!(
            Action::try_from(attack).expect("action"),
            Action::Attack {
                target_id: "pirate-1".into()
            }
        );

        let transfer: V1ActionRequest = serde_json::from_value(json!({
            "type": "transfer",
            "request": { "subject": { "kind": "all_cargo" }, "from": { "kind": "cargo" }, "to": { "kind": "storage" } }
        })).expect("canonical transfer");
        assert!(matches!(
            Action::try_from(transfer),
            Ok(Action::Transfer(_))
        ));
    }

    #[test]
    fn idempotency_index_round_trips_atomically() {
        let path = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-v1-idempotency-{}.json", Uuid::new_v4()));
        let mut index = HashMap::new();
        index.insert(
            ("bot".into(), "key".into(), "action"),
            IdempotencyRecord {
                fingerprint: "digest".into(),
                run_id: RunId("run".into()),
            },
        );
        persist_idempotency(&path, &index).expect("persist");
        let restored = load_idempotency(&path);
        assert_eq!(
            restored
                .get(&("bot".into(), "key".into(), "action"))
                .expect("record")
                .run_id,
            RunId("run".into())
        );
    }

    #[tokio::test]
    async fn idempotency_single_flight_allows_unrelated_keys_to_overlap() {
        let path = std::path::PathBuf::from("/tmp").join(format!(
            "prayer-v1-idempotency-overlap-{}.json",
            Uuid::new_v4()
        ));
        let store = IdempotencyStore::new(path);
        let first = ("bot-a".into(), "key-a".into(), "script");
        let second = ("bot-b".into(), "key-b".into(), "script");
        let _first_guard = store.key_guard(&first).await;

        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            store.key_guard(&second),
        )
        .await
        .expect("an unrelated key must not wait for the first key");
    }

    #[tokio::test]
    async fn idempotency_single_flight_serializes_the_same_key() {
        let path = std::path::PathBuf::from("/tmp").join(format!(
            "prayer-v1-idempotency-serial-{}.json",
            Uuid::new_v4()
        ));
        let store = IdempotencyStore::new(path);
        let key = ("bot-a".into(), "same-key".into(), "script");
        let first_guard = store.key_guard(&key).await;
        let waiting = {
            let store = store.clone();
            let key = key.clone();
            tokio::spawn(async move { store.key_guard(&key).await })
        };

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !waiting.is_finished(),
            "same-key waiter must remain blocked"
        );
        drop(first_guard);
        tokio::time::timeout(std::time::Duration::from_millis(100), waiting)
            .await
            .expect("same-key waiter should proceed after release")
            .expect("same-key task should succeed");
    }

    #[tokio::test]
    async fn concurrent_idempotency_persistence_keeps_the_latest_complete_snapshot() {
        let path = std::path::PathBuf::from("/tmp").join(format!(
            "prayer-v1-idempotency-concurrent-{}.json",
            Uuid::new_v4()
        ));
        let store = IdempotencyStore::new(path.clone());
        let mut inserts = tokio::task::JoinSet::new();
        for index in 0..8 {
            let store = store.clone();
            inserts.spawn(async move {
                store
                    .insert_and_persist(
                        (format!("bot-{index}"), format!("key-{index}"), "script"),
                        IdempotencyRecord {
                            fingerprint: format!("fingerprint-{index}"),
                            run_id: RunId(format!("run-{index}")),
                        },
                    )
                    .await
            });
        }
        while let Some(result) = inserts.join_next().await {
            result
                .expect("idempotency task")
                .expect("persist idempotency record");
        }

        assert_eq!(load_idempotency(&path).len(), 8);
    }

    #[test]
    fn v1_handlers_do_not_reach_runtime_service_or_spacemolt() {
        let source = include_str!("v1.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("RuntimeService"));
        assert!(!production.contains("spacemolt_client"));
        assert!(!production.contains(".service()"));
    }
}
