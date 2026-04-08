# GameState -> VFS Mapping Specification

## Status
This document currently mixes:
- implemented VFS behavior in `prayer-mcp`, and
- target/canonical mapping we intend to grow into.

Important: the current implementation is a focused subset, not full field-by-field GameState coverage yet.
The implemented surface today is concentrated around:
- `/context.json`, `/status.json`
- `/missions/active.json`, `/missions/available.json`
- `/ship/ship.json`, `/ship/cargo.jsonl`
- `/market/station_market.json`, `/market/orders/*.jsonl`, `/market/deals.jsonl`
- `/storage/items.jsonl`
- `/systems/{id}.json`, `/systems/index.json`
- `/catalog/**`, `/exploration/**`
- `/station/context.json`, `/notifications.jsonl`, `/chat.jsonl`

## Purpose
This document defines a complete, deterministic mapping from `prayer-runtime`'s `GameState` model to a virtual filesystem (VFS) surface exposed by `prayer-mcp`.

Source model:
- `prayer-runtime/src/state.rs`
- `GameState`
- nested: `GalaxyData`, `MarketData`, `MissionData`, `OpenOrderInfo`, `MarketOrderInfo`, `CatalogEntryData`

## Canonical Root

Session-scoped root:
- `prayer://sessions/{id}/fs/`

Canonical internal path root (for tool payloads like `fs_read`):
- `/`

Example full URI:
- `prayer://sessions/{id}/fs/player/credits.json`

## Mapping Rules

1. Target state: every `GameState` field represented at least once in VFS.
   Current state: partial coverage only (see Status above).
2. Scalar fields get a dedicated file under a logical namespace.
3. Complex maps/lists get:
   - a raw aggregate file, and
   - optional split/index files for targeted reads/grep.
4. Paths are lowercase and stable.
5. Dictionary-key-addressable paths are normalized as path-safe segments.
6. Missing optional values are represented as JSON `null`.

## Directory Layout

```text
/
  player/
  location/
  cargo/
  stash/
  missions/
  market/
  ship/
  galaxy/
    catalog/
      items/
      ships/
      recipes/
```

## Complete Field Mapping

### GameState Scalars and Simple Optionals

Primary approach: group related scalars into a few high-signal files.

| Grouped file | Included GameState fields |
|---|---|
| `/status.json` | `system`, `home_base`, `nearest_station`, `current_poi`, `docked`, `credits`, `ship_id`, `fuel_pct`, `cargo_pct`, `cargo_used`, `cargo_capacity`, `active_route` |

Rationale:
- Better ergonomics for agents (fewer reads to build context).
- Lower path noise and smaller directory listings.
- Still grep-friendly because each file remains focused by domain.

### GameState Cargo and Stash

| GameState field | Type | Canonical file | Split/index files |
|---|---|---|---|
| `cargo` | `HashMap<String, i64>` | `/ship/cargo/by_item.json` | `/ship/cargo/items/{item_id}.json`, `/ship/cargo/items.jsonl` |
| `stash` | `HashMap<String, HashMap<String, i64>>` | `/stash/by_poi/by_item.json` | `/stash/pois/{poi_id}/items/{item_id}.json`, `/stash/pois/{poi_id}/items.json`, `/stash/pois.jsonl` |

Recommended `stash` file shapes:
- `/stash/by_poi/by_item.json`
```json
{
  "poi-1": { "iron_ore": 50, "copper_ore": 15 },
  "poi-2": { "fuel_cell": 8 }
}
```
- `/stash/pois/{poi_id}/items.json`
```json
{
  "poiId": "poi-1",
  "items": { "iron_ore": 50, "copper_ore": 15 }
}
```

### GameState Mission Completion and Mission Buckets

| GameState field | Type | Canonical file | Split/index files |
|---|---|---|---|
| `mission_complete` | `HashMap<String, bool>` | `/missions/completion/by_id.json` | `/missions/completion/{mission_id}.json`, `/missions/completed.jsonl`, `/missions/incomplete.jsonl` |
| `missions` | `MissionData` | `/missions/index.json` | `/missions/active.json`, `/missions/available.json` |

`MissionData` expansion:
- `missions.active` -> `/missions/active.json`
- `missions.available` -> `/missions/available.json`

### GameState Ships, Modules, and Orders

| GameState field | Type | Canonical file | Split/index files |
|---|---|---|---|
| `owned_ships` | `Vec<String>` | `/ship/owned_ship_ids.json` | `/ship/owned_ship_ids.jsonl` |
| `installed_modules` | `Vec<String>` | `/ship/installed_modules.json` | `/ship/installed_modules.jsonl` |
| `own_buy_orders` | `Vec<OpenOrderInfo>` | `/market/orders/own_buy.json` | `/market/orders/own_buy/{order_id}.json`, `/market/orders/own_buy.jsonl` |
| `own_sell_orders` | `Vec<OpenOrderInfo>` | `/market/orders/own_sell.json` | `/market/orders/own_sell/{order_id}.json`, `/market/orders/own_sell.jsonl` |

`OpenOrderInfo` object shape files (for both buy/sell order entries):
- `order_id`
- `item_id`
- `price_each`
- `quantity`

## Nested Type Mapping

### GalaxyData -> `/systems/**`, `/catalog/**`, `/exploration/**`

Aggregate root file:
- `/galaxy/state.json` (full `GalaxyData` as JSON)

Field-level mapping:

| GalaxyData field | Type | Canonical file | Split/index files |
|---|---|---|---|
| `systems` | `Vec<String>` | `/systems/index.json` | `/systems/index.jsonl` |
| `item_ids` | `Vec<String>` | `/catalog/item_ids.json` | `/catalog/item_ids.jsonl` |
| `ship_ids` | `Vec<String>` | `/catalog/ship_ids.json` | `/catalog/ship_ids.jsonl` |
| `recipe_ids` | `Vec<String>` | `/catalog/recipe_ids.json` | `/catalog/recipe_ids.jsonl` |
| `item_catalog_entries` | `HashMap<String, CatalogEntryData>` | `/catalog/items/by_id.json` | `/catalog/items/{item_id}.json`, `/catalog/items.jsonl` |
| `ship_catalog_entries` | `HashMap<String, CatalogEntryData>` | `/catalog/ships/by_id.json` | `/catalog/ships/{ship_id}.json`, `/catalog/ships.jsonl` |
| `recipe_catalog_entries` | `HashMap<String, CatalogEntryData>` | `/catalog/recipes/by_id.json` | `/catalog/recipes/{recipe_id}.json`, `/catalog/recipes.jsonl` |
| `catalog_version` | `Option<String>` | `/catalog/version.json` | none |
| `system_connections` | `HashMap<String, Vec<String>>` | folded into per-system summary | `/systems/{system_id}/summary.json` |
| `system_coordinates` | `HashMap<String, (f64, f64)>` | folded into per-system summary | `/systems/{system_id}/summary.json` |
| `pois_by_resource` | `HashMap<String, Vec<String>>` | folded into per-system resources | `/systems/{system_id}/resources.json` |
| `explored_systems` | `HashSet<String>` | `/exploration/explored_systems.json` | `/exploration/explored_systems.jsonl` |
| `visited_pois` | `HashSet<String>` | `/exploration/visited_pois.json` | `/exploration/visited_pois.jsonl` |
| `surveyed_systems` | `HashSet<String>` | `/exploration/surveyed_systems.json` | `/exploration/surveyed_systems.jsonl` |
| `dockable_pois_by_system` | `HashMap<String, Vec<String>>` | folded into per-system POI file | `/systems/{system_id}/pois.json` |
| `station_pois_by_system` | `HashMap<String, Vec<String>>` | folded into per-system station file | `/systems/{system_id}/stations.json` |

### MarketData -> `/market/**`

Aggregate root file:
- `/market/state.json` (full `MarketData`)

Field-level mapping:

| MarketData field | Type | Canonical file | Split/index files |
|---|---|---|---|
| `shipyard_listings` | `Vec<String>` | `/market/shipyard/listing_ids.json` | `/market/shipyard/listing_ids.jsonl` |
| `buy_orders` | `HashMap<String, Vec<MarketOrderInfo>>` | `/market/orders/buy/by_item.json` | `/market/orders/buy/items/{item_id}.json`, `/market/orders/buy.jsonl` |
| `sell_orders` | `HashMap<String, Vec<MarketOrderInfo>>` | `/market/orders/sell/by_item.json` | `/market/orders/sell/items/{item_id}.json`, `/market/orders/sell.jsonl` |

`MarketOrderInfo` entry files contain:
- `price_each`
- `quantity`

### MissionData -> `/missions/**`

Aggregate root file:
- `/missions/index.json` (same content as `/missions/state.json` if both are exposed; prefer `/missions/index.json` as canonical)

Field-level mapping:

| MissionData field | Type | Canonical file | Split/index files |
|---|---|---|---|
| `active` | `Vec<String>` | `/missions/active.json` | `/missions/active.jsonl` |
| `available` | `Vec<String>` | `/missions/available.json` | `/missions/available.jsonl` |

### CatalogEntryData -> `/catalog/**`

Each catalog entry object is mapped as:
- `/catalog/{kind}/{id}.json`

Where:
- `{kind}` is `items`, `ships`, or `recipes`
- file content shape:

```json
{
  "id": "entry-id",
  "raw": { "...": "original SpaceMolt payload" }
}
```

## Synthetic/Derived Files (Non-Source Fields)

These are not direct `GameState` fields but improve discoverability and grep quality:

- `/README.txt`
  - quick orientation and high-signal paths.
- `/index/files.json`
  - complete list of all leaf files.
- `/index/by_prefix/{prefix}.json`
  - optional precomputed path index for large trees.
- `/status.json`
  - `system`, `current_poi`, `home_base`, `nearest_station`, `docked`, ship/economy quick readout.
- `/ship/cargo/by_item.json`
  - `cargo_used`, `cargo_capacity`, `cargo_pct`, top items.
- `/market/index.json`
  - counts of order books and own orders.

## Path Encoding for Dynamic Keys

Dynamic map keys (`item_id`, `poi_id`, `system_id`, etc.) become path segments using safe encoding:

1. Preserve `[a-zA-Z0-9._-]`.
2. Percent-encode all other bytes.
3. Lowercase only path static segments, not ids.

Example:
- `iron/ore` -> `iron%2Fore`

## Null/Empty Semantics

- `Option<T>` absent -> JSON `null` in canonical file.
- Empty collections -> `[]` or `{}` (not omitted).
- Missing map key projections (`/foo/{id}.json`) return not-found while aggregate files (`/foo/by_id.json`) remain present.

## Deterministic Ordering Rules

To maximize stable diffs and reproducible grep results:

1. JSON object keys sorted lexicographically before serialization where practical.
2. `HashSet` values materialized as sorted arrays.
3. `HashMap` split outputs (`*.jsonl`) sorted by key.
4. Directory entries sorted lexicographically.

## VFS Return Size Guardrails

Hard limits apply to emitted VFS tool responses:
- `fs_read`: reject payloads above `max_read_return_bytes`.
- `fs_grep`: reject payloads above `max_grep_return_bytes`.

Behavior on limit breach:
1. Do not truncate-and-return.
2. Return a structured error (`result_too_large`) with:
   - operation
   - estimated bytes
   - configured limit
   - recommendation to narrow path/glob.
3. Emit an error-level log with:
   - `session_id`
   - operation
   - path or glob
   - estimated bytes
   - limit bytes
   - state version (if available)

## Minimal Read Set Recommendations for Agents

Common goals and highest-value files:

1. Navigation context
- `/status.json`
- `/systems/{system_id}/summary.json`

2. Mission execution
- `/missions/active.json`
- `/missions/completion/by_id.json`
- `/status.json`

3. Trading
- `/market/orders/buy/by_item.json`
- `/market/orders/sell/by_item.json`
- `/market/orders/own_buy.json`
- `/market/orders/own_sell.json`

4. Mining/crafting
- `/ship/cargo/by_item.json`
- `/systems/{system_id}/resources.json`
- `/catalog/recipes/by_id.json`

## Traceability Matrix (Target Coverage Check)

Target `GameState` direct fields to cover:
- `system`
- `home_base`
- `nearest_station`
- `current_poi`
- `docked`
- `credits`
- `fuel_pct`
- `cargo_pct`
- `cargo_used`
- `cargo_capacity`
- `cargo`
- `stash`
- `mission_complete`
- `galaxy`
- `market`
- `missions`
- `owned_ships`
- `installed_modules`
- `own_buy_orders`
- `own_sell_orders`

Explicitly excluded from VFS scope:
- `last_mined`
- `last_stashed`
- `script_mined_by_item`
- `script_stashed_by_item`

All nested fields in `GalaxyData`, `MarketData`, `MissionData`, `OpenOrderInfo`, `MarketOrderInfo`, and `CatalogEntryData` are mapped above as target design.

## Implementation Notes

- Build canonical files first; add split/index files incrementally.
- Prefer shared helper functions:
  - `write_scalar(path, value)`
  - `write_map(path, map)`
  - `write_map_entries(prefix, map)`
  - `write_vec(path, vec)`
  - `write_jsonl(path, iter)`
- Keep VFS generation pure and side-effect free:
  - `fn project_game_state_to_vfs(state: &GameState) -> VfsSnapshot`
