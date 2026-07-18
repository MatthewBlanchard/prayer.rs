# Prayer

One SDK for connecting, observing, and directing your
[SpaceMolt](https://spacemolt.com/) bots.

Prayer handles authentication, bot discovery, live state, durable execution,
and background synchronization. Connect once, select a bot, and direct it with
typed actions or PrayerLang:

```ts
import { Prayer, dock, go, refuel, selectRoute } from "@prayer/sdk";

const prayer = await Prayer.connect({
  baseUrl: "http://127.0.0.1:7777",
});
const bot = await prayer.bot("my-miner");

// Queue typed actions: fly to Sol Central, dock, and fill the tank.
await bot.execute([go({ poi: "sol_central" }), dock(), refuel()]);

// Use maintained world state to find the nearest known source of iron.
const state = await prayer.state();
const here = (await bot.state()).state.location.poi_id ?? "sol";
const ironSource = (state.world.resources.poisByResource.iron_ore ?? [])
  .flatMap((poi) => {
    const route = selectRoute(state.world.map, here, poi);
    return route ? [{ poi, jumps: route.totalJumps }] : [];
  })
  .sort((a, b) => a.jumps - b.jumps)[0]?.poi;

if (!ironSource) throw new Error("No reachable source of iron is known");

// Hand the bot a PrayerLang routine for the mining trip.
const miningRun = await bot.startScript(`
  go ${ironSource};
  mine;
`);
console.log(await miningRun.wait());
```

That is the basic model: connect to your account, get a handle to a bot, and
give it work. Prayer owns the session loop and maintains the state needed to
carry that work through.

Prayer is under active development. Its APIs and persistence formats are not
yet stable, and it currently targets contributors and local operators.

## Direct bots your way

### Typed actions

Typed actions work well when commands originate in application code and should
be explicit and compiler-checked.

```rust
let outcome = bot
    .execute_actions([
        Action::Undock,
        Action::Go {
            destination: GoTarget::Poi("sol_central".into()),
        },
        Action::Dock,
    ])
    .await?;

println!("{outcome:?}");
```

For work you do not want to await immediately, start a run and retain its
handle:

```rust
let run = bot
    .start_actions([
        Action::Undock,
        Action::Go {
            destination: GoTarget::Poi("sol_central".into()),
        },
    ])
    .await?;

println!("run: {:?}", run.id());
println!("status: {:?}", run.status().await?);

let outcome = run.wait().await?;
```

Runs can be inspected, awaited, reattached by ID, or cancelled. Prayer
executes their actions through a durable, exclusive lane for that bot.

### PrayerLang

PrayerLang is a strictly linear list of commands. Clients resolve dynamic
choices such as the nearest station before submitting a script.

```rust
let run = bot
    .start_script(
        r#"
        go station-sol;
        dock;
        "#,
    )
    .await?;

println!("run: {:?}", run.id());
let outcome = run.wait().await?;
```

Typed actions and PrayerLang share the same bot state, execution lane, and
runtime. An application can use either interface—or choose based on where an
instruction originates.

Client policy can preempt normal work between atomic SpaceMolt actions without
embedding conditions in PrayerLang:

```rust
bot.execute_action_override(
    [prayer_sdk::Action::Refuel(prayer_sdk::ServiceTransferRequest {
        target: None,
        quantity: None,
        item: None,
    })],
    prayer_sdk::OverrideOptions::default(),
).await?;
```

`execute_script_override` accepts the same kind of linear PrayerLang plan.
Set `return_to_origin` to opt into a best-effort return before normal work
resumes; it defaults to `false`.

## Inspect live state

Prayer connects owned accounts, restores durable state, hydrates its maintained
cache, and starts background synchronization. Bot-local state is available
from the bot handle:

```rust
let bot_state = bot.state().await?;

println!("ship: {:?}", bot_state.state.ship);
println!("cargo: {:?}", bot_state.state.cargo);
println!("location: {:?}", bot_state.state.location);
```

Shared fleet and world knowledge is available from the SDK:

```rust
let state = sdk.state().await;

println!("bots: {}", state.fleet.bots.len());
println!("systems: {}", state.world.state.galaxy.systems.len());
println!(
    "known station markets: {}",
    state.world.state.station_markets.len()
);
```

Consumers do not need to build a separate session manager, refresh loop, or
world-state cache.

## Choose your integration

- **TypeScript SDK** — connect browser or Node applications with typed actions,
  PrayerLang runs, and maintained state.
- **Python SDK** — use the same HTTP workflows from async Python 3.11–3.13
  applications. This SDK is currently untested.
- **Rust SDK** — embed the Prayer runtime directly without an HTTP boundary.
- **HTTP API** — control bots from another process or language.

The Rust SDK is the embedded runtime interface. The TypeScript and Python SDKs
are typed clients for the HTTP API.

The included **control room** is a reference consumer of these interfaces. It
demonstrates how to operate sessions and jobs and build a complete web UI on the
same bot, action, run, and state contracts available to other applications.

## Prerequisites and bootstrap

Prayer supports Rust 1.78 or newer and Node.js 22 or newer with npm 10 or 11. Install
Xcode Command Line Tools on macOS; a C compiler, linker, `pkg-config`, and
TLS/build packages on Linux; or the Rust MSVC toolchain and Visual Studio C++
Build Tools on native Windows. WSL is an optional fallback, not a requirement.
Python 3.11–3.13 is required to develop the Python SDK. `curl` and `jq` are
maintenance conveniences and are not needed to build or run Prayer.

From a fresh clone, install locked JavaScript dependencies and build the local
TypeScript SDK before anything consumes it:

```console
cargo xtask bootstrap
cargo xtask check
```

Bootstrap reports a missing `SPACEMOLT_CLERK_API_KEY` but does not require
credentials to compile or test. Copy `.env.example` for a complete configuration
reference; programs read environment variables, so load that file using your
preferred platform tooling.

## Embed the Rust SDK

A live account requires a SpaceMolt Clerk API key. In Bash:

Set your key and run the included SDK example:

```bash
export SPACEMOLT_CLERK_API_KEY="..."
cargo run -p prayer-sdk --example bot_bootstrap
```

In PowerShell:

```powershell
$env:SPACEMOLT_CLERK_API_KEY = "..."
cargo run -p prayer-sdk --example bot_bootstrap
```

The example connects available accounts, selects `my-miner`, prints maintained
state, executes typed actions, and starts a PrayerLang run. Change the selector
in `prayer-sdk/examples/bot_bootstrap.rs` to the username of one of your bots.

To use Prayer from another Rust project while it is developed in this
repository:

```toml
[dependencies]
prayer-sdk = { path = "../prayerrs/prayer-sdk" }
prayer-actions = { path = "../prayerrs/prayer-actions" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Run the HTTP API

Start the HTTP API:

```bash
export SPACEMOLT_CLERK_API_KEY="..."
cargo run -p prayer-api
```

PowerShell uses `$env:SPACEMOLT_CLERK_API_KEY = "..."` before the same Cargo
command. The API listens on `127.0.0.1:7777`; `PRAYER_RS_BIND` overrides it.
A non-loopback bind requires `PRAYER_API_TOKEN`.

## Use the TypeScript SDK

After `cargo xtask bootstrap`, import `@prayer/sdk` from
`prayer-sdk-ts`. It is a source-distributed local package during development;
its compiled JavaScript and declarations are produced in `dist/` by bootstrap.
Applications connect it to a running Prayer HTTP API.

## Use the Python SDK

> **Note:** The Python SDK is currently untested.

Install `prayer-sdk-py` into a virtual environment, connect once at application
startup, and share the returned client:

```python
from prayer_sdk import Prayer
from prayer_sdk.actions import dock, go

prayer = await Prayer.connect("http://127.0.0.1:7777", token="...")
bot = await prayer.bot("my-miner")
result = await bot.execute([go(poi="sol_central"), dock()])
await prayer.aclose()
```

See [`prayer-sdk-py/README.md`](prayer-sdk-py/README.md) for generated API,
error, idempotency, and reattachment details.

## Run the complete local control room

The canonical launcher starts the API, waits for readiness, then starts the
selected services. The web client uses ports 3001 (agent server) and 5173
(Vite), and the API uses 7777:

```console
cargo xtask bootstrap
cargo xtask run --client web
```

Omit `--client web` to run the API without the control room.

The API's durable JSON defaults are under the platform data directory selected
by `dirs`; control-room JSON defaults are in `reference-client-ts`, and logs
default to `logs/`. Override their paths with the variables in `.env.example`.
The process needs read/write permission for those chosen locations.

## Contribute

From a clean clone:

```console
cargo xtask bootstrap
cargo xtask check
```

`check` deliberately does not access the network. `cargo xtask build` downloads
the current official SpaceMolt OpenAPI specification, validates and replaces the
checked-in snapshot, regenerates contracts, and then runs the complete Rust and
TypeScript build/test boundary. Use `cargo xtask build --offline` only when a
networkless, reproducible build from the checked-in SpaceMolt snapshot is needed.
Network-dependent SpaceMolt smoke tests remain separate from deterministic checks.

## Repository map

| Path                  | Purpose                                                  |
| --------------------- | -------------------------------------------------------- |
| `prayer-sdk`          | Bot connection, selection, state, actions, and lifecycle |
| `prayer-actions`      | Shared typed action contracts                            |
| `prayer-lang`         | PrayerLang parsing and validation                        |
| `prayer-scheduler`    | Durable queues, run ownership, and checkpoints           |
| `prayer-runtime`      | Execution planning and game mechanics                    |
| `spacemolt-lib-rs`    | Typed SpaceMolt transport and authentication             |
| `prayer-api`          | HTTP service over the SDK                                |
| `reference-client-ts` | Agent server and web control room                        |

## Development

The root build pipeline validates the upstream SpaceMolt contract, regenerates the
Prayer OpenAPI and TypeScript SDK, and then compiles and tests Rust and TypeScript:

```bash
cargo xtask generate # update checked-in generated contracts
cargo xtask check    # fail if contracts drift, then compile/test
cargo xtask build    # refresh SpaceMolt, regenerate, then compile/test everything
cargo xtask build --offline # use the checked-in SpaceMolt specification
```

Ordinary `cargo build` remains crate/workspace-local and does not invoke generation.
CI and release builds use `cargo xtask build`, ensuring every canonical build
starts from the current official SpaceMolt contract.

## License

MIT
