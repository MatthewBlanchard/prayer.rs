//! DSL reference text served as `prayer://dsl/reference`.
//!
//! Derived statically from the LANGUAGE.md documentation.  When the
//! `prayer-api` exposes a catalog metadata endpoint in the future, this
//! can be replaced with a dynamic derivation.

/// Return the full PrayerLang reference as a UTF-8 string.
pub fn dsl_reference_text() -> &'static str {
    include_str!("dsl_reference.txt")
}

/// Return a compact JSON summary of built-in commands and predicates.
pub fn dsl_reference_json() -> serde_json::Value {
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
        "commands": [
            { "name": "halt",                  "args": [] },
            { "name": "mine",                  "args": [{"name": "resource", "optional": true}] },
            { "name": "survey",                "args": [] },
            { "name": "explore",               "args": [] },
            { "name": "go",                    "args": [{"name": "destination", "type": "go_target"}] },
            { "name": "accept_mission",        "args": [{"name": "mission_id", "type": "mission_id"}] },
            { "name": "abandon_mission",       "args": [{"name": "mission_id", "type": "mission_id"}] },
            { "name": "dock",                  "args": [] },
            { "name": "set_home",              "args": [] },
            { "name": "repair",                "args": [] },
            { "name": "refuel",                "args": [] },
            { "name": "self_destruct",         "args": [] },
            { "name": "sell",                  "args": [{"name": "item", "type": "item_id", "optional": true}] },
            { "name": "buy",                   "args": [{"name": "item", "type": "item_id"}, {"name": "quantity", "type": "integer"}] },
            { "name": "cancel_buy",            "args": [{"name": "item", "type": "item_id"}] },
            { "name": "cancel_sell",           "args": [{"name": "item", "type": "item_id"}] },
            { "name": "retrieve",              "args": [{"name": "item", "type": "item_id"}, {"name": "quantity", "type": "integer", "optional": true}] },
            { "name": "stash",                 "args": [{"name": "item", "type": "item_id", "optional": true}] },
            { "name": "switch_ship",           "args": [{"name": "ship", "type": "ship_id"}] },
            { "name": "install_mod",           "args": [{"name": "mod", "type": "module_id"}] },
            { "name": "uninstall_mod",         "args": [{"name": "mod", "type": "module_id"}] },
            { "name": "buy_ship",              "args": [{"name": "ship_class", "type": "ship_id"}] },
            { "name": "buy_listed_ship",       "args": [{"name": "listing", "type": "listing_id"}] },
            { "name": "commission_ship",       "args": [{"name": "ship_class", "type": "ship_id"}] },
            { "name": "sell_ship",             "args": [{"name": "ship", "type": "ship_id"}] },
            { "name": "list_ship_for_sale",    "args": [{"name": "ship", "type": "ship_id"}, {"name": "price", "type": "integer"}] },
            { "name": "wait",                  "args": [{"name": "ticks", "type": "integer", "optional": true}] },
            { "name": "craft",                 "args": [{"name": "recipe_id", "type": "recipe_id"}, {"name": "count", "type": "integer", "optional": true}] },
        ],
        "predicates": {
            "boolean": [
                { "name": "MISSION_COMPLETE", "args": [{"name": "mission_id", "type": "mission_id"}] }
            ],
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
