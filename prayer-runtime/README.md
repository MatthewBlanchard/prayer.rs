# prayer-runtime

`prayer-runtime` contains Prayer's transport-independent execution core. It
resolves typed actions, evaluates PrayerLang work, plans SpaceMolt operations,
maintains checkpoints, and models the knowledge needed by economic and
navigation decisions. Live authentication, HTTP, and process lifecycle belong
to higher-level crates such as `prayer-sdk` and `prayer-api`.

## Main concepts

- `action_resolution` converts `prayer_actions::Action` values to and from the
  runtime's resolved command representation.
- `engine` evaluates scripts and produces resumable checkpoints.
- `execution` defines scheduler-facing execution snapshots, persisted runs,
  workflow controllers, and terminal action outcomes.
- `orchestration` turns commands into atomic operations such as navigation,
  docking, market, mining, battle, social, and transfer work.
- `read_context` separates bot-local execution state from shared world state and
  declares the capabilities actions require.
- `knowledge` and `economy` provide maintained world models, reservations,
  virtual markets, crafting, logistics, and arbitrage planning.
- `snapshot` defines observations used to merge newly received state.

The crate deliberately has no dependency on the SDK, API, Axum, environment
configuration, or a live SpaceMolt transport. Embedders should normally start
with `prayer-sdk`; this crate is intended for runtime development and lower-level
integration.

## Development

From the repository root:

```console
cargo test -p prayer-runtime
cargo check -p prayer-runtime
```

The tests include architecture checks that enforce the downward-only dependency
direction and keep mixed bot/world state models from being reintroduced.
