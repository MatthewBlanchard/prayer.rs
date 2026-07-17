use std::collections::HashMap;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use http_body_util::BodyExt;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

const MAX_ADMIN_ORDERS: usize = 1_000;

#[derive(Clone, Default)]
struct MutationIdempotency {
    records: Arc<tokio::sync::Mutex<HashMap<String, CachedMutation>>>,
}

#[derive(Clone)]
struct CachedMutation {
    fingerprint: u64,
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

use crate::contracts::ErrorBody;

use prayer_api_contracts::{
    RuntimeVirtualCraftOrderReserveResponse, RuntimeVirtualCraftOrdersRequest,
    RuntimeVirtualCraftOrdersResponse, RuntimeVirtualMarketOrdersRequest,
    RuntimeVirtualMarketOrdersResponse, RuntimeVirtualOrderReserveRequest,
    RuntimeVirtualOrderReserveResponse,
};
use prayer_sdk::PrayerAdministration;
use prayer_sdk::SdkError as ApiError;

/// Build router.
pub fn build_v1_resource_router(administration: Arc<PrayerAdministration>) -> Router {
    let idempotency = MutationIdempotency::default();
    Router::new()
        .route("/health", get(health))
        .route(
            "/api/v1/admin/virtual-orders",
            get(get_virtual_orders).post(put_virtual_orders),
        )
        .route(
            "/api/v1/admin/virtual-orders/reservations",
            post(reserve_virtual_orders),
        )
        .route(
            "/api/v1/admin/virtual-orders/:order_id/fills",
            post(fill_virtual_order),
        )
        .route(
            "/api/v1/admin/virtual-orders/:order_id/reservation",
            axum::routing::delete(release_virtual_order),
        )
        .route(
            "/api/v1/admin/virtual-craft-orders",
            get(get_virtual_craft_orders).post(put_virtual_craft_orders),
        )
        .route(
            "/api/v1/admin/virtual-craft-orders/reservations",
            post(reserve_virtual_craft_orders),
        )
        .route(
            "/api/v1/admin/virtual-craft-orders/:order_id/fills",
            post(fill_virtual_craft_order),
        )
        .route(
            "/api/v1/admin/virtual-craft-orders/:order_id/reservation",
            axum::routing::delete(release_virtual_craft_order),
        )
        .route(
            "/api/v1/admin/market-movements",
            get(list_inventory_movements),
        )
        .route(
            "/api/v1/admin/market-movements/reservations",
            post(reserve_inventory_movement),
        )
        .route(
            "/api/v1/admin/market-movements/:id/start",
            post(start_inventory_movement),
        )
        .route(
            "/api/v1/admin/market-movements/:id/complete",
            post(complete_inventory_movement),
        )
        .route(
            "/api/v1/admin/market-movements/:id/fail",
            post(fail_inventory_movement),
        )
        .route(
            "/api/v1/admin/market-movements/:id/release",
            post(release_inventory_movement),
        )
        .route(
            "/api/v1/admin/market-movements/:id/reconcile",
            post(reconcile_inventory_movement),
        )
        .with_state(administration)
        .layer(middleware::from_fn_with_state(
            idempotency,
            enforce_mutation_idempotency,
        ))
        .layer(CorsLayer::permissive())
}

async fn enforce_mutation_idempotency(
    State(state): State<MutationIdempotency>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !requires_idempotency(request.method(), request.uri().path()) {
        return next.run(request).await;
    }
    let Some(key) = request
        .headers()
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return mutation_error(
            StatusCode::BAD_REQUEST,
            "idempotency_key_required",
            "Idempotency-Key header is required",
        );
    };
    let method = request.method().clone();
    let uri = request.uri().clone();
    let (parts, body) = request.into_parts();
    let body = match body.collect().await {
        Ok(body) => body.to_bytes(),
        Err(error) => {
            return mutation_error(
                StatusCode::BAD_REQUEST,
                "invalid_body",
                &format!("failed to read request body: {error}"),
            )
        }
    };
    let fingerprint = body.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    let cache_key = format!("{method} {} {key}", uri.path());
    let mut records = state.records.lock().await;
    if let Some(cached) = records.get(&cache_key) {
        if cached.fingerprint != fingerprint {
            return mutation_error(
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "idempotency key was reused with a different request",
            );
        }
        return cached_response(cached);
    }
    let request = Request::from_parts(parts, Body::from(body));
    let response = next.run(request).await;
    let (parts, body) = response.into_parts();
    let body = match body.collect().await {
        Ok(body) => body.to_bytes(),
        Err(error) => {
            return mutation_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "response_buffer_failed",
                &error.to_string(),
            )
        }
    };
    let cached = CachedMutation {
        fingerprint,
        status: parts.status,
        headers: parts.headers,
        body,
    };
    let response = cached_response(&cached);
    records.insert(cache_key, cached);
    response
}

fn requires_idempotency(method: &Method, path: &str) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) || path.contains("/plans/") {
        return false;
    }
    path.starts_with("/api/v1/admin/") || path.ends_with("/config")
}

fn cached_response(cached: &CachedMutation) -> Response {
    let mut response = Response::new(Body::from(cached.body.clone()));
    *response.status_mut() = cached.status;
    *response.headers_mut() = cached.headers.clone();
    response
}

fn mutation_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": {"code": code, "message": message, "retryable": false},
            "requestId": Uuid::new_v4().to_string(),
        })),
    )
        .into_response()
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "service": "Prayer", "status": "ok", "utc": Utc::now() }))
}

async fn list_inventory_movements(
    State(administration): State<Arc<PrayerAdministration>>,
) -> Json<prayer_api_contracts::RuntimeInventoryMovementsResponse> {
    Json(administration.inventory_movements())
}

async fn reserve_inventory_movement(
    State(administration): State<Arc<PrayerAdministration>>,
    Json(request): Json<prayer_api_contracts::RuntimeInventoryMovementReserveRequest>,
) -> Result<
    Json<prayer_api_contracts::RuntimeInventoryMovementReserveResponse>,
    (StatusCode, Json<ErrorBody>),
> {
    administration
        .reserve_inventory_movement(request)
        .await
        .map(Json)
        .map_err(map_api_error)
}

async fn transition_inventory_movement(
    administration: &PrayerAdministration,
    id: &str,
    status: prayer_api_contracts::RuntimeInventoryMovementStatusDto,
) -> Result<Json<prayer_api_contracts::RuntimeInventoryMovementDto>, (StatusCode, Json<ErrorBody>)>
{
    let movement_id = Uuid::parse_str(id)
        .map_err(|_| map_api_error(ApiError::BadRequest("invalid movement id".to_string())))?;
    administration
        .transition_inventory_movement(movement_id, status)
        .await
        .map(Json)
        .map_err(map_api_error)
}

macro_rules! inventory_movement_transition_handler {
    ($name:ident, $status:ident) => {
        async fn $name(
            State(administration): State<Arc<PrayerAdministration>>,
            Path(id): Path<String>,
        ) -> Result<
            Json<prayer_api_contracts::RuntimeInventoryMovementDto>,
            (StatusCode, Json<ErrorBody>),
        > {
            transition_inventory_movement(
                administration.as_ref(),
                &id,
                prayer_api_contracts::RuntimeInventoryMovementStatusDto::$status,
            )
            .await
        }
    };
}

inventory_movement_transition_handler!(start_inventory_movement, Running);
inventory_movement_transition_handler!(complete_inventory_movement, Completed);
inventory_movement_transition_handler!(fail_inventory_movement, Failed);
inventory_movement_transition_handler!(release_inventory_movement, Released);

async fn reconcile_inventory_movement(
    State(administration): State<Arc<PrayerAdministration>>,
    Path(id): Path<String>,
    Json(body): Json<crate::contracts::MarketMovementTransitionRequest>,
) -> Result<Json<prayer_api_contracts::RuntimeInventoryMovementDto>, (StatusCode, Json<ErrorBody>)>
{
    let movement_id = Uuid::parse_str(&id)
        .map_err(|_| map_api_error(ApiError::BadRequest("invalid movement id".to_string())))?;
    administration
        .reconcile_inventory_movement(movement_id, &body.reason)
        .await
        .map(Json)
        .map_err(map_api_error)
}

async fn get_virtual_orders(
    State(administration): State<Arc<PrayerAdministration>>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    Ok(Json(RuntimeVirtualMarketOrdersResponse {
        orders: administration
            .virtual_orders()
            .into_iter()
            .take(MAX_ADMIN_ORDERS)
            .collect(),
    })
    .into_response())
}

async fn put_virtual_orders(
    State(administration): State<Arc<PrayerAdministration>>,
    Json(body): Json<RuntimeVirtualMarketOrdersRequest>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    if body.orders.len() > MAX_ADMIN_ORDERS {
        return Ok(mutation_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "order_limit_exceeded",
            "virtual order collection exceeds the 1000-order limit",
        ));
    }
    Ok(Json(RuntimeVirtualMarketOrdersResponse {
        orders: administration.replace_virtual_orders(body.orders),
    })
    .into_response())
}

async fn reserve_virtual_orders(
    State(administration): State<Arc<PrayerAdministration>>,
    Json(body): Json<RuntimeVirtualOrderReserveRequest>,
) -> Response {
    let (orders, reservation_results) = administration.reserve_virtual_orders_detailed(body.uses);
    if reservation_results
        .iter()
        .any(|result| result.accepted != result.requested)
    {
        return mutation_error(
            StatusCode::CONFLICT,
            "reservation_conflict",
            "one or more virtual market orders could not be reserved",
        );
    }
    Json(RuntimeVirtualOrderReserveResponse {
        orders,
        reservation_results,
    })
    .into_response()
}

async fn fill_virtual_order(
    State(administration): State<Arc<PrayerAdministration>>,
    Path(order_id): Path<String>,
) -> Response {
    let Some(order) = administration
        .virtual_orders()
        .into_iter()
        .find(|order| order.id == order_id)
    else {
        return mutation_error(
            StatusCode::NOT_FOUND,
            "order_not_found",
            "virtual order not found",
        );
    };
    if order.reserved <= 0 {
        return mutation_error(
            StatusCode::CONFLICT,
            "reservation_conflict",
            "virtual order has no active reservation to fill",
        );
    }
    Json(RuntimeVirtualMarketOrdersResponse {
        orders: administration.fill_virtual_order(&order_id),
    })
    .into_response()
}

async fn release_virtual_order(
    State(administration): State<Arc<PrayerAdministration>>,
    Path(order_id): Path<String>,
) -> Response {
    let Some(order) = administration
        .virtual_orders()
        .into_iter()
        .find(|order| order.id == order_id)
    else {
        return mutation_error(
            StatusCode::NOT_FOUND,
            "order_not_found",
            "virtual order not found",
        );
    };
    if order.reserved <= 0 {
        return mutation_error(
            StatusCode::CONFLICT,
            "reservation_conflict",
            "virtual order has no active reservation to release",
        );
    }
    Json(RuntimeVirtualMarketOrdersResponse {
        orders: administration.release_virtual_order(&order_id),
    })
    .into_response()
}

async fn get_virtual_craft_orders(
    State(administration): State<Arc<PrayerAdministration>>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    Ok(Json(RuntimeVirtualCraftOrdersResponse {
        orders: administration
            .virtual_craft_orders()
            .into_iter()
            .take(MAX_ADMIN_ORDERS)
            .collect(),
    })
    .into_response())
}

async fn put_virtual_craft_orders(
    State(administration): State<Arc<PrayerAdministration>>,
    Json(body): Json<RuntimeVirtualCraftOrdersRequest>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    if body.orders.len() > MAX_ADMIN_ORDERS {
        return Ok(mutation_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "order_limit_exceeded",
            "virtual craft order collection exceeds the 1000-order limit",
        ));
    }
    Ok(Json(RuntimeVirtualCraftOrdersResponse {
        orders: administration.replace_virtual_craft_orders(body.orders),
    })
    .into_response())
}

async fn reserve_virtual_craft_orders(
    State(administration): State<Arc<PrayerAdministration>>,
    Json(body): Json<RuntimeVirtualOrderReserveRequest>,
) -> Response {
    let (orders, reservation_results) =
        administration.reserve_virtual_craft_orders_detailed(body.uses);
    if reservation_results
        .iter()
        .any(|result| result.accepted != result.requested)
    {
        return mutation_error(
            StatusCode::CONFLICT,
            "reservation_conflict",
            "one or more virtual craft orders could not be reserved",
        );
    }
    Json(RuntimeVirtualCraftOrderReserveResponse {
        orders,
        reservation_results,
    })
    .into_response()
}

async fn fill_virtual_craft_order(
    State(administration): State<Arc<PrayerAdministration>>,
    Path(order_id): Path<String>,
) -> Response {
    let Some(order) = administration
        .virtual_craft_orders()
        .into_iter()
        .find(|order| order.id == order_id)
    else {
        return mutation_error(
            StatusCode::NOT_FOUND,
            "order_not_found",
            "virtual craft order not found",
        );
    };
    if order.reserved <= 0 {
        return mutation_error(
            StatusCode::CONFLICT,
            "reservation_conflict",
            "virtual craft order has no active reservation to fill",
        );
    }
    Json(RuntimeVirtualCraftOrdersResponse {
        orders: administration.fill_virtual_craft_order(&order_id),
    })
    .into_response()
}

async fn release_virtual_craft_order(
    State(administration): State<Arc<PrayerAdministration>>,
    Path(order_id): Path<String>,
) -> Response {
    let Some(order) = administration
        .virtual_craft_orders()
        .into_iter()
        .find(|order| order.id == order_id)
    else {
        return mutation_error(
            StatusCode::NOT_FOUND,
            "order_not_found",
            "virtual craft order not found",
        );
    };
    if order.reserved <= 0 {
        return mutation_error(
            StatusCode::CONFLICT,
            "reservation_conflict",
            "virtual craft order has no active reservation to release",
        );
    }
    Json(RuntimeVirtualCraftOrdersResponse {
        orders: administration.release_virtual_craft_order(&order_id),
    })
    .into_response()
}

fn map_api_error(error: ApiError) -> (StatusCode, Json<ErrorBody>) {
    let status = match &error {
        ApiError::SessionNotFound | ApiError::BotNotFound { .. } | ApiError::RunNotFound { .. } => {
            StatusCode::NOT_FOUND
        }
        ApiError::AmbiguousBot { .. } | ApiError::LaneBusy { .. } => StatusCode::CONFLICT,
        ApiError::WaitTimedOut { .. } => StatusCode::REQUEST_TIMEOUT,
        ApiError::ShutdownInProgress => StatusCode::SERVICE_UNAVAILABLE,
        ApiError::InvalidSessionId | ApiError::Engine(_) => StatusCode::BAD_REQUEST,
        ApiError::Client(_) | ApiError::InvalidRuntimeState(_) => StatusCode::BAD_GATEWAY,
        ApiError::BadRequest(_) | ApiError::Command(_) => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(ErrorBody {
            error: error.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    async fn response_json(response: Response) -> Value {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("response json")
    }

    #[tokio::test]
    async fn market_movement_list_route_starts_empty() {
        let router = build_v1_resource_router(Arc::new(PrayerAdministration::default()));
        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/admin/market-movements")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["movements"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn retained_mutations_require_idempotency_keys() {
        let response = build_v1_resource_router(Arc::new(PrayerAdministration::default()))
            .oneshot(
                axum::http::Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/admin/virtual-orders")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"orders":[]}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "idempotency_key_required"
        );
    }

    #[tokio::test]
    async fn retained_mutations_replay_and_reject_conflicting_key_reuse() {
        let router = build_v1_resource_router(Arc::new(PrayerAdministration::default()));
        let request = |body: &'static str| {
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/v1/admin/virtual-orders")
                .header("content-type", "application/json")
                .header("idempotency-key", "same-key")
                .body(Body::from(body))
                .expect("request")
        };
        let first = router
            .clone()
            .oneshot(request(r#"{"orders":[]}"#))
            .await
            .expect("first response");
        let replay = router
            .clone()
            .oneshot(request(r#"{"orders":[]}"#))
            .await
            .expect("replay response");
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(replay.status(), StatusCode::OK);
        assert_eq!(response_json(first).await, response_json(replay).await);

        let conflict = router
            .oneshot(request(r#"{"orders":[],"different":true}"#))
            .await
            .expect("conflict response");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(conflict).await["error"]["code"],
            "idempotency_conflict"
        );
    }
}
