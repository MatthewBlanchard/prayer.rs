//! Virtual filesystem projection of EffectiveState.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use serde_json::Value;

/// Response size limits (VFS files themselves are unbounded).
pub const MAX_READ_RETURN_BYTES: usize = 262_144;
pub const MAX_QUERY_RETURN_BYTES: usize = 262_144;
pub const MAX_QUERY_ROWS_DEFAULT: usize = 200;
pub const MAX_QUERY_ROWS_HARD: usize = 1000;
pub const MAX_PIPELINE_STAGES: usize = 8;
pub const MAX_PIPELINE_CHARS: usize = 4096;

#[derive(Debug, Clone)]
pub enum VirtualNode {
    Dir { children: Vec<String> },
    File { mime: &'static str, text: String },
}

#[derive(Debug)]
pub struct VfsCache {
    pub state_version: u64,
    pub nodes: HashMap<String, VirtualNode>,
}

// ── internal builder ──────────────────────────────────────────────────────────

struct VfsBuilder {
    nodes: HashMap<String, VirtualNode>,
}

impl VfsBuilder {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    fn insert_file(&mut self, path: String, mime: &'static str, text: String) {
        self.nodes.insert(path, VirtualNode::File { mime, text });
    }
}

// ── VfsCache impl ─────────────────────────────────────────────────────────────

impl VfsCache {
    pub fn build(state_version: u64, state: &Value) -> Self {
        let mut b = VfsBuilder::new();

        project_canonical_files(&mut b, state);

        let mut nodes = b.nodes;
        for (dir, children) in synthesize_directory_nodes(&nodes) {
            nodes.insert(dir, VirtualNode::Dir { children });
        }

        Self {
            state_version,
            nodes,
        }
    }

    pub fn read_file(&self, path: &str) -> Option<(&str, &str)> {
        match self.nodes.get(path)? {
            VirtualNode::File { mime, text } => Some((mime, text.as_str())),
            VirtualNode::Dir { .. } => None,
        }
    }

    pub fn list_dir(&self, path: &str) -> Option<&[String]> {
        match self.nodes.get(path)? {
            VirtualNode::Dir { children } => Some(children),
            VirtualNode::File { .. } => None,
        }
    }

    pub fn all_file_paths(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().filter_map(|(k, v)| match v {
            VirtualNode::File { .. } => Some(k.as_str()),
            VirtualNode::Dir { .. } => None,
        })
    }
}

// ── pipeline execution ────────────────────────────────────────────────────────

pub type PipelineRow = serde_json::Map<String, Value>;

pub fn run_pipeline(
    vfs: &VfsCache,
    pipeline: &str,
    max_results: usize,
) -> Result<(Vec<PipelineRow>, bool, usize, u64), String> {
    if pipeline.len() > MAX_PIPELINE_CHARS {
        return Err(format!(
            "pipeline is {} chars; maximum is {MAX_PIPELINE_CHARS}",
            pipeline.len()
        ));
    }
    let stages: Vec<&str> = pipeline.split('|').map(str::trim).collect();
    if stages.len() > MAX_PIPELINE_STAGES {
        return Err(format!(
            "pipeline has {} stages; maximum is {MAX_PIPELINE_STAGES}",
            stages.len()
        ));
    }

    let mut paths: Vec<String> = vfs.all_file_paths().map(str::to_owned).collect();
    let mut rows: Option<Vec<PipelineRow>> = None;
    let mut limit: usize = max_results;

    for stage in &stages {
        let (op, rest) = split_op(stage);
        match op {
            "find" => {
                let glob = build_glob(rest)?;
                paths.retain(|p| glob.is_match(p.trim_start_matches('/')));
            }
            "grep" => {
                let pattern = rest.trim();
                if pattern.is_empty() {
                    return Err("grep requires a pattern".into());
                }
                let re = Regex::new(pattern).map_err(|e| format!("invalid grep pattern: {e}"))?;
                if let Some(ref mut row_list) = rows {
                    row_list.retain(|row| row.values().any(|v| re.is_match(&v.to_string())));
                } else {
                    paths.retain(|p| {
                        vfs.nodes
                            .get(p.as_str())
                            .and_then(|n| {
                                if let VirtualNode::File { text, .. } = n {
                                    Some(text)
                                } else {
                                    None
                                }
                            })
                            .map(|text| text.lines().any(|l| re.is_match(l)))
                            .unwrap_or(false)
                    });
                }
            }
            "read" => {
                let mut new_rows: Vec<PipelineRow> = Vec::new();
                for path in &paths {
                    if let Some(VirtualNode::File { text, .. }) = vfs.nodes.get(path.as_str()) {
                        match serde_json::from_str::<Value>(text) {
                            Ok(Value::Array(arr)) => {
                                for item in arr {
                                    if let Value::Object(mut map) = item {
                                        map.insert("_path".into(), Value::String(path.clone()));
                                        new_rows.push(map);
                                    }
                                }
                            }
                            Ok(Value::Object(mut map)) => {
                                map.insert("_path".into(), Value::String(path.clone()));
                                new_rows.push(map);
                            }
                            _ => {
                                // Plain-text file: emit one row per non-empty line.
                                for line in text.lines() {
                                    let line = line.trim();
                                    if line.is_empty() {
                                        continue;
                                    }
                                    let mut row = serde_json::Map::new();
                                    row.insert("_path".into(), Value::String(path.clone()));
                                    row.insert("_line".into(), Value::String(line.into()));
                                    new_rows.push(row);
                                }
                            }
                        }
                    }
                }
                rows = Some(new_rows);
            }
            "project" => {
                let fields: Vec<&str> = rest.split(',').map(str::trim).collect();
                if let Some(ref mut row_list) = rows {
                    for row in row_list.iter_mut() {
                        let keep: Vec<String> = fields
                            .iter()
                            .filter(|f| row.contains_key(**f))
                            .map(|f| f.to_string())
                            .collect();
                        row.retain(|k, _| keep.contains(k));
                    }
                }
            }
            "sort" => {
                let mut parts = rest.splitn(2, ' ');
                let field = parts.next().unwrap_or("").trim().to_string();
                let dir = parts.next().unwrap_or("asc").trim().to_string();
                let desc = dir == "desc";
                if let Some(ref mut row_list) = rows {
                    row_list.sort_by(|a, b| {
                        let av = a.get(&field).map(value_sort_key).unwrap_or(0.0_f64);
                        let bv = b.get(&field).map(value_sort_key).unwrap_or(0.0_f64);
                        if desc {
                            bv.partial_cmp(&av)
                        } else {
                            av.partial_cmp(&bv)
                        }
                        .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }
            "unique" => {
                let field = rest.trim().to_string();
                if let Some(ref mut row_list) = rows {
                    let mut seen = std::collections::HashSet::new();
                    row_list.retain(|row| {
                        let key = row.get(&field).map(|v| v.to_string()).unwrap_or_default();
                        seen.insert(key)
                    });
                }
            }
            "limit" => {
                let n: usize = rest
                    .trim()
                    .parse()
                    .map_err(|_| format!("limit requires an integer, got '{rest}'"))?;
                limit = n.min(limit);
            }
            other => return Err(format!("unknown pipeline stage '{other}'")),
        }
    }

    let final_rows: Vec<PipelineRow> = match rows {
        Some(r) => r,
        None => paths
            .into_iter()
            .map(|p| {
                let mut m = serde_json::Map::new();
                m.insert("path".into(), Value::String(p));
                m
            })
            .collect(),
    };

    let total = final_rows.len();
    let capped = limit.min(MAX_QUERY_ROWS_HARD);
    let truncated = total > capped;
    let result = final_rows.into_iter().take(capped).collect();
    Ok((result, truncated, total, vfs.state_version))
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn split_op(stage: &str) -> (&str, &str) {
    match stage.find(' ') {
        Some(i) => (&stage[..i], &stage[i + 1..]),
        None => (stage, ""),
    }
}

fn build_glob(pattern: &str) -> Result<GlobSet, String> {
    let pattern = pattern.trim().trim_start_matches('/');
    let pattern = if pattern.is_empty() { "**" } else { pattern };
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(pattern).map_err(|e| format!("invalid glob '{pattern}': {e}"))?);
    builder.build().map_err(|e| e.to_string())
}

fn value_sort_key(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn project_canonical_files(b: &mut VfsBuilder, state: &Value) {
    let game = state.get("state").unwrap_or(&Value::Null);

    // top-level state files
    if let Some(memory) = state.get("memory").and_then(Value::as_array) {
        write_json_file(b, "/memory.json", Value::Array(memory.clone()));
    }
    if let Some(lines) = state.get("executionStatusLines").and_then(Value::as_array) {
        write_json_file(b, "/status_lines.json", Value::Array(lines.clone()));
    }
    if let Some(script) = game.get("currentScript").and_then(Value::as_str) {
        b.insert_file("/script.txt".into(), "text/plain", script.to_string());
    }
    if let Some(notifs) = game.get("notifications").and_then(Value::as_array) {
        write_json_file(b, "/notifications.json", Value::Array(notifs.clone()));
    }
    if let Some(chat) = game.get("chatMessages").and_then(Value::as_array) {
        write_json_file(b, "/chat.json", Value::Array(chat.clone()));
    }
    let ship = game.get("ship").unwrap_or(&Value::Null);
    let cargo_used = ship.get("cargoUsed").and_then(Value::as_i64).unwrap_or(0);
    let cargo_capacity = ship
        .get("cargoCapacity")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let cargo_pct = if cargo_capacity > 0 {
        ((cargo_used as f64 / cargo_capacity as f64) * 100.0).round() as i64
    } else {
        0
    };
    write_json_file(
        b,
        "/status.json",
        serde_json::json!({
            "system": game.get("system"),
            "current_poi": game.get("currentPoi"),
            "docked": game.get("docked"),
            "home_base": game.get("homeBase"),
            "nearest_station": game.get("nearestStation"),
            "credits": game.get("credits"),
            "ship_name": ship.get("name"),
            "ship_class": ship.get("classId"),
            "fuel_pct": ship.get("fuelPercent"),
            "cargo_pct": cargo_pct,
            "cargo_used": cargo_used,
            "cargo_capacity": cargo_capacity,
            "active_route": state.get("activeRoute"),
        }),
    );

    // /ship.json
    write_json_file(b, "/ship.json", ship.clone());

    // /missions.json
    write_json_file(
        b,
        "/missions.json",
        {
            let active = game.get("activeMissions").and_then(Value::as_array).cloned();
            let available = game.get("availableMissions").and_then(Value::as_array).cloned();
            serde_json::json!({
                "active_known": active.is_some(),
                "active": active.unwrap_or_default(),
                "available_known": available.is_some(),
                "available": available.unwrap_or_default(),
            })
        },
    );

    // /station.json — only when docked
    if let Some(station) = game.get("station") {
        write_json_file(b, "/station.json", station.clone());
    }

    // /stash/**
    let current_poi_id = game
        .get("currentPoi")
        .and_then(Value::as_object)
        .and_then(|o| {
            o.get("id")
                .and_then(Value::as_str)
                .or_else(|| o.get("poiId").and_then(Value::as_str))
        })
        .unwrap_or("unknown");
    if let Some(storage) = game.get("storageItems").and_then(Value::as_object) {
        for (item_id, stack) in storage {
            let qty = stack
                .get("quantity")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| stack.as_i64().unwrap_or(0));
            write_json_file(
                b,
                &format!(
                    "/stash/pois/{}/items/{}.json",
                    encode_segment(current_poi_id),
                    encode_segment(item_id)
                ),
                serde_json::json!({
                    "poi_id": current_poi_id,
                    "item_id": item_id,
                    "quantity": qty
                }),
            );
        }
    }

    // Galaxy-derived VFS projections.
    if let Some(galaxy) = game.get("galaxy").and_then(Value::as_object) {
        let map = galaxy.get("map").and_then(Value::as_object);
        let systems = map
            .and_then(|m| m.get("systems"))
            .and_then(Value::as_array)
            .cloned();
        let systems_known = systems.is_some();
        let systems = systems.unwrap_or_default();
        let known_pois = map
            .and_then(|m| m.get("knownPois"))
            .and_then(Value::as_array)
            .cloned();
        let known_pois_known = known_pois.is_some();
        let known_pois = known_pois.unwrap_or_default();

        let mut all_system_ids: Vec<String> = Vec::new();
        let mut summary_by_system: BTreeMap<String, serde_json::Map<String, Value>> =
            BTreeMap::new();
        for system in &systems {
            let Some(system_id) = system.get("id").and_then(Value::as_str) else {
                continue;
            };
            all_system_ids.push(system_id.to_string());
            summary_by_system.insert(
                system_id.to_string(),
                serde_json::Map::from_iter([
                    (
                        "system_id".to_string(),
                        Value::String(system_id.to_string()),
                    ),
                    (
                        "x".to_string(),
                        system.get("x").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "y".to_string(),
                        system.get("y").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "neighbors".to_string(),
                        system.get("connections").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "neighbors_known".to_string(),
                        Value::Bool(system.get("connections").is_some()),
                    ),
                ]),
            );
        }

        if let Some(catalog) = galaxy.get("catalog").and_then(Value::as_object) {
            write_catalog_group(
                b,
                "items",
                catalog
                    .get("itemsById")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default(),
            );
            write_catalog_group(
                b,
                "ships",
                catalog
                    .get("shipsById")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default(),
            );
            write_catalog_group(
                b,
                "recipes",
                catalog
                    .get("recipesById")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default(),
            );
        }

        let mut poi_system_lookup: HashMap<String, String> = HashMap::new();
        for poi in &known_pois {
            let poi_id = poi.get("id").and_then(Value::as_str);
            let system_id = poi.get("systemId").and_then(Value::as_str);
            if let (Some(pid), Some(sid)) = (poi_id, system_id) {
                poi_system_lookup.insert(pid.to_string(), sid.to_string());
                if !summary_by_system.contains_key(sid) {
                    all_system_ids.push(sid.to_string());
                    summary_by_system.insert(
                        sid.to_string(),
                        serde_json::Map::from_iter([
                            ("system_id".to_string(), Value::String(sid.to_string())),
                            ("x".to_string(), Value::Null),
                            ("y".to_string(), Value::Null),
                            ("neighbors".to_string(), Value::Null),
                            ("neighbors_known".to_string(), Value::Bool(false)),
                        ]),
                    );
                }
            }
        }
        all_system_ids.sort_unstable();
        all_system_ids.dedup();

        let mut poi_to_resources: HashMap<String, Vec<String>> = HashMap::new();
        let mut pois_by_resource_known = false;
        if let Some(resources) = galaxy.get("resources").and_then(Value::as_object) {
            if let Some(by_resource) = resources.get("poisByResource").and_then(Value::as_object) {
                pois_by_resource_known = true;
                for (resource_id, pois) in by_resource {
                    if let Some(poi_arr) = pois.as_array() {
                        for poi in poi_arr {
                            let Some(poi_id) = poi.as_str() else {
                                continue;
                            };
                            poi_to_resources
                                .entry(poi_id.to_string())
                                .or_default()
                                .push(resource_id.to_string());
                        }
                    }
                }
            }
        }

        let exploration = galaxy.get("exploration").and_then(Value::as_object);
        if let Some(exploration) = exploration {
            for (field, path) in [
                ("exploredSystems", "/exploration/explored_systems.json"),
                ("visitedPois", "/exploration/visited_pois.json"),
                ("surveyedSystems", "/exploration/surveyed_systems.json"),
            ] {
                write_json_file(
                    b,
                    path,
                    exploration
                        .get(field)
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!([])),
                );
            }
        }
        write_json_file(
            b,
            "/exploration/meta.json",
            serde_json::json!({
                "explored_systems_known": exploration
                    .and_then(|e| e.get("exploredSystems"))
                    .and_then(Value::as_array)
                    .is_some(),
                "visited_pois_known": exploration
                    .and_then(|e| e.get("visitedPois"))
                    .and_then(Value::as_array)
                    .is_some(),
                "surveyed_systems_known": exploration
                    .and_then(|e| e.get("surveyedSystems"))
                    .and_then(Value::as_array)
                    .is_some(),
            }),
        );

        let mut dockable_by_system: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut station_by_system: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for poi in &known_pois {
            let poi_id = poi.get("id").and_then(Value::as_str);
            let system_id = poi.get("systemId").and_then(Value::as_str);
            let poi_type = poi.get("type").and_then(Value::as_str);
            let (Some(pid), Some(sid)) = (poi_id, system_id) else {
                continue;
            };

            let poi_obj = serde_json::json!({
                "id": pid,
                "system_id": sid,
                "name": poi.get("name"),
                "type": poi.get("type"),
                "base_id": poi.get("baseId"),
                "base_name": poi.get("baseName"),
                "has_base": poi.get("hasBase"),
                "x": poi.get("x"),
                "y": poi.get("y"),
                "resources_known": pois_by_resource_known,
                "resources": poi_to_resources.get(pid).cloned().unwrap_or_default(),
            });
            dockable_by_system
                .entry(sid.to_string())
                .or_default()
                .push(poi_obj.clone());
            let has_base = poi.get("hasBase").and_then(Value::as_bool).unwrap_or(false);
            if poi_type == Some("station") || has_base {
                station_by_system
                    .entry(sid.to_string())
                    .or_default()
                    .push(poi_obj);
            }
        }

        for sid in &all_system_ids {
            let pois = dockable_by_system.get(sid).cloned().unwrap_or_default();
            let stations = station_by_system.get(sid).cloned().unwrap_or_default();

            if let Some(summary) = summary_by_system.get_mut(sid) {
                summary.insert("systems_known".to_string(), Value::Bool(systems_known));
                summary.insert("pois_known".to_string(), Value::Bool(known_pois_known));
                summary.insert(
                    "pois_count".to_string(),
                    if known_pois_known {
                        serde_json::json!(pois.len())
                    } else {
                        Value::Null
                    },
                );
                summary.insert(
                    "stations_count".to_string(),
                    if known_pois_known {
                        serde_json::json!(stations.len())
                    } else {
                        Value::Null
                    },
                );
            }

            write_json_file(
                b,
                &format!("/systems/{}.json", encode_segment(sid)),
                serde_json::json!({
                    "summary": summary_by_system.get(sid),
                    "systems_known": systems_known,
                    "pois_known": known_pois_known,
                    "pois": pois,
                    "stations": stations,
                }),
            );
        }

        write_json_file(
            b,
            "/systems/meta.json",
            serde_json::json!({
                "systems_known": systems_known,
                "pois_known": known_pois_known,
                "system_count": if systems_known {
                    serde_json::json!(all_system_ids.len())
                } else {
                    Value::Null
                },
            }),
        );
    }
}

fn write_catalog_group(b: &mut VfsBuilder, kind: &str, map: serde_json::Map<String, Value>) {
    for (id, value) in map {
        write_json_file(
            b,
            &format!("/catalog/{kind}/{}.json", encode_segment(&id)),
            value,
        );
    }
}

fn write_json_file(b: &mut VfsBuilder, path: &str, value: Value) {
    b.insert_file(
        path.to_string(),
        "application/json",
        serde_json::to_string_pretty(&value).unwrap_or_default(),
    );
}

fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
            out.push(c);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

fn synthesize_directory_nodes(
    nodes: &HashMap<String, VirtualNode>,
) -> HashMap<String, Vec<String>> {
    let mut dirs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    dirs.entry("/".to_string()).or_default();

    for path in nodes.keys() {
        if path == "/" {
            continue;
        }
        let trimmed = path.trim_start_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let mut parent = "/".to_string();
        for (i, part) in parts.iter().enumerate() {
            dirs.entry(parent.clone())
                .or_default()
                .insert((*part).to_string());
            if i + 1 < parts.len() {
                parent = if parent == "/" {
                    format!("/{}", part)
                } else {
                    format!("{parent}/{}", part)
                };
                dirs.entry(parent.clone()).or_default();
            }
        }
    }

    dirs.into_iter()
        .map(|(dir, children)| (dir, children.into_iter().collect()))
        .collect()
}
