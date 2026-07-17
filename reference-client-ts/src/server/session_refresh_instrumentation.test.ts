import assert from "node:assert/strict";
import test from "node:test";

import { recordSessionRefresh, sessionRefreshContext, withSessionRefreshMetrics } from "./session_refresh_instrumentation.js";

test("refresh metrics retain async operation context and aggregate endpoint timings", async () => {
  const measured = await withSessionRefreshMetrics({ workflow: "auto-arb", cycle: 3, sessionHandle: "silver", operation: "scan" }, async () => {
    await Promise.resolve();
    assert.deepEqual(sessionRefreshContext(), {
      workflow: "auto-arb",
      cycle: 3,
      sessionHandle: "silver",
      operation: "scan",
      metrics: { count: 0, totalMs: 0, botsMs: 0, stateMs: 0 },
    });
    recordSessionRefresh({ totalMs: 90, botsMs: 30, stateMs: 80 });
    recordSessionRefresh({ totalMs: 40, botsMs: 25, stateMs: 35 });
    return "done";
  });

  assert.equal(measured.result, "done");
  assert.deepEqual(measured.metrics, {
    count: 2,
    totalMs: 130,
    botsMs: 55,
    stateMs: 115,
  });
  assert.equal(sessionRefreshContext(), undefined);
});

test("parallel refresh scopes do not mix metrics", async () => {
  const [scan, reserve] = await Promise.all([
    withSessionRefreshMetrics({ operation: "scan" }, async () => {
      await Promise.resolve();
      recordSessionRefresh({ totalMs: 10, botsMs: 4, stateMs: 9 });
    }),
    withSessionRefreshMetrics({ operation: "reserve" }, async () => {
      recordSessionRefresh({ totalMs: 20, botsMs: 8, stateMs: 18 });
    }),
  ]);

  assert.deepEqual(scan.metrics, { count: 1, totalMs: 10, botsMs: 4, stateMs: 9 });
  assert.deepEqual(reserve.metrics, { count: 1, totalMs: 20, botsMs: 8, stateMs: 18 });
});
