//! Error types surfaced by the SpaceMolt client.

use std::time::Duration;

use serde_json::Value;
use thiserror::Error;

use crate::protocol::{ActionErrorFrame, ErrorFrame};

/// A server-reported error.
#[derive(Debug, Clone, PartialEq, Error)]
#[error("{message}")]
pub struct SpacemoltError {
    /// Machine-readable server error code.
    pub code: String,
    /// Human-readable server error message.
    pub message: String,
    /// Structured details, when present.
    pub details: Option<Value>,
    /// Correlation id, when present.
    pub request_id: Option<String>,
    /// Mutation command name for action-error outcomes.
    pub command: Option<String>,
    /// Game tick for action-error outcomes.
    pub tick: Option<u64>,
    /// On `action_pending` errors: the mutation already queued.
    pub pending_command: Option<String>,
}

impl SpacemoltError {
    /// Build a server error.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            request_id: None,
            command: None,
            tick: None,
            pending_command: None,
        }
    }

    /// Server-provided delay before retrying this operation, when present.
    pub fn retry_after(&self) -> Option<Duration> {
        let seconds = self
            .details
            .as_ref()
            .and_then(|details| details.get("retry_after"))
            .and_then(|value| {
                value
                    .as_f64()
                    .or_else(|| value.as_str().and_then(|value| value.parse::<f64>().ok()))
            })
            .or_else(|| retry_after_seconds_from_message(&self.message))?;
        let millis = (seconds.max(0.0) * 1_000.0).round() as u64;
        Some(Duration::from_millis(millis))
    }
}

fn retry_after_seconds_from_message(message: &str) -> Option<f64> {
    let lower = message.to_ascii_lowercase();
    ["retry in ", "try again in ", "retry after "]
        .into_iter()
        .find_map(|marker| {
            let (_, rest) = lower.split_once(marker)?;
            let number = rest
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
                .collect::<String>();
            number.parse::<f64>().ok()
        })
}

/// Convert a generic server error frame.
pub fn error_from_frame(frame: &ErrorFrame) -> SpacemoltError {
    SpacemoltError {
        code: frame.payload.code.clone(),
        message: frame.payload.message.clone(),
        details: frame.payload.details.clone(),
        request_id: frame.request_id.clone(),
        command: None,
        tick: None,
        pending_command: frame.payload.pending_command.clone(),
    }
}

/// Convert a queued mutation error frame.
pub fn error_from_action_frame(frame: &ActionErrorFrame) -> SpacemoltError {
    SpacemoltError {
        code: frame.payload.code.clone(),
        message: frame.payload.message.clone(),
        details: frame.payload.details.clone(),
        request_id: frame.request_id.clone(),
        command: Some(frame.payload.command.clone()),
        tick: Some(frame.payload.tick),
        pending_command: None,
    }
}

/// Raised against every in-flight request when the socket closes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct ConnectionClosedError {
    /// Human-readable close message.
    pub message: String,
    /// WebSocket close code.
    pub code: Option<u16>,
    /// WebSocket close reason.
    pub reason: Option<String>,
}

impl ConnectionClosedError {
    /// Build a connection close error.
    pub fn new(message: impl Into<String>, code: Option<u16>, reason: Option<String>) -> Self {
        Self {
            message: message.into(),
            code,
            reason,
        }
    }
}

/// Custom server close code: another connection took this player's session.
pub const CLOSE_CODE_SESSION_REPLACED: u16 = 4001;
/// Custom server close code: socket authenticated too slowly.
pub const CLOSE_CODE_AUTH_TIMEOUT: u16 = 4002;
/// Custom server close code: per-IP connection rate limit.
pub const CLOSE_CODE_CONNECTION_RATE_LIMITED: u16 = 4003;

/// Parse `retry_after=<seconds>` from a connection-rate-limited close reason.
pub fn retry_after_ms_from_close(err: &ConnectionClosedError) -> Option<u64> {
    if err.code != Some(CLOSE_CODE_CONNECTION_RATE_LIMITED) {
        return None;
    }
    let reason = err.reason.as_deref()?;
    let (_, rest) = reason.split_once("retry_after=")?;
    let seconds = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .ok()?;
    Some(seconds * 1_000)
}

/// Top-level client error.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Server-reported error frame.
    #[error(transparent)]
    Server(#[from] SpacemoltError),
    /// Socket closed while work was in flight.
    #[error(transparent)]
    ConnectionClosed(#[from] ConnectionClosedError),
    /// Client-side operation timed out.
    #[error("timeout: {0}")]
    Timeout(String),
    /// Credential storage failed outside a completed registration exchange.
    #[error("credential store: {0}")]
    CredentialStore(String),
    /// Registration completed remotely, with credentials available for recovery,
    /// but the account could not be made safe to return to callers.
    #[error("registration succeeded for {username}, but account setup failed: {message}")]
    PostRegistration {
        username: String,
        password: String,
        player_id: String,
        message: String,
    },
    /// Requested action does not exist in the generated catalog.
    #[error("unknown action: {0}")]
    UnknownAction(String),
    /// Feature is not ported yet.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_retry_after_from_connection_rate_limit_close() {
        let err = ConnectionClosedError::new(
            "closed",
            Some(CLOSE_CODE_CONNECTION_RATE_LIMITED),
            Some("connection_rate_limited retry_after=30".to_string()),
        );
        assert_eq!(retry_after_ms_from_close(&err), Some(30_000));
    }

    #[test]
    fn ignores_retry_after_for_other_close_codes() {
        let err = ConnectionClosedError::new(
            "closed",
            Some(CLOSE_CODE_AUTH_TIMEOUT),
            Some("retry_after=30".to_string()),
        );
        assert_eq!(retry_after_ms_from_close(&err), None);
    }

    #[test]
    fn reads_server_retry_after_without_rendering_the_error() {
        let mut err = SpacemoltError::new("rate_limited", "Too many requests");
        err.details = Some(serde_json::json!({ "retry_after": 2.5 }));

        assert_eq!(err.retry_after(), Some(Duration::from_millis(2_500)));
    }

    #[test]
    fn reads_server_retry_after_from_original_message_field() {
        let err = SpacemoltError::new("rate_limited", "Try again in 1.25 seconds");

        assert_eq!(err.retry_after(), Some(Duration::from_millis(1_250)));
    }
}
