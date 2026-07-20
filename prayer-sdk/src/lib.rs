//! Embeddable Prayer application facade.
//!
//! This crate owns in-process Prayer lifecycle and coordination. HTTP hosts
//! should adapt their wire contracts to this API instead of reaching into the
//! runtime or SpaceMolt account transport directly.
//!
//! The normal path is [`PrayerSdk::connect`] → [`PrayerSdk::bot`] → typed
//! actions or PrayerLang runs. See the `bot_bootstrap` and `run_handles`
//! examples for complete lifecycle and recovery flows. Common imports are
//! collected in [`prelude`]. HTTP wire records live in `prayer-api-contracts`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub use prayer_actions::{
    Action, BuyRequest, CommissionShipRequest, CraftRequest, FacilityAccessRequest,
    FacilityNameRequest, FacilityOutputPriceRequest, FacilityUpgradeRequest, FindRequest, GoTarget,
    ItemId, RecycleRequest, RunId, SayRequest, SellRequest, ServiceTransferRequest, TradeItem,
    TradeOfferRequest, TransferEndpoint, TransferItem, TransferRequest, TransferSubject,
};
use prayer_actions::{ActionEnvelope, ActionOrigin};
use prayer_runtime::execution::ActionBatchOutcome;
#[cfg(test)]
use prayer_runtime::execution::PersistedActionRun;
use prayer_runtime::operation_failure::OperationFailure;
use prayer_scheduler::{QueueClaim, QueueOwner};
use prayer_state::BotId;
use serde::{Deserialize, Serialize};
use spacemolt_lib_rs::auth::MemoryCredentialStore;
pub use spacemolt_lib_rs::{SpacemoltClient, SpacemoltClientOptions};
use thiserror::Error;
use uuid::Uuid;

#[doc(hidden)]
pub mod administration;
#[doc(hidden)]
#[allow(dead_code)]
mod service;
#[doc(hidden)]
pub mod spacemolt_origin;
#[doc(hidden)]
pub mod spacemolt_projection;
#[doc(hidden)]
pub mod state_mapping;

pub use administration::PrayerAdministration;
use prayer_api_contracts::*;
use service::RuntimeService;
pub use service::RuntimeServiceOptions;

/// Runtime-backed game state exposed directly while the SDK is alpha.
pub type BotState = prayer_state::BotState;
/// Runtime-backed catalog exposed directly while the SDK is alpha.
pub type Catalog = prayer_state::CatalogData;
/// Runtime-backed galaxy knowledge exposed directly while the SDK is alpha.
pub type Galaxy = prayer_state::GalaxyData;
/// Runtime-backed observed fleet entry exposed directly while the SDK is alpha.
pub type BotStateEntry = prayer_state::FleetEntry;
/// Runtime-backed station market exposed directly while the SDK is alpha.
pub type StationMarket = prayer_state::StationMarketData;

/// Execution semantics for a client-submitted override.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct OverrideOptions {
    #[serde(default)]
    pub return_to_origin: bool,
}

/// Terminal result of a submitted typed-action batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActionRunOutcome {
    Succeeded,
    Failed {
        action_index: usize,
        message: String,
    },
    Cancelled {
        reason: String,
    },
    Halted {
        reason: String,
    },
}
/// Terminal result of a submitted PrayerLang script.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ScriptRunOutcome {
    Success {
        message: Option<String>,
    },
    Error {
        kind: ScriptErrorKind,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScriptErrorKind {
    Runtime,
    UserHalt,
    Cancelled,
    Replaced,
    Shutdown,
    RunnerExited,
    Internal,
}

impl From<ActionBatchOutcome> for ActionRunOutcome {
    fn from(value: ActionBatchOutcome) -> Self {
        match value {
            ActionBatchOutcome::Succeeded => Self::Succeeded,
            ActionBatchOutcome::Failed {
                action_index,
                message,
            } => Self::Failed {
                action_index,
                message,
            },
            ActionBatchOutcome::Cancelled { reason } => Self::Cancelled { reason },
            ActionBatchOutcome::Halted { reason } => Self::Halted { reason },
        }
    }
}

impl From<ScriptOutcomeDto> for ScriptRunOutcome {
    fn from(value: ScriptOutcomeDto) -> Self {
        match value {
            ScriptOutcomeDto::Success { message } => Self::Success { message },
            ScriptOutcomeDto::Error { kind, message } => Self::Error {
                kind: kind.into(),
                message,
            },
        }
    }
}

impl From<ScriptErrorKindDto> for ScriptErrorKind {
    fn from(value: ScriptErrorKindDto) -> Self {
        match value {
            ScriptErrorKindDto::Runtime => Self::Runtime,
            ScriptErrorKindDto::UserHalt => Self::UserHalt,
            ScriptErrorKindDto::Cancelled => Self::Cancelled,
            ScriptErrorKindDto::Replaced => Self::Replaced,
            ScriptErrorKindDto::Shutdown => Self::Shutdown,
            ScriptErrorKindDto::RunnerExited => Self::RunnerExited,
            ScriptErrorKindDto::Internal => Self::Internal,
        }
    }
}

/// Common application-facing imports.
pub mod prelude {
    pub use crate::{
        Action, ActionRunHandle, ActionRunOutcome, BotHandle, BotSelector, BotState, BotStateEntry,
        Catalog, Galaxy, GoTarget, LaneOwner, PrayerSdk, PrayerSdkOptions, PrayerState,
        QueueSnapshot, RunId, RunStatus, ScriptErrorKind, ScriptRunHandle, ScriptRunOutcome,
        SdkClientError, SdkError, SdkExecutionError, StartupAccountStatus, StartupReport,
        StationMarket, WaitOptions,
    };
}

pub fn options_from_client(
    client: std::sync::Arc<SpacemoltClient>,
    origin: impl Into<String>,
) -> PrayerSdkOptions {
    PrayerSdkOptions::new(client, origin)
}

pub fn options_from_client_options(
    options: SpacemoltClientOptions,
    origin: impl Into<String>,
) -> PrayerSdkOptions {
    PrayerSdkOptions::from_client_options(options, origin)
}

pub fn with_runtime_options(
    mut options: PrayerSdkOptions,
    runtime: RuntimeServiceOptions,
) -> PrayerSdkOptions {
    options.runtime = runtime;
    options
}

pub fn with_persistence_paths(
    mut options: PrayerSdkOptions,
    knowledge: impl Into<std::path::PathBuf>,
    sessions: impl Into<std::path::PathBuf>,
) -> PrayerSdkOptions {
    options.runtime.knowledge_state_path = knowledge.into();
    options.runtime.session_state_path = sessions.into();
    options
}

/// Constructs an embedding-host SDK without application bootstrap.
pub fn sdk_from_options(options: PrayerSdkOptions) -> PrayerSdk {
    PrayerSdk::new(options)
}

/// Restores persisted sessions for a host-managed SDK.
pub async fn restore(sdk: &PrayerSdk) -> Result<(), SdkError> {
    sdk.restore().await
}

/// Starts refresh workers for a host-managed SDK.
pub fn start_background_workers(sdk: &PrayerSdk) {
    sdk.start_background_workers();
}

/// Stable identifier for an in-process Prayer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
struct SessionId(Uuid);

impl SessionId {
    /// Creates a fresh session identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the underlying UUID for persistence and adapter conversion.
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for SessionId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<SessionId> for Uuid {
    fn from(value: SessionId) -> Self {
        value.0
    }
}

/// Construction inputs supplied by an embedding host.
#[derive(Clone)]
pub struct PrayerSdkOptions {
    /// A fully configured SpaceMolt client. Authentication and reconnect
    /// behavior remain owned by `spacemolt-lib-rs`.
    client: Option<Arc<SpacemoltClient>>,
    /// Canonical upstream origin associated with the configured client.
    spacemolt_base_url: String,
    /// Persistence and runtime tuning supplied by the embedding host.
    runtime: service::RuntimeServiceOptions,
    clerk_api_key: Option<String>,
}

impl PrayerSdkOptions {
    /// Builds options from an already configured client.
    fn new(client: Arc<SpacemoltClient>, spacemolt_base_url: impl Into<String>) -> Self {
        Self {
            client: Some(client),
            spacemolt_base_url: spacemolt_base_url.into(),
            runtime: service::RuntimeServiceOptions::default(),
            clerk_api_key: None,
        }
    }

    /// Convenience constructor for hosts that own explicit client options.
    fn from_client_options(
        options: SpacemoltClientOptions,
        spacemolt_base_url: impl Into<String>,
    ) -> Self {
        Self::new(
            Arc::new(SpacemoltClient::new(
                options,
                MemoryCredentialStore::default(),
            )),
            spacemolt_base_url,
        )
    }

    /// Configures Clerk discovery for the normal one-call bootstrap path.
    pub fn with_clerk_api_key(mut self, clerk_api_key: impl Into<String>) -> Self {
        self.clerk_api_key = Some(clerk_api_key.into());
        self.client = None;
        self
    }

    /// Overrides the production SpaceMolt origin for tests and custom hosts.
    pub fn with_spacemolt_origin(mut self, origin: impl Into<String>) -> Self {
        self.spacemolt_base_url = crate::spacemolt_origin::normalize_origin(&origin.into());
        self
    }

    /// Stores all durable SDK state beneath one application-owned directory.
    pub fn with_state_directory(mut self, directory: impl AsRef<std::path::Path>) -> Self {
        let directory = directory.as_ref();
        self.runtime.knowledge_state_path = directory.join("knowledge.json");
        self.runtime.session_state_path = directory.join("sessions.json");
        self
    }
}

impl Default for PrayerSdkOptions {
    fn default() -> Self {
        Self {
            client: None,
            spacemolt_base_url: crate::spacemolt_origin::DEFAULT_SPACEMOLT_ORIGIN.to_string(),
            runtime: service::RuntimeServiceOptions::default(),
            clerk_api_key: None,
        }
    }
}

/// A stable bot id, player id, or unique username/label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BotSelector(String);

impl BotSelector {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Readiness result for one Clerk-owned SpaceMolt player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupAccountStatus {
    pub player_id: String,
    pub username: String,
    pub connected: bool,
    pub ready: bool,
    pub error: Option<String>,
}

/// Result of the initial owned-player discovery, connection, and cache hydration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupReport {
    pub accounts: Vec<StartupAccountStatus>,
}

/// Stable description of the owner of a bot's exclusive action lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaneOwner {
    PrayerLang,
    Controller { controller_kind: String },
    Manual,
}

impl From<&QueueOwner> for LaneOwner {
    fn from(owner: &QueueOwner) -> Self {
        match owner {
            QueueOwner::PrayerLang { .. } => Self::PrayerLang,
            QueueOwner::Controller { kind, .. } => Self::Controller {
                controller_kind: kind.clone(),
            },
            QueueOwner::Manual { .. } => Self::Manual,
        }
    }
}

/// Structured upstream/client failure without exposing runtime planner types.
#[derive(Debug, Error)]
#[error("{inner}")]
pub struct SdkClientError {
    inner: OperationFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkErrorDetails {
    pub server_code: Option<String>,
    pub upstream_message: Option<String>,
    pub retry_after_millis: Option<u64>,
}

/// Stable SDK execution error without exposing the runtime engine type.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct SdkExecutionError {
    message: String,
}

impl From<prayer_runtime::engine::EngineError> for SdkExecutionError {
    fn from(error: prayer_runtime::engine::EngineError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

impl SdkClientError {
    pub fn is_retryable(&self) -> bool {
        self.inner.is_transient()
    }

    pub fn is_network(&self) -> bool {
        self.inner.is_network()
    }

    pub fn server_code(&self) -> Option<&str> {
        self.inner.server_code()
    }

    pub fn upstream_message(&self) -> Option<&str> {
        self.inner.upstream_message()
    }

    pub fn retry_after(&self) -> Option<Duration> {
        self.inner.retry_after()
    }

    pub fn details(&self) -> SdkErrorDetails {
        SdkErrorDetails {
            server_code: self.server_code().map(str::to_owned),
            upstream_message: self.upstream_message().map(str::to_owned),
            retry_after_millis: self
                .retry_after()
                .map(|value| value.as_millis().min(u128::from(u64::MAX)) as u64),
        }
    }

    pub(crate) fn into_inner(self) -> OperationFailure {
        self.inner
    }

    pub(crate) fn as_inner(&self) -> &OperationFailure {
        &self.inner
    }
}

impl From<OperationFailure> for SdkClientError {
    fn from(inner: OperationFailure) -> Self {
        Self { inner }
    }
}

impl StartupReport {
    pub fn is_ready(&self) -> bool {
        self.accounts.iter().all(|account| account.ready)
    }
}

impl From<&str> for BotSelector {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for BotSelector {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<BotId> for BotSelector {
    fn from(value: BotId) -> Self {
        Self(value.as_str().to_string())
    }
}

/// Transport-neutral SDK failure.
#[derive(Debug, Error)]
pub enum SdkError {
    /// Session was not found.
    #[error("session not found")]
    SessionNotFound,
    /// No bot matched the supplied selector.
    #[error("bot not found: {selector}")]
    BotNotFound { selector: String },
    /// A mutable bot name matched more than one bot.
    #[error("bot selector is ambiguous: {selector}")]
    AmbiguousBot { selector: String },
    /// The SDK is shutting down and no longer accepts work.
    #[error("SDK shutdown is in progress")]
    ShutdownInProgress,
    /// The session's exclusive normal action lane already has an owner.
    #[error("session action lane is busy (generation {generation}, owner {owner:?})")]
    LaneBusy {
        owner: LaneOwner,
        run_id: RunId,
        generation: u64,
    },
    /// A durable run identifier is unknown for this bot.
    #[error("run not found: {run_id:?}")]
    RunNotFound { run_id: RunId },
    /// Waiting for a run exceeded the caller-supplied timeout.
    #[error("timed out waiting for run: {run_id:?}")]
    WaitTimedOut { run_id: RunId },
    /// A session identifier could not be parsed.
    #[error("invalid session id")]
    InvalidSessionId,
    /// The requested operation was invalid for the current domain state.
    #[error("{0}")]
    BadRequest(String),
    /// Invalid or unsupported runtime command/configuration.
    #[error("{0}")]
    Command(String),
    /// Runtime execution failed.
    #[error("runtime error: {0}")]
    Engine(#[from] SdkExecutionError),
    /// SpaceMolt communication failed.
    #[error("client error: {0}")]
    Client(#[from] SdkClientError),
    /// Persisted state could not be loaded or stored.
    #[error("persistence error: {0}")]
    InvalidRuntimeState(String),
}

impl From<spacemolt_lib_rs::ClientError> for SdkError {
    fn from(value: spacemolt_lib_rs::ClientError) -> Self {
        match value {
            spacemolt_lib_rs::ClientError::UnknownAction(action) => {
                Self::Command(format!("unknown action: {action}"))
            }
            spacemolt_lib_rs::ClientError::NotImplemented(feature) => {
                Self::Command(format!("not implemented yet: {feature}"))
            }
            other => Self::Client(OperationFailure::Client(other).into()),
        }
    }
}

impl From<OperationFailure> for SdkError {
    fn from(value: OperationFailure) -> Self {
        Self::Client(value.into())
    }
}

impl From<prayer_runtime::engine::EngineError> for SdkError {
    fn from(value: prayer_runtime::engine::EngineError) -> Self {
        Self::Engine(value.into())
    }
}

impl From<serde_json::Error> for SdkError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidRuntimeState(error.to_string())
    }
}

/// Root facade for embedding Prayer in a process.
pub struct PrayerSdk {
    service: Arc<RuntimeService>,
    client: Arc<SpacemoltClient>,
    #[cfg(test)]
    #[allow(dead_code)]
    spacemolt_base_url: String,
    startup_report: StartupReport,
    shutdown_gate: tokio::sync::Mutex<()>,
    shutdown_complete: AtomicBool,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
struct CreateSession {
    label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationResult {
    pub player_id: String,
    pub password: String,
}

#[derive(Clone)]
struct SdkSessionHandle {
    id: SessionId,
    service: Arc<RuntimeService>,
}

/// Bot-facing handle over the internal execution session.
pub struct BotHandle {
    bot_id: BotId,
    inner: SdkSessionHandle,
}

/// One immutable, internally consistent view of Prayer's maintained state.
///
/// Acquiring a snapshot is asynchronous; querying data already captured in it
/// is synchronous and never performs transport I/O or takes runtime locks.
#[derive(Debug, Clone)]
pub struct PrayerState {
    inner: prayer_state::StateSnapshot<RuntimeVirtualMarketOrderDto, RuntimeVirtualCraftOrderDto>,
}

impl PrayerState {
    /// Selects an observed bot by stable id or unique username.
    pub fn bot(&self, selector: impl Into<BotSelector>) -> Result<&BotStateEntry, SdkError> {
        let selector = selector.into();
        if let Some(bot) = self
            .inner
            .fleet
            .bots
            .values()
            .find(|bot| bot.id.as_str() == selector.as_str())
        {
            return Ok(bot);
        }

        let mut matches = self
            .inner
            .fleet
            .bots
            .values()
            .filter(|bot| bot.username.as_deref() == Some(selector.as_str()));
        let Some(bot) = matches.next() else {
            return Err(SdkError::BotNotFound {
                selector: selector.0,
            });
        };
        if matches.next().is_some() {
            return Err(SdkError::AmbiguousBot {
                selector: selector.0,
            });
        }
        Ok(bot)
    }

    /// Returns the remembered market book for a station/POI id.
    pub fn market(&self, station_id: &str) -> Option<&StationMarket> {
        self.inner.world.state.station_markets.get(station_id)
    }
}

impl std::ops::Deref for PrayerState {
    type Target =
        prayer_state::StateSnapshot<RuntimeVirtualMarketOrderDto, RuntimeVirtualCraftOrderDto>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus<T> {
    Running,
    Terminal(T),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WaitOptions {
    timeout: Option<Duration>,
}

impl WaitOptions {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub generation: u64,
    pub owner: Option<LaneOwner>,
    pub running_action: bool,
    pub pending_actions: usize,
    pub interrupt_active: bool,
    pub halted: bool,
    pub halt_reason: Option<String>,
    #[serde(skip)]
    prayerlang: String,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueueLaneSnapshot {
    pub active: bool,
    pub pending_actions: usize,
    pub prayerlang: String,
}

/// One cached galaxy route lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteQuery {
    pub from: String,
    pub to: String,
}

/// Controls how cached galaxy routes are selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteOptions {
    pub safe: bool,
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self { safe: true }
    }
}

/// An authoritative route selected from the server's cached all-pairs table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteSelection {
    pub from: String,
    pub from_system: String,
    pub to: String,
    pub to_system: String,
    pub hops: Vec<String>,
    pub total_jumps: usize,
    pub cost: usize,
    pub safe: bool,
}

impl QueueSnapshot {
    pub fn rendered_prayerlang(&self) -> &str {
        &self.prayerlang
    }
}

#[derive(Clone)]
pub struct ActionRunHandle {
    session_id: SessionId,
    run_id: RunId,
    service: Arc<RuntimeService>,
    prayerlang: String,
}

#[derive(Clone)]
pub struct ScriptRunHandle {
    session_id: SessionId,
    run_id: RunId,
    service: Arc<RuntimeService>,
    prayerlang: String,
}

#[cfg(test)]
type ActionRunSnapshot = PersistedActionRun;

struct ActionLane {
    session_id: SessionId,
    run_id: RunId,
    claim: QueueClaim,
    service: Arc<RuntimeService>,
}

impl PrayerSdk {
    /// Select one authoritative route from the cached all-pairs route table.
    pub async fn route(
        &self,
        from: impl Into<String>,
        to: impl Into<String>,
        options: RouteOptions,
    ) -> Option<RouteSelection> {
        self.routes(
            &[RouteQuery {
                from: from.into(),
                to: to.into(),
            }],
            options,
        )
        .await
        .into_iter()
        .next()
        .flatten()
    }

    /// Select many routes while sharing one immutable world snapshot and cache.
    pub async fn routes(
        &self,
        queries: &[RouteQuery],
        options: RouteOptions,
    ) -> Vec<Option<RouteSelection>> {
        let state = self.state().await;
        let galaxy = &state.world.state.galaxy;
        queries
            .iter()
            .map(|query| {
                let from_system = if galaxy.system_records.contains_key(&query.from) {
                    query.from.clone()
                } else {
                    galaxy.poi_records.get(&query.from)?.system_id.clone()
                };
                let to_system = if galaxy.system_records.contains_key(&query.to) {
                    query.to.clone()
                } else {
                    galaxy.poi_records.get(&query.to)?.system_id.clone()
                };
                let hops = galaxy.route_hops(&from_system, &to_system, options.safe)?;
                let cost = if options.safe {
                    galaxy.path_cost(&from_system, &to_system)?
                } else {
                    galaxy.hop_distance(&from_system, &to_system)?
                };
                Some(RouteSelection {
                    from: query.from.clone(),
                    from_system,
                    to: query.to.clone(),
                    to_system,
                    total_jumps: hops.len(),
                    hops,
                    cost,
                    safe: options.safe,
                })
            })
            .collect()
    }

    /// Register a new SpaceMolt bot and attach it to this Prayer process.
    pub async fn register_bot(
        &self,
        username: String,
        empire: String,
        registration_code: String,
    ) -> Result<(BotHandle, RegistrationResult), SdkError> {
        self.ensure_running()?;
        let (id, result) = self
            .service
            .register_spacemolt_account(spacemolt_lib_rs::RegisterParams {
                username,
                empire,
                registration_code,
            })
            .await?;
        let bot = self.bot(id.to_string()).await?;
        Ok((
            bot,
            RegistrationResult {
                player_id: result.player_id,
                password: result.password,
            },
        ))
    }

    /// Constructs the facade without reading process environment variables.
    fn new(options: PrayerSdkOptions) -> Self {
        let spacemolt_base_url = options.spacemolt_base_url;
        let client = options.client.unwrap_or_else(|| {
            let mut client_options = SpacemoltClientOptions::from_origin(&spacemolt_base_url);
            client_options.clerk_api_key = options.clerk_api_key;
            Arc::new(SpacemoltClient::new(
                client_options,
                MemoryCredentialStore::default(),
            ))
        });
        Self {
            service: Arc::new(RuntimeService::with_spacemolt_client(
                Arc::clone(&client),
                spacemolt_base_url.clone(),
                options.runtime,
            )),
            client,
            #[cfg(test)]
            spacemolt_base_url,
            startup_report: StartupReport::default(),
            shutdown_gate: tokio::sync::Mutex::new(()),
            shutdown_complete: AtomicBool::new(false),
        }
    }

    /// Performs the complete normal SDK bootstrap.
    pub async fn connect(options: PrayerSdkOptions) -> Result<Self, SdkError> {
        if options.client.is_none() && options.clerk_api_key.is_none() {
            return Err(SdkError::BadRequest(
                "PrayerSdkOptions requires with_clerk_api_key or an injected client".into(),
            ));
        }
        let mut sdk = Self::new(options);
        let owned_players = sdk.client.list_owned_players().await?;
        sdk.restore().await?;
        let mut accounts = Vec::with_capacity(owned_players.len());
        for player in owned_players {
            let status = match sdk.bot(player.id.as_str()).await {
                Ok(bot) => {
                    let snapshot = bot.state().await?;
                    let connected =
                        snapshot.connection == prayer_state::BotConnectionState::Connected;
                    let ready = connected && snapshot.observed_at.is_some();
                    StartupAccountStatus {
                        player_id: player.id,
                        username: player.username,
                        connected,
                        ready,
                        error: (!ready).then(|| {
                            if connected {
                                "initial state has not been observed".to_string()
                            } else {
                                "SpaceMolt account is disconnected".to_string()
                            }
                        }),
                    }
                }
                Err(SdkError::BotNotFound { .. }) => StartupAccountStatus {
                    player_id: player.id,
                    username: player.username,
                    connected: false,
                    ready: false,
                    error: Some("SpaceMolt account failed to connect".to_string()),
                },
                Err(error) => return Err(error),
            };
            accounts.push(status);
        }
        sdk.startup_report = StartupReport { accounts };
        Arc::clone(&sdk.service).start_idle_session_refresher();
        Ok(sdk)
    }

    pub fn startup_report(&self) -> &StartupReport {
        &self.startup_report
    }

    async fn restore(&self) -> Result<(), SdkError> {
        Arc::clone(&self.service)
            .restore_persisted_sessions_on_startup()
            .await;
        Ok(())
    }

    #[cfg(test)]
    async fn create_session(&self, request: CreateSession) -> Result<SessionId, SdkError> {
        self.ensure_running()?;
        self.service
            .create_session_with_label(request.label)
            .map(SessionId::from)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    async fn attach_account(
        &self,
        id: SessionId,
        account: spacemolt_lib_rs::Account,
        selector: impl Into<String>,
    ) -> Result<SessionSummary, SdkError> {
        self.ensure_running()?;
        let session = self.service.get_session(id.into_uuid()).await?;
        let mut session = session.lock().await;
        RuntimeService::install_spacemolt_account(
            &mut session,
            account,
            selector.into(),
            self.spacemolt_base_url.clone(),
        );
        drop(session);
        self.service
            .session_summary(&id.into_uuid().to_string())
            .await
    }

    #[cfg(test)]
    async fn session(&self, id: SessionId) -> Result<SdkSessionHandle, SdkError> {
        self.service.get_session(id.into_uuid()).await?;
        Ok(SdkSessionHandle {
            id,
            service: Arc::clone(&self.service),
        })
    }

    /// Returns an immutable cache-only fleet and world snapshot.
    pub async fn state(&self) -> PrayerState {
        PrayerState {
            inner: self.service.state_snapshot().await,
        }
    }

    /// Raw host projection input. Application code should use [`Self::state`].
    #[doc(hidden)]
    pub async fn host_state(
        &self,
    ) -> prayer_state::StateSnapshot<RuntimeVirtualMarketOrderDto, RuntimeVirtualCraftOrderDto>
    {
        self.service.state_snapshot().await
    }

    /// Resolves a stable bot id, username, or session label to its execution handle.
    pub async fn bot(&self, selector: impl Into<BotSelector>) -> Result<BotHandle, SdkError> {
        let selector = selector.into();
        let id = self
            .service
            .session_for_bot_selector(selector.as_str())
            .await
            .map_err(|error| match error {
                SdkError::SessionNotFound => SdkError::BotNotFound {
                    selector: selector.0.clone(),
                },
                other => other,
            })?;
        let inner = SdkSessionHandle {
            id: SessionId::from(id),
            service: Arc::clone(&self.service),
        };
        let bot_id = inner.state().await?.id;
        Ok(BotHandle { bot_id, inner })
    }

    /// Lists all bots currently represented in the maintained fleet cache.
    pub async fn bots(&self) -> Result<Vec<BotHandle>, SdkError> {
        let state = self.state().await;
        let mut ids = state.fleet.bots.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        let mut bots = Vec::with_capacity(ids.len());
        for id in ids {
            bots.push(self.bot(id).await?);
        }
        Ok(bots)
    }

    #[doc(hidden)]
    pub async fn host_bot_snapshots(&self) -> Vec<prayer_state::FleetEntry> {
        let state = self.host_state().await;
        let mut bots = state.fleet.bots.values().cloned().collect::<Vec<_>>();
        bots.sort_by(|a, b| a.id.cmp(&b.id));
        bots
    }

    /// Flushes durable state, stops workers, and closes SpaceMolt accounts.
    pub async fn shutdown(&self) -> Result<(), SdkError> {
        let _shutdown = self.shutdown_gate.lock().await;
        if self.shutdown_complete.load(Ordering::Acquire) {
            return Ok(());
        }
        self.service.request_shutdown();
        self.service.persist_sessions("SDK shutdown").await;
        self.client.close_all();
        self.service.stop_background_workers().await;
        self.shutdown_complete.store(true, Ordering::Release);
        Ok(())
    }

    /// Returns the configured client used to associate connected accounts.
    /// This is intentionally crate-private once lifecycle migration is complete.
    #[cfg(test)]
    fn service(&self) -> &Arc<RuntimeService> {
        &self.service
    }

    /// Returns the narrow administration/planning facade.
    #[doc(hidden)]
    pub fn administration(&self) -> PrayerAdministration {
        PrayerAdministration {
            service: Arc::clone(&self.service),
        }
    }

    /// Starts host-managed background refresh workers for injected-client embeddings.
    #[doc(hidden)]
    fn start_background_workers(&self) {
        Arc::clone(&self.service).start_idle_session_refresher();
    }

    /// Creates an observed bot without transport I/O for downstream contract tests.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub async fn inject_test_bot(
        &self,
        bot_id: impl Into<BotId>,
        username: impl Into<String>,
    ) -> Result<BotHandle, SdkError> {
        let id = self.service.create_session_with_label(None)?;
        let session = self.service.get_session(id).await?;
        let mut session = session.lock().await;
        let bot_id = bot_id.into();
        session.bot_id = Some(bot_id.clone());
        session.has_state = true;
        session.bot_state_mut().player.username = Some(username.into());
        drop(session);
        self.bot(bot_id.as_str()).await
    }

    fn ensure_running(&self) -> Result<(), SdkError> {
        if self.service.is_shutting_down() {
            Err(SdkError::ShutdownInProgress)
        } else {
            Ok(())
        }
    }
}

impl BotHandle {
    pub fn id(&self) -> &str {
        self.bot_id.as_str()
    }

    /// Returns the latest canonical cached bot snapshot without upstream I/O.
    pub async fn state(&self) -> Result<BotStateEntry, SdkError> {
        self.inner.state().await
    }

    #[doc(hidden)]
    pub async fn host_state(&self) -> Result<prayer_state::FleetEntry, SdkError> {
        self.inner.state().await
    }

    pub async fn halt(&self, reason: Option<String>) -> Result<(), SdkError> {
        self.inner.halt(reason).await
    }

    /// Returns the current or most recent PrayerLang execution for this bot.
    pub async fn script_execution(&self) -> Result<Option<ScriptExecutionDto>, SdkError> {
        self.inner
            .service
            .script_execution(self.inner.id.into_uuid())
            .await
    }

    pub async fn start_actions(
        &self,
        actions: impl IntoIterator<Item = Action>,
    ) -> Result<ActionRunHandle, SdkError> {
        self.inner.ensure_running()?;
        let actions: Vec<Action> = actions.into_iter().collect();
        validate_action_batch(&actions)?;
        let lane = self.inner.try_acquire_action_lane().await?;
        let run_id = lane.run_id();
        let prayerlang = actions
            .iter()
            .map(prayer_lang::render_action)
            .collect::<Vec<_>>()
            .join("\n");
        let envelopes = action_envelopes(&run_id, actions);
        if let Err(error) = lane
            .service
            .submit_action_batch(lane.session_id.into_uuid(), &lane.claim, envelopes)
            .await
        {
            let _ = lane
                .service
                .cancel_action_run(
                    lane.session_id.into_uuid(),
                    &run_id,
                    "action batch submission failed".into(),
                )
                .await;
            return Err(error);
        }
        let session_id = lane.session_id;
        if let Err(error) = lane
            .service
            .ensure_action_runner(session_id.into_uuid(), "sdk action run")
            .await
        {
            let _ = lane
                .service
                .cancel_action_run(
                    session_id.into_uuid(),
                    &run_id,
                    "action runner failed to start".into(),
                )
                .await;
            return Err(error);
        }
        Ok(ActionRunHandle {
            session_id,
            run_id,
            service: Arc::clone(&lane.service),
            prayerlang,
        })
    }

    pub async fn execute_actions(
        &self,
        actions: impl IntoIterator<Item = Action>,
    ) -> Result<ActionRunOutcome, SdkError> {
        self.start_actions(actions).await?.wait().await
    }

    /// Enqueue typed work on the higher-precedence lane.
    pub async fn execute_action_override(
        &self,
        actions: impl IntoIterator<Item = Action>,
        options: OverrideOptions,
    ) -> Result<(), SdkError> {
        self.inner.ensure_running()?;
        let actions = actions.into_iter().collect::<Vec<_>>();
        validate_action_batch(&actions)?;
        self.inner
            .service
            .submit_action_override(self.inner.id.into_uuid(), actions, options)
            .await
    }

    /// Parse a linear PrayerLang plan and enqueue it on the override lane.
    pub async fn execute_script_override(
        &self,
        script: impl Into<String>,
        options: OverrideOptions,
    ) -> Result<(), SdkError> {
        self.inner.ensure_running()?;
        self.inner
            .service
            .submit_script_override(self.inner.id.into_uuid(), script.into(), options)
            .await
    }

    pub async fn action_run(&self, run_id: RunId) -> Result<ActionRunHandle, SdkError> {
        let run = self
            .inner
            .service
            .action_run(self.inner.id.into_uuid(), &run_id)
            .await?
            .ok_or_else(|| SdkError::RunNotFound {
                run_id: run_id.clone(),
            })?;
        let prayerlang = run
            .actions
            .iter()
            .map(|envelope| prayer_lang::render_action(&envelope.action))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(ActionRunHandle {
            session_id: self.inner.id,
            run_id,
            service: Arc::clone(&self.inner.service),
            prayerlang,
        })
    }

    pub async fn start_script(
        &self,
        source: impl Into<String>,
    ) -> Result<ScriptRunHandle, SdkError> {
        let source = source.into();
        let (run_id, prayerlang) = self.inner.try_start_script_details(source).await?;
        Ok(ScriptRunHandle {
            session_id: self.inner.id,
            run_id,
            service: Arc::clone(&self.inner.service),
            prayerlang,
        })
    }

    pub async fn script_run(&self, run_id: RunId) -> Result<ScriptRunHandle, SdkError> {
        let execution = self
            .inner
            .service
            .script_execution(self.inner.id.into_uuid())
            .await?
            .filter(|execution| execution.run_id.as_ref() == Some(&run_id))
            .ok_or_else(|| SdkError::RunNotFound {
                run_id: run_id.clone(),
            })?;
        let prayerlang = self
            .inner
            .service
            .execution_snapshot(self.inner.id.into_uuid())
            .await?
            .source_prayer;
        debug_assert_eq!(execution.run_id.as_ref(), Some(&run_id));
        Ok(ScriptRunHandle {
            session_id: self.inner.id,
            run_id,
            service: Arc::clone(&self.inner.service),
            prayerlang,
        })
    }

    pub async fn queue(&self) -> Result<QueueSnapshot, SdkError> {
        let scheduler = self
            .inner
            .service
            .scheduler_snapshot(self.inner.id.into_uuid())
            .await?;
        let override_active =
            scheduler.interrupt.is_some() || !scheduler.interrupt_pending.is_empty();
        let prayerlang = if override_active {
            self.inner
                .service
                .override_scheduler_prayer(self.inner.id.into_uuid())
                .await?
        } else {
            self.inner
                .service
                .normal_scheduler_prayer(self.inner.id.into_uuid())
                .await?
        };
        Ok(QueueSnapshot {
            generation: scheduler.generation,
            owner: scheduler.claim.as_ref().map(|claim| (&claim.owner).into()),
            running_action: scheduler.running.is_some(),
            pending_actions: scheduler.pending.len(),
            interrupt_active: override_active,
            halted: scheduler.halted,
            halt_reason: scheduler.halt_reason,
            prayerlang,
        })
    }

    pub async fn normal_queue(&self) -> Result<QueueLaneSnapshot, SdkError> {
        let scheduler = self
            .inner
            .service
            .scheduler_snapshot(self.inner.id.into_uuid())
            .await?;
        Ok(QueueLaneSnapshot {
            active: scheduler.running.is_some(),
            pending_actions: scheduler.pending.len(),
            prayerlang: self
                .inner
                .service
                .normal_scheduler_prayer(self.inner.id.into_uuid())
                .await?,
        })
    }

    pub async fn override_queue(&self) -> Result<QueueLaneSnapshot, SdkError> {
        let scheduler = self
            .inner
            .service
            .scheduler_snapshot(self.inner.id.into_uuid())
            .await?;
        Ok(QueueLaneSnapshot {
            active: scheduler.interrupt.is_some(),
            pending_actions: scheduler.interrupt_pending.len(),
            prayerlang: self
                .inner
                .service
                .override_scheduler_prayer(self.inner.id.into_uuid())
                .await?,
        })
    }
}

impl ActionRunHandle {
    pub fn id(&self) -> &RunId {
        &self.run_id
    }
    pub fn prayerlang(&self) -> &str {
        &self.prayerlang
    }

    pub async fn status(&self) -> Result<RunStatus<ActionRunOutcome>, SdkError> {
        let run = self
            .service
            .action_run(self.session_id.into_uuid(), &self.run_id)
            .await?
            .ok_or_else(|| SdkError::RunNotFound {
                run_id: self.run_id.clone(),
            })?;
        Ok(match run.outcome {
            Some(outcome) => RunStatus::Terminal(outcome.into()),
            None => RunStatus::Running,
        })
    }

    pub async fn wait(&self) -> Result<ActionRunOutcome, SdkError> {
        self.wait_with(WaitOptions::default()).await
    }

    pub async fn wait_with(&self, options: WaitOptions) -> Result<ActionRunOutcome, SdkError> {
        let wait = async {
            loop {
                if let RunStatus::Terminal(outcome) = self.status().await? {
                    return Ok::<_, SdkError>(outcome);
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        };
        match options.timeout {
            Some(timeout) => {
                tokio::time::timeout(timeout, wait)
                    .await
                    .map_err(|_| SdkError::WaitTimedOut {
                        run_id: self.run_id.clone(),
                    })?
            }
            None => wait.await,
        }
    }

    pub async fn cancel(&self, reason: impl Into<String>) -> Result<ActionRunOutcome, SdkError> {
        let run = self
            .service
            .cancel_action_run(self.session_id.into_uuid(), &self.run_id, reason.into())
            .await?;
        run.outcome
            .map(Into::into)
            .ok_or_else(|| SdkError::InvalidRuntimeState("cancelled run is not terminal".into()))
    }
}

impl ScriptRunHandle {
    pub fn id(&self) -> &RunId {
        &self.run_id
    }
    pub fn prayerlang(&self) -> &str {
        &self.prayerlang
    }

    pub async fn status(&self) -> Result<RunStatus<ScriptRunOutcome>, SdkError> {
        let execution = self
            .service
            .script_execution(self.session_id.into_uuid())
            .await?
            .ok_or_else(|| SdkError::RunNotFound {
                run_id: self.run_id.clone(),
            })?;
        if execution.run_id.as_ref() != Some(&self.run_id) {
            return Err(SdkError::RunNotFound {
                run_id: self.run_id.clone(),
            });
        }
        Ok(match execution.state {
            ScriptExecutionStateDto::Running { .. } => RunStatus::Running,
            ScriptExecutionStateDto::Stopped { outcome, .. } => RunStatus::Terminal(outcome.into()),
        })
    }

    pub async fn wait(&self) -> Result<ScriptRunOutcome, SdkError> {
        self.wait_with(WaitOptions::default()).await
    }

    pub async fn wait_with(&self, options: WaitOptions) -> Result<ScriptRunOutcome, SdkError> {
        let wait = async {
            loop {
                if let RunStatus::Terminal(outcome) = self.status().await? {
                    return Ok::<_, SdkError>(outcome);
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        };
        match options.timeout {
            Some(timeout) => {
                tokio::time::timeout(timeout, wait)
                    .await
                    .map_err(|_| SdkError::WaitTimedOut {
                        run_id: self.run_id.clone(),
                    })?
            }
            None => wait.await,
        }
    }

    pub async fn cancel(&self, reason: impl Into<String>) -> Result<ScriptRunOutcome, SdkError> {
        self.service
            .cancel_script_run(self.session_id.into_uuid(), reason.into())
            .await?;
        self.wait().await
    }
}

fn validate_action_batch(actions: &[Action]) -> Result<(), SdkError> {
    if actions.is_empty() {
        return Err(SdkError::BadRequest(
            "action batch must not be empty".into(),
        ));
    }
    for action in actions {
        prayer_runtime::resolve_action(action.clone())
            .map_err(|error| SdkError::BadRequest(error.to_string()))?;
    }
    Ok(())
}

fn action_envelopes(run_id: &RunId, actions: Vec<Action>) -> Vec<ActionEnvelope> {
    let origin = ActionOrigin::Manual {
        run_id: run_id.clone(),
    };
    actions
        .into_iter()
        .enumerate()
        .map(|(index, action)| {
            ActionEnvelope::new(format!("{}-{index}", run_id.0), action, origin.clone())
        })
        .collect()
}

#[cfg_attr(test, allow(dead_code))]
impl SdkSessionHandle {
    fn ensure_running(&self) -> Result<(), SdkError> {
        if self.service.is_shutting_down() {
            Err(SdkError::ShutdownInProgress)
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    const fn id(&self) -> SessionId {
        self.id
    }

    #[cfg(test)]
    async fn snapshot(&self) -> Result<SessionSummary, SdkError> {
        self.service
            .session_summary(&self.id.into_uuid().to_string())
            .await
    }

    /// Returns the latest canonical cached bot snapshot without performing I/O.
    /// Shared facts are available from [`PrayerSdk::state`].
    async fn state(&self) -> Result<prayer_state::FleetEntry, SdkError> {
        self.service.bot_snapshot(self.id.into_uuid()).await
    }

    #[cfg(test)]
    async fn set_script(&self, script: impl Into<String>) -> Result<String, SdkError> {
        self.ensure_running()?;
        self.service
            .set_script(self.id.into_uuid(), script.into())
            .await
    }

    #[cfg(test)]
    async fn execute(&self, reason: impl Into<String>) -> Result<(), SdkError> {
        self.ensure_running()?;
        let _reason = reason.into();
        self.service
            .start_script_runner(self.id.into_uuid(), "sdk")
            .await
    }

    #[cfg(test)]
    async fn try_start_script(&self, script: impl Into<String>) -> Result<RunId, SdkError> {
        self.try_start_script_details(script)
            .await
            .map(|(run_id, _)| run_id)
    }

    async fn try_start_script_details(
        &self,
        script: impl Into<String>,
    ) -> Result<(RunId, String), SdkError> {
        self.ensure_running()?;
        let normalized = self
            .service
            .set_script(self.id.into_uuid(), script.into())
            .await?;
        let run_id = self
            .service
            .get_session(self.id.into_uuid())
            .await?
            .lock()
            .await
            .engine
            .normal_lane_claim()
            .and_then(|claim| match claim.owner {
                QueueOwner::PrayerLang { run_id } => Some(run_id),
                _ => None,
            })
            .ok_or_else(|| {
                SdkError::InvalidRuntimeState("script lane claim was not installed".into())
            })?;
        self.service
            .start_script_runner(self.id.into_uuid(), "sdk script")
            .await?;
        Ok((run_id, normalized))
    }

    async fn halt(&self, reason: Option<String>) -> Result<(), SdkError> {
        self.ensure_running()?;
        self.service.halt(self.id.into_uuid(), reason).await
    }

    #[cfg(test)]
    async fn refresh(&self) -> Result<prayer_state::FleetEntry, SdkError> {
        self.ensure_running()?;
        self.service.refresh_state(self.id.into_uuid()).await
    }

    async fn try_acquire_action_lane(&self) -> Result<ActionLane, SdkError> {
        self.ensure_running()?;
        let run_id = RunId(Uuid::new_v4().to_string());
        let claim = self
            .service
            .try_acquire_action_lane(self.id.into_uuid(), run_id.clone())
            .await?;
        Ok(ActionLane {
            session_id: self.id,
            run_id,
            claim,
            service: Arc::clone(&self.service),
        })
    }

    #[cfg(test)]
    async fn action_run(&self, run_id: RunId) -> Result<Option<ActionRunSnapshot>, SdkError> {
        self.service.action_run(self.id.into_uuid(), &run_id).await
    }

    #[cfg(test)]
    async fn cancel_action_run(
        &self,
        run_id: RunId,
        reason: impl Into<String>,
    ) -> Result<ActionRunSnapshot, SdkError> {
        self.ensure_running()?;
        self.service
            .cancel_action_run(self.id.into_uuid(), &run_id, reason.into())
            .await
    }
}

#[cfg_attr(test, allow(dead_code))]
impl ActionLane {
    pub fn run_id(&self) -> RunId {
        self.run_id.clone()
    }

    #[cfg(test)]
    async fn execute(
        self,
        actions: impl IntoIterator<Item = Action>,
    ) -> Result<ActionBatchOutcome, SdkError> {
        if self.service.is_shutting_down() {
            return Err(SdkError::ShutdownInProgress);
        }
        let origin = ActionOrigin::Manual {
            run_id: self.run_id.clone(),
        };
        let actions: Vec<ActionEnvelope> = actions
            .into_iter()
            .enumerate()
            .map(|(index, action)| {
                ActionEnvelope::new(format!("{}-{index}", self.run_id.0), action, origin.clone())
            })
            .collect();
        if actions.is_empty() {
            let _ = self
                .service
                .cancel_action_run(
                    self.session_id.into_uuid(),
                    &self.run_id,
                    "empty action batch".into(),
                )
                .await;
            return Err(SdkError::BadRequest(
                "action batch must not be empty".into(),
            ));
        }
        for envelope in &actions {
            if let Err(error) = prayer_runtime::resolve_action(envelope.action.clone()) {
                let _ = self
                    .service
                    .cancel_action_run(
                        self.session_id.into_uuid(),
                        &self.run_id,
                        "invalid action batch".into(),
                    )
                    .await;
                return Err(SdkError::BadRequest(error.to_string()));
            }
        }
        self.service
            .submit_action_batch(self.session_id.into_uuid(), &self.claim, actions)
            .await?;
        let session_id = self.session_id.into_uuid();
        if let Err(error) = self
            .service
            .ensure_action_runner(session_id, "sdk action lane")
            .await
        {
            let _ = self
                .service
                .cancel_action_run(
                    session_id,
                    &self.run_id,
                    "action runner failed to start".into(),
                )
                .await;
            return Err(error);
        }
        loop {
            if let Some(run) = self.service.action_run(session_id, &self.run_id).await? {
                if let Some(outcome) = run.outcome {
                    return Ok(outcome);
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn ordinary_public_state_signatures_do_not_name_runtime_models() {
        let source = include_str!("lib.rs");
        for suffix in [
            "BotState",
            "FleetEntry",
            "CatalogData",
            "GalaxyData",
            "StationMarketData",
            "knowledge::StateSnapshot",
        ] {
            let forbidden = ["prayer_", "runtime::", suffix].concat();
            assert!(
                !source.contains(&forbidden),
                "runtime state leaked: {forbidden}"
            );
        }
    }

    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;

    #[derive(Default)]
    struct EmptyClerk;

    #[async_trait]
    impl spacemolt_lib_rs::auth::ClerkHttpClient for EmptyClerk {
        async fn request_json(
            &self,
            _method: &str,
            url: &str,
            _api_key: &str,
            _body: Option<serde_json::Value>,
        ) -> Result<serde_json::Value, String> {
            if url.ends_with("/api/registration-code") {
                Ok(serde_json::json!({ "registration_code": "rc", "players": [] }))
            } else {
                Err(format!("unexpected Clerk URL: {url}"))
            }
        }
    }

    #[test]
    fn clerk_bootstrap_uses_the_production_origin_by_default() {
        let options = PrayerSdkOptions::default().with_clerk_api_key("sk_test");
        assert_eq!(
            options.spacemolt_base_url,
            spacemolt_origin::DEFAULT_SPACEMOLT_ORIGIN
        );
        assert!(options.client.is_none());
    }

    #[tokio::test]
    async fn connect_requires_clerk_or_an_injected_client() {
        let error = match PrayerSdk::connect(PrayerSdkOptions::default()).await {
            Ok(_) => panic!("unconfigured bootstrap must fail"),
            Err(error) => error,
        };
        assert!(matches!(error, SdkError::BadRequest(_)));
    }

    #[tokio::test]
    async fn one_call_bootstrap_reports_an_empty_owned_fleet_and_shuts_down() {
        let mut client_options = SpacemoltClientOptions::default();
        client_options.clerk_api_key = Some("sk_test".into());
        client_options.clerk_http_client = Some(Arc::new(EmptyClerk));
        let client = Arc::new(SpacemoltClient::new(
            client_options,
            MemoryCredentialStore::default(),
        ));
        let mut options = isolated_options("one-call-bootstrap");
        options.client = Some(client);
        options.runtime.local_auth_bypass = false;

        let sdk = PrayerSdk::connect(options)
            .await
            .expect("one-call bootstrap");
        assert!(sdk.startup_report().accounts.is_empty());
        assert!(sdk.startup_report().is_ready());
        sdk.shutdown().await.expect("shutdown");
    }

    fn isolated_options(name: &str) -> PrayerSdkOptions {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root =
            std::path::PathBuf::from("/tmp").join(format!("prayer-sdk-{name}-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");
        options.runtime.local_auth_bypass = true;
        options
    }

    #[test]
    fn startup_report_distinguishes_partial_readiness() {
        let report = StartupReport {
            accounts: vec![
                StartupAccountStatus {
                    player_id: "ready".into(),
                    username: "Ready".into(),
                    connected: true,
                    ready: true,
                    error: None,
                },
                StartupAccountStatus {
                    player_id: "offline".into(),
                    username: "Offline".into(),
                    connected: false,
                    ready: false,
                    error: Some("SpaceMolt account failed to connect".into()),
                },
            ],
        };
        assert!(!report.is_ready());
    }

    #[tokio::test]
    async fn duplicate_mutable_bot_names_are_ambiguous() {
        let sdk = PrayerSdk::new(isolated_options("ambiguous-selector"));
        for index in 0..2 {
            let id = sdk
                .create_session(CreateSession::default())
                .await
                .expect("session");
            let session = sdk
                .service()
                .get_session(id.into_uuid())
                .await
                .expect("session");
            let mut session = session.lock().await;
            session.bot_id = Some(BotId::from(format!("duplicate-{index}")));
            session.has_state = true;
            session.bot_state_mut().location.system_id = Some("test-system".into());
            session.bot_state_mut().player.username = Some("Duplicated".into());
        }

        assert!(matches!(
            sdk.bot("Duplicated").await,
            Err(SdkError::AmbiguousBot { .. })
        ));
        let snapshot = sdk.state().await;
        assert!(matches!(
            snapshot.bot("Duplicated"),
            Err(SdkError::AmbiguousBot { .. })
        ));
    }

    #[tokio::test]
    async fn stable_bot_id_wins_over_a_matching_mutable_name() {
        let sdk = PrayerSdk::new(isolated_options("stable-selector"));
        let stable = sdk
            .create_session(CreateSession::default())
            .await
            .expect("stable session");
        let named = sdk
            .create_session(CreateSession::default())
            .await
            .expect("named session");
        {
            let session = sdk
                .service()
                .get_session(stable.into_uuid())
                .await
                .expect("stable session");
            let mut session = session.lock().await;
            session.bot_id = Some(BotId::from("stable-id"));
            session.has_state = true;
            session.bot_state_mut().location.system_id = Some("test-system".into());
            session.bot_state_mut().player.username = Some("Original".into());
        }
        {
            let session = sdk
                .service()
                .get_session(named.into_uuid())
                .await
                .expect("named session");
            let mut session = session.lock().await;
            session.bot_id = Some(BotId::from("other-id"));
            session.has_state = true;
            session.bot_state_mut().location.system_id = Some("test-system".into());
            session.bot_state_mut().player.username = Some("stable-id".into());
        }

        assert_eq!(
            sdk.bot("stable-id").await.expect("stable bot").id(),
            "stable-id"
        );
        let snapshot = sdk.state().await;
        assert_eq!(
            snapshot
                .bot("stable-id")
                .expect("stable snapshot bot")
                .id
                .as_str(),
            "stable-id"
        );
        assert_eq!(
            snapshot
                .bot("Original")
                .expect("username snapshot bot")
                .id
                .as_str(),
            "stable-id"
        );
    }

    async fn test_bot(name: &str) -> (PrayerSdk, BotHandle) {
        let sdk = PrayerSdk::new(isolated_options(name));
        let id = sdk
            .create_session(CreateSession::default())
            .await
            .expect("session");
        let session = sdk
            .service()
            .get_session(id.into_uuid())
            .await
            .expect("session");
        session.lock().await.bot_id = Some(BotId::from("test-bot"));
        let bot = sdk.bot("test-bot").await.expect("bot");
        (sdk, bot)
    }

    #[tokio::test]
    async fn bot_start_validates_before_claiming_and_projects_prayerlang() {
        let (_sdk, bot) = test_bot("public-action-run").await;
        assert!(matches!(
            bot.start_actions(Vec::<Action>::new()).await,
            Err(SdkError::BadRequest(_))
        ));
        assert!(bot.queue().await.expect("queue").owner.is_none());

        let run = bot
            .start_actions([Action::Wait { ticks: 100 }])
            .await
            .expect("run");
        assert!(run.prayerlang().contains("wait 100"));
        assert_eq!(run.id(), &run.run_id);
        let outcome = run.cancel("test cancellation").await.expect("cancel");
        assert!(matches!(outcome, ActionRunOutcome::Cancelled { .. }));
    }

    #[tokio::test]
    async fn dropping_action_handle_does_not_cancel_accepted_work() {
        let (_sdk, bot) = test_bot("detached-action-run").await;
        let run = bot
            .start_actions([Action::Wait { ticks: 100 }])
            .await
            .expect("run");
        let run_id = run.id().clone();
        drop(run);

        let attached = bot.action_run(run_id).await.expect("reattach run");
        assert!(attached.prayerlang().contains("wait 100"));
        let outcome = attached.cancel("detached cleanup").await.expect("cancel");
        assert!(matches!(outcome, ActionRunOutcome::Cancelled { .. }));
    }

    #[tokio::test]
    async fn script_handle_exposes_identity_projection_and_cancellation() {
        let (_sdk, bot) = test_bot("public-script-run").await;
        let run = bot.start_script("go alpha;").await.expect("script run");
        assert!(!run.id().0.is_empty());
        assert_eq!(run.prayerlang(), "go alpha;");
        let outcome = run.cancel("test cancellation").await.expect("cancel");
        assert!(matches!(
            outcome,
            ScriptRunOutcome::Error {
                kind: ScriptErrorKind::Cancelled,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn shutdown_is_idempotent_and_rejects_new_work() {
        let sdk = PrayerSdk::new(isolated_options("shutdown"));
        Arc::clone(sdk.service()).start_idle_session_refresher();

        sdk.shutdown().await.expect("first shutdown");
        sdk.shutdown().await.expect("second shutdown");

        assert!(matches!(
            sdk.create_session(CreateSession::default()).await,
            Err(SdkError::ShutdownInProgress)
        ));
    }
    use std::path::Path;

    fn crate_text(root: &Path) -> String {
        let mut files = vec![root.to_path_buf()];
        let mut text = String::new();
        while let Some(path) = files.pop() {
            for entry in std::fs::read_dir(path).expect("read SDK sources") {
                let path = entry.expect("SDK source entry").path();
                if path.is_dir() {
                    files.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    text.push_str(&std::fs::read_to_string(path).expect("read SDK source"));
                }
            }
        }
        text
    }

    #[test]
    fn sdk_has_no_http_or_environment_ownership() {
        let manifest =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
                .expect("SDK manifest");
        let source = crate_text(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));

        assert!(!manifest.contains("axum"));
        assert!(!manifest.lines().any(|line| {
            line.trim_start()
                .strip_prefix("prayer-api")
                .is_some_and(|suffix| suffix.trim_start().starts_with('='))
        }));
        for forbidden in [
            ["SPACEMOLT", "_CLERK_API_KEY"].concat(),
            ["std", "::env::"].concat(),
            ["Status", "Code"].concat(),
        ] {
            assert!(
                !source.contains(&forbidden),
                "forbidden SDK ownership: {forbidden}"
            );
        }
    }

    #[test]
    fn canonical_hot_paths_do_not_recompose_game_state() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        for relative in [
            "src/lib.rs",
            "src/knowledge/inventory/mod.rs",
            "src/knowledge/projection/commander.rs",
            "src/knowledge/virtual_market/service.rs",
            "src/knowledge/ingestion/service.rs",
            "src/service/execution/mod.rs",
        ] {
            let source = std::fs::read_to_string(manifest_dir.join(relative))
                .unwrap_or_else(|error| panic!("read {relative}: {error}"));
            let legacy_type = ["Runtime", "Game", "State", "Dto"].concat();
            let legacy_conversion = ["to_", "game_state"].concat();
            assert!(
                !source.contains(&legacy_type) && !source.contains(&legacy_conversion),
                "canonical hot path {relative} reintroduced legacy state composition"
            );
        }
    }

    #[tokio::test]
    async fn non_http_consumer_can_create_and_read_a_session() {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-sdk-embedding-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");

        let sdk = PrayerSdk::new(options);
        let id = sdk
            .create_session(CreateSession {
                label: Some("embedded-session".to_string()),
            })
            .await
            .expect("create embedded session");
        let session = sdk.session(id).await.expect("lookup embedded session");
        let snapshot = session.snapshot().await.expect("read embedded session");

        assert_eq!(snapshot.id, id.into_uuid().to_string());
        assert_eq!(snapshot.label, "embedded-session");
    }

    #[tokio::test]
    async fn cached_state_exposes_shared_facility_instances_without_refreshing() {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-sdk-cached-state-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");
        let sdk = PrayerSdk::new(options);
        let _id = sdk
            .create_session(CreateSession::default())
            .await
            .expect("session");
        sdk.service().remember_facility_snapshot(
            "station-alpha",
            prayer_state::PoiFacilitiesSnapshot {
                observed_at_unix: 42,
                current: None,
                faction_current: None,
            },
        );

        let state = sdk.state().await;

        assert_eq!(
            state.world.state.galaxy.facilities_by_poi["station-alpha"].observed_at_unix,
            42
        );
    }

    #[tokio::test]
    async fn script_and_typed_actions_share_one_exclusive_lane() {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-sdk-action-lane-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");
        let sdk = PrayerSdk::new(options);
        let id = sdk
            .create_session(CreateSession::default())
            .await
            .expect("session");
        let session = sdk.session(id).await.expect("handle");

        let lane = session
            .try_acquire_action_lane()
            .await
            .expect("manual claim");
        let run_id = lane.run_id();
        assert!(matches!(
            session.try_acquire_action_lane().await,
            Err(SdkError::LaneBusy { .. })
        ));
        assert!(matches!(
            session.set_script("go alpha;").await,
            Err(SdkError::LaneBusy { .. })
        ));

        session
            .cancel_action_run(run_id, "test complete")
            .await
            .expect("cancel");
        session.set_script("go alpha;").await.expect("script claim");
        assert!(matches!(
            session.try_acquire_action_lane().await,
            Err(SdkError::LaneBusy { .. })
        ));
    }

    #[tokio::test]
    async fn client_overrides_share_one_exclusive_lane() {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-sdk-override-lane-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");
        let sdk = PrayerSdk::new(options);
        let id = sdk
            .create_session(CreateSession::default())
            .await
            .expect("session");
        sdk.service
            .submit_action_override(
                id.into_uuid(),
                vec![prayer_actions::Action::Wait { ticks: 30 }],
                OverrideOptions::default(),
            )
            .await
            .expect("first override");
        assert!(matches!(
            sdk.service
                .submit_action_override(
                    id.into_uuid(),
                    vec![prayer_actions::Action::Wait { ticks: 1 }],
                    OverrideOptions::default(),
                )
                .await,
            Err(SdkError::LaneBusy { .. })
        ));

        sdk.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn aggregate_state_is_cache_only_and_reuses_canonical_arcs() {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-sdk-aggregate-state-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");
        let sdk = PrayerSdk::new(options);
        let session_id = sdk
            .create_session(CreateSession {
                label: Some("Scout".into()),
            })
            .await
            .expect("session");
        let session = sdk
            .service()
            .get_session(session_id.into_uuid())
            .await
            .expect("session");
        let bot_arc = {
            let mut session = session.lock().await;
            session.bot_id = Some(prayer_state::BotId::from("player-1"));
            session.has_state = true;
            session.bot_state_mut().player.username = Some("Scout".into());
            Arc::clone(&session.actor.observed)
        };

        let first = sdk.state().await;
        let second = sdk.state().await;
        let bot = sdk
            .session(session_id)
            .await
            .expect("session handle")
            .state()
            .await
            .expect("bot state");
        assert!(Arc::ptr_eq(
            &first.fleet.bots[&prayer_state::BotId::from("player-1")].state,
            &bot_arc,
        ));
        assert!(Arc::ptr_eq(&bot.state, &bot_arc));
        assert!(Arc::ptr_eq(&first.world.state, &second.world.state));
        assert_eq!(
            sdk.bot("player-1").await.expect("bot handle").id(),
            "player-1",
        );
        let snapshots = first.fleet.bots.values().collect::<Vec<_>>();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id.as_str(), "player-1");
        assert!(Arc::ptr_eq(&snapshots[0].state, &bot_arc));
    }

    #[tokio::test]
    async fn fleet_snapshot_keeps_two_bots_distinct_and_reports_versions() {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-sdk-two-bots-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");
        let sdk = PrayerSdk::new(options);
        let first_id = sdk
            .create_session(CreateSession::default())
            .await
            .expect("first session");
        let second_id = sdk
            .create_session(CreateSession::default())
            .await
            .expect("second session");

        for (session_id, bot_id, system, cargo_item, mission, ship_id, version) in [
            (first_id, "bot-a", "sol", "iron", "mission-a", "ship-a", 3),
            (
                second_id,
                "bot-b",
                "vega",
                "water",
                "mission-b",
                "ship-b",
                7,
            ),
        ] {
            let session = sdk
                .service()
                .get_session(session_id.into_uuid())
                .await
                .expect("session");
            let mut session = session.lock().await;
            session.bot_id = Some(prayer_state::BotId::from(bot_id));
            session.has_state = true;
            session.state_version = version;
            let state = session.bot_state_mut();
            state.location.system_id = Some(system.to_string());
            state.location.poi_id = Some(format!("{system}-station"));
            state.cargo = Arc::new(HashMap::from([(cargo_item.to_string(), 5)]));
            state.missions = Arc::new(prayer_state::MissionData {
                active: vec![mission.to_string()],
                ..prayer_state::MissionData::default()
            });
            state.ship.id = Some(ship_id.to_string());
        }
        sdk.service().remember_facility_snapshot(
            "sol-station",
            prayer_state::PoiFacilitiesSnapshot {
                observed_at_unix: 5,
                current: None,
                faction_current: None,
            },
        );

        let snapshot = sdk.state().await;
        let first = &snapshot.fleet.bots[&prayer_state::BotId::from("bot-a")];
        let second = &snapshot.fleet.bots[&prayer_state::BotId::from("bot-b")];

        assert_eq!(first.state.location.system_id.as_deref(), Some("sol"));
        assert_eq!(second.state.location.system_id.as_deref(), Some("vega"));
        assert_eq!(first.state.cargo.get("iron"), Some(&5));
        assert_eq!(second.state.cargo.get("water"), Some(&5));
        assert_eq!(first.state.missions.active, ["mission-a"]);
        assert_eq!(second.state.missions.active, ["mission-b"]);
        assert_eq!(first.state.ship.id.as_deref(), Some("ship-a"));
        assert_eq!(second.state.ship.id.as_deref(), Some("ship-b"));
        assert_eq!((first.version, second.version), (3, 7));
        assert_eq!(snapshot.world.version, 1);
        assert_eq!(snapshot.version, 7);
    }

    #[tokio::test]
    async fn consumer_owned_snapshot_does_not_hold_runtime_locks() {
        let client = Arc::new(SpacemoltClient::default());
        let mut options = PrayerSdkOptions::new(client, "https://game.spacemolt.com");
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("prayer-sdk-lock-free-snapshot-{}", Uuid::new_v4()));
        options.runtime.knowledge_state_path = root.join("knowledge.json");
        options.runtime.session_state_path = root.join("sessions.json");
        let sdk = PrayerSdk::new(options);
        let id = sdk
            .create_session(CreateSession::default())
            .await
            .expect("session");
        let session = sdk
            .service()
            .get_session(id.into_uuid())
            .await
            .expect("session");
        {
            let mut locked = session.lock().await;
            locked.bot_id = Some(prayer_state::BotId::from("lock-test"));
            locked.has_state = true;
        }

        let retained_by_consumer = sdk.state().await;
        let mut session_guard = tokio::time::timeout(Duration::from_millis(100), session.lock())
            .await
            .expect("snapshot retained a session lock");
        session_guard.bot_state_mut().player.credits = Some(99);
        drop(session_guard);
        sdk.service().remember_facility_snapshot(
            "station-after-snapshot",
            prayer_state::PoiFacilitiesSnapshot {
                observed_at_unix: 9,
                current: None,
                faction_current: None,
            },
        );

        assert_eq!(
            retained_by_consumer.fleet.bots[&prayer_state::BotId::from("lock-test")]
                .state
                .player
                .credits
                .unwrap_or_default(),
            0
        );
        assert!(!retained_by_consumer
            .world
            .state
            .galaxy
            .facilities_by_poi
            .contains_key("station-after-snapshot"));
    }
}
