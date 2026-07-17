# Prayer reference client

This package is Prayer's web control room and a reference consumer of
`@prayer/sdk`. It combines a Vite/React frontend with an Express server for
fleet sessions, squads, executable jobs, routing, plugins, and server-sent
updates.

It is an example operator application rather than the reusable SDK. Applications
that only need Prayer's typed client should depend on `../prayer-sdk-ts`.

## Run locally

The client requires Node.js 22 or newer and npm 10 or 11. It also expects the
Prayer API at `http://127.0.0.1:7777` by default. From the repository root, the
workspace runner installs dependencies, builds the local SDK, starts the API,
and launches both client development servers:

```console
cargo xtask bootstrap
cargo xtask run --client web
```

The Vite frontend runs at `http://127.0.0.1:5173` and proxies API and event
requests to the Express server on port `3001`.

To work on this package directly after bootstrap:

```console
cd reference-client-ts
npm run dev
```

## Configuration

The Express server accepts `--port` and `--prayer-api-url` command-line options.
Equivalent environment variables and persistence overrides are:

| Variable                      | Default                                                 |
| ----------------------------- | ------------------------------------------------------- |
| `PRAYER_CLIENT_PORT`          | `3001`                                                  |
| `PRAYER_CLIENT_API_URL`       | `http://127.0.0.1:7777`                                 |
| `PRAYER_CLIENT_JOBS_PATH`     | `.prayer-client-jobs.json` in the working directory     |
| `PRAYER_CLIENT_JOB_RUNS_PATH` | `.prayer-client-job-runs.json` in the working directory |
| `PRAYER_CLIENT_SQUADS_PATH`   | `.prayer-client-squads.json` in the working directory   |

For example:

```console
npm run dev:server -- --port 3002 --prayer-api-url http://127.0.0.1:7777
```

See [`plugins/README.md`](plugins/README.md) for the plugin manifest and runtime
contract.

## Checks and production build

```console
npm test
npm run typecheck
npm run lint
npm run build
npm run check
```

`npm run build` emits the frontend, server, and plugin runtime under `dist/`.
After building, run it with `npm start`; the production Express server serves
the compiled frontend as well as its API routes.
