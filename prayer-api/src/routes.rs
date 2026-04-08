use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::spacemolt_auth::{
    local_auth_bypass_enabled, spacemolt_base_url, spacemolt_create_session, spacemolt_login,
    spacemolt_register,
};
use crate::state_mapping::map_runtime_state;
use crate::{
    ActiveGoRouteDto, ApiError, CommandAckResponse, CreateSessionRequest, ErrorBody,
    ExecuteScriptResponse, GalaxyPricesResponse, RegisterSessionRequest, RegisterSessionResponse,
    RuntimeGameStateDto, RuntimeHostSnapshotDto, RuntimeService, RuntimeSnapshotResponse,
    SessionSummary, SetScriptRequest, SetSkillLibraryRequest, SetTransportRequest,
    SkillLibraryTextResponse, SpaceMoltPassthroughRequest, SpaceMoltPassthroughResponse,
    StationShipyardResponse, StationStorageResponse,
};

const MAX_WAIT_MS: u64 = 30_000;

#[derive(Debug, Clone, Deserialize)]
struct StateQuery {
    since: Option<u64>,
    wait_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct RouteQuery {
    target: String,
}

/// Build router.
pub fn build_router(service: Arc<RuntimeService>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/api/runtime/sessions",
            get(list_sessions).post(create_session),
        )
        .route("/api/runtime/sessions/register", post(register_session))
        .route(
            "/api/runtime/sessions/:id",
            get(get_session).delete(delete_session),
        )
        .route("/api/runtime/sessions/:id/snapshot", get(snapshot_v2))
        .route("/api/runtime/sessions/:id/status", get(status_lines))
        .route("/api/runtime/sessions/:id/route", get(route))
        .route(
            "/api/runtime/sessions/:id/spacemolt/stats",
            get(spacemolt_stats),
        )
        .route("/api/runtime/sessions/:id/state", get(state_v2))
        .route("/api/runtime/sessions/:id/galaxy/map", get(galaxy_map))
        .route("/api/runtime/sessions/:id/galaxy/pois", get(galaxy_pois))
        .route(
            "/api/runtime/sessions/:id/galaxy/prices",
            get(galaxy_prices),
        )
        .route(
            "/api/runtime/sessions/:id/galaxy/resources",
            get(galaxy_resources),
        )
        .route(
            "/api/runtime/sessions/:id/galaxy/explored",
            get(galaxy_explored),
        )
        .route(
            "/api/runtime/sessions/:id/galaxy/catalog/items",
            get(galaxy_catalog_items),
        )
        .route(
            "/api/runtime/sessions/:id/galaxy/catalog/ships",
            get(galaxy_catalog_ships),
        )
        .route("/api/runtime/sessions/:id/station", get(station))
        .route(
            "/api/runtime/sessions/:id/station/storage",
            get(station_storage),
        )
        .route(
            "/api/runtime/sessions/:id/station/shipyard",
            get(station_shipyard),
        )
        .route(
            "/api/runtime/sessions/:id/station/craftable",
            get(station_craftable),
        )
        .route("/api/runtime/sessions/:id/script", post(set_script_v2))
        .route(
            "/api/runtime/sessions/:id/script/execute",
            post(execute_script_v2),
        )
        .route("/api/runtime/sessions/:id/halt", post(halt_v2))
        .route(
            "/api/runtime/sessions/:id/skills",
            get(get_skills).post(set_skills),
        )
        .route(
            "/api/runtime/sessions/:id/spacemolt/passthrough",
            post(spacemolt_passthrough),
        )
        .with_state(service)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "service": "Prayer", "status": "ok", "utc": Utc::now() }))
}

async fn list_sessions(State(service): State<Arc<RuntimeService>>) -> Json<Vec<SessionSummary>> {
    Json(service.list_sessions().await)
}

async fn create_session(
    State(service): State<Arc<RuntimeService>>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    if body.username.trim().is_empty() || body.password.trim().is_empty() {
        return Err(map_api_error(ApiError::BadRequest(
            "username and password are required".to_string(),
        )));
    }
    let id = service.create_session_with_label(body.label);
    if !local_auth_bypass_enabled() {
        let base_url = spacemolt_base_url();
        let session_id = spacemolt_create_session(&base_url)
            .await
            .map_err(map_api_error)?;
        spacemolt_login(&base_url, &session_id, &body.username, &body.password)
            .await
            .map_err(map_api_error)?;
        service
            .set_transport(
                id,
                SetTransportRequest::SpaceMolt {
                    base_url: base_url.clone(),
                    token: session_id,
                },
            )
            .await
            .map_err(map_api_error)?;
        if let Err(e) = service.refresh_state(id).await {
            tracing::warn!(%id, "initial state refresh failed: {e}");
        }
    }
    let summary = service
        .session_summary(&id.to_string())
        .await
        .map_err(map_api_error)?;
    Ok((StatusCode::CREATED, Json(summary)).into_response())
}

async fn register_session(
    State(service): State<Arc<RuntimeService>>,
    Json(body): Json<RegisterSessionRequest>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    if body.username.trim().is_empty()
        || body.empire.trim().is_empty()
        || body.registration_code.trim().is_empty()
    {
        return Err(map_api_error(ApiError::BadRequest(
            "username, empire, and registrationCode are required".to_string(),
        )));
    }
    let id = service.create_session_with_label(body.label);
    let password = if local_auth_bypass_enabled() {
        "generated-password".to_string()
    } else {
        let base_url = spacemolt_base_url();
        let session_id = spacemolt_create_session(&base_url)
            .await
            .map_err(map_api_error)?;
        let generated = spacemolt_register(
            &base_url,
            &session_id,
            &body.username,
            &body.empire,
            &body.registration_code,
        )
        .await
        .map_err(map_api_error)?
        .unwrap_or_else(|| "generated-password".to_string());
        service
            .set_transport(
                id,
                SetTransportRequest::SpaceMolt {
                    base_url: base_url.clone(),
                    token: session_id,
                },
            )
            .await
            .map_err(map_api_error)?;
        if let Err(e) = service.refresh_state(id).await {
            tracing::warn!(%id, "initial state refresh failed: {e}");
        }
        generated
    };
    let response = RegisterSessionResponse {
        session_id: id.to_string(),
        password,
    };
    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn delete_session(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    match service.remove_session(&id).map_err(map_api_error)? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Ok(StatusCode::NOT_FOUND),
    }
}

async fn get_session(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Json<SessionSummary>, (StatusCode, Json<ErrorBody>)> {
    service
        .session_summary(&id)
        .await
        .map(Json)
        .map_err(map_api_error)
}

async fn snapshot_v2(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Json<RuntimeSnapshotResponse>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let snapshot = service.snapshot(uid).await.map_err(map_api_error)?;
    let state = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?;
    let response = RuntimeSnapshotResponse {
        session_id: id,
        snapshot: RuntimeHostSnapshotDto {
            is_halted: snapshot.is_halted,
            has_active_command: false,
            current_script_line: snapshot.current_script_line,
            current_script: if snapshot.script.is_empty() {
                None
            } else {
                Some(snapshot.script)
            },
        },
        latest_system: state.as_ref().map(|s| s.system.clone()),
        latest_poi: state.as_ref().map(|s| s.current_poi.id.clone()),
        docked: state.as_ref().map(|s| s.docked),
        fuel: state.as_ref().map(|s| s.ship.fuel),
        max_fuel: state.as_ref().map(|s| s.ship.max_fuel),
        credits: state.as_ref().map(|s| s.credits),
        last_updated_utc: Utc::now(),
    };
    Ok(Json(response))
}

async fn status_lines(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorBody>)> {
    let session = service
        .get_session_by_str(&id)
        .await
        .map_err(map_api_error)?;
    let session = session.lock().await;
    Ok(Json(session.status_lines.clone()))
}

async fn route(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
    Query(query): Query<RouteQuery>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let state = service.state(uid).await.map_err(map_api_error)?;
    let target = query.target.trim();
    if target.is_empty() {
        return Err(map_api_error(ApiError::BadRequest(
            "target is required".to_string(),
        )));
    }
    let Some(start) = state.system.as_deref() else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let Some(hops) = state.galaxy.astar_shortest_path_hops(start, target) else {
        return Err(map_api_error(ApiError::BadRequest(format!(
            "no route found from `{start}` to `{target}`"
        ))));
    };
    let total_jumps = i32::try_from(hops.len())
        .map_err(|_| map_api_error(ApiError::BadRequest("route too long".to_string())))?;
    Ok(Json(ActiveGoRouteDto {
        target: target.to_string(),
        hops,
        total_jumps,
        estimated_fuel_use: total_jumps,
        arrival_time: None,
    })
    .into_response())
}

async fn spacemolt_stats(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let session = service.get_session(uid).await.map_err(map_api_error)?;
    let session = session.lock().await;
    Ok(Json(serde_json::json!({
        "stateVersion": session.state_version,
        "lastUpdatedUtc": session.last_updated_utc,
        "statusLines": session.status_lines.len(),
    })))
}

async fn state_v2(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
    Query(query): Query<StateQuery>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let since = query.since.unwrap_or(0);
    let wait_ms = query.wait_ms.unwrap_or(0).min(MAX_WAIT_MS);
    if wait_ms > 0 {
        let changed = service
            .wait_for_state_change(uid, since, wait_ms)
            .await
            .map_err(map_api_error)?;
        if !changed {
            return Ok(StatusCode::NO_CONTENT.into_response());
        }
    }
    let (version, snapshot) = service
        .state_snapshot_with_version(uid)
        .await
        .map_err(map_api_error)?;
    let mut headers = HeaderMap::new();
    let value = HeaderValue::from_str(&version.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));
    headers.insert("X-Prayer-State-Version", value);
    Ok((headers, Json(snapshot)).into_response())
}

async fn galaxy_map(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(state.galaxy.map).into_response())
}

async fn galaxy_pois(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(state.galaxy.map.known_pois).into_response())
}

async fn galaxy_prices(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let market = state.galaxy.market;
    Ok(Json(GalaxyPricesResponse {
        global_median_buy_prices: market.global_median_buy_prices,
        global_median_sell_prices: market.global_median_sell_prices,
        global_weighted_mid_prices: market.global_weighted_mid_prices,
    })
    .into_response())
}

async fn galaxy_resources(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(state.galaxy.resources).into_response())
}

async fn galaxy_explored(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(state.galaxy.exploration).into_response())
}

async fn galaxy_catalog_items(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(state.galaxy.catalog.items_by_id).into_response())
}

async fn galaxy_catalog_ships(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(state.galaxy.catalog.ships_by_id).into_response())
}

async fn station(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let Some(station) = state.station else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(station).into_response())
}


async fn station_storage(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let Some(station) = state.station else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(StationStorageResponse {
        storage_credits: station.storage_credits,
        storage_items: station.storage_items,
    })
    .into_response())
}

async fn station_shipyard(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let Some(station) = state.station else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(StationShipyardResponse {
        shipyard_showroom: station.shipyard_showroom,
        shipyard_listings: station.shipyard_listings,
    })
    .into_response())
}

async fn station_craftable(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let Some(state) = load_runtime_state(&service, uid)
        .await
        .map_err(map_api_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let Some(station) = state.station else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    Ok(Json(station.craftable).into_response())
}

async fn set_script_v2(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
    Json(body): Json<SetScriptRequest>,
) -> Result<Json<CommandAckResponse>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    service
        .set_script(uid, body.script)
        .await
        .map_err(map_api_error)?;
    Ok(Json(CommandAckResponse {
        session_id: id,
        command: "set_script".to_string(),
        message: "script loaded and activated".to_string(),
    }))
}

async fn execute_script_v2(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Json<ExecuteScriptResponse>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let result = service
        .execute_script(uid, None)
        .await
        .map_err(map_api_error)?;
    Ok(Json(result))
}

async fn halt_v2(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Json<CommandAckResponse>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    service
        .halt(uid, Some("halt requested".to_string()))
        .await
        .map_err(map_api_error)?;
    Ok(Json(CommandAckResponse {
        session_id: id,
        command: "halt".to_string(),
        message: "halted".to_string(),
    }))
}

async fn get_skills(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
) -> Result<Json<SkillLibraryTextResponse>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    service
        .get_library_text(uid)
        .await
        .map(Json)
        .map_err(map_api_error)
}

async fn set_skills(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
    Json(body): Json<SetSkillLibraryRequest>,
) -> Result<Json<SkillLibraryTextResponse>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    service
        .set_library_text(uid, body.text)
        .await
        .map(Json)
        .map_err(map_api_error)
}

async fn load_runtime_state(
    service: &RuntimeService,
    id: Uuid,
) -> Result<Option<RuntimeGameStateDto>, ApiError> {
    let session = service.get_session(id).await?;
    let session = session.lock().await;
    if !session.has_state {
        return Ok(None);
    }
    Ok(Some(map_runtime_state(&session.effective_state)))
}

async fn spacemolt_passthrough(
    State(service): State<Arc<RuntimeService>>,
    Path(id): Path<String>,
    Json(body): Json<SpaceMoltPassthroughRequest>,
) -> Result<Json<SpaceMoltPassthroughResponse>, (StatusCode, Json<ErrorBody>)> {
    let uid = RuntimeService::parse_id(&id).map_err(map_api_error)?;
    let session = service.get_session(uid).await.map_err(map_api_error)?;
    let session = session.lock().await;
    let result = session
        .transport
        .execute_passthrough(&body.command, body.payload, Some(&session.effective_state))
        .await
        .map_err(|err| map_api_error(ApiError::Transport(err)))?;
    Ok(Json(SpaceMoltPassthroughResponse {
        succeeded: true,
        result: serde_json::json!({
            "completed": result.completed,
            "haltScript": result.halt_script,
            "resultMessage": result.result_message
        }),
        error: None,
    }))
}

fn map_api_error(error: ApiError) -> (StatusCode, Json<ErrorBody>) {
    let status = error.status();
    (
        status,
        Json(ErrorBody {
            error: error.to_string(),
        }),
    )
}
