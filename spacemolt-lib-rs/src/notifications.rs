//! Generated SpaceMolt notification metadata and payload DTO exports.

pub use crate::schema::*;

/// One generated server push notification payload mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationDef {
    /// Server push `msg_type` / WebSocket frame type.
    pub msg_type: &'static str,
    /// Rust DTO type generated from `Notification_<msg_type>`.
    pub payload_type: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/notifications.gen.rs"));

/// Find a generated notification payload definition by push type.
pub fn find_notification(msg_type: &str) -> Option<&'static NotificationDef> {
    NOTIFICATIONS
        .iter()
        .find(|notification| notification.msg_type == msg_type)
}

/// Returns true when the spec publishes a schema for this push type.
pub fn is_typed_notification(msg_type: &str) -> bool {
    find_notification(msg_type).is_some()
}
