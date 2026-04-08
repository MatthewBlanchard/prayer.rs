# Prayer Runtime — DSL Execution Pipeline

Scripts move through three stages before anything happens in the game world.
Each stage has a single responsibility and a defined error surface.

```
DSL source text
      │
      ▼
┌─────────────┐
│   PARSING   │  text → AstProgram, args as raw strings, structural errors thrown
└──────┬──────┘
       │ AstProgram (args: Vec<String>)
       ▼
┌─────────────┐
│   ANALYZER  │  AstProgram + GameState → AnalyzedProgram
│             │  $here substituted, other args canonicalized, identity errors thrown
└──────┬──────┘
       │ AnalyzedProgram (canonical args, dynamic refs, original tokens preserved)
       ▼
┌─────────────┐
│  EXECUTION  │  emit typed EngineCommand per tick, run, bubble CommandFailure
└─────────────┘
```

---

## Stage 1 — Parsing

**Input:** raw DSL source string  
**Output:** `AstProgram` with `CommandNode { name, args: Vec<String> }`  
**Errors:** syntax errors, structural constraint violations (DSL090, DSL200–DSL207)

### What happens

- Tokenise and parse into `AstProgram` / `SkillLibraryAst`
- Validate structural rules: unknown commands, wrong arg counts, unknown
  predicates, recursive skills
- Canonically normalise the script text (`normalize()`)

### What does NOT happen

- No macro resolution — all `$macro` tokens are kept as raw strings and
  forwarded to the analyzer unchanged
- No resolution of arg values against game data
- No type-casting of arg strings

### Key types

```rust
// args stay as raw string tokens — may contain "$here", "$home", "$nearest_station"
pub struct CommandNode {
    pub name: String,
    pub args: Vec<String>,
    pub span: Span,
}
```

### Entry points

- `dsl::parse_script(src)` — parse a script body
- `dsl::parse_library(src)` — parse a skill/override library
- `dsl::validate(program, context)` — structural validation pass
- `RuntimeEngine::set_script(src, &state)` — parse + validate, then hand off to analyzer

---

## Stage 2 — Analyzer

**Input:** `AstProgram` + `GameState` (snapshot at script-load time)  
**Output:** `AnalyzedProgram` — original AST preserved, resolved args alongside  
**Errors:** unknown identifiers, type mismatches, "did you mean X?" suggestions

### Macro resolution

Macros have different resolution semantics:

| Macro | When | Resolves to |
|---|---|---|
| `$here` | Analyzer (script-load time) | `state.system` — fixed origin, never changes mid-script |
| `$home` | Execution (per emit) | current `state.home_base` — may change if `set_home` runs |
| `$nearest_station` | Execution (per emit) | current `state.nearest_station` — changes as player travels |

`$here` is substituted to a concrete string during analysis. `$home` and
`$nearest_station` become `AnalyzedArg::Dynamic` and are resolved by the engine
at emit time from the live `GameState`.

### Arg resolution

For each `CommandNode`, look up its `CommandSpec` from the catalog and resolve
each arg + `ArgSpec` pair:

| ArgType | Resolution |
|---|---|
| `Integer` | Parse to `i64`; hard error if non-numeric |
| `GoTarget` | Match against `state.systems`, `state.pois`, galaxy map |
| `ItemId` | Match against `state.cargo`, `state.galaxy.catalog.items_by_id` |
| `MissionId` | Match against `state.missions.active`, `state.missions.available` |
| `SystemId` | Match against `state.systems`, galaxy map |
| `PoiId` | Match against `state.pois`, galaxy map |
| `ShipId` | Match against `state.owned_ships` |
| `ModuleId` | Match against `state.ship.installed_modules` |
| `RecipeId` | Match against `state.galaxy.catalog.recipes` |
| `ListingId` | Match against `state.market.shipyard_listings` |
| `Any` | Pass through unchanged |

Resolution order for string types:
1. Exact match (normalised: lowercase, spaces/hyphens → `_`)
2. Fuzzy match via `strsim` — if best score ≥ 0.62, error with `"did you mean '{canonical}'?"`
3. No match — hard error

### What does NOT happen

- No game API calls
- No availability checks (`IsAvailable` is an execution concern)
- No command execution
- `AstProgram` is not mutated — original tokens are preserved alongside resolved args

### Key types

```rust
// dsl/analyzer.rs

pub enum AnalyzedArg {
    Resolved(String),       // canonical ID or parsed integer string
    Dynamic(DynamicMacro),  // resolved at emit time by the engine
}

pub enum DynamicMacro {
    Home,
    NearestStation,
}

// Mirrors AstNode — tree structure preserved, not flattened
pub enum AnalyzedNode {
    Command(AnalyzedCommand),
    If(AnalyzedConditional),
    Until(AnalyzedConditional),
}

pub struct AnalyzedCommand {
    pub source: CommandNode,        // original, untouched
    pub args: Vec<AnalyzedArg>,     // resolved or dynamic
}

pub struct AnalyzedConditional {
    pub condition: ConditionExpr,
    pub body: Vec<AnalyzedNode>,
    pub span: Span,
}

pub struct AnalyzedProgram {
    pub statements: Vec<AnalyzedNode>,
    pub here: Option<String>,       // resolved value of $here at script-load time
}

pub struct AnalyzerError {
    pub command: String,
    pub arg_index: usize,
    pub value: String,
    pub suggestion: Option<String>,
    pub span: Span,
}

pub fn analyze(
    program: &AstProgram,
    catalog: &HashMap<String, CommandSpec>,
    state: &GameState,
) -> Result<AnalyzedProgram, Vec<AnalyzerError>>;
```

### GameState — heavy fields use `Arc` to eliminate clones

`GameState` is cloned at various points in the session loop. There is no
separate `GameWorld` struct — markets, missions, and catalog data all live in
`GameState`, but large fields are wrapped in `Arc<T>` so cloning is a refcount
bump rather than a heap copy:

```rust
pub struct GameState {
    // cheap scalars — clone freely
    pub system: Option<String>,
    pub home_base: Option<String>,
    pub nearest_station: Option<String>, // populated fresh by transport each tick
    pub credits: i64,
    pub fuel_pct: i64,
    pub cargo_pct: i64,
    pub docked: bool,

    // heap data — Arc makes clone free
    pub cargo: Arc<HashMap<String, i64>>,
    pub stash: Arc<HashMap<String, HashMap<String, i64>>>,
    pub galaxy: Arc<GalaxyData>,        // map, known POIs, item/ship/recipe catalog
    pub market: Arc<MarketData>,        // market listings, prices, shipyard
    pub missions: Arc<MissionData>,     // active + available missions
    pub owned_ships: Arc<Vec<ShipInfo>>,
    pub script_mined_by_item: Arc<HashMap<String, i64>>,
    pub script_stashed_by_item: Arc<HashMap<String, i64>>,
}
```

### Dependencies

- `strsim` for fuzzy scoring and Levenshtein distance
- `catalog::default_command_catalog()` for `CommandSpec` lookup

---

## Stage 3 — Execution

**Input:** `AnalyzedProgram` (canonical args + dynamic refs), live `GameState` per tick  
**Output:** game actions; `CommandFailure` on failure

### What happens

The `RuntimeEngine` frame-stack walker calls `decide_next(&state)` to emit one
`EngineCommand` at a time. `AnalyzedArg::Resolved` values cast to `CommandArg`
infallibly. `AnalyzedArg::Dynamic` refs are resolved from the live `GameState`
at the moment of emission:

```rust
pub enum CommandArg {
    Any(String),
    Integer(i64),
    ItemId(String),
    SystemId(String),
    PoiId(String),
    GoTarget(String),
    ShipId(String),
    ListingId(String),
    MissionId(String),
    ModuleId(String),
    RecipeId(String),
}

pub struct EngineCommand {
    pub action: String,
    pub args: Vec<CommandArg>,
    pub source_line: Option<usize>,
}
```

### Multi-turn commands

`mine`, `go`, `refuel`, and `explore` span multiple ticks. Rather than holding
state in a non-serializable trait object (the C# approach), each multi-turn
command defines a serializable state enum stored in the execution frame. This
makes multi-turn state part of the checkpoint automatically:

```rust
pub enum ActiveCommandState {
    Mine(MineState),
    Go(GoState),
    Refuel(RefuelState),
    Explore(ExploreState),
}

pub struct MineState {
    pub resource: Option<String>,
    pub target_poi: Option<String>,
    pub excluded_pois: Vec<String>,
    pub excluded_systems: Vec<String>,
}

pub struct GoState {
    pub target: String,
    pub resolved_system: Option<String>,
    pub resolved_poi: Option<String>,
    pub did_move: bool,
}
```

### Error bubbling

```rust
pub enum CommandFailure {
    Transient(String),  // retry same command next tick
    Skip(String),       // log and advance past this command
    Fatal(String),      // halt the script
}
```

Failures are surfaced directly to the bot, not swallowed into a status line.

---

## Immediate Improvements

These can be shipped before the full pipeline port and unblock later work.

### Bugs

**MINED / STASHED predicates always return 0**  
Predicates read `state.last_mined` / `state.last_stashed` (per-turn deltas,
never populated by transport). Session totals live in the engine's internal
`mined_by_item` / `stashed_by_item` accumulators but predicates can't see them.
Scripts using `until MINED(ore) > 5` are silently broken.

Fix: add `script_mined_by_item` / `script_stashed_by_item` to `GameState`.
Add `engine.inject_session_counters(&mut state)` to copy accumulators in before
each `decide_next` call. Predicates read from those fields.

**STASH predicate missing**  
`STASH(poi_id, item_id)` exists in C# but is not registered in
`install_default_predicates()`. Data is already in `GameState.stash`.

### Preparatory

**`ArgSpec` has no `name` field**  
Required for the analyzer (error messages) and transport (named JSON payloads).
Add `name: String` to `ArgSpec` now to avoid a larger refactor when the catalog
is built.

**`set_script` does not receive `GameState`**  
`RuntimeEngine::set_script` needs an `Option<&GameState>` parameter so the
analyzer phase can be wired in. `service.rs` should pass `&session.state`.

**`go` test fixture uses wrong arg type**  
The test `ValidationContext` registers `go` with `ArgType::SystemId` — should
be `ArgType::GoTarget`.

---

## Catalog

All command and predicate specs live in `prayer-runtime/src/catalog.rs`:

```rust
pub fn default_command_catalog() -> HashMap<String, CommandSpec>
pub fn default_predicate_catalog() -> (HashMap<String, PredicateSpec>, HashMap<String, PredicateSpec>)
```

`ArgSpec` carries a `name` field used by both the analyzer (error messages) and
the transport (building named JSON payloads):

```rust
pub struct ArgSpec {
    pub name: String,       // e.g. "destination", "item_id", "quantity"
    pub kind: ArgType,
    pub required: bool,
    pub default: Option<String>,
}
```

The 26 commands: `mine`, `survey`, `explore`, `go`, `accept_mission`,
`abandon_mission`, `dock`, `set_home`, `repair`, `refuel`, `sell`, `buy`,
`cancel_buy`, `cancel_sell`, `retrieve`, `stash`, `switch_ship`, `install_mod`,
`uninstall_mod`, `buy_ship`, `buy_listed_ship`, `commission_ship`, `sell_ship`,
`list_ship_for_sale`, `wait`, `craft`.
