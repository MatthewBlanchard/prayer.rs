# Maintenance scripts

This directory contains shell entry points for Prayer's workspace maintenance
and log plumbing. Run the user-facing wrappers from any working directory; they
change to the repository root before invoking `cargo xtask`.

| Script | Purpose |
| --- | --- |
| `audit-public-json-value.sh` | Runs `cargo xtask audit-public-api` to find public Rust APIs exposing untyped JSON values. |
| `refresh-spacemolt-v2-docs.sh` | Runs `cargo xtask refresh-spacemolt` and forwards options such as `--guides-only`, `--openapi-only`, `--base-url`, and `--delay`. |
| `show-prayer-logs.sh` | Runs `cargo xtask show-logs`; an optional positional argument selects the number of recent lines. |
| `prayer-prune-logs.sh` | Runs `cargo xtask prune-logs` to enforce log age and total-size limits. |
| `prayer-log-writer.sh` | Internal stdin-to-file logger with rotation; service launch tooling supplies its three required path/name arguments. |

Examples:

```console
./scripts/audit-public-json-value.sh
./scripts/refresh-spacemolt-v2-docs.sh --openapi-only
./scripts/show-prayer-logs.sh 120
./scripts/prayer-prune-logs.sh
```

## Log settings

- `PRAYER_LOG_DIR` changes the log root used by the `xtask` log commands from
  `logs/`.
- `PRAYER_LOG_RETENTION_DAYS` controls age-based pruning (default: `14`).
- `PRAYER_LOG_TOTAL_MB` caps retained run logs (default: `500`).
- `PRAYER_LOG_MAX_FILE_MB` controls the writer's per-file rotation threshold
  (default: `25`). Rotated files are compressed when `gzip` is available.

The wrapper scripts require a POSIX shell; `prayer-log-writer.sh` specifically
requires Bash. The workspace tasks also require Cargo, Node.js, and npm on
`PATH`.
