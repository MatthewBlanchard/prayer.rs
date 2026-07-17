//! Clerk-backed owned-account discovery and token minting.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Player account owned by a Clerk user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClerkPlayer {
    pub id: String,
    pub username: String,
    pub empire: String,
    #[serde(default)]
    pub hidden: bool,
}

/// HTTP boundary used by ClerkSource.
#[async_trait]
pub trait ClerkHttpClient: Send + Sync {
    /// Send one JSON request with Clerk bearer credentials.
    async fn request_json(
        &self,
        method: &str,
        url: &str,
        api_key: &str,
        body: Option<Value>,
    ) -> Result<Value, String>;
}

/// Reqwest-backed Clerk HTTP client.
#[derive(Debug, Clone, Default)]
pub struct ReqwestClerkHttpClient {
    client: reqwest::Client,
}

#[async_trait]
impl ClerkHttpClient for ReqwestClerkHttpClient {
    async fn request_json(
        &self,
        method: &str,
        url: &str,
        api_key: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|err| format!("invalid HTTP method {method}: {err}"))?;
        let mut request = self
            .client
            .request(method.clone(), url)
            .bearer_auth(api_key)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|err| format!("{method} {url} failed: {err}"))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("{method} {url} -> {status}: {text}"));
        }
        response
            .json::<Value>()
            .await
            .map_err(|err| format!("{method} {url} returned invalid JSON: {err}"))
    }
}

/// Source for listing Clerk-owned players and minting WS login tokens.
#[derive(Clone)]
pub struct ClerkSource {
    api_key: String,
    http_base_url: String,
    http: Arc<dyn ClerkHttpClient>,
}

impl ClerkSource {
    /// Create a source using reqwest.
    pub fn new(api_key: impl Into<String>, http_base_url: impl Into<String>) -> Self {
        Self::with_http_client(
            api_key.into(),
            http_base_url.into(),
            Arc::new(ReqwestClerkHttpClient::default()),
        )
    }

    /// Create a source with an injected HTTP client.
    pub fn with_http_client(
        api_key: String,
        http_base_url: String,
        http: Arc<dyn ClerkHttpClient>,
    ) -> Self {
        Self {
            api_key,
            http_base_url: trim_trailing_slash(&http_base_url),
            http,
        }
    }

    /// Clerk API key used by this source.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// HTTP origin without a trailing slash.
    pub fn http_base_url(&self) -> &str {
        &self.http_base_url
    }

    /// List player accounts owned by the authenticated Clerk user.
    pub async fn list_players(&self) -> Result<Vec<ClerkPlayer>, String> {
        let data = self
            .http
            .request_json(
                "GET",
                &format!("{}/api/registration-code", self.http_base_url),
                &self.api_key,
                None,
            )
            .await?;
        let players = data
            .get("players")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        serde_json::from_value::<Vec<ClerkPlayer>>(players)
            .map_err(|err| format!("registration-code response had invalid players: {err}"))
    }

    /// Mint a single-use WebSocket login token for one player.
    pub async fn mint_ws_token(&self, player_id: &str) -> Result<String, String> {
        let data = self
            .http
            .request_json(
                "POST",
                &format!(
                    "{}/api/player/{}/ws-token",
                    self.http_base_url,
                    encode_path_segment(player_id)
                ),
                &self.api_key,
                None,
            )
            .await?;
        data.get("token")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "ws-token response had no token".to_string())
    }
}

/// Mint one WebSocket token using an injected or default HTTP client.
pub async fn mint_ws_token(
    http_base_url: &str,
    api_key: &str,
    player_id: &str,
    http: Arc<dyn ClerkHttpClient>,
) -> Result<String, String> {
    ClerkSource::with_http_client(api_key.to_string(), http_base_url.to_string(), http)
        .mint_ws_token(player_id)
        .await
}

fn trim_trailing_slash(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}

fn encode_path_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
