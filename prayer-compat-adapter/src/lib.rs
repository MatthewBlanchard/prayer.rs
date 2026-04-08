//! Compatibility adapter routes (`/compat/v1`) that map legacy payloads to native runtime API.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use prayer_api::{ErrorBody, RuntimeService};
use prayer_runtime::engine::{EngineCheckpoint, RuntimeSnapshot};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Legacy command payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LegacyCommandRequest {
    /// Action name.
    pub action: String,
    /// Args array.
    pub args: Vec<String>,
    /// Optional source line.
    pub source_line: Option<usize>,
}

/// Legacy execution request.
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyExecuteRequest {
    /// Session id.
    pub session_id: Uuid,
    /// Command payload.
    pub command: LegacyCommandRequest,
    /// Optional result message.
    pub result_message: Option<String>,
    /// Halt request flag.
    pub halt_script: Option<bool>,
}

/// Legacy status payload.
#[derive(Debug, Clone, Serialize)]
pub struct LegacyStatusResponse {
    /// Script line.
    pub current_script_line: Option<usize>,
    /// Halt status.
    pub halted: bool,
    /// Compatibility message.
    pub status: String,
}

/// Build compatibility router mounted under `/compat/v1`.
pub fn build_router(service: Arc<RuntimeService>) -> Router {
    Router::new()
        .route("/compat/v1/session", post(create_session))
        .route("/compat/v1/session/:id/script", post(set_script))
        .route("/compat/v1/session/:id/next", post(next_command))
        .route(
            "/compat/v1/session/:id/execute",
            post(submit_command_result),
        )
        .route("/compat/v1/session/:id/status", get(status))
        .route("/compat/v1/session/:id/checkpoint", get(checkpoint))
        .route(
            "/compat/v1/session/:id/checkpoint",
            post(restore_checkpoint),
        )
        .route("/compat/v1/session/:id/halt", post(halt))
        .route("/compat/v1/session/:id/resume", post(resume))
        .with_state(service)
}

async fn create_session(State(service): State<Arc<RuntimeService>>) -> Json<serde_json::Value> {
    let id = service.create_session();
    Json(serde_json::json!({ "session_id": id }))
}

async fn set_script(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let script = body
        .get("script")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    service
        .set_script(id, script)
        .await
        .map(|normalized| Json(serde_json::json!({ "normalized_script": normalized })))
        .map_err(map_api_error)
}

async fn next_command(
    State(_service): State<Arc<RuntimeService>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<Option<LegacyCommandRequest>>, (StatusCode, Json<ErrorBody>)> {
    Ok(Json(None))
}

async fn submit_command_result(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<Uuid>,
    Json(_body): Json<LegacyExecuteRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    service
        .execute_step(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(map_api_error)
}

async fn status(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<LegacyStatusResponse>, (StatusCode, Json<ErrorBody>)> {
    service
        .snapshot(id)
        .await
        .map(|snapshot| {
            Json(LegacyStatusResponse {
                current_script_line: snapshot.current_script_line,
                halted: snapshot.is_halted,
                status: if snapshot.is_halted {
                    "Halted".to_string()
                } else {
                    "Running".to_string()
                },
            })
        })
        .map_err(map_api_error)
}

async fn checkpoint(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<EngineCheckpoint>, (StatusCode, Json<ErrorBody>)> {
    service
        .checkpoint(id)
        .await
        .map(Json)
        .map_err(map_api_error)
}

async fn restore_checkpoint(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<Uuid>,
    Json(checkpoint): Json<EngineCheckpoint>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    service
        .restore_checkpoint(id, checkpoint)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(map_api_error)
}

async fn halt(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    service
        .halt(id, Some("compat halt".to_string()))
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(map_api_error)
}

async fn resume(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    service
        .resume(id, Some("compat resume".to_string()))
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(map_api_error)
}

/// Utility for mapping snapshot into compatibility shape.
pub fn snapshot_to_compat(snapshot: RuntimeSnapshot) -> LegacyStatusResponse {
    LegacyStatusResponse {
        current_script_line: snapshot.current_script_line,
        halted: snapshot.is_halted,
        status: if snapshot.is_halted {
            "Halted".to_string()
        } else {
            "Running".to_string()
        },
    }
}

fn map_api_error(error: prayer_api::ApiError) -> (StatusCode, Json<ErrorBody>) {
    let status = match error {
        prayer_api::ApiError::SessionNotFound => StatusCode::NOT_FOUND,
        prayer_api::ApiError::InvalidSessionId => StatusCode::BAD_REQUEST,
        prayer_api::ApiError::Engine(_) => StatusCode::BAD_REQUEST,
        prayer_api::ApiError::Transport(_) => StatusCode::BAD_GATEWAY,
        prayer_api::ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(ErrorBody {
            error: error.to_string(),
        }),
    )
}
