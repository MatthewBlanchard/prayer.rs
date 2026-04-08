//! Session handle helpers for MCP-facing tools/resources.

use serde_json::{Map, Value};

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn extract_handle(obj: &Map<String, Value>) -> Option<String> {
    non_empty_string(obj.get("playerName"))
        .or_else(|| non_empty_string(obj.get("label")))
        .or_else(|| non_empty_string(obj.get("username")))
        .or_else(|| non_empty_string(obj.get("handle")))
}

fn sanitize_session_object(obj: &Map<String, Value>) -> Value {
    let mut out = obj.clone();
    out.remove("id");
    out.remove("label");
    out.remove("handle");
    if let Some(handle) = extract_handle(obj) {
        out.insert("playerName".to_string(), Value::String(handle));
    }
    Value::Object(out)
}

pub fn sanitize_session_entry(raw: &Value) -> Value {
    match raw {
        Value::Object(obj) => sanitize_session_object(obj),
        _ => raw.clone(),
    }
}

pub fn sanitize_sessions(raw: &Value) -> Value {
    match raw {
        Value::Array(arr) => Value::Array(arr.iter().map(sanitize_session_entry).collect()),
        _ => raw.clone(),
    }
}

pub fn strip_session_id_fields(raw: &Value) -> Value {
    match raw {
        Value::Array(arr) => Value::Array(arr.iter().map(strip_session_id_fields).collect()),
        Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            for (k, v) in obj {
                if k == "session_id" || k == "sessionId" {
                    continue;
                }
                out.insert(k.clone(), strip_session_id_fields(v));
            }
            Value::Object(out)
        }
        _ => raw.clone(),
    }
}

pub fn resolve_session_id(raw_sessions: &Value, session_handle: &str) -> Result<String, String> {
    let handle = session_handle.trim();
    if handle.is_empty() {
        return Err("error: session_handle is required (use playerName)".to_string());
    }

    let sessions = raw_sessions
        .as_array()
        .ok_or_else(|| "error: list_sessions returned an invalid payload".to_string())?;

    let mut matches: Vec<String> = Vec::new();
    let mut available_handles: Vec<String> = Vec::new();

    for session in sessions {
        let Some(obj) = session.as_object() else {
            continue;
        };

        if let Some(existing_handle) = extract_handle(obj) {
            available_handles.push(existing_handle.clone());
            if existing_handle == handle {
                if let Some(id) = obj.get("id").and_then(Value::as_str) {
                    matches.push(id.to_string());
                }
            }
        }
    }

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => {
            available_handles.sort_unstable();
            available_handles.dedup();
            if available_handles.is_empty() {
                Err(format!(
                    "error: session handle '{handle}' not found (no playerNames available)"
                ))
            } else {
                Err(format!(
                    "error: session handle '{handle}' not found (available playerNames: {})",
                    available_handles.join(", ")
                ))
            }
        }
        _ => Err(format!(
            "error: session handle '{handle}' is ambiguous ({})",
            matches.len()
        )),
    }
}
