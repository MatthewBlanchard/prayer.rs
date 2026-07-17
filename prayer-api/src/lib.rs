#![recursion_limit = "512"]

//! Prayer HTTP API for runtime/session control.

mod contracts;
mod http;
mod openapi;
mod process_telemetry;
mod v1;
pub use http::build_v1_resource_router;
pub use openapi::openapi_v1;
pub use process_telemetry::start_process_telemetry;
pub use v1::build_v1_router;

pub fn build_router_with_sdk(sdk: std::sync::Arc<prayer_sdk::PrayerSdk>) -> axum::Router {
    build_v1_resource_router(std::sync::Arc::new(sdk.administration()))
        .merge(build_v1_router(sdk))
        .layer(tower_http::cors::CorsLayer::permissive())
}

#[cfg(test)]
mod tests {
    use prayer_sdk::SdkError as ApiError;
    use std::time::Duration;

    #[test]
    fn spacemolt_timeout_and_connection_close_are_transient_client_errors() {
        let timeout = ApiError::from(spacemolt_lib_rs::ClientError::Timeout(
            "query timed out".to_string(),
        ));
        assert!(matches!(
            timeout,
            ApiError::Client(ref err) if err.is_retryable() && err.is_network()
        ));

        let closed = ApiError::from(spacemolt_lib_rs::ClientError::ConnectionClosed(
            spacemolt_lib_rs::errors::ConnectionClosedError::new(
                "closed",
                Some(spacemolt_lib_rs::errors::CLOSE_CODE_CONNECTION_RATE_LIMITED),
                Some("connection_rate_limited retry_after=7".to_string()),
            ),
        ));
        assert!(matches!(
            closed,
            ApiError::Client(ref err)
                if err.is_retryable()
                    && err.is_network()
                    && err.retry_after() == Some(Duration::from_secs(7))
        ));
    }

    #[test]
    fn spacemolt_server_error_preserves_structured_retry_metadata() {
        let mut server =
            spacemolt_lib_rs::errors::SpacemoltError::new("rate_limited", "Too many requests");
        server.details = Some(serde_json::json!({ "retry_after": 2.5, "scope": "query" }));

        let error = ApiError::from(spacemolt_lib_rs::ClientError::Server(server));
        let ApiError::Client(error) = error else {
            panic!("client error")
        };
        assert_eq!(error.server_code(), Some("rate_limited"));
        assert_eq!(error.retry_after(), Some(Duration::from_millis(2_500)));
        let details = error.details();
        assert_eq!(details.server_code.as_deref(), Some("rate_limited"));
        assert_eq!(details.retry_after_millis, Some(2_500));
    }

    #[test]
    fn spacemolt_catalog_errors_remain_non_transient_bad_requests() {
        let unknown = ApiError::from(spacemolt_lib_rs::ClientError::UnknownAction(
            "spacemolt/missing".to_string(),
        ));
        let unimplemented = ApiError::from(spacemolt_lib_rs::ClientError::NotImplemented(
            "future action",
        ));

        assert!(matches!(unknown, ApiError::Command(_)));
        assert!(matches!(unimplemented, ApiError::Command(_)));
    }

    #[test]
    fn http_handlers_do_not_call_session_transport_directly() {
        let http = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http.rs"),
        )
        .expect("http handlers");
        assert!(!http.contains(".transport"));
        assert!(!http.contains("execute_api("));
        assert!(!http.contains(".commands()"));
        assert!(!http.contains(".send("));
        assert!(!http.contains("spacemolt_account("));
        let production = http.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("RuntimeService"));
        assert!(!production.contains(".service()"));
    }

    #[test]
    fn prayer_does_not_duplicate_spacemolt_client_timeout_or_retry_policy() {
        let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo")
            .join("prayer-sdk/src");
        let host = rust_sources_under(&source_dir.join("service"));
        let sessions = std::fs::read_to_string(source_dir.join("service/sessions.rs"))
            .expect("sessions source");

        for forbidden in [
            "PRAYER_SCRIPT_STEP_IO_TIMEOUT_MS",
            "PRAYER_TRANSIENT_RETRY_BASE_MS",
            "PRAYER_TRANSIENT_RETRY_MAX_MS",
            "TRANSIENT_RETRY_MAX_ATTEMPTS",
            "await_step_io",
        ] {
            assert!(
                !host.contains(forbidden),
                "Prayer must leave SpaceMolt timeout/retry policy to spacemolt-lib-rs: {forbidden}"
            );
        }
        assert!(!sessions.contains("transient_error_attempts"));
    }

    #[test]
    fn prayer_runtime_has_no_live_spacemolt_transport_file() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo");
        assert!(!repo.join("prayer-runtime/src/transport/http.rs").exists());
        assert!(!repo
            .join("prayer-runtime/src/transport/state_mapping.rs")
            .exists());
        assert!(!repo.join("prayer-runtime/src/transport/wire.rs").exists());
        assert!(!repo
            .join("prayer-runtime/src/transport/golden_tests.rs")
            .exists());
    }

    #[test]
    fn service_has_no_runtime_transport_field() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let service = rust_sources_under(&root.join("service"));
        assert!(!service.contains("transport: Arc<dyn RuntimeTransport>"));
        assert!(!service.contains("SpaceMoltTransport"));
        assert!(!service.contains("local_harness"));
        assert!(!service.contains("fetch_state_with_session_retry"));
        assert!(!service.contains("fetch_state_once"));
        assert!(!service.contains("fetch_state_request_for"));
        assert!(!service.contains("StateRefreshPlan"));
        assert!(!service.contains("FetchStateRequest"));
        assert!(!service.contains("execute_spacemolt_command"));
    }

    #[test]
    fn sessions_cannot_retain_shared_world_projection() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo")
            .join("prayer-sdk/src");
        let sessions =
            std::fs::read_to_string(root.join("service/sessions.rs")).expect("sessions source");
        let handle = sessions
            .split("pub struct SessionHandle")
            .nth(1)
            .and_then(|source| source.split("}\n").next())
            .expect("SessionHandle body");
        assert!(!handle.contains("effective_state"));
        assert!(!handle.contains("GameState"));
        assert!(!handle.contains("GalaxyData"));
        assert!(!handle.contains("MarketData"));
        assert!(!handle.contains("SalvageData"));
        assert!(!handle.contains("WorldState"));

        let state_source = std::fs::read_to_string(
            root.parent()
                .and_then(std::path::Path::parent)
                .expect("repository root")
                .join("prayer-state/src/state.rs"),
        )
        .expect("shared state source");
        let actor_fields = state_source
            .split("pub struct BotState {")
            .nth(1)
            .and_then(|source| source.split("\n}").next())
            .expect("BotState field whitelist");
        for forbidden in [
            "galaxy:",
            "market:",
            "storage:",
            "faction_storage:",
            "faction_garage:",
            "salvage:",
            "wildlife_by_poi:",
            "system_agents:",
            "managed_players:",
            "nearest_station:",
        ] {
            assert!(
                !actor_fields.contains(forbidden),
                "BotState can retain shared field {forbidden}"
            );
        }
    }

    #[test]
    fn execution_builds_borrowed_context_without_compatibility_materialization() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo")
            .join("prayer-sdk/src");
        let service = rust_sources_under(&root.join("service"));
        let execution = rust_sources_under(&root.join("service/execution"));
        assert!(!service.contains("ExecutionStateAdapter"));
        let adapter = std::fs::read_to_string(root.join("knowledge/execution_adapter.rs"))
            .expect("adapter source");
        assert!(!adapter.contains("ExecutionStateAdapter"));
        assert!(adapter.contains("world_read_state"));
        assert!(execution.contains("ExecutionReadContext"));
        assert!(execution.contains("world_read_state"));
    }

    #[test]
    fn mcp_workflows_do_not_own_cross_bot_assignment_truth() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo");
        let workflows = rust_sources_under_any_extension(
            &repo.join("reference-client-ts/src/server/scoped-mcp/workflows"),
            "ts",
        );
        for forbidden in [
            "assignedArbitrage",
            "assignedLogistics",
            "assignedCrafts",
            "assignmentMutex",
            "Map<string, helpers.AutoArbitrageReservation>",
        ] {
            assert!(
                !workflows.contains(forbidden),
                "forbidden MCP truth: {forbidden}"
            );
        }
    }

    fn rust_sources_under(root: &std::path::Path) -> String {
        let mut source = String::new();
        for entry in std::fs::read_dir(root).expect("source directory") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                source.push_str(&rust_sources_under(&path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                source.push_str(&std::fs::read_to_string(path).expect("Rust source"));
            }
        }
        source
    }

    fn rust_sources_under_any_extension(root: &std::path::Path, extension: &str) -> String {
        let mut source = String::new();
        for entry in std::fs::read_dir(root).expect("source directory") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                source.push_str(&rust_sources_under_any_extension(&path, extension));
            } else if path
                .extension()
                .is_some_and(|candidate| candidate == extension)
            {
                source.push_str(&std::fs::read_to_string(path).expect("source"));
            }
        }
        source
    }

    #[test]
    fn operation_failure_module_cannot_fetch_state() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo");
        let failure = std::fs::read_to_string(repo.join("prayer-runtime/src/operation_failure.rs"))
            .expect("operation failure module");
        assert!(!failure.contains("fetch_state"));
    }

    #[test]
    fn legacy_runtime_prefix_cannot_reappear_in_production_source() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo");
        let mut production = String::new();
        for (path, extension) in [
            ("prayer-api/src", "rs"),
            ("reference-client-ts/src", "ts"),
            ("reference-client-ts/src", "tsx"),
            ("prayer-sdk/src", "rs"),
            ("prayer-sdk-ts/src", "ts"),
        ] {
            production.push_str(&rust_sources_under_any_extension(
                &repo.join(path),
                extension,
            ));
        }
        let forbidden = concat!("/api/", "runtime");
        assert!(
            !production.contains(forbidden),
            "forbidden production prefix"
        );
    }
}
