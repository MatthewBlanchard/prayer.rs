import assert from "node:assert/strict";
import test from "node:test";
import { parseJobConfig } from "./validation.js";

test("applies catalog defaults and deduplicates stable bot IDs", () => {
  const config = parseJobConfig({ kind: "navigate", botIds: ["bot-a", "bot-a"], destination: "Sol" });
  assert.deepEqual(config.botIds, ["bot-a"]);
  if (config.kind !== "navigate") throw new Error("wrong config kind");
  assert.equal(config.destination, "Sol");
});

test("validates required and positive fields", () => {
  assert.throws(() => parseJobConfig({ kind: "navigate", botIds: [], destination: "Sol" }), /bot ID/);
  assert.throws(() => parseJobConfig({ kind: "mine", botIds: ["a"], resourceId: "iron", miningPoi: "iron_belt", quantity: 0 }), /greater than zero/);
});
