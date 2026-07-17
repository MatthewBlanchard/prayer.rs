import assert from "node:assert/strict";
import test from "node:test";
import { fleetLocation, hasEquippedSurveyScanner, isAvailableForExplore } from "./state.js";

const entry = (state: unknown) => ({ id: "bot", connection: "Connected", in_transit: false, observed_at: null, state, username: null, version: 1 }) as never;

test("detects only equipped survey scanners in state.modules", () => {
  assert.equal(hasEquippedSurveyScanner(entry({ location: {}, modules: [{ type_id: "survey_scanner" }] })), true);
  assert.equal(hasEquippedSurveyScanner(entry({ location: {}, modules: [{ module_id: "instance_t2-survey-scanner_42" }] })), false);
  assert.equal(hasEquippedSurveyScanner(entry({ location: {}, modules: [{ name: "Advanced Survey Scanner" }] })), false);
});

test("reads generated fleet location fields", () => {
  assert.deepEqual(fleetLocation(entry({ location: { system_id: "sol", poi_id: "earth" }, modules: [] })), { systemId: "sol", poiId: "earth" });
});

test("a terminal script record does not leave an explorer permanently busy", () => {
  const state = { location: { system_id: "sol" }, modules: [] };
  assert.equal(isAvailableForExplore({ ...entry(state), script_execution: { id: "old", state: "stopped" } } as never), true);
  assert.equal(isAvailableForExplore({ ...entry(state), script_execution: { id: "active", state: "running" } } as never), false);
});
