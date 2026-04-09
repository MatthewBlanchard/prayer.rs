//! Prayer HTTP API for runtime/session control.

use axum::http::StatusCode;
use prayer_runtime::engine::EngineError;
use prayer_runtime::transport::TransportError;
use thiserror::Error;

mod contracts;
mod routes;
mod service;
mod spacemolt_auth;
mod state_mapping;

pub use contracts::*;
pub use routes::build_router;
pub use service::RuntimeService;

/// App-level API errors.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Session was not found.
    #[error("session not found")]
    SessionNotFound,
    /// Session id was invalid.
    #[error("invalid session id")]
    InvalidSessionId,
    /// Engine failure.
    #[error("engine error: {0}")]
    Engine(#[from] EngineError),
    /// Transport failure.
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    /// Invalid client request.
    #[error("{0}")]
    BadRequest(String),
    /// Runtime state is missing required fields.
    #[error("invalid runtime state: {0}")]
    InvalidRuntimeState(String),
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::SessionNotFound => StatusCode::NOT_FOUND,
            Self::InvalidSessionId => StatusCode::BAD_REQUEST,
            Self::Engine(_) => StatusCode::BAD_REQUEST,
            Self::Transport(_) => StatusCode::BAD_GATEWAY,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::InvalidRuntimeState(_) => StatusCode::BAD_GATEWAY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Method, Request};
    use prayer_runtime::engine::GameState;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_and_get_session_summary() {
        let service = Arc::new(RuntimeService::new());
        let app = build_router(service);

        let payload = serde_json::to_vec(&CreateSessionRequest {
            username: "bot".to_string(),
            password: "pw".to_string(),
            label: Some("test".to_string()),
        })
        .expect("payload");
        let create = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/runtime/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(create.status(), StatusCode::CREATED);

        let list = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/runtime/sessions")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(list.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn state_contract_matches_csharp_shape() {
        let service = Arc::new(RuntimeService::new());
        let app = build_router(service.clone());

        let payload = serde_json::to_vec(&CreateSessionRequest {
            username: "bot".to_string(),
            password: "pw".to_string(),
            label: Some("contract-test".to_string()),
        })
        .expect("payload");
        let create = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/runtime/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(create.status(), StatusCode::CREATED);
        let body = to_bytes(create.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let created: Value = serde_json::from_slice(&body).expect("session summary json");
        let session_id = created
            .get("id")
            .and_then(Value::as_str)
            .expect("session id present")
            .to_string();
        let uid = RuntimeService::parse_id(&session_id).expect("valid session id");

        let state = GameState {
            system: Some("sol".to_string()),
            home_base: Some("earth_station".to_string()),
            nearest_station: Some("earth_station".to_string()),
            credits: 42,
            fuel_pct: 87,
            cargo_pct: 33,
            cargo: Arc::new(HashMap::from([("ore".to_string(), 5)])),
            stash: Arc::new(HashMap::from([(
                "earth_station".to_string(),
                HashMap::from([("ore".to_string(), 2)]),
            )])),
            mission_complete: Arc::new(HashMap::new()),
            last_mined: Arc::new(HashMap::new()),
            last_stashed: Arc::new(HashMap::new()),
            script_mined_by_item: Arc::new(HashMap::new()),
            script_stashed_by_item: Arc::new(HashMap::new()),
            ..GameState::default()
        };
        service
            .set_transport(
                uid,
                SetTransportRequest::Mock {
                    state: Some(Box::new(state)),
                    responses: None,
                },
            )
            .await
            .expect("set transport");
        service.refresh_state(uid).await.expect("refresh state");

        let state_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!(
                        "/api/runtime/sessions/{}/state?since=0&wait_ms=0",
                        session_id
                    ))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(state_response.status(), StatusCode::OK);
        let body = to_bytes(state_response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let json: Value = serde_json::from_slice(&body).expect("json");

        assert!(json.get("state").is_some());
        let state = json
            .get("state")
            .and_then(Value::as_object)
            .expect("state object");
        assert!(state.contains_key("currentPoi"));
        assert!(state.contains_key("galaxy"));
        assert!(state.contains_key("ship"));
        assert!(state.contains_key("station"));

        let galaxy = state
            .get("galaxy")
            .and_then(Value::as_object)
            .expect("galaxy object");
        assert!(galaxy.contains_key("map"));
        assert!(galaxy.contains_key("market"));
        assert!(galaxy.contains_key("catalog"));
        assert!(galaxy.contains_key("resources"));
        assert!(galaxy.contains_key("exploration"));
    }
}
