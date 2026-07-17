//! HTTP-only request and response wrappers.

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteBatchRequest {
    pub routes: Vec<prayer_sdk::RouteQuery>,
    #[serde(default = "default_safe_route")]
    pub safe: bool,
}

fn default_safe_route() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteBatchResponse {
    pub routes: Vec<Option<prayer_sdk::RouteSelection>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct V1MetaResponse {
    pub api_version: &'static str,
    pub server_version: &'static str,
    pub action_schema_version: u32,
    pub capabilities: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct V1BotSummary {
    pub bot_id: String,
    pub name: Option<String>,
    pub connection: V1BotConnectionState,
    pub state_version: u64,
    pub observed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBotRequest {
    pub username: String,
    pub empire: String,
    pub registration_code: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBotResponse {
    pub bot: V1BotSummary,
    pub player_id: String,
    pub password: String,
}

#[derive(Debug, Clone, Copy, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum V1BotConnectionState {
    Connected,
    Disconnected,
}

impl From<prayer_state::BotConnectionState> for V1BotConnectionState {
    fn from(value: prayer_state::BotConnectionState) -> Self {
        match value {
            prayer_state::BotConnectionState::Connected => Self::Connected,
            prayer_state::BotConnectionState::Disconnected => Self::Disconnected,
        }
    }
}

#[cfg(test)]
mod bot_summary_tests {
    use super::*;

    #[test]
    fn connection_matches_the_lowercase_v1_contract() {
        assert_eq!(
            serde_json::to_string(&V1BotConnectionState::Connected).unwrap(),
            "\"connected\""
        );
        assert_eq!(
            serde_json::to_string(&V1BotConnectionState::Disconnected).unwrap(),
            "\"disconnected\""
        );
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct V1ActionRunRequest {
    pub idempotency_key: Option<String>,
    pub actions: Vec<V1ActionRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct V1ActionRequest(pub prayer_actions::Action);

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct V1ScriptRunRequest {
    pub idempotency_key: Option<String>,
    pub script: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct V1ActionOverrideRequest {
    pub actions: Vec<V1ActionRequest>,
    #[serde(default)]
    pub return_to_origin: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct V1ScriptOverrideRequest {
    pub script: String,
    #[serde(default)]
    pub return_to_origin: bool,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct V1OverrideResponse {
    pub accepted: bool,
}

#[derive(Debug, Clone, Deserialize, Default, schemars::JsonSchema)]
pub struct V1CancelRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct V1RunIdentity {
    pub run_id: String,
    pub bot_id: String,
    pub run_version: u64,
    pub prayerlang: String,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum V1ActionRunResponse {
    Running {
        #[serde(flatten)]
        run: V1RunIdentity,
    },
    Succeeded {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ActionRunOutcome,
    },
    Failed {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ActionRunOutcome,
    },
    Cancelled {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ActionRunOutcome,
    },
    Halted {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ActionRunOutcome,
    },
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum V1ScriptRunResponse {
    Running {
        #[serde(flatten)]
        run: V1RunIdentity,
    },
    Succeeded {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ScriptRunOutcome,
    },
    Failed {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ScriptRunOutcome,
    },
    Cancelled {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ScriptRunOutcome,
    },
    Halted {
        #[serde(flatten)]
        run: V1RunIdentity,
        outcome: prayer_sdk::ScriptRunOutcome,
    },
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct V1ErrorEnvelope {
    pub error: V1ErrorDetail,
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct V1ErrorDetail {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct V1QueueResponse {
    pub scheduler: prayer_sdk::QueueSnapshot,
    pub prayerlang: String,
    pub script_execution: Option<prayer_api_contracts::ScriptExecutionDto>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyRequest {}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MarketMovementTransitionRequest {
    pub reason: String,
}
