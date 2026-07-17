//! Planner failures and response projections at the SpaceMolt execution edge.

use std::time::Duration;

use serde_json::Value;
use spacemolt_lib_rs::errors::{retry_after_ms_from_close, ClientError};
use thiserror::Error;

/// Failure returned to Prayer's pure operation planner.
#[derive(Debug, Error)]
pub enum OperationFailure {
    /// Failure reported by the canonical SpaceMolt client.
    #[error(transparent)]
    Client(#[from] ClientError),
    /// Command was not supported.
    #[error("unsupported command '{0}'")]
    InvalidIntent(String),
    /// Failure surfaced by command orchestration; the message is already
    /// fully rendered for the user.
    #[error("{0}")]
    Policy(String),
}

impl OperationFailure {
    /// Whether this error is likely to be resolved by retrying after a short
    /// delay. These are infrastructure/rate-limit failures, not game-rule
    /// rejections such as `400 no_equipment`.
    pub fn is_transient(&self) -> bool {
        match self {
            OperationFailure::Client(ClientError::Server(err)) => {
                err.retry_after().is_some()
                    || matches!(
                        err.code.as_str(),
                        "rate_limited"
                            | "timeout"
                            | "temporarily_unavailable"
                            | "service_unavailable"
                            | "server_error"
                            | "internal_error"
                    )
            }
            OperationFailure::Client(
                ClientError::ConnectionClosed(_) | ClientError::Timeout(_),
            ) => true,
            OperationFailure::Client(
                ClientError::UnknownAction(_)
                | ClientError::NotImplemented(_)
                | ClientError::CredentialStore(_)
                | ClientError::PostRegistration { .. },
            ) => false,
            OperationFailure::InvalidIntent(_) | OperationFailure::Policy(_) => false,
        }
    }

    /// Whether the failure occurred at the network/WebSocket layer.
    pub fn is_network(&self) -> bool {
        matches!(
            self,
            OperationFailure::Client(ClientError::ConnectionClosed(_) | ClientError::Timeout(_))
        )
    }

    /// Machine-readable SpaceMolt server error code, when available.
    pub fn server_code(&self) -> Option<&str> {
        match self {
            OperationFailure::Client(ClientError::Server(err)) => Some(&err.code),
            _ => None,
        }
    }

    /// Human-readable upstream message without rendering away structured fields.
    pub fn upstream_message(&self) -> Option<&str> {
        match self {
            OperationFailure::Client(ClientError::Server(err)) => Some(&err.message),
            OperationFailure::Client(ClientError::ConnectionClosed(err)) => Some(&err.message),
            OperationFailure::Client(ClientError::Timeout(message)) => Some(message),
            OperationFailure::Client(ClientError::UnknownAction(message)) => Some(message),
            OperationFailure::Client(ClientError::NotImplemented(message)) => Some(message),
            OperationFailure::Client(ClientError::CredentialStore(message)) => Some(message),
            OperationFailure::Client(ClientError::PostRegistration { message, .. }) => {
                Some(message)
            }
            OperationFailure::Policy(message) | OperationFailure::InvalidIntent(message) => {
                Some(message)
            }
        }
    }

    /// Reconstruct a JSON error envelope from structured SpaceMolt fields.
    pub fn structured_error_payload(&self) -> Option<Value> {
        let OperationFailure::Client(ClientError::Server(error)) = self else {
            return None;
        };
        Some(serde_json::json!({
            "error": {
                "code": error.code,
                "message": error.message,
                "details": error.details,
            }
        }))
    }

    /// Server-provided retry hint, when the upstream error payload includes
    /// `retry_after` or renders it as text.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            OperationFailure::Client(ClientError::Server(err)) => err.retry_after(),
            OperationFailure::Client(ClientError::ConnectionClosed(err)) => {
                retry_after_ms_from_close(err).map(Duration::from_millis)
            }
            OperationFailure::Client(
                ClientError::Timeout(_)
                | ClientError::UnknownAction(_)
                | ClientError::NotImplemented(_)
                | ClientError::CredentialStore(_)
                | ClientError::PostRegistration { .. },
            )
            | OperationFailure::InvalidIntent(_)
            | OperationFailure::Policy(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::orchestration::command_map::{
        args_to_generated_payload, craft_args_to_payload, resolve_command,
    };
    use prayer_actions::ActionArg;

    use super::*;

    #[test]
    fn operation_failure_classifies_transient_client_failures() {
        assert!(OperationFailure::Client(ClientError::Timeout("query".to_string())).is_transient());
        let mut rate_limit = spacemolt_lib_rs::SpacemoltError::new("rate_limited", "slow down");
        rate_limit.details = Some(serde_json::json!({ "retry_after": 3 }));
        assert!(OperationFailure::Client(ClientError::Server(rate_limit)).is_transient());
        let equipment = spacemolt_lib_rs::SpacemoltError::new("no_equipment", "missing equipment");
        assert!(!OperationFailure::Client(ClientError::Server(equipment)).is_transient());
        assert!(!OperationFailure::InvalidIntent("mine".to_string()).is_transient());
        assert!(!OperationFailure::Policy("no route".to_string()).is_transient());
    }

    #[test]
    fn client_error_supplies_retry_after() {
        let server = spacemolt_lib_rs::SpacemoltError::new(
            "rate_limited",
            "Too many requests. Try again in 27 seconds.",
        );
        let err = OperationFailure::Client(ClientError::Server(server));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(27)));
    }

    fn object_map(value: &Value) -> &serde_json::Map<String, Value> {
        value.as_object().expect("expected object")
    }

    #[test]
    fn args_to_payload_uses_named_command_schema() {
        let action = resolve_command("accept_mission").expect("action");
        let payload = args_to_generated_payload(
            "accept_mission",
            &[ActionArg::MissionId("m_1".to_string())],
            action,
        )
        .expect("payload");
        let map = object_map(&payload);
        assert_eq!(
            map.get("mission_id"),
            Some(&Value::String("m_1".to_string()))
        );
    }

    #[test]
    fn args_to_payload_encodes_integers_as_numbers() {
        let action = resolve_command("list_ship_for_sale").expect("action");
        let payload = args_to_generated_payload(
            "list_ship_for_sale",
            &[
                ActionArg::ShipId("ship_1".to_string()),
                ActionArg::Integer(1200),
            ],
            action,
        )
        .expect("payload");
        let map = object_map(&payload);
        assert_eq!(
            map.get("ship_id"),
            Some(&Value::String("ship_1".to_string()))
        );
        assert_eq!(
            map.get("price"),
            Some(&Value::Number(serde_json::Number::from(1200)))
        );
    }

    #[test]
    fn craft_args_to_payload_defaults_delivery_to_storage() {
        let payload = craft_args_to_payload(&[
            ActionArg::RecipeId("iron_ingot".to_string()),
            ActionArg::Integer(10),
        ])
        .expect("payload");
        let map = object_map(&payload);

        assert_eq!(
            map.get("recipe_id"),
            Some(&Value::String("iron_ingot".to_string()))
        );
        assert_eq!(
            map.get("quantity"),
            Some(&Value::Number(serde_json::Number::from(10)))
        );
        assert_eq!(
            map.get("deliver_to"),
            Some(&Value::String("storage".to_string()))
        );
    }

    #[test]
    fn craft_args_to_payload_maps_v2_routing_fields() {
        let payload = craft_args_to_payload(&[
            ActionArg::RecipeId("steel_plate".to_string()),
            ActionArg::Integer(6),
            ActionArg::Any("source=faction".to_string()),
            ActionArg::Any("deliver_to=storage".to_string()),
            ActionArg::Any("facility_id=facility_123".to_string()),
            ActionArg::Any("preset=fast".to_string()),
        ])
        .expect("payload");
        let map = object_map(&payload);

        assert_eq!(
            map.get("source"),
            Some(&Value::String("faction".to_string()))
        );
        assert_eq!(
            map.get("deliver_to"),
            Some(&Value::String("storage".to_string()))
        );
        assert_eq!(
            map.get("facility_id"),
            Some(&Value::String("facility_123".to_string()))
        );
        assert_eq!(map.get("preset"), Some(&Value::String("fast".to_string())));
    }

    #[test]
    fn generated_action_uses_csharp_api_name_for_survey() {
        let spec = resolve_command("survey").expect("spec");
        assert_eq!(spec.action, "survey_system");
    }

    #[test]
    fn generated_action_maps_buy_ship_to_buy_listed_ship() {
        let spec = resolve_command("buy_ship").expect("spec");
        assert_eq!(spec.action, "buy_listed_ship");
    }

    #[test]
    fn passenger_commands_use_v2_payload_keys() {
        let load = resolve_command("load_passenger").expect("load spec");
        assert_eq!(load.action, "load_passenger");
        let load_payload = args_to_generated_payload(
            "load_passenger",
            &[ActionArg::PoiId("sol_central".to_string())],
            load,
        )
        .expect("load payload");
        assert_eq!(
            object_map(&load_payload).get("destination"),
            Some(&Value::String("sol_central".to_string()))
        );

        let unload = resolve_command("unload_passenger").expect("unload spec");
        assert_eq!(unload.action, "unload_passenger");
        let unload_payload = args_to_generated_payload(
            "unload_passenger",
            &[ActionArg::Any("all".to_string())],
            unload,
        )
        .expect("unload payload");
        assert_eq!(
            object_map(&unload_payload).get("id"),
            Some(&Value::String("all".to_string()))
        );
    }

    #[test]
    fn generated_action_rejects_high_level_go() {
        let err = resolve_command("go").expect_err("expected unsupported");
        assert!(err.to_string().contains("unsupported command"));
    }

    #[test]
    fn generated_action_supports_remote_refuel() {
        let action = resolve_command("refuel").expect("remote refuel endpoint");
        assert_eq!(action.key, "spacemolt/refuel");
    }
}
