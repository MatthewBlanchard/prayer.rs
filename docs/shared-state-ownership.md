# Shared state ownership

This inventory records the ownership boundary for every category reachable
from the public fleet and world snapshots. Canonical records have exactly one
definition in `prayer-state`; runtime and SDK code borrow, clone `Arc`s, or
mutate those records directly rather than translating them into parallel DTOs.

| Reachable state | Owner | Notes |
| --- | --- | --- |
| `BotId`, `BotState`, ship, cargo, mission, passenger, crafting projections | `prayer-state` | Passive observed player facts; generated SpaceMolt records remain canonical during alpha. |
| `FleetEntry`, `FleetState`, `FleetSnapshot`, script and active-route projections | `prayer-state` | JSON-backed projections retain their existing serialized representation. |
| `CatalogData` | `prayer-state` | Contains generated item, ship, recipe, facility, and skill definitions. |
| `GalaxyData`, systems, POIs, facilities, sightings, wildlife | `prayer-state` | `RouteTable` and `RouteCache` are pure derived queries and live beside galaxy facts. |
| Station markets, passengers, salvage, storage, faction garage and treasury facts | `prayer-state` | Process-local collections retain their existing serialization policy. |
| `WorldState`, `WorldSnapshot`, `StateSnapshot`, `WorldLens` | `prayer-state` | Snapshot roots preserve `Arc` sharing and contain no clocks, locks, workers, or service handles. |
| Refresh timestamps and invalidation clocks | `prayer-runtime::knowledge::WorldRuntimeMetadata` | Process-local `Instant` values are held in the runtime service sidecar and never enter shared snapshots or persistence. |
| Observation merge policy and freshness decisions | `prayer-runtime` / runtime service | Mutates canonical shared records without changing their ownership. |
| Execution, scheduling, transport, persistence orchestration | `prayer-runtime` and host crates | Capabilities are not reachable through shared state. |
| HTTP endpoint DTOs, casing, deltas, and response envelopes | `prayer-api-contracts` / `prayer-api` | Transport projections remain separate from canonical in-memory state. |

`prayer-state` intentionally depends on `spacemolt-lib-rs` during alpha. This
avoids duplicating the generated game schema; changes to those embedded records
may therefore remain breaking shared-state changes until the pre-beta policy is
revisited.

The boundary is enforced by crate architecture tests, SDK public-signature
audits, the external Rust consumer fixture, generated OpenAPI/TypeScript drift
checks, and the repository verification task.
