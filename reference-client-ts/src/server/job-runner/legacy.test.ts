import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { archiveLegacyJobs } from "./legacy.js";

test("archives legacy squad data without interpreting its conversations", async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "prayer-legacy-jobs-"));
  const source = path.join(directory, "jobs.json");
  await fs.writeFile(source, JSON.stringify({ jobs: [{ messages: ["private transcript"] }] }));
  const archived = await archiveLegacyJobs(source, new Date("2026-07-14T12:00:00.000Z"));
  assert.equal(archived, `${source}.archived-2026-07-14T12-00-00-000Z`);
  await assert.rejects(fs.access(source));
  assert.match(await fs.readFile(archived!, "utf8"), /private transcript/);
});
