//! DSL reference text served as `prayer://dsl/reference`.
//!
//! Generated at build time from `prayer_runtime::catalog::COMMAND_DOCS` and the
//! static template files `dsl_ref_header.txt` / `dsl_ref_footer.txt`.
//! Adding a command to the catalog without a doc entry is a compile-time build error.

/// Return the full PrayerLang reference as a UTF-8 string.
pub fn dsl_reference_text() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/dsl_reference.txt"))
}

/// Return a compact JSON summary of built-in commands and predicates.
pub fn dsl_reference_json() -> serde_json::Value {
    let commands: serde_json::Value =
        serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/dsl_commands.json")))
            .expect("generated dsl_commands.json is valid JSON");

    serde_json::json!({
        "version": "1.0",
        "syntax": {
            "statement_terminator": ";",
            "comment": "//",
            "control_flow": ["if", "until"],
            "arg_token_pattern": "[A-Za-z0-9_][A-Za-z0-9_-]*",
            "macro_token_pattern": "\\$[A-Za-z_][A-Za-z0-9_-]*",
        },
        "built_in_macros": ["$here", "$home", "$nearest_station"],
        "commands": commands,
        "predicates": {
            "boolean": [],
            "numeric": [
                { "name": "FUEL",    "args": [] },
                { "name": "CREDITS", "args": [] },
                { "name": "CARGO_PCT", "args": [] },
                { "name": "CARGO",   "args": [{"name": "item_id", "type": "item_id"}] },
                { "name": "MINED",   "args": [{"name": "item_id", "type": "item_id"}] },
                { "name": "STASHED", "args": [{"name": "item_id", "type": "item_id"}] },
                { "name": "STASH",   "args": [{"name": "poi_id", "type": "poi_id"}, {"name": "item_id", "type": "item_id"}] },
            ]
        },
        "comparison_operators": [">", ">=", "<", "<=", "==", "!="],
        "skill_param_types": [
            "any", "integer", "item_id", "system_id", "poi_id",
            "go_target", "ship_id", "listing_id", "mission_id",
            "module_id", "recipe_id"
        ],
        "skill_library": {
            "keywords": ["skill", "override", "when", "@disable"],
            "case_sensitive": true
        }
    })
}
