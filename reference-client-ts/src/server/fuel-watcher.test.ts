import assert from "node:assert/strict";
import test from "node:test";
import { knownFuelStations, nearestKnownFuelStation } from "./fuel-watcher.js";

const map = {
  knownPois: [
    { id: "empty-local", systemId: "sol", type: "station", hasBase: true },
    { id: "fuel-near", systemId: "alpha", type: "station", hasBase: true },
    { id: "fuel-far", systemId: "beta", type: "station", hasBase: true },
    { id: "asteroid", systemId: "sol", type: "asteroid", hasBase: false },
  ],
};

const markets = {
  "empty-local": { sell_orders: { fuel: [{ quantity: 0 }] } },
  "fuel-near": { sell_orders: { fuel: [{ quantity: 20 }] } },
  "fuel-far": { sell_orders: { fuel: [{ quantity: 50 }] } },
  asteroid: { sell_orders: { fuel: [{ quantity: 100 }] } },
};

test("knownFuelStations requires a station with positive known fuel supply", () => {
  assert.deepEqual(knownFuelStations(map, markets), [
    { id: "fuel-far", systemId: "beta" },
    { id: "fuel-near", systemId: "alpha" },
  ]);
});

test("nearestKnownFuelStation ignores empty and unreachable stations", async () => {
  const selected = await nearestKnownFuelStation("sol", map, markets, async (routes) => routes.map((route) => (route.to === "fuel-near" ? { cost: 3 } : null)));
  assert.equal(selected, "fuel-near");
});

test("nearestKnownFuelStation falls back to the nearest station when no known fuel is available", async () => {
  const noFuelMarkets = {
    "empty-local": { sell_orders: {} },
    "fuel-near": { sell_orders: { fuel: [{ quantity: 0 }] } },
    "fuel-far": { sell_orders: {} },
  };
  const selected = await nearestKnownFuelStation("sol", map, noFuelMarkets, async () => {
    throw new Error("same-system fallback should not need routing");
  });
  assert.equal(selected, "empty-local");
});

test("nearestKnownFuelStation falls back when known-fuel stations are unreachable", async () => {
  const selected = await nearestKnownFuelStation("gamma", map, markets, async (routes) =>
    routes.map((route) => {
      if (route.to === "fuel-near" || route.to === "fuel-far") return null;
      return route.to === "empty-local" ? { cost: 2 } : null;
    }),
  );
  assert.equal(selected, "empty-local");
});
