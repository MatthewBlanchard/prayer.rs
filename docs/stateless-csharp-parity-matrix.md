# Stateless C# Parity Matrix

This audit tracks parity between:
- C# command engine behavior in `SpaceMoltLLM/src/Prayer/MiddleRuntime/Commands`
- Rust stateless orchestration in `prayer-runtime/src/transport/orchestrator.rs`

Design target: keep Rust orchestration stateless (args + latest runtime snapshot) while aligning command names, payload shapes, and user-visible outcomes where practical.

## Command Matrix

| Command | C# Behavior | Rust Before | Rust Target (Now) | Intentional Stateless Divergence |
|---|---|---|---|---|
| `wait` | Single-turn delay; clamp to max 30 ticks; 10s/tick | Same | Same | None |
| `mine` | Multi-turn, route/target selection, repeated mine | Single API `mine` call | Stateless multi-turn: route to nearest known resource POI and mine until cargo full | No transport-local continuation state |
| `explore` | Multi-turn, route/unvisited discovery/survey | Single API `survey_system` call | Stateless multi-turn: undock/jump/travel/survey based on snapshot exploration sets | No transport-local continuation state |
| `go` | Multi-turn routing, resolve POI/system, undock/jump/travel | Single-step jump/travel fallback | Stateless multi-turn: undock/jump/travel step-by-step from runtime snapshot | No transport-local continuation state |
| `retrieve` | Auto-dock flow, quantity heuristics/retries | Single `withdraw_items` | Stateless auto-dock + `withdraw_items` | Retry heuristics not implemented |
| `stash <item>` | Auto-dock single-turn deposit stack quantity | Single `deposit_items` | Stateless auto-dock + `deposit_items` | No transport-local continuation state |
| `stash` | Auto-dock batch deposit all cargo stacks | Batch from runtime snapshot | Stateless auto-dock, repeated deposit until cargo empty | No transport-local continuation state |
| `buy` | Auto-dock, market-aware order creation | Direct `buy` API | Stateless auto-dock + `create_buy_order` | Price strategy simplified (fixed default) |
| `sell <item>` | Auto-dock create sell order with market strategy | Direct `sell` API | Stateless auto-dock + `create_sell_order` | Price strategy simplified (fixed default) |
| `sell` | Auto-dock multi-step sell queue | Not supported / item-required mismatch | Stateless auto-dock + repeated `create_sell_order` over cargo stacks | No transport-local continuation state |
| `cancel_buy <item>` | Auto-dock; cancel matching open buy orders by `order_id` | Direct `cancel_buy` API | Stateless auto-dock + `cancel_order` per matching runtime `own_buy_orders` | No transport-local continuation state |
| `cancel_sell <item>` | Auto-dock; cancel matching open sell orders by `order_id` | Direct `cancel_sell` API | Stateless auto-dock + `cancel_order` per matching runtime `own_sell_orders` | No transport-local continuation state |
| `set_home` | Auto-dock, `set_home_base` | Unsupported | Unsupported | Kept unsupported by decision |

## API Action Name Mapping

| DSL Command | Rust Stateless API Action(s) |
|---|---|
| `wait` | in-process sleep |
| `mine` | `mine` |
| `explore` | `undock` (if docked), `jump`/`travel` step-by-step, `survey_system` |
| `go` | `undock` (if docked), `jump`/`travel` step-by-step |
| `retrieve` | auto-dock steps (`undock`/`travel`/`dock`) + `withdraw_items` |
| `stash` | auto-dock steps (`undock`/`travel`/`dock`) + `deposit_items` |
| `buy` | auto-dock steps (`undock`/`travel`/`dock`) + `create_buy_order` |
| `sell` | auto-dock steps (`undock`/`travel`/`dock`) + `create_sell_order` |
| `cancel_buy` | auto-dock steps (`undock`/`travel`/`dock`) + `cancel_order` |
| `cancel_sell` | auto-dock steps (`undock`/`travel`/`dock`) + `cancel_order` |

## Temporary Mismatch (Tracked)

- `set_home` is present in command catalog but remains unsupported in Rust transport orchestration by explicit decision for this pass.

## Stateful Scaffolding Review

- Engine active multi-turn checkpoint state is only used for `mine`, `go`, `refuel`, and `explore`.
- `sell`, `stash`, `buy`, `retrieve`, and cancel commands do not rely on active-command continuation state in Rust runtime.
- Current stateless transport changes do not introduce dependencies on runtime continuation state for those commands.
- `go`/`mine` continuation in Rust transport is stateless and driven by requeued command + fresh runtime snapshot (no transport-local checkpoint state).
