# Workspace tasks

The `xtask` crate is Prayer's cross-platform workspace task runner. Invoke it
from the repository root as `cargo xtask <command>`; it changes to the workspace
root before doing any work.

Rust 1.78 or newer, Node.js 22 or newer, and npm 10 or 11 must be available on
`PATH` for every command.

## Commands

| Command | Purpose |
| --- | --- |
| `cargo xtask bootstrap` | Install locked dependencies for the TypeScript SDK and reference client, then build the SDK. |
| `cargo xtask generate` | Regenerate Prayer's OpenAPI document and generated TypeScript SDK files. |
| `cargo xtask check` | Verify generated files are current, then build, test, and document the Rust and TypeScript workspace. |
| `cargo xtask build` | Refresh the SpaceMolt OpenAPI specification, regenerate contracts, and run the complete checks. |
| `cargo xtask build --offline` | Run the complete build with the checked-in SpaceMolt specification. |
| `cargo xtask run` | Build and run the Prayer API on `127.0.0.1:7777`. |
| `cargo xtask run --client web` | Run the API plus the reference client's development servers. |
| `cargo xtask audit-public-api` | Report public Rust APIs that expose `serde_json::Value`. |
| `cargo xtask show-logs [LINES]` | Show recent API/client logs and the largest archived run logs; defaults to 80 lines. |
| `cargo xtask prune-logs` | Remove old run logs and enforce the configured total-size limit. |
| `cargo xtask refresh-spacemolt` | Refresh the checked-in v2 guides and OpenAPI specification. |

Use `cargo xtask <command> --help` for command-specific options. Notable refresh
options are `--guides-only`, `--openapi-only`, `--base-url <URL>`, and
`--delay <SECONDS>`.

## Configuration

- `SPACEMOLT_CLERK_API_KEY` is needed to connect live services, but not to
  bootstrap, compile, or test.
- `PRAYER_LOG_DIR` changes the log root from the repository's `logs/` directory.
- `PRAYER_LOG_RETENTION_DAYS` controls age-based pruning (default: `14`).
- `PRAYER_LOG_TOTAL_MB` caps retained run logs (default: `500`).

Start a fresh checkout with:

```console
cargo xtask bootstrap
cargo xtask check
```
