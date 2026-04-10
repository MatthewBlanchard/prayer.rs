//! Thin HTTP client wrapper around `prayer-api` endpoints.

use std::time::Duration;

use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

/// Errors returned by `PrayerApiClient`.
#[derive(Debug, Error)]
pub enum ClientError {
    /// HTTP transport error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// Non-2xx status from prayer-api.
    #[error("prayer-api {status} at {endpoint}: {body}")]
    Api {
        status: u16,
        endpoint: String,
        request_id: Option<String>,
        body: String,
    },
}

/// Full state response including the version header.
#[derive(Debug)]
pub struct StateWithVersion {
    /// `X-Prayer-State-Version` value (0 if absent).
    pub version: u64,
    /// JSON body.
    pub body: Value,
}

/// Thin wrapper around prayer-api HTTP endpoints.
#[derive(Debug, Clone)]
pub struct PrayerApiClient {
    inner: reqwest::Client,
    base_url: String,
}

impl PrayerApiClient {
    /// Create a new client.
    pub fn new(base_url: String, timeout_ms: u64) -> Self {
        let mut builder = reqwest::Client::builder();
        if timeout_ms > 0 {
            builder = builder.timeout(Duration::from_millis(timeout_ms));
        }
        let inner = builder.build().expect("failed to build reqwest client");
        Self { inner, base_url }
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.inner.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp, path).await);
        }
        Ok(resp.json::<T>().await?)
    }

    async fn post_empty_with_endpoint<T: DeserializeOwned>(
        &self,
        path: &str,
        endpoint: &str,
    ) -> Result<T, ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.inner.post(&url).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp, endpoint).await);
        }
        Ok(resp.json::<T>().await?)
    }

    async fn post_body_raw(&self, path: &str, json: &Value) -> Result<Value, ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.inner.post(&url).json(json).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp, path).await);
        }
        Ok(resp.json().await?)
    }

    async fn post_body_raw_with_endpoint(
        &self,
        path: &str,
        json: &Value,
        endpoint: &str,
    ) -> Result<Value, ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.inner.post(&url).json(json).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp, endpoint).await);
        }
        Ok(resp.json().await?)
    }

    async fn delete_with_endpoint(&self, path: &str, endpoint: &str) -> Result<(), ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.inner.delete(&url).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp, endpoint).await);
        }
        Ok(())
    }

    // ── sessions ─────────────────────────────────────────────────────────────

    /// `GET /api/runtime/sessions`
    pub async fn list_sessions(&self) -> Result<Value, ClientError> {
        self.get("/api/runtime/sessions").await
    }

    /// `POST /api/runtime/sessions`
    pub async fn create_session(
        &self,
        username: &str,
        password: &str,
        label: Option<&str>,
    ) -> Result<Value, ClientError> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "label": label,
        });
        self.post_body_raw("/api/runtime/sessions", &body).await
    }

    /// `POST /api/runtime/sessions/register`
    pub async fn register_session(
        &self,
        username: &str,
        empire: &str,
        registration_code: &str,
        label: Option<&str>,
    ) -> Result<Value, ClientError> {
        let body = serde_json::json!({
            "username": username,
            "empire": empire,
            "registrationCode": registration_code,
            "label": label,
        });
        self.post_body_raw_with_endpoint(
            "/api/runtime/sessions/register",
            &body,
            "/api/runtime/sessions/register",
        )
        .await
    }

    /// `DELETE /api/runtime/sessions/:id`
    pub async fn delete_session(&self, session_id: &str) -> Result<(), ClientError> {
        self.delete_with_endpoint(
            &format!("/api/runtime/sessions/{session_id}"),
            "/api/runtime/sessions/:id",
        )
        .await
    }

    /// `POST /api/runtime/sessions/:id/script`
    pub async fn load_script(&self, session_id: &str, script: &str) -> Result<Value, ClientError> {
        let body = serde_json::json!({ "script": script });
        self.post_body_raw_with_endpoint(
            &format!("/api/runtime/sessions/{session_id}/script"),
            &body,
            "/api/runtime/sessions/:id/script",
        )
        .await
    }

    /// `POST /api/runtime/sessions/:id/script/execute`
    pub async fn execute_script(
        &self,
        session_id: &str,
        max_steps: Option<u64>,
    ) -> Result<Value, ClientError> {
        // max_steps is not currently wired in the API but kept for future use
        let _ = max_steps;
        self.post_empty_with_endpoint(
            &format!("/api/runtime/sessions/{session_id}/script/execute"),
            "/api/runtime/sessions/:id/script/execute",
        )
        .await
    }

    /// `POST /api/runtime/sessions/:id/halt`
    pub async fn halt_session(
        &self,
        session_id: &str,
        reason: Option<&str>,
    ) -> Result<Value, ClientError> {
        let body = serde_json::json!({ "reason": reason });
        self.post_body_raw_with_endpoint(
            &format!("/api/runtime/sessions/{session_id}/halt"),
            &body,
            "/api/runtime/sessions/:id/halt",
        )
        .await
    }

    /// `GET /api/runtime/sessions/:id/skills`
    pub async fn get_skills(&self, session_id: &str) -> Result<Value, ClientError> {
        self.get(&format!("/api/runtime/sessions/{session_id}/skills"))
            .await
    }

    /// `POST /api/runtime/sessions/:id/skills`
    pub async fn set_skills(&self, session_id: &str, text: &str) -> Result<Value, ClientError> {
        let body = serde_json::json!({ "text": text });
        self.post_body_raw_with_endpoint(
            &format!("/api/runtime/sessions/{session_id}/skills"),
            &body,
            "/api/runtime/sessions/:id/skills",
        )
        .await
    }

    /// `GET /api/runtime/sessions/:id/state` — returns body + `X-Prayer-State-Version`.
    pub async fn get_state(
        &self,
        session_id: &str,
        since: Option<u64>,
        wait_ms: Option<u64>,
    ) -> Result<StateWithVersion, ClientError> {
        let mut url = format!(
            "{}/api/runtime/sessions/{}/state",
            self.base_url, session_id
        );
        let mut sep = '?';
        if let Some(s) = since {
            url.push_str(&format!("{sep}since={s}"));
            sep = '&';
        }
        if let Some(w) = wait_ms {
            url.push_str(&format!("{sep}wait_ms={w}"));
        }
        let resp = self.inner.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp, "/api/runtime/sessions/:id/state").await);
        }
        let version = parse_state_version(resp.headers());
        let body: Value = resp.json().await?;
        Ok(StateWithVersion { version, body })
    }

    /// `GET /api/runtime/sessions/:id/snapshot`
    /// `POST /api/runtime/sessions/:id/spacemolt/passthrough`
    pub async fn passthrough(
        &self,
        session_id: &str,
        command: &str,
        payload: Option<Value>,
    ) -> Result<Value, ClientError> {
        let payload_len = payload.as_ref().map(|p| p.to_string().len()).unwrap_or(0);
        tracing::info!(
            session_id,
            command,
            payload_bytes = payload_len,
            "passthrough call"
        );
        let body = serde_json::json!({ "command": command, "payload": payload });
        self.post_body_raw_with_endpoint(
            &format!("/api/runtime/sessions/{session_id}/spacemolt/passthrough"),
            &body,
            "/api/runtime/sessions/:id/spacemolt/passthrough",
        )
        .await
    }
}

fn parse_state_version(headers: &HeaderMap) -> u64 {
    headers
        .get("x-prayer-state-version")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

async fn api_error(resp: reqwest::Response, endpoint: &str) -> ClientError {
    let status = resp.status().as_u16();
    let request_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = resp.text().await.unwrap_or_default();
    ClientError::Api {
        status,
        endpoint: endpoint.to_string(),
        request_id,
        body,
    }
}
