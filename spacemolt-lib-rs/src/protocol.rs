//! WebSocket v2 frame envelopes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Inbound frame: client -> server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboundFrame {
    /// OpenAPI tool name.
    pub tool: String,
    /// OpenAPI action name.
    pub action: String,
    /// JSON request payload, omitted when the action takes no body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    /// Opaque correlation token echoed by result/outcome frames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// The synchronous query or mutation-ack response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub payload: ResultPayload,
}

/// Payload of a `result` frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultPayload {
    /// Human-rendered result text or object.
    pub result: Value,
    /// Programmatic response payload, when present.
    #[serde(rename = "structuredContent", skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

/// Mutation-ack structured content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAck {
    pub pending: bool,
    pub command: String,
    pub message: String,
}

/// Alias matching the upstream TypeScript library's public name.
pub type MutationAck = PendingAck;

/// Outcome push for a queued mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResultFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub payload: ActionResultPayload,
}

/// Payload of an `action_result` frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResultPayload {
    pub command: String,
    pub tick: u64,
    pub result: StateDelta,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_docked: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_undocked: bool,
}

/// Failure outcome for a queued mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionErrorFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub payload: ActionErrorPayload,
}

/// Payload of an `action_error` frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionErrorPayload {
    pub command: String,
    pub tick: u64,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// Generic error frame emitted by the framing layer or a command handler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub payload: ErrorPayload,
}

/// Payload of an `error` frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_command: Option<String>,
}

/// Unsolicited frame sent immediately after socket upgrade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WelcomeFrame {
    pub payload: WelcomePayload,
}

/// Payload of a `welcome` frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WelcomePayload {
    pub version: String,
    pub release_date: String,
    pub release_notes: Vec<String>,
    pub tick_rate: u64,
    pub current_tick: u64,
    pub server_time: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motd: Option<String>,
    pub game_info: String,
    pub website: String,
    pub help_text: String,
    pub terms: String,
}

/// Auth success frame carrying the full initial session state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggedInFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub payload: Value,
}

/// Auth success frame after `register`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub payload: RegisteredPayload,
}

/// Payload of a `registered` frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredPayload {
    pub password: String,
    pub player_id: String,
}

/// Any inbound frame as parsed off the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawFrame {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

/// Typed outbound frame variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutboundFrame {
    #[serde(rename = "result")]
    Result(ResultFrame),
    #[serde(rename = "action_result")]
    ActionResult(ActionResultFrame),
    #[serde(rename = "action_error")]
    ActionError(ActionErrorFrame),
    #[serde(rename = "error")]
    Error(ErrorFrame),
    #[serde(rename = "welcome")]
    Welcome(WelcomeFrame),
    #[serde(rename = "logged_in")]
    LoggedIn(LoggedInFrame),
    #[serde(rename = "registered")]
    Registered(RegisteredFrame),
    /// Server push frames whose payload schema is not generated yet.
    #[serde(untagged)]
    Notification(RawFrame),
}

/// Cacheable V2 game-state sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateSection {
    Player,
    Ship,
    Modules,
    Cargo,
    Location,
    Missions,
    Queue,
    Skills,
}

impl StateSection {
    /// All cacheable state sections in the server's stable order.
    pub const ALL: [Self; 8] = [
        Self::Player,
        Self::Ship,
        Self::Modules,
        Self::Cargo,
        Self::Location,
        Self::Missions,
        Self::Queue,
        Self::Skills,
    ];

    /// JSON field name for this section.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Ship => "ship",
            Self::Modules => "modules",
            Self::Cargo => "cargo",
            Self::Location => "location",
            Self::Missions => "missions",
            Self::Queue => "queue",
            Self::Skills => "skills",
        }
    }
}

/// A full or partial V2 game-state object.
pub type GameState = Value;

/// A V2 game-state delta carried on `action_result`.
pub type StateDelta = Value;

/// Resolved value of a synchronous query command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub result: Value,
    #[serde(rename = "structuredContent", skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

/// Resolved value of a two-phase mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationResult {
    pub command: String,
    pub tick: u64,
    pub delta: StateDelta,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_docked: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_undocked: bool,
}
