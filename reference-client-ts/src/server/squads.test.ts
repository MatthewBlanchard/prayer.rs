import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { SquadStore } from "./squads.js";

test("migrates legacy job metadata into squads without conversation state", async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "prayer-squads-"));
  const legacy = path.join(directory, "jobs.json");
  await fs.writeFile(
    `${legacy}.archived-2026-01-01`,
    JSON.stringify({ jobs: [{ id: "miners", name: "Miners", color: "#112233", priority: 4, sessionHandles: ["Ada", "ada"], messages: ["ignored"] }] }),
  );
  const store = new SquadStore(path.join(directory, "squads.json"), legacy);
  await store.load();
  assert.deepEqual(
    store.list().map(({ id, name, color, priority, botIds }) => ({ id, name, color, priority, botIds })),
    [{ id: "miners", name: "Miners", color: "#112233", priority: 4, botIds: ["Ada", "ada"] }],
  );
  const updated = await store.update("miners", { botIds: ["bot-a"] });
  assert.deepEqual(updated.botIds, ["bot-a"]);
  assert.doesNotMatch(await fs.readFile(path.join(directory, "squads.json"), "utf8"), /messages|ignored/);
});
