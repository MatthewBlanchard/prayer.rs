import assert from "node:assert/strict";
import test from "node:test";
import { readVersionedStoredStringSet, writeVersionedStored } from "../src/client/persistence.js";

test("versioned persistence migrates legacy values and writes an envelope", () => {
  const values = new Map<string, string>([["tracked", JSON.stringify(["alpha"])]]);
  const localStorage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  };
  Object.defineProperty(globalThis, "window", { configurable: true, value: { localStorage } });

  assert.deepEqual([...readVersionedStoredStringSet("tracked")], ["alpha"]);
  assert.equal(writeVersionedStored("tracked", 1, ["beta"]), true);
  assert.deepEqual(JSON.parse(values.get("tracked")!), { version: 1, data: ["beta"] });

  values.set("tracked", JSON.stringify({ version: 2, data: ["future"] }));
  assert.deepEqual([...readVersionedStoredStringSet("tracked", 1)], []);

  Reflect.deleteProperty(globalThis, "window");
});
