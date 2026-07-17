//! Multi-account client.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::account::{Account, AccountOptions, ReconnectOptions, RegisterParams, RegisterResult};
use crate::auth::{
    AuthCredentials, ClerkHttpClient, ClerkPlayer, ClerkSource, CredentialStore,
    MemoryCredentialStore, ReqwestClerkHttpClient, StoredAccount,
};
use crate::data::{
    load_mobile_base_location, CatalogCache, DataHttpClient, MapCache, MobileBaseLocation,
    ReqwestDataHttpClient,
};
use crate::errors::{
    retry_after_ms_from_close, ClientError, ConnectionClosedError, SpacemoltError,
    CLOSE_CODE_AUTH_TIMEOUT, CLOSE_CODE_SESSION_REPLACED,
};
use crate::transport::socket::SocketFactory;

/// Client options.
#[derive(Clone)]
pub struct SpacemoltClientOptions {
    /// WebSocket URL of the v2 endpoint.
    pub url: String,
    /// Seed each account's state cache after auth.
    pub seed_state: bool,
    /// Welcome and authentication exchange timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// Delay between connecting accounts.
    pub connect_stagger_ms: u64,
    /// Max accounts per connection-rate-limit batch.
    pub connect_batch_size: usize,
    /// Pause between connection batches.
    pub connect_batch_wait_ms: u64,
    /// Retry failed initial connect/auth attempts with exponential backoff.
    pub connect_retry: Option<ReconnectOptions>,
    /// HTTP origin for bulk data fetches.
    pub http_base_url: String,
    /// HTTP client used by readonly data caches.
    pub data_http_client: Option<Arc<dyn DataHttpClient>>,
    /// Clerk API key used by owned-account helpers.
    pub clerk_api_key: Option<String>,
    /// HTTP client used by Clerk helpers.
    pub clerk_http_client: Option<Arc<dyn ClerkHttpClient>>,
    /// Injectable socket factory.
    pub socket_factory: Option<Arc<dyn SocketFactory>>,
}

impl Default for SpacemoltClientOptions {
    fn default() -> Self {
        Self {
            url: "wss://game.spacemolt.com/ws/v2".to_string(),
            seed_state: true,
            connect_timeout_ms: 15_000,
            connect_stagger_ms: 600,
            connect_batch_size: 100,
            connect_batch_wait_ms: 65_000,
            connect_retry: Some(DEFAULT_CONNECT_RETRY),
            http_base_url: "https://game.spacemolt.com".to_string(),
            data_http_client: None,
            clerk_api_key: None,
            clerk_http_client: None,
            socket_factory: None,
        }
    }
}

impl SpacemoltClientOptions {
    /// Configure both transports from one HTTP(S) SpaceMolt origin.
    pub fn from_origin(origin: impl AsRef<str>) -> Self {
        let http_base_url = origin.as_ref().trim().trim_end_matches('/').to_string();
        let url = if let Some(rest) = http_base_url.strip_prefix("https://") {
            format!("wss://{rest}/ws/v2")
        } else if let Some(rest) = http_base_url.strip_prefix("http://") {
            format!("ws://{rest}/ws/v2")
        } else {
            format!("{http_base_url}/ws/v2")
        };
        Self {
            url,
            http_base_url,
            ..Self::default()
        }
    }
}

const DEFAULT_CONNECT_RETRY: ReconnectOptions = ReconnectOptions {
    max_retries: Some(8),
    base_delay_ms: 2_000,
    max_delay_ms: 60_000,
};

type AccountHandler = Arc<dyn Fn(Account) + Send + Sync>;
type AccountDisconnectedHandler = Arc<dyn Fn(String, ConnectionClosedError) + Send + Sync>;

/// Options for connecting a fleet of stored accounts.
#[derive(Clone, Default)]
pub struct ConnectAllOptions {
    on_connect: Option<AccountHandler>,
}

impl ConnectAllOptions {
    /// Fire as each account finishes connecting.
    pub fn new<F>(on_connect: F) -> Self
    where
        F: Fn(Account) + Send + Sync + 'static,
    {
        Self {
            on_connect: Some(Arc::new(on_connect)),
        }
    }
}

/// Options for connecting Clerk-owned accounts.
#[derive(Clone, Default)]
pub struct ConnectOwnedOptions {
    filter: Option<Arc<dyn Fn(&ClerkPlayer) -> bool + Send + Sync>>,
    on_connect: Option<AccountHandler>,
}

impl ConnectOwnedOptions {
    /// Connect only players accepted by `filter`.
    pub fn new<F>(filter: F) -> Self
    where
        F: Fn(&ClerkPlayer) -> bool + Send + Sync + 'static,
    {
        Self {
            filter: Some(Arc::new(filter)),
            on_connect: None,
        }
    }

    /// Fire as each owned account finishes connecting.
    pub fn on_connect<F>(mut self, on_connect: F) -> Self
    where
        F: Fn(Account) + Send + Sync + 'static,
    {
        self.on_connect = Some(Arc::new(on_connect));
        self
    }
}

/// Multi-account SpaceMolt client.
pub struct SpacemoltClient<S = MemoryCredentialStore> {
    opts: SpacemoltClientOptions,
    store: Mutex<S>,
    connected: Arc<Mutex<HashMap<String, Account>>>,
    account_connected_listeners: Arc<Mutex<Vec<AccountHandler>>>,
    account_reconnected_listeners: Arc<Mutex<Vec<AccountHandler>>>,
    account_disconnected_listeners: Arc<Mutex<Vec<AccountDisconnectedHandler>>>,
    reconnecting: Arc<Mutex<HashSet<String>>>,
    reconnect_limiter: Arc<tokio::sync::Mutex<ReconnectLimiter>>,
    catalog_cache: tokio::sync::Mutex<Option<Arc<CatalogCache>>>,
    map_cache: tokio::sync::Mutex<Option<Arc<MapCache>>>,
}

impl Default for SpacemoltClient<MemoryCredentialStore> {
    fn default() -> Self {
        Self::new(
            SpacemoltClientOptions::default(),
            MemoryCredentialStore::default(),
        )
    }
}

impl<S> SpacemoltClient<S>
where
    S: CredentialStore,
{
    /// Create a client from options and a credential store.
    pub fn new(opts: SpacemoltClientOptions, store: S) -> Self {
        Self {
            opts,
            store: Mutex::new(store),
            connected: Arc::new(Mutex::new(HashMap::new())),
            account_connected_listeners: Arc::new(Mutex::new(Vec::new())),
            account_reconnected_listeners: Arc::new(Mutex::new(Vec::new())),
            account_disconnected_listeners: Arc::new(Mutex::new(Vec::new())),
            reconnecting: Arc::new(Mutex::new(HashSet::new())),
            reconnect_limiter: Arc::new(tokio::sync::Mutex::new(ReconnectLimiter::default())),
            catalog_cache: tokio::sync::Mutex::new(None),
            map_cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Persist username/password credentials.
    pub fn add_login(&self, username: impl Into<String>, password: impl Into<String>) -> String {
        let username = username.into();
        self.store
            .lock()
            .expect("credential store")
            .put(StoredAccount {
                id: username.clone(),
                credentials: AuthCredentials::Login {
                    username: username.clone(),
                    password: password.into(),
                },
                player_id: None,
            })
            .expect("persist login credentials");
        username
    }

    /// Persist a login-token credential under an explicit id.
    pub fn add_token(&self, id: impl Into<String>, token: impl Into<String>) -> String {
        let id = id.into();
        self.store
            .lock()
            .expect("credential store")
            .put(StoredAccount {
                id: id.clone(),
                credentials: AuthCredentials::LoginToken {
                    token: token.into(),
                },
                player_id: None,
            })
            .expect("persist token credentials");
        id
    }

    /// Register, persist, and begin managing a new account.
    pub async fn register(
        &self,
        params: RegisterParams,
    ) -> Result<(Account, RegisterResult), ClientError> {
        if params.username.trim().is_empty()
            || params.empire.trim().is_empty()
            || params.registration_code.trim().is_empty()
        {
            return Err(ClientError::Server(SpacemoltError::new(
                "invalid_registration",
                "username, empire, and registration code must not be blank",
            )));
        }
        let id = params.username.clone();
        let stored = StoredAccount {
            id: id.clone(),
            credentials: AuthCredentials::Login {
                username: id.clone(),
                password: String::new(),
            },
            player_id: None,
        };
        let account = self.create_account(&stored);
        self.connected
            .lock()
            .expect("connected")
            .insert(id.clone(), account.clone());
        let result = async {
            account.connect().await?;
            account.wait_for_welcome().await?;
            account.register(params).await
        }
        .await;
        match result {
            Ok(mut result) => {
                let persistence = self
                    .store
                    .lock()
                    .expect("credential store")
                    .put(StoredAccount {
                        id: id.clone(),
                        credentials: AuthCredentials::Login {
                            username: id.clone(),
                            password: result.password.clone(),
                        },
                        player_id: Some(result.player_id.clone()),
                    })
                    .map_err(|err| ClientError::PostRegistration {
                        username: id.clone(),
                        password: result.password.clone(),
                        player_id: result.player_id.clone(),
                        message: format!("credentials could not be persisted: {err}"),
                    });
                if let Err(err) = persistence {
                    self.connected.lock().expect("connected").remove(&id);
                    account.close();
                    return Err(err);
                }

                let ready = account_state_ready(&account);
                if ready.is_err() {
                    if let Err(err) = account.refresh().await {
                        self.connected.lock().expect("connected").remove(&id);
                        account.close();
                        return Err(ClientError::PostRegistration {
                            username: id.clone(),
                            password: result.password.clone(),
                            player_id: result.player_id.clone(),
                            message: format!("initial account state could not be hydrated: {err}"),
                        });
                    }
                }
                if let Err(message) = account_state_ready(&account) {
                    self.connected.lock().expect("connected").remove(&id);
                    account.close();
                    return Err(ClientError::PostRegistration {
                        username: id.clone(),
                        password: result.password.clone(),
                        player_id: result.player_id.clone(),
                        message,
                    });
                }
                result.state = account.state_snapshot();
                self.wire_account_disconnect(id, account.clone());
                self.notify_account_connected(account.clone());
                Ok((account, result))
            }
            Err(err) => {
                self.connected.lock().expect("connected").remove(&id);
                account.close();
                Err(err)
            }
        }
    }

    /// Connect and authenticate one stored account. Idempotent per id.
    pub async fn connect(&self, id: &str) -> Result<Account, ClientError> {
        if let Some(existing) = self.connected.lock().expect("connected").get(id) {
            return Ok(existing.clone());
        }
        let stored = self
            .store
            .lock()
            .expect("credential store")
            .get(id)
            .cloned()
            .ok_or_else(|| missing_stored_credentials(id))?;
        let retry = self.opts.connect_retry;
        let max_retries = retry
            .map(|retry| retry.max_retries.unwrap_or(u64::MAX))
            .unwrap_or(0);
        let mut failures = 0_u64;

        loop {
            let account = self.create_account(&stored);
            self.connected
                .lock()
                .expect("connected")
                .insert(id.to_string(), account.clone());

            let result = async {
                account.connect().await?;
                account.wait_for_welcome().await?;
                account.authenticate(stored.credentials.clone()).await?;
                Ok::<(), ClientError>(())
            }
            .await;

            match result {
                Ok(()) => {
                    self.capture_player_id(&stored);
                    self.wire_account_disconnect(id.to_string(), account.clone());
                    self.notify_account_connected(account.clone());
                    return Ok(account);
                }
                Err(err) => {
                    self.connected.lock().expect("connected").remove(id);
                    account.close();
                    if failures >= max_retries {
                        return Err(err);
                    }
                    if let Some(retry) = retry {
                        tokio::time::sleep(Duration::from_millis(connect_retry_delay(
                            retry, failures, &err,
                        )))
                        .await;
                    }
                    failures = failures.saturating_add(1);
                }
            }
        }
    }

    /// Connect every stored account, paced by the configured fleet limits.
    pub async fn connect_all(&self, opts: ConnectAllOptions) -> Vec<Account> {
        let ids = self
            .store
            .lock()
            .expect("credential store")
            .list()
            .into_iter()
            .map(|stored| stored.id.clone())
            .collect::<Vec<_>>();
        self.connect_ids(ids, opts.on_connect).await
    }

    /// List player accounts owned by the configured Clerk API key.
    pub async fn list_owned_players(&self) -> Result<Vec<ClerkPlayer>, ClientError> {
        self.clerk_source()?
            .list_players()
            .await
            .map_err(clerk_error)
    }

    /// Connect selected accounts owned by the configured Clerk user.
    pub async fn connect_owned(
        &self,
        opts: ConnectOwnedOptions,
    ) -> Result<Vec<Account>, ClientError> {
        let source = self.clerk_source()?;
        let players = source.list_players().await.map_err(clerk_error)?;
        let mut ids = Vec::new();
        for player in players {
            if opts
                .filter
                .as_ref()
                .map(|filter| filter(&player))
                .unwrap_or(true)
            {
                self.store
                    .lock()
                    .expect("credential store")
                    .put(StoredAccount {
                        id: player.username.clone(),
                        credentials: AuthCredentials::Clerk {
                            player_id: player.id.clone(),
                            api_key: source.api_key().to_string(),
                            http_base_url: source.http_base_url().to_string(),
                        },
                        player_id: Some(player.id),
                    })
                    .map_err(|err| ClientError::CredentialStore(err.to_string()))?;
                ids.push(player.username);
            }
        }
        Ok(self.connect_ids(ids, opts.on_connect).await)
    }

    /// Fetch and cache the bulk catalog. Pass `force` to refetch.
    pub async fn catalog(&self, force: bool) -> Result<Arc<CatalogCache>, ClientError> {
        let mut cache = self.catalog_cache.lock().await;
        if force || cache.is_none() {
            let http = self.data_http_client();
            *cache = Some(Arc::new(
                CatalogCache::load(&self.opts.http_base_url, http.as_ref())
                    .await
                    .map_err(data_error)?,
            ));
        }
        Ok(Arc::clone(
            cache.as_ref().expect("catalog just initialized"),
        ))
    }

    /// Fetch and cache the static galaxy map. Pass `force` to refetch.
    pub async fn map(&self, force: bool) -> Result<Arc<MapCache>, ClientError> {
        let mut cache = self.map_cache.lock().await;
        if force || cache.is_none() {
            let http = self.data_http_client();
            *cache = Some(Arc::new(
                MapCache::load(&self.opts.http_base_url, http.as_ref())
                    .await
                    .map_err(data_error)?,
            ));
        }
        Ok(Arc::clone(cache.as_ref().expect("map just initialized")))
    }

    /// Fetch SpaceMolt's current mobile-base location.
    pub async fn mobile_base_location(&self) -> Result<MobileBaseLocation, ClientError> {
        let http = self.data_http_client();
        load_mobile_base_location(&self.opts.http_base_url, http.as_ref())
            .await
            .map_err(data_error)
    }

    async fn connect_ids(
        &self,
        ids: Vec<String>,
        on_connect: Option<AccountHandler>,
    ) -> Vec<Account> {
        let mut accounts = Vec::new();
        for id in ids {
            wait_reconnect_slot(Arc::clone(&self.reconnect_limiter), &self.opts).await;
            if let Ok(account) = self.connect(&id).await {
                if let Some(on_connect) = on_connect.as_ref() {
                    on_connect(account.clone());
                }
                accounts.push(account);
            }
        }
        if self.reconnecting.lock().expect("reconnecting").is_empty() {
            self.reconnect_limiter.lock().await.next_index = 0;
        }
        accounts
    }

    /// Subscribe to first-connect notifications for managed accounts.
    pub fn on_account_connected<F>(&mut self, handler: F)
    where
        F: Fn(Account) + Send + Sync + 'static,
    {
        self.account_connected_listeners
            .lock()
            .expect("listeners")
            .push(Arc::new(handler));
    }

    /// Subscribe to successful reconnect notifications for managed accounts.
    pub fn on_account_reconnected<F>(&mut self, handler: F)
    where
        F: Fn(Account) + Send + Sync + 'static,
    {
        self.account_reconnected_listeners
            .lock()
            .expect("listeners")
            .push(Arc::new(handler));
    }

    /// Subscribe to terminal disconnect notifications for managed accounts.
    pub fn on_account_disconnected<F>(&mut self, handler: F)
    where
        F: Fn(String, ConnectionClosedError) + Send + Sync + 'static,
    {
        self.account_disconnected_listeners
            .lock()
            .expect("listeners")
            .push(Arc::new(handler));
    }

    /// Get a connected account by id.
    pub fn account(&self, id: &str) -> Option<Account> {
        self.connected.lock().expect("connected").get(id).cloned()
    }

    /// All currently connected accounts.
    pub fn accounts(&self) -> Vec<Account> {
        let connected = self.connected.lock().expect("connected");
        self.store
            .lock()
            .expect("credential store")
            .list()
            .into_iter()
            .filter_map(|stored| connected.get(&stored.id).cloned())
            .collect()
    }

    /// Connected account ids.
    pub fn ids(&self) -> Vec<String> {
        let connected = self.connected.lock().expect("connected");
        self.store
            .lock()
            .expect("credential store")
            .list()
            .into_iter()
            .filter(|stored| connected.contains_key(&stored.id))
            .map(|stored| stored.id.clone())
            .collect()
    }

    /// Close and forget an account, removing stored credentials.
    pub fn remove(&self, id: &str) {
        let account = self.connected.lock().expect("connected").remove(id);
        if let Some(account) = account {
            account.close();
        }
        self.store.lock().expect("credential store").remove(id);
    }

    /// Close every managed connection while retaining stored credentials.
    pub fn close_all(&self) {
        let accounts = self
            .connected
            .lock()
            .expect("connected")
            .drain()
            .map(|(_, account)| account)
            .collect::<Vec<_>>();
        for account in accounts {
            account.close();
        }
    }

    /// Create and track one stored account handle without opening its socket.
    pub fn connect_shell(&self, id: &str) -> Option<Account> {
        let stored = self
            .store
            .lock()
            .expect("credential store")
            .get(id)
            .cloned()?;
        let account = Account::new(AccountOptions {
            url: self.opts.url.clone(),
            seed_state: self.opts.seed_state,
            connect_timeout_ms: self.opts.connect_timeout_ms,
            id: Some(stored.id.clone()),
            credentials: Some(stored.credentials.clone()),
            clerk_http_client: self.opts.clerk_http_client.clone(),
            ..AccountOptions::default()
        });
        self.connected
            .lock()
            .expect("connected")
            .insert(stored.id.clone(), account.clone());
        Some(account)
    }

    /// Credential store backing this client.
    pub fn credential_store(&self) -> std::sync::MutexGuard<'_, S> {
        self.store.lock().expect("credential store")
    }

    fn create_account(&self, stored: &StoredAccount) -> Account {
        let opts = AccountOptions {
            url: self.opts.url.clone(),
            seed_state: self.opts.seed_state,
            connect_timeout_ms: self.opts.connect_timeout_ms,
            id: Some(stored.id.clone()),
            credentials: Some(stored.credentials.clone()),
            clerk_http_client: self.opts.clerk_http_client.clone(),
            ..AccountOptions::default()
        };
        if let Some(factory) = self.opts.socket_factory.as_ref() {
            Account::with_socket_factory(opts, Arc::clone(factory))
        } else {
            Account::new(opts)
        }
    }

    fn capture_player_id(&self, stored: &StoredAccount) {
        let Some(account) = self
            .connected
            .lock()
            .expect("connected")
            .get(&stored.id)
            .cloned()
        else {
            return;
        };
        let player_id = account
            .state_snapshot()
            .get("player")
            .and_then(|player| player.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        if player_id.is_some() && player_id != stored.player_id {
            let _ = self
                .store
                .lock()
                .expect("credential store")
                .put(StoredAccount {
                    id: stored.id.clone(),
                    credentials: stored.credentials.clone(),
                    player_id,
                });
        }
    }

    fn notify_account_connected(&self, account: Account) {
        let listeners = self
            .account_connected_listeners
            .lock()
            .expect("listeners")
            .clone();
        for listener in listeners {
            let account = account.clone();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(account)));
        }
    }

    fn clerk_source(&self) -> Result<ClerkSource, ClientError> {
        let Some(api_key) = self.opts.clerk_api_key.clone() else {
            return Err(ClientError::Server(SpacemoltError::new(
                "missing_clerk_api_key",
                "connect_owned/list_owned_players require clerk_api_key in SpacemoltClientOptions",
            )));
        };
        Ok(ClerkSource::with_http_client(
            api_key,
            self.opts.http_base_url.clone(),
            self.opts
                .clerk_http_client
                .clone()
                .unwrap_or_else(|| Arc::new(ReqwestClerkHttpClient::default())),
        ))
    }

    fn data_http_client(&self) -> Arc<dyn DataHttpClient> {
        self.opts
            .data_http_client
            .clone()
            .unwrap_or_else(|| Arc::new(ReqwestDataHttpClient::default()))
    }

    fn wire_account_disconnect(&self, id: String, account: Account) {
        let connected = Arc::clone(&self.connected);
        let reconnecting = Arc::clone(&self.reconnecting);
        let limiter = Arc::clone(&self.reconnect_limiter);
        let opts = self.opts.clone();
        let reconnected_listeners = Arc::clone(&self.account_reconnected_listeners);
        let disconnected_listeners = Arc::clone(&self.account_disconnected_listeners);
        account.clone().on_disconnected(move |err| {
            let current_matches = connected
                .lock()
                .expect("connected")
                .get(&id)
                .map(|current| current.same_handle(&account))
                .unwrap_or(false);
            if !current_matches {
                return;
            }

            if is_terminal_close(&err) {
                connected.lock().expect("connected").remove(&id);
                notify_account_disconnected(&disconnected_listeners, id.clone(), err);
                return;
            }

            {
                let mut reconnecting = reconnecting.lock().expect("reconnecting");
                if !reconnecting.insert(id.clone()) {
                    return;
                }
            }

            let connected = Arc::clone(&connected);
            let reconnecting = Arc::clone(&reconnecting);
            let limiter = Arc::clone(&limiter);
            let opts = opts.clone();
            let account = account.clone();
            let id = id.clone();
            let reconnected_listeners = Arc::clone(&reconnected_listeners);
            let disconnected_listeners = Arc::clone(&disconnected_listeners);
            tokio::spawn(async move {
                wait_reconnect_slot(limiter, &opts).await;
                let result =
                    reconnect_account_with_retry(account.clone(), opts.connect_retry).await;
                reconnecting.lock().expect("reconnecting").remove(&id);
                match result {
                    Ok(()) => {
                        notify_account_reconnected(&reconnected_listeners, account);
                    }
                    Err(_) => {
                        connected.lock().expect("connected").remove(&id);
                        notify_account_disconnected(&disconnected_listeners, id, err);
                    }
                }
            });
        });
    }
}

fn account_state_ready(account: &Account) -> Result<(), String> {
    let player = account
        .state()
        .player()
        .map_err(|err| format!("initial player state is invalid: {err}"))?
        .ok_or_else(|| "initial account state has no player identity".to_string())?;
    if player.id.as_deref().is_none_or(str::is_empty) {
        return Err("initial account state has no player ID".to_string());
    }
    if player.username.as_deref().is_none_or(str::is_empty) {
        return Err("initial account state has no username".to_string());
    }
    let location = account
        .state()
        .location()
        .map_err(|err| format!("initial location state is invalid: {err}"))?
        .ok_or_else(|| "initial account state has no system location".to_string())?;
    if location.system_id.as_deref().is_none_or(str::is_empty) {
        return Err("initial account state has no system location".to_string());
    }
    if location.poi_id.as_deref().is_none_or(str::is_empty) {
        return Err("initial account state has no POI location".to_string());
    }
    Ok(())
}

async fn reconnect_account_with_retry(
    account: Account,
    retry: Option<ReconnectOptions>,
) -> Result<(), ClientError> {
    let max_retries = retry
        .map(|retry| retry.max_retries.unwrap_or(u64::MAX))
        .unwrap_or(0);
    let mut failures = 0_u64;
    loop {
        match account.reconnect_once().await {
            Ok(()) => return Ok(()),
            Err(err) => {
                if failures >= max_retries {
                    return Err(err);
                }
                if let Some(retry) = retry {
                    tokio::time::sleep(Duration::from_millis(connect_retry_delay(
                        retry, failures, &err,
                    )))
                    .await;
                }
                failures = failures.saturating_add(1);
            }
        }
    }
}

#[derive(Default)]
struct ReconnectLimiter {
    next_index: usize,
}

async fn wait_reconnect_slot(
    limiter: Arc<tokio::sync::Mutex<ReconnectLimiter>>,
    opts: &SpacemoltClientOptions,
) {
    let mut limiter = limiter.lock().await;
    let index = limiter.next_index;
    limiter.next_index = limiter.next_index.saturating_add(1);
    if index == 0 {
        return;
    }
    if opts.connect_batch_size > 0 && index % opts.connect_batch_size == 0 {
        tokio::time::sleep(Duration::from_millis(opts.connect_batch_wait_ms)).await;
    } else if opts.connect_stagger_ms > 0 {
        tokio::time::sleep(Duration::from_millis(opts.connect_stagger_ms)).await;
    }
}

fn connect_retry_delay(config: ReconnectOptions, failure_index: u64, err: &ClientError) -> u64 {
    if let ClientError::ConnectionClosed(err) = err {
        if let Some(retry_after) = retry_after_ms_from_close(err) {
            return retry_after;
        }
    }
    config
        .base_delay_ms
        .saturating_mul(2_u64.saturating_pow(failure_index.min(32) as u32))
        .min(config.max_delay_ms)
}

fn is_terminal_close(err: &ConnectionClosedError) -> bool {
    err.code == Some(CLOSE_CODE_SESSION_REPLACED) || err.code == Some(CLOSE_CODE_AUTH_TIMEOUT)
}

fn notify_account_reconnected(listeners: &Arc<Mutex<Vec<AccountHandler>>>, account: Account) {
    let listeners = listeners.lock().expect("listeners").clone();
    for listener in listeners {
        let account = account.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(account)));
    }
}

fn notify_account_disconnected(
    listeners: &Arc<Mutex<Vec<AccountDisconnectedHandler>>>,
    id: String,
    err: ConnectionClosedError,
) {
    let listeners = listeners.lock().expect("listeners").clone();
    for listener in listeners {
        let id = id.clone();
        let err = err.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(id, err)));
    }
}

fn missing_stored_credentials(id: &str) -> ClientError {
    ClientError::Server(SpacemoltError::new(
        "missing_credentials",
        format!("no stored credentials for account \"{id}\""),
    ))
}

fn clerk_error(message: String) -> ClientError {
    ClientError::Server(SpacemoltError::new("clerk_error", message))
}

fn data_error(message: String) -> ClientError {
    ClientError::Server(SpacemoltError::new("data_error", message))
}
