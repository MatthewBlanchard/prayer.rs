use crate::ApiError;
use serde_json::Value;

pub(crate) fn local_auth_bypass_enabled() -> bool {
    cfg!(test)
        || std::env::var("PRAYER_LOCAL_AUTH_BYPASS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

pub(crate) fn spacemolt_base_url() -> String {
    std::env::var("PRAYER_SPACEMOLT_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "https://game.spacemolt.com".to_string())
}

pub(crate) async fn spacemolt_create_session(base_url: &str) -> Result<String, ApiError> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/session", base_url.trim_end_matches('/'));
    let response =
        client.post(url).send().await.map_err(|e| {
            ApiError::BadRequest(format!("failed to create SpaceMolt session: {}", e))
        })?;
    if !response.status().is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<failed to read body: {e}>"));
        return Err(ApiError::BadRequest(format!(
            "spacemolt session create failed: {}",
            body
        )));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid SpaceMolt session response: {}", e)))?;
    let session_id = value
        .get("session")
        .and_then(|s| s.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::BadRequest("spacemolt session id missing".to_string()))?;
    Ok(session_id.to_string())
}

pub(crate) async fn spacemolt_login(
    base_url: &str,
    session_id: &str,
    username: &str,
    password: &str,
) -> Result<(), ApiError> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/login", base_url.trim_end_matches('/'));
    let response = client
        .post(url)
        .header("X-Session-Id", session_id)
        .json(&serde_json::json!({
            "username": username,
            "password": password
        }))
        .send()
        .await
        .map_err(|e| ApiError::BadRequest(format!("spacemolt login failed: {}", e)))?;
    if !response.status().is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<failed to read body: {e}>"));
        return Err(ApiError::BadRequest(format!(
            "spacemolt login rejected: {}",
            body
        )));
    }
    Ok(())
}

pub(crate) async fn spacemolt_register(
    base_url: &str,
    session_id: &str,
    username: &str,
    empire: &str,
    registration_code: &str,
) -> Result<Option<String>, ApiError> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/register", base_url.trim_end_matches('/'));
    let response = client
        .post(url)
        .header("X-Session-Id", session_id)
        .json(&serde_json::json!({
            "username": username,
            "empire": empire,
            "registration_code": registration_code
        }))
        .send()
        .await
        .map_err(|e| ApiError::BadRequest(format!("spacemolt register failed: {}", e)))?;
    if !response.status().is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<failed to read body: {e}>"));
        return Err(ApiError::BadRequest(format!(
            "spacemolt register rejected: {}",
            body
        )));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid register response: {}", e)))?;
    let password = value
        .get("result")
        .and_then(|r| r.get("password"))
        .or_else(|| value.get("password"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Ok(password)
}
