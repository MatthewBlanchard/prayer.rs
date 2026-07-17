# prayer-api

`prayer-api` exposes the embedded `prayer-sdk` runtime as an Axum HTTP service.
Its versioned API supports bot discovery and registration, state and route
queries, queued typed-action and PrayerLang runs, overrides, cancellation, and
administrative virtual-market, crafting, and inventory-movement resources.

## Run locally

Set a SpaceMolt Clerk key and start the server from the repository root:

```console
export SPACEMOLT_CLERK_API_KEY="..."
cargo run -p prayer-api
```

The default listener is `127.0.0.1:7777`. Verify it with:

```console
curl http://127.0.0.1:7777/health
curl http://127.0.0.1:7777/api/v1/meta
```

Important configuration:

- `PRAYER_RS_BIND` changes the listener address. A non-loopback address requires
  `PRAYER_API_TOKEN`.
- `PRAYER_API_TOKEN` enables `Authorization: Bearer <token>` authentication on
  `/api/v1` routes.
- `PRAYER_SPACEMOLT_BASE_URL` changes the upstream origin.
- `PRAYER_KNOWLEDGE_STATE_PATH`, `PRAYER_SESSION_STATE_PATH`, and
  `PRAYER_V1_IDEMPOTENCY_PATH` override durable state locations.
- `RUST_LOG` controls tracing output.

Mutating administrative routes require an `Idempotency-Key` header. Action and
script run creation also persists idempotency records for safe retries.

## API contract

The service builds an OpenAPI 3.1 document directly from its maintained route
and schema definitions. Regenerate the committed document with:

```console
cargo run -p prayer-api --bin generate-openapi
```

Pass a path as the final argument to write somewhere other than
`prayer-api/openapi/prayer-v1.json`.

## Development

```console
cargo test -p prayer-api
cargo check -p prayer-api
```

Handlers adapt HTTP contracts to `PrayerSdk`/`PrayerAdministration`; they do not
call SpaceMolt session transport directly.
