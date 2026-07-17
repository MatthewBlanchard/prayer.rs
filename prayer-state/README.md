# prayer-state

`prayer-state` contains Prayer's canonical passive state models and pure state
queries. The runtime and SDK use these shared types to describe bot, fleet,
galaxy, catalog, market, mission, faction, and other observed state without
introducing locks, async runtimes, or execution capabilities into the model
layer.

Important entry points include:

- `BotState`, `FleetState`, and the generic `StateSnapshot`/`WorldSnapshot`
  models.
- `GalaxyData` and `CatalogData`, including convenience lookups and cached
  route queries.
- `WorldLens`, a read-only view over world knowledge.
- `RouteTable`, a deterministic all-pairs routing table with hop, path, next
  hop, and optional penalized-route queries.

```rust
use std::collections::HashMap;
use prayer_state::RouteTable;

let graph = HashMap::from([
    ("sol".to_owned(), vec!["alpha".to_owned()]),
    ("alpha".to_owned(), vec!["beta".to_owned()]),
]);
let routes = RouteTable::build(&graph);

assert_eq!(routes.hop_distance("sol", "beta"), Some(2));
assert_eq!(routes.next_hop_toward("sol", "beta").as_deref(), Some("alpha"));
```

From the repository root:

```console
cargo check -p prayer-state
cargo test -p prayer-state
```
