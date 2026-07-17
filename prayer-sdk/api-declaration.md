# `prayer-sdk` curated public API

This declaration records the current alpha surface. `cargo xtask check`
compiles it through the external consumer fixture and Rustdoc. Host-facing
items are intentionally absent from the prelude; `#[doc(hidden)]` modules
exist only while the HTTP host is migrated onto stable SDK adapters.

The rich state graph is owned by `prayer-state` and exposed directly during
alpha. Shared-model and SpaceMolt changes may therefore break Rust SDK consumers before
beta. Execution, scheduler, transport, and service implementations remain
private.

## Stable root and prelude

- Lifecycle: `PrayerSdk`, `PrayerSdkOptions`, `StartupReport`,
  `StartupAccountStatus`, `PrayerSdk::{connect, shutdown, startup_report,
  state, bot, bots}`.
- State: `PrayerState`, `BotState`, `BotStateEntry`, `Catalog`, `Galaxy`,
  `StationMarket`, `BotSelector`.
- Execution: `BotHandle`, `ActionRunHandle`, `ScriptRunHandle`, `RunStatus`,
  `WaitOptions`, `ActionRunOutcome`, `ScriptRunOutcome`, `ScriptErrorKind`,
  `LaneOwner`, `QueueSnapshot`.
- Errors: `SdkError`, `SdkClientError`, `SdkExecutionError`,
  `SdkErrorDetails`.
- Typed actions: the root re-exports `Action`, `RunId`, `GoTarget`, `ItemId`,
  all supported request records, and the transfer/trade value types from
  `prayer-actions`.

## Host-facing root API

- Hidden modules: `selectors`, `spacemolt_origin`, `spacemolt_projection`, and
  `state_mapping`.
- Host construction and lifecycle: `options_from_client`,
  `options_from_client_options`, `with_runtime_options`,
  `with_persistence_paths`, `sdk_from_options`, `restore`,
  `start_background_workers`, `RuntimeServiceOptions`, `SpacemoltClient`, and
  `SpacemoltClientOptions`.
- Host administration: `PrayerAdministration`.

## Disposition of former “maybe remove” items

- Root `PrayerSdk::new`, `restore`, and `start_background_workers` methods were
  removed. Explicit free-function host equivalents remain for `prayer-api`.
- `administration` remains a hidden host bridge while its consumers migrate.
- `bot_snapshots` and `bot_states` were removed. Applications use
  `state().fleet.bots`; the HTTP host temporarily uses the hidden
  `host_bot_snapshots` projection bridge.
- `script_execution` remains a hidden HTTP-host bridge. Applications recover
  executions with `script_run` and inspect them through run handles.
- `register_bot` remains because the HTTP onboarding endpoint is its external
  consumer. Its result is the Prayer-owned `RegistrationResult`.
- Direct selectors, projections, and mappings remain available through hidden
  root modules. HTTP contracts live in `prayer-api-contracts`.
- Direct `galaxy` and `catalog` methods were removed. Applications read the
  corresponding fields from `PrayerState`.

## Removed or renamed before stabilization

- `PrayerState::Deref` intentionally exposes the rich shared-state snapshot
  during alpha.
- `BotRuntimeStatus` and the redundant combined `BotHandle::status` view were
  removed. Observed facts live in bot/fleet state; scheduler and halt facts
  live in `QueueSnapshot`.
- `BotHandle::{start, execute}` became `{start_actions, execute_actions}`.
- `QueueSnapshot::prayerlang` became `rendered_prayerlang`.
- `SdkClientError::structured_error_payload` became `details`.
- Ordinary lifecycle and execution signatures do not expose scheduler,
  service, HTTP-host, or SpaceMolt client implementations.
