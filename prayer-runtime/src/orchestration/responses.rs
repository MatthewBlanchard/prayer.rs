//! Explicit compatibility decoder for execution responses that predate the
//! generated command DTO boundary. Supported wire envelopes are:
//!
//! - current execution errors: `{ "error": { "code": ... } }`;
//! - legacy execution errors: `{ "error": "..." }`;
//! - legacy MCP command errors: `{ "result": { "error": "..." } }`;
//! - MCP command outcomes: `{ "result": { "message" | "error" | "status": ... } }`.
//!
//! Crossing-order payloads were emitted with both snake_case and camelCase
//! order ids and may nest those ids under the top-level error details. Keep
//! these aliases here until those persisted execution responses age out.

use serde_json::Value;

use crate::engine::EngineExecutionResult;

use super::RuntimeOperation;

pub(super) fn complete(result: EngineExecutionResult) -> RuntimeOperation {
    RuntimeOperation::Complete { result }
}

pub(super) fn completed_with_message(message: impl Into<String>) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: Some(message.into()),
        completed: true,
        halt_script: false,
    }
}

pub(super) fn halted_with_message(message: impl Into<String>) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: Some(message.into()),
        completed: true,
        halt_script: true,
    }
}

pub(super) fn incomplete_with_message(message: impl Into<String>) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: Some(message.into()),
        completed: false,
        halt_script: false,
    }
}

pub(super) fn completed_with_api_message(value: &Value) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: extract_result_message(value),
        completed: true,
        halt_script: false,
    }
}

pub(super) fn incomplete_with_api_message(value: &Value) -> EngineExecutionResult {
    EngineExecutionResult {
        result_message: extract_result_message(value),
        completed: false,
        halt_script: false,
    }
}

/// Whether the response carries an error payload (top-level or under `result`).
pub(crate) fn has_error_payload(value: &Value) -> bool {
    value.get("error").is_some() || value.get("result").and_then(|v| v.get("error")).is_some()
}

/// Machine-readable error code: `error.code`, a bare `error` string, or
/// `result.error`.
pub(crate) fn error_code(value: &Value) -> Option<String> {
    value
        .get("error")
        .and_then(|e| {
            if let Some(code) = e.get("code").and_then(Value::as_str) {
                Some(code.to_string())
            } else {
                e.as_str().map(ToOwned::to_owned)
            }
        })
        .or_else(|| {
            value
                .get("result")
                .and_then(|r| r.get("error"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

/// Whether a `mine` response reports the location as depleted.
pub(crate) fn is_mine_depleted(value: &Value) -> bool {
    let code = error_code(value).unwrap_or_default().to_ascii_lowercase();
    if code.contains("depleted") {
        return true;
    }
    extract_result_message(value)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("depleted")
}

/// Order ids referenced by a `crossing_order` rejection, anywhere in the
/// error payload.
pub(crate) fn extract_crossing_order_ids(value: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(error) = value.get("error") {
        collect_order_ids(error, &mut ids);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn collect_order_ids(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for key in ["order_id", "orderId"] {
                if let Some(id) = map.get(key).and_then(Value::as_str) {
                    let trimmed = id.trim();
                    if !trimmed.is_empty() {
                        out.push(trimmed.to_string());
                    }
                }
            }
            for nested in map.values() {
                collect_order_ids(nested, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_order_ids(item, out);
            }
        }
        _ => {}
    }
}

/// Pull the human-readable outcome out of an API response: `result.message`,
/// falling back to `result.error`, then `result.status`.
pub(crate) fn extract_result_message(value: &Value) -> Option<String> {
    value
        .get("result")
        .and_then(|v| {
            v.get("message")
                .or_else(|| v.get("error"))
                .or_else(|| v.get("status"))
        })
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decodes_each_supported_error_envelope() {
        assert_eq!(
            error_code(&json!({"error": {"code": "depleted"}})).as_deref(),
            Some("depleted")
        );
        assert_eq!(
            error_code(&json!({"error": "legacy"})).as_deref(),
            Some("legacy")
        );
        assert_eq!(
            error_code(&json!({"result": {"error": "mcp"}})).as_deref(),
            Some("mcp")
        );
        assert_eq!(
            extract_result_message(&json!({"result": {"message": "done"}})).as_deref(),
            Some("done")
        );
        assert_eq!(
            extract_result_message(&json!({"result": {"error": "failed"}})).as_deref(),
            Some("failed")
        );
        assert_eq!(
            extract_result_message(&json!({"result": {"status": "queued"}})).as_deref(),
            Some("queued")
        );
    }

    #[test]
    fn decodes_persisted_crossing_order_aliases() {
        let value = json!({"error": {"details": [{"order_id": "a"}, {"orderId": "b"}]}});
        assert_eq!(extract_crossing_order_ids(&value), vec!["a", "b"]);
    }
}
