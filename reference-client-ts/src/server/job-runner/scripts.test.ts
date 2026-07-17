import assert from "node:assert/strict";
import test from "node:test";
import { scriptFor } from "./scripts.js";

test("builds canonical navigation and bounded mining PrayerLang", () => {
  assert.equal(scriptFor({ kind: "navigate", botIds: ["a"], destination: "poi-1" }), "go poi-1;");
  assert.match(
    scriptFor({ kind: "mine", botIds: ["a"], resourceId: "iron", quantity: 50, disposition: "storage", destinationPoi: "base" }),
    /transfer from cargo to storage;/,
  );
  assert.match(
    scriptFor({ kind: "mine", botIds: ["a"], resourceId: "iron", quantity: 50, disposition: "storage", destinationPoi: "base", storageTarget: "faction" }),
    /transfer from cargo to faction;/,
  );
});

test("rejects script injection in generated token fields", () => {
  assert.throws(
    () => scriptFor({ kind: "navigate", botIds: ["a"], destination: "home; self_destruct" }),
    /explicit literal PrayerLang identifier/,
  );
});
