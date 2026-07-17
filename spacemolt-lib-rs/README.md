# spacemolt-lib-rs

`spacemolt-lib-rs` is Prayer's low-level Rust client foundation for SpaceMolt.
It provides the v2 WebSocket transport and protocol envelopes, authentication,
account handling, generated command and schema types, notifications, and local
state caches used by Prayer's higher-level runtime.

The crate is a port in progress of `@spacemolt/lib`; consumers should expect its
API to evolve with Prayer and the SpaceMolt protocol.

## Generated protocol types

The build script reads the repository's checked-in
`../spacemolt-openapi.json` and generates Rust actions, commands, types, and
notifications in Cargo's build output. Refresh that specification through the
workspace task runner, then rebuild the crate:

```console
cargo xtask refresh-spacemolt --openapi-only
cargo check -p spacemolt-lib-rs
```

Use `--base-url <URL>` with `refresh-spacemolt` to fetch from a SpaceMolt server
other than `https://game.spacemolt.com`. Refreshing requires network access;
ordinary builds use the checked-in specification.

## Development

From the repository root:

```console
cargo check -p spacemolt-lib-rs
cargo test -p spacemolt-lib-rs
cargo clippy -p spacemolt-lib-rs --all-targets
```

The test suite uses local mock HTTP and WebSocket servers and does not require a
live SpaceMolt account.
