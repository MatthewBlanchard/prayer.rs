use serde_json::{json, Value};

pub fn openapi_v1() -> Value {
    let error = json!({"$ref":"#/components/schemas/ErrorEnvelope"});
    let errors = json!({
        "400":{"description":"Invalid request","content":{"application/json":{"schema":error}}},
        "401":{"description":"Unauthorized","content":{"application/json":{"schema":error}}},
        "404":{"description":"Resource not found","content":{"application/json":{"schema":error}}},
        "409":{"description":"Conflict","content":{"application/json":{"schema":error}}},
        "422":{"description":"Validation failed","content":{"application/json":{"schema":error}}},
        "503":{"description":"Temporarily unavailable","content":{"application/json":{"schema":error}}}
    });
    let operation = |operation_id: &str, summary: &str, response: &str| {
        json!({
            "operationId": operation_id,
            "summary": summary,
            "responses": {"200":{"description":"Success","content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{response}")}}}}}
        })
    };
    let mut paths = serde_json::Map::new();
    paths.insert(
        "/api/v1/meta".into(),
        json!({"get":operation("getMeta", "Read API capabilities", "Meta")}),
    );
    paths.insert(
        "/api/v1/state".into(),
        json!({"get":{
            "operationId":"getState",
            "summary":"Read conditionally versioned fleet, world, and catalog state",
            "parameters":[
                {"name":"fleet_version","in":"query","schema":{"type":"integer"}},
                {"name":"world_version","in":"query","schema":{"type":"integer"}},
                {"name":"map_version","in":"query","schema":{"type":"integer"}},
                {"name":"resources_version","in":"query","schema":{"type":"integer"}},
                {"name":"wildlife_version","in":"query","schema":{"type":"integer"}},
                {"name":"markets_version","in":"query","schema":{"type":"integer"}},
                {"name":"storage_version","in":"query","schema":{"type":"integer"}},
                {"name":"facilities_version","in":"query","schema":{"type":"integer"}},
                {"name":"observations_version","in":"query","schema":{"type":"integer"}},
                {"name":"communications_version","in":"query","schema":{"type":"integer"}},
                {"name":"factions_version","in":"query","schema":{"type":"integer"}},
                {"name":"catalog_version","in":"query","schema":{"type":"string"}}
            ],
            "responses":{"200":{"description":"Success","content":{"application/json":{"schema":{"$ref":"#/components/schemas/StateResponse"}}}}}
        }}),
    );
    paths.insert(
        "/api/v1/routes".into(),
        json!({"post":request_operation("selectRoutes", "Select authoritative cached galaxy routes", "RouteBatchRequest", "RouteBatchResponse", errors.clone(), false)}),
    );
    paths.insert(
        "/api/v1/bots".into(),
        json!({"get":operation("listBots", "List bots", "BotList")}),
    );
    paths.insert(
        "/api/v1/bots/register".into(),
        json!({"post":request_operation("registerBot", "Register a new SpaceMolt bot", "RegisterBotRequest", "RegisterBotResponse", errors.clone(), false)}),
    );
    paths.insert(
        "/api/v1/bots/{botId}".into(),
        json!({"get":operation("getBot", "Resolve a bot", "BotSummary")}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/queue".into(),
        json!({"get":operation("getBotQueue", "Read bot queue", "QueueResponse")}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/queue/normal".into(),
        json!({"get":operation("getBotNormalQueue", "Read the bot normal queue", "QueueLane")}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/queue/override".into(),
        json!({"get":operation("getBotOverrideQueue", "Read the bot override queue", "QueueLane")}),
    );
    paths.insert("/api/v1/bots/{botId}/halt".into(), json!({"post":{"operationId":"haltBot","summary":"Halt bot execution","requestBody":{"required":false,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/CancelRequest"}}}},"responses":{"204":{"description":"Halted"}}}}));
    paths.insert(
        "/api/v1/bots/{botId}/action-runs".into(),
        json!({"post":run_submit("startActionRun", "Submit typed action run", "ActionRunRequest", "ActionRunResponse", errors.clone())}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/action-overrides".into(),
        json!({"post":accepted_operation("executeActionOverride", "Enqueue typed actions on the override lane", "ActionOverrideRequest", errors.clone())}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/script-overrides".into(),
        json!({"post":accepted_operation("executeScriptOverride", "Enqueue a linear PrayerLang override", "ScriptOverrideRequest", errors.clone())}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/action-runs/{runId}".into(),
        json!({"get":operation("getActionRun", "Read typed action run", "ActionRunResponse")}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/action-runs/{runId}/cancel".into(),
        json!({"post":optional_request_operation("cancelActionRun", "Cancel typed action run", "CancelRequest", "ActionRunResponse")}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/script-runs".into(),
        json!({"post":run_submit("startScriptRun", "Submit PrayerLang run", "ScriptRunRequest", "ScriptRunResponse", errors.clone())}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/script-runs/{runId}".into(),
        json!({"get":operation("getScriptRun", "Read PrayerLang run", "ScriptRunResponse")}),
    );
    paths.insert(
        "/api/v1/bots/{botId}/script-runs/{runId}/cancel".into(),
        json!({"post":optional_request_operation("cancelScriptRun", "Cancel PrayerLang run", "CancelRequest", "ScriptRunResponse")}),
    );
    paths.insert("/api/v1/admin/virtual-orders".into(), json!({
        "get": operation("listVirtualOrders", "List virtual market orders", "VirtualOrderList"),
        "post": request_operation("createVirtualOrders", "Create or update virtual market orders", "VirtualOrderWrite", "VirtualOrderList", errors.clone(), true)
    }));
    paths.insert("/api/v1/admin/virtual-orders/reservations".into(), json!({"post":request_operation("reserveVirtualOrders", "Reserve virtual market orders", "ReservationRequest", "ReservationResponse", errors.clone(), true)}));
    paths.insert("/api/v1/admin/virtual-orders/{orderId}/fills".into(), json!({"post":request_operation("fillVirtualOrder", "Fill a virtual market order", "EmptyRequest", "VirtualOrderList", errors.clone(), true)}));
    paths.insert("/api/v1/admin/virtual-orders/{orderId}/reservation".into(), json!({"delete":request_operation("releaseVirtualOrder", "Release a virtual market reservation", "EmptyRequest", "VirtualOrderList", errors.clone(), true)}));
    paths.insert("/api/v1/admin/virtual-craft-orders".into(), json!({
        "get": operation("listVirtualCraftOrders", "List virtual craft orders", "VirtualCraftOrderList"),
        "post": request_operation("createVirtualCraftOrders", "Create or update virtual craft orders", "VirtualCraftOrderWrite", "VirtualCraftOrderList", errors.clone(), true)
    }));
    paths.insert("/api/v1/admin/virtual-craft-orders/reservations".into(), json!({"post":request_operation("reserveVirtualCraftOrders", "Reserve virtual craft orders", "ReservationRequest", "CraftReservationResponse", errors.clone(), true)}));
    paths.insert("/api/v1/admin/virtual-craft-orders/{orderId}/fills".into(), json!({"post":request_operation("fillVirtualCraftOrder", "Fill a virtual craft order", "EmptyRequest", "VirtualCraftOrderList", errors.clone(), true)}));
    paths.insert("/api/v1/admin/virtual-craft-orders/{orderId}/reservation".into(), json!({"delete":request_operation("releaseVirtualCraftOrder", "Release a virtual craft reservation", "EmptyRequest", "VirtualCraftOrderList", errors.clone(), true)}));
    paths.insert("/api/v1/admin/market-movements".into(), json!({"get":operation("listMarketMovements", "List market movements", "MarketMovementList")}));
    paths.insert("/api/v1/admin/market-movements/reservations".into(), json!({"post":request_operation("reserveMarketMovement", "Reserve a market movement", "MarketMovementReserveRequest", "MarketMovementReserveResponse", errors.clone(), true)}));
    for (suffix, id, summary) in [
        ("start", "startMarketMovement", "Start a market movement"),
        (
            "complete",
            "completeMarketMovement",
            "Complete a market movement",
        ),
        ("fail", "failMarketMovement", "Fail a market movement"),
        (
            "release",
            "releaseMarketMovement",
            "Release a market movement",
        ),
        (
            "reconcile",
            "reconcileMarketMovement",
            "Reconcile a market movement with an audit reason",
        ),
    ] {
        let request = if suffix == "reconcile" {
            "MarketMovementTransitionRequest"
        } else {
            "EmptyRequest"
        };
        paths.insert(format!("/api/v1/admin/market-movements/{{movementId}}/{suffix}"), json!({"post":request_operation(id, summary, request, "MarketMovement", errors.clone(), true)}));
    }

    // Path parameters are route metadata, shared by every operation on a path.
    for (path, item) in &mut paths {
        let parameters = path
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name))
            .map(|name| json!({"name":name,"in":"path","required":true,"schema":{"type":"string"}}))
            .collect::<Vec<_>>();
        if !parameters.is_empty() {
            item.as_object_mut()
                .expect("path item object")
                .insert("parameters".into(), Value::Array(parameters));
        }
    }

    json!({
        "openapi":"3.1.0",
        "info":{"title":"Prayer Bot API","version":"1.0.0"},
        "servers":[{"url":"http://127.0.0.1:7777"}],
        "paths": paths,
        "components":{"securitySchemes":{"bearerAuth":{"type":"http","scheme":"bearer"}},"schemas":schemas()}
    })
}

fn request_operation(
    operation_id: &str,
    summary: &str,
    request: &str,
    response: &str,
    errors: Value,
    idempotent: bool,
) -> Value {
    let mut responses = errors.as_object().cloned().unwrap_or_default();
    responses.insert("200".into(), json!({"description":"Success","content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{response}")}}}}));
    let mut parameters = Vec::new();
    if idempotent {
        parameters.push(json!({"name":"Idempotency-Key","in":"header","required":true,"schema":{"type":"string","minLength":1}}));
    }
    json!({
        "operationId": operation_id,
        "summary": summary,
        "parameters": parameters,
        "requestBody": if request == "EmptyRequest" { Value::Null } else { json!({"required":true, "content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{request}")}}}}) },
        "responses": responses
    })
}

fn run_submit(
    operation_id: &str,
    summary: &str,
    request: &str,
    response: &str,
    errors: Value,
) -> Value {
    let mut responses = errors.as_object().cloned().unwrap_or_default();
    responses.insert("202".into(), json!({"description":"Accepted","content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{response}")}}}}));
    json!({
        "operationId":operation_id,
        "summary":summary,
        "parameters":[{"name":"Idempotency-Key","in":"header","schema":{"type":"string"}}],
        "requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{request}")}}}},
        "responses":responses
    })
}

fn accepted_operation(operation_id: &str, summary: &str, request: &str, errors: Value) -> Value {
    let mut responses = errors.as_object().cloned().unwrap_or_default();
    responses.insert("202".into(), json!({"description":"Accepted","content":{"application/json":{"schema":{"$ref":"#/components/schemas/OverrideResponse"}}}}));
    json!({
        "operationId": operation_id,
        "summary": summary,
        "requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{request}")}}}},
        "responses": responses
    })
}

fn optional_request_operation(
    operation_id: &str,
    summary: &str,
    request: &str,
    response: &str,
) -> Value {
    json!({
        "operationId": operation_id,
        "summary": summary,
        "requestBody":{"required":false,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{request}")}}}},
        "responses":{"200":{"description":"Success","content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{response}")}}}}}
    })
}

fn schemas() -> Value {
    let mut schemas = serde_json::Map::new();
    insert_schema::<prayer_state::BotState>(&mut schemas, "BotState");
    insert_schema::<crate::contracts::V1MetaResponse>(&mut schemas, "Meta");
    insert_schema::<crate::contracts::RouteBatchRequest>(&mut schemas, "RouteBatchRequest");
    insert_schema::<crate::contracts::RouteBatchResponse>(&mut schemas, "RouteBatchResponse");
    insert_schema::<crate::contracts::V1BotSummary>(&mut schemas, "BotSummary");
    insert_schema::<Vec<crate::contracts::V1BotSummary>>(&mut schemas, "BotList");
    insert_schema::<crate::contracts::RegisterBotRequest>(&mut schemas, "RegisterBotRequest");
    insert_schema::<crate::contracts::RegisterBotResponse>(&mut schemas, "RegisterBotResponse");
    insert_schema::<crate::contracts::V1CancelRequest>(&mut schemas, "CancelRequest");
    insert_schema::<crate::contracts::V1ErrorEnvelope>(&mut schemas, "ErrorEnvelope");
    insert_schema::<crate::contracts::V1ActionRunRequest>(&mut schemas, "ActionRunRequest");
    insert_schema::<crate::contracts::V1ScriptRunRequest>(&mut schemas, "ScriptRunRequest");
    insert_schema::<crate::contracts::V1ActionOverrideRequest>(
        &mut schemas,
        "ActionOverrideRequest",
    );
    insert_schema::<crate::contracts::V1ScriptOverrideRequest>(
        &mut schemas,
        "ScriptOverrideRequest",
    );
    insert_schema::<crate::contracts::V1OverrideResponse>(&mut schemas, "OverrideResponse");
    insert_schema::<crate::contracts::V1ActionRunResponse>(&mut schemas, "ActionRunResponse");
    insert_schema::<crate::contracts::V1ScriptRunResponse>(&mut schemas, "ScriptRunResponse");
    insert_schema::<crate::contracts::V1QueueResponse>(&mut schemas, "QueueResponse");
    insert_schema::<prayer_sdk::QueueLaneSnapshot>(&mut schemas, "QueueLane");
    insert_schema::<crate::v1::V1StateResponse>(&mut schemas, "StateResponse");
    insert_schema::<std::collections::HashMap<String, prayer_state::StationMarketData>>(
        &mut schemas,
        "StationMarkets",
    );
    insert_schema::<
        std::collections::HashMap<
            String,
            std::collections::HashMap<String, std::collections::HashMap<String, i64>>,
        >,
    >(&mut schemas, "StorageByOwner");
    insert_schema::<prayer_api_contracts::RuntimeVirtualMarketOrdersResponse>(
        &mut schemas,
        "VirtualOrderList",
    );
    insert_schema::<prayer_api_contracts::RuntimeVirtualMarketOrdersRequest>(
        &mut schemas,
        "VirtualOrderWrite",
    );
    insert_schema::<prayer_api_contracts::RuntimeVirtualOrderReserveResponse>(
        &mut schemas,
        "ReservationResponse",
    );
    insert_schema::<prayer_api_contracts::RuntimeVirtualCraftOrdersResponse>(
        &mut schemas,
        "VirtualCraftOrderList",
    );
    insert_schema::<prayer_api_contracts::RuntimeVirtualCraftOrdersRequest>(
        &mut schemas,
        "VirtualCraftOrderWrite",
    );
    insert_schema::<prayer_api_contracts::RuntimeVirtualCraftOrderReserveResponse>(
        &mut schemas,
        "CraftReservationResponse",
    );
    insert_schema::<crate::contracts::EmptyRequest>(&mut schemas, "EmptyRequest");
    insert_schema::<crate::contracts::MarketMovementTransitionRequest>(
        &mut schemas,
        "MarketMovementTransitionRequest",
    );
    insert_schema::<prayer_api_contracts::RuntimeVirtualOrderReservationResultDto>(
        &mut schemas,
        "ReservationResult",
    );
    insert_schema::<prayer_api_contracts::RuntimeVirtualOrderReserveRequest>(
        &mut schemas,
        "ReservationRequest",
    );
    insert_schema::<prayer_api_contracts::RuntimeInventoryClaimDto>(&mut schemas, "InventoryClaim");
    insert_schema::<prayer_api_contracts::RuntimeInventoryMovementReserveRequest>(
        &mut schemas,
        "MarketMovementReserveRequest",
    );
    insert_schema::<prayer_api_contracts::RuntimeInventoryMovementDto>(
        &mut schemas,
        "MarketMovement",
    );
    insert_schema::<prayer_api_contracts::RuntimeInventoryMovementReserveResponse>(
        &mut schemas,
        "MarketMovementReserveResponse",
    );
    insert_schema::<prayer_api_contracts::RuntimeInventoryMovementsResponse>(
        &mut schemas,
        "MarketMovementList",
    );
    Value::Object(schemas)
}

fn insert_schema<T: schemars::JsonSchema>(
    schemas: &mut serde_json::Map<String, Value>,
    public_name: &str,
) {
    let root = schemars::schema_for!(T);
    let mut schema = serde_json::to_value(root.schema).expect("serialize DTO schema");
    rewrite_definition_refs(&mut schema);
    schemas.insert(public_name.to_owned(), schema);
    for (name, definition) in root.definitions {
        let mut definition = serde_json::to_value(definition).expect("serialize DTO definition");
        rewrite_definition_refs(&mut definition);
        schemas.insert(public_schema_name(&name).to_owned(), definition);
    }
}

fn rewrite_definition_refs(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/definitions/") {
                    *reference = format!("#/components/schemas/{}", public_schema_name(name));
                }
            }
            for child in object.values_mut() {
                rewrite_definition_refs(child);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(rewrite_definition_refs),
        _ => {}
    }
}

fn public_schema_name(name: &str) -> &str {
    match name {
        "RuntimeInventoryClaimDto" => "InventoryClaim",
        "RuntimeInventoryMovementDto" => "MarketMovement",
        "RuntimeInventoryMovementStatusDto" => "MarketMovementStatus",
        "FleetActiveRoute" => "ActiveRoute",
        "FleetScriptExecution" => "ScriptExecution",
        "FleetScriptOutcome" => "ScriptExecutionOutcome",
        "OwnedShipInfo" => "OwnedShipDetail",
        "VirtualOrderUse" => "ReservationUse",
        "RuntimeGalaxyCatalogDto" => "GalaxyCatalog",
        "RuntimeGalaxyMapSnapshotDto" => "GalaxyMap",
        "RuntimeGalaxyResourcesDto" => "GalaxyResources",
        "RuntimeGalaxyWildlifeDto" => "GalaxyWildlife",
        "V1StateVersions" => "StateVersions",
        "V1StationMarketDelta" => "StationMarketDelta",
        "V1WorldState" => "WorldState",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visit_refs(value: &Value, refs: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                if let Some(Value::String(reference)) = object.get("$ref") {
                    refs.push(reference.clone());
                }
                object.values().for_each(|child| visit_refs(child, refs));
            }
            Value::Array(values) => values.iter().for_each(|child| visit_refs(child, refs)),
            _ => {}
        }
    }

    #[test]
    fn every_component_reference_resolves_and_structured_schemas_are_typed() {
        let document = openapi_v1();
        let schemas = document["components"]["schemas"].as_object().unwrap();
        let mut refs = Vec::new();
        visit_refs(&document["paths"], &mut refs);
        visit_refs(&document["components"], &mut refs);
        for reference in refs {
            let name = reference
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("non-component schema reference: {reference}"));
            assert!(
                schemas.contains_key(name),
                "missing schema component {name}"
            );
        }

        // A closed empty request is intentional: these operations have no body.
        let opaque_or_empty_allowlist = std::collections::HashMap::from([(
            "EmptyRequest",
            "closed marker used to suppress request bodies for bodyless mutations",
        )]);
        for (name, schema) in schemas {
            let untyped_object = schema == &json!({})
                || (schema.get("type") == Some(&json!("object"))
                    && schema.get("properties").is_none()
                    && schema.get("additionalProperties").is_none());
            assert!(
                !untyped_object || opaque_or_empty_allowlist.contains_key(name.as_str()),
                "structured public schema {name} is an untyped object"
            );
        }
    }
    #[test]
    fn schema_is_openapi_31_and_covers_every_v1_route() {
        let schema = openapi_v1();
        assert_eq!(schema["openapi"], "3.1.0");
        assert_eq!(schema["paths"].as_object().unwrap().len(), 33);
    }

    #[test]
    fn committed_schema_matches_generator() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("openapi")
            .join("prayer-v1.json");
        let committed: Value =
            serde_json::from_slice(&std::fs::read(path).expect("committed OpenAPI schema"))
                .expect("valid committed OpenAPI schema");
        assert_eq!(committed, openapi_v1());
    }

    #[test]
    fn fleet_state_is_a_typed_wire_contract() {
        let schema = openapi_v1();
        let schemas = &schema["components"]["schemas"];
        assert_eq!(
            schemas["FleetEntry"]["properties"]["state"]["$ref"],
            "#/components/schemas/BotState"
        );
        assert_eq!(
            schemas["BotState"]["properties"]["owned_ship_details"]["items"]["$ref"],
            "#/components/schemas/OwnedShipDetail"
        );
        assert!(schemas["BotState"]["properties"]
            .get("ownedShips")
            .is_none());
    }

    #[test]
    fn rejected_movement_serialization_matches_generated_contract() {
        let response = prayer_api_contracts::RuntimeInventoryMovementReserveResponse {
            accepted: false,
            movement: None,
            unavailable_claims: vec![prayer_api_contracts::RuntimeInventoryClaimDto {
                lot_id: None,
                source_kind: "cargo".into(),
                owner_id: "player-1".into(),
                location_id: "station-1".into(),
                item_id: "ore".into(),
                quantity: 2,
            }],
            unavailable_virtual_order_uses: vec![],
        };
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["accepted"], false);
        assert!(value.get("movement").is_none());
        assert_eq!(value["unavailableClaims"][0]["sourceKind"], "cargo");

        let contract = openapi_v1();
        let schema = &contract["components"]["schemas"]["MarketMovementReserveResponse"];
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "accepted"));
        assert!(schema["properties"].get("movement").is_some());
        assert!(schema["properties"].get("unavailableClaims").is_some());
    }

    #[test]
    fn typed_run_serialization_matches_discriminated_openapi_contracts() {
        let identity = crate::contracts::V1RunIdentity {
            run_id: "run-1".into(),
            bot_id: "bot-1".into(),
            run_version: 2,
            prayerlang: "wait 1;".into(),
        };
        let action = crate::contracts::V1ActionRunResponse::Failed {
            run: identity.clone(),
            outcome: prayer_sdk::ActionRunOutcome::Failed {
                action_index: 0,
                message: "boom".into(),
            },
        };
        let script = crate::contracts::V1ScriptRunResponse::Cancelled {
            run: identity,
            outcome: prayer_sdk::ScriptRunOutcome::Error {
                kind: prayer_sdk::ScriptErrorKind::Cancelled,
                message: "stopped".into(),
            },
        };
        let action = serde_json::to_value(action).unwrap();
        let script = serde_json::to_value(script).unwrap();
        assert_eq!(action["status"], "failed");
        assert_eq!(action["outcome"]["status"], "failed");
        assert_eq!(action["outcome"]["action_index"], 0);
        assert_eq!(script["status"], "cancelled");
        assert_eq!(script["outcome"]["status"], "error");
        assert_eq!(script["outcome"]["kind"], "cancelled");

        let schemas = &openapi_v1()["components"]["schemas"];
        assert!(schemas["ActionRunResponse"]["oneOf"].is_array());
        assert!(schemas["ScriptRunResponse"]["oneOf"].is_array());
    }
}
