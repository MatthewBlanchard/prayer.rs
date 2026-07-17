# `prayer-state` alpha public API

`prayer-state` owns Prayer's canonical passive in-memory state graph. Its
public surface consists of:

- bot, fleet, identity, ship, cargo, mission, passenger, crafting, and script
  projection records;
- catalog, galaxy, system, POI, facility, market, salvage, social, and wildlife
  records;
- `WorldState`, `WorldSnapshot`, `StateSnapshot`, and `WorldLens`;
- the pure `RouteTable`, `RouteCache`, lookup helpers, and aggregate queries.

The crate intentionally exposes generated `spacemolt-lib-rs` facts during
alpha and therefore does not yet promise field-level or serialized-form
stability. It contains no runtime services, schedulers, locks, workers,
transport clients, persistence orchestration, refresh behavior, or
process-local clocks.

`prayer-sdk` re-exports ergonomic names for the primary state roots. Whether
applications should import this crate directly as a separately supported
surface remains an explicit pre-beta decision.
