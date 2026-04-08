# Prayer MCP + EffectiveState Virtual Filesystem Plan

## Objective
Create a new Rust crate, `prayer-mcp`, that runs as an MCP server and proxies LLM requests through `prayer-api` into SpaceMolt.

The LLM should not call SpaceMolt directly. It should:
1. Manage Prayer sessions.
2. Author and run PrayerLang scripts.
3. Inspect EffectiveState through a virtual filesystem and searchable MCP interface.

## High-Level Architecture

```text
LLM Client (Claude, etc.)
      ↓  MCP (stdio or HTTP/SSE)
  prayer-mcp  (new)
      ↓  HTTP (reqwest)
  prayer-api  (existing)
      ↓  RuntimeTransport HTTP
  SpaceMolt API
```

## Scope

### In Scope
- New crate: `prayer.rs/prayer-mcp`
- MCP server loop with `rmcp`.
- Session/script/state tool surface mapped to existing Prayer API routes.
- MCP resources that expose session state and docs.
- Virtual filesystem projection of EffectiveState.
- Filesystem-style search tools (`find`, `grep`, `read`) over projected state.

### Out of Scope
- No re-implementation of runtime engine behavior.
- No direct SpaceMolt HTTP calls from `prayer-mcp`.
- No duplicate business logic that already exists in `prayer-runtime` or `prayer-api`.

## Workspace Changes
Update `prayer.rs/Cargo.toml`:
- Add `prayer-mcp` to `[workspace].members`.

Create new crate layout:

```text
prayer-mcp/
  Cargo.toml
  src/
    main.rs
    server.rs
    client.rs
    tools.rs
    resources.rs
    vfs.rs
    dsl_ref.rs
```

Notes:
- `vfs.rs` is added for virtual filesystem projection/indexing.
- `dsl_ref.rs` is added for generating/serving `prayer://dsl/reference`.

## Runtime Configuration
`prayer-mcp` startup options:
- `--prayer-url` (default: `http://127.0.0.1:7777`)
- `--transport stdio|sse` (default: `stdio`)
- `--bind` (default: `127.0.0.1:5000`, used when transport is `sse`)
- `--mcp-path` (default: `/mcp`, used when transport is `sse`)
- `--request-timeout-ms` (default: `30000`)

Environment aliases (optional):
- `PRAYER_URL`
- `PRAYER_MCP_TRANSPORT`
- `PRAYER_MCP_BIND`
- `PRAYER_MCP_PATH`

## Dependencies

Core:
- `rmcp` (official Rust MCP SDK)
- `tokio`
- `serde`, `serde_json`
- `reqwest`
- `thiserror`
- `tracing`, `tracing-subscriber`
- `clap` (or equivalent) for startup args

Optional:
- `regex` for grep pattern support
- `globset` for path filtering

## Module Responsibilities

### `main.rs`
- Parse CLI/env config.
- Initialize logging.
- Build runtime config and delegate startup to `server.rs`.

### `server.rs`
- MCP server wiring.
- Register tools/resources.
- Route MCP method handlers to `tools.rs` / `resources.rs`.

### `client.rs`
Thin wrapper around `prayer-api` endpoints:
- `create_session`
- `list_sessions`
- `load_script`
- `execute_script`
- `halt_session`
- `get_state`
- `get_status`
- `get_snapshot`
- `passthrough`
- Galaxy/station endpoints where needed by VFS leaf reads

### `tools.rs`
- Typed input/output structs for MCP tools.
- Validation and error shaping.
- Calls into `PrayerApiClient`.

### `resources.rs`
- URI parsing and resource dispatch.
- Directory/listing generation.
- Leaf reads from VFS or direct `prayer-api` endpoints.

### `vfs.rs`
- Convert EffectiveState into deterministic virtual paths.
- Build text index per file path for grep/find.
- Cache by `(session_id, state_version)`.

### `dsl_ref.rs`
- Generate DSL reference text/JSON for `prayer://dsl/reference`.
- Prefer dynamic derivation from runtime catalog metadata.

## MCP Tool Surface

### Core Tools

1. `create_session`
- Prayer API: `POST /api/runtime/sessions`
- Input:
  - `username: string`
  - `password: string`
  - `label?: string`
- Output:
  - Created session summary.

2. `list_sessions`
- Prayer API: `GET /api/runtime/sessions`
- Input: none
- Output:
  - Array of session summaries.

3. `load_script`
- Prayer API: `POST /api/runtime/sessions/:id/script`
- Input:
  - `session_id: string`
  - `script: string`
- Output:
  - Normalized script response/ack.

4. `execute_script`
- Prayer API: `POST /api/runtime/sessions/:id/script/execute`
- Input:
  - `session_id: string`
  - `max_steps?: integer`
- Output:
  - `steps_executed`, `halted`, `completed`.

5. `halt_session`
- Prayer API: `POST /api/runtime/sessions/:id/halt`
- Input:
  - `session_id: string`
  - `reason?: string`
- Output:
  - Halt ack.

6. `get_state`
- Prayer API: `GET /api/runtime/sessions/:id/state`
- Input:
  - `session_id: string`
  - `since?: integer`
  - `wait_ms?: integer`
- Output:
  - Full `RuntimeStateResponse`.

7. `passthrough`
- Prayer API: `POST /api/runtime/sessions/:id/spacemolt/passthrough`
- Input:
  - `session_id: string`
  - `command: string`
  - `payload?: object`
- Output:
  - Passthrough execution result.

Passthrough policy:
- Kept enabled for now.
- Always log `session_id`, `command`, and payload size.

### Virtual Filesystem Query Tools

8. `fs_read`
- Purpose: read one virtual file directly.
- Input:
  - `session_id: string`
  - `path: string`
- Output:
  - File content + content type.

9. `fs_query` (pipeline-style)
- Purpose: shell-like composable querying without exposing arbitrary command execution.
- Input:
  - `session_id: string`
  - `pipeline: string`
  - `max_results?: integer`
- Output:
  - Structured result rows plus metadata (`truncated`, `result_count`, `state_version`).

Initial pipeline stages:
- `find <glob>`
- `grep <pattern>`
- `read`
- `project <field[,field...]>`
- `sort <field> [asc|desc]`
- `unique <field>`
- `limit <n>`

Example pipelines:
- `find **/missions/*.json | grep turn in | limit 20`
- `find **/market/orders/*.jsonl | read | project item_id,price_each,quantity | sort price_each desc | limit 50`

Constraints:
- Stage count and total pipeline length are hard-limited.
- No arbitrary shell, subprocesses, filesystem passthrough, or external network calls.
- Same hard return-size policy as `fs_read` (oversize => error + error log).

## MCP Resource Surface

### Required Resources

1. `prayer://sessions`
- Content: JSON array of active sessions.
- Source: `GET /api/runtime/sessions`

2. `prayer://sessions/{id}/state`
- Content: full `RuntimeStateResponse` JSON.
- Source: `GET /api/runtime/sessions/:id/state`

3. `prayer://sessions/{id}/status`
- Content: status line history.
- Source: `GET /api/runtime/sessions/:id/status`

4. `prayer://dsl/reference`
- Content: PrayerLang reference for commands, arg kinds, predicates.
- Source preference:
  - Derived from runtime catalog metadata.
  - If needed, via a dedicated Prayer API endpoint added later.

### Virtual Filesystem Resources
Expose EffectiveState as browsable resource URIs rooted at:
- `prayer://sessions/{id}/fs/`

This document intentionally stays high-level. Detailed path taxonomy and full field-by-field mapping live in:
- `docs/game-state-vfs-mapping.md`

VFS implementation requirements (summary):
- Deterministic path naming and directory ordering.
- Session-scoped virtual surface only (never host filesystem passthrough).
- `fs_query`/`fs_read` operate on projected VFS content.
- Version-aware caching keyed by session + state version.
- Hard-limited responses for search/read operations to protect token budgets.

### Performance and Size Limits

Recommended limits:
- `max_virtual_files`: 512
- `max_file_bytes`: 1 MiB per rendered file
- `max_total_index_bytes`: 16 MiB per session cache entry
- `max_read_return_bytes`: 262_144 (hard limit on `fs_read` response payload)
- `max_query_return_bytes`: 262_144 (hard limit on `fs_query` response payload)
- `max_query_rows`: 200 default, hard cap 1000

Handling large payloads:
- Prefer summary JSON plus sibling `*.jsonl` for large arrays.
- Do not return oversized responses.
- If a read/search result exceeds hard limits:
  - Emit an error-level log entry with `session_id`, operation (`fs_read`/`fs_query`), pipeline or requested path, estimated bytes, and configured limit.
  - Return a structured MCP error (`result_too_large`) that includes limit metadata and a suggestion to narrow scope.

### Example Agent Workflow

Example goal: \"find a mission turn-in target quickly\"

1. `fs_query(session_id, \"find **/missions/*.json | grep turn in | limit 20\")`
2. `fs_read(session_id, \"/missions/active.json\")`
3. `fs_query(session_id, \"find **/position/*.json | read | limit 10\")`
4. Generate script and run through `load_script` + `execute_script`.

Why this helps:
- The model can discover/filter/project in one composable query, then read only the minimum needed payload.

## EffectiveState Projection Strategy

Source of truth:
- `GET /api/runtime/sessions/:id/state` (returns `RuntimeStateResponse` + headers)

Projection pipeline:
1. Fetch state snapshot.
2. Read `X-Prayer-State-Version` header.
3. If unchanged for session, reuse cached VFS.
4. If changed, rebuild VFS path map and text index.

VFS cache key:
- `(session_id, state_version)`

In-memory model:
- `HashMap<VirtualPath, VirtualNode>`
- `VirtualNode::Dir { children }` or `VirtualNode::File { mime, text, bytes? }`

Search index:
- `HashMap<VirtualPath, Vec<String>>` line-split text for grep.
- Optional token pre-index if needed later.

## URI + Path Rules

Normalization:
- Paths always absolute within virtual root and lowercase.
- Use `/` separators.
- No `..` traversal.
- Invalid unicode/control characters in paths are rejected.
- Multiple slashes are normalized (`//` -> `/`).
- Dot-segments are rejected rather than resolved (`.` or `..`).

Not found behavior:
- Missing session: MCP error mapped from Prayer API 404.
- Missing virtual path: MCP not-found error.
- Undocked station leaves:
  - Return a small explanatory JSON file for `context.json`.
  - Return not-found for docked-only leaves (`market.json`, etc.) if no station context.

## Error Handling

Map Prayer API errors to MCP errors with structured detail:
- `400` -> invalid params
- `404` -> session/resource not found
- `409` -> conflict
- `429` -> rate-limited/backoff message
- `5xx` -> upstream service failure

Include:
- `status_code`
- `endpoint`
- `request_id` (if available)

## Observability

Structured logs:
- transport mode
- mcp method
- tool/resource name
- session id
- latency ms
- upstream status code

Passthrough logs:
- command name
- payload byte length
- result status (success/failure)

Metrics (future):
- tool call counts
- query call counts
- cache hit/miss per session
- state rebuild latency

## Implementation Phases

### Phase 1: Crate + Core MCP Loop
- Scaffold crate and workspace entry.
- Stand up rmcp server with stdio transport.
- Implement `list_sessions` + `create_session`.

### Phase 2: Core Script Loop
- Implement `load_script`, `execute_script`, `halt_session`, `get_state`, `passthrough`.
- Add robust error mapping and logging.

### Phase 3: Baseline Resources
- Implement:
  - `prayer://sessions`
  - `prayer://sessions/{id}/state`
  - `prayer://sessions/{id}/status`
  - `prayer://dsl/reference`

### Phase 4: Virtual Filesystem
- Implement VFS projection from EffectiveState.
- Expose `prayer://sessions/{id}/fs/...` tree.
- Add cached rebuild keyed by state version.

### Phase 5: Search Tools
- Implement `fs_query`, `fs_read`.
- Add guardrails (`max_query_rows`, return-size hard limits).
- Tune output schema for agent usability.

### Phase 6: SSE Transport
- Add HTTP/SSE transport mode and `/mcp` path.
- Validate compatibility with existing clients that expect `http://localhost:5000/mcp`.

## Testing Plan

### Unit Tests
- URI parsing and normalization.
- Path projection correctness.
- query parsing/execution logic.
- error mapping.

### Contract Tests (mocked prayer-api)
- Tool input/output mapping.
- Resource read/list behavior.
- 404/429/500 handling.

### Integration Tests
- Boot `prayer-api` locally.
- Run `prayer-mcp` in stdio mode.
- Validate sequence:
  - initialize
  - tools/list
  - create_session
  - load_script
  - execute_script
  - read fs resources
  - fs_query

### Manual Smoke
- Claude Desktop/Code config with stdio.
- SSE config against `/mcp` endpoint.

## Security and Safety Notes

- `passthrough` intentionally remains available; treat as privileged escape hatch.
- Never expose local host filesystem through these tools/resources.
- VFS is synthetic and session-scoped only.
- Validate all user-provided path/glob/pattern inputs for size and complexity.

## Future Enhancements

- Add recipe catalog route if/when API exposes it (`/galaxy/catalog/recipes`).
- Add `fs_stat` for metadata (size, mime, version).
- Add incremental index updates instead of full rebuild.
- Add server-side pagination for very large result sets.
- Consider exposing `snapshot` as lightweight resource for polling efficiency.

## Acceptance Criteria

1. `prayer-mcp` runs in stdio mode and responds to MCP initialize/tools/resources calls.
2. Core tool loop works end-to-end through `prayer-api` (`load_script` + `execute_script`).
3. EffectiveState can be browsed as stable virtual URIs.
4. Bot can discover/filter state via `fs_query` and inspect exact paths via `fs_read` without raw giant JSON prompting.
5. No direct SpaceMolt call path exists in `prayer-mcp`.
