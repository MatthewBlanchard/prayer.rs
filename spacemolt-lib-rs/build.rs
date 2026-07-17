use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use heck::ToUpperCamelCase;
use serde_json::Value;
use typify::{TypeSpace, TypeSpaceSettings};

#[derive(Debug)]
struct ParamDef {
    name: String,
    ty: String,
    required: bool,
    description: Option<String>,
    enum_values: Vec<String>,
    positional: Option<i64>,
}

#[derive(Debug)]
struct ActionDef {
    key: String,
    tool: String,
    action: String,
    kind: &'static str,
    summary: Option<String>,
    params: Vec<ParamDef>,
    response_type: Option<String>,
    details_type: Option<String>,
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=../spacemolt-openapi.json");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let spec_path = manifest_dir.join("..").join("spacemolt-openapi.json");
    let spec_text = fs::read_to_string(&spec_path)
        .with_context(|| format!("reading {}", spec_path.display()))?;
    let spec: Value = serde_json::from_str(&spec_text).context("parsing spacemolt-openapi.json")?;

    let version = spec
        .pointer("/info/x-gameserver-version")
        .or_else(|| spec.pointer("/info/version"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let actions = extract_actions(&spec)?;

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    fs::write(
        out_dir.join("actions.gen.rs"),
        render_actions(version, &actions),
    )?;
    fs::write(out_dir.join("commands.gen.rs"), render_commands(&actions))?;
    fs::write(out_dir.join("types.gen.rs"), render_types(&spec)?)?;
    fs::write(
        out_dir.join("notifications.gen.rs"),
        render_notifications(&spec)?,
    )?;
    Ok(())
}

fn extract_actions(spec: &Value) -> Result<Vec<ActionDef>> {
    let paths = spec
        .get("paths")
        .and_then(Value::as_object)
        .context("OpenAPI spec missing paths object")?;
    let mut actions = Vec::new();

    for (path, methods) in paths {
        let Some((tool, action)) = parse_v2_action_path(path) else {
            continue;
        };
        if action == "help" {
            continue;
        }
        let Some(op) = methods.get("post") else {
            continue;
        };
        let kind = if op
            .get("x-is-mutation")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            "Mutation"
        } else {
            "Query"
        };
        let params = extract_params(op);
        let response_type = if kind == "Query" {
            extract_response_type(op)
        } else {
            None
        };
        let details_type = if kind == "Mutation" {
            extract_details_type(op)
        } else {
            None
        };
        actions.push(ActionDef {
            key: format!("{tool}/{action}"),
            tool: tool.to_string(),
            action: action.to_string(),
            kind,
            summary: op
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string),
            params,
            response_type,
            details_type,
        });
    }

    actions.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(actions)
}

fn extract_response_type(op: &Value) -> Option<String> {
    let schema = op.pointer("/responses/200/content/application~1json/schema")?;
    for part in schema_parts(schema) {
        if let Some(name) = part
            .pointer("/properties/structuredContent/$ref")
            .and_then(Value::as_str)
            .and_then(ref_name)
        {
            return Some(name.to_string());
        }
    }
    None
}

fn extract_details_type(op: &Value) -> Option<String> {
    let schema = op.pointer("/responses/200/content/application~1json/schema")?;
    for part in schema_parts(schema) {
        let Some(structured_content) = part
            .get("properties")
            .and_then(|properties| properties.get("structuredContent"))
        else {
            continue;
        };
        for sc_part in schema_parts(structured_content) {
            if let Some(name) = sc_part
                .pointer("/properties/details/$ref")
                .and_then(Value::as_str)
                .and_then(ref_name)
            {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn schema_parts(schema: &Value) -> Vec<&Value> {
    schema
        .get("allOf")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_else(|| vec![schema])
}

fn ref_name(reference: &str) -> Option<&str> {
    reference.split('/').next_back()
}

fn parse_v2_action_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/api/v2/")?;
    let mut parts = rest.split('/');
    let tool = parts.next()?;
    let action = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((tool, action))
}

fn extract_params(op: &Value) -> Vec<ParamDef> {
    let schema = op.pointer("/requestBody/content/application~1json/schema");
    let required = schema
        .and_then(|v| v.get("required"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let required = required
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();

    let mut params = schema
        .and_then(|v| v.get("properties"))
        .and_then(Value::as_object)
        .map(|props| {
            props
                .iter()
                .map(|(name, prop)| ParamDef {
                    name: name.clone(),
                    ty: schema_type(prop),
                    required: required.contains(name),
                    description: prop
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    enum_values: prop
                        .get("enum")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default(),
                    positional: prop.get("x-positional-index").and_then(Value::as_i64),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    params.sort_by(|a, b| {
        a.positional
            .cmp(&b.positional)
            .then_with(|| a.name.cmp(&b.name))
    });
    params
}

fn schema_type(prop: &Value) -> String {
    if let Some(items) = prop.get("enum").and_then(Value::as_array) {
        if !items.is_empty() {
            return items
                .iter()
                .filter_map(Value::as_str)
                .map(|v| format!("{v:?}"))
                .collect::<Vec<_>>()
                .join(" | ");
        }
    }

    match prop.get("type").and_then(Value::as_str) {
        Some("integer") => "integer".to_string(),
        Some("number") => "number".to_string(),
        Some("boolean") => "boolean".to_string(),
        Some("array") => {
            let inner = prop
                .get("items")
                .map(schema_type)
                .unwrap_or_else(|| "unknown".to_string());
            if inner.contains(" | ") && !inner.starts_with("{ ") {
                format!("({inner})[]")
            } else {
                format!("{inner}[]")
            }
        }
        Some("object") => {
            let Some(props) = prop.get("properties").and_then(Value::as_object) else {
                return "object".to_string();
            };
            let required = prop
                .get("required")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            let mut fields = BTreeMap::new();
            for (name, value) in props {
                fields.insert(
                    name.as_str(),
                    format!(
                        "{}{}: {}",
                        name,
                        if required.contains(name) { "" } else { "?" },
                        schema_type(value)
                    ),
                );
            }
            format!(
                "{{ {} }}",
                fields.values().cloned().collect::<Vec<_>>().join("; ")
            )
        }
        Some(other) => other.to_string(),
        None => "string".to_string(),
    }
}

fn render_actions(version: &str, actions: &[ActionDef]) -> String {
    let mut out = String::new();
    out.push_str("// AUTO-GENERATED by spacemolt-lib-rs/build.rs from spacemolt-openapi.json. DO NOT EDIT.\n");
    out.push_str(&format!(
        "pub const GENERATED_SPEC_VERSION: &str = {:?};\n\n",
        version
    ));

    for (idx, action) in actions.iter().enumerate() {
        out.push_str(&format!("const PARAMS_{idx}: &[ParamDef] = &[\n"));
        for param in &action.params {
            out.push_str("    ParamDef {\n");
            out.push_str(&format!("        name: {:?},\n", param.name));
            out.push_str(&format!("        ty: {:?},\n", param.ty));
            out.push_str(&format!("        required: {},\n", param.required));
            out.push_str(&format!(
                "        description: {},\n",
                option_str(param.description.as_deref())
            ));
            out.push_str("        enum_values: &[\n");
            for value in &param.enum_values {
                out.push_str(&format!("            {:?},\n", value));
            }
            out.push_str("        ],\n");
            out.push_str(&format!(
                "        positional: {},\n",
                option_i64(param.positional)
            ));
            out.push_str("    },\n");
        }
        out.push_str("];\n\n");
    }

    out.push_str("pub const ACTIONS: &[ActionDef] = &[\n");
    for (idx, action) in actions.iter().enumerate() {
        out.push_str("    ActionDef {\n");
        out.push_str(&format!("        key: {:?},\n", action.key));
        out.push_str(&format!("        tool: {:?},\n", action.tool));
        out.push_str(&format!("        action: {:?},\n", action.action));
        out.push_str(&format!(
            "        path: {:?},\n",
            format!("/api/v2/{}", action.key)
        ));
        out.push_str(&format!("        kind: ActionKind::{},\n", action.kind));
        out.push_str(&format!(
            "        summary: {},\n",
            option_str(action.summary.as_deref())
        ));
        out.push_str(&format!("        params: PARAMS_{idx},\n"));
        out.push_str(&format!(
            "        response_type: {},\n",
            option_str(action.response_type.as_deref())
        ));
        out.push_str(&format!(
            "        details_type: {},\n",
            option_str(action.details_type.as_deref())
        ));
        out.push_str("    },\n");
    }
    out.push_str("];\n");
    out
}

fn render_commands(actions: &[ActionDef]) -> String {
    let mut out = String::new();
    out.push_str("// AUTO-GENERATED by spacemolt-lib-rs/build.rs from spacemolt-openapi.json. DO NOT EDIT.\n\n");

    for action in actions {
        if action.params.is_empty() {
            continue;
        }
        out.push_str("#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]\n");
        out.push_str(&format!(
            "pub struct {}Params {{\n",
            command_type_name(&action.tool, &action.action)
        ));
        for param in &action.params {
            if let Some(description) = param.description.as_deref() {
                out.push_str(&format!(
                    "    #[doc = {:?}]\n",
                    description.replace("*/", "* /")
                ));
            }
            let field = rust_field_name(&param.name);
            if field != param.name {
                out.push_str(&format!("    #[serde(rename = {:?})]\n", param.name));
            }
            if !param.required {
                out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
            }
            out.push_str(&format!(
                "    pub {}: {},\n",
                field,
                rust_param_type(param, !param.required)
            ));
        }
        out.push_str("}\n\n");
    }

    let mut tools = actions
        .iter()
        .map(|action| action.tool.as_str())
        .collect::<Vec<_>>();
    tools.sort();
    tools.dedup();

    out.push_str("#[derive(Debug, Clone)]\n");
    out.push_str("pub struct Commands {\n");
    out.push_str("    account: crate::account::Account,\n");
    out.push_str("}\n\n");
    out.push_str("impl Commands {\n");
    out.push_str("    pub(crate) fn new(account: crate::account::Account) -> Self {\n");
    out.push_str("        Self { account }\n");
    out.push_str("    }\n\n");
    for tool in &tools {
        out.push_str(&format!(
            "    pub fn {}(&self) -> {}Commands {{\n",
            rust_method_name(tool),
            rust_type_name(tool)
        ));
        out.push_str(&format!(
            "        {}Commands {{ account: self.account.clone() }}\n",
            rust_type_name(tool)
        ));
        out.push_str("    }\n\n");
    }
    out.push_str("}\n\n");

    for tool in tools {
        let mut tool_actions = actions
            .iter()
            .filter(|action| action.tool == tool)
            .collect::<Vec<_>>();
        tool_actions.sort_by(|a, b| a.action.cmp(&b.action));
        let tool_type = rust_type_name(tool);
        out.push_str("#[derive(Debug, Clone)]\n");
        out.push_str(&format!("pub struct {tool_type}Commands {{\n"));
        out.push_str("    account: crate::account::Account,\n");
        out.push_str("}\n\n");
        out.push_str(&format!("impl {tool_type}Commands {{\n"));
        for action in tool_actions {
            if let Some(summary) = action.summary.as_deref() {
                out.push_str(&format!(
                    "    #[doc = {:?}]\n",
                    summary.replace("*/", "* /")
                ));
            }
            render_command_method(&mut out, action);
        }
        out.push_str("}\n\n");
    }

    out
}

fn render_types(spec: &Value) -> Result<String> {
    let schema = typify_schema(spec)?;
    let root: schemars::schema::RootSchema =
        serde_json::from_value(schema).context("converting OpenAPI schemas to JSON Schema")?;
    let mut settings = TypeSpaceSettings::default();
    settings.with_struct_builder(false);
    // Canonical generated facts are stored directly in Prayer snapshots and
    // knowledge models, whose change detection relies on structural equality.
    settings.with_derive("PartialEq".to_string());
    settings.with_derive("schemars::JsonSchema".to_string());
    let mut type_space = TypeSpace::new(&settings);
    type_space
        .add_root_schema(root)
        .context("generating Rust types from OpenAPI schemas")?;
    let tokens = type_space.to_stream();
    let file = syn::parse2(tokens).context("parsing generated Typify tokens")?;
    let mut out =
        "// AUTO-GENERATED by spacemolt-lib-rs/build.rs from spacemolt-openapi.json. DO NOT EDIT.\n\n"
            .to_string();
    out.push_str(&prettyplease::unparse(&file));
    Ok(out)
}

fn render_notifications(spec: &Value) -> Result<String> {
    let typed = extract_notifications(spec)?;
    let mut out = String::new();
    out.push_str("// AUTO-GENERATED by spacemolt-lib-rs/build.rs from spacemolt-openapi.json. DO NOT EDIT.\n\n");

    out.push_str("pub const TYPED_NOTIFICATION_TYPES: &[&str] = &[\n");
    for notification in &typed {
        out.push_str(&format!("    {:?},\n", notification.msg_type));
    }
    out.push_str("];\n\n");

    out.push_str("pub const NOTIFICATIONS: &[NotificationDef] = &[\n");
    for notification in typed {
        out.push_str("    NotificationDef {\n");
        out.push_str(&format!("        msg_type: {:?},\n", notification.msg_type));
        out.push_str(&format!(
            "        payload_type: {:?},\n",
            notification.payload_type
        ));
        out.push_str("    },\n");
    }
    out.push_str("];\n");
    Ok(out)
}

#[derive(Debug)]
struct NotificationType {
    msg_type: String,
    payload_type: String,
}

fn extract_notifications(spec: &Value) -> Result<Vec<NotificationType>> {
    let schemas = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .context("OpenAPI spec missing components.schemas object")?;
    let mut typed = schemas
        .keys()
        .filter_map(|name| {
            name.strip_prefix("Notification_")
                .map(|msg_type| NotificationType {
                    msg_type: msg_type.to_string(),
                    payload_type: rust_type_name(name),
                })
        })
        .collect::<Vec<_>>();
    typed.sort_by(|a, b| a.msg_type.cmp(&b.msg_type));
    Ok(typed)
}

fn typify_schema(spec: &Value) -> Result<Value> {
    let schemas = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .context("OpenAPI spec missing components.schemas object")?;
    let mut definitions = serde_json::Map::new();
    for (name, schema) in schemas {
        let mut schema = schema.clone();
        rewrite_schema_refs(&mut schema);
        if let Value::Object(map) = &mut schema {
            map.entry("title")
                .or_insert_with(|| Value::String(rust_type_name(name)));
        }
        definitions.insert(rust_type_name(name), schema);
    }
    Ok(serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "definitions": definitions
    }))
}

fn rewrite_schema_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                    map.insert(
                        "$ref".to_string(),
                        Value::String(format!("#/definitions/{}", rust_type_name(name))),
                    );
                }
            }
            for value in map.values_mut() {
                rewrite_schema_refs(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_schema_refs(item);
            }
        }
        _ => {}
    }
}

fn render_command_method(out: &mut String, action: &ActionDef) {
    let method = rust_method_name(&action.action);
    let params_type = format!("{}Params", command_type_name(&action.tool, &action.action));
    let response_type = action
        .response_type
        .as_deref()
        .or(action.details_type.as_deref())
        .map(rust_type_name)
        .unwrap_or_else(|| "serde_json::Value".to_string());
    let return_type = if action.kind == "Mutation" {
        format!("TypedMutationResult<{response_type}>")
    } else {
        format!("TypedQueryResult<{response_type}>")
    };
    let optional_params =
        !action.params.is_empty() && action.params.iter().all(|param| !param.required);
    if action.params.is_empty() {
        out.push_str(&format!(
            "    pub fn {method}(&self) -> CommandFuture<{return_type}> {{\n"
        ));
        render_dispatch(out, action, "None");
    } else if optional_params {
        out.push_str(&format!(
            "    pub fn {method}(&self, params: Option<{params_type}>) -> CommandFuture<{return_type}> {{\n"
        ));
        out.push_str("        let payload = match optional_payload_from_params(params) {\n");
        out.push_str("            Ok(payload) => payload,\n");
        out.push_str("            Err(err) => return ready_err(err),\n");
        out.push_str("        };\n");
        render_dispatch(out, action, "payload");
    } else {
        out.push_str(&format!(
            "    pub fn {method}(&self, params: {params_type}) -> CommandFuture<{return_type}> {{\n"
        ));
        out.push_str("        let payload = match payload_from_params(params) {\n");
        out.push_str("            Ok(payload) => payload,\n");
        out.push_str("            Err(err) => return ready_err(err),\n");
        out.push_str("        };\n");
        render_dispatch(out, action, "payload");
    }
    out.push_str("    }\n\n");
}

fn render_dispatch(out: &mut String, action: &ActionDef, payload: &str) {
    if action.kind == "Mutation" {
        out.push_str(&format!(
            "        mutate_command(&self.account, {:?}, {:?}, {payload})\n",
            action.tool, action.action
        ));
    } else {
        out.push_str(&format!(
            "        query_command(&self.account, {:?}, {:?}, {payload})\n",
            action.tool, action.action
        ));
    }
}

fn rust_param_type(param: &ParamDef, optional: bool) -> String {
    let base = rust_schema_type_name(&param.ty);
    if optional {
        format!("Option<{base}>")
    } else {
        base
    }
}

fn rust_schema_type_name(schema_type: &str) -> String {
    if schema_type.contains(" | ") {
        return "String".to_string();
    }
    if let Some(inner) = schema_type.strip_suffix("[]") {
        return format!(
            "Vec<{}>",
            rust_schema_type_name(inner.trim_matches(|c| c == '(' || c == ')'))
        );
    }
    if schema_type.starts_with("{ ") || schema_type == "object" {
        return "serde_json::Value".to_string();
    }
    match schema_type {
        "integer" => "i64".to_string(),
        "number" => "f64".to_string(),
        "boolean" => "bool".to_string(),
        "string" => "String".to_string(),
        _ => "String".to_string(),
    }
}

fn command_type_name(tool: &str, action: &str) -> String {
    rust_type_name(&format!("{tool}_{action}"))
}

fn rust_type_name(name: &str) -> String {
    let out = name.to_upper_camel_case();
    if out.is_empty() {
        "GeneratedType".to_string()
    } else {
        out
    }
}

fn rust_method_name(name: &str) -> String {
    let field = rust_field_name(name);
    if field.starts_with("r#") {
        field
    } else {
        field.replace('-', "_")
    }
}

fn rust_field_name(name: &str) -> String {
    let mut out = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if out
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(true)
    {
        out.insert(0, '_');
    }
    if is_rust_keyword(&out) {
        format!("r#{out}")
    } else {
        out
    }
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    )
}

fn option_str(value: Option<&str>) -> String {
    value
        .map(|value| format!("Some({value:?})"))
        .unwrap_or_else(|| "None".to_string())
}

fn option_i64(value: Option<i64>) -> String {
    value
        .map(|value| format!("Some({value})"))
        .unwrap_or_else(|| "None".to_string())
}
