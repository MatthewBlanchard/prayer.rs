# run_script Diff Return Plan

## Goal
Add a `diff` section to MCP `run_script` results so callers can see what changed during script execution without manually fetching and comparing snapshots.

## Current State
- `run_script` currently returns:
  - `session`
  - `script` (normalized)
  - `status`
  - `steps`
- Diff-style logic exists in `prayer-api` service internals, but is not surfaced through MCP `run_script`.

## Desired Output Shape
Extend `run_script` payload with:

```json
{
  "session": "bot-a",
  "script": "mine iron_ore;",
  "status": "completed",
  "steps": 3,
  "diff": {
    "credits": { "before": 100, "after": 120, "delta": 20 },
    "fuel": { "before": 50, "after": 45, "delta": -5 },
    "cargo": [
      { "item": "iron_ore", "before": 0, "after": 2, "delta": 2 }
    ],
    "storage": [
      { "item": "iron_ore", "before": 10, "after": 12, "delta": 2 }
    ],
    "flags": {
      "docked_before": false,
      "docked_after": true,
      "halted_after": false
    }
  }
}
```

Notes:
- Keep `diff` stable and deterministic (sorted keys/items).
- Include fields only when both before/after values are known.
- Keep existing fields for backward compatibility.
- Use `storage` (not `stash`) in the diff output for clarity to callers; internal code uses `stash`/`stash_deltas` — translate at the boundary.
- Always emit `"diff": {}` for no-op/empty scripts rather than omitting the field — callers can rely on its presence unconditionally.

## Implementation Plan

1. Define diff contract in API layer
- Add/extend response DTOs in `prayer-api/src/contracts.rs` for script execution diff.
- Include scalar deltas (credits/fuel), inventory deltas (cargo/storage), and useful flags.

2. Compute before/after around execute
- In `prayer-api/src/service.rs`, snapshot state immediately before the step loop in `execute_script` (via `state_snapshot_with_version` or equivalent — confirm this does not conflict with the session lock held inside `execute_step`).
- The existing `diff_positive_item_deltas` helper only emits positive gains and cannot be reused directly. Write a new `diff_item_deltas` helper that returns full `before`/`after`/`delta` for all items with any change (positive or negative). This is the main implementation risk — budget accordingly.
- Extract shared scalar diff for credits and fuel alongside the item helpers.
- Ensure no panic on missing/null state (treat absent fields as zero).

3. Return diff from runtime execute endpoint
- Adding `diff` to `ExecuteScriptResponse` in `contracts.rs` is sufficient — `execute_script_v2` in `routes.rs` returns `Json(result)` directly and will propagate the new field automatically. No route handler changes needed.

4. Thread diff through MCP client — no-op
- `client.rs::execute_script` returns the raw `Value` from `post_empty_with_endpoint` with no field filtering. Diff will pass through transparently. No changes needed here.

5. Extend MCP `run_script` tool output
- In `prayer-mcp/src/tools.rs` `run_script`, include `diff` from execute response in final JSON output.
- Keep normalized script behavior unchanged.

6. Optional client rendering (nice-to-have)
- In `prayer-mcp-client-ts`, optionally display compact diff summary in tool result card.
- Scope separately from backend correctness to avoid blocking core delivery.

## Edge Cases
- Step-limit exits (`status: "step limit reached"`): still return partial diff.
- Halted runs: still return diff from executed portion.
- Runtime/transport error before execute: `diff` omitted (or `null`) and status error unchanged.
- Empty/no-op scripts: always emit `"diff": {}` — do not omit.
- Items present before but absent after (fully consumed): include with `after: 0, delta: <negative>`.
- Concurrent session lock: pre-loop snapshot must not hold the session lock across the full execute loop — take a clone of state and release before entering the step loop.

## Testing Plan

1. Unit tests (API/service)
- Credits/fuel delta correctness.
- Cargo/storage additions/removals and zero-delta filtering.
- Deterministic ordering of item deltas.

2. Integration tests (API route)
- Execute script endpoint returns new `diff` object with expected schema.
- Backward compatibility: existing fields unchanged.

3. MCP tool tests
- `run_script` output includes `diff` when available.
- Error and halt statuses still formatted correctly.

4. Manual smoke
- Run a mining script and verify cargo/stash/fuel changes appear in `diff`.
- Run a no-op/halt script and verify behavior is sane and documented.

## Rollout
- Phase 1: API + MCP backend support (ship first).
- Phase 2: TS client UX polish for displaying `diff`.
- Phase 3: Prompt guidance update to encourage model to summarize `diff` in responses when useful.

## Acceptance Criteria
- `run_script` returns `diff` for successful/halted/step-limited runs.
- Existing consumers depending on current fields do not break.
- Tests cover core delta math and output schema stability.
- Documentation updated to describe the new `diff` field.
