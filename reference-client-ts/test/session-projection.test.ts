import assert from "node:assert/strict";
import test from "node:test";
import type { FleetEntry } from "@prayer/sdk";
import { activeRoutePath } from "../src/client/galaxyRoute.js";
import { projectRunningScript, projectSessionLocation } from "../src/client/prayer/sessionProjection.js";
import { selectBotView } from "../src/client/prayer/selectors.js";
import { projectFleetSessions } from "../src/client/prayer/useFleetSessions.js";

function fleetEntry(scriptExecution: FleetEntry["script_execution"] = null): FleetEntry {
  return {
    id: "bot-1",
    username: "ada",
    version: 1,
    observed_at: null,
    connection: "Connected",
    script_execution: scriptExecution,
    active_route: { target: "gamma", targetSystem: "gamma", targetPoi: null, hops: ["beta", "gamma"], totalJumps: 2, estimatedFuelUse: 4 },
    in_transit: true,
    transit_dest_system: "beta",
    transit_dest_poi: null,
    state: {
      fuel_pct: 100,
      fuel: 10,
      max_fuel: 10,
      cargo_pct: 0,
      cargo_used: 0,
      cargo_capacity: 5,
      cargo: {},
      player: {},
      ship: {},
      location: { system_id: "alpha", poi_id: "alpha-base" },
      in_battle: false,
      passengers: { economy_berths: { current: 0, max: 0 }, business_berths: { current: 0, max: 0 }, first_berths: { current: 0, max: 0 } },
      skills: {},
    },
  };
}

test("typed active route reaches the session location projection", () => {
  const location = projectSessionLocation(selectBotView(fleetEntry()));
  assert.deepEqual(location.activeRouteHops, ["beta", "gamma"]);
  assert.equal(location.activeRouteDestSystem, "gamma");
  assert.equal(location.inTransit, true);
});

test("a valid multi-hop route produces a drawable path", () => {
  assert.deepEqual(activeRoutePath("alpha", ["beta", "gamma"], new Set(["alpha", "beta", "gamma"])), ["alpha", "beta", "gamma"]);
});

test("projects running, successful, failed, stopped, and absent script executions", () => {
  const running = projectRunningScript(
    selectBotView(fleetEntry({ id: "e1", script: "go beta;", state: "running", currentLine: 3 } as FleetEntry["script_execution"])),
  );
  assert.deepEqual(running, { script: "go beta;", currentLine: 3, isRunning: true, frameKind: "main", frameName: null });

  for (const outcome of [
    { status: "success" as const, message: "done" },
    { status: "error" as const, kind: "runtime", message: "failed" },
  ]) {
    const stopped = projectRunningScript(
      selectBotView(fleetEntry({ id: "e2", script: "mine;", state: "stopped", lastLine: 5, outcome } as FleetEntry["script_execution"])),
    );
    assert.equal(stopped?.isRunning, false);
    assert.equal(stopped?.currentLine, 5);
  }

  assert.equal(projectRunningScript(selectBotView(fleetEntry({ id: "e3", script: "dock;", state: "stopped" })))?.isRunning, false);
  assert.equal(projectRunningScript(selectBotView(fleetEntry(null))), null);
});

test("fleet projection preserves UI-only session state while refreshing runtime fields", () => {
  const bot = selectBotView(fleetEntry());
  const [initial] = projectFleetSessions([bot], []);
  assert.ok(initial);

  const passengersAboard = [{ id: "passenger-1" }] as typeof initial.passengersAboard;
  const [refreshed] = projectFleetSessions([bot], [{ ...initial, passengersAboard, battleStartedAt: "2026-07-14T00:00:00Z" }]);

  assert.equal(refreshed?.passengersAboard, passengersAboard);
  assert.equal(refreshed?.battleStartedAt, "2026-07-14T00:00:00Z");
  assert.equal(refreshed?.location.system, "alpha");
});
