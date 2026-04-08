//! Lightweight persisted store for MCP-created sessions.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::client::PrayerApiClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSession {
    session_id: String,
    username: String,
    password: String,
    label: Option<String>,
    script: Option<String>,
    halted: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct PersistedStore {
    sessions: Vec<PersistedSession>,
}

#[derive(Debug)]
pub struct SessionStore {
    path: PathBuf,
    inner: Mutex<PersistedStore>,
}

impl SessionStore {
    pub async fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let inner = match fs::read_to_string(&path) {
            Ok(text) if !text.trim().is_empty() => serde_json::from_str(&text).unwrap_or_default(),
            Ok(_) => PersistedStore::default(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => PersistedStore::default(),
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            path,
            inner: Mutex::new(inner),
        })
    }

    pub async fn restore_startup(self: &Arc<Self>, client: &PrayerApiClient) -> anyhow::Result<()> {
        tracing::info!("restore_startup: fetching existing sessions from prayer-api");
        let existing = client
            .list_sessions()
            .await
            .unwrap_or(serde_json::json!([]));
        tracing::info!("restore_startup: list_sessions returned");
        let mut existing_ids = std::collections::HashSet::new();
        if let Some(arr) = existing.as_array() {
            for s in arr {
                if let Some(id) = s.get("id").and_then(serde_json::Value::as_str) {
                    existing_ids.insert(id.to_string());
                }
            }
        }

        let mut changed = false;
        let mut guard = self.inner.lock().await;
        for saved in &mut guard.sessions {
            if existing_ids.contains(&saved.session_id) {
                continue;
            }

            if saved
                .label
                .as_deref()
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
            {
                saved.label = Some(saved.username.clone());
                changed = true;
            }

            let created = client
                .create_session(&saved.username, &saved.password, saved.label.as_deref())
                .await?;
            let Some(new_id) = created
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
            else {
                tracing::warn!("session restore create_session returned no id");
                continue;
            };

            if let Some(script) = &saved.script {
                if let Err(err) = client.load_script(&new_id, script).await {
                    tracing::warn!(session_id = %new_id, "restore load_script failed: {err}");
                }
            }
            if saved.halted {
                if let Err(err) = client
                    .halt_session(&new_id, Some("restored from prayer-mcp session store"))
                    .await
                {
                    tracing::warn!(session_id = %new_id, "restore halt_session failed: {err}");
                }
            }

            saved.session_id = new_id;
            changed = true;
        }
        drop(guard);

        if changed {
            self.save().await?;
        }

        Ok(())
    }

    pub async fn remember_created(
        &self,
        session_id: &str,
        username: &str,
        password: &str,
        label: Option<&str>,
    ) {
        let mut guard = self.inner.lock().await;
        if let Some(existing) = guard
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
        {
            existing.username = username.to_string();
            existing.password = password.to_string();
            existing.label = label.map(ToString::to_string);
        } else {
            guard.sessions.push(PersistedSession {
                session_id: session_id.to_string(),
                username: username.to_string(),
                password: password.to_string(),
                label: label.map(ToString::to_string),
                script: None,
                halted: false,
            });
        }
        drop(guard);
        if let Err(err) = self.save().await {
            tracing::warn!("session store save failed: {err}");
        }
    }

    pub async fn remember_script(&self, session_id: &str, script: &str) {
        let mut guard = self.inner.lock().await;
        if let Some(existing) = guard
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
        {
            existing.script = Some(script.to_string());
            existing.halted = false;
            drop(guard);
            if let Err(err) = self.save().await {
                tracing::warn!("session store save failed: {err}");
            }
        }
    }

    pub async fn remember_halted(&self, session_id: &str, halted: bool) {
        let mut guard = self.inner.lock().await;
        if let Some(existing) = guard
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
        {
            existing.halted = halted;
            drop(guard);
            if let Err(err) = self.save().await {
                tracing::warn!("session store save failed: {err}");
            }
        }
    }

    pub async fn forget_session(&self, session_id: &str) {
        let mut guard = self.inner.lock().await;
        let before = guard.sessions.len();
        guard.sessions.retain(|s| s.session_id != session_id);
        let removed = guard.sessions.len() != before;
        drop(guard);

        if removed {
            if let Err(err) = self.save().await {
                tracing::warn!("session store save failed: {err}");
            }
        }
    }

    async fn save(&self) -> anyhow::Result<()> {
        let guard = self.inner.lock().await;
        let data = serde_json::to_string_pretty(&*guard)?;
        drop(guard);

        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        write_secure_store(&self.path, &data)?;
        Ok(())
    }
}

fn write_secure_store(path: &Path, data: &str) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(data.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(path, data)?;
        Ok(())
    }
}
