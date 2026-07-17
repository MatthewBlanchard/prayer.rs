# SDK compatibility and release policy

Both SDKs remain `0.1.0-alpha.x` source distributions. They are not represented
as published registry artifacts or stable APIs.

Once a release line is declared stable, semantic versioning covers the Rust
crate's public items and feature flags; the TypeScript root, `@prayer/sdk/api`,
and `@prayer/sdk/types` exports; generated Prayer API major-version contracts;
and serialized action/run contracts. The high-level facade may add compatible
conveniences without changing HTTP contracts. Generated OpenAPI changes are
HTTP contract changes and must regenerate the TypeScript wire layer.

The checked-in TypeScript declaration report detects public additions,
removals, renames, and type changes. Intentional changes require regenerating
that report and documenting the compatibility impact in release notes. The
external Node, browser, reference-client, and Rust fixtures protect artifact
resolution independently from in-workspace source imports. Rust semver baseline
comparison will be added when the first registry/crates.io release establishes
a meaningful baseline.

Alpha versions may break these surfaces between releases. Beta requires stable
run, idempotency, error, export, and packaging contracts. Stable requires the
documented ordinary workflows to avoid runtime, scheduler, transport, or
generated-client internals and to pass the cross-platform release matrix.

## Rust alpha state policy

During alpha, the Rust SDK intentionally exposes the canonical `prayer-state`
game-state types through `PrayerState`. This keeps state rich and direct, avoids a second
mirrored schema that can drift, and lets applications use normal fields, maps,
iterators, and serialization without an accessor layer.

SpaceMolt is changing quickly during this phase. We prefer those upstream and
shared-model changes to break Rust consumers immediately and visibly at compile
time rather than preserve compatibility through copied models that become
stale or semantically drift from the game.

Those data types may change between alpha releases. Upstream SpaceMolt types
embedded in canonical records may also change. That exposure is accepted
as long as implementation capabilities do not leak with it. The ordinary SDK
must not expose runtime services, scheduler control, locks, workers,
persistence machinery, SpaceMolt clients, or HTTP-host administration. State
data may be public; the machinery that produces and operates on it remains
encapsulated.

Before beta, consumer usage will determine whether consumers should import
`prayer-state` directly or rely on `prayer-sdk` re-exports. Until then, shared
state changes are allowed to be Rust SDK breaking changes.
