import assert from "node:assert/strict";
import test from "node:test";
import { decodeServerEvent } from "../src/client/api/events.js";

test("SSE decoder rejects malformed discriminated-union payloads", () => {
  assert.equal(decodeServerEvent({ type: "job_run_updated", run: { id: "partial" } }), null);
  assert.equal(decodeServerEvent({ type: "unknown" }), null);
  assert.equal(decodeServerEvent("state_sync"), null);
});

test("SSE decoder normalizes the browser event name before construction", () => {
  assert.deepEqual(decodeServerEvent({}, "state_sync"), { type: "state_sync" });
});
