import assert from "node:assert/strict";
import test from "node:test";
import type { JobDefinition } from "../src/shared/types.js";
import { buildJobConfig } from "../src/client/JobsPanel.js";

const definition = {
  kind: "mine",
  title: "Mine",
  description: "Mine ore",
  mode: "continuous",
  fields: [
    { name: "botIds", label: "Bots", type: "text", required: true },
    { name: "resourceId", label: "Resource", type: "text", required: true },
    { name: "limit", label: "Limit", type: "number" },
  ],
  defaults: { limit: 5 },
  capabilities: [],
} satisfies JobDefinition;

test("job config builder validates persisted draft fields", () => {
  assert.deepEqual(buildJobConfig(definition, { resourceId: "iron", limit: 8 }, ["bot-1"]), {
    kind: "mine",
    botIds: ["bot-1"],
    resourceId: "iron",
    limit: 8,
  });
  assert.equal(buildJobConfig(definition, { resourceId: "iron", limit: "eight" }, ["bot-1"]), null);
  assert.equal(buildJobConfig(definition, { resourceId: "" }, ["bot-1"]), null);
});
