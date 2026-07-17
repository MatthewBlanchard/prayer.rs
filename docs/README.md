# Design documentation

This directory records Prayer's current SDK compatibility and shared-state
architecture decisions.

- [SDK compatibility and release policy](sdk-compatibility.md) defines the
  alpha compatibility posture, the surfaces that semantic versioning will
  cover once stable, and the policy for exposing canonical Rust state types.
- [Shared state ownership](shared-state-ownership.md) assigns ownership for
  fleet and world snapshot data, runtime metadata, orchestration capabilities,
  and HTTP projections, including the checks that enforce those boundaries.

These documents describe the current design rather than a stable public API.
For setup and consumer examples, see the repository's main README.
