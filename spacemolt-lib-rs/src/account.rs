//! A single authenticated SpaceMolt connection.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::oneshot;
use tracing::{info, warn};
use uuid::Uuid;

use crate::actions::{find_action_parts, ActionKind};
use crate::auth::{mint_ws_token, AuthCredentials, ClerkHttpClient, ReqwestClerkHttpClient};
use crate::errors::{
    retry_after_ms_from_close, ClientError, ConnectionClosedError, SpacemoltError,
    CLOSE_CODE_AUTH_TIMEOUT, CLOSE_CODE_SESSION_REPLACED,
};
use crate::events::{EventStream, ListenerId, TypedEmitter};
use crate::protocol::{
    InboundFrame, MutationAck, MutationResult, QueryResult, RawFrame, RegisteredPayload,
    StateSection, WelcomePayload,
};
use crate::state::{MarketBook, MarketCache, ObservationCache, ObservationView, StateCache};
use crate::transport::correlator::Correlator;
use crate::transport::socket::{
    SocketCallbacks, SocketFactory, SocketHandle, TokioWebSocketFactory,
};

/// Reconnect options for an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectOptions {
    /// Max reconnect attempts before giving up.
    pub max_retries: Option<u64>,
    /// Base exponential backoff in milliseconds.
    pub base_delay_ms: u64,
    /// Backoff ceiling in milliseconds.
    pub max_delay_ms: u64,
}

impl Default for ReconnectOptions {
    fn default() -> Self {
        Self {
            max_retries: None,
            base_delay_ms: 1_000,
            max_delay_ms: 30_000,
        }
    }
}

/// Account construction options.
#[derive(Clone)]
pub struct AccountOptions {
    /// WebSocket URL of the v2 endpoint.
    pub url: String,
    /// Seed local state via `get_status` after auth.
    pub seed_state: bool,
    /// Max automatic retries when a command is rate limited.
    pub max_rate_limit_retries: u32,
    /// Stable id this account is managed under.
    pub id: Option<String>,
    /// Welcome and authentication exchange timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// Query timeout in milliseconds.
    pub query_timeout_ms: u64,
    /// Long mutation timeout in milliseconds for transit actions.
    pub mutation_timeout_ms: u64,
    /// Short mutation timeout in milliseconds for same-tick actions.
    pub fast_mutation_timeout_ms: u64,
    /// Automatic reconnect policy.
    pub reconnect: Option<ReconnectOptions>,
    /// Credentials used to re-authenticate automatic reconnect attempts.
    pub credentials: Option<AuthCredentials>,
    /// HTTP client used for Clerk token minting.
    pub clerk_http_client: Option<Arc<dyn ClerkHttpClient>>,
}

impl Default for AccountOptions {
    fn default() -> Self {
        Self {
            url: "wss://game.spacemolt.com/ws/v2".to_string(),
            seed_state: true,
            max_rate_limit_retries: 5,
            id: None,
            connect_timeout_ms: 15_000,
            query_timeout_ms: 15_000,
            mutation_timeout_ms: 600_000,
            fast_mutation_timeout_ms: 180_000,
            reconnect: None,
            credentials: None,
            clerk_http_client: None,
        }
    }
}

type AccountFuture<T> = Pin<Box<dyn Future<Output = Result<T, ClientError>> + Send>>;
type StateChangeHandler = Arc<dyn Fn(&[StateSection]) + Send + Sync>;
type ReconnectedHandler = Arc<dyn Fn() + Send + Sync>;
type DisconnectedHandler = Arc<dyn Fn(ConnectionClosedError) + Send + Sync>;

const TRANSIT_ACTIONS: &[&str] = &["spacemolt/jump", "spacemolt/travel"];

#[derive(Clone, Copy)]
struct ReconciliationPolicy {
    sections: &'static [StateSection],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTarget {
    Personal,
    Faction,
}

// SPACEMOLT_INCOMPLETE_DELTA_WORKAROUND: spacemolt/sell omits cargo and ship;
// remove this entry once production telemetry shows both sections consistently present.
// SPACEMOLT_INCOMPLETE_DELTA_WORKAROUND: spacemolt_market/create_sell_order can
// immediately fill while omitting cargo and ship; remove once both are consistently present.
// SPACEMOLT_INCOMPLETE_DELTA_WORKAROUND: spacemolt_market/create_buy_order can
// immediately fill while omitting cargo and ship; remove once both are consistently present.
// SPACEMOLT_INCOMPLETE_DELTA_WORKAROUND: spacemolt_storage/deposit omits cargo and
// the destination storage view; remove this entry once those sections are consistently present.
// Buy and withdraw are precautionary until their result shapes have been captured.
const CARGO_SHIP: &[StateSection] = &[StateSection::Cargo, StateSection::Ship];
fn reconciliation_policy(tool: &str, action: &str) -> Option<ReconciliationPolicy> {
    matches!(
        (tool, action),
        ("spacemolt", "sell")
            | ("spacemolt", "buy")
            | ("spacemolt_market", "create_sell_order")
            | ("spacemolt_market", "create_buy_order")
            | ("spacemolt_storage", "deposit")
            | ("spacemolt_storage", "withdraw")
    )
    .then_some(ReconciliationPolicy {
        sections: CARGO_SHIP,
    })
}

/// Username/password login parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginParams {
    pub username: String,
    pub password: String,
}

/// Account registration parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterParams {
    pub username: String,
    pub empire: String,
    pub registration_code: String,
}

/// Credentials and initial state returned by registration.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisterResult {
    pub password: String,
    pub player_id: String,
    pub state: Value,
}

/// A single SpaceMolt account.
#[derive(Clone)]
pub struct Account {
    opts: AccountOptions,
    inner: Arc<Mutex<AccountInner>>,
    socket_factory: Option<Arc<dyn SocketFactory>>,
}

struct AccountInner {
    cache: StateCache,
    correlator: Correlator,
    emitter: TypedEmitter,
    socket: Option<Arc<dyn SocketHandle>>,
    welcome: Option<WelcomePayload>,
    welcome_waiter: Option<oneshot::Sender<WelcomePayload>>,
    authenticated: bool,
    login_payload: Option<Value>,
    pending_auth: Option<PendingAuth>,
    state_listeners: Vec<StateChangeHandler>,
    reconnected_listeners: Vec<ReconnectedHandler>,
    reconnecting_listeners: Vec<Arc<dyn Fn(u64) + Send + Sync>>,
    disconnected_listeners: Vec<DisconnectedHandler>,
    market_cache: MarketCache,
    market_subscribed: bool,
    subscribed_market_base_id: Option<String>,
    observation_cache: ObservationCache,
    observation_subscribed: bool,
    observation_active_scan: bool,
    subscribed_observation_poi_id: Option<String>,
    mutation_active: bool,
    mutation_waiters: VecDeque<oneshot::Sender<()>>,
    reconnecting: bool,
    storage_cache: HashMap<(String, String), crate::schema::StorageResponse>,
}

/// Owns the account's single mutation slot.
///
/// Mutation futures are routinely cancelled when a script is halted. Releasing
/// the slot from `Drop` keeps cancellation from stranding every later mutation
/// behind an operation that no longer has a future driving it.
struct MutationPermit {
    inner: Arc<Mutex<AccountInner>>,
}

impl MutationPermit {
    fn new(inner: Arc<Mutex<AccountInner>>) -> Self {
        Self { inner }
    }
}

impl Drop for MutationPermit {
    fn drop(&mut self) {
        finish_mutation(&self.inner);
    }
}

struct PendingAuth {
    request_id: String,
    registered: Option<RegisteredPayload>,
    tx: PendingAuthTx,
}

enum PendingAuthTx {
    Login(oneshot::Sender<Result<Value, ClientError>>),
    Register(oneshot::Sender<Result<RegisterResult, ClientError>>),
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("id", &self.opts.id)
            .field("url", &self.opts.url)
            .finish_non_exhaustive()
    }
}

impl Account {
    /// Create a disconnected account handle.
    pub fn new(opts: AccountOptions) -> Self {
        Self {
            opts,
            inner: Arc::new(Mutex::new(AccountInner::default())),
            socket_factory: Some(Arc::new(TokioWebSocketFactory)),
        }
    }

    /// Create a disconnected account handle with an injectable socket factory.
    pub fn with_socket_factory(
        opts: AccountOptions,
        socket_factory: Arc<dyn SocketFactory>,
    ) -> Self {
        Self {
            opts,
            inner: Arc::new(Mutex::new(AccountInner::default())),
            socket_factory: Some(socket_factory),
        }
    }

    /// Account id, when managed by a client.
    pub fn id(&self) -> Option<&str> {
        self.opts.id.as_deref()
    }

    /// True when two account handles point at the same underlying account.
    pub fn same_handle(&self, other: &Account) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Cached state copy.
    pub fn state(&self) -> StateCache {
        self.inner.lock().expect("account").cache.clone()
    }

    /// Cached state as a JSON object keyed by server section names.
    pub fn state_snapshot(&self) -> Value {
        let inner = self.inner.lock().expect("account");
        let mut map = serde_json::Map::new();
        for (section, value) in inner.cache.raw_snapshot() {
            map.insert(section.as_str().to_string(), value.clone());
        }
        Value::Object(map)
    }

    /// Welcome payload recorded after the server greets the socket.
    pub fn welcome(&self) -> Option<WelcomePayload> {
        self.inner.lock().expect("account").welcome.clone()
    }

    /// True after a `logged_in` frame has seeded this account session.
    pub fn authenticated(&self) -> bool {
        self.inner.lock().expect("account").authenticated
    }

    /// Raw payload from the most recent successful login.
    pub fn login_payload(&self) -> Option<Value> {
        self.inner.lock().expect("account").login_payload.clone()
    }

    /// Open the configured socket.
    pub async fn connect(&self) -> Result<(), ClientError> {
        let Some(factory) = self.socket_factory.as_ref() else {
            return Err(ClientError::NotImplemented(
                "WebSocket socket implementation is not ported yet",
            ));
        };
        let weak_for_frame = Arc::downgrade(&self.inner);
        let account_for_close = self.clone();
        let callbacks = SocketCallbacks::new(
            move |frame| route_frame(&weak_for_frame, frame),
            move |err| account_for_close.handle_close(err),
        );
        let socket = factory.connect(self.opts.url.clone(), callbacks).await?;
        let mut inner = self.inner.lock().expect("account");
        inner.welcome = None;
        inner.socket = Some(socket);
        Ok(())
    }

    /// Close the current socket deliberately.
    pub fn close(&self) {
        let socket = self.inner.lock().expect("account").socket.clone();
        if let Some(socket) = socket {
            socket.close();
        }
    }

    /// Reconnect this same account handle once, without internal retry/backoff.
    pub async fn reconnect_once(&self) -> Result<(), ClientError> {
        let Some(credentials) = self.opts.credentials.clone() else {
            return Err(ClientError::Server(SpacemoltError::new(
                "missing_credentials",
                "reconnect_once requires credentials",
            )));
        };
        self.connect().await?;
        self.wait_for_welcome().await?;
        self.authenticate(credentials).await?;
        self.restore_subscriptions().await
    }

    /// Wait for the server welcome frame after `connect`.
    pub async fn wait_for_welcome(&self) -> Result<WelcomePayload, ClientError> {
        let rx = {
            let mut inner = self.inner.lock().expect("account");
            if let Some(welcome) = inner.welcome.clone() {
                return Ok(welcome);
            }
            let (tx, rx) = oneshot::channel();
            inner.welcome_waiter = Some(tx);
            rx
        };
        match tokio::time::timeout(Duration::from_millis(self.opts.connect_timeout_ms), rx).await {
            Ok(Ok(welcome)) => Ok(welcome),
            Ok(Err(_)) => Err(ClientError::ConnectionClosed(ConnectionClosedError::new(
                "connection closed before welcome",
                None,
                None,
            ))),
            Err(_) => {
                self.inner.lock().expect("account").welcome_waiter.take();
                self.close();
                Err(ClientError::Timeout(format!(
                    "No welcome frame received within {}ms",
                    self.opts.connect_timeout_ms
                )))
            }
        }
    }

    /// Listen for one push frame type.
    pub fn on<F>(&self, kind: impl Into<String>, handler: F) -> ListenerId
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        self.inner
            .lock()
            .expect("account")
            .emitter
            .on(kind, handler)
    }

    /// Listen for one push frame type and deserialize its payload into `T`.
    ///
    /// Decode failures are ignored for this typed listener only; raw listeners
    /// registered with [`Self::on`] still receive the original payload.
    pub fn on_typed<T, F>(&self, kind: impl Into<String>, handler: F) -> ListenerId
    where
        T: DeserializeOwned + 'static,
        F: Fn(&T) + Send + Sync + 'static,
    {
        self.on(kind, move |payload| {
            if let Ok(decoded) = serde_json::from_value::<T>(payload.clone()) {
                handler(&decoded);
            }
        })
    }

    /// Listen for every push frame.
    pub fn on_any<F>(&self, handler: F) -> ListenerId
    where
        F: Fn(&RawFrame) + Send + Sync + 'static,
    {
        self.inner.lock().expect("account").emitter.on_any(handler)
    }

    /// Remove a push-frame callback listener.
    pub fn off(&self, id: ListenerId) {
        self.inner.lock().expect("account").emitter.off(id);
    }

    /// Stream payloads for one push frame type.
    pub fn events(&self, kind: impl Into<String>) -> EventStream<Value> {
        self.inner.lock().expect("account").emitter.stream(kind)
    }

    /// Stream payloads for one push frame type after deserializing them into `T`.
    ///
    /// Payloads that fail to decode are skipped; raw streams from [`Self::events`]
    /// still receive every payload.
    pub fn typed_events<T>(&self, kind: impl Into<String>) -> EventStream<T>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let mut raw = self.events(kind);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(payload) = raw.recv().await {
                let Ok(decoded) = serde_json::from_value(payload) else {
                    continue;
                };
                if tx.send(decoded).is_err() {
                    break;
                }
            }
        });
        rx
    }

    /// Stream every push frame.
    pub fn any_events(&self) -> EventStream<RawFrame> {
        self.inner.lock().expect("account").emitter.any_stream()
    }

    /// Listen for state sections changed by server state snapshots and deltas.
    pub fn on_state_change<F>(&self, handler: F)
    where
        F: Fn(&[StateSection]) + Send + Sync + 'static,
    {
        self.inner
            .lock()
            .expect("account")
            .state_listeners
            .push(Arc::new(handler));
    }

    /// Fire after automatic reconnect and re-authentication succeeds.
    pub fn on_reconnected<F>(&self, handler: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.inner
            .lock()
            .expect("account")
            .reconnected_listeners
            .push(Arc::new(handler));
    }

    /// Fire at the start of each automatic reconnect attempt (one-based).
    pub fn on_reconnecting<F>(&self, handler: F)
    where
        F: Fn(u64) + Send + Sync + 'static,
    {
        self.inner
            .lock()
            .expect("account")
            .reconnecting_listeners
            .push(Arc::new(handler));
    }

    /// Fire when a socket close is terminal and no reconnect will run.
    pub fn on_disconnected<F>(&self, handler: F)
    where
        F: Fn(ConnectionClosedError) + Send + Sync + 'static,
    {
        self.inner
            .lock()
            .expect("account")
            .disconnected_listeners
            .push(Arc::new(handler));
    }

    /// Subscribe to live market updates at the current station.
    pub fn subscribe_market(&self) -> AccountFuture<Value> {
        let query = self.query("spacemolt_market", "subscribe_market", None);
        let account = self.clone();
        Box::pin(async move {
            let result = query.await?;
            let snapshot = result.structured_content.unwrap_or(Value::Null);
            if !snapshot.is_null() {
                account.seed_market_subscription(&snapshot);
            }
            Ok(snapshot)
        })
    }

    /// Cached station market book, when subscribed and seeded.
    pub fn market(&self, base_id: &str) -> Option<MarketBook> {
        self.inner
            .lock()
            .expect("account")
            .market_cache
            .book(base_id)
            .cloned()
    }

    /// True when the account believes its live market subscription is active.
    pub fn market_subscribed(&self) -> bool {
        self.inner.lock().expect("account").market_subscribed
    }

    /// Subscribe to live presence updates at the current point of interest.
    pub fn subscribe_observation(&self, active_scan: bool) -> AccountFuture<Value> {
        let payload = active_scan.then(|| serde_json::json!({ "active_scan": true }));
        let query = self.query("spacemolt", "subscribe_observation", payload);
        let account = self.clone();
        Box::pin(async move {
            let result = query.await?;
            let snapshot = result.structured_content.unwrap_or(Value::Null);
            if !snapshot.is_null() {
                account.seed_observation_subscription(&snapshot, active_scan);
            }
            Ok(snapshot)
        })
    }

    /// Current subscribed observation view.
    pub fn observation(&self) -> Option<ObservationView> {
        self.inner
            .lock()
            .expect("account")
            .observation_cache
            .current()
            .cloned()
    }

    /// True when the account believes its live observation subscription is active.
    pub fn observation_subscribed(&self) -> bool {
        self.inner.lock().expect("account").observation_subscribed
    }

    pub fn observation_active_scan(&self) -> bool {
        self.inner.lock().expect("account").observation_active_scan
    }

    pub fn player(&self) -> Option<crate::schema::V2GameStatePlayer> {
        self.state().player().ok().flatten()
    }
    pub fn ship(&self) -> Option<crate::schema::V2GameStateShip> {
        self.state().ship().ok().flatten()
    }
    pub fn location(&self) -> Option<crate::schema::V2GameStateLocation> {
        self.state().location().ok().flatten()
    }
    pub fn cargo(&self) -> Option<Vec<crate::schema::V2GameStateCargoItem>> {
        self.state().cargo().ok().flatten()
    }
    pub fn skills(&self) -> Option<HashMap<String, crate::schema::V2GameStateSkillsValue>> {
        self.state().skills().ok().flatten()
    }
    pub fn credits(&self) -> Option<i64> {
        self.state().credits()
    }
    pub fn has_pending_action(&self) -> bool {
        self.state().has_pending_action()
    }

    /// Re-seed the cache from the canonical full state.
    pub fn refresh(&self) -> AccountFuture<Value> {
        let account = self.clone();
        Box::pin(async move {
            let result = account.query("spacemolt", "get_status", None).await?;
            let snapshot = result
                .structured_content
                .or_else(|| result.result.is_object().then_some(result.result))
                .unwrap_or(Value::Null);
            if snapshot.is_object() {
                let (listeners, changed) = {
                    let mut inner = account.inner.lock().expect("account");
                    let changed = inner.cache.seed(&snapshot);
                    apply_silent_subscription_drops(&mut inner);
                    (inner.state_listeners.clone(), changed)
                };
                if !changed.is_empty() {
                    emit_state_change(&listeners, &changed);
                }
            }
            Ok(account.state_snapshot())
        })
    }

    /// Refresh only the authoritative cargo section from the lean cargo query.
    pub fn refresh_cargo(&self) -> AccountFuture<Value> {
        let account = self.clone();
        Box::pin(async move {
            account.refresh_sections(&[StateSection::Cargo]).await?;
            Ok(account
                .inner
                .lock()
                .expect("account")
                .cache
                .raw_cargo()
                .cloned()
                .unwrap_or(Value::Null))
        })
    }

    /// Refresh selected cache sections without clearing unrelated state.
    pub fn refresh_sections(&self, sections: &[StateSection]) -> AccountFuture<Vec<StateSection>> {
        let account = self.clone();
        let sections = sections.to_vec();
        Box::pin(async move {
            let mut refreshed = Vec::new();
            for section in sections {
                if refreshed.contains(&section) {
                    continue;
                }
                let (action, returned) = match section {
                    StateSection::Player => ("get_player", &[StateSection::Player][..]),
                    StateSection::Ship | StateSection::Modules => {
                        ("get_ship", &[StateSection::Ship, StateSection::Modules][..])
                    }
                    StateSection::Cargo => ("get_cargo", &[StateSection::Cargo][..]),
                    StateSection::Location => ("get_location", &[StateSection::Location][..]),
                    StateSection::Skills => ("get_skills", &[StateSection::Skills][..]),
                    StateSection::Queue => ("get_queue", &[StateSection::Queue][..]),
                    StateSection::Missions => {
                        ("get_active_missions", &[StateSection::Missions][..])
                    }
                };
                let result = account.query("spacemolt", action, None).await?;
                let snapshot = result
                    .structured_content
                    .or_else(|| result.result.is_object().then_some(result.result))
                    .unwrap_or(Value::Null);
                let (listeners, changed) = {
                    let mut inner = account.inner.lock().expect("account");
                    let mut changed = Vec::new();
                    for candidate in returned.iter().copied() {
                        let value = lean_section_value(candidate, &snapshot);
                        if let Some(value) = value {
                            changed.extend(inner.cache.replace_section(
                                candidate,
                                value,
                                &format!("spacemolt/{action}"),
                            ));
                        }
                    }
                    (inner.state_listeners.clone(), changed)
                };
                emit_state_change(&listeners, &changed);
                refreshed.extend(changed);
            }
            Ok(refreshed)
        })
    }

    /// Refresh and cache a storage view without replacing another station's view.
    pub fn refresh_storage(
        &self,
        target: StorageTarget,
        station_id: impl Into<String>,
    ) -> AccountFuture<crate::schema::StorageResponse> {
        let account = self.clone();
        let station_id = station_id.into();
        Box::pin(async move {
            let target_name = match target {
                StorageTarget::Personal => "self",
                StorageTarget::Faction => "faction",
            };
            let result = account
                .query(
                    "spacemolt_storage",
                    "view",
                    Some(serde_json::json!({
                        "target": target_name, "station_id": station_id,
                    })),
                )
                .await?;
            let view = result
                .structured_content
                .or_else(|| result.result.is_object().then_some(result.result))
                .unwrap_or(Value::Null);
            let view: crate::schema::StorageResponse =
                serde_json::from_value(view).map_err(|err| {
                    ClientError::Server(crate::errors::SpacemoltError::new(
                        "invalid_storage_response",
                        err.to_string(),
                    ))
                })?;
            let owner = account.storage_owner_key(target);
            account
                .inner
                .lock()
                .expect("account")
                .storage_cache
                .insert((owner, station_id), view.clone());
            Ok(view)
        })
    }

    pub fn storage(
        &self,
        target: StorageTarget,
        station_id: &str,
    ) -> Option<crate::schema::StorageResponse> {
        let owner = self.storage_owner_key(target);
        self.inner
            .lock()
            .expect("account")
            .storage_cache
            .get(&(owner, station_id.to_string()))
            .cloned()
    }

    fn storage_owner_key(&self, target: StorageTarget) -> String {
        match target {
            StorageTarget::Personal => {
                format!("account:{}", self.opts.id.as_deref().unwrap_or("unknown"))
            }
            StorageTarget::Faction => {
                let faction = self
                    .player()
                    .and_then(|player| player.faction_id)
                    .unwrap_or_else(|| "unknown".to_string());
                format!("faction:{faction}")
            }
        }
    }

    /// Log out and clear the local authenticated marker.
    pub fn logout(&self) -> AccountFuture<()> {
        let account = self.clone();
        Box::pin(async move {
            account.query("spacemolt_auth", "logout", None).await?;
            account.inner.lock().expect("account").authenticated = false;
            Ok(())
        })
    }

    /// Stop market updates and discard the subscribed book.
    pub fn unsubscribe_market(&self) -> AccountFuture<()> {
        let account = self.clone();
        Box::pin(async move {
            account
                .query("spacemolt_market", "unsubscribe_market", None)
                .await?;
            let mut inner = account.inner.lock().expect("account");
            if let Some(base) = inner.subscribed_market_base_id.take() {
                inner.market_cache.drop(&base);
            }
            inner.market_subscribed = false;
            Ok(())
        })
    }

    /// Stop observation updates and discard the current observation view.
    pub fn unsubscribe_observation(&self) -> AccountFuture<()> {
        let account = self.clone();
        Box::pin(async move {
            account
                .query("spacemolt", "unsubscribe_observation", None)
                .await?;
            let mut inner = account.inner.lock().expect("account");
            inner.observation_cache.clear();
            inner.observation_subscribed = false;
            inner.observation_active_scan = false;
            inner.subscribed_observation_poi_id = None;
            Ok(())
        })
    }

    fn seed_market_subscription(&self, snapshot: &Value) {
        let mut inner = self.inner.lock().expect("account");
        if let Ok(snapshot) = serde_json::from_value(snapshot.clone()) {
            let base_id = inner.market_cache.seed(snapshot);
            inner.market_subscribed = true;
            inner.subscribed_market_base_id = Some(base_id);
        }
    }

    fn seed_observation_subscription(&self, snapshot: &Value, active_scan: bool) {
        let listeners = {
            let mut inner = self.inner.lock().expect("account");
            let Ok(snapshot) = serde_json::from_value(snapshot.clone()) else {
                return;
            };
            let view = inner.observation_cache.seed(snapshot);
            let view_active_scan = view.active_scan;
            let view_poi_id = view.poi_id.clone();
            inner.observation_subscribed = true;
            inner.observation_active_scan = active_scan || view_active_scan;
            inner.subscribed_observation_poi_id = view_poi_id;
            let changed = bridge_observation_to_location(&mut inner);
            if changed.is_empty() {
                None
            } else {
                Some((inner.state_listeners.clone(), changed))
            }
        };
        if let Some((listeners, changed)) = listeners {
            emit_state_change(&listeners, &changed);
        }
    }

    /// Authenticate with username and password.
    pub fn login(&self, params: LoginParams) -> AccountFuture<Value> {
        let first = self.start_login_once(params.clone());
        let account = self.clone();
        let max_retries = self.opts.max_rate_limit_retries;
        Box::pin(async move {
            let mut attempts = 0;
            let mut next = first;
            loop {
                match next.await {
                    Err(ClientError::Server(err))
                        if attempts < max_retries && err.code == "rate_limited" =>
                    {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(retry_after_ms(&err))).await;
                        next = account.start_login_once(params.clone());
                    }
                    other => return other,
                }
            }
        })
    }

    /// Authenticate with a short-lived login token.
    pub fn login_token(&self, token: impl Into<String>) -> AccountFuture<Value> {
        self.auth_action("login_token", serde_json::json!({ "token": token.into() }))
    }

    fn start_login_once(&self, params: LoginParams) -> AccountFuture<Value> {
        self.auth_action(
            "login",
            serde_json::json!({
                "username": params.username,
                "password": params.password,
            }),
        )
    }

    /// Authenticate with a stored credential variant.
    pub fn authenticate(&self, credentials: AuthCredentials) -> AccountFuture<()> {
        match credentials {
            AuthCredentials::Login { username, password } => {
                let login = self.login(LoginParams { username, password });
                let account = self.clone();
                Box::pin(async move {
                    login.await?;
                    account.seed_state_after_auth().await?;
                    Ok(())
                })
            }
            AuthCredentials::LoginToken { token } => {
                let auth = self.auth_action("login_token", serde_json::json!({ "token": token }));
                let account = self.clone();
                Box::pin(async move {
                    auth.await?;
                    account.seed_state_after_auth().await?;
                    Ok(())
                })
            }
            AuthCredentials::Clerk {
                player_id,
                api_key,
                http_base_url,
            } => {
                let account = self.clone();
                let http = self.clerk_http_client();
                let max_retries = self.opts.max_rate_limit_retries;
                Box::pin(async move {
                    let mut attempts = 0;
                    loop {
                        let token =
                            mint_ws_token(&http_base_url, &api_key, &player_id, Arc::clone(&http))
                                .await
                                .map_err(clerk_error)?;
                        match account
                            .authenticate(AuthCredentials::LoginToken { token })
                            .await
                        {
                            Err(ClientError::Server(err))
                                if attempts < max_retries && err.code == "rate_limited" =>
                            {
                                attempts += 1;
                                tokio::time::sleep(Duration::from_millis(retry_after_ms(&err)))
                                    .await;
                            }
                            other => return other,
                        }
                    }
                })
            }
        }
    }

    async fn seed_state_after_auth(&self) -> Result<(), ClientError> {
        if !self.opts.seed_state {
            return Ok(());
        }
        let result = self.query("spacemolt", "get_status", None).await?;
        let snapshot = result
            .structured_content
            .or_else(|| result.result.is_object().then_some(result.result));
        let Some(snapshot) = snapshot else {
            return Ok(());
        };
        let changed = {
            let mut inner = self.inner.lock().expect("account");
            inner.cache.seed(&snapshot)
        };
        if !changed.is_empty() {
            let listeners = self.inner.lock().expect("account").state_listeners.clone();
            emit_state_change(&listeners, &changed);
        }
        Ok(())
    }

    fn clerk_http_client(&self) -> Arc<dyn ClerkHttpClient> {
        self.opts
            .clerk_http_client
            .clone()
            .unwrap_or_else(|| Arc::new(ReqwestClerkHttpClient::default()))
    }

    fn auth_action(&self, action: &str, payload: Value) -> AccountFuture<Value> {
        let (tx, rx) = oneshot::channel();
        if let Err(err) = self.start_auth(action, Some(payload), PendingAuthTx::Login(tx)) {
            return ready_err(err);
        }
        let account = self.clone();
        let timeout_ms = self.opts.connect_timeout_ms;
        Box::pin(async move {
            match tokio::time::timeout(Duration::from_millis(timeout_ms), await_auth_receiver(rx))
                .await
            {
                Ok(result) => result,
                Err(_) => {
                    account.inner.lock().expect("account").pending_auth.take();
                    Err(ClientError::Timeout(format!(
                        "No auth response received within {timeout_ms}ms"
                    )))
                }
            }
        })
    }

    /// Register a new account and resolve with generated credentials plus state.
    pub fn register(&self, params: RegisterParams) -> AccountFuture<RegisterResult> {
        if params.username.trim().is_empty()
            || params.empire.trim().is_empty()
            || params.registration_code.trim().is_empty()
        {
            return ready_err(ClientError::Server(SpacemoltError::new(
                "invalid_registration",
                "username, empire, and registration code must not be blank",
            )));
        }
        let (tx, rx) = oneshot::channel();
        let mut payload = serde_json::Map::new();
        payload.insert("username".to_string(), Value::String(params.username));
        payload.insert("empire".to_string(), Value::String(params.empire));
        payload.insert(
            "registration_code".to_string(),
            Value::String(params.registration_code),
        );
        if let Err(err) = self.start_auth(
            "register",
            Some(Value::Object(payload)),
            PendingAuthTx::Register(tx),
        ) {
            return ready_err(err);
        }
        let account = self.clone();
        let timeout_ms = self.opts.connect_timeout_ms;
        Box::pin(async move {
            match tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                await_register_receiver(rx),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    account.inner.lock().expect("account").pending_auth.take();
                    Err(ClientError::Timeout(format!(
                        "No register response received within {timeout_ms}ms"
                    )))
                }
            }
        })
    }

    fn start_auth(
        &self,
        action: &str,
        payload: Option<Value>,
        tx: PendingAuthTx,
    ) -> Result<(), ClientError> {
        let request_id = self.next_request_id();
        let mut inner = self.inner.lock().expect("account");
        if inner.pending_auth.is_some() {
            return Err(ClientError::Server(SpacemoltError::new(
                "auth_in_progress",
                "an auth exchange is already in flight",
            )));
        }
        if inner.authenticated {
            return Err(ClientError::Server(SpacemoltError::new(
                "already_authenticated",
                "this connection is already authenticated",
            )));
        }
        let Some(socket) = inner.socket.clone() else {
            return Err(ClientError::ConnectionClosed(ConnectionClosedError::new(
                "cannot authenticate before connect",
                None,
                None,
            )));
        };
        inner.pending_auth = Some(PendingAuth {
            request_id: request_id.clone(),
            registered: None,
            tx,
        });
        if let Err(err) = socket.send(InboundFrame {
            tool: "spacemolt_auth".to_string(),
            action: action.to_string(),
            payload,
            request_id: Some(request_id),
        }) {
            let pending = inner.pending_auth.take();
            drop(pending);
            return Err(ClientError::ConnectionClosed(err));
        }
        Ok(())
    }

    /// Low-level query call.
    pub fn query(
        &self,
        tool: &str,
        action: &str,
        payload: Option<Value>,
    ) -> AccountFuture<QueryResult> {
        let Some(def) = find_action_parts(tool, action) else {
            return ready_err(ClientError::UnknownAction(format!("{tool}/{action}")));
        };
        if def.kind != ActionKind::Query {
            return ready_err(ClientError::UnknownAction(format!(
                "{tool}/{action} is a mutation, not a query"
            )));
        }
        let first = self.start_query_once(tool.to_string(), action.to_string(), payload.clone());
        let account = self.clone();
        let tool = tool.to_string();
        let action = action.to_string();
        let max_retries = self.opts.max_rate_limit_retries;
        Box::pin(async move {
            let mut attempts = 0;
            let mut next = first;
            loop {
                match next.await {
                    Err(ClientError::Server(err))
                        if attempts < max_retries && err.code == "rate_limited" =>
                    {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(retry_after_ms(&err))).await;
                        next =
                            account.start_query_once(tool.clone(), action.clone(), payload.clone());
                    }
                    other => return other,
                }
            }
        })
    }

    fn start_query_once(
        &self,
        tool: String,
        action: String,
        payload: Option<Value>,
    ) -> AccountFuture<QueryResult> {
        let request_id = self.next_request_id();
        let rx = {
            let mut inner = self.inner.lock().expect("account");
            let Some(socket) = inner.socket.clone() else {
                return ready_err(ClientError::ConnectionClosed(ConnectionClosedError::new(
                    "cannot send before connect",
                    None,
                    None,
                )));
            };
            let rx = inner.correlator.await_query(request_id.clone());
            if let Err(err) = socket.send(InboundFrame {
                tool: tool.clone(),
                action: action.clone(),
                payload,
                request_id: Some(request_id.clone()),
            }) {
                inner.correlator.cancel(&request_id);
                return ready_err(ClientError::ConnectionClosed(err));
            }
            rx
        };
        let timeout_ms = self.opts.query_timeout_ms;
        let inner_for_timeout = Arc::clone(&self.inner);
        let timeout_message = format!("No response to {tool}/{action} within {timeout_ms}ms");
        Box::pin(async move {
            match tokio::time::timeout(Duration::from_millis(timeout_ms), await_query_receiver(rx))
                .await
            {
                Ok(result) => result,
                Err(_) => {
                    inner_for_timeout
                        .lock()
                        .expect("account")
                        .correlator
                        .cancel(&request_id);
                    Err(ClientError::Timeout(timeout_message))
                }
            }
        })
    }

    /// Low-level mutation call.
    pub fn mutate(
        &self,
        tool: &str,
        action: &str,
        payload: Option<Value>,
    ) -> AccountFuture<MutationResult> {
        self.mutate_inner(tool, action, payload, None)
    }

    /// Low-level mutation call with a pending-ack callback.
    pub fn mutate_with_ack<F>(
        &self,
        tool: &str,
        action: &str,
        payload: Option<Value>,
        on_ack: F,
    ) -> AccountFuture<MutationResult>
    where
        F: FnMut(MutationAck) + Send + 'static,
    {
        self.mutate_inner(tool, action, payload, Some(Box::new(on_ack)))
    }

    fn mutate_inner(
        &self,
        tool: &str,
        action: &str,
        payload: Option<Value>,
        on_ack: Option<Box<dyn FnMut(MutationAck) + Send>>,
    ) -> AccountFuture<MutationResult> {
        let Some(def) = find_action_parts(tool, action) else {
            return ready_err(ClientError::UnknownAction(format!("{tool}/{action}")));
        };
        if def.kind != ActionKind::Mutation {
            return ready_err(ClientError::UnknownAction(format!(
                "{tool}/{action} is a query, not a mutation"
            )));
        }
        let mutation_timeout_ms = if TRANSIT_ACTIONS.contains(&format!("{tool}/{action}").as_str())
        {
            self.opts.mutation_timeout_ms
        } else {
            self.opts.fast_mutation_timeout_ms
        };
        let waiter = {
            let mut inner = self.inner.lock().expect("account");
            if inner.mutation_active {
                let (tx, rx) = oneshot::channel();
                inner.mutation_waiters.push_back(tx);
                Some(rx)
            } else {
                inner.mutation_active = true;
                None
            }
        };
        let account = self.clone();
        let tool = tool.to_string();
        let action = action.to_string();
        if let Some(waiter) = waiter {
            return Box::pin(async move {
                waiter.await.map_err(|_| {
                    ClientError::ConnectionClosed(ConnectionClosedError::new(
                        "mutation canceled",
                        None,
                        None,
                    ))
                })?;
                let permit = MutationPermit::new(Arc::clone(&account.inner));
                account
                    .start_mutation_now(tool, action, payload, on_ack, mutation_timeout_ms, permit)
                    .await
            });
        }
        self.start_mutation_now(
            tool,
            action,
            payload,
            on_ack,
            mutation_timeout_ms,
            MutationPermit::new(Arc::clone(&self.inner)),
        )
    }

    fn start_mutation_now(
        &self,
        tool: String,
        action: String,
        payload: Option<Value>,
        on_ack: Option<Box<dyn FnMut(MutationAck) + Send>>,
        mutation_timeout_ms: u64,
        permit: MutationPermit,
    ) -> AccountFuture<MutationResult> {
        let on_ack = Arc::new(Mutex::new(on_ack));
        let account = self.clone();
        let max_retries = self.opts.max_rate_limit_retries;
        let first = self.start_mutation_attempt(
            tool.clone(),
            action.clone(),
            payload.clone(),
            Arc::clone(&on_ack),
            mutation_timeout_ms,
        );
        Box::pin(async move {
            // Keep ownership of the mutation slot for the lifetime of this
            // future. Its Drop implementation also runs when this future is
            // cancelled before the upstream request completes.
            let _permit = permit;
            let mut attempts = 0;
            let mut next = first;
            loop {
                match next.await {
                    Err(ClientError::Server(err))
                        if attempts < max_retries && err.code == "rate_limited" =>
                    {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(retry_after_ms(&err))).await;
                        next = account.start_mutation_attempt(
                            tool.clone(),
                            action.clone(),
                            payload.clone(),
                            Arc::clone(&on_ack),
                            mutation_timeout_ms,
                        );
                    }
                    other => {
                        if let Ok(result) = &other {
                            if result.auto_docked || result.auto_undocked {
                                if let Err(error) =
                                    account.refresh_sections(&[StateSection::Location]).await
                                {
                                    account
                                        .inner
                                        .lock()
                                        .expect("account")
                                        .cache
                                        .mark_dirty(StateSection::Location);
                                    warn!(
                                        tool,
                                        action,
                                        auto_docked = result.auto_docked,
                                        auto_undocked = result.auto_undocked,
                                        error = %error,
                                        "post-transition location refresh failed; mutation remains successful"
                                    );
                                }
                            }
                            if let Some(policy) = reconciliation_policy(&tool, &action) {
                                if let Err(error) = account.refresh_sections(policy.sections).await
                                {
                                    let mut inner = account.inner.lock().expect("account");
                                    for section in policy.sections {
                                        inner.cache.mark_dirty(*section);
                                    }
                                    drop(inner);
                                    warn!(
                                        tool,
                                        action,
                                        error = %error,
                                        "post-mutation section refresh failed; mutation remains successful"
                                    );
                                }
                            }
                            // SPACEMOLT_INCOMPLETE_DELTA_WORKAROUND: storage deposit/withdraw
                            // omits its station storage view; remove after telemetry shows that
                            // the destination/source storage is consistently authoritative.
                            let storage_target = match (tool.as_str(), action.as_str()) {
                                ("spacemolt_storage", "deposit" | "withdraw") => Some(
                                    if payload
                                        .as_ref()
                                        .and_then(|p| p.get("target"))
                                        .and_then(Value::as_str)
                                        == Some("faction")
                                    {
                                        StorageTarget::Faction
                                    } else {
                                        StorageTarget::Personal
                                    },
                                ),
                                ("spacemolt", "buy")
                                    if payload
                                        .as_ref()
                                        .and_then(|p| p.get("deliver_to"))
                                        .and_then(Value::as_str)
                                        == Some("storage") =>
                                {
                                    Some(StorageTarget::Personal)
                                }
                                _ => None,
                            };
                            if let (Some(target), Some(station_id)) =
                                (storage_target, account.current_station_id())
                            {
                                if let Err(error) =
                                    account.refresh_storage(target, station_id).await
                                {
                                    warn!(tool, action, error = %error, "post-mutation storage refresh failed; mutation remains successful");
                                }
                            }
                        }
                        return other;
                    }
                }
            }
        })
    }

    fn current_station_id(&self) -> Option<String> {
        let location = self.location()?;
        location.docked_at.or(location.poi_id)
    }

    fn start_mutation_attempt(
        &self,
        tool: String,
        action: String,
        payload: Option<Value>,
        on_ack_handler: Arc<Mutex<Option<Box<dyn FnMut(MutationAck) + Send>>>>,
        mutation_timeout_ms: u64,
    ) -> AccountFuture<MutationResult> {
        let request_id = self.next_request_id();
        let (ack_tx, ack_rx) = oneshot::channel();
        let mut ack_tx = Some(ack_tx);
        let on_ack = Box::new(move |ack: MutationAck| {
            if let Some(tx) = ack_tx.take() {
                let _ = tx.send(());
            }
            let mut handler = on_ack_handler.lock().expect("ack handler");
            if let Some(handler) = handler.as_mut() {
                handler(ack);
            }
        });
        let rx = {
            let mut inner = self.inner.lock().expect("account");
            let Some(socket) = inner.socket.clone() else {
                return ready_err(ClientError::ConnectionClosed(ConnectionClosedError::new(
                    "cannot send before connect",
                    None,
                    None,
                )));
            };
            let rx = inner
                .correlator
                .await_mutation(request_id.clone(), Some(on_ack));
            if let Err(err) = socket.send(InboundFrame {
                tool,
                action,
                payload,
                request_id: Some(request_id.clone()),
            }) {
                inner.correlator.cancel(&request_id);
                return ready_err(ClientError::ConnectionClosed(err));
            }
            rx
        };
        let inner = Arc::clone(&self.inner);
        let query_timeout_ms = self.opts.query_timeout_ms;
        Box::pin(async move {
            let result = await_mutation_with_timeout(
                Arc::clone(&inner),
                request_id,
                rx,
                ack_rx,
                query_timeout_ms,
                mutation_timeout_ms,
            )
            .await;
            result
        })
    }

    /// Route a command through query or mutation classification.
    pub fn send(
        &self,
        tool: &str,
        action: &str,
        payload: Option<Value>,
    ) -> AccountFuture<CommandResult> {
        let Some(def) = find_action_parts(tool, action) else {
            return ready_err(ClientError::UnknownAction(format!("{tool}/{action}")));
        };
        match def.kind {
            ActionKind::Query => {
                let rx = self.query(tool, action, payload);
                Box::pin(async move { rx.await.map(CommandResult::Query) })
            }
            ActionKind::Mutation => {
                let rx = self.mutate(tool, action, payload);
                Box::pin(async move { rx.await.map(CommandResult::Mutation) })
            }
        }
    }

    /// Route a generated `tool/action` key without making callers split it.
    pub fn send_action(&self, key: &str, payload: Option<Value>) -> AccountFuture<CommandResult> {
        let Some(def) = crate::actions::find_action(key) else {
            return ready_err(ClientError::UnknownAction(key.to_string()));
        };
        self.send(def.tool, def.action, payload)
    }

    /// Produce a request id compatible with WebSocket correlation.
    pub fn next_request_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn handle_close(&self, err: ConnectionClosedError) {
        let (should_reconnect, disconnected_listeners) = {
            let mut inner = self.inner.lock().expect("account");
            inner.authenticated = false;
            if let Some(pending) = inner.pending_auth.take() {
                pending.reject(ClientError::ConnectionClosed(err.clone()));
            }
            inner.welcome_waiter.take();
            inner.correlator.reject_all_connection_closed(err.clone());
            inner.emitter.close_streams();
            inner.socket = None;

            let reconnectable = self.opts.reconnect.is_some()
                && self.opts.credentials.is_some()
                && !inner.reconnecting
                && err.code != Some(CLOSE_CODE_SESSION_REPLACED)
                && err.code != Some(CLOSE_CODE_AUTH_TIMEOUT);
            if reconnectable {
                inner.reconnecting = true;
                (true, Vec::new())
            } else {
                (false, inner.disconnected_listeners.clone())
            }
        };

        if should_reconnect {
            let account = self.clone();
            tokio::spawn(async move {
                account.reconnect_loop(err).await;
            });
        } else {
            emit_disconnected(&disconnected_listeners, err);
        }
    }

    async fn reconnect_loop(self, close_err: ConnectionClosedError) {
        let Some(config) = self.opts.reconnect else {
            self.finish_reconnect_failure(close_err);
            return;
        };
        let Some(credentials) = self.opts.credentials.clone() else {
            self.finish_reconnect_failure(close_err);
            return;
        };

        let retry_after = retry_after_ms_from_close(&close_err);
        let max_retries = config.max_retries.unwrap_or(u64::MAX);
        for attempt in 1..=max_retries {
            let reconnecting_listeners = self
                .inner
                .lock()
                .expect("account")
                .reconnecting_listeners
                .clone();
            for listener in reconnecting_listeners {
                let _ =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(attempt)));
            }
            let delay = if attempt == 1 {
                retry_after.unwrap_or(config.base_delay_ms)
            } else {
                config
                    .base_delay_ms
                    .saturating_mul(2_u64.saturating_pow((attempt - 1).min(32) as u32))
                    .min(config.max_delay_ms)
            };
            tokio::time::sleep(Duration::from_millis(delay)).await;

            let result = async {
                self.connect().await?;
                self.wait_for_welcome().await?;
                self.authenticate(credentials.clone()).await?;
                self.restore_subscriptions().await?;
                Ok::<(), ClientError>(())
            }
            .await;

            if result.is_ok() {
                let listeners = {
                    let mut inner = self.inner.lock().expect("account");
                    inner.reconnecting = false;
                    inner.reconnected_listeners.clone()
                };
                emit_reconnected(&listeners);
                return;
            }
        }

        self.finish_reconnect_failure(close_err);
    }

    fn finish_reconnect_failure(&self, err: ConnectionClosedError) {
        let listeners = {
            let mut inner = self.inner.lock().expect("account");
            inner.reconnecting = false;
            inner.disconnected_listeners.clone()
        };
        emit_disconnected(&listeners, err);
    }

    async fn restore_subscriptions(&self) -> Result<(), ClientError> {
        let (restore_market, restore_observation, active_scan) = {
            let inner = self.inner.lock().expect("account");
            (
                inner.market_subscribed,
                inner.observation_subscribed,
                inner.observation_active_scan,
            )
        };
        if restore_market {
            self.subscribe_market().await?;
        }
        if restore_observation {
            self.subscribe_observation(active_scan).await?;
        }
        Ok(())
    }
}

/// Extract a cache section from the query's structured presentation envelope.
/// Lean endpoints do not consistently return the same shape as `get_status`.
fn lean_section_value(section: StateSection, snapshot: &Value) -> Option<Value> {
    if let Some(value) = snapshot.get(section.as_str()) {
        return Some(value.clone());
    }

    match section {
        StateSection::Cargo => snapshot
            .get("items")
            .filter(|value| value.is_array())
            .cloned()
            // An empty get_cargo response omits `items`; its presentation
            // envelope still identifies itself with this message.
            .or_else(|| {
                (snapshot.get("message").and_then(Value::as_str) == Some("Cargo contents"))
                    .then(|| Value::Array(Vec::new()))
            }),
        StateSection::Ship => snapshot.get("ship").cloned().or_else(|| {
            snapshot
                .get("cargo_capacity")
                .is_some()
                .then(|| snapshot.clone())
        }),
        StateSection::Modules => snapshot.get("modules").cloned(),
        // These lean queries commonly return the section object directly.
        StateSection::Player
        | StateSection::Location
        | StateSection::Missions
        | StateSection::Queue
        | StateSection::Skills => snapshot.is_object().then(|| snapshot.clone()),
    }
}

#[cfg(test)]
mod lean_section_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cargo_envelope_extracts_items_instead_of_caching_the_envelope() {
        let envelope = json!({
            "credits": 42,
            "message": "Cargo contents",
            "ship": { "cargo_used": 2 },
            "items": [{ "item_id": "ore", "quantity": 2 }]
        });
        assert_eq!(
            lean_section_value(StateSection::Cargo, &envelope),
            Some(json!([
                { "item_id": "ore", "quantity": 2 }
            ]))
        );
    }

    #[test]
    fn empty_cargo_envelope_becomes_an_empty_array() {
        let envelope = json!({
            "credits": 42,
            "message": "Cargo contents",
            "ship": { "cargo_used": 0 }
        });
        assert_eq!(
            lean_section_value(StateSection::Cargo, &envelope),
            Some(json!([]))
        );
    }

    #[test]
    fn cargo_does_not_accept_an_unrecognized_object() {
        assert_eq!(
            lean_section_value(StateSection::Cargo, &json!({"credits": 42})),
            None
        );
    }

    #[test]
    fn immediate_market_order_paths_force_cargo_and_ship_reconciliation() {
        for action in ["create_sell_order", "create_buy_order"] {
            let policy = reconciliation_policy("spacemolt_market", action).expect("policy");
            assert_eq!(policy.sections, CARGO_SHIP);
        }
    }
}

impl Default for AccountInner {
    fn default() -> Self {
        Self {
            cache: StateCache::default(),
            correlator: Correlator::default(),
            emitter: TypedEmitter::default(),
            socket: None,
            welcome: None,
            welcome_waiter: None,
            authenticated: false,
            login_payload: None,
            pending_auth: None,
            state_listeners: Vec::new(),
            reconnected_listeners: Vec::new(),
            reconnecting_listeners: Vec::new(),
            disconnected_listeners: Vec::new(),
            market_cache: MarketCache::default(),
            market_subscribed: false,
            subscribed_market_base_id: None,
            observation_cache: ObservationCache::default(),
            observation_subscribed: false,
            observation_active_scan: false,
            subscribed_observation_poi_id: None,
            mutation_active: false,
            mutation_waiters: VecDeque::new(),
            reconnecting: false,
            storage_cache: HashMap::new(),
        }
    }
}

fn route_frame(inner: &Weak<Mutex<AccountInner>>, frame: RawFrame) {
    let Some(inner) = inner.upgrade() else {
        return;
    };
    let mut inner = inner.lock().expect("account");
    match frame.kind.as_str() {
        "welcome" => {
            if let Some(payload) = frame.payload.as_ref() {
                if let Ok(welcome) = serde_json::from_value::<WelcomePayload>(payload.clone()) {
                    inner.welcome = Some(welcome.clone());
                    if let Some(waiter) = inner.welcome_waiter.take() {
                        let _ = waiter.send(welcome);
                    }
                }
            }
            return;
        }
        "logged_in" => {
            if let Some(payload) = frame.payload.as_ref() {
                inner.login_payload = Some(payload.clone());
                if let Some(pending) = inner.pending_auth.take() {
                    resolve_pending_auth(&mut inner, pending, payload.clone());
                    return;
                }
                inner.cache.seed(payload);
                inner.authenticated = true;
            }
            return;
        }
        "registered" => {
            if let Some(pending) = inner.pending_auth.as_mut() {
                if let Some(payload) = frame.payload.as_ref() {
                    if let Ok(registered) =
                        serde_json::from_value::<RegisteredPayload>(payload.clone())
                    {
                        pending.registered = Some(registered);
                    }
                }
            }
            return;
        }
        "error" => {
            let is_auth_error = inner
                .pending_auth
                .as_ref()
                .map(|pending| {
                    frame.request_id.as_deref() == Some(pending.request_id.as_str())
                        || frame.request_id.is_none()
                })
                .unwrap_or(false);
            if is_auth_error {
                if let Some(pending) = inner.pending_auth.take() {
                    pending.reject(ClientError::Server(error_from_raw(&frame)));
                }
                return;
            }
        }
        "action_result" => {
            let payload = frame.payload.as_ref();
            let delta = payload.and_then(|payload| payload.get("result"));
            let command = payload
                .and_then(|payload| payload.get("command"))
                .and_then(Value::as_str)
                .unwrap_or("(unknown)");
            let before_cargo = inner.cache.raw_cargo().cloned();
            let before_ship = inner.cache.raw_ship().cloned();
            info!(
                request_id = frame.request_id.as_deref().unwrap_or("(none)"),
                command,
                payload_keys = ?json_object_keys(payload),
                result_kind = json_value_kind(delta),
                result_keys = ?json_object_keys(delta),
                cargo_delta_present = delta.and_then(|value| value.get("cargo")).is_some(),
                cargo_delta_kind = json_value_kind(delta.and_then(|value| value.get("cargo"))),
                cargo_delta = ?delta.and_then(|value| value.get("cargo")),
                ship_delta_present = delta.and_then(|value| value.get("ship")).is_some(),
                ship_cargo_used_delta = delta
                    .and_then(|value| value.pointer("/ship/cargo_used"))
                    .and_then(|value| value.as_i64()),
                cached_cargo_before = ?before_cargo,
                cached_ship_cargo_used_before = before_ship
                    .as_ref()
                    .and_then(|value| value.get("cargo_used"))
                    .and_then(|value| value.as_i64()),
                "client action result state delta received"
            );
            if let Some(delta) = delta {
                let mut changed = inner.cache.apply_delta(delta);
                if delta.get(StateSection::Location.as_str()).is_some() {
                    apply_silent_subscription_drops(&mut inner);
                    merge_changed(&mut changed, bridge_observation_to_location(&mut inner));
                }
                if !changed.is_empty() {
                    emit_state_change(&inner.state_listeners, &changed);
                }
                info!(
                    request_id = frame.request_id.as_deref().unwrap_or("(none)"),
                    command,
                    changed_sections = ?changed,
                    cached_cargo_after = ?inner.cache.raw_cargo(),
                    cached_ship_cargo_used_after = inner
                        .cache
                        .raw_ship()
                        .and_then(|value| value.get("cargo_used"))
                        .and_then(|value| value.as_i64()),
                    "client action result state delta applied"
                );
            }
        }
        "market_update" => {
            if let Some(payload) = frame.payload.as_ref() {
                if let Ok(update) = serde_json::from_value(payload.clone()) {
                    inner.market_cache.apply_update(update);
                }
            }
        }
        "observation_update" => {
            if let Some(payload) = frame.payload.as_ref() {
                if let Ok(update) = serde_json::from_value(payload.clone()) {
                    inner.observation_cache.apply_update(update);
                }
                let changed = bridge_observation_to_location(&mut inner);
                if !changed.is_empty() {
                    emit_state_change(&inner.state_listeners, &changed);
                }
            }
        }
        _ => {}
    }

    let correlated = inner.correlator.handle(&frame);
    if frame.kind == "action_result" {
        info!(
            request_id = frame.request_id.as_deref().unwrap_or("(none)"),
            correlated, "client action result correlation completed"
        );
    }
    if !correlated {
        inner.emitter.emit(&frame);
    }
}

fn json_object_keys(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_object)
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

fn json_value_kind(value: Option<&Value>) -> &'static str {
    match value {
        None => "missing",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "bool",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn apply_silent_subscription_drops(inner: &mut AccountInner) {
    if inner.market_subscribed {
        if let Some(base_id) = inner.subscribed_market_base_id.clone() {
            if location_string(inner, "docked_at").as_deref() != Some(base_id.as_str()) {
                inner.market_cache.drop(&base_id);
                inner.market_subscribed = false;
                inner.subscribed_market_base_id = None;
            }
        }
    }

    if inner.observation_subscribed {
        if let Some(poi_id) = inner.subscribed_observation_poi_id.clone() {
            if location_string(inner, "poi_id").as_deref() != Some(poi_id.as_str()) {
                inner.observation_cache.clear();
                inner.observation_subscribed = false;
                inner.observation_active_scan = false;
                inner.subscribed_observation_poi_id = None;
            }
        }
    }
}

fn bridge_observation_to_location(inner: &mut AccountInner) -> Vec<StateSection> {
    let Some(view) = inner.observation_cache.current() else {
        return Vec::new();
    };
    let nearby_players = view
        .nearby
        .values()
        .filter_map(|player| serde_json::to_value(player).ok())
        .collect::<Vec<_>>();
    let mut patch = serde_json::Map::new();
    patch.insert(
        "nearby_player_count".to_string(),
        Value::from(nearby_players.len()),
    );
    patch.insert("nearby_players".to_string(), Value::Array(nearby_players));
    inner.cache.patch_section(StateSection::Location, patch)
}

fn location_string(inner: &AccountInner, key: &str) -> Option<String> {
    inner
        .cache
        .raw_section(StateSection::Location)
        .and_then(|location| location.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn merge_changed(target: &mut Vec<StateSection>, additional: Vec<StateSection>) {
    for section in additional {
        if !target.contains(&section) {
            target.push(section);
        }
    }
}

fn resolve_pending_auth(inner: &mut AccountInner, pending: PendingAuth, state: Value) {
    inner.login_payload = Some(state.clone());
    match pending.tx {
        PendingAuthTx::Login(tx) => {
            let changed = inner.cache.seed(&state);
            inner.authenticated = true;
            if !changed.is_empty() {
                emit_state_change(&inner.state_listeners, &changed);
            }
            let _ = tx.send(Ok(state));
        }
        PendingAuthTx::Register(tx) => {
            if let Some(registered) = pending.registered {
                let changed = inner.cache.seed(&state);
                inner.authenticated = true;
                if !changed.is_empty() {
                    emit_state_change(&inner.state_listeners, &changed);
                }
                let _ = tx.send(Ok(RegisterResult {
                    password: registered.password,
                    player_id: registered.player_id,
                    state,
                }));
            } else {
                let _ = tx.send(Err(ClientError::Server(SpacemoltError::new(
                    "missing_credentials",
                    "register succeeded but no credentials frame was received",
                ))));
            }
        }
    }
}

fn emit_state_change(listeners: &[StateChangeHandler], changed: &[StateSection]) {
    for listener in listeners.iter().cloned() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(changed)));
    }
}

fn emit_reconnected(listeners: &[ReconnectedHandler]) {
    for listener in listeners.iter().cloned() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener()));
    }
}

fn emit_disconnected(listeners: &[DisconnectedHandler], err: ConnectionClosedError) {
    for listener in listeners.iter().cloned() {
        let err = err.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(err)));
    }
}

impl PendingAuth {
    fn reject(self, err: ClientError) {
        match self.tx {
            PendingAuthTx::Login(tx) => {
                let _ = tx.send(Err(err));
            }
            PendingAuthTx::Register(tx) => {
                let _ = tx.send(Err(err));
            }
        }
    }
}

fn error_from_raw(frame: &RawFrame) -> SpacemoltError {
    let payload = frame.payload.as_ref();
    SpacemoltError {
        code: payload
            .and_then(|v| v.get("code"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        message: payload
            .and_then(|v| v.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        details: payload.and_then(|v| v.get("details")).cloned(),
        request_id: frame.request_id.clone(),
        command: None,
        tick: None,
        pending_command: payload
            .and_then(|v| v.get("pending_command"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn retry_after_ms(err: &SpacemoltError) -> u64 {
    err.retry_after()
        .map(|delay| u64::try_from(delay.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(1_000)
        .max(250)
}

fn clerk_error(message: String) -> ClientError {
    ClientError::Server(SpacemoltError::new("clerk_error", message))
}

fn ready_err<T: Send + 'static>(err: ClientError) -> AccountFuture<T> {
    Box::pin(async move { Err(err) })
}

async fn await_auth_receiver(
    rx: oneshot::Receiver<Result<Value, ClientError>>,
) -> Result<Value, ClientError> {
    rx.await.map_err(|_| {
        ClientError::ConnectionClosed(ConnectionClosedError::new("auth canceled", None, None))
    })?
}

async fn await_register_receiver(
    rx: oneshot::Receiver<Result<RegisterResult, ClientError>>,
) -> Result<RegisterResult, ClientError> {
    rx.await.map_err(|_| {
        ClientError::ConnectionClosed(ConnectionClosedError::new("auth canceled", None, None))
    })?
}

async fn await_mutation_with_timeout(
    inner: Arc<Mutex<AccountInner>>,
    request_id: String,
    rx: oneshot::Receiver<Result<MutationResult, ClientError>>,
    ack_rx: oneshot::Receiver<()>,
    query_timeout_ms: u64,
    mutation_timeout_ms: u64,
) -> Result<MutationResult, ClientError> {
    let result = await_mutation_receiver(rx);
    tokio::pin!(result);
    let ack = ack_rx;
    tokio::pin!(ack);

    tokio::select! {
        result = &mut result => result,
        _ = &mut ack => {
            let timeout_message = format!(
                "No action_result for mutation {request_id} within {mutation_timeout_ms}ms of its ack"
            );
            match tokio::time::timeout(Duration::from_millis(mutation_timeout_ms), &mut result).await {
                Ok(result) => result,
                Err(_) => {
                    inner.lock().expect("account").correlator.cancel(&request_id);
                    Err(ClientError::Timeout(timeout_message))
                }
            }
        }
        _ = tokio::time::sleep(Duration::from_millis(query_timeout_ms)) => {
            inner.lock().expect("account").correlator.cancel(&request_id);
            Err(ClientError::Timeout(format!(
                "No response to mutation {request_id} within {query_timeout_ms}ms"
            )))
        }
    }
}

fn finish_mutation(inner: &Arc<Mutex<AccountInner>>) {
    let mut inner = inner.lock().expect("account");
    while let Some(waiter) = inner.mutation_waiters.pop_front() {
        if waiter.send(()).is_ok() {
            return;
        }
    }
    inner.mutation_active = false;
}

async fn await_query_receiver(
    rx: oneshot::Receiver<Result<QueryResult, ClientError>>,
) -> Result<QueryResult, ClientError> {
    rx.await.map_err(|_| {
        ClientError::ConnectionClosed(ConnectionClosedError::new("request canceled", None, None))
    })?
}

async fn await_mutation_receiver(
    rx: oneshot::Receiver<Result<MutationResult, ClientError>>,
) -> Result<MutationResult, ClientError> {
    rx.await.map_err(|_| {
        ClientError::ConnectionClosed(ConnectionClosedError::new("request canceled", None, None))
    })?
}

/// Result of an auto-routed command.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    /// Query result.
    Query(QueryResult),
    /// Mutation result.
    Mutation(MutationResult),
}

impl CommandResult {
    /// Consume the protocol envelope and return its canonical payload.
    pub fn into_value(self) -> Value {
        match self {
            Self::Query(query) => query.structured_content.unwrap_or(query.result),
            Self::Mutation(mutation) => mutation.delta,
        }
    }
}
