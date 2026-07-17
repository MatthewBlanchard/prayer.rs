//! Credential types used by the client.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// Credentials accepted by the SpaceMolt auth flow.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthCredentials {
    /// Username/password login.
    Login { username: String, password: String },
    /// Single-use login token.
    LoginToken { token: String },
    /// Clerk API key, used to mint short-lived WebSocket tokens.
    Clerk {
        #[serde(rename = "playerId")]
        player_id: String,
        #[serde(rename = "apiKey")]
        api_key: String,
        #[serde(rename = "httpBaseUrl")]
        http_base_url: String,
    },
}

/// Stored account entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredAccount {
    /// Stable account id.
    pub id: String,
    /// Auth credentials for this account.
    pub credentials: AuthCredentials,
    /// Player id captured from authenticated state, when known.
    #[serde(rename = "playerId", skip_serializing_if = "Option::is_none")]
    pub player_id: Option<String>,
}

/// Minimal credential store interface.
pub trait CredentialStore: Send {
    /// Persist or replace one account.
    fn put(&mut self, account: StoredAccount) -> io::Result<()>;
    /// Remove one account.
    fn remove(&mut self, id: &str) -> Option<StoredAccount>;
    /// Get one account.
    fn get(&self, id: &str) -> Option<&StoredAccount>;
    /// List all stored accounts.
    fn list(&self) -> Vec<&StoredAccount>;
}

/// In-memory credential store.
#[derive(Debug, Clone, Default)]
pub struct MemoryCredentialStore {
    accounts: HashMap<String, StoredAccount>,
    order: Vec<String>,
}

impl CredentialStore for MemoryCredentialStore {
    fn put(&mut self, account: StoredAccount) -> io::Result<()> {
        if !self.accounts.contains_key(&account.id) {
            self.order.push(account.id.clone());
        }
        self.accounts.insert(account.id.clone(), account);
        Ok(())
    }

    fn remove(&mut self, id: &str) -> Option<StoredAccount> {
        self.order.retain(|existing| existing != id);
        self.accounts.remove(id)
    }

    fn get(&self, id: &str) -> Option<&StoredAccount> {
        self.accounts.get(id)
    }

    fn list(&self) -> Vec<&StoredAccount> {
        self.order
            .iter()
            .filter_map(|id| self.accounts.get(id))
            .collect()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FileShape {
    version: u32,
    accounts: HashMap<String, StoredAccount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    order: Vec<String>,
}

/// File-backed credential store.
///
/// Credentials are stored as plaintext JSON. Point this at a path protected by
/// the caller's environment.
#[derive(Debug, Clone)]
pub struct FileCredentialStore {
    path: PathBuf,
    inner: MemoryCredentialStore,
}

impl FileCredentialStore {
    /// Open a file-backed credential store, creating an empty in-memory cache
    /// when the file does not exist.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let inner = match std::fs::read_to_string(&path) {
            Ok(raw) => memory_store_from_file_shape(
                serde_json::from_str(&raw)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?,
            ),
            Err(err) if err.kind() == io::ErrorKind::NotFound => MemoryCredentialStore::default(),
            Err(err) => return Err(err),
        };
        Ok(Self { path, inner })
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let shape = FileShape {
            version: 1,
            accounts: self.inner.accounts.clone(),
            order: self.inner.order.clone(),
        };
        let text = serde_json::to_string_pretty(&shape)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let tmp = self.path.with_file_name(format!(
            "{}.tmp-{}",
            self.path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("credentials.json"),
            std::process::id()
        ));
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, &self.path)
    }
}

impl CredentialStore for FileCredentialStore {
    fn put(&mut self, account: StoredAccount) -> io::Result<()> {
        let previous = self.inner.clone();
        self.inner.put(account)?;
        if let Err(err) = self.save() {
            self.inner = previous;
            return Err(err);
        }
        Ok(())
    }

    fn remove(&mut self, id: &str) -> Option<StoredAccount> {
        let removed = self.inner.remove(id);
        self.save().expect("persist credential store");
        removed
    }

    fn get(&self, id: &str) -> Option<&StoredAccount> {
        self.inner.get(id)
    }

    fn list(&self) -> Vec<&StoredAccount> {
        self.inner.list()
    }
}

fn memory_store_from_file_shape(shape: FileShape) -> MemoryCredentialStore {
    let mut store = MemoryCredentialStore {
        accounts: shape.accounts,
        order: shape.order,
    };
    store.order.retain(|id| store.accounts.contains_key(id));
    let mut missing = store
        .accounts
        .keys()
        .filter(|id| !store.order.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    missing.sort();
    store.order.extend(missing);
    store
}
