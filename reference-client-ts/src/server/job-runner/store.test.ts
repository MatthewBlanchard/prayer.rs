import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { JobRunStore } from "./store.js";
import type { JobRun } from "./types.js";

test("serializes rapid writes and reloads the newest revision", async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "job-runs-"));
  const file = path.join(directory, "runs.json");
  const store = new JobRunStore(file);
  const run: JobRun = {
    id: "one",
    squadId: "squad-one",
    squadName: "Squad One",
    kind: "navigate",
    config: { kind: "navigate", botIds: ["a"], destination: "Sol" },
    status: "queued",
    phase: "queued",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    summary: {},
    botStates: {},
    events: [],
    revision: 1,
  };
  const first = store.put(run);
  run.revision = 2;
  const second = store.put(run);
  await Promise.all([first, second]);
  const loaded = new JobRunStore(file);
  await loaded.load();
  assert.equal(loaded.get("one")?.revision, 2);
});
