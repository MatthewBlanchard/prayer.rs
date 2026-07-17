# prayer-sdk

`prayer-sdk` is Prayer's embeddable Rust application facade. It authenticates
and discovers SpaceMolt accounts, restores durable sessions, maintains bot and
shared-world state, runs background refresh workers, and coordinates exclusive
typed-action and PrayerLang execution lanes.

Use this crate when Prayer should run inside your Rust process. Use
`prayer-api` when clients need an HTTP boundary.

## Quick start

The included example connects all available accounts, inspects maintained
state, selects `my-miner`, executes typed actions, and queues a PrayerLang run:

```console
export SPACEMOLT_CLERK_API_KEY="..."
cargo run -p prayer-sdk --example bot_bootstrap
```

Change the selector in `examples/bot_bootstrap.rs` to an owned bot username.
The basic embedded flow is:

```rust
use prayer_sdk::prelude::*;

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let key = std::env::var("SPACEMOLT_CLERK_API_KEY")?;
let sdk = PrayerSdk::connect(
    PrayerSdkOptions::default().with_clerk_api_key(key),
).await?;

let bot = sdk.bot("my-miner").await?;
let run = bot.start_actions([
    Action::Undock,
    Action::Go {
        destination: GoTarget::Poi("sol_central".into()),
    },
    Action::Dock,
]).await?;
println!("{:?}", run.wait().await?);

let script = bot.start_script("go station-sol;\ndock;").await?;
println!("{:?}", script.wait().await?);
sdk.shutdown().await?;
# Ok(())
# }
```

## Main APIs

- `PrayerSdk::connect`, `state`, `bot`, and `shutdown` own application
  lifecycle and maintained snapshots.
- `BotHandle` starts or directly awaits typed action and PrayerLang runs and
  exposes queue and bot state.
- `ActionRunHandle` and `ScriptRunHandle` expose IDs, status, waiting,
  reattachment, and cancellation.
- `PrayerState` separates fleet snapshots from shared world knowledge and
  provides synchronous queries over a captured snapshot.
- Action/script overrides can preempt normal work between atomic operations;
  `OverrideOptions` controls optional best-effort return to origin.
- `PrayerAdministration` exposes host-level virtual-order, crafting, and
  inventory-movement operations.

For host-managed construction, use `options_from_client`,
`with_runtime_options`, `sdk_from_options`, `restore`, and
`start_background_workers`. HTTP wire DTOs remain in `prayer-api-contracts`.

## More examples and checks

```console
PRAYER_BOT=my-miner cargo run -p prayer-sdk --example run_handles
cargo run -p prayer-sdk --example canonical_types
cargo test -p prayer-sdk
cargo check -p prayer-sdk
```

The SDK and its persistence formats are currently alpha APIs.
