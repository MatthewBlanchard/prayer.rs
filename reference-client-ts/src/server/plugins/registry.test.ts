import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { discoverPlugins, validateManifest } from "./registry.js";

const valid = { id: "sample", version: "1.2.3", hostApiVersion: 1, server: "./server.mjs", capabilities: ["jobs"] };

test("validates the plugin manifest contract", () => {
  assert.equal(validateManifest(valid).id, "sample");
  assert.throws(() => validateManifest({ ...valid, id: "Bad ID" }), /invalid plugin ID/);
  assert.throws(() => validateManifest({ ...valid, hostApiVersion: 2 }), /incompatible host API version/);
  assert.throws(() => validateManifest({ ...valid, server: "../escape.mjs" }), /invalid server entry/);
});

test("host starts with no plugins directory", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "prayer-plugins-empty-"));
  const registry = await discoverPlugins(path.join(root, "missing"));
  assert.deepEqual(registry.plugins, []);
});

test("discovers a dropped-in job plugin", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "prayer-plugins-install-"));
  const folder = path.join(root, "sample");
  await mkdir(folder);
  await writeFile(path.join(folder, "plugin.json"), JSON.stringify(valid));
  await writeFile(path.join(folder, "server.mjs"), "export default { jobs: [{ definition: { kind: 'sample_job', title: 'Sample', description: '', mode: 'one_shot', fields: [], defaults: {}, capabilities: [] }, execute: async () => {} }] };\n");
  const registry = await discoverPlugins(root);
  assert.equal(registry.job("sample_job")?.definition.title, "Sample");
});

test("rejects duplicate IDs before loading plugin code", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "prayer-plugins-duplicate-"));
  for (const folderName of ["one", "two"]) {
    const folder = path.join(root, folderName);
    await mkdir(folder);
    await writeFile(path.join(folder, "plugin.json"), JSON.stringify(valid));
  }
  await assert.rejects(discoverPlugins(root), /duplicate plugin ID/);
});

test("discovers Explore with automatic target selection", async () => {
  const registry = await discoverPlugins(path.resolve("plugins/ExploreExample/.."));
  const explore = registry.job("explore");
  assert.equal(explore?.definition.mode, "continuous");
  assert.equal(explore?.definition.fields.some((field) => field.name === "selection"), false);
  assert.equal(explore?.validate?.({ kind: "explore", botIds: ["bot"], selection: "oldest_visit" }).selection, undefined);
  assert.throws(() => explore?.validate?.({ kind: "explore", botIds: ["bot"], strongholdExclusionHops: -1 }), /non-negative integer/);
});
