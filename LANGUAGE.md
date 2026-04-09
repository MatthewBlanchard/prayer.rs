# PrayerLang Reference

This document describes the DSL currently implemented in `prayer-runtime`.

## Script syntax

Scripts are statement-based.

- Command statements must end with `;`
- `if` / `until` blocks use `{ ... }`
- Line comments use `//`

Example:

```prayer
// mine until cargo is full, then halt
until CARGO_PCT() >= 100 {
  mine;
}
halt;
```

## Control flow

Control-flow keywords are case-sensitive:

- `if`
- `until`

Examples:

```prayer
if MISSION_COMPLETE(mission_1) {
  halt;
}

until FUEL() >= 50 {
  refuel;
}
```

## Conditions

A condition is either:

- A metric call, e.g. `MISSION_COMPLETE(m1)`
- A comparison, e.g. `FUEL() >= 50`

Supported operators:

- `>`
- `>=`
- `<`
- `<=`
- `==`
- `!=`

## Arguments and tokens

Command arguments are identifier-like tokens:

- Plain: starts with `[A-Za-z0-9_]`, then `[A-Za-z0-9_-]*`
- Macro/param ref: starts with `$`, then `[A-Za-z_]`, then `[A-Za-z0-9_-]*`

Integer arguments are parsed as signed 64-bit integers where required.

## Built-in macros

Macros are resolved by analyzer/engine:

- `$here`: resolved at script-load time to current system (error if unknown)
- `$home`: dynamic at execution time
- `$nearest_station`: dynamic at execution time

Skill parameters are referenced as `$param_name`.

## Skill library syntax

Skill libraries support:

- `skill`
- `override ... when ...`
- `@disable`

Keywords are case-sensitive (`skill`, `override`, `when`, `@disable`).

Example:

```prayer
@disable mine;

skill travel(system: system_id) {
  go $system;
}

override safety when FUEL() <= 5 {
  halt;
}
```

Notes:

- `@disable` accepts optional trailing `;`
- Skill parameter types:
  - `any`
  - `integer`
  - `item_id`
  - `system_id`
  - `poi_id`
  - `go_target`
  - `ship_id`
  - `listing_id`
  - `mission_id`
  - `module_id`
  - `recipe_id`

## Default command catalog

Current built-in commands:

- `halt`
- `mine [resource]`
- `survey`
- `explore`
- `go <destination>`
- `accept_mission <mission_id>`
- `abandon_mission <mission_id>`
- `decline_mission <template_id>`
- `complete_mission <mission_id>`
- `dock`
- `set_home`
- `repair`
- `refuel`
- `self_destruct`
- `sell [item]`
- `buy <item> <quantity>`
- `cancel_buy <item>`
- `cancel_sell <item>`
- `retrieve <item> [quantity]`
- `stash [item]`
- `jettison [item]`
- `use_item <item_id> [quantity]`
- `switch_ship <ship>`
- `install_mod <mod>`
- `uninstall_mod <mod>`
- `buy_ship <ship_class>`
- `buy_listed_ship <listing>`
- `commission_ship <ship_class>`
- `sell_ship <ship>`
- `list_ship_for_sale <ship> <price>`
- `wait [ticks]`
- `craft <recipe_id> [count]`
- `salvage_wreck <wreck_id>`
- `tow_wreck <wreck_id>`
- `loot_wreck <wreck_id> <item_id> <quantity>`
- `scrap_wreck`
- `sell_wreck`
- `distress_signal [fuel|repair|combat]`

## Default predicates

Boolean predicates:

- `MISSION_COMPLETE(mission_id)`

Numeric predicates:

- `FUEL()`
- `CREDITS()`
- `CARGO_PCT()`
- `CARGO(item_id)`
- `MINED(item_id)`
- `STASHED(item_id)`
- `STASH(poi_id, item_id)`

## Mining and cargo semantics

- `mine` and `mine <resource>` are multi-turn commands that keep mining until cargo is full.
- `mine` and `mine <resource>` add mined items to ship cargo.
- `mine ore;` targets ore sources, but mined cargo can still include non-ore/byproduct items.
- Cargo is not moved automatically to storage or market.
- Use `stash` to move cargo into station storage.
- Use `sell` to convert cargo to credits at market.

## Practical examples (from saved C# script examples)

These examples are taken from `SpaceMoltLLM/src/Prayer/cache/script_generation_examples.json` and kept in valid PrayerLang form.

### 1) Simple mining

Prompt:

```txt
mine
```

Script:

```prayer
mine;
```

### 2) Travel with explicit docking between jumps

Prompt:

```txt
jump horizon then dock then jump nexus
```

Script:

```prayer
go horizon;
dock;
go nexus;
```

### 3) Mine, return to current system, then sell

Prompt:

```txt
go node_alpha mine and sell here
```

Script:

```prayer
go node_alpha;
mine;
go $here;
sell;
```

### 4) Buy specific quantity

Prompt:

```txt
buy 9 iron
```

Script:

```prayer
buy iron_ore 9;
```

### 5) Targeted mining resource

Prompt:

```txt
mine carbon
```

Script:

```prayer
mine carbon_ore;
```

### 6) Repeat until enough has been stashed

Prompt:

```txt
mine iron and stash 50 this run
```

Script:

```prayer
until STASHED(iron_ore) >= 50 {
  mine iron_ore;
  go $home;
  stash;
}
```

### 7) Mission progress loop using predicate

Prompt:

```txt
mine_resource: Mine 25 units of Copper Ore (24 / 25)
mission_id=f0b9db
```

Script:

```prayer
until MISSION_COMPLETE(f0b9db) {
  mine copper_ore;
  go $home;
  stash;
}
```

### 8) Batch accept mission IDs

Prompt:

```txt
accept_mission copper_requisition, accept_mission deep_core_prospecting, accept_mission iron_supply_run
```

Script:

```prayer
accept_mission copper_requisition;
accept_mission deep_core_prospecting;
accept_mission iron_supply_run;
```

## Compatibility note for older C# helper commands

Some saved C# scripts may include helper skills/commands like:

```prayer
mine_and_stash 50 iron_ore $home;
```

If that helper is not defined in your current skill library, rewrite it using core DSL control flow:

```prayer
until STASHED(iron_ore) >= 50 {
  mine iron_ore;
  go $home;
  stash;
}
```

## Parsing and validation pipeline

1. Parse script/library into AST.
2. Validate against command/predicate catalogs + skill signatures.
3. Analyze arguments (resolve static values + dynamic macros).
4. Execute analyzed commands in runtime loop.

If you change syntax or catalogs, update this file and `README.md`.
