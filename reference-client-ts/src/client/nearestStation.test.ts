import assert from "node:assert/strict";
import test from "node:test";
import { findNearestStationPoi } from "./nearestStation.js";

const map = {
  systems: [],
  knownPois: [
    { id: "station_far", systemId: "beta", name: "Far Station", type: "station", x: null, y: null },
    { id: "station_near", systemId: "alpha", name: "Near Station", type: "station", x: null, y: null },
  ],
};

test("returns a concrete same-system station without routing", async () => {
  assert.equal(await findNearestStationPoi(map, "alpha", async () => assert.fail("route lookup should not run")), "station_near");
});

test("chooses the lowest-cost reachable station", async () => {
  const result = await findNearestStationPoi(map, "gamma", async () => [
    { cost: 4 } as never,
    { cost: 2 } as never,
  ]);
  assert.equal(result, "station_near");
});
