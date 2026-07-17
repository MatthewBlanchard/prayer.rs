# prayer-actions

`prayer-actions` defines the source-independent action protocol shared by
Prayer producers, schedulers, persistence, and runtime executors. It contains
data types only; it does not execute SpaceMolt commands.

The main API is the exhaustive `Action` enum. Dedicated request types model
navigation, transfers, trading, crafting, facilities, ship operations, and
other high-level operations. `ActionEnvelope` adds an ID, origin, optional
source reference, and the current `ACTION_SCHEMA_VERSION` for durable queues.
`ActionOutcome` and `ContinuationEnvelope` describe execution results and
resumable work.

All protocol types support Serde serialization, and action-facing types also
derive JSON Schema through `schemars`.

```rust
use prayer_actions::{Action, ActionEnvelope, ActionOrigin, GoTarget};

let action = ActionEnvelope::new(
    "action-1",
    Action::Go {
        destination: GoTarget::Poi("sol_central".into()),
    },
    ActionOrigin::Manual {
        run_id: "run-1".into(),
    },
);
```

From the repository root:

```console
cargo check -p prayer-actions
cargo test -p prayer-actions
```
