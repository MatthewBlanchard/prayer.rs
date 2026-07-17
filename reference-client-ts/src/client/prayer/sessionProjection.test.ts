import assert from "node:assert/strict";
import test from "node:test";
import type { BotView } from "./selectors.js";
import { projectRunningScript } from "./sessionProjection.js";

function botWithExecution(scriptExecution: BotView["scriptExecution"]): BotView {
  return { scriptExecution } as BotView;
}

test("session script projection uses canonical camel-case current line", () => {
  const projected = projectRunningScript(
    botWithExecution({
      id: "run",
      runId: "run",
      script: "go alpha;\nwait 1;",
      state: "running",
      currentLine: 2,
      lastLine: 1,
      outcome: null,
    }),
  );

  assert.equal(projected?.script, "go alpha;\nwait 1;");
  assert.equal(projected?.currentLine, 2);
  assert.equal(projected?.isRunning, true);
});

test("session script projection falls back to the canonical last line", () => {
  const projected = projectRunningScript(
    botWithExecution({
      id: "run",
      script: "wait 1;",
      state: "stopped",
      currentLine: null,
      lastLine: 1,
      outcome: { status: "success", message: null },
    }),
  );

  assert.equal(projected?.currentLine, 1);
  assert.equal(projected?.isRunning, false);
});

test("session script projection preserves the override frame", () => {
  const projected = projectRunningScript(
    botWithExecution({
      id: "run",
      script: "go ramens_rest;\nrefuel;",
      state: "running",
      currentLine: 1,
      lastLine: null,
      outcome: null,
      frameKind: "override",
      frameName: null,
    }),
  );

  assert.equal(projected?.frameKind, "override");
  assert.equal(projected?.script, "go ramens_rest;\nrefuel;");
});
