# `@prayer/sdk`

The high-level TypeScript SDK for Prayer. This package is currently an alpha,
source/tarball distribution; it is not yet published to a public npm registry.

## Install

Build and pack the repository checkout, then install the resulting artifact:

```sh
cd prayer-sdk-ts
npm ci
npm run build
npm pack
npm install /path/to/prayer-sdk-0.1.0-alpha.0.tgz
```

Node.js 22 or newer and npm 10 or 11 are supported. The package is ESM and
uses the standard Fetch, AbortSignal, and Web Crypto APIs available in modern
browsers.

## Quick start

```ts
import { Prayer, wait } from "@prayer/sdk";

const prayer = await Prayer.connect({
  baseUrl: "http://127.0.0.1:7777",
  token: "your-bearer-token",
});
const [summary] = await prayer.bots();
if (!summary) throw new Error("No bots are connected");

const bot = await prayer.bot(summary.botId);
const state = await bot.state(); // immutable, cache-aware aggregate state
console.log(state.id);

const run = await bot.startActions(wait(1));
console.log(run.id, run.prayerlang, run.idempotencyKey);
const terminal = await run.wait({
  onStatus: (snapshot) => console.log(snapshot.status),
});

switch (terminal.status) {
  case "succeeded": console.log("done"); break;
  case "failed": console.error(terminal.outcome.message); break;
  case "cancelled": console.log(terminal.outcome.reason); break;
  case "halted": console.log(terminal.outcome.reason); break;
}

await bot.halt("application shutdown");
```

`Prayer.state()` conditionally refreshes changed domains and returns a deeply
frozen snapshot. Select bots by stable bot id whenever it is available.

## Runs, cancellation, and recovery

Use `startActions(action)` or `startActions([action, ...])` for typed actions,
and `startScript(prayerlang)` for PrayerLang. Handles expose `id`, rendered
`prayerlang`, the current `snapshot`, `status()`, `wait()`, `cancel()`, and an
`errorMessage` convenience over structured terminal outcomes.

Recover after restart with `bot.actionRun(runId)` or `bot.scriptRun(runId)`.
`wait()` returns every terminal status rather than throwing for a run-level
failure. Network and HTTP failures still throw structured SDK errors.

Starts generate a UUID idempotency key by default. Save `run.idempotencyKey`
when durable coordination matters. Retry an uncertain submission with the same
explicit key; use a new key only to intentionally create new work. Recover
known work by run id instead of submitting it again. Blank keys are rejected
before network I/O.

Client-owned interruption policy can enqueue higher-precedence work with
`bot.executeActionOverride(actions)` or
`bot.executeScriptOverride(prayerlang)`. Pass `{ returnToOrigin: true }` for a
best-effort return to the interruption location; the default is `false`.

```ts
const run = await bot.startScript("wait 1;", {
  idempotencyKey: "deployment-42-step-1",
});
await run.cancel("operator request");
```

## Connection and errors

`Prayer.connect` accepts `baseUrl`, bearer `token`, caller `headers`, a custom
`fetch`, and a default `timeoutMs`. Every request/wait accepts a caller abort
`signal` and timeout override.

```ts
import {
  PrayerAbortError,
  PrayerApiError,
  PrayerCompatibilityError,
  PrayerNotFoundError,
  PrayerTimeoutError,
  isRetryableError,
} from "@prayer/sdk";

async function inspectQueue(): Promise<void> {
  try {
    await bot.queue();
  } catch (error) {
    if (error instanceof PrayerAbortError) return;
    if (error instanceof PrayerTimeoutError || isRetryableError(error)) {
      // Retry according to error.retryAfterMs when supplied.
    } else if (error instanceof PrayerNotFoundError) {
      // Refresh bot selection.
    } else if (error instanceof PrayerCompatibilityError) {
      // Upgrade the SDK or use a compatible Prayer API major version.
    } else if (error instanceof PrayerApiError) {
      console.error(error.code, error.requestId, error.details);
    } else throw error;
  }
}
```

Common troubleshooting: `PrayerConnectionError` means the API is unreachable;
`PrayerCompatibilityError` means its major version is unsupported;
`PrayerNotFoundError` means the selector/run no longer exists;
`LaneBusyError` means another run owns the bot lane; and
`PrayerAuthenticationError` means the bearer token is absent or unauthorized.

## Advanced API

Ordinary workflows should use the root facade. The complete generated HTTP
client and wire contracts remain explicit escape hatches:

```ts
import { PrayerApi } from "@prayer/sdk/api";
import type { StateResponse } from "@prayer/sdk/types";
```

Connected facades expose the configured generated client as
`prayer.advanced.api`; the subpath export is available for custom transports.

The alpha package may make breaking convenience/API changes between releases.
The generated HTTP contracts target Prayer API major version 1. Root exports,
the `/api` and `/types` subpaths, serialized actions, and run contracts become
semver-governed when the package leaves alpha.
